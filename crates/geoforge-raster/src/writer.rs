//! A GeoTIFF encoder for tiled or striped layouts, every band, LZW/Deflate/
//! PackBits with the horizontal or floating-point predictor, internal overviews
//! and BigTIFF.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use geoforge_core::{Error, Result};

use crate::{Compression, DataType, Raster};

/// How overview cells are computed from the full-resolution cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverviewResampling {
    #[default]
    Nearest,
    Average,
}

/// Options for [`write_geotiff_with`].
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    pub compression: Compression,
    /// Tile size (multiples of 16); `None` writes strips.
    pub tile: Option<(usize, usize)>,
    /// Horizontal (integers) or floating-point (floats) prediction; LZW and
    /// Deflate only.
    pub predictor: bool,
    /// Overview decimation factors, e.g. `[2, 4, 8]`.
    pub overviews: Vec<usize>,
    pub overview_resampling: OverviewResampling,
    /// Write BigTIFF (64-bit offsets); chosen automatically past 4 GB.
    pub bigtiff: bool,
}

const MODEL_PIXEL_SCALE: u16 = 33550;
const MODEL_TIEPOINT: u16 = 33922;
const MODEL_TRANSFORMATION: u16 = 34264;
const GEO_KEY_DIRECTORY: u16 = 34735;
const GDAL_NODATA: u16 = 42113;

/// A TIFF field value.
enum Field {
    Short(Vec<u16>),
    Long(Vec<u32>),
    Long8(Vec<u64>),
    Double(Vec<f64>),
    Ascii(String),
}

impl Field {
    fn type_and_count(&self) -> (u16, usize) {
        match self {
            Field::Short(v) => (3, v.len()),
            Field::Long(v) => (4, v.len()),
            Field::Long8(v) => (16, v.len()),
            Field::Double(v) => (12, v.len()),
            Field::Ascii(s) => (2, s.len() + 1),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        match self {
            Field::Short(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Field::Long(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Field::Long8(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Field::Double(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Field::Ascii(s) => {
                let mut b = s.as_bytes().to_vec();
                b.push(0);
                b
            }
        }
    }
}

fn bits(t: DataType) -> u16 {
    match t {
        DataType::U8 | DataType::I8 => 8,
        DataType::U16 | DataType::I16 => 16,
        DataType::U32 | DataType::I32 | DataType::F32 => 32,
        DataType::F64 => 64,
    }
}

fn sample_format(t: DataType) -> u16 {
    match t {
        DataType::U8 | DataType::U16 | DataType::U32 => 1,
        DataType::I8 | DataType::I16 | DataType::I32 => 2,
        DataType::F32 | DataType::F64 => 3,
    }
}

/// Little-endian bytes of one value stored as `t`.
fn encode(v: f64, t: DataType, out: &mut Vec<u8>) {
    match t {
        DataType::U8 => out.push(v as u8),
        DataType::I8 => out.push(v as i8 as u8),
        DataType::U16 => out.extend((v as u16).to_le_bytes()),
        DataType::I16 => out.extend((v as i16).to_le_bytes()),
        DataType::U32 => out.extend((v as u32).to_le_bytes()),
        DataType::I32 => out.extend((v as i32).to_le_bytes()),
        DataType::F32 => out.extend((v as f32).to_le_bytes()),
        DataType::F64 => out.extend(v.to_le_bytes()),
    }
}

/// Applies the predictor to one row of `samples` values in place.
fn predict(row: &mut [u8], t: DataType, samples: usize) {
    let bps = bits(t) as usize / 8;
    if t.is_float() {
        // Floating-point predictor: bytes regrouped most significant first per
        // value, then differenced byte by byte.
        let n = row.len() / bps;
        let mut planes = vec![0u8; row.len()];
        for i in 0..n {
            for b in 0..bps {
                planes[b * n + i] = row[i * bps + (bps - 1 - b)];
            }
        }
        for i in (samples..planes.len()).rev() {
            planes[i] = planes[i].wrapping_sub(planes[i - samples]);
        }
        row.copy_from_slice(&planes);
    } else {
        let stride = samples * bps;
        for i in (stride..row.len()).step_by(bps).rev() {
            let read = |b: &[u8]| {
                let mut buf = [0u8; 8];
                buf[..bps].copy_from_slice(b);
                u64::from_le_bytes(buf)
            };
            let d = read(&row[i..i + bps]).wrapping_sub(read(&row[i - stride..i - stride + bps]));
            row[i..i + bps].copy_from_slice(&d.to_le_bytes()[..bps]);
        }
    }
}

fn compress(data: &[u8], c: Compression) -> Result<Vec<u8>> {
    Ok(match c {
        Compression::None => data.to_vec(),
        Compression::Lzw => weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
            .encode(data)
            .map_err(|e| Error::format("GeoTIFF", format!("LZW: {e}")))?,
        Compression::Deflate => {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data)
                .map_err(|e| Error::format("GeoTIFF", e.to_string()))?;
            e.finish()
                .map_err(|e| Error::format("GeoTIFF", e.to_string()))?
        }
        Compression::PackBits => unreachable!("PackBits is packed row by row"),
    })
}

fn packbits(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let mut run = 1;
        while i + run < data.len() && run < 128 && data[i + run] == data[i] {
            run += 1;
        }
        if run >= 2 {
            out.push((1i16 - run as i16) as i8 as u8);
            out.push(data[i]);
            i += run;
        } else {
            let start = i;
            let mut len = 0;
            while i < data.len() && len < 128 {
                if i + 1 < data.len() && data[i + 1] == data[i] {
                    break;
                }
                i += 1;
                len += 1;
            }
            if len == 0 {
                i += 1;
                len = 1;
            }
            out.push((len - 1) as u8);
            out.extend_from_slice(&data[start..start + len]);
        }
    }
    out
}

/// A level of the pyramid: width, height and one value vector per band.
struct Level {
    width: usize,
    height: usize,
    bands: Vec<Vec<f64>>,
}

fn overview(r: &Raster, factor: usize, how: OverviewResampling) -> Level {
    let (w, h) = (r.width.div_ceil(factor), r.height.div_ceil(factor));
    let nodata = r.nodata;
    let bands = r
        .bands
        .iter()
        .map(|band| {
            let mut out = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let v = match how {
                        OverviewResampling::Nearest => {
                            let (sx, sy) = (
                                (x * factor).min(r.width - 1),
                                (y * factor).min(r.height - 1),
                            );
                            band[sy * r.width + sx]
                        }
                        OverviewResampling::Average => {
                            let (mut sum, mut n) = (0.0, 0usize);
                            for sy in y * factor..((y + 1) * factor).min(r.height) {
                                for sx in x * factor..((x + 1) * factor).min(r.width) {
                                    let v = band[sy * r.width + sx];
                                    if nodata.is_none_or(|nd| v != nd) {
                                        sum += v;
                                        n += 1;
                                    }
                                }
                            }
                            if n == 0 {
                                nodata.unwrap_or(0.0)
                            } else {
                                r.data_type.store(sum / n as f64)
                            }
                        }
                    };
                    out.push(v);
                }
            }
            out
        })
        .collect();
    Level {
        width: w,
        height: h,
        bands,
    }
}

/// Encodes the chunks of one level (pixel-interleaved), returning compressed
/// chunk data.
fn chunks(level: &Level, t: DataType, opts: &WriteOptions) -> Result<(usize, usize, Vec<Vec<u8>>)> {
    let samples = level.bands.len();
    let (cw, ch) = opts.tile.unwrap_or((
        level.width,
        (8192 / level.width.max(1)).clamp(1, level.height.max(1)),
    ));
    let across = level.width.div_ceil(cw);
    let down = level.height.div_ceil(ch);
    let mut out = Vec::with_capacity(across * down);
    for ty in 0..down {
        for tx in 0..across {
            // Strips at the bottom are short; tiles are always full and padded.
            let rows = if opts.tile.is_some() {
                ch
            } else {
                ch.min(level.height - ty * ch)
            };
            let mut data = Vec::new();
            let mut packed = Vec::new();
            for r in 0..rows {
                let y = ty * ch + r;
                let mut row = Vec::new();
                for c in 0..cw {
                    let x = tx * cw + c;
                    for band in &level.bands {
                        let v = if x < level.width && y < level.height {
                            band[y * level.width + x]
                        } else {
                            0.0
                        };
                        encode(v, t, &mut row);
                    }
                }
                if opts.predictor {
                    predict(&mut row, t, samples);
                }
                // PackBits runs may not cross rows (TIFF 6.0, section 9).
                if opts.compression == Compression::PackBits {
                    packed.extend(packbits(&row));
                } else {
                    data.extend(row);
                }
            }
            out.push(if opts.compression == Compression::PackBits {
                packed
            } else {
                compress(&data, opts.compression)?
            });
        }
    }
    Ok((cw, ch, out))
}

/// Writes every band of `raster` as a GeoTIFF with `opts`.
pub fn write_geotiff_with(path: &Path, raster: &Raster, opts: &WriteOptions) -> Result<()> {
    // libtiff only applies predictors with LZW and Deflate.
    let opts = &WriteOptions {
        predictor: opts.predictor
            && matches!(opts.compression, Compression::Lzw | Compression::Deflate),
        ..opts.clone()
    };
    if let Some((tw, th)) = opts.tile
        && (tw % 16 != 0 || th % 16 != 0 || tw == 0 || th == 0)
    {
        return Err(Error::Unsupported(
            "tile sizes must be multiples of 16".into(),
        ));
    }
    let t = raster.data_type;
    let mut levels = vec![Level {
        width: raster.width,
        height: raster.height,
        bands: raster.bands.clone(),
    }];
    for &f in &opts.overviews {
        if f > 1 {
            levels.push(overview(raster, f, opts.overview_resampling));
        }
    }
    let encoded: Vec<(usize, usize, Vec<Vec<u8>>)> = levels
        .iter()
        .map(|l| chunks(l, t, opts))
        .collect::<Result<_>>()?;
    let payload: u64 = encoded
        .iter()
        .flat_map(|e| e.2.iter())
        .map(|c| c.len() as u64)
        .sum();
    let big = opts.bigtiff || payload > 0xF000_0000;

    let file = File::create(path).map_err(|e| Error::io(path, e))?;
    let mut w = BufWriter::new(file);
    let io = |e| Error::io(path, e);
    // Header; the first IFD offset is patched in later.
    if big {
        w.write_all(b"II\x2b\x00\x08\x00\x00\x00").map_err(io)?;
        w.write_all(&0u64.to_le_bytes()).map_err(io)?;
    } else {
        w.write_all(b"II\x2a\x00").map_err(io)?;
        w.write_all(&0u32.to_le_bytes()).map_err(io)?;
    }
    // Where the previous "next IFD" offset lives.
    let mut next_pointer: u64 = if big { 8 } else { 4 };
    for (li, (level, (_, ch, data))) in levels.iter().zip(&encoded).enumerate() {
        // Chunk data first, word aligned.
        let mut offsets = Vec::with_capacity(data.len());
        for chunk in data {
            let pos = w.stream_position().map_err(io)?;
            if pos % 2 == 1 {
                w.write_all(&[0]).map_err(io)?;
            }
            offsets.push(w.stream_position().map_err(io)?);
            w.write_all(chunk).map_err(io)?;
        }
        let counts: Vec<u64> = data.iter().map(|c| c.len() as u64).collect();
        let samples = level.bands.len() as u16;
        let mut fields: Vec<(u16, Field)> = Vec::new();
        if li > 0 {
            fields.push((254, Field::Long(vec![1])));
        }
        fields.push((256, Field::Long(vec![level.width as u32])));
        fields.push((257, Field::Long(vec![level.height as u32])));
        fields.push((258, Field::Short(vec![bits(t); samples as usize])));
        let comp = match opts.compression {
            Compression::None => 1,
            Compression::Lzw => 5,
            Compression::Deflate => 8,
            Compression::PackBits => 32773,
        };
        fields.push((259, Field::Short(vec![comp])));
        fields.push((262, Field::Short(vec![1])));
        let offsets_field = |v: Vec<u64>| {
            if big {
                Field::Long8(v)
            } else {
                Field::Long(v.into_iter().map(|x| x as u32).collect())
            }
        };
        if opts.tile.is_none() {
            fields.push((273, offsets_field(offsets.clone())));
        }
        fields.push((277, Field::Short(vec![samples])));
        if opts.tile.is_none() {
            fields.push((278, Field::Long(vec![*ch as u32])));
            fields.push((279, offsets_field(counts.clone())));
        }
        fields.push((284, Field::Short(vec![1])));
        if opts.predictor {
            fields.push((317, Field::Short(vec![if t.is_float() { 3 } else { 2 }])));
        }
        if let Some((tw, th)) = opts.tile {
            fields.push((322, Field::Long(vec![tw as u32])));
            fields.push((323, Field::Long(vec![th as u32])));
            fields.push((324, offsets_field(offsets)));
            fields.push((325, offsets_field(counts)));
        }
        if samples > 1 {
            // Bands beyond the first grey channel are unspecified extra samples.
            fields.push((338, Field::Short(vec![0; samples as usize - 1])));
        }
        fields.push((339, Field::Short(vec![sample_format(t); samples as usize])));
        if li == 0 {
            geo_fields(raster, &mut fields);
        }
        fields.sort_by_key(|f| f.0);

        // The IFD, with values that do not fit inline written after it.
        let pos = w.stream_position().map_err(io)?;
        let ifd_pos = pos + pos % 2;
        if pos % 2 == 1 {
            w.write_all(&[0]).map_err(io)?;
        }
        // Patch the previous pointer.
        w.seek(SeekFrom::Start(next_pointer)).map_err(io)?;
        if big {
            w.write_all(&ifd_pos.to_le_bytes()).map_err(io)?;
        } else {
            w.write_all(&(ifd_pos as u32).to_le_bytes()).map_err(io)?;
        }
        w.seek(SeekFrom::Start(ifd_pos)).map_err(io)?;
        let (entry_size, inline) = if big { (20u64, 8usize) } else { (12, 4) };
        let count_size = if big { 8 } else { 2 };
        let ifd_size = count_size + fields.len() as u64 * entry_size + if big { 8 } else { 4 };
        let mut extra_pos = ifd_pos + ifd_size;
        let mut extra: Vec<u8> = Vec::new();
        let mut ifd: Vec<u8> = Vec::new();
        if big {
            ifd.extend((fields.len() as u64).to_le_bytes());
        } else {
            ifd.extend((fields.len() as u16).to_le_bytes());
        }
        for (tag, field) in &fields {
            let (ty, count) = field.type_and_count();
            let bytes = field.bytes();
            ifd.extend(tag.to_le_bytes());
            ifd.extend(ty.to_le_bytes());
            if big {
                ifd.extend((count as u64).to_le_bytes());
            } else {
                ifd.extend((count as u32).to_le_bytes());
            }
            if bytes.len() <= inline {
                let mut b = bytes.clone();
                b.resize(inline, 0);
                ifd.extend(b);
            } else {
                if extra_pos % 2 == 1 {
                    extra.push(0);
                    extra_pos += 1;
                }
                if big {
                    ifd.extend(extra_pos.to_le_bytes());
                } else {
                    ifd.extend((extra_pos as u32).to_le_bytes());
                }
                extra.extend(&bytes);
                extra_pos += bytes.len() as u64;
            }
        }
        next_pointer = ifd_pos + count_size + fields.len() as u64 * entry_size;
        if big {
            ifd.extend(0u64.to_le_bytes());
        } else {
            ifd.extend(0u32.to_le_bytes());
        }
        w.write_all(&ifd).map_err(io)?;
        w.write_all(&extra).map_err(io)?;
    }
    w.flush().map_err(io)?;
    Ok(())
}

/// GeoTIFF georeferencing and NoData tags for the main image.
fn geo_fields(r: &Raster, fields: &mut Vec<(u16, Field)>) {
    let t = &r.transform;
    if t[2] == 0.0 && t[4] == 0.0 {
        fields.push((MODEL_PIXEL_SCALE, Field::Double(vec![t[1], -t[5], 0.0])));
        fields.push((
            MODEL_TIEPOINT,
            Field::Double(vec![0.0, 0.0, 0.0, t[0], t[3], 0.0]),
        ));
    } else {
        fields.push((
            MODEL_TRANSFORMATION,
            Field::Double(vec![
                t[1], t[2], 0.0, t[0], t[4], t[5], 0.0, t[3], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                1.0,
            ]),
        ));
    }
    fields.push((
        GEO_KEY_DIRECTORY,
        Field::Short(crate::geotiff::geo_keys(r.epsg)),
    ));
    if let Some(nd) = r.nodata {
        fields.push((
            GDAL_NODATA,
            Field::Ascii(crate::geotiff::format_nodata(nd, r.data_type)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::packbits;

    #[test]
    fn packbits_round_trip() {
        let cases: Vec<Vec<u8>> = vec![
            vec![203, 0, 204, 0, 0, 0, 0, 5, 5, 6],
            vec![1; 300],
            (0..=255).collect(),
            vec![1, 2, 2, 3, 3, 3, 4],
        ];
        for data in cases {
            let enc = packbits(&data);
            let dec = crate::geotiff::decompress(&enc, 32773, data.len()).unwrap();
            assert_eq!(dec, data);
        }
    }
}
