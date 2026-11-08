//! Set Raster Properties: byte-preserving TIFF metadata and PAM statistics/source type.
use crate::{DataType, read_geotiff};
use roxmltree::{Document, Node};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use unigis_core::{Error, Result};

pub const SOURCE_TYPES: &[&str] = &[
    "GENERIC",
    "ELEVATION",
    "THEMATIC",
    "PROCESSED",
    "SCIENTIFIC",
    "VECTOR_UV",
    "VECTOR_MAGDIR",
    "DATE",
    "SAR",
];
const SOURCE_NAMES: &[&str] = &[
    "Generic",
    "Elevation",
    "Thematic",
    "Processed",
    "Scientific",
    "Vector-UV",
    "Vector-MagDir",
    "StdTime",
    "SAR",
];
const KEYS: &[&str] = &[
    "MINIMUM",
    "MAXIMUM",
    "MEAN",
    "STDDEV",
    "MEDIAN",
    "COUNT",
    "SKIPFACTORX",
    "SKIPFACTORY",
    "EXCLUDEDVALUES",
];
fn err(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}
fn aux_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".aux.xml");
    p.into()
}
fn read_aux(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}
fn parse(xml: &str) -> Result<Document<'_>> {
    let doc = Document::parse(xml).map_err(|e| err(format!("invalid raster PAM metadata: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "PAMDataset" || root.tag_name().namespace().is_some() {
        return Err(err("unsupported raster PAM root"));
    }
    let mut bands = std::collections::HashSet::new();
    for p in
        std::iter::once(root).chain(root.children().filter(|n| n.has_tag_name("PAMRasterBand")))
    {
        if p.has_tag_name("PAMRasterBand") && !bands.insert(p.attribute("band")) {
            return Err(err("duplicate PAM band"));
        }
        let metadata: Vec<_> = p
            .children()
            .filter(|n| {
                n.has_tag_name("Metadata") && n.attribute("domain").is_none_or(str::is_empty)
            })
            .collect();
        if metadata.len() > 1 {
            return Err(err("duplicate default PAM metadata domain"));
        }
        if let Some(m) = metadata.first() {
            let mut keys = std::collections::HashSet::new();
            for item in m.children().filter(|n| n.has_tag_name("MDI")) {
                if !keys.insert(item.attribute("key")) {
                    return Err(err("duplicate PAM metadata key"));
                }
            }
        }
    }
    Ok(doc)
}
fn container<'a>(root: Node<'a, 'a>, band: Option<usize>) -> Option<Node<'a, 'a>> {
    band.map_or(Some(root), |b| {
        root.children().find(|n| {
            n.has_tag_name("PAMRasterBand")
                && n.attribute("band").and_then(|b| b.parse::<usize>().ok()) == Some(b)
        })
    })
}
fn add_child(xml: &str, parent: Node, child: &str) -> String {
    let range = parent.range();
    let raw = &xml[range.clone()];
    let (at, replace) = if raw.ends_with("/>") {
        (range.end - 2, true)
    } else {
        (range.start + raw.rfind("</").unwrap(), false)
    };
    if replace {
        format!(
            "{}>{child}</{}>{}",
            &xml[..at],
            parent.tag_name().name(),
            &xml[range.end..]
        )
    } else {
        format!("{}{child}{}", &xml[..at], &xml[at..])
    }
}
/// Drops an empty metadata item (ArcGIS removes the empty `STATISTICS_EXCLUDEDVALUES` of the
/// bands whose statistics it did not set).
fn remove_if_empty(xml: &mut String, band: usize, key: &str) -> Result<()> {
    let doc = parse(xml)?;
    let item = container(doc.root_element(), Some(band)).and_then(|b| {
        b.children()
            .filter(|n| {
                n.has_tag_name("Metadata") && n.attribute("domain").is_none_or(str::is_empty)
            })
            .flat_map(|m| m.children())
            .find(|n| n.has_tag_name("MDI") && n.attribute("key") == Some(key))
            .filter(|n| n.text().is_none_or(str::is_empty))
            .map(|n| n.range())
    });
    if let Some(r) = item {
        let start = xml[..r.start].trim_end_matches([' ', '\t']).len();
        let start = if xml[..start].ends_with('\n') {
            start - 1
        } else {
            start
        };
        *xml = format!("{}{}", &xml[..start], &xml[r.end..]);
    }
    Ok(())
}
fn set(xml: &mut String, band: Option<usize>, key: &str, value: &str) -> Result<()> {
    let doc = parse(xml)?;
    let root = doc.root_element();
    let Some(parent) = container(root, band) else {
        let b = band.unwrap();
        *xml = add_child(
            xml,
            root,
            &format!("\n  <PAMRasterBand band=\"{b}\"><Metadata/></PAMRasterBand>\n"),
        );
        return set(xml, band, key, value);
    };
    let Some(metadata) = parent
        .children()
        .find(|n| n.has_tag_name("Metadata") && n.attribute("domain").is_none_or(str::is_empty))
    else {
        *xml = add_child(xml, parent, "\n<Metadata/>\n");
        return set(xml, band, key, value);
    };
    let item = format!("<MDI key=\"{key}\">{value}</MDI>");
    if let Some(old) = metadata
        .children()
        .find(|n| n.has_tag_name("MDI") && n.attribute("key") == Some(key))
    {
        let range = old.range();
        *xml = format!("{}{item}{}", &xml[..range.start], &xml[range.end..]);
    } else {
        *xml = add_child(xml, metadata, &format!("\n{item}\n"));
    }
    Ok(())
}
/// PAM source type is the value ArcGIS's Get Raster Properties exposes.
pub fn source_type(path: &Path) -> Result<String> {
    let Some(bytes) = read_aux(&aux_path(path))? else {
        return Ok("Generic".into());
    };
    let xml = std::str::from_utf8(&bytes).map_err(|_| err("PAM metadata must be UTF-8"))?;
    let doc = parse(xml)?;
    let name = doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("Metadata") && n.attribute("domain").is_none_or(str::is_empty))
        .flat_map(|m| m.children())
        .find(|n| n.has_tag_name("MDI") && n.attribute("key") == Some("DataType"))
        .and_then(|n| n.text())
        .unwrap_or("Generic");
    Ok(name.to_owned())
}
fn index(raw: &str, bands: usize) -> Result<usize> {
    raw.parse::<usize>().ok().filter(|b|(1..=bands).contains(b))
        .ok_or_else(||err(format!("ERROR 000622: Parameters are not valid. ERROR 000800: band must be a member of 1..{bands}")))
}
fn number(raw: &str) -> Result<f64> {
    raw.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| err("ERROR 001192: Invalid statistics or NoData number"))
}
fn limits(t: DataType) -> (f64, f64) {
    match t {
        DataType::U8 => (0., 255.),
        DataType::I8 => (-128., 127.),
        DataType::U16 => (0., 65535.),
        DataType::I16 => (-32768., 32767.),
        DataType::U32 => (0., u32::MAX as f64),
        DataType::I32 => (i32::MIN as f64, i32::MAX as f64),
        DataType::F32 => (-(f32::MAX as f64), f32::MAX as f64),
        DataType::F64 => (-f64::MAX, f64::MAX),
    }
}
/// Edits a GeoTIFF in place. Statistics are supplied, never recomputed. NoData is
/// limited to one marker on a single-band classic TIFF. Optional richer schemas
/// are refused by the CLI before reaching this function.
pub fn apply(
    path: &Path,
    data_type: Option<&str>,
    statistics: Option<&str>,
    nodata: Option<&str>,
) -> Result<()> {
    if !path.is_file() {
        return Err(err(
            "ERROR 000732: Input Raster does not exist or is not supported",
        ));
    }
    let raster = read_geotiff(path)?;
    let bands = raster.bands.len();
    let source = data_type
        .map(|v| {
            SOURCE_TYPES
                .iter()
                .position(|t| t.eq_ignore_ascii_case(v))
                .map(|i| SOURCE_NAMES[i])
                .ok_or_else(|| {
                    err(format!(
                        "ERROR 000800: data_type must be a member of {}",
                        SOURCE_TYPES.join(" | ")
                    ))
                })
        })
        .transpose()?;
    let mut stats = Vec::new();
    if let Some(raw) = statistics {
        for row in raw.split(';') {
            let vals: Vec<_> = row.split_whitespace().collect();
            if vals.len() != 5 {
                return Err(err(
                    "ERROR 000622: statistics requires band min max mean std_dev",
                ));
            }
            let band = index(vals[0], bands)?;
            let nums = vals[1..]
                .iter()
                .map(|v| number(v))
                .collect::<Result<Vec<_>>>()?;
            let (lo, hi) = limits(raster.data_type);
            if nums.iter().any(|v| *v < lo || *v > hi) {
                return Err(err(
                    "ERROR 001192: Invalid statistics number: out of the pixel range",
                ));
            }
            stats.push((band, nums));
        }
    }
    let marker = if let Some(raw) = nodata {
        if bands != 1 || raw.contains(';') {
            return Err(err("NoData edits support one value on one band only"));
        }
        let vals: Vec<_> = raw.split_whitespace().collect();
        if vals.len() != 2 {
            return Err(err(
                "NoData requires band value; multiple values and clearing are unsupported",
            ));
        }
        index(vals[0], bands)?;
        let v = number(vals[1])?;
        Some(match raster.data_type {
            DataType::F32 => v as f32 as f64,
            DataType::F64 => v,
            _ => v.round_ties_even(),
        })
    } else {
        None
    };
    if marker.is_some_and(|v| !v.is_finite()) {
        return Err(err("NoData is outside the supported finite range"));
    }
    // All parameters and existing PAM must be readable before either file changes.
    let aux = aux_path(path);
    let old_aux = read_aux(&aux)?;
    let mut xml = match &old_aux {
        Some(b) => String::from_utf8(b.clone()).map_err(|_| err("PAM metadata must be UTF-8"))?,
        None => "<PAMDataset>\n</PAMDataset>\n".into(),
    };
    parse(&xml)?;
    if let Some(name) = source {
        set(&mut xml, None, "DataType", name)?;
    }
    let stored = crate::stats_metadata::read(path)?;
    for (band, nums) in &stats {
        let values: Vec<_> = nums
            .iter()
            .map(|v| crate::stats::format_g14(*v))
            .chain([
                "nan".into(),
                "0.000000".into(),
                "1".into(),
                "1".into(),
                String::new(),
            ])
            .collect();
        for (offset, (key, value)) in KEYS.iter().zip(values).enumerate() {
            let name = format!("STATISTICS_{key}");
            let value = if offset >= 4 {
                stored[*band - 1].get(&name).cloned().unwrap_or(value)
            } else {
                value
            };
            set(&mut xml, Some(*band), &name, &value)?;
        }
    }
    if !stats.is_empty() {
        for band in 1..=bands {
            if !stats.iter().any(|(b, _)| *b == band) {
                remove_if_empty(&mut xml, band, "STATISTICS_EXCLUDEDVALUES")?;
            }
        }
    }
    let mut changes = Vec::new();
    if source.is_some() || !stats.is_empty() {
        changes.push((aux, old_aux, xml.into_bytes()));
    }
    if let Some(marker) = marker {
        let old = fs::read(path).map_err(|e| Error::io(path, e))?;
        let new = crate::colormap::with_ascii_tag(
            old.clone(),
            path,
            42113,
            &crate::worldfile::format_g(marker, 17),
        )?;
        changes.push((path.to_owned(), Some(old), new));
    }
    replace_files(changes)
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn stage(path: &Path, bytes: &[u8]) -> Result<Temp> {
    let mut p = path.as_os_str().to_owned();
    p.push(format!(
        ".unigis-properties-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let p = PathBuf::from(p);
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&p)
        .map_err(|e| Error::io(&p, e))?;
    let temp = Temp(p);
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| Error::io(&temp.0, e))?;
    if let Ok(m) = fs::metadata(path) {
        fs::set_permissions(&temp.0, m.permissions()).map_err(|e| Error::io(&temp.0, e))?;
    }
    Ok(temp)
}
fn replace_files(changes: Vec<(PathBuf, Option<Vec<u8>>, Vec<u8>)>) -> Result<()> {
    let temps = changes
        .iter()
        .map(|(p, _, new)| stage(p, new))
        .collect::<Result<Vec<_>>>()?;
    for (i, ((path, _, _), temp)) in changes.iter().zip(&temps).enumerate() {
        if let Err(e) = fs::rename(&temp.0, path) {
            for (done, old, _) in changes[..i].iter().rev() {
                let rollback = match old {
                    Some(b) => stage(done, b)
                        .and_then(|t| fs::rename(&t.0, done).map_err(|e| Error::io(done, e))),
                    None => fs::remove_file(done).map_err(|e| Error::io(done, e)),
                };
                if let Err(r) = rollback {
                    return Err(err(format!(
                        "metadata update failed ({e}); rollback failed ({r})"
                    )));
                }
            }
            return Err(Error::io(path, e));
        }
    }
    Ok(())
}
