//! OGC GeoPackage (`.gpkg`): feature and attribute tables read through the
//! crate's own read-only SQLite reader, presented as ArcGIS Pro presents them:
//! tables named `main.<table>`, the integer primary key as the ObjectID, SQLite
//! column types mapped to ArcGIS field types, rings oriented the Esri way
//! (outer rings clockwise) with their start vertices kept, and empty geometries
//! read as null (empty points stay empty points).

mod base;
pub mod mobile;
mod mobile_seed;
pub mod sqlite;
pub mod sqlite_writer;
pub mod write;

use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime};
use unigis_core::{
    Envelope, Error, Feature, FeatureClass, Field, FieldType, Geometry, GeometryKind, Result, Value,
};

use sqlite::{Database, SqlValue, Table};

fn bad(message: impl Into<String>) -> Error {
    Error::format("GeoPackage", message)
}

/// A table listed in `gpkg_contents`.
#[derive(Debug, Clone)]
pub struct Contents {
    pub table_name: String,
    /// `features` or `attributes` (other kinds, such as tiles, are skipped).
    pub data_type: String,
    pub srs_id: Option<i64>,
}

/// An open GeoPackage.
pub struct GeoPackage {
    db: Database,
    contents: Vec<Contents>,
}

impl GeoPackage {
    pub fn open(path: &Path) -> Result<GeoPackage> {
        let db = Database::open(path)?;
        let table = db
            .table("gpkg_contents")
            .ok_or_else(|| bad(format!("{}: no gpkg_contents table", path.display())))?
            .clone();
        let col = |name: &str| table.columns.iter().position(|c| c.name == name);
        let (Some(name), Some(kind)) = (col("table_name"), col("data_type")) else {
            return Err(bad("gpkg_contents without table_name/data_type"));
        };
        let srs = col("srs_id");
        let contents = db
            .select(&table)?
            .into_iter()
            .filter_map(|(_, row)| {
                Some(Contents {
                    table_name: row[name].as_str()?.to_string(),
                    data_type: row[kind].as_str()?.to_string(),
                    srs_id: srs.and_then(|i| row[i].as_i64()),
                })
            })
            .filter(|c| c.data_type == "features" || c.data_type == "attributes")
            .collect();
        Ok(GeoPackage { db, contents })
    }

    /// Feature and attribute tables, in `gpkg_contents` order.
    pub fn contents(&self) -> &[Contents] {
        &self.contents
    }

    /// Opens a table by its name, with or without ArcGIS's `main.` prefix.
    pub fn open_table(&self, name: &str) -> Result<GpkgTable> {
        let bare = name
            .strip_prefix("main.")
            .or_else(|| name.strip_prefix("MAIN."))
            .unwrap_or(name);
        let contents = self
            .contents
            .iter()
            .find(|c| c.table_name.eq_ignore_ascii_case(bare))
            .ok_or_else(|| Error::Unsupported(format!("no table {name} in the GeoPackage")))?;
        let table = self
            .db
            .table(&contents.table_name)
            .ok_or_else(|| bad(format!("{} listed but missing", contents.table_name)))?;
        GpkgTable::load(&self.db, table, contents)
    }
}

/// Where each kind of column goes in a feature.
#[derive(Debug, Clone, Copy)]
enum Slot {
    Oid,
    Geometry,
    Field(usize),
}

/// A feature or attribute table, fully read.
pub struct GpkgTable {
    name: String,
    kind: Option<GeometryKind>,
    has_z: bool,
    has_m: bool,
    oid_field: String,
    shape_field: String,
    fields: Vec<Field>,
    wkt: Option<String>,
    extent: Option<Envelope>,
    features: Vec<Feature>,
}

impl GpkgTable {
    fn load(db: &Database, table: &Table, contents: &Contents) -> Result<GpkgTable> {
        // The geometry column and its declared type.
        let mut geometry: Option<(String, GeometryKind, bool, bool)> = None;
        if contents.data_type == "features"
            && let Some(gc) = db.table("gpkg_geometry_columns")
        {
            let col = |name: &str| gc.columns.iter().position(|c| c.name == name);
            let (t, c, g, z, m) = (
                col("table_name"),
                col("column_name"),
                col("geometry_type_name"),
                col("z"),
                col("m"),
            );
            for (_, row) in db.select(gc)? {
                let text = |i: Option<usize>| i.and_then(|i| row[i].as_str()).unwrap_or("");
                let flag = |i: Option<usize>| i.and_then(|i| row[i].as_i64()).unwrap_or(0) == 1;
                if text(t).eq_ignore_ascii_case(&table.name) {
                    let kind = geometry_kind(text(g))
                        .ok_or_else(|| Error::Unsupported(format!("geometry type {}", text(g))))?;
                    geometry = Some((text(c).to_string(), kind, flag(z), flag(m)));
                }
            }
        }
        let mut slots = Vec::new();
        let mut fields = Vec::new();
        let mut oid_field = None;
        for column in &table.columns {
            if column.rowid_alias {
                oid_field = Some(column.name.clone());
                slots.push(Slot::Oid);
            } else if geometry
                .as_ref()
                .is_some_and(|g| g.0.eq_ignore_ascii_case(&column.name))
            {
                slots.push(Slot::Geometry);
            } else {
                slots.push(Slot::Field(fields.len()));
                let mut field = field_for(&column.declared_type, &column.name)?;
                field.nullable = !column.not_null;
                fields.push(field);
            }
        }
        let oid_field = oid_field.ok_or_else(|| {
            Error::Unsupported(format!("{}: no INTEGER PRIMARY KEY column", table.name))
        })?;
        let wkt = match contents.srs_id {
            Some(id) if geometry.is_some() => srs_definition(db, id)?,
            _ => None,
        };
        let mut features = Vec::new();
        let mut extent: Option<Envelope> = None;
        for (rowid, values) in db.select(table)? {
            let mut feature = Feature {
                oid: rowid,
                geometry: None,
                attributes: vec![Value::Null; fields.len()],
            };
            for (value, slot) in values.into_iter().zip(&slots) {
                match *slot {
                    Slot::Oid => {}
                    Slot::Geometry => {
                        if let SqlValue::Blob(b) = value {
                            let (kind, has_z, has_m) =
                                geometry.as_ref().map(|g| (g.1, g.2, g.3)).unwrap();
                            feature.geometry = read_geometry(&b, kind, has_z, has_m)?;
                        }
                    }
                    Slot::Field(i) => feature.attributes[i] = convert(value, &fields[i])?,
                }
            }
            if let Some(g) = &feature.geometry
                && let Some(e) = envelope_of(g)
            {
                extent = Some(match extent {
                    Some(x) => Envelope {
                        xmin: x.xmin.min(e.xmin),
                        ymin: x.ymin.min(e.ymin),
                        xmax: x.xmax.max(e.xmax),
                        ymax: x.ymax.max(e.ymax),
                        z: None,
                        m: None,
                    },
                    None => e,
                });
            }
            features.push(feature);
        }
        Ok(GpkgTable {
            name: format!("main.{}", table.name),
            kind: geometry.as_ref().map(|g| g.1),
            has_z: geometry.as_ref().is_some_and(|g| g.2),
            has_m: geometry.as_ref().is_some_and(|g| g.3),
            shape_field: geometry.map(|g| g.0).unwrap_or_default(),
            oid_field,
            fields,
            wkt,
            extent,
            features,
        })
    }

    /// The name ArcGIS lists the table under (`main.<table>`).
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl FeatureClass for GpkgTable {
    fn geometry_kind(&self) -> Option<GeometryKind> {
        self.kind
    }
    fn has_z(&self) -> bool {
        self.has_z
    }
    fn has_m(&self) -> bool {
        self.has_m
    }
    fn fields(&self) -> &[Field] {
        &self.fields
    }
    fn oid_field_name(&self) -> &str {
        &self.oid_field
    }
    fn shape_field_name(&self) -> &str {
        &self.shape_field
    }
    fn spatial_reference_wkt(&self) -> Option<&str> {
        self.wkt.as_deref()
    }
    fn stored_extent(&self) -> Option<Envelope> {
        self.extent
    }
    fn features(&self) -> Box<dyn Iterator<Item = Result<Feature>> + '_> {
        Box::new(self.features.iter().cloned().map(Ok))
    }
}

/// The ArcGIS shape type of a GeoPackage geometry type name.
fn geometry_kind(name: &str) -> Option<GeometryKind> {
    Some(match name.to_ascii_uppercase().as_str() {
        "POINT" => GeometryKind::Point,
        "MULTIPOINT" => GeometryKind::Multipoint,
        "LINESTRING" | "MULTILINESTRING" | "CURVE" | "MULTICURVE" => GeometryKind::Polyline,
        "POLYGON" | "MULTIPOLYGON" | "SURFACE" | "MULTISURFACE" => GeometryKind::Polygon,
        _ => return None,
    })
}

/// The field ArcGIS shows for a column's declared SQLite type (observed in
/// ArcGIS Pro 3.7): SMALLINT, TINYINT and BOOLEAN are Short, MEDIUMINT Long,
/// INTEGER Big Integer, FLOAT Single, REAL and DOUBLE Double, TEXT(n) Text of
/// length n (unbounded TEXT 2147483647), DATE Date Only, DATETIME Date, BLOB
/// Blob.
fn field_for(declared: &str, name: &str) -> Result<Field> {
    let base = declared.split('(').next().unwrap_or("").trim();
    let length = declared
        .split_once('(')
        .and_then(|(_, r)| r.trim_end_matches(')').trim().parse::<u32>().ok());
    let (field_type, len) = match base {
        "BOOLEAN" | "TINYINT" | "SMALLINT" => (FieldType::SmallInteger, 2),
        "MEDIUMINT" => (FieldType::Integer, 4),
        "INT" | "INTEGER" => (FieldType::BigInteger, 8),
        "FLOAT" => (FieldType::Single, 4),
        "DOUBLE" | "REAL" => (FieldType::Double, 8),
        "TEXT" => (FieldType::String, length.unwrap_or(2147483647)),
        "DATE" => (FieldType::DateOnly, 8),
        "DATETIME" => (FieldType::Date, 8),
        "BLOB" => (FieldType::Blob, 0),
        other => {
            return Err(Error::Unsupported(format!(
                "GeoPackage column {name} of type {other}"
            )));
        }
    };
    let mut f = Field::new(name, field_type);
    f.length = len;
    Ok(f)
}

/// A stored value as the field's type.
fn convert(v: SqlValue, field: &Field) -> Result<Value> {
    if v == SqlValue::Null {
        return Ok(Value::Null);
    }
    let mismatch = || {
        bad(format!(
            "{}: value {v:?} for a {:?} field",
            field.name, field.field_type
        ))
    };
    Ok(match field.field_type {
        FieldType::SmallInteger => Value::SmallInteger(v.as_i64().ok_or_else(mismatch)? as i16),
        FieldType::Integer => Value::Integer(v.as_i64().ok_or_else(mismatch)? as i32),
        FieldType::BigInteger => Value::BigInteger(v.as_i64().ok_or_else(mismatch)?),
        FieldType::Single => Value::Single(v.as_f64().ok_or_else(mismatch)? as f32),
        FieldType::Double => Value::Double(v.as_f64().ok_or_else(mismatch)?),
        FieldType::String => match &v {
            SqlValue::Text(t) => Value::String(t.clone()),
            SqlValue::Integer(i) => Value::String(i.to_string()),
            SqlValue::Real(r) => Value::String(r.to_string()),
            SqlValue::Blob(b) => Value::String(String::from_utf8_lossy(b).into_owned()),
            SqlValue::Null => Value::Null,
        },
        FieldType::DateOnly => Value::DateOnly(
            NaiveDate::parse_from_str(v.as_str().ok_or_else(mismatch)?.trim(), "%Y-%m-%d")
                .map_err(|_| mismatch())?,
        ),
        FieldType::Date => {
            Value::Date(parse_datetime(v.as_str().ok_or_else(mismatch)?).ok_or_else(mismatch)?)
        }
        FieldType::Blob => match v {
            SqlValue::Blob(b) => Value::Blob(b),
            SqlValue::Text(t) => Value::Blob(t.into_bytes()),
            _ => return Err(mismatch()),
        },
        _ => return Err(mismatch()),
    })
}

/// A GeoPackage DATETIME (ISO 8601, `Z` or no zone, optional fraction).
fn parse_datetime(text: &str) -> Option<NaiveDateTime> {
    let t = text.trim().trim_end_matches('Z');
    [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
    ]
    .iter()
    .find_map(|f| NaiveDateTime::parse_from_str(t, f).ok())
}

/// The `definition` of a spatial reference system, if it has one.
fn srs_definition(db: &Database, id: i64) -> Result<Option<String>> {
    let Some(t) = db.table("gpkg_spatial_ref_sys") else {
        return Ok(None);
    };
    let col = |name: &str| t.columns.iter().position(|c| c.name == name);
    let (Some(idc), Some(def)) = (col("srs_id"), col("definition")) else {
        return Ok(None);
    };
    for (_, row) in db.select(t)? {
        if row[idc].as_i64() == Some(id) {
            let d = row[def].as_str().unwrap_or("").trim();
            return Ok((!d.is_empty() && d != "undefined").then(|| d.to_string()));
        }
    }
    Ok(None)
}

fn envelope_of(g: &Geometry) -> Option<Envelope> {
    let first = g.xy.first()?;
    let mut e = Envelope {
        xmin: first[0],
        ymin: first[1],
        xmax: first[0],
        ymax: first[1],
        z: None,
        m: None,
    };
    for p in &g.xy {
        e.xmin = e.xmin.min(p[0]);
        e.ymin = e.ymin.min(p[1]);
        e.xmax = e.xmax.max(p[0]);
        e.ymax = e.ymax.max(p[1]);
    }
    Some(e)
}

/// Decodes a GeoPackage geometry blob (header, optional envelope, then ISO
/// WKB) into an Esri-style shape; empty geometries give `None`, except empty
/// points.
pub fn read_geometry(
    blob: &[u8],
    kind: GeometryKind,
    has_z: bool,
    has_m: bool,
) -> Result<Option<Geometry>> {
    if blob.len() < 8 || &blob[..2] != b"GP" {
        return Err(bad("geometry blob without the GP header"));
    }
    let flags = blob[3];
    if flags & 0x20 != 0 {
        return Err(Error::Unsupported("extended GeoPackage geometry".into()));
    }
    // An empty point stays an empty point (ArcGIS writes it to shapefiles as
    // such); other empty shapes read as null.
    let empty_point = || {
        let mut g = Geometry::point(0.0, 0.0);
        g.xy.clear();
        g.z = has_z.then(Vec::new);
        g.m = has_m.then(Vec::new);
        g
    };
    if flags & 0x10 != 0 {
        return Ok((kind == GeometryKind::Point).then(empty_point));
    }
    let envelope_len = match (flags >> 1) & 0x7 {
        0 => 0,
        1 => 32,
        2 | 3 => 48,
        4 => 64,
        n => return Err(bad(format!("envelope indicator {n}"))),
    };
    let mut wkb = Wkb {
        b: blob,
        at: 8 + envelope_len,
        little: true,
    };
    let mut out = Shape::default();
    wkb.geometry(&mut out, 0)?;
    if out.xy.is_empty() {
        return Ok((kind == GeometryKind::Point).then(empty_point));
    }
    let mut g = Geometry {
        kind,
        parts: Vec::new(),
        part_types: None,
        xy: out.xy,
        z: has_z.then_some(out.z),
        m: has_m.then_some(out.m),
        curves: Vec::new(),
    };
    if matches!(kind, GeometryKind::Polyline | GeometryKind::Polygon) {
        g.parts = out.parts;
    }
    if kind == GeometryKind::Polygon {
        orient_rings(&mut g, &out.outer);
    }
    Ok(Some(g))
}

/// Coordinates gathered from WKB: parts start offsets and, per part, whether it
/// is a polygon's outer ring.
#[derive(Default)]
struct Shape {
    xy: Vec<[f64; 2]>,
    z: Vec<f64>,
    m: Vec<f64>,
    parts: Vec<u32>,
    outer: Vec<bool>,
}

struct Wkb<'a> {
    b: &'a [u8],
    at: usize,
    little: bool,
}

impl Wkb<'_> {
    fn u32(&mut self) -> Result<u32> {
        let s = self
            .b
            .get(self.at..self.at + 4)
            .ok_or_else(|| bad("short WKB"))?;
        self.at += 4;
        let a = [s[0], s[1], s[2], s[3]];
        Ok(if self.little {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }

    fn f64(&mut self) -> Result<f64> {
        let s = self
            .b
            .get(self.at..self.at + 8)
            .ok_or_else(|| bad("short WKB"))?;
        self.at += 8;
        let a: [u8; 8] = s.try_into().unwrap();
        Ok(if self.little {
            f64::from_le_bytes(a)
        } else {
            f64::from_be_bytes(a)
        })
    }

    fn point(&mut self, out: &mut Shape, z: bool, m: bool) -> Result<bool> {
        let x = self.f64()?;
        let y = self.f64()?;
        let zv = if z { self.f64()? } else { 0.0 };
        let mv = if m { self.f64()? } else { f64::NAN };
        // An empty point is stored as NaN coordinates.
        if x.is_nan() && y.is_nan() {
            return Ok(false);
        }
        out.xy.push([x, y]);
        out.z.push(zv);
        out.m.push(mv);
        Ok(true)
    }

    fn ring(&mut self, out: &mut Shape, z: bool, m: bool, outer: bool) -> Result<()> {
        let n = self.u32()?;
        if n == 0 {
            return Ok(());
        }
        out.parts.push(out.xy.len() as u32);
        out.outer.push(outer);
        for _ in 0..n {
            self.point(out, z, m)?;
        }
        Ok(())
    }

    fn geometry(&mut self, out: &mut Shape, depth: usize) -> Result<()> {
        if depth > 4 {
            return Err(bad("WKB nested too deeply"));
        }
        let order = *self.b.get(self.at).ok_or_else(|| bad("short WKB"))?;
        self.at += 1;
        self.little = order == 1;
        let code = self.u32()?;
        // ISO WKB: +1000 Z, +2000 M, +3000 ZM; also accept the EWKB flag bits.
        let (base, z, m) = if code & 0xE000_0000 != 0 {
            (
                code & 0xffff,
                code & 0x8000_0000 != 0,
                code & 0x4000_0000 != 0,
            )
        } else {
            let dims = code / 1000;
            (code % 1000, dims == 1 || dims == 3, dims == 2 || dims == 3)
        };
        match base {
            1 => {
                self.point(out, z, m)?;
            }
            2 => self.ring(out, z, m, false)?,
            3 => {
                let rings = self.u32()?;
                for k in 0..rings {
                    self.ring(out, z, m, k == 0)?;
                }
            }
            4..=7 => {
                let n = self.u32()?;
                for _ in 0..n {
                    self.geometry(out, depth + 1)?;
                }
            }
            other => {
                return Err(Error::Unsupported(format!("WKB geometry type {other}")));
            }
        }
        Ok(())
    }
}

/// Esri ring orientation: outer rings clockwise, holes counterclockwise; a
/// reversed ring keeps its start vertex (as ArcGIS shows it).
fn orient_rings(g: &mut Geometry, outer: &[bool]) {
    let n = g.xy.len();
    for (k, &is_outer) in outer.iter().enumerate() {
        let start = g.parts[k] as usize;
        let end = g.parts.get(k + 1).map_or(n, |&e| e as usize);
        let ring = &g.xy[start..end];
        let area2: f64 = ring
            .windows(2)
            .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
            .sum();
        // Negative signed area is clockwise.
        if (area2 < 0.0) != is_outer {
            // Reverse all but the first vertex (rings are closed).
            g.xy[start + 1..end - 1].reverse();
            if let Some(z) = &mut g.z {
                z[start + 1..end - 1].reverse();
            }
            if let Some(m) = &mut g.m {
                m[start + 1..end - 1].reverse();
            }
        }
    }
}
