//! `.gdbtable` / `.gdbtablx`: one table or feature class.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use chrono::{Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
use geoforge_core::{
    Envelope, Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value,
};

use crate::bytes::{Reader, err};
use crate::geometry;

/// The CLSID written instead of WKT when a layer has no spatial reference.
const UNKNOWN_SRS: &str = "{B286C06B-0879-11D2-AACA-00C04FA33C20}";

/// FileGDB field type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GdbFieldType {
    Int16,
    Int32,
    Float32,
    Float64,
    String,
    DateTime,
    ObjectId,
    Geometry,
    Binary,
    Raster,
    Guid,
    GlobalId,
    Xml,
    Int64,
    DateOnly,
    TimeOnly,
    DateTimeOffset,
}

impl GdbFieldType {
    fn from_code(code: u8) -> Option<Self> {
        use GdbFieldType::*;
        Some(match code {
            0 => Int16,
            1 => Int32,
            2 => Float32,
            3 => Float64,
            4 => String,
            5 => DateTime,
            6 => ObjectId,
            7 => Geometry,
            8 => Binary,
            9 => Raster,
            10 => Guid,
            11 => GlobalId,
            12 => Xml,
            13 => Int64,
            14 => DateOnly,
            15 => TimeOnly,
            16 => DateTimeOffset,
            _ => return None,
        })
    }

    pub fn esri(self) -> FieldType {
        use GdbFieldType as G;
        match self {
            G::Int16 => FieldType::SmallInteger,
            G::Int32 => FieldType::Integer,
            G::Float32 => FieldType::Single,
            G::Float64 => FieldType::Double,
            G::String => FieldType::String,
            G::DateTime => FieldType::Date,
            G::ObjectId => FieldType::ObjectId,
            G::Geometry => FieldType::Geometry,
            G::Binary => FieldType::Blob,
            G::Raster => FieldType::Raster,
            G::Guid => FieldType::Guid,
            G::GlobalId => FieldType::GlobalId,
            G::Xml => FieldType::Xml,
            G::Int64 => FieldType::BigInteger,
            G::DateOnly => FieldType::DateOnly,
            G::TimeOnly => FieldType::TimeOnly,
            G::DateTimeOffset => FieldType::TimestampOffset,
        }
    }
}

/// Coordinate storage parameters of a geometry field.
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryDef {
    /// Esri WKT, or `None` for an unknown coordinate system.
    pub srs_wkt: Option<String>,
    pub has_z: bool,
    pub has_m: bool,
    pub x_origin: f64,
    pub y_origin: f64,
    pub xy_scale: f64,
    pub m_origin: f64,
    pub m_scale: f64,
    pub z_origin: f64,
    pub z_scale: f64,
    pub xy_tolerance: f64,
    pub m_tolerance: f64,
    pub z_tolerance: f64,
    pub extent: Envelope,
    pub grid_sizes: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct GdbField {
    pub name: String,
    pub alias: Option<String>,
    pub field_type: GdbFieldType,
    pub nullable: bool,
    /// Byte width for fixed-size types, maximum length for strings.
    pub length: u32,
    pub default: Option<Vec<u8>>,
    pub geometry: Option<GeometryDef>,
    /// Raster fields: 0 external, 1 managed, 2 inline.
    pub raster_type: Option<u8>,
    /// Date fields: millisecond precision (ArcGIS Pro 3.2+). Legacy date fields hold
    /// whole seconds. Known only from the geodatabase's item definitions.
    pub high_precision: bool,
}

impl GdbField {
    pub fn to_field(&self) -> Field {
        let mut f = Field::new(&self.name, self.field_type.esri());
        f.alias = self.alias.clone();
        f.nullable = self.nullable;
        f.length = self.length;
        f
    }
}

/// An open table. Feature IDs are OBJECTIDs (1-based).
pub struct Table {
    file: File,
    path: PathBuf,
    pub(crate) utf8: bool,
    /// Uses ArcGIS Pro 3.2 extensions (64-bit OBJECTIDs or new field types).
    pro_3_2: bool,
    valid_rows: u64,
    geometry_type: u8,
    has_z: bool,
    has_m: bool,
    fields: Vec<GdbField>,
    oid_index: Option<usize>,
    shape_index: Option<usize>,
    attribute_fields: Vec<Field>,
    nullable_count: usize,
    tablx: Tablx,
}

impl Table {
    /// Opens `aXXXXXXXX.gdbtable` (the path may omit the extension).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().with_extension("gdbtable");
        if path.with_extension("gdbtable.cdf").is_file() {
            return Err(Error::Unsupported(
                "compressed (.cdf) tables; uncompress the geodatabase in ArcGIS first".into(),
            ));
        }
        let file = File::open(&path).map_err(|e| Error::io(&path, e))?;
        let io = |e| Error::io(&path, e);
        let file_len = file.metadata().map_err(io)?.len();

        let mut h = [0u8; 40];
        file.read_exact_at(&mut h, 0).map_err(io)?;
        let mut r = Reader::new(&h);
        let version = r.i32()?;
        let valid_rows = match version {
            3 => r.i32()? as u32 as u64,
            4 => {
                r.p = 16;
                r.i64()? as u64
            }
            v => return Err(err(format!("unsupported .gdbtable version {v}"))),
        };
        r.p = 32;
        let field_offset = r.i64()? as u64;

        let mut size = [0u8; 4];
        file.read_exact_at(&mut size, field_offset).map_err(io)?;
        let size = u32::from_le_bytes(size) as u64;
        if field_offset + 4 + size > file_len {
            return Err(err("field section extends past end of file"));
        }
        let mut section = vec![0u8; size as usize];
        file.read_exact_at(&mut section, field_offset + 4)
            .map_err(io)?;
        let mut r = Reader::new(&section);
        let field_version = r.i32()?;
        let flags = r.u32()?;
        let n = r.i16()?;
        let geometry_type = (flags & 0xff) as u8;
        let utf8 = flags & (1 << 8) != 0;
        let has_m = flags & (1 << 30) != 0;
        let has_z = flags & (1 << 31) != 0;

        let mut fields = Vec::with_capacity(n.max(0) as usize);
        for _ in 0..n {
            fields.push(read_field(&mut r, has_z, has_m)?);
        }
        let oid_index = fields
            .iter()
            .position(|f| f.field_type == GdbFieldType::ObjectId);
        let shape_index = fields
            .iter()
            .position(|f| f.field_type == GdbFieldType::Geometry);
        let attribute_fields = fields
            .iter()
            .filter(|f| {
                !matches!(
                    f.field_type,
                    GdbFieldType::ObjectId | GdbFieldType::Geometry
                )
            })
            .map(GdbField::to_field)
            .collect();
        let nullable_count = fields.iter().filter(|f| f.nullable).count();
        let tablx = Tablx::open(&path.with_extension("gdbtablx"))?;

        Ok(Self {
            file,
            path,
            utf8,
            pro_3_2: version == 4
                || field_version >= 6
                || fields.iter().any(|f| {
                    use GdbFieldType::*;
                    matches!(f.field_type, Int64 | DateOnly | TimeOnly | DateTimeOffset)
                }),
            valid_rows,
            geometry_type,
            has_z,
            has_m,
            fields,
            oid_index,
            shape_index,
            attribute_fields,
            nullable_count,
            tablx,
        })
    }

    /// All fields, including OBJECTID and the geometry field.
    pub fn all_fields(&self) -> &[GdbField] {
        &self.fields
    }

    /// Whether the table uses ArcGIS Pro 3.2 format extensions (64-bit OBJECTIDs,
    /// BigInteger/DateOnly/TimeOnly/TimestampOffset fields). Older software can't read it.
    pub fn requires_pro_3_2(&self) -> bool {
        self.pro_3_2
    }

    /// Marks date fields as high precision (from the item definition XML).
    pub fn set_high_precision(&mut self, field_names: &[String]) {
        for f in &mut self.fields {
            f.high_precision = field_names.iter().any(|n| n.eq_ignore_ascii_case(&f.name));
        }
    }

    /// Attribute fields: everything except OBJECTID and the geometry field.
    pub fn fields(&self) -> &[Field] {
        &self.attribute_fields
    }

    pub fn oid_field_name(&self) -> Option<&str> {
        self.oid_index.map(|i| self.fields[i].name.as_str())
    }

    pub fn shape_field(&self) -> Option<&GdbField> {
        self.shape_index.map(|i| &self.fields[i])
    }

    pub fn geometry_def(&self) -> Option<&GeometryDef> {
        self.shape_field().and_then(|f| f.geometry.as_ref())
    }

    /// The feature class geometry type, or `None` for a plain table.
    pub fn geometry_kind(&self) -> Option<GeometryKind> {
        self.shape_index?;
        Some(match self.geometry_type {
            1 => GeometryKind::Point,
            2 => GeometryKind::Multipoint,
            3 => GeometryKind::Polyline,
            4 => GeometryKind::Polygon,
            9 => GeometryKind::MultiPatch,
            _ => return None,
        })
    }

    pub fn has_z(&self) -> bool {
        self.has_z
    }

    pub fn has_m(&self) -> bool {
        self.has_m
    }

    /// Number of rows that are not deleted.
    pub fn row_count(&self) -> u64 {
        self.valid_rows
    }

    /// One more than the highest OBJECTID slot (deleted rows included).
    pub fn max_oid(&self) -> u64 {
        self.tablx.rows
    }

    /// Reads the row with OBJECTID `oid`; `None` if it is deleted or out of range.
    pub fn get(&self, oid: u64) -> Result<Option<Feature>> {
        let Some(offset) = self.tablx.offset(&self.path, oid)? else {
            return Ok(None);
        };
        let io = |e| Error::io(&self.path, e);
        let mut len = [0u8; 4];
        self.file.read_exact_at(&mut len, offset).map_err(io)?;
        let len = u32::from_le_bytes(len) as usize;
        let mut blob = vec![0u8; len];
        self.file.read_exact_at(&mut blob, offset + 4).map_err(io)?;
        self.decode_row(oid, &blob).map(Some)
    }

    /// All rows in OBJECTID order.
    pub fn features(&self) -> impl Iterator<Item = Result<Feature>> + '_ {
        self.tablx
            .candidate_oids()
            .filter_map(|oid| self.get(oid).transpose())
    }

    fn decode_row(&self, oid: u64, blob: &[u8]) -> Result<Feature> {
        let mut r = Reader::new(blob);
        let nulls = r.bytes(self.nullable_count.div_ceil(8))?;
        let mut nullable_i = 0;
        let mut geometry = None;
        let mut attributes = Vec::with_capacity(self.attribute_fields.len());
        for (i, f) in self.fields.iter().enumerate() {
            if f.field_type == GdbFieldType::ObjectId {
                continue;
            }
            let is_null = f.nullable && {
                let bit = nulls[nullable_i / 8] & (1 << (nullable_i % 8)) != 0;
                nullable_i += 1;
                bit
            };
            if Some(i) == self.shape_index {
                if !is_null {
                    let len = r.count(1)?;
                    let blob = r.bytes(len)?;
                    let def = f
                        .geometry
                        .as_ref()
                        .ok_or_else(|| err("geometry field without definition"))?;
                    geometry = geometry::decode(blob, def, self.has_z, self.has_m)
                        .map_err(|e| err(format!("OBJECTID {oid}: {e}")))?;
                }
                continue;
            }
            attributes.push(if is_null {
                Value::Null
            } else {
                self.read_value(&mut r, f)?
            });
        }
        Ok(Feature {
            oid: oid as i64,
            geometry,
            attributes,
        })
    }

    fn read_value(&self, r: &mut Reader, f: &GdbField) -> Result<Value> {
        use GdbFieldType as G;
        Ok(match f.field_type {
            G::Int16 => Value::SmallInteger(r.i16()?),
            G::Int32 => Value::Integer(r.i32()?),
            G::Int64 => Value::BigInteger(r.i64()?),
            G::Float32 => Value::Single(r.f32()?),
            G::Float64 => Value::Double(r.f64()?),
            G::String => {
                let n = r.count(1)?;
                Value::String(self.decode_string(r.bytes(n)?))
            }
            G::Xml => {
                let n = r.count(1)?;
                Value::Xml(String::from_utf8_lossy(r.bytes(n)?).into_owned())
            }
            G::Binary => {
                let n = r.count(1)?;
                Value::Blob(r.bytes(n)?.to_vec())
            }
            G::Raster => match f.raster_type {
                Some(1) => Value::Integer(r.i32()?),
                _ => {
                    let n = r.count(1)?;
                    Value::Blob(r.bytes(n)?.to_vec())
                }
            },
            G::Guid | G::GlobalId => Value::Guid(format_guid(r.bytes(16)?)),
            G::DateTime => {
                let days = r.f64()?;
                let t = if f.high_precision {
                    datetime(days)
                } else {
                    datetime_seconds(days)
                };
                t.map_or(Value::Null, Value::Date)
            }
            G::DateOnly => datetime(r.f64()?).map_or(Value::Null, |d| Value::DateOnly(d.date())),
            G::TimeOnly => {
                let ms = (r.f64()? * 86_400_000.0).round() as i64;
                Value::TimeOnly(
                    NaiveTime::MIN
                        .overflowing_add_signed(Duration::milliseconds(ms))
                        .0,
                )
            }
            G::DateTimeOffset => {
                let days = r.f64()?;
                let minutes = r.i16()?;
                match (
                    datetime(days),
                    FixedOffset::east_opt(i32::from(minutes) * 60),
                ) {
                    (Some(local), Some(tz)) => local
                        .and_local_timezone(tz)
                        .single()
                        .map_or(Value::Null, Value::TimestampOffset),
                    _ => Value::Null,
                }
            }
            G::ObjectId | G::Geometry => unreachable!("handled by decode_row"),
        })
    }

    pub(crate) fn decode_string(&self, bytes: &[u8]) -> String {
        if self.utf8 {
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
    }
}

/// Days since 1899-12-30 → timestamp, rounded to the millisecond.
fn datetime(days: f64) -> Option<NaiveDateTime> {
    if !days.is_finite() {
        return None;
    }
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?.and_hms_opt(0, 0, 0)?;
    let ms = (days * 86_400_000.0).round();
    if ms.abs() > 1e15 {
        return None;
    }
    epoch.checked_add_signed(Duration::milliseconds(ms as i64))
}

/// Legacy (second-precision) date: round away float noise to the nearest second.
fn datetime_seconds(days: f64) -> Option<NaiveDateTime> {
    datetime((days * 86_400.0).round() / 86_400.0)
}

/// Windows CLSID string order.
fn format_guid(b: &[u8]) -> String {
    format!(
        "{{{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        b[3],
        b[2],
        b[1],
        b[0],
        b[5],
        b[4],
        b[7],
        b[6],
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    )
}

fn read_field(r: &mut Reader, layer_z: bool, layer_m: bool) -> Result<GdbField> {
    let n = usize::from(r.u8()?);
    let name = r.utf16(n)?;
    let n = usize::from(r.u8()?);
    let alias = (n > 0)
        .then(|| r.utf16(n))
        .transpose()?
        .filter(|a| *a != name);
    let code = r.u8()?;
    let field_type = GdbFieldType::from_code(code)
        .ok_or_else(|| err(format!("field `{name}` has unknown type {code}")))?;
    let mut field = GdbField {
        name,
        alias,
        field_type,
        nullable: false,
        length: 0,
        default: None,
        geometry: None,
        raster_type: None,
        high_precision: false,
    };
    use GdbFieldType as G;
    match field_type {
        G::String => {
            field.length = r.u32()?;
            field.nullable = r.u8()? & 1 != 0;
            // The default-value length is always present, whatever the flags say.
            let n = r.count(1)?;
            field.default = (n > 0)
                .then(|| r.bytes(n).map(<[u8]>::to_vec))
                .transpose()?;
        }
        G::ObjectId => {
            field.length = u32::from(r.u8()?);
            r.u8()?;
        }
        G::Geometry => {
            r.u8()?;
            field.nullable = r.u8()? & 1 != 0;
            field.geometry = Some(read_geometry_def(r, layer_z, layer_m)?);
        }
        G::Raster => {
            r.u8()?;
            field.nullable = r.u8()? & 1 != 0;
            let n = usize::from(r.u8()?);
            r.utf16(n)?;
            let wkt_len = r.i16()?.max(0) as usize;
            r.bytes(wkt_len)?;
            let flags = r.u8()?;
            if flags != 0 {
                let (z, m) = (flags & 2 != 0, flags & 4 != 0);
                // origins/scales, then tolerances
                r.bytes(8 * (3 + 2 * usize::from(m) + 2 * usize::from(z)))?;
                r.bytes(8 * (1 + usize::from(m) + usize::from(z)))?;
            }
            field.raster_type = Some(r.u8()?);
        }
        G::Binary | G::Guid | G::GlobalId | G::Xml => {
            field.length = u32::from(r.u8()?);
            field.nullable = r.u8()? & 1 != 0;
        }
        _ => {
            field.length = u32::from(r.u8()?);
            field.nullable = r.u8()? & 1 != 0;
            let n = usize::from(r.u8()?);
            field.default = (n > 0)
                .then(|| r.bytes(n).map(<[u8]>::to_vec))
                .transpose()?;
        }
    }
    Ok(field)
}

fn read_geometry_def(r: &mut Reader, layer_z: bool, layer_m: bool) -> Result<GeometryDef> {
    // WKT is UTF-16LE; the length is in bytes.
    let wkt_len = r.i16()?.max(0) as usize;
    let wkt = r.utf16(wkt_len / 2)?.trim_end_matches('\0').to_owned();
    let srs_wkt = (!wkt.is_empty() && wkt != UNKNOWN_SRS).then_some(wkt);
    let flags = r.u8()?;
    let (has_z, has_m) = (flags & 2 != 0, flags & 4 != 0);
    let (x_origin, y_origin, xy_scale) = (r.f64()?, r.f64()?, r.f64()?);
    let (m_origin, m_scale) = if has_m {
        (r.f64()?, r.f64()?)
    } else {
        (0.0, 1.0)
    };
    let (z_origin, z_scale) = if has_z {
        (r.f64()?, r.f64()?)
    } else {
        (0.0, 1.0)
    };
    let xy_tolerance = r.f64()?;
    let m_tolerance = if has_m { r.f64()? } else { 0.0 };
    let z_tolerance = if has_z { r.f64()? } else { 0.0 };
    let (xmin, ymin, xmax, ymax) = (r.f64()?, r.f64()?, r.f64()?, r.f64()?);
    let pair = |r: &mut Reader| -> Result<Option<(f64, f64)>> {
        let (lo, hi) = (r.f64()?, r.f64()?);
        Ok((!lo.is_nan() && !hi.is_nan()).then_some((lo, hi)))
    };
    let z = if layer_z { pair(r)? } else { None };
    let m = if layer_m { pair(r)? } else { None };
    let extent = Envelope {
        xmin,
        ymin,
        xmax,
        ymax,
        z,
        m,
    };

    // Spatial grid: a 0 byte, a count, then the sizes. Some writers stop early.
    let mut grid_sizes = Vec::new();
    if r.remaining() >= 5 {
        r.u8()?;
        let n = r.u32()? as usize;
        for _ in 0..n.min(3) {
            grid_sizes.push(r.f64()?);
        }
    }
    Ok(GeometryDef {
        srs_wkt,
        has_z,
        has_m,
        x_origin,
        y_origin,
        xy_scale,
        m_origin,
        m_scale,
        z_origin,
        z_scale,
        xy_tolerance,
        m_tolerance,
        z_tolerance,
        extent,
        grid_sizes,
    })
}

impl FeatureClass for Table {
    fn geometry_kind(&self) -> Option<GeometryKind> {
        Table::geometry_kind(self)
    }

    fn has_z(&self) -> bool {
        self.has_z
    }

    fn has_m(&self) -> bool {
        self.has_m
    }

    fn fields(&self) -> &[Field] {
        &self.attribute_fields
    }

    fn oid_field_name(&self) -> &str {
        Table::oid_field_name(self).unwrap_or("OBJECTID")
    }

    fn shape_field_name(&self) -> &str {
        self.shape_field().map_or("SHAPE", |f| f.name.as_str())
    }

    fn spatial_reference_wkt(&self) -> Option<&str> {
        self.geometry_def()?.srs_wkt.as_deref()
    }

    fn stored_extent(&self) -> Option<Envelope> {
        self.geometry_def()
            .map(|d| d.extent)
            .filter(|e| !e.xmin.is_nan())
    }

    fn features(&self) -> Box<dyn Iterator<Item = Result<Feature>> + '_> {
        Box::new(Table::features(self))
    }
}

/// `.gdbtablx`: OBJECTID → row offset.
struct Tablx {
    file: File,
    /// Number of OBJECTID slots (max OBJECTID, deleted rows included).
    rows: u64,
    offset_size: u64,
    blocks: BlockMap,
}

/// Which 1024-row blocks have offset entries, and where.
enum BlockMap {
    /// Every block is present, in order.
    Dense,
    /// v3 bitmap: the slot index of each block, or `None` if absent.
    Bitmap(Vec<Option<u32>>),
    /// v4 sparse map. The bitmap is stored in 1024-byte chunks, each covering one
    /// "page" of 8192 blocks; `pages[c]` is the page held by chunk `c`, and
    /// `before[c]` the number of present blocks in earlier chunks.
    Paged {
        bitmap: Vec<u8>,
        pages: Vec<u64>,
        before: Vec<u64>,
    },
}

const BLOCK_ROWS: u64 = 1024;
const CHUNK_BYTES: usize = 1024;
const PAGE_BLOCKS: u64 = CHUNK_BYTES as u64 * 8;

impl Tablx {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| Error::io(path, e))?;
        let io = |e| Error::io(path, e);
        let file_len = file.metadata().map_err(io)?.len();
        let mut h = [0u8; 16];
        file.read_exact_at(&mut h, 0).map_err(io)?;
        let mut r = Reader::new(&h);
        let version = r.i32()?;
        let present = r.u32()? as u64;
        let rows_v3 = r.u32()? as u64;
        let offset_size = r.u32()? as u64;
        if !(4..=6).contains(&offset_size) {
            return Err(err(format!(
                "unsupported .gdbtablx offset size {offset_size}"
            )));
        }
        let trailer_at = 16 + offset_size * BLOCK_ROWS * present;
        let mut trailer = vec![0u8; file_len.saturating_sub(trailer_at) as usize];
        file.read_exact_at(&mut trailer, trailer_at).map_err(io)?;
        let mut t = Reader::new(&trailer);

        let (rows, blocks) = match version {
            3 => {
                let bitmap_words = t.u32().unwrap_or(0) as usize;
                let total_blocks = t.u32().unwrap_or(0) as usize;
                if bitmap_words == 0 {
                    (rows_v3, BlockMap::Dense)
                } else {
                    t.bytes(8)?;
                    let bitmap = t.bytes(total_blocks.div_ceil(8))?;
                    (rows_v3, BlockMap::Bitmap(block_map(bitmap, total_blocks)))
                }
            }
            4 => {
                let rows = t.i64().unwrap_or(0) as u64;
                let size = t.u32().unwrap_or(0) as usize;
                if size == 0 {
                    (rows, BlockMap::Dense)
                } else {
                    (rows, paged_map(t.bytes(size)?, present)?)
                }
            }
            v => return Err(err(format!("unsupported .gdbtablx version {v}"))),
        };
        Ok(Self {
            file,
            rows,
            offset_size,
            blocks,
        })
    }

    /// Slot of `block` in the offset array, or `None` if the block is absent.
    fn slot_of_block(&self, block: u64) -> Option<u64> {
        match &self.blocks {
            BlockMap::Dense => Some(block),
            BlockMap::Bitmap(map) => map.get(block as usize).copied().flatten().map(u64::from),
            BlockMap::Paged {
                bitmap,
                pages,
                before,
            } => {
                let chunk = pages.iter().position(|&p| p == block / PAGE_BLOCKS)?;
                let bit = (block % PAGE_BLOCKS) as usize;
                let bytes = &bitmap[chunk * CHUNK_BYTES..(chunk + 1) * CHUNK_BYTES];
                if bytes[bit / 8] & (1 << (bit % 8)) == 0 {
                    return None;
                }
                let full: u64 = bytes[..bit / 8]
                    .iter()
                    .map(|b| u64::from(b.count_ones()))
                    .sum();
                let partial = u64::from((bytes[bit / 8] & ((1u8 << (bit % 8)) - 1)).count_ones());
                Some(before[chunk] + full + partial)
            }
        }
    }

    /// OBJECTIDs whose block is present (rows inside may still be deleted).
    fn candidate_oids(&self) -> Box<dyn Iterator<Item = u64> + '_> {
        let rows = self.rows;
        let block_oids = move |b: u64| (b * BLOCK_ROWS + 1)..=((b + 1) * BLOCK_ROWS).min(rows);
        match &self.blocks {
            BlockMap::Dense => Box::new(1..=rows),
            BlockMap::Bitmap(map) => Box::new(
                (0..map.len() as u64)
                    .filter(|&b| map[b as usize].is_some())
                    .flat_map(block_oids),
            ),
            BlockMap::Paged { bitmap, pages, .. } => {
                let mut blocks: Vec<u64> = Vec::new();
                for (c, &page) in pages.iter().enumerate() {
                    let bytes = &bitmap[c * CHUNK_BYTES..(c + 1) * CHUNK_BYTES];
                    for (i, &byte) in bytes.iter().enumerate() {
                        for bit in 0..8 {
                            if byte & (1 << bit) != 0 {
                                blocks.push(page * PAGE_BLOCKS + (i * 8 + bit) as u64);
                            }
                        }
                    }
                }
                blocks.sort_unstable();
                Box::new(blocks.into_iter().flat_map(block_oids))
            }
        }
    }

    /// Row offset in the `.gdbtable`, or `None` if deleted / out of range.
    fn offset(&self, path: &Path, oid: u64) -> Result<Option<u64>> {
        if oid == 0 || oid > self.rows {
            return Ok(None);
        }
        let row = oid - 1;
        let Some(block_slot) = self.slot_of_block(row / BLOCK_ROWS) else {
            return Ok(None);
        };
        let slot = block_slot * BLOCK_ROWS + row % BLOCK_ROWS;
        let mut buf = [0u8; 8];
        let n = self.offset_size as usize;
        self.file
            .read_exact_at(&mut buf[..n], 16 + slot * self.offset_size)
            .map_err(|e| Error::io(path.with_extension("gdbtablx"), e))?;
        let off = u64::from_le_bytes(buf);
        Ok((off != 0).then_some(off))
    }
}

fn block_map(bitmap: &[u8], total: usize) -> Vec<Option<u32>> {
    let mut next = 0;
    (0..total)
        .map(|i| {
            (bitmap[i / 8] & (1 << (i % 8)) != 0).then(|| {
                next += 1;
                next - 1
            })
        })
        .collect()
}

/// Parses the v4 sparse section: 22 header bytes, the chunked bitmap, then
/// `u32 nChunks, u64 pageOfChunk0`, `nChunks × (u64 stamp, u64 blocksInChunk,
/// u64 chunkIndex, u64 pageOfNextChunk)` and 8 bytes of 0xFF. Worked out from the
/// dumps in the FGDB spec and checked against GDAL on its test files.
fn paged_map(var: &[u8], present: u64) -> Result<BlockMap> {
    const HEAD: usize = 22;
    let body = var
        .len()
        .checked_sub(HEAD + 20)
        .ok_or_else(|| err("sparse .gdbtablx section too short"))?;
    let bitmap_len = body / CHUNK_BYTES * CHUNK_BYTES;
    let nrec = (body % CHUNK_BYTES) / 32;
    let mut t = Reader::new(&var[HEAD + bitmap_len..]);
    if t.u32()? as usize != nrec {
        return Err(Error::Unsupported(
            "unrecognized sparse .gdbtablx layout".into(),
        ));
    }
    let mut pages = vec![t.i64()? as u64];
    let mut before = vec![0u64];
    let mut total = 0;
    for i in 0..nrec {
        t.i64()?; // stamp
        let count = t.i64()? as u64;
        if t.i64()? as usize != i {
            return Err(Error::Unsupported(
                "unrecognized sparse .gdbtablx record".into(),
            ));
        }
        let next = t.i64()? as u64;
        total += count;
        if i + 1 < nrec {
            pages.push(next);
            before.push(total);
        }
    }
    if total != present || pages.len() * CHUNK_BYTES > bitmap_len {
        return Err(err("sparse .gdbtablx block counts are inconsistent"));
    }
    Ok(BlockMap::Paged {
        bitmap: var[HEAD..HEAD + bitmap_len].to_vec(),
        pages,
        before,
    })
}
