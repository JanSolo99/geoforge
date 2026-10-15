//! Writes one `.gdbtable` + `.gdbtablx` (version 3: 32-bit OBJECTIDs).

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveTime};
use geoforge_core::{Envelope, Error, Geometry, GeometryKind, Result, Value};

use super::geometry::encode;
use super::keys;
use crate::table::{GdbField, GdbFieldType};

const HEADER_LEN: u64 = 40;
const UNKNOWN_SRS: &str = "{B286C06B-0879-11D2-AACA-00C04FA33C20}";

fn err(message: impl Into<String>) -> Error {
    Error::format("FileGDB", message)
}

/// What a finished table hands back for its index and metadata.
pub(crate) struct Finished {
    pub envelopes: Vec<(u64, Envelope)>,
    pub extent: Envelope,
    /// Spatial grid size chosen (feature classes only).
    pub grid: Option<f64>,
    /// Date fields that hold sub-second values.
    pub subsecond: Vec<String>,
    /// Attribute indexes, entries sorted.
    pub indexes: Vec<BuiltIndex>,
}

pub(crate) struct TableWriter {
    out: BufWriter<File>,
    path: PathBuf,
    fields: Vec<GdbField>,
    geometry_kind: Option<GeometryKind>,
    has_z: bool,
    has_m: bool,
    nullable_count: usize,
    /// Where the geometry field's extent and (single) grid size are stored, to
    /// patch them at the end.
    extent_at: Option<u64>,
    grid_at: Option<u64>,
    field_section_len: u32,
    pos: u64,
    offsets: Vec<u64>,
    max_row: u32,
    pub extent: Envelope,
    /// Per-feature envelopes, for building the spatial index.
    pub envelopes: Vec<(u64, Envelope)>,
    row: Vec<u8>,
    /// Date fields that received sub-second values (need `HighPrecision`).
    pub subsecond: Vec<String>,
    /// Attribute indexes being collected.
    indexes: Vec<IndexCollector>,
}

/// Keys of one attribute index as rows are written.
struct IndexCollector {
    name: String,
    field: String,
    /// Position of the field among the values passed to `push`.
    value_at: usize,
    layout: keys::Layout,
    entries: Vec<(keys::Key, u32)>,
}

/// A finished attribute index: sorted `(key bytes, OBJECTID)` pairs.
pub(crate) struct BuiltIndex {
    pub name: String,
    pub field: String,
    pub key_size: usize,
    pub kind: u8,
    pub entries: Vec<(Vec<u8>, u32)>,
}

/// Layer flag bits: geometry type in the low byte, UTF-8 strings, "high precision"
/// storage, has M, has Z (as GDAL and ArcGIS write them).
fn layer_flags(kind: Option<GeometryKind>, has_z: bool, has_m: bool) -> u32 {
    let geom = match kind {
        None => return 0x100,
        Some(GeometryKind::Point) => 1,
        Some(GeometryKind::Multipoint) => 2,
        Some(GeometryKind::Polyline) => 3,
        Some(GeometryKind::Polygon) => 4,
        Some(GeometryKind::MultiPatch) => 9,
    };
    geom | 0x100 | 0x200 | (u32::from(has_m) << 30) | (u32::from(has_z) << 31)
}

impl TableWriter {
    /// `fields` must start with the OBJECTID field; at most one geometry field.
    pub fn create(
        base: &Path,
        fields: Vec<GdbField>,
        geometry_kind: Option<GeometryKind>,
        has_z: bool,
        has_m: bool,
    ) -> Result<Self> {
        let path = base.with_extension("gdbtable");
        let file = File::create(&path).map_err(|e| Error::io(&path, e))?;
        let mut out = BufWriter::new(file);

        let mut sec = Vec::new();
        // 4 = FileGDB 10; 6 = ArcGIS Pro 3.2 extensions (BigInteger, DateOnly, TimeOnly,
        // TimestampOffset fields), which older readers must not misinterpret.
        let pro_3_2 = fields.iter().any(|f| {
            matches!(
                f.field_type,
                GdbFieldType::Int64
                    | GdbFieldType::DateOnly
                    | GdbFieldType::TimeOnly
                    | GdbFieldType::DateTimeOffset
            )
        });
        sec.extend_from_slice(&(if pro_3_2 { 6i32 } else { 4 }).to_le_bytes());
        sec.extend_from_slice(&layer_flags(geometry_kind, has_z, has_m).to_le_bytes());
        sec.extend_from_slice(&(fields.len() as i16).to_le_bytes());
        let (mut extent_in_sec, mut grid_in_sec) = (None, None);
        for f in &fields {
            write_field(
                &mut sec,
                f,
                has_z,
                has_m,
                &mut extent_in_sec,
                &mut grid_in_sec,
            )?;
        }

        let mut header = [0u8; HEADER_LEN as usize];
        header[..4].copy_from_slice(&3i32.to_le_bytes());
        header[12..16].copy_from_slice(&5i32.to_le_bytes());
        header[32..40].copy_from_slice(&HEADER_LEN.to_le_bytes());
        let io = |e| Error::io(&path, e);
        out.write_all(&header).map_err(io)?;
        out.write_all(&(sec.len() as u32).to_le_bytes())
            .map_err(io)?;
        out.write_all(&sec).map_err(io)?;

        let nullable_count = fields.iter().filter(|f| f.nullable).count();
        Ok(Self {
            out,
            path,
            nullable_count,
            extent_at: extent_in_sec.map(|o: usize| HEADER_LEN + 4 + o as u64),
            grid_at: grid_in_sec.map(|o: usize| HEADER_LEN + 4 + o as u64),
            field_section_len: sec.len() as u32,
            pos: HEADER_LEN + 4 + sec.len() as u64,
            fields,
            geometry_kind,
            has_z,
            has_m,
            offsets: Vec::new(),
            max_row: 0,
            extent: Envelope::EMPTY,
            envelopes: Vec::new(),
            row: Vec::new(),
            subsecond: Vec::new(),
            indexes: Vec::new(),
        })
    }

    /// Indexes the values of `field` (named in the table) as rows are written.
    pub fn add_index(&mut self, name: &str, field: &str) -> Result<()> {
        let mut value_at = 0;
        let mut found = None;
        for f in &self.fields {
            if matches!(
                f.field_type,
                GdbFieldType::ObjectId | GdbFieldType::Geometry
            ) {
                continue;
            }
            if f.name.eq_ignore_ascii_case(field) {
                found = Some(f);
                break;
            }
            value_at += 1;
        }
        let f = found.ok_or_else(|| err(format!("cannot index {field}: no such field")))?;
        let layout = keys::layout(f).ok_or_else(|| {
            err(format!(
                "fields of type {:?} cannot be indexed",
                f.field_type
            ))
        })?;
        if self
            .indexes
            .iter()
            .any(|i| i.name.eq_ignore_ascii_case(name))
        {
            return Err(err(format!("index {name} already exists")));
        }
        self.indexes.push(IndexCollector {
            name: name.to_owned(),
            field: f.name.clone(),
            value_at,
            layout,
            entries: Vec::new(),
        });
        Ok(())
    }

    pub fn row_count(&self) -> u64 {
        self.offsets.len() as u64
    }

    /// Appends a row and returns its OBJECTID. `values` follow the non-OID,
    /// non-geometry fields in order.
    pub fn push(&mut self, geometry: Option<&Geometry>, values: &[Value]) -> Result<u64> {
        let oid = self.offsets.len() as u64 + 1;
        let envelope = encode_row(
            &RowLayout {
                fields: &self.fields,
                geometry_kind: self.geometry_kind,
                has_z: self.has_z,
                has_m: self.has_m,
                nullable_count: self.nullable_count,
            },
            &mut self.subsecond,
            oid,
            geometry,
            values,
            &mut self.row,
        )?;
        let row = &self.row;
        let io = |e| Error::io(&self.path, e);
        self.out
            .write_all(&(row.len() as u32).to_le_bytes())
            .map_err(io)?;
        self.out.write_all(row).map_err(io)?;
        self.offsets.push(self.pos);
        self.pos += 4 + row.len() as u64;
        self.max_row = self.max_row.max(row.len() as u32);
        if let Some(e) = envelope {
            self.extent.union(&e);
            self.envelopes.push((oid, e));
        }
        for ix in &mut self.indexes {
            if let Some(k) = values
                .get(ix.value_at)
                .and_then(|v| keys::key_of(&ix.layout, v))
            {
                ix.entries.push((k, oid as u32));
            }
        }
        Ok(oid)
    }

    /// Patches the header, extent and spatial grid size, and writes the
    /// `.gdbtablx`. Returns the per-feature envelopes and the grid size chosen.
    pub fn finish(mut self) -> Result<Finished> {
        let io = |e| Error::io(&self.path, e);
        self.out.flush().map_err(io)?;
        let mut file = self
            .out
            .into_inner()
            .map_err(|e| Error::io(&self.path, e.into_error()))?;
        let rows = self.offsets.len() as u32;
        let mut h = Vec::new();
        h.extend_from_slice(&rows.to_le_bytes());
        h.extend_from_slice(&self.max_row.max(self.field_section_len).to_le_bytes());
        file.seek(SeekFrom::Start(4)).map_err(io)?;
        file.write_all(&h).map_err(io)?;
        file.seek(SeekFrom::Start(24)).map_err(io)?;
        file.write_all(&self.pos.to_le_bytes()).map_err(io)?;
        if let (Some(at), false) = (self.extent_at, self.extent.is_empty()) {
            let e = self.extent;
            file.seek(SeekFrom::Start(at)).map_err(io)?;
            for v in [e.xmin, e.ymin, e.xmax, e.ymax] {
                file.write_all(&v.to_le_bytes()).map_err(io)?;
            }
            if self.has_z {
                let (lo, hi) = e.z.unwrap_or((f64::NAN, f64::NAN));
                file.write_all(&lo.to_le_bytes()).map_err(io)?;
                file.write_all(&hi.to_le_bytes()).map_err(io)?;
            }
            if self.has_m {
                let (lo, hi) = e.m.unwrap_or((f64::NAN, f64::NAN));
                file.write_all(&lo.to_le_bytes()).map_err(io)?;
                file.write_all(&hi.to_le_bytes()).map_err(io)?;
            }
        }
        let resolution = self
            .fields
            .iter()
            .find_map(|f| f.geometry.as_ref())
            .map_or(1e-9, |d| 1.0 / d.xy_scale);
        let grid = self
            .grid_at
            .map(|_| super::btree::grid_size(&self.envelopes, &self.extent, resolution));
        if let (Some(at), Some(g)) = (self.grid_at, grid) {
            file.seek(SeekFrom::Start(at)).map_err(io)?;
            file.write_all(&g.to_le_bytes()).map_err(io)?;
        }
        file.flush().map_err(io)?;
        write_tablx(
            &self.path.with_extension("gdbtablx"),
            &self.offsets,
            self.pos,
        )?;
        let indexes = self
            .indexes
            .into_iter()
            .map(|mut ix| {
                ix.entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
                BuiltIndex {
                    key_size: ix.layout.key_size,
                    kind: ix.layout.kind,
                    entries: ix
                        .entries
                        .iter()
                        .map(|(k, oid)| (keys::encode(&ix.layout, k), *oid))
                        .collect(),
                    name: ix.name,
                    field: ix.field,
                }
            })
            .collect();
        Ok(Finished {
            envelopes: self.envelopes,
            extent: self.extent,
            grid,
            subsecond: self.subsecond,
            indexes,
        })
    }
}

/// The parts of a table's definition that decide how a row is encoded.
pub(crate) struct RowLayout<'a> {
    pub fields: &'a [GdbField],
    pub geometry_kind: Option<GeometryKind>,
    pub has_z: bool,
    pub has_m: bool,
    pub nullable_count: usize,
}

/// Encodes one row (without its leading size) into `row`; returns the geometry's
/// envelope as stored (snapped to the resolution). `values` follow the non-OID,
/// non-geometry fields in order.
pub(crate) fn encode_row(
    layout: &RowLayout,
    subsecond: &mut Vec<String>,
    oid: u64,
    geometry: Option<&Geometry>,
    values: &[Value],
    row: &mut Vec<u8>,
) -> Result<Option<Envelope>> {
    row.clear();
    let null_bytes = layout.nullable_count.div_ceil(8);
    // Spare bits are set, like ArcGIS.
    row.resize(null_bytes, 0xFF);
    let mut nullable_i = 0;
    let mut vi = 0;
    let mut envelope = None;
    for f in layout.fields {
        if f.field_type == GdbFieldType::ObjectId {
            continue;
        }
        let value_present = if f.field_type == GdbFieldType::Geometry {
            match geometry {
                Some(g) => {
                    let def = f
                        .geometry
                        .as_ref()
                        .ok_or_else(|| err("geometry field without definition"))?;
                    // FileGDB polygons need clockwise shells, counter-clockwise holes.
                    let oriented;
                    let g = if g.kind == GeometryKind::Polygon && g.curves.is_empty() {
                        let mut o = g.clone();
                        o.orient_rings_esri();
                        oriented = o;
                        &oriented
                    } else {
                        g
                    };
                    let mut blob = Vec::new();
                    encode(
                        g,
                        def,
                        layout.geometry_kind,
                        layout.has_z,
                        layout.has_m,
                        &mut blob,
                    )
                    .map_err(|e| err(format!("OBJECTID {oid}: {e}")))?;
                    put_varuint(row, blob.len() as u64);
                    row.extend_from_slice(&blob);
                    // Index the coordinates as stored (snapped to the resolution).
                    let mut e = g.curve_envelope();
                    let snap = |v: f64, o: f64| ((v - o) * def.xy_scale).round() / def.xy_scale + o;
                    e.xmin = snap(e.xmin, def.x_origin);
                    e.xmax = snap(e.xmax, def.x_origin);
                    e.ymin = snap(e.ymin, def.y_origin);
                    e.ymax = snap(e.ymax, def.y_origin);
                    if !e.is_empty() {
                        envelope = Some(e);
                    }
                    true
                }
                None => false,
            }
        } else {
            let v = values.get(vi).unwrap_or(&Value::Null);
            vi += 1;
            if v.is_null() {
                false
            } else {
                write_value(row, f, v)
                    .map_err(|e| err(format!("OBJECTID {oid}, field {}: {e}", f.name)))?;
                if let (GdbFieldType::DateTime, Value::Date(t)) = (f.field_type, v)
                    && t.and_utc().timestamp_subsec_millis() != 0
                    && !subsecond.contains(&f.name)
                {
                    subsecond.push(f.name.clone());
                }
                true
            }
        };
        if f.nullable {
            if value_present {
                row[nullable_i / 8] &= !(1 << (nullable_i % 8));
            }
            nullable_i += 1;
        } else if !value_present {
            return Err(err(format!(
                "OBJECTID {oid}: field {} is not nullable",
                f.name
            )));
        }
    }
    Ok(envelope)
}

fn write_tablx(path: &Path, offsets: &[u64], file_size: u64) -> Result<()> {
    let size: usize = if file_size < 1 << 32 {
        4
    } else if file_size < 1 << 40 {
        5
    } else {
        6
    };
    let blocks = offsets.len().div_ceil(1024).max(1);
    let mut out = Vec::with_capacity(16 + blocks * 1024 * size + 16);
    out.extend_from_slice(&3i32.to_le_bytes());
    out.extend_from_slice(&(blocks as u32).to_le_bytes());
    out.extend_from_slice(&(offsets.len() as u32).to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
    for i in 0..blocks * 1024 {
        let o = offsets.get(i).copied().unwrap_or(0);
        out.extend_from_slice(&o.to_le_bytes()[..size]);
    }
    // Trailer: no block bitmap (all blocks present).
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(blocks as u32).to_le_bytes());
    out.extend_from_slice(&(blocks as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    std::fs::write(path, out).map_err(|e| Error::io(path, e))
}

pub(crate) fn put_varuint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

pub(crate) fn put_varint(out: &mut Vec<u8>, v: i64) {
    let mut mag = v.unsigned_abs();
    let mut b = (mag & 0x3F) as u8;
    if v < 0 {
        b |= 0x40;
    }
    mag >>= 6;
    if mag == 0 {
        out.push(b);
        return;
    }
    out.push(b | 0x80);
    put_varuint(out, mag);
}

fn utf16(out: &mut Vec<u8>, s: &str) {
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
}

fn write_field(
    sec: &mut Vec<u8>,
    f: &GdbField,
    layer_z: bool,
    layer_m: bool,
    extent_at: &mut Option<usize>,
    grid_at: &mut Option<usize>,
) -> Result<()> {
    let name_len = f.name.encode_utf16().count();
    if name_len > 255 {
        return Err(err(format!("field name too long: {}", f.name)));
    }
    sec.push(name_len as u8);
    utf16(sec, &f.name);
    let alias = f.alias.as_deref().unwrap_or("");
    sec.push(alias.encode_utf16().count().min(255) as u8);
    utf16(sec, alias);
    use GdbFieldType as G;
    let code = match f.field_type {
        G::Int16 => 0,
        G::Int32 => 1,
        G::Float32 => 2,
        G::Float64 => 3,
        G::String => 4,
        G::DateTime => 5,
        G::ObjectId => 6,
        G::Geometry => 7,
        G::Binary => 8,
        G::Raster => return Err(Error::Unsupported("writing raster fields".into())),
        G::Guid => 10,
        G::GlobalId => 11,
        G::Xml => 12,
        G::Int64 => 13,
        G::DateOnly => 14,
        G::TimeOnly => 15,
        G::DateTimeOffset => 16,
    };
    sec.push(code);
    // Flag bits: 1 nullable, 2 required, 4 editable.
    let flag = if f.nullable { 5 } else { 4 };
    match f.field_type {
        G::ObjectId => {
            sec.push(4);
            sec.push(2);
        }
        G::String => {
            sec.extend_from_slice(&f.length.to_le_bytes());
            sec.push(flag);
            let d = f.default.as_deref().unwrap_or_default();
            put_varuint(sec, d.len() as u64);
            sec.extend_from_slice(d);
        }
        G::Geometry => {
            let def = f
                .geometry
                .as_ref()
                .ok_or_else(|| err("geometry field without definition"))?;
            sec.push(0);
            sec.push(if f.nullable { 7 } else { 6 });
            let wkt = def.srs_wkt.as_deref().unwrap_or(UNKNOWN_SRS);
            let mut w = Vec::new();
            utf16(&mut w, wkt);
            sec.extend_from_slice(&(w.len() as i16).to_le_bytes());
            sec.extend_from_slice(&w);
            sec.push(1 | (u8::from(def.has_z) << 1) | (u8::from(def.has_m) << 2));
            let mut put = |v: f64| sec.extend_from_slice(&v.to_le_bytes());
            put(def.x_origin);
            put(def.y_origin);
            put(def.xy_scale);
            if def.has_m {
                put(def.m_origin);
                put(def.m_scale);
            }
            if def.has_z {
                put(def.z_origin);
                put(def.z_scale);
            }
            put(def.xy_tolerance);
            if def.has_m {
                put(def.m_tolerance);
            }
            if def.has_z {
                put(def.z_tolerance);
            }
            *extent_at = Some(sec.len());
            for _ in 0..4 + 2 * usize::from(layer_z) + 2 * usize::from(layer_m) {
                sec.extend_from_slice(&f64::NAN.to_le_bytes());
            }
            sec.push(0);
            let grids = if def.grid_sizes.is_empty() {
                vec![0.0]
            } else {
                def.grid_sizes.clone()
            };
            sec.extend_from_slice(&(grids.len() as u32).to_le_bytes());
            if grids.len() == 1 && grids[0] == 0.0 {
                *grid_at = Some(sec.len()); // computed from the data in finish()
            }
            for g in grids {
                sec.extend_from_slice(&g.to_le_bytes());
            }
        }
        G::Binary | G::Xml => {
            sec.push(0);
            sec.push(flag);
        }
        G::Guid | G::GlobalId => {
            sec.push(38);
            sec.push(flag);
        }
        _ => {
            let width = match f.field_type {
                G::Int16 => 2,
                G::Int32 | G::Float32 => 4,
                G::DateTimeOffset => 10,
                _ => 8,
            };
            sec.push(width);
            sec.push(flag);
            let d = f.default.as_deref().unwrap_or_default();
            sec.push(d.len().min(255) as u8);
            sec.extend_from_slice(&d[..d.len().min(255)]);
        }
    }
    Ok(())
}

fn num(v: &Value) -> Option<f64> {
    Some(match v {
        Value::SmallInteger(n) => f64::from(*n),
        Value::Integer(n) => f64::from(*n),
        Value::BigInteger(n) => *n as f64,
        Value::Single(n) => f64::from(*n),
        Value::Double(n) => *n,
        Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    })
}

fn int_in<T: TryFrom<i64>>(v: &Value) -> Result<T> {
    let n = match v {
        Value::BigInteger(n) => *n,
        other => {
            let f = num(other).ok_or_else(|| err(format!("{other:?} is not a number")))?;
            if f.fract() != 0.0 {
                return Err(err(format!("{f} is not an integer")));
            }
            f as i64
        }
    };
    T::try_from(n).map_err(|_| err(format!("{n} is out of range for the field")))
}

fn days(t: chrono::NaiveDateTime) -> f64 {
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    (t - epoch).num_milliseconds() as f64 / 86_400_000.0
}

fn write_value(out: &mut Vec<u8>, f: &GdbField, v: &Value) -> Result<()> {
    use GdbFieldType as G;
    let text = |v: &Value| -> Result<String> {
        Ok(match v {
            Value::String(s) | Value::Guid(s) | Value::Xml(s) => s.clone(),
            Value::SmallInteger(n) => n.to_string(),
            Value::Integer(n) => n.to_string(),
            Value::BigInteger(n) => n.to_string(),
            Value::Single(n) => n.to_string(),
            Value::Double(n) => n.to_string(),
            Value::Date(d) => d.to_string(),
            Value::DateOnly(d) => d.to_string(),
            Value::TimeOnly(t) => t.to_string(),
            Value::TimestampOffset(t) => t.to_rfc3339(),
            other => return Err(err(format!("{other:?} cannot be stored as text"))),
        })
    };
    match f.field_type {
        G::Int16 => out.extend_from_slice(&int_in::<i16>(v)?.to_le_bytes()),
        G::Int32 => out.extend_from_slice(&int_in::<i32>(v)?.to_le_bytes()),
        G::Int64 => out.extend_from_slice(&int_in::<i64>(v)?.to_le_bytes()),
        G::Float32 => out
            .extend_from_slice(&(num(v).ok_or_else(|| err("not a number"))? as f32).to_le_bytes()),
        G::Float64 => {
            out.extend_from_slice(&num(v).ok_or_else(|| err("not a number"))?.to_le_bytes())
        }
        G::String | G::Xml => {
            let s = text(v)?;
            if f.field_type == G::String && f.length > 0 && s.chars().count() > f.length as usize {
                return Err(err(format!(
                    "text longer than the field length {}",
                    f.length
                )));
            }
            put_varuint(out, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        G::Binary => match v {
            Value::Blob(b) => {
                put_varuint(out, b.len() as u64);
                out.extend_from_slice(b);
            }
            other => return Err(err(format!("{other:?} is not binary"))),
        },
        G::Guid | G::GlobalId => out.extend_from_slice(&parse_guid(&text(v)?)?),
        G::DateTime => match v {
            Value::Date(t) => out.extend_from_slice(&days(*t).to_le_bytes()),
            Value::DateOnly(d) => {
                out.extend_from_slice(&days(d.and_hms_opt(0, 0, 0).unwrap()).to_le_bytes())
            }
            Value::TimestampOffset(t) => {
                out.extend_from_slice(&days(t.naive_local()).to_le_bytes())
            }
            other => return Err(err(format!("{other:?} is not a date"))),
        },
        G::DateOnly => match v {
            Value::DateOnly(d) => {
                out.extend_from_slice(&days(d.and_hms_opt(0, 0, 0).unwrap()).to_le_bytes())
            }
            Value::Date(t) => {
                out.extend_from_slice(&days(t.date().and_hms_opt(0, 0, 0).unwrap()).to_le_bytes())
            }
            other => return Err(err(format!("{other:?} is not a date"))),
        },
        G::TimeOnly => match v {
            Value::TimeOnly(t) => {
                let ms = (*t - NaiveTime::MIN).num_milliseconds() as f64;
                out.extend_from_slice(&(ms / 86_400_000.0).to_le_bytes());
            }
            other => return Err(err(format!("{other:?} is not a time"))),
        },
        G::DateTimeOffset => match v {
            Value::TimestampOffset(t) => {
                out.extend_from_slice(&days(t.naive_local()).to_le_bytes());
                out.extend_from_slice(&((t.offset().local_minus_utc() / 60) as i16).to_le_bytes());
            }
            other => return Err(err(format!("{other:?} is not a timestamp with offset"))),
        },
        G::ObjectId | G::Geometry | G::Raster => unreachable!("handled by the caller"),
    }
    Ok(())
}

/// `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` → the 16 stored bytes (CLSID order).
pub(crate) fn parse_guid(s: &str) -> Result<[u8; 16]> {
    let hex: String = s.chars().filter(char::is_ascii_hexdigit).collect();
    if hex.len() != 32 {
        return Err(err(format!("invalid GUID {s}")));
    }
    let b: Vec<u8> = (0..16)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    Ok([
        b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13],
        b[14], b[15],
    ])
}
