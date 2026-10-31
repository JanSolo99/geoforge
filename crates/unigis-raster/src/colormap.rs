//! Raster colormaps as ArcGIS's Add Colormap stores them: an 8-bit unsigned GeoTIFF whose
//! entries all fit gets a TIFF palette (tag 320, photometric 3) patched into the file, the
//! pixels untouched; any other raster gets a `<raster>.clr` sidecar listing the entries.

use std::fs;
use std::path::{Path, PathBuf};

use unigis_core::{Error, Result};

use crate::DataType;

/// One colormap entry: cell value and 8-bit red, green, blue.
pub type Entry = (i64, [u8; 3]);

const PHOTOMETRIC: u16 = 262;
const COLORMAP: u16 = 320;

/// Parses an Esri `.clr` file: `value red green blue` per line; blank lines and lines
/// starting with `#` are skipped.
pub fn parse_clr(text: &str) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let nums: Vec<i64> = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .take(4)
            .map(|t| t.parse::<f64>().map(|v| v as i64))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::format("CLR", format!("invalid line `{line}`")))?;
        let [value, r, g, b] = nums[..] else {
            return Err(Error::format("CLR", format!("invalid line `{line}`")));
        };
        let byte = |v: i64| v.clamp(0, 255) as u8;
        entries.push((value, [byte(r), byte(g), byte(b)]));
    }
    Ok(entries)
}

/// The `.clr` sidecar ArcGIS reads and writes next to a raster (`a.tif` → `a.tif.clr`).
pub fn sidecar_path(raster: &Path) -> PathBuf {
    let mut name = raster.as_os_str().to_owned();
    name.push(".clr");
    PathBuf::from(name)
}

/// A TIFF's embedded palette as 16-bit (red, green, blue) entries, if it has one.
pub fn read_tiff_colormap(path: &Path) -> Result<Option<Vec<[u16; 3]>>> {
    let bytes = fs::read(path).map_err(|e| Error::io(path, e))?;
    let tiff = Tiff::parse(&bytes, path)?;
    let Some(e) = tiff.entries.iter().find(|e| e.tag == COLORMAP) else {
        return Ok(None);
    };
    let n = e.count as usize;
    let start = if n * 2 <= 4 {
        e.raw_value_pos
    } else {
        tiff.u32(&bytes, e.raw_value_pos) as usize
    };
    if start + 2 * n > bytes.len() {
        return Err(Error::format(
            "GeoTIFF",
            format!("{}: colormap out of range", path.display()),
        ));
    }
    let words: Vec<u16> = (0..n).map(|i| tiff.u16(&bytes, start + 2 * i)).collect();
    let third = n / 3;
    Ok(Some(
        (0..third)
            .map(|i| [words[i], words[third + i], words[2 * third + i]])
            .collect(),
    ))
}

/// A palette TIFF's bytes with photometric 1 instead of 3 in its first IFD (None for any
/// other file), so that it decodes as plain gray samples. Reads only the header and IFD
/// unless the file is a palette image.
pub(crate) fn without_palette(path: &Path) -> Result<Option<Vec<u8>>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = fs::File::open(path).map_err(|e| Error::io(path, e))?;
    let mut head = [0u8; 8];
    if file.read_exact(&mut head).is_err() {
        return Ok(None);
    }
    let le = match &head[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Ok(None),
    };
    let u16_of = |b: [u8; 2]| {
        if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    };
    if u16_of([head[2], head[3]]) != 42 {
        return Ok(None);
    }
    let ifd = if le {
        u32::from_le_bytes([head[4], head[5], head[6], head[7]])
    } else {
        u32::from_be_bytes([head[4], head[5], head[6], head[7]])
    };
    let mut count = [0u8; 2];
    if file.seek(SeekFrom::Start(u64::from(ifd))).is_err() || file.read_exact(&mut count).is_err() {
        return Ok(None);
    }
    let mut entries = vec![0u8; 12 * usize::from(u16_of(count))];
    if file.read_exact(&mut entries).is_err() {
        return Ok(None);
    }
    let Some(i) = entries
        .as_chunks::<12>()
        .0
        .iter()
        .position(|e| u16_of([e[0], e[1]]) == PHOTOMETRIC && u16_of([e[8], e[9]]) == 3)
    else {
        return Ok(None);
    };
    let mut bytes = fs::read(path).map_err(|e| Error::io(path, e))?;
    let at = ifd as usize + 2 + 12 * i + 8;
    let one = if le {
        1u16.to_le_bytes()
    } else {
        1u16.to_be_bytes()
    };
    bytes[at..at + 2].copy_from_slice(&one);
    Ok(Some(bytes))
}

/// A raster's colormap as ArcGIS sees it: the embedded palette (all entries), else the
/// `.clr` sidecar.
pub fn read_colormap(path: &Path) -> Result<Option<Vec<Entry>>> {
    let is_tiff = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff"));
    if is_tiff && let Some(table) = read_tiff_colormap(path)? {
        return Ok(Some(
            table
                .iter()
                .enumerate()
                .map(|(i, c)| (i as i64, c.map(|v| (v / 257) as u8)))
                .collect(),
        ));
    }
    let clr = sidecar_path(path);
    if clr.exists() {
        let text = fs::read_to_string(&clr).map_err(|e| Error::io(&clr, e))?;
        return Ok(Some(parse_clr(&text)?));
    }
    Ok(None)
}

/// The colormap Add Colormap applies: the template's (it wins when both are given) or the
/// `.clr` file's. Fails like ArcGIS for a template without one.
pub fn source_entries(template: Option<&Path>, clr: Option<&Path>) -> Result<Vec<Entry>> {
    match (template, clr) {
        (Some(t), _) => read_colormap(t)?.ok_or_else(|| {
            Error::Unsupported(
                "ERROR 000250: The template raster dataset does not have a colormap".into(),
            )
        }),
        (None, Some(c)) => parse_clr(&fs::read_to_string(c).map_err(|e| Error::io(c, e))?),
        (None, None) => Err(Error::Unsupported(
            "ERROR 000735: Input Template Raster or Input .clr File is required".into(),
        )),
    }
}

/// Fails like ArcGIS for rasters that cannot take a colormap (floating point).
pub fn check_raster(raster: &Path) -> Result<DataType> {
    let data_type = crate::read_geotiff(raster)?.data_type;
    if data_type.is_float() {
        return Err(Error::Unsupported(
            "ERROR 000199: Failed to add Colormap".into(),
        ));
    }
    Ok(data_type)
}

/// Data Management Add Colormap: gives `raster` the colormap of `template` or of a `.clr`
/// file, embedding it in an 8-bit unsigned GeoTIFF when every value is 0..=255 (replacing
/// any palette there) and writing the `.clr` sidecar otherwise (other types, wider values).
pub fn add_colormap(raster: &Path, template: Option<&Path>, clr: Option<&Path>) -> Result<()> {
    let entries = source_entries(template, clr)?;
    let data_type = check_raster(raster)?;
    apply(raster, data_type, &entries)
}

/// Stores `entries` as [`add_colormap`] describes.
pub fn apply(raster: &Path, data_type: DataType, entries: &[Entry]) -> Result<()> {
    if data_type == DataType::U8 && entries.iter().all(|(v, _)| (0..=255).contains(v)) {
        let mut table = [[0u16; 3]; 256];
        for (v, rgb) in entries {
            table[*v as usize] = rgb.map(|c| u16::from(c) * 257);
        }
        return write_tiff_colormap(raster, &table);
    }
    let text: String = entries
        .iter()
        .map(|(v, [r, g, b])| format!("{v} {r} {g} {b}\n"))
        .collect();
    let path = sidecar_path(raster);
    fs::write(&path, text).map_err(|e| Error::io(&path, e))
}

/// Patches a palette into a classic TIFF: a copy of the first IFD with photometric 3 and
/// the colormap is appended and the header pointed at it; pixel data stays where it is.
pub fn write_tiff_colormap(path: &Path, table: &[[u16; 3]; 256]) -> Result<()> {
    rewrite_palette(path, Some(table))
}

/// Data Management Delete Colormap: removes an embedded TIFF palette the way ArcGIS does (a
/// copy of the first IFD with photometric 1 and no colormap is appended). A `.clr` sidecar
/// is left alone (ArcGIS reports success and changes nothing); a raster with neither fails
/// with ERROR 000235.
pub fn delete_colormap(path: &Path) -> Result<()> {
    if read_tiff_colormap(path)?.is_some() {
        return rewrite_palette(path, None);
    }
    if sidecar_path(path).exists() {
        return Ok(());
    }
    Err(Error::Unsupported(
        "ERROR 000235: Input raster does not have a colormap".into(),
    ))
}

/// Appends a copy of the first IFD with `table` as its palette (photometric 3), or without
/// a palette (photometric 1), and points the header at it.
fn rewrite_palette(path: &Path, table: Option<&[[u16; 3]; 256]>) -> Result<()> {
    let mut bytes = fs::read(path).map_err(|e| Error::io(path, e))?;
    let tiff = Tiff::parse(&bytes, path)?;
    let le = tiff.little_endian;
    let put16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let put32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    let map_offset = bytes.len() as u32;
    for channel in 0..3 {
        for entry in table.into_iter().flatten() {
            bytes.extend_from_slice(&put16(entry[channel]));
        }
    }
    let mut entries: Vec<[u8; 12]> = tiff
        .entries
        .iter()
        .filter(|e| e.tag != PHOTOMETRIC && e.tag != COLORMAP)
        .map(|e| e.raw)
        .collect();
    let mut entry = |tag: u16, count: u32, value: [u8; 4]| {
        let mut raw = [0u8; 12];
        raw[0..2].copy_from_slice(&put16(tag));
        raw[2..4].copy_from_slice(&put16(3));
        raw[4..8].copy_from_slice(&put32(count));
        raw[8..12].copy_from_slice(&value);
        entries.push(raw);
    };
    let [a, b] = put16(if table.is_some() { 3 } else { 1 });
    entry(PHOTOMETRIC, 1, [a, b, 0, 0]);
    if table.is_some() {
        entry(COLORMAP, 768, put32(map_offset));
    }
    let tag_of = |raw: &[u8; 12]| {
        if le {
            u16::from_le_bytes([raw[0], raw[1]])
        } else {
            u16::from_be_bytes([raw[0], raw[1]])
        }
    };
    entries.sort_by_key(tag_of);
    let ifd_offset = bytes.len() as u32;
    bytes.extend_from_slice(&put16(entries.len() as u16));
    for raw in &entries {
        bytes.extend_from_slice(raw);
    }
    bytes.extend_from_slice(&put32(tiff.next_ifd));
    bytes[4..8].copy_from_slice(&put32(ifd_offset));
    fs::write(path, bytes).map_err(|e| Error::io(path, e))
}

struct IfdEntry {
    tag: u16,
    count: u32,
    /// Position of the 4-byte value/offset field in the file.
    raw_value_pos: usize,
    raw: [u8; 12],
}

/// The first IFD of a classic TIFF.
struct Tiff {
    little_endian: bool,
    ifd: usize,
    entries: Vec<IfdEntry>,
    next_ifd: u32,
}

impl Tiff {
    fn parse(bytes: &[u8], path: &Path) -> Result<Tiff> {
        let bad = |what: &str| Error::format("GeoTIFF", format!("{}: {what}", path.display()));
        let little_endian = match bytes.get(0..2) {
            Some(b"II") => true,
            Some(b"MM") => false,
            _ => return Err(bad("not a TIFF")),
        };
        let mut t = Tiff {
            little_endian,
            ifd: 0,
            entries: Vec::new(),
            next_ifd: 0,
        };
        if bytes.len() < 8 {
            return Err(bad("truncated header"));
        }
        match t.u16(bytes, 2) {
            42 => {}
            43 => return Err(Error::Unsupported("colormaps in BigTIFF files".into())),
            _ => return Err(bad("not a TIFF")),
        }
        t.ifd = t.u32(bytes, 4) as usize;
        if t.ifd + 2 > bytes.len() {
            return Err(bad("IFD offset out of range"));
        }
        let n = t.u16(bytes, t.ifd) as usize;
        let end = t.ifd + 2 + 12 * n;
        if end + 4 > bytes.len() {
            return Err(bad("truncated IFD"));
        }
        for i in 0..n {
            let at = t.ifd + 2 + 12 * i;
            let mut raw = [0u8; 12];
            raw.copy_from_slice(&bytes[at..at + 12]);
            t.entries.push(IfdEntry {
                tag: t.u16(bytes, at),
                count: t.u32(bytes, at + 4),
                raw_value_pos: at + 8,
                raw,
            });
        }
        t.next_ifd = t.u32(bytes, end);
        Ok(t)
    }

    fn u16(&self, b: &[u8], at: usize) -> u16 {
        let v = [b[at], b[at + 1]];
        if self.little_endian {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        }
    }

    fn u32(&self, b: &[u8], at: usize) -> u32 {
        let v = [b[at], b[at + 1], b[at + 2], b[at + 3]];
        if self.little_endian {
            u32::from_le_bytes(v)
        } else {
            u32::from_be_bytes(v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clr_with_comments() {
        let e = parse_clr("# c\n1 255 0 0\n\n7\t10 20 30\n").unwrap();
        assert_eq!(e, vec![(1, [255, 0, 0]), (7, [10, 20, 30])]);
    }

    #[test]
    fn embeds_palette_and_reads_it_back() {
        let dir = std::env::temp_dir().join(format!("acm-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let tif = dir.join("a.tif");
        let mut r = crate::Raster::new(3, 2, DataType::U8, [0.0, 1.0, 0.0, 2.0, 0.0, -1.0]);
        r.bands[0] = vec![0.0, 1.0, 2.0, 3.0, 7.0, 1.0];
        crate::write_geotiff(&tif, &r, crate::Compression::None).unwrap();
        let clr = dir.join("c.clr");
        fs::write(&clr, "1 255 0 0\n7 10 20 30\n").unwrap();
        add_colormap(&tif, None, Some(&clr)).unwrap();
        let map = read_tiff_colormap(&tif).unwrap().unwrap();
        assert_eq!(map.len(), 256);
        assert_eq!(map[1], [65535, 0, 0]);
        assert_eq!(map[7], [2570, 5140, 7710]);
        assert_eq!(map[0], [0, 0, 0]);
        assert_eq!(crate::read_geotiff(&tif).unwrap().bands, r.bands);
        delete_colormap(&tif).unwrap();
        assert!(read_tiff_colormap(&tif).unwrap().is_none());
        assert_eq!(crate::read_geotiff(&tif).unwrap().bands, r.bands);
        assert!(delete_colormap(&tif).is_err());
        fs::remove_dir_all(&dir).ok();
    }
}
