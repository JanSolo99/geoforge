//! Raster datasets stored in a file geodatabase (read-only).
//!
//! Layout (from GDAL's OpenFileGDB reverse engineering, `docs/formats/filegdb.md` › Raster
//! datasets): a raster dataset `<name>` is a `GDB_Items` item of type `Raster Dataset` with four
//! tables beside its own:
//!
//! - `fras_bnd_<name>`: one row per band: `sequence_nbr` (band order), `raster_id`, `band_width`,
//!   `band_height`, `band_types` (pixel type and compression bits), `block_width/height`,
//!   `block_origin_x/y` and the extent `eminx…emaxy` (pixel *centres*), and `srid` (the row of
//!   `GDB_SpatialRefs` holding its coordinate system).
//! - `fras_blk_<name>`: one row per stored block: `rasterband_id`, `rrd_factor` (0 = full
//!   resolution, then overviews), `row_nbr`, `col_nbr`, `block_data`. Missing blocks are empty.
//! - `fras_ras_<name>`, `fras_aux_<name>`: storage definition, statistics, attribute tables.
//!
//! `band_types`: bits 19–25 the bit width (1, 2, 4, 8, 16, 32, 64), bits 16–17 the kind
//! (0 unsigned, 1 signed, 2 floating point; 64 bits with kind 0 is a double), bits 8–15 the
//! compression (0 none, 4 zlib "LZ77", 8 JPEG, 12 JPEG 2000). Pixels are big-endian, sub-byte
//! ones packed most significant bit first, rows top to bottom. A block may end with a 1-bit
//! validity mask (`ceil(n / 8)` bytes, most significant bit first, 1 = valid); for JPEG the
//! blob starts with `0x01` (no mask) or `0xFE` + a little-endian u32 JPEG length, the zlib mask
//! following the JPEG (JPEG 2000: `0x00`, or `0xFF` with a mask). Each JPEG is one band's block.

use std::collections::HashMap;

use unigis_core::{Error, Result, Value};

use crate::Geodatabase;
use crate::table::Table;

/// How a band's pixels are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelType {
    U1,
    U2,
    U4,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    F64,
}

impl PixelType {
    fn from_bits(width: u32, kind: u32) -> Option<PixelType> {
        Some(match (width, kind) {
            (1, 0) => PixelType::U1,
            (2, 0) => PixelType::U2,
            (4, 0) => PixelType::U4,
            (8, 0) => PixelType::U8,
            (8, 1) => PixelType::I8,
            (16, 0) => PixelType::U16,
            (16, 1) => PixelType::I16,
            (32, 0) => PixelType::U32,
            (32, 1) => PixelType::I32,
            (32, 2) => PixelType::F32,
            (64, 0) | (64, 2) => PixelType::F64,
            _ => return None,
        })
    }

    /// Bits per stored pixel.
    pub fn bits(self) -> usize {
        match self {
            PixelType::U1 => 1,
            PixelType::U2 => 2,
            PixelType::U4 => 4,
            PixelType::U8 | PixelType::I8 => 8,
            PixelType::U16 | PixelType::I16 => 16,
            PixelType::U32 | PixelType::I32 | PixelType::F32 => 32,
            PixelType::F64 => 64,
        }
    }

    /// ArcGIS's name for it (Describe's `pixelType`).
    pub fn arcgis_name(self) -> &'static str {
        match self {
            PixelType::U1 => "U1",
            PixelType::U2 => "U2",
            PixelType::U4 => "U4",
            PixelType::U8 => "U8",
            PixelType::I8 => "S8",
            PixelType::U16 => "U16",
            PixelType::I16 => "S16",
            PixelType::U32 => "U32",
            PixelType::I32 => "S32",
            PixelType::F32 => "F32",
            PixelType::F64 => "F64",
        }
    }

    /// Whether it is a floating-point type.
    pub fn is_float(self) -> bool {
        matches!(self, PixelType::F32 | PixelType::F64)
    }

    fn decode(self, b: &[u8], i: usize) -> f64 {
        let at = |n: usize| b.get(i * n..i * n + n);
        match self {
            PixelType::U1 | PixelType::U2 | PixelType::U4 => {
                let bits = self.bits();
                let mut v = 0u32;
                for k in 0..bits {
                    let bit = i * bits + k;
                    if b.get(bit / 8)
                        .is_some_and(|byte| byte & (0x80 >> (bit % 8)) != 0)
                    {
                        v |= 1 << (bits - 1 - k);
                    }
                }
                f64::from(v)
            }
            PixelType::U8 => at(1).map_or(0.0, |x| f64::from(x[0])),
            PixelType::I8 => at(1).map_or(0.0, |x| f64::from(x[0] as i8)),
            PixelType::U16 => at(2)
                .and_then(|x| x.try_into().ok())
                .map_or(0.0, |x| f64::from(u16::from_be_bytes(x))),
            PixelType::I16 => at(2)
                .and_then(|x| x.try_into().ok())
                .map_or(0.0, |x| f64::from(i16::from_be_bytes(x))),
            PixelType::U32 => at(4)
                .and_then(|x| x.try_into().ok())
                .map_or(0.0, |x| f64::from(u32::from_be_bytes(x))),
            PixelType::I32 => at(4)
                .and_then(|x| x.try_into().ok())
                .map_or(0.0, |x| f64::from(i32::from_be_bytes(x))),
            PixelType::F32 => at(4)
                .and_then(|x| x.try_into().ok())
                .map_or(0.0, |x| f64::from(f32::from_be_bytes(x))),
            PixelType::F64 => at(8)
                .and_then(|x| x.try_into().ok())
                .map_or(0.0, f64::from_be_bytes),
        }
    }
}

/// How a band's blocks are compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    /// zlib (ArcGIS's "LZ77").
    Lz77,
    Jpeg,
    Jpeg2000,
}

/// A raster dataset of a geodatabase, opened.
pub struct RasterDataset {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub pixel_type: PixelType,
    pub compression: Compression,
    /// GDAL-order geotransform of the full-resolution grid (top-left corner, pixel size).
    pub geotransform: [f64; 6],
    /// The coordinate system's WKT, when the geodatabase has it.
    pub srs_wkt: Option<String>,
    /// Pyramid levels stored beside full resolution.
    pub overviews: usize,
    block_width: usize,
    block_height: usize,
    shift_x: i64,
    shift_y: i64,
    /// `fras_bnd` row ids (the blocks' `rasterband_id`), in band order.
    band_ids: Vec<i64>,
    blocks: Table,
    /// (band id, level, row, col) → OBJECTID of the block's row.
    index: HashMap<(i64, i64, i64, i64), u64>,
    block_data: usize,
}

/// One band's pixels at full resolution, rows top to bottom, with which are valid.
pub struct Band {
    pub values: Vec<f64>,
    /// `None` when every pixel is valid.
    pub valid: Option<Vec<bool>>,
}

/// A decoded block: its pixels and, when it has a mask, which are valid.
type Block = (Vec<f64>, Option<Vec<bool>>);

/// Largest raster read whole (pixels per band).
const MAX_PIXELS: usize = 1 << 28;

fn bad(msg: impl Into<String>) -> Error {
    Error::format("FileGDB raster", msg.into())
}

fn int(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::SmallInteger(i) => Some(i64::from(*i)),
        Value::Integer(i) => Some(i64::from(*i)),
        Value::BigInteger(i) => Some(*i),
        _ => None,
    }
}

fn real(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Double(f) => Some(*f),
        Value::Single(f) => Some(f64::from(*f)),
        other => int(Some(other)).map(|i| i as f64),
    }
}

impl Geodatabase {
    /// The raster datasets, by name.
    pub fn raster_datasets(&self) -> Vec<String> {
        self.items()
            .iter()
            .filter(|i| i.kind == "Raster Dataset")
            .map(|i| i.name.clone())
            .collect()
    }

    /// Opens a raster dataset by name (case-insensitive).
    pub fn open_raster(&self, name: &str) -> Result<RasterDataset> {
        let name = self
            .raster_datasets()
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| bad(format!("no raster dataset {name}")))?;
        let table_named = |prefix: &str| -> Result<Table> {
            let t = format!("{prefix}{name}");
            let id = self
                .catalog()
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&t))
                .map(|(_, id)| *id)
                .ok_or_else(|| bad(format!("{name}: no {t} table")))?;
            Table::open_in(
                self.store(),
                self.path().join(format!("a{id:08x}.gdbtable")),
            )
        };
        let bnd = table_named("fras_bnd_")?;
        let col = |t: &Table, n: &str| {
            t.fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(n))
        };
        let need =
            |t: &Table, n: &str| col(t, n).ok_or_else(|| bad(format!("{name}: no field {n}")));
        let (seq, rid) = (need(&bnd, "sequence_nbr")?, need(&bnd, "raster_id")?);
        let mut bands: Vec<(i64, i64, Vec<Value>)> = Vec::new();
        for row in bnd.features() {
            let row = row?;
            // Only the dataset's own raster (raster_id 1), as GDAL reads it.
            if int(row.attributes.get(rid)) != Some(1) {
                continue;
            }
            bands.push((
                int(row.attributes.get(seq)).unwrap_or(0),
                row.oid,
                row.attributes,
            ));
        }
        bands.sort_by_key(|b| b.0);
        let first = bands
            .first()
            .ok_or_else(|| bad(format!("{name}: no bands")))?
            .2
            .clone();
        let get_i = |n: &str| -> Result<i64> {
            int(first.get(need(&bnd, n)?)).ok_or_else(|| bad(format!("{name}: {n} unset")))
        };
        let get_f = |n: &str| -> Result<f64> {
            real(first.get(need(&bnd, n)?)).ok_or_else(|| bad(format!("{name}: {n} unset")))
        };
        let (mut width, mut height) = (get_i("band_width")?, get_i("band_height")?);
        let (bw, bh) = (get_i("block_width")?, get_i("block_height")?);
        if !(1..=32_768).contains(&bw) || !(1..=32_768).contains(&bh) || width <= 0 || height <= 0 {
            return Err(bad(format!(
                "{name}: impossible size {width}×{height}, blocks {bw}×{bh}"
            )));
        }
        let types = get_i("band_types")? as u32;
        let pixel_type = PixelType::from_bits((types >> 19) & 0x7f, (types >> 16) & 0x3)
            .ok_or_else(|| bad(format!("{name}: pixel type {:#x} isn't known", types)))?;
        let compression = match (types >> 8) & 0xff {
            0 => Compression::None,
            4 => Compression::Lz77,
            8 => Compression::Jpeg,
            12 => Compression::Jpeg2000,
            c => return Err(bad(format!("{name}: compression {c} isn't known"))),
        };
        let (minx, miny, maxx, maxy) = (
            get_f("eminx")?,
            get_f("eminy")?,
            get_f("emaxx")?,
            get_f("emaxy")?,
        );
        let (ox, oy) = (get_f("block_origin_x")?, get_f("block_origin_y")?);
        if !(maxx > minx && maxy > miny) || width < 2 || height < 2 {
            return Err(bad(format!("{name}: its extent doesn't give a cell size")));
        }
        // The extent is of pixel centres; the grid is registered on the block origin.
        let (rx, ry) = (
            (maxx - minx) / (width - 1) as f64,
            (maxy - miny) / (height - 1) as f64,
        );
        let (gw, gh) = (rx * bw as f64, ry * bh as f64);
        let mut shift_x = 0i64;
        if minx != ox {
            shift_x = ((minx - ox) / gw).floor() as i64;
            width = 1 + ((maxx - (ox + shift_x as f64 * gw)) / rx).round() as i64;
        }
        let mut shift_y = 0i64;
        if maxy != oy {
            shift_y = ((oy - maxy) / gh).floor() as i64;
            height = 1 + (((oy - shift_y as f64 * gh) - miny) / ry).round() as i64;
        }
        let geotransform = [
            ox + shift_x as f64 * gw - rx / 2.0,
            rx,
            0.0,
            oy - shift_y as f64 * gh + ry / 2.0,
            0.0,
            -ry,
        ];
        let srid = col(&bnd, "srid")
            .and_then(|i| int(first.get(i)))
            .unwrap_or(0);
        let srs_wkt = self.spatial_ref_text(srid);
        let blocks = table_named("fras_blk_")?;
        let (bi, li, ri, ci) = (
            need(&blocks, "rasterband_id")?,
            need(&blocks, "rrd_factor")?,
            need(&blocks, "row_nbr")?,
            need(&blocks, "col_nbr")?,
        );
        let block_data = need(&blocks, "block_data")?;
        let mut index = HashMap::new();
        let mut overviews = 0;
        for row in blocks.features() {
            let row = row?;
            let a = &row.attributes;
            let (Some(b), Some(l), Some(r), Some(c)) = (
                int(a.get(bi)),
                int(a.get(li)),
                int(a.get(ri)),
                int(a.get(ci)),
            ) else {
                continue;
            };
            overviews = overviews.max(l.max(0) as usize);
            index.insert((b, l, r, c), row.oid as u64);
        }
        let (width, height) = (width as usize, height as usize);
        Ok(RasterDataset {
            name,
            width,
            height,
            pixel_type,
            compression,
            geotransform,
            srs_wkt,
            overviews,
            block_width: bw as usize,
            block_height: bh as usize,
            shift_x,
            shift_y,
            band_ids: bands.iter().map(|b| b.1).collect(),
            blocks,
            index,
            block_data,
        })
    }

    /// The WKT of a `GDB_SpatialRefs` row (1-based), if it holds one.
    fn spatial_ref_text(&self, srid: i64) -> Option<String> {
        let id = self
            .catalog()
            .iter()
            .find(|(n, _)| n == "GDB_SpatialRefs")
            .map(|(_, id)| *id)?;
        let t = Table::open_in(
            self.store(),
            self.path().join(format!("a{id:08x}.gdbtable")),
        )
        .ok()?;
        let i = t
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case("SRTEXT"))?;
        let row = t.get(u64::try_from(srid).ok().filter(|s| *s > 0)?).ok()??;
        match row.attributes.get(i)? {
            Value::String(s) if !s.starts_with('{') && !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }
}

impl RasterDataset {
    pub fn band_count(&self) -> usize {
        self.band_ids.len()
    }

    /// Reads a band (0-based) at full resolution.
    pub fn read_band(&self, band: usize) -> Result<Band> {
        let id = *self
            .band_ids
            .get(band)
            .ok_or_else(|| bad(format!("{}: no band {}", self.name, band + 1)))?;
        let (w, h, bw, bh) = (self.width, self.height, self.block_width, self.block_height);
        if w.saturating_mul(h) > MAX_PIXELS {
            return Err(bad(format!(
                "{}: {w}×{h} pixels is too large to read whole",
                self.name
            )));
        }
        let mut values = vec![0.0; w * h];
        let mut valid = vec![false; w * h];
        let mut all_valid = true;
        for by in 0..h.div_ceil(bh) {
            for bx in 0..w.div_ceil(bw) {
                let key = (id, 0, by as i64 + self.shift_y, bx as i64 + self.shift_x);
                let Some(block) = self.block(key)? else {
                    all_valid = false;
                    continue;
                };
                for y in 0..bh {
                    let row = by * bh + y;
                    if row >= h {
                        break;
                    }
                    for x in 0..bw {
                        let col = bx * bw + x;
                        if col >= w {
                            break;
                        }
                        let i = y * bw + x;
                        let ok = block
                            .1
                            .as_ref()
                            .is_none_or(|m| m.get(i).copied().unwrap_or(false));
                        values[row * w + col] = block.0.get(i).copied().unwrap_or(0.0);
                        valid[row * w + col] = ok;
                        all_valid &= ok;
                    }
                }
            }
        }
        Ok(Band {
            values,
            valid: (!all_valid).then_some(valid),
        })
    }

    /// One block's pixels and validity, or `None` when it isn't stored.
    fn block(&self, key: (i64, i64, i64, i64)) -> Result<Option<Block>> {
        let Some(&oid) = self.index.get(&key) else {
            return Ok(None);
        };
        let Some(row) = self.blocks.get(oid)? else {
            return Ok(None);
        };
        let Some(Value::Blob(data)) = row.attributes.get(self.block_data) else {
            return Ok(None);
        };
        if data.is_empty() {
            return Ok(None);
        }
        let n = self.block_width * self.block_height;
        let packed = (n * self.pixel_type.bits()).div_ceil(8);
        let mask_len = n.div_ceil(8);
        let mask_of = |m: &[u8]| -> Vec<bool> {
            (0..n)
                .map(|i| m.get(i / 8).is_some_and(|b| b & (0x80 >> (i % 8)) != 0))
                .collect()
        };
        let pixels =
            |b: &[u8]| -> Vec<f64> { (0..n).map(|i| self.pixel_type.decode(b, i)).collect() };
        let raw;
        let bytes: &[u8] = match self.compression {
            Compression::None => data,
            Compression::Lz77 => {
                use std::io::Read;
                let mut out = Vec::with_capacity(packed + mask_len);
                flate2::read::ZlibDecoder::new(data.as_slice())
                    .take((packed + mask_len) as u64 + 1)
                    .read_to_end(&mut out)
                    .map_err(|e| bad(format!("{}: a block doesn't inflate: {e}", self.name)))?;
                raw = out;
                &raw
            }
            Compression::Jpeg => return self.jpeg_block(data, n, mask_len, &mask_of).map(Some),
            Compression::Jpeg2000 => {
                return self.jpeg2000_block(data, n, mask_len, &mask_of).map(Some);
            }
        };
        if bytes.len() == packed {
            Ok(Some((pixels(bytes), None)))
        } else if bytes.len() == packed + mask_len {
            Ok(Some((
                pixels(&bytes[..packed]),
                Some(mask_of(&bytes[packed..])),
            )))
        } else {
            Err(bad(format!(
                "{}: a block holds {} bytes, not {packed} (+{mask_len} mask)",
                self.name,
                bytes.len()
            )))
        }
    }

    fn jpeg2000_block(
        &self,
        data: &[u8],
        n: usize,
        mask_len: usize,
        mask_of: &dyn Fn(&[u8]) -> Vec<bool>,
    ) -> Result<(Vec<f64>, Option<Vec<bool>>)> {
        let (j2k, mask) = match data.first() {
            Some(0) => (data.get(1..).unwrap_or_default(), None),
            Some(0xFF) => {
                let len = data
                    .get(1..5)
                    .and_then(|b| b.try_into().ok())
                    .map(u32::from_le_bytes)
                    .unwrap_or(u32::MAX) as usize;
                let j2k = data.get(5..5usize.saturating_add(len)).ok_or_else(|| {
                    bad(format!(
                        "{}: a JPEG 2000 block's length is wrong",
                        self.name
                    ))
                })?;
                use std::io::Read;
                let mut m = Vec::with_capacity(mask_len);
                flate2::read::ZlibDecoder::new(data.get(5 + len..).unwrap_or_default())
                    .take(mask_len as u64 + 1)
                    .read_to_end(&mut m)
                    .map_err(|e| bad(e.to_string()))?;
                (j2k, (m.len() == mask_len).then(|| mask_of(&m)))
            }
            _ => {
                return Err(bad(format!(
                    "{}: a JPEG 2000 block's header isn't known",
                    self.name
                )));
            }
        };
        let settings = hayro_jpeg2000::DecodeSettings {
            resolve_palette_indices: false,
            ..Default::default()
        };
        let image = hayro_jpeg2000::Image::new(j2k, &settings)
            .map_err(|e| bad(format!("{}: a JPEG 2000 block: {e:?}", self.name)))?;
        let mut ctx = hayro_jpeg2000::DecoderContext::default();
        let decoded = image
            .decode(&mut ctx)
            .map_err(|e| bad(format!("{}: a JPEG 2000 block: {e:?}", self.name)))?;
        let Some(c) = decoded.components().first() else {
            return Err(bad(format!(
                "{}: a JPEG 2000 block without samples",
                self.name
            )));
        };
        let samples = c.samples();
        if samples.len() < n {
            return Err(bad(format!(
                "{}: a JPEG 2000 block is smaller than the block",
                self.name
            )));
        }
        // Samples come unclamped: as a decoder outputs them, within the component's bit depth.
        let max = f64::from((1u32 << c.bit_depth().clamp(1, 31)) - 1);
        Ok((
            samples[..n]
                .iter()
                .map(|&v| f64::from(v.round()).clamp(0.0, max))
                .collect(),
            mask,
        ))
    }

    fn jpeg_block(
        &self,
        data: &[u8],
        n: usize,
        mask_len: usize,
        mask_of: &dyn Fn(&[u8]) -> Vec<bool>,
    ) -> Result<(Vec<f64>, Option<Vec<bool>>)> {
        let (jpeg, mask) = match data.first() {
            Some(1) => (data.get(1..).unwrap_or_default(), None),
            Some(0xFE) => {
                let len = data
                    .get(1..5)
                    .and_then(|b| b.try_into().ok())
                    .map(u32::from_le_bytes)
                    .unwrap_or(u32::MAX) as usize;
                let jpeg = data
                    .get(5..5usize.saturating_add(len))
                    .ok_or_else(|| bad(format!("{}: a JPEG block's length is wrong", self.name)))?;
                let rest = data.get(5 + len..).unwrap_or_default();
                use std::io::Read;
                let mut m = Vec::with_capacity(mask_len);
                flate2::read::ZlibDecoder::new(rest)
                    .take(mask_len as u64 + 1)
                    .read_to_end(&mut m)
                    .map_err(|e| bad(e.to_string()))?;
                (jpeg, (m.len() == mask_len).then(|| mask_of(&m)))
            }
            _ => {
                return Err(bad(format!(
                    "{}: a JPEG block's header isn't known",
                    self.name
                )));
            }
        };
        let with_tables = with_standard_huffman_tables(jpeg);
        let jpeg = with_tables.as_deref().unwrap_or(jpeg);
        let options = zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::Luma);
        let mut d = zune_jpeg::JpegDecoder::new_with_options(
            zune_core::bytestream::ZCursor::new(jpeg),
            options,
        );
        let px = d
            .decode()
            .map_err(|e| bad(format!("{}: a JPEG block: {e:?}", self.name)))?;
        if px.len() < n {
            return Err(bad(format!(
                "{}: a JPEG block is smaller than the block",
                self.name
            )));
        }
        Ok((px[..n].iter().map(|&v| f64::from(v)).collect(), mask))
    }
}

/// The JPEG standard's typical Huffman tables (ITU T.81 Annex K.3): DC and AC, luminance and
/// chrominance, as one DHT segment. ArcGIS leaves them out of some blocks (as Motion JPEG does),
/// counting on a decoder to supply them, as libjpeg does.
const STANDARD_DHT: &[u8] = &[
    0xFF, 0xC4, 0x01, 0xA2, //
    // DC luminance (class 0, id 0)
    0x00, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    11, //
    // DC chrominance (class 0, id 1)
    0x01, 0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    11, //
    // AC luminance (class 1, id 0)
    0x10, 0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d, //
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa, //
    // AC chrominance (class 1, id 1)
    0x11, 0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77, //
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// `jpeg` with the standard Huffman tables after its SOI marker when it has none of its own
/// before its first scan; `None` when it needs nothing.
fn with_standard_huffman_tables(jpeg: &[u8]) -> Option<Vec<u8>> {
    if jpeg.get(..2) != Some(&[0xFF, 0xD8]) {
        return None;
    }
    // Walk the marker segments up to the first scan (SOS), looking for a DHT.
    let mut at = 2;
    while let (Some(&0xFF), Some(&m)) = (jpeg.get(at), jpeg.get(at + 1)) {
        match m {
            0xC4 => return None,
            0xDA => break,
            0xD0..=0xD9 | 0x01 | 0xFF => at += if m == 0xFF { 1 } else { 2 },
            _ => {
                let len = jpeg
                    .get(at + 2..at + 4)
                    .map(|b| usize::from(u16::from_be_bytes([b[0], b[1]])))?;
                at += 2 + len;
            }
        }
    }
    let mut out = Vec::with_capacity(jpeg.len() + STANDARD_DHT.len());
    out.extend_from_slice(&jpeg[..2]);
    out.extend_from_slice(STANDARD_DHT);
    out.extend_from_slice(&jpeg[2..]);
    Some(out)
}
