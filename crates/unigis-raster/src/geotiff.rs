//! GeoTIFF reading and writing on top of the `tiff` crate; the GeoTIFF tags
//! (ModelPixelScale, ModelTiepoint, ModelTransformation, GeoKeyDirectory and
//! GDAL_NODATA) are handled here.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult, Limits};
use tiff::tags::{PlanarConfiguration, SampleFormat, Tag};
use unigis_core::{Error, Result};

use crate::{DataType, Raster};

const MODEL_TRANSFORMATION: u16 = 34264;
const GT_MODEL_TYPE: u16 = 1024;
const GT_RASTER_TYPE: u16 = 1025;
const GEOGRAPHIC_TYPE: u16 = 2048;
const PROJECTED_CS_TYPE: u16 = 3072;
const PIXEL_IS_POINT: u16 = 2;

fn tiff_error(path: &Path, e: tiff::TiffError) -> Error {
    match e {
        tiff::TiffError::IoError(io) => Error::io(path, io),
        other => Error::format("GeoTIFF", format!("{}: {other}", path.display())),
    }
}

/// Compression for [`write_geotiff`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compression {
    #[default]
    None,
    Lzw,
    Deflate,
    PackBits,
}

/// Reads a GeoTIFF (striped or tiled; uncompressed, LZW, Deflate, PackBits or
/// JPEG; any predictor) into memory.
pub fn read_geotiff(path: &Path) -> Result<Raster> {
    let file = File::open(path).map_err(|e| Error::io(path, e))?;
    let mut dec = Decoder::new(BufReader::new(file))
        .map_err(|e| tiff_error(path, e))?
        .with_limits(Limits::unlimited());
    let err = |e| tiff_error(path, e);

    let (width, height) = dec.dimensions().map_err(err)?;
    let (width, height) = (width as usize, height as usize);
    let (bits, samples) = match dec.colortype().map_err(err)? {
        ColorType::Gray(b) => (b, 1usize),
        ColorType::Multiband {
            bit_depth,
            num_samples,
        } => (bit_depth, num_samples as usize),
        ColorType::RGB(b) => (b, 3),
        ColorType::RGBA(b) => (b, 4),
        other => {
            return Err(Error::Unsupported(format!(
                "GeoTIFF color type {other:?} in {}",
                path.display()
            )));
        }
    };
    let planar = dec
        .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
        .map_err(err)?
        .unwrap_or(1);
    // One entry per sample; all bands share it here.
    let sample_format = dec
        .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
        .map_err(err)?
        .and_then(|v| v.first().copied())
        .unwrap_or(1);
    let data_type = match (SampleFormat::from_u16(sample_format), bits) {
        (Some(SampleFormat::IEEEFP), 32) => DataType::F32,
        (Some(SampleFormat::IEEEFP), 64) => DataType::F64,
        (Some(SampleFormat::Int), 8) => DataType::I8,
        (Some(SampleFormat::Int), 16) => DataType::I16,
        (Some(SampleFormat::Int), 32) => DataType::I32,
        (_, 1 | 2 | 4 | 8) => DataType::U8,
        (_, 16) => DataType::U16,
        (_, 32) => DataType::U32,
        _ => {
            return Err(Error::Unsupported(format!(
                "{bits}-bit samples (format {sample_format}) in {}",
                path.display()
            )));
        }
    };

    let mut transform = geotransform(&mut dec).map_err(err)?;
    let keys = dec
        .find_tag(Tag::GeoKeyDirectoryTag)
        .map_err(err)?
        .map(|v| v.into_u16_vec())
        .transpose()
        .map_err(err)?
        .unwrap_or_default();
    let key = |id: u16| -> Option<u16> {
        // Header (version, revision, minor, count) then 4-short entries; only
        // entries stored inline (location 0) are needed here.
        keys.get(4..)?
            .as_chunks::<4>()
            .0
            .iter()
            .find(|e| e[0] == id && e[1] == 0)
            .map(|e| e[3])
    };
    if key(GT_RASTER_TYPE) == Some(PIXEL_IS_POINT) {
        // Tie points name cell centers; shift to the corner like GDAL and ArcGIS.
        transform[0] -= 0.5 * transform[1] + 0.5 * transform[2];
        transform[3] -= 0.5 * transform[4] + 0.5 * transform[5];
    }
    let epsg = [PROJECTED_CS_TYPE, GEOGRAPHIC_TYPE]
        .into_iter()
        .filter_map(key)
        .find(|&c| c != 0 && c != 32767)
        .map(u32::from);

    let nodata = match dec.find_tag(Tag::GdalNodata).map_err(err)? {
        // Compared against cells of the stored type, so round it to that type.
        Some(v) => parse_nodata(&v.into_string().map_err(err)?).map(|nd| {
            if data_type == DataType::F32 {
                nd as f32 as f64
            } else {
                nd
            }
        }),
        None => None,
    };

    // Stored statistics: an ArcGIS/GDAL .aux.xml beside the file, else the
    // GDAL_METADATA tag.
    let aux = std::fs::read_to_string(format!("{}.aux.xml", path.display())).ok();
    let tag = match dec.find_tag(Tag::Unknown(42112)).map_err(err)? {
        Some(v) => v.into_string().ok(),
        None => None,
    };
    // ArcGIS treats the source as cached only when every band has extrema.
    // Union all stored ranges for the dataset's common output type. Metadata
    // remains band-specific; custom domains do not enable cache planning.
    let cached = crate::stats_metadata::from_xml(samples, tag.as_deref(), aux.as_deref());
    let extrema: Option<Vec<(f64, f64)>> = cached
        .iter()
        .map(|band| {
            Some((
                band.get("STATISTICS_MINIMUM")?.parse().ok()?,
                band.get("STATISTICS_MAXIMUM")?.parse().ok()?,
            ))
        })
        .collect();
    let statistics = extrema.and_then(|bands| {
        if bands.len() == 1 {
            return bands.first().copied();
        }
        if bands
            .iter()
            .any(|&(lo, hi)| !lo.is_finite() || !hi.is_finite() || lo > hi)
        {
            // Keep cache presence for existing float planning, while the
            // registry refuses cached integer math with invalid extrema.
            return Some((f64::NAN, f64::NAN));
        }
        bands
            .into_iter()
            .reduce(|(lo, hi), (a, b)| (lo.min(a), hi.max(b)))
    });

    let compression = dec
        .find_tag_unsigned::<u16>(Tag::Compression)
        .map_err(err)?
        .unwrap_or(1);
    let bands = if matches!(compression, 1 | 5 | 8 | 32946 | 32773) {
        let layout =
            Layout::from_tags(&mut dec, width, height, samples, planar == 2, bits).map_err(err)?;
        let big_endian = dec.byte_order() == tiff::tags::ByteOrder::BigEndian;
        let file = dec.inner();
        read_chunks(file, &layout, compression, big_endian, data_type)
            .map_err(|e| Error::format("GeoTIFF", format!("{}: {e}", path.display())))?
    } else {
        if samples > 1 && planar == PlanarConfiguration::Planar.to_u16() {
            return Err(Error::Unsupported(format!(
                "band-sequential (planar) GeoTIFF with compression {compression}: {}",
                path.display()
            )));
        }
        interleaved_to_bands(dec.read_image().map_err(err)?, width * height, samples).ok_or_else(
            || Error::format("GeoTIFF", format!("{}: short image data", path.display())),
        )?
    };

    Ok(Raster {
        width,
        height,
        data_type,
        transform,
        nodata,
        epsg,
        statistics,
        bands,
    })
}

fn interleaved_to_bands(
    image: DecodingResult,
    cells: usize,
    samples: usize,
) -> Option<Vec<Vec<f64>>> {
    let values: Vec<f64> = match image {
        DecodingResult::U8(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::I8(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::U16(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::I16(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::U32(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::I32(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::F32(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::F64(v) => v,
        DecodingResult::U64(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::I64(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::F16(v) => v.into_iter().map(f64::from).collect(),
    };
    if values.len() < cells * samples {
        return None;
    }
    Some(
        (0..samples)
            .map(|b| (0..cells).map(|i| values[i * samples + b]).collect())
            .collect(),
    )
}

/// Where the strips or tiles are and how they cover the image.
struct Layout {
    width: usize,
    height: usize,
    samples: usize,
    planar: bool,
    bytes_per_sample: usize,
    /// Bits per sample when below 8 (packed, most significant bits first).
    sub_byte_bits: usize,
    chunk_width: usize,
    chunk_height: usize,
    offsets: Vec<u64>,
    byte_counts: Vec<u64>,
    predictor: u16,
}

impl Layout {
    fn from_tags<R: std::io::Read + std::io::Seek>(
        dec: &mut Decoder<R>,
        width: usize,
        height: usize,
        samples: usize,
        planar: bool,
        bits: u8,
    ) -> tiff::TiffResult<Layout> {
        let tile_width = dec.find_tag_unsigned::<u32>(Tag::TileWidth)?;
        let (chunk_width, chunk_height, offsets, counts) = match tile_width {
            Some(tw) => (
                tw as usize,
                dec.get_tag_unsigned::<u32>(Tag::TileLength)? as usize,
                dec.get_tag(Tag::TileOffsets)?.into_u64_vec()?,
                dec.get_tag(Tag::TileByteCounts)?.into_u64_vec()?,
            ),
            None => (
                width,
                dec.find_tag_unsigned::<u32>(Tag::RowsPerStrip)?
                    .map_or(height, |r| (r as usize).min(height).max(1)),
                dec.get_tag(Tag::StripOffsets)?.into_u64_vec()?,
                dec.get_tag(Tag::StripByteCounts)?.into_u64_vec()?,
            ),
        };
        Ok(Layout {
            width,
            height,
            samples,
            planar,
            bytes_per_sample: (bits as usize / 8).max(1),
            sub_byte_bits: if bits < 8 { bits as usize } else { 0 },
            chunk_width,
            chunk_height,
            offsets,
            byte_counts: counts,
            predictor: dec.find_tag_unsigned::<u16>(Tag::Predictor)?.unwrap_or(1),
        })
    }
}

/// Decodes every strip or tile into per-band `f64` vectors.
fn read_chunks<R: std::io::Read + std::io::Seek>(
    file: &mut R,
    l: &Layout,
    compression: u16,
    big_endian: bool,
    data_type: DataType,
) -> std::result::Result<Vec<Vec<f64>>, String> {
    use std::io::SeekFrom;
    let across = l.width.div_ceil(l.chunk_width);
    let down = l.height.div_ceil(l.chunk_height);
    let per_band = across * down;
    let chunk_samples = if l.planar { 1 } else { l.samples };
    let bps = l.bytes_per_sample;
    let row_bytes = if l.sub_byte_bits > 0 {
        (l.chunk_width * chunk_samples * l.sub_byte_bits).div_ceil(8)
    } else {
        l.chunk_width * chunk_samples * bps
    };
    let mut bands = vec![vec![0.0; l.width * l.height]; l.samples];
    let chunk_count = if l.planar {
        per_band * l.samples
    } else {
        per_band
    };
    if l.offsets.len() < chunk_count || l.byte_counts.len() < chunk_count {
        return Err(format!(
            "{} chunk offsets for {chunk_count} chunks",
            l.offsets.len()
        ));
    }
    let mut raw = Vec::new();
    for c in 0..chunk_count {
        let (band0, i) = (c / per_band, c % per_band);
        let (cx, cy) = (i % across * l.chunk_width, i / across * l.chunk_height);
        // Strips at the bottom may be short; tiles are always full size.
        let rows = l.chunk_height.min(l.height - cy);
        let expected = row_bytes
            * if l.chunk_width == l.width {
                rows
            } else {
                l.chunk_height
            };
        raw.clear();
        raw.resize(l.byte_counts[c] as usize, 0);
        if l.byte_counts[c] > 0 {
            file.seek(SeekFrom::Start(l.offsets[c]))
                .map_err(|e| e.to_string())?;
            file.read_exact(&mut raw).map_err(|e| e.to_string())?;
        }
        let mut bytes = if l.byte_counts[c] == 0 {
            vec![0; expected] // sparse chunk
        } else {
            decompress(&raw, compression, expected)?
        };
        bytes.resize(expected, 0);
        let chunk_rows = expected / row_bytes;
        for r in 0..chunk_rows {
            let row = &mut bytes[r * row_bytes..(r + 1) * row_bytes];
            unpredict(row, l.predictor, bps, chunk_samples, big_endian)?;
        }
        let float_planes = l.predictor == 3;
        for r in 0..rows {
            let y = cy + r;
            let row = &bytes[r * row_bytes..(r + 1) * row_bytes];
            for x in cx..(cx + l.chunk_width).min(l.width) {
                for s in 0..chunk_samples {
                    let k = (x - cx) * chunk_samples + s;
                    let v = if l.sub_byte_bits > 0 {
                        let bit = k * l.sub_byte_bits;
                        let byte = row[bit / 8];
                        let shift = 8 - l.sub_byte_bits - bit % 8;
                        ((byte >> shift) & ((1u8 << l.sub_byte_bits) - 1)) as f64
                    } else {
                        let at = k * bps;
                        sample(&row[at..at + bps], data_type, big_endian || float_planes)
                    };
                    let band = if l.planar { band0 } else { s };
                    bands[band][y * l.width + x] = v;
                }
            }
        }
    }
    Ok(bands)
}

pub(crate) fn decompress(
    raw: &[u8],
    compression: u16,
    expected: usize,
) -> std::result::Result<Vec<u8>, String> {
    match compression {
        1 => Ok(raw.to_vec()),
        5 => {
            // libtiff ends each chunk with an end code but some writers do not;
            // stop at the end of the input either way.
            let mut out = Vec::with_capacity(expected);
            let mut decoder =
                weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
            let result = decoder.into_vec(&mut out).decode(raw);
            result.status.map_err(|e| format!("LZW: {e}"))?;
            Ok(out)
        }
        8 | 32946 => {
            use std::io::Read;
            let mut out = Vec::with_capacity(expected);
            flate2::read::ZlibDecoder::new(raw)
                .read_to_end(&mut out)
                .map_err(|e| format!("Deflate: {e}"))?;
            Ok(out)
        }
        32773 => {
            let mut out = Vec::with_capacity(expected);
            let mut i = 0;
            while i < raw.len() && out.len() < expected {
                let n = raw[i] as i8;
                i += 1;
                if n >= 0 {
                    let len = n as usize + 1;
                    out.extend_from_slice(&raw[i..(i + len).min(raw.len())]);
                    i += len;
                } else if n != -128 {
                    if let Some(&b) = raw.get(i) {
                        out.extend(std::iter::repeat_n(b, (1 - n as isize) as usize));
                    }
                    i += 1;
                }
            }
            Ok(out)
        }
        other => Err(format!("compression {other}")),
    }
}

/// Undoes the horizontal (2) or floating-point (3) predictor on one row in place.
/// After predictor 3 the row holds each value's bytes in big-endian order.
fn unpredict(
    row: &mut [u8],
    predictor: u16,
    bps: usize,
    samples: usize,
    big_endian: bool,
) -> std::result::Result<(), String> {
    match predictor {
        1 => {}
        2 => {
            let stride = samples * bps;
            for i in (stride..row.len()).step_by(bps) {
                let (prev, cur) = row.split_at_mut(i);
                let p = &prev[i - stride..i - stride + bps];
                let c = &mut cur[..bps];
                let read = |b: &[u8]| -> u64 {
                    let mut buf = [0u8; 8];
                    if big_endian {
                        buf[8 - bps..].copy_from_slice(b);
                        u64::from_be_bytes(buf)
                    } else {
                        buf[..bps].copy_from_slice(b);
                        u64::from_le_bytes(buf)
                    }
                };
                let sum = read(c).wrapping_add(read(p));
                if big_endian {
                    c.copy_from_slice(&sum.to_be_bytes()[8 - bps..]);
                } else {
                    c.copy_from_slice(&sum.to_le_bytes()[..bps]);
                }
            }
        }
        3 => {
            for i in samples..row.len() {
                row[i] = row[i].wrapping_add(row[i - samples]);
            }
            let count = row.len() / bps;
            let planes = row.to_vec();
            for i in 0..count {
                for b in 0..bps {
                    row[i * bps + b] = planes[b * count + i];
                }
            }
        }
        other => return Err(format!("predictor {other}")),
    }
    Ok(())
}

fn sample(b: &[u8], data_type: DataType, big_endian: bool) -> f64 {
    macro_rules! num {
        ($t:ty) => {{
            let a = b.try_into().unwrap();
            (if big_endian {
                <$t>::from_be_bytes(a)
            } else {
                <$t>::from_le_bytes(a)
            }) as f64
        }};
    }
    match data_type {
        DataType::U8 => b[0] as f64,
        DataType::I8 => b[0] as i8 as f64,
        DataType::U16 => num!(u16),
        DataType::I16 => num!(i16),
        DataType::U32 => num!(u32),
        DataType::I32 => num!(i32),
        DataType::F32 => num!(f32),
        DataType::F64 => num!(f64),
    }
}

fn geotransform<R: std::io::Read + std::io::Seek>(
    dec: &mut Decoder<R>,
) -> tiff::TiffResult<[f64; 6]> {
    if let Some(m) = dec.find_tag(Tag::Unknown(MODEL_TRANSFORMATION))? {
        let m = m.into_f64_vec()?;
        if m.len() >= 8 {
            return Ok([m[3], m[0], m[1], m[7], m[4], m[5]]);
        }
    }
    let scale = dec
        .find_tag(Tag::ModelPixelScaleTag)?
        .map(|v| v.into_f64_vec())
        .transpose()?;
    let tie = dec
        .find_tag(Tag::ModelTiepointTag)?
        .map(|v| v.into_f64_vec())
        .transpose()?;
    Ok(match (scale, tie) {
        (Some(s), Some(t)) if s.len() >= 2 && t.len() >= 6 => {
            let (i, j, x, y) = (t[0], t[1], t[3], t[4]);
            [x - i * s[0], s[0], 0.0, y + j * s[1], 0.0, -s[1]]
        }
        _ => [0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    })
}

fn parse_nodata(text: &str) -> Option<f64> {
    let t = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    match t.to_ascii_lowercase().as_str() {
        "nan" | "-nan" => Some(f64::NAN),
        "inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        _ => t.parse().ok(),
    }
}

/// Writes `raster` as a striped GeoTIFF (see [`crate::write_geotiff_with`] for
/// tiles, predictors and overviews).
pub fn write_geotiff(path: &Path, raster: &Raster, compression: Compression) -> Result<()> {
    crate::write_geotiff_with(
        path,
        raster,
        &crate::WriteOptions {
            compression,
            ..Default::default()
        },
    )
}

/// The GeoKeyDirectory for an EPSG code (raster type PixelIsArea).
pub(crate) fn geo_keys(epsg: Option<u32>) -> Vec<u16> {
    let mut keys: Vec<u16> = vec![1, 1, 0, 0];
    let mut key = |id: u16, value: u16| keys.extend([id, 0, 1, value]);
    match epsg {
        Some(code) if code <= u16::MAX as u32 && is_geographic_code(code) => {
            key(GT_MODEL_TYPE, 2);
            key(GT_RASTER_TYPE, 1);
            key(GEOGRAPHIC_TYPE, code as u16);
        }
        Some(code) if code <= u16::MAX as u32 => {
            key(GT_MODEL_TYPE, 1);
            key(GT_RASTER_TYPE, 1);
            key(PROJECTED_CS_TYPE, code as u16);
        }
        _ => key(GT_RASTER_TYPE, 1),
    }
    keys[3] = (keys.len() as u16 - 4) / 4;
    keys
}

pub(crate) fn format_nodata(v: f64, data_type: DataType) -> String {
    if v.is_nan() {
        "nan".into()
    } else if data_type.is_float() {
        // Shortest text of the stored value, read back exactly.
        format!("{}", data_type.store(v))
    } else if v.is_finite() && v >= i64::MIN as f64 && v < i64::MAX as f64 {
        format!("{}", v as i64)
    } else {
        // ArcGIS can retain a floating sentinel far outside integer storage
        // (e.g. 3.4e38 on signed16 output). Do not saturate that metadata to i64.
        format!("{v}")
    }
}

/// EPSG identifiers are not a coordinate-system type range (e.g. GDA2020 is 7844).
fn is_geographic_code(code: u32) -> bool {
    crate::geographic_crs::GEOGRAPHIC_EPSG
        .binary_search(&code)
        .is_ok()
}
