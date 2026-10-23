//! `.gdbtable` / `.gdbtablx`: one table or feature class.

use std::cmp::Ordering;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use chrono::{Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
use unigis_core::{
    Envelope, Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value,
};

use crate::bytes::{Reader, err};
use crate::geometry;
use crate::index::{BTree, IndexDef, read_gdbindexes, spatial_candidates};
use crate::schema::{Subtypes, TableDefinition};

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
    /// File offsets of the stored extent (four f64) and of the first grid size, for a
    /// definition read from a table (`None` when built in memory).
    pub extent_at: Option<u64>,
    pub grid_at: Option<u64>,
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
    /// Attribute domain name, from the item definition.
    pub domain: Option<String>,
}

impl GdbField {
    pub fn to_field(&self) -> Field {
        let mut f = Field::new(&self.name, self.field_type.esri());
        f.alias = self.alias.clone();
        f.domain = self.domain.clone();
        f.nullable = self.nullable;
        f.length = self.length;
        f.default = self
            .default
            .as_deref()
            .and_then(|d| decode_default(self.field_type, d));
        f
    }
}

/// A field's default value from the bytes stored in the table's field section.
pub(crate) fn decode_default(t: GdbFieldType, d: &[u8]) -> Option<Value> {
    use GdbFieldType as G;
    Some(match t {
        G::Int16 => Value::SmallInteger(i16::from_le_bytes(d.try_into().ok()?)),
        G::Int32 => Value::Integer(i32::from_le_bytes(d.try_into().ok()?)),
        G::Int64 => Value::BigInteger(i64::from_le_bytes(d.try_into().ok()?)),
        G::Float32 => Value::Single(f32::from_le_bytes(d.try_into().ok()?)),
        G::Float64 => Value::Double(f64::from_le_bytes(d.try_into().ok()?)),
        G::String => Value::String(String::from_utf8(d.to_vec()).ok()?),
        _ => return None,
    })
}

/// The bytes a table stores for a default value of a field of type `t`; `None` when the type
/// holds no default or the value does not fit it.
pub(crate) fn encode_default(t: GdbFieldType, v: &Value) -> Option<Vec<u8>> {
    use GdbFieldType as G;
    let int = |v: &Value| match v {
        Value::SmallInteger(i) => Some(i64::from(*i)),
        Value::Integer(i) => Some(i64::from(*i)),
        Value::BigInteger(i) => Some(*i),
        _ => None,
    };
    let float = |v: &Value| match v {
        Value::Single(f) => Some(f64::from(*f)),
        Value::Double(f) => Some(*f),
        other => int(other).map(|i| i as f64),
    };
    Some(match t {
        G::Int16 => i16::try_from(int(v)?).ok()?.to_le_bytes().to_vec(),
        G::Int32 => i32::try_from(int(v)?).ok()?.to_le_bytes().to_vec(),
        G::Int64 => int(v)?.to_le_bytes().to_vec(),
        G::Float32 => (float(v)? as f32).to_le_bytes().to_vec(),
        G::Float64 => float(v)?.to_le_bytes().to_vec(),
        G::String => match v {
            Value::String(s) if !s.is_empty() => s.as_bytes().to_vec(),
            _ => return None,
        },
        _ => return None,
    })
}

/// An open table. Feature IDs are OBJECTIDs (1-based).
pub struct Table {
    file: File,
    path: PathBuf,
    pub(crate) utf8: bool,
    /// Uses ArcGIS Pro 3.2 extensions (64-bit OBJECTIDs or new field types).
    pro_3_2: bool,
    version: i32,
    field_section_len: u32,
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
    subtypes: Option<Subtypes>,
    indexes: Vec<IndexDef>,
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
        // Section-relative offsets become file offsets.
        for g in fields.iter_mut().filter_map(|f| f.geometry.as_mut()) {
            g.extent_at = g.extent_at.map(|o| o + field_offset + 4);
            g.grid_at = g.grid_at.map(|o| o + field_offset + 4);
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
        let indexes = read_gdbindexes(&path.with_extension("gdbindexes")).unwrap_or_default();

        Ok(Self {
            file,
            path,
            utf8,
            version,
            field_section_len: size as u32,
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
            subtypes: None,
            indexes,
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

    /// Applies schema from the geodatabase's item definition: field domains,
    /// high-precision dates and subtypes.
    pub fn apply_definition(&mut self, def: &TableDefinition) {
        for f in &mut self.fields {
            if let Some(info) = def
                .fields
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(&f.name))
            {
                f.high_precision = info.high_precision;
                f.domain = info.domain.clone();
            }
        }
        self.attribute_fields = self
            .fields
            .iter()
            .filter(|f| {
                !matches!(
                    f.field_type,
                    GdbFieldType::ObjectId | GdbFieldType::Geometry
                )
            })
            .map(GdbField::to_field)
            .collect();
        self.subtypes = def.subtypes.clone();
        // ArcGIS lists only some fields in the definition, so a subtype's default may have been
        // read as text where its field is numeric: the table knows the field's type.
        if let Some(st) = &mut self.subtypes {
            for sf in st.subtypes.iter_mut().flat_map(|s| s.fields.iter_mut()) {
                let Some(f) = self
                    .fields
                    .iter()
                    .find(|f| f.name.eq_ignore_ascii_case(&sf.field))
                else {
                    continue;
                };
                if let Some(Value::String(raw)) = &sf.default {
                    sf.default = Some(crate::schema::typed_value(raw, f.field_type.esri()));
                }
            }
        }
    }

    /// Indexes listed in `.gdbindexes` (attribute and spatial).
    pub fn indexes(&self) -> &[IndexDef] {
        &self.indexes
    }

    /// OBJECTIDs the spatial index (`.spx`) lists for cells touching `env`, or
    /// `None` when the table has no usable spatial index. May include features that
    /// do not intersect `env`; never misses one.
    pub fn spatial_index_candidates(&self, env: &Envelope) -> Result<Option<Vec<u64>>> {
        let spx = self.path.with_extension("spx");
        // A grid size of 0 means every key is 0: the index carries no location.
        let Some(def) = self
            .geometry_def()
            .filter(|d| d.grid_sizes.iter().any(|&g| g > 0.0))
        else {
            return Ok(None);
        };
        if !spx.is_file() {
            return Ok(None);
        }
        spatial_candidates(&BTree::open(&spx)?, &def.grid_sizes, env).map(Some)
    }

    /// Features whose geometry envelope intersects `env` (ArcGIS "select by
    /// location" prefilter). Uses the spatial index when present and readable.
    pub fn features_in_extent(&self, env: &Envelope) -> Result<Vec<Feature>> {
        let intersects = |f: &Feature| {
            f.geometry.as_ref().is_some_and(|g| {
                let e = g.envelope();
                !e.is_empty()
                    && e.xmin <= env.xmax
                    && e.xmax >= env.xmin
                    && e.ymin <= env.ymax
                    && e.ymax >= env.ymin
            })
        };
        let candidates = self.spatial_index_candidates(env).ok().flatten();
        let mut out = Vec::new();
        match candidates {
            Some(oids) => {
                for oid in oids {
                    if let Some(f) = self.get(oid)?
                        && intersects(&f)
                    {
                        out.push(f);
                    }
                }
            }
            None => {
                for f in self.features() {
                    let f = f?;
                    if intersects(&f) {
                        out.push(f);
                    }
                }
            }
        }
        Ok(out)
    }

    /// OBJECTIDs whose attribute-index key for `field` lies in `[lo, hi]`, or `None`
    /// when no plain-field attribute index exists. Keys may be truncated (strings
    /// over 80 characters), so callers re-check row values.
    pub fn attribute_index_candidates(
        &self,
        field: &str,
        lo: &Value,
        hi: &Value,
    ) -> Result<Option<Vec<u64>>> {
        let Some(def) = self
            .indexes
            .iter()
            .find(|d| d.expression.eq_ignore_ascii_case(field))
        else {
            return Ok(None);
        };
        let Some(f) = self
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(field))
        else {
            return Ok(None);
        };
        let atx = self.path.with_extension(format!("{}.atx", def.name));
        if !atx.is_file() {
            return Ok(None);
        }
        let tree = BTree::open(&atx)?;
        let (Some(lo), Some(hi)) = (
            index_key(f.field_type, lo, tree.key_size),
            index_key(f.field_type, hi, tree.key_size),
        ) else {
            return Ok(None);
        };
        let ty = f.field_type;
        // Legacy dates are read rounded to the second; the index holds raw values, so
        // widen the range by half a second to find everything that rounds into it.
        let (lo, hi) = if ty == GdbFieldType::DateTime && !f.high_precision {
            let shift = |k: &[u8], d: f64| {
                (f64::from_le_bytes(k.try_into().unwrap()) + d / 86_400.0)
                    .to_le_bytes()
                    .to_vec()
            };
            (shift(&lo, -0.5), shift(&hi, 0.5))
        } else {
            (lo, hi)
        };
        let mut oids = Vec::new();
        tree.scan(
            &|k| cmp_keys(ty, k, &lo),
            &|k| cmp_keys(ty, k, &hi),
            &mut |oid, _| oids.push(oid),
        )?;
        oids.sort_unstable();
        oids.dedup();
        Ok(Some(oids))
    }

    /// Rows whose `field` value lies in `[lo, hi]` (pass the same value twice for
    /// equality). Uses an attribute index when present, else scans.
    pub fn search_attribute(&self, field: &str, lo: &Value, hi: &Value) -> Result<Vec<Feature>> {
        let i = self
            .attribute_fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(field))
            .ok_or_else(|| err(format!("no field {field}")))?;
        let in_range = |f: &Feature| {
            let v = &f.attributes[i];
            compare_values(v, lo).is_some_and(Ordering::is_ge)
                && compare_values(v, hi).is_some_and(Ordering::is_le)
        };
        let mut out = Vec::new();
        match self
            .attribute_index_candidates(field, lo, hi)
            .ok()
            .flatten()
        {
            Some(oids) => {
                for oid in oids {
                    if let Some(f) = self.get(oid)?
                        && in_range(&f)
                    {
                        out.push(f);
                    }
                }
            }
            None => {
                for f in self.features() {
                    let f = f?;
                    if in_range(&f) {
                        out.push(f);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Subtypes, if the table defines any.
    pub fn subtypes(&self) -> Option<&Subtypes> {
        self.subtypes.as_ref()
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

    /// Decodes the row stored at `offset` of the `.gdbtable` as OBJECTID `oid`.
    pub(crate) fn read_at(&self, oid: u64, offset: u64) -> Result<Feature> {
        let io = |e| Error::io(&self.path, e);
        let mut len = [0u8; 4];
        self.file.read_exact_at(&mut len, offset).map_err(io)?;
        let mut blob = vec![0u8; u32::from_le_bytes(len) as usize];
        self.file.read_exact_at(&mut blob, offset + 4).map_err(io)?;
        self.decode_row(oid, &blob)
    }

    /// The file offset of row `oid` in the `.gdbtable`, or `None` if it is deleted.
    pub(crate) fn raw_offset(&self, oid: u64) -> Result<Option<u64>> {
        self.tablx.offset(&self.path, oid)
    }

    /// Whether this is a 64-bit OBJECTID table (ArcGIS Pro 3.2 format, version 4).
    pub(crate) fn is_version_4(&self) -> bool {
        self.version == 4
    }

    /// The field section's size in bytes (for the header's "maximum row size").
    pub(crate) fn field_section_size(&self) -> u32 {
        self.field_section_len
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
                .as_chunks::<2>()
                .0
                .iter()
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

/// Encodes a search value as an index key of `size` bytes.
fn index_key(ty: GdbFieldType, v: &Value, size: usize) -> Option<Vec<u8>> {
    use GdbFieldType as G;
    let num = || -> Option<f64> {
        Some(match v {
            Value::SmallInteger(n) => f64::from(*n),
            Value::Integer(n) => f64::from(*n),
            Value::BigInteger(n) => *n as f64,
            Value::Single(n) => f64::from(*n),
            Value::Double(n) => *n,
            _ => return None,
        })
    };
    let key = match ty {
        G::Int16 => (num()?.clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16)
            .to_le_bytes()
            .to_vec(),
        G::Int32 => (num()?.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32)
            .to_le_bytes()
            .to_vec(),
        G::Int64 => match v {
            Value::BigInteger(n) => n.to_le_bytes().to_vec(),
            _ => (num()? as i64).to_le_bytes().to_vec(),
        },
        G::Float32 => (num()? as f32).to_le_bytes().to_vec(),
        G::Float64 => num()?.to_le_bytes().to_vec(),
        G::DateTime | G::DateOnly | G::DateTimeOffset => {
            let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?.and_hms_opt(0, 0, 0)?;
            let t = match v {
                Value::Date(t) => *t,
                Value::DateOnly(d) => d.and_hms_opt(0, 0, 0)?,
                // Indexed in UTC.
                Value::TimestampOffset(t) => t.naive_utc(),
                _ => return None,
            };
            ((t - epoch).num_milliseconds() as f64 / 86_400_000.0)
                .to_le_bytes()
                .to_vec()
        }
        G::TimeOnly => match v {
            Value::TimeOnly(t) => {
                let ms = (*t - NaiveTime::MIN).num_milliseconds();
                (ms as f64 / 86_400_000.0).to_le_bytes().to_vec()
            }
            _ => return None,
        },
        // GUID keys are the 38-character "{...}" form in ASCII.
        G::Guid | G::GlobalId => match v {
            Value::String(s) | Value::Guid(s) => s.to_ascii_uppercase().into_bytes(),
            _ => return None,
        },
        G::String => {
            let s = match v {
                Value::String(s) | Value::Guid(s) => s,
                _ => return None,
            };
            let mut units: Vec<u16> = s.encode_utf16().take(size / 2).collect();
            units.resize(size / 2, u16::from(b' '));
            units.iter().flat_map(|u| u.to_le_bytes()).collect()
        }
        _ => return None,
    };
    (key.len() == size).then_some(key)
}

/// Orders two index keys of the given field type.
fn cmp_keys(ty: GdbFieldType, a: &[u8], b: &[u8]) -> Ordering {
    use GdbFieldType as G;
    let f = |k: &[u8]| -> f64 {
        match ty {
            G::Int16 => f64::from(i16::from_le_bytes(k.try_into().unwrap())),
            G::Int32 => f64::from(i32::from_le_bytes(k.try_into().unwrap())),
            G::Int64 => i64::from_le_bytes(k.try_into().unwrap()) as f64,
            G::Float32 => f64::from(f32::from_le_bytes(k.try_into().unwrap())),
            _ => f64::from_le_bytes(k.try_into().unwrap()),
        }
    };
    match ty {
        G::Guid | G::GlobalId => a.cmp(b),
        G::String => {
            let units = |k: &[u8]| {
                k.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>()
            };
            units(a).cmp(&units(b))
        }
        G::Int64 => i64::from_le_bytes(a.try_into().unwrap())
            .cmp(&i64::from_le_bytes(b.try_into().unwrap())),
        _ => f(a).total_cmp(&f(b)),
    }
}

/// Orders two attribute values of compatible types (`None` if incomparable or null).
pub fn compare_values(a: &Value, b: &Value) -> Option<Ordering> {
    let num = |v: &Value| -> Option<f64> {
        Some(match v {
            Value::SmallInteger(n) => f64::from(*n),
            Value::Integer(n) => f64::from(*n),
            Value::BigInteger(n) => *n as f64,
            Value::Single(n) => f64::from(*n),
            Value::Double(n) => *n,
            _ => return None,
        })
    };
    match (a, b) {
        (Value::String(x) | Value::Guid(x), Value::String(y) | Value::Guid(y)) => Some(x.cmp(y)),
        (Value::Date(x), Value::Date(y)) => Some(x.cmp(y)),
        (Value::DateOnly(x), Value::DateOnly(y)) => Some(x.cmp(y)),
        (Value::TimeOnly(x), Value::TimeOnly(y)) => Some(x.cmp(y)),
        (Value::TimestampOffset(x), Value::TimestampOffset(y)) => Some(x.cmp(y)),
        _ => num(a)?.partial_cmp(&num(b)?),
    }
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
        domain: None,
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
    let extent_at = r.p as u64;
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
    let mut grid_at = None;
    if r.remaining() >= 5 {
        r.u8()?;
        let n = r.u32()? as usize;
        grid_at = (n > 0).then_some(r.p as u64);
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
        extent_at: Some(extent_at),
        grid_at,
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
