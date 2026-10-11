//! Writing feature classes to a GeoPackage as ArcGIS Pro 3.7 does: a new file
//! gets the schema ArcGIS creates (`base.rs`); a feature table gets ArcGIS's
//! column definitions with type checks, an autoincrementing `OBJECTID`
//! numbered from 1, a `Shape` column of little-endian GeoPackage blobs without
//! an envelope (polygons as MULTIPOLYGON with OGC ring order, lines as
//! MULTILINESTRING), and rows in gpkg_contents, gpkg_geometry_columns,
//! gpkg_spatial_ref_sys and sqlite_sequence.
//!
//! Adding a table to an existing GeoPackage rewrites the file: every table and
//! index is read and bulk-loaded again with the new table, so the result is as
//! compact as a fresh file.

use std::collections::HashSet;
use std::path::Path;

use chrono::{Timelike, Utc};
use geoforge_core::{Error, Feature, Field, FieldType, Geometry, GeometryKind, Result, Value};

use crate::base;
use crate::sqlite::{self, Column, Database, Row, SqlValue};
use crate::sqlite_writer::{Options, ValueRef, Writer};

/// A coordinate system as stored in gpkg_spatial_ref_sys.
#[derive(Debug, Clone)]
pub struct SpatialRef {
    /// `srs_id`; ArcGIS uses the EPSG code.
    pub srs_id: i64,
    pub srs_name: String,
    pub organization: String,
    pub organization_coordsys_id: i64,
    /// Esri WKT.
    pub definition: String,
    pub description: Option<String>,
    /// WKT2 (`definition_12_063`), or `undefined`.
    pub definition_12_063: String,
}

/// A feature class or table to write.
pub struct NewTable<'a> {
    pub name: &'a str,
    /// Geometry type, Z and M; `None` for a table.
    pub geometry: Option<(GeometryKind, bool, bool)>,
    pub fields: &'a [Field],
    pub spatial_ref: Option<&'a SpatialRef>,
    /// The XY domain (x origin, y origin, resolution) coordinates are snapped
    /// to, as ArcGIS stores them; `None` keeps them as they are.
    pub xy_domain: Option<(f64, f64, f64)>,
    pub features: &'a [Feature],
}

/// One schema object of the database being rebuilt.
enum Object {
    Table {
        name: String,
        sql: String,
        columns: Vec<Column>,
        rows: Vec<Row>,
    },
    Index {
        name: String,
        table: String,
        sql: Option<String>,
        /// Entries copied from the source file (tables that did not change).
        entries: Option<Vec<(Vec<SqlValue>, i64)>>,
    },
    Other {
        kind: &'static str,
        name: String,
        table: String,
        sql: String,
    },
}

impl Object {
    fn table_name(&self) -> &str {
        match self {
            Object::Table { name, .. } => name,
            Object::Index { table, .. } | Object::Other { table, .. } => table,
        }
    }

    fn name(&self) -> &str {
        match self {
            Object::Table { name, .. }
            | Object::Index { name, .. }
            | Object::Other { name, .. } => name,
        }
    }
}

/// Writes `table` into the GeoPackage at `path` (created if missing), replacing
/// a table of the same name.
pub fn write_table(path: &Path, table: &NewTable) -> Result<()> {
    let (mut objects, options) = if path.exists() {
        load(path)?
    } else {
        (
            fresh(),
            Options {
                page_size: 4096,
                application_id: 0x4750_4B47,
                user_version: 10400,
            },
        )
    };
    let lower = table.name.to_ascii_lowercase();
    // Drop an existing table of that name with its spatial index and triggers.
    let rtree = format!("rtree_{lower}_");
    objects.retain(|o| {
        let t = o.table_name().to_ascii_lowercase();
        let n = o.name().to_ascii_lowercase();
        t != lower && !n.starts_with(&rtree) && !t.starts_with(&rtree)
    });
    let mut changed: HashSet<String> = HashSet::new();
    for (sys, column) in [
        ("gpkg_contents", "table_name"),
        ("gpkg_geometry_columns", "table_name"),
        ("gpkg_extensions", "table_name"),
        ("gpkg_data_columns", "table_name"),
        ("sqlite_sequence", "name"),
    ] {
        if let Some(Object::Table { columns, rows, .. }) = find_table(&mut objects, sys) {
            let c = columns
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(column));
            if let Some(c) = c {
                let before = rows.len();
                rows.retain(|(_, v)| {
                    !v[c]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(table.name))
                });
                if rows.len() != before {
                    changed.insert(sys.to_string());
                }
            }
        }
    }

    // gpkg_spatial_ref_sys, gpkg_contents, gpkg_geometry_columns rows.
    let srs_id = match table.spatial_ref {
        Some(sr) => {
            let srs = find_table(&mut objects, "gpkg_spatial_ref_sys")
                .ok_or_else(|| Error::format("GeoPackage", "no gpkg_spatial_ref_sys"))?;
            if let Object::Table { rows, .. } = srs
                && !rows.iter().any(|(_, v)| v[1].as_i64() == Some(sr.srs_id))
            {
                // srs_id is the rowid.
                let at = rows.partition_point(|(r, _)| *r < sr.srs_id);
                rows.insert(
                    at,
                    (
                        sr.srs_id,
                        vec![
                            SqlValue::Text(sr.srs_name.clone()),
                            SqlValue::Integer(sr.srs_id),
                            SqlValue::Text(sr.organization.clone()),
                            SqlValue::Integer(sr.organization_coordsys_id),
                            SqlValue::Text(sr.definition.clone()),
                            sr.description
                                .clone()
                                .map_or(SqlValue::Null, SqlValue::Text),
                            SqlValue::Text(sr.definition_12_063.clone()),
                        ],
                    ),
                );
            }
            sr.srs_id
        }
        None => -1,
    };
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    append_row(
        &mut objects,
        "gpkg_contents",
        vec![
            SqlValue::Text(table.name.to_string()),
            SqlValue::Text(
                if table.geometry.is_some() {
                    "features"
                } else {
                    "attributes"
                }
                .into(),
            ),
            SqlValue::Text(table.name.to_string()),
            SqlValue::Null,
            SqlValue::Text(now),
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::Null,
            SqlValue::Null,
            if table.geometry.is_some() {
                SqlValue::Integer(srs_id)
            } else {
                SqlValue::Null
            },
        ],
    )?;
    changed.insert("gpkg_contents".into());
    if let Some((kind, z, m)) = table.geometry {
        append_row(
            &mut objects,
            "gpkg_geometry_columns",
            vec![
                SqlValue::Text(table.name.to_string()),
                SqlValue::Text("Shape".into()),
                SqlValue::Text(geometry_type_name(kind)?.into()),
                SqlValue::Integer(srs_id),
                SqlValue::Integer(z as i64),
                SqlValue::Integer(m as i64),
            ],
        )?;
        changed.insert("gpkg_geometry_columns".into());
    }
    let count = table.features.len() as i64;
    append_row(
        &mut objects,
        "sqlite_sequence",
        vec![
            SqlValue::Text(table.name.to_string()),
            SqlValue::Integer(count),
        ],
    )?;
    changed.insert("sqlite_sequence".into());

    // The feature table itself.
    let sql = create_table_sql(table)?;
    let columns = sqlite::parse_columns(&sql);
    let mut rows = Vec::with_capacity(table.features.len());
    let mut boxes = Vec::new();
    for (k, f) in table.features.iter().enumerate() {
        let oid = k as i64 + 1;
        let mut values = Vec::with_capacity(table.fields.len() + 2);
        values.push(SqlValue::Integer(oid));
        if let Some((_, z, m)) = table.geometry {
            values.push(match &f.geometry {
                Some(g) => {
                    let g = match table.xy_domain {
                        Some(d) => snapped(g, d),
                        None => g.clone(),
                    };
                    if let Some(b) = bounds(&g) {
                        boxes.push((oid, b));
                    }
                    SqlValue::Blob(encode_geometry(&g, srs_id as i32, z, m)?)
                }
                None => SqlValue::Null,
            });
        }
        for (v, field) in f.attributes.iter().zip(table.fields) {
            values.push(value(v, field)?);
        }
        rows.push((oid, values));
    }
    objects.push(Object::Table {
        name: table.name.to_string(),
        sql,
        columns,
        rows,
    });
    if table.geometry.is_some() {
        // The spatial index, as ArcGIS creates it, bulk-loaded.
        let node_size = (options.page_size - 64).min(4 + 24 * 51);
        let (nodes, rowids, parents) = build_rtree(boxes, node_size);
        let mut shadow = [Some(rowids), Some(nodes), Some(parents)].into_iter();
        for &(kind, name, sql) in base::RTREE_OBJECTS {
            let (name, sql) = (
                name.replace("{t}", table.name),
                sql.replace("{t}", table.name),
            );
            if kind == "table" && !sql.starts_with("CREATE VIRTUAL") {
                let rows = shadow.next().flatten().unwrap_or_default();
                objects.push(Object::Table {
                    columns: sqlite::parse_columns(&sql),
                    name,
                    sql,
                    rows,
                });
            } else {
                objects.push(Object::Other {
                    kind: static_kind(kind),
                    table: if kind == "trigger" {
                        table.name.to_string()
                    } else {
                        name.clone()
                    },
                    name,
                    sql,
                });
            }
        }
        append_row(
            &mut objects,
            "gpkg_extensions",
            vec![
                SqlValue::Text(table.name.to_string()),
                SqlValue::Text("Shape".into()),
                SqlValue::Text("gpkg_rtree_index".into()),
                SqlValue::Text("F.3 RTree Spatial Index".into()),
                SqlValue::Text("write-only".into()),
            ],
        )?;
        changed.insert("gpkg_extensions".into());
    }

    // Indexes of changed tables are rebuilt from their rows.
    for o in &mut objects {
        if let Object::Index { table, entries, .. } = o
            && changed.contains(&table.to_ascii_lowercase())
        {
            *entries = None;
        }
    }
    write_objects(path, objects, options)
}

fn find_table<'a>(objects: &'a mut [Object], name: &str) -> Option<&'a mut Object> {
    objects
        .iter_mut()
        .find(|o| matches!(o, Object::Table { name: n, .. } if n.eq_ignore_ascii_case(name)))
}

fn append_row(objects: &mut [Object], table: &str, values: Vec<SqlValue>) -> Result<()> {
    let Some(Object::Table { rows, .. }) = find_table(objects, table) else {
        return Err(Error::format("GeoPackage", format!("no {table} table")));
    };
    let rowid = rows.last().map_or(1, |r| r.0 + 1);
    rows.push((rowid, values));
    Ok(())
}

/// The objects of a new GeoPackage as ArcGIS creates it.
fn fresh() -> Vec<Object> {
    let mut objects = Vec::new();
    for &(kind, name, table, _, sql) in base::OBJECTS {
        objects.push(match (kind, sql) {
            ("table", Some(sql)) if !sql.starts_with("CREATE VIRTUAL") => Object::Table {
                name: name.into(),
                sql: sql.into(),
                columns: sqlite::parse_columns(sql),
                rows: Vec::new(),
            },
            ("index", sql) => Object::Index {
                name: name.into(),
                table: table.into(),
                sql: sql.map(str::to_string),
                entries: None,
            },
            (kind, sql) => Object::Other {
                kind: static_kind(kind),
                name: name.into(),
                table: table.into(),
                sql: sql.unwrap_or_default().into(),
            },
        });
    }
    let text = |s: &str| SqlValue::Text(s.to_string());
    if let Some(Object::Table { rows, .. }) = find_table(&mut objects, "gpkg_spatial_ref_sys") {
        for &(name, id, org, org_id, def, desc, def2) in base::SPATIAL_REF_SYS {
            rows.push((
                id,
                vec![
                    text(name),
                    SqlValue::Integer(id),
                    text(org),
                    SqlValue::Integer(org_id),
                    text(def),
                    desc.map_or(SqlValue::Null, text),
                    text(def2),
                ],
            ));
        }
        rows.sort_by_key(|r| r.0);
    }
    if let Some(Object::Table { rows, .. }) = find_table(&mut objects, "gpkg_extensions") {
        for (k, &(t, c, ext, def, scope)) in base::EXTENSIONS.iter().enumerate() {
            let opt = |v: Option<&str>| v.map_or(SqlValue::Null, text);
            rows.push((
                k as i64 + 1,
                vec![opt(t), opt(c), text(ext), text(def), text(scope)],
            ));
        }
    }
    objects
}

fn static_kind(kind: &str) -> &'static str {
    match kind {
        "table" => "table",
        "index" => "index",
        "view" => "view",
        _ => "trigger",
    }
}

/// Reads every object of an existing database.
fn load(path: &Path) -> Result<(Vec<Object>, Options)> {
    let db = Database::open(path)?;
    let (application_id, user_version) = db.ids();
    let mut objects = Vec::new();
    for o in db.objects() {
        let sql = o.sql.clone().unwrap_or_default();
        objects.push(match o.kind.as_str() {
            "table" if o.root_page != 0 => {
                let t = db
                    .table(&o.name)
                    .ok_or_else(|| Error::format("SQLite", format!("table {}", o.name)))?;
                Object::Table {
                    name: o.name.clone(),
                    sql,
                    columns: t.columns.clone(),
                    rows: db.select(t)?,
                }
            }
            "index" => Object::Index {
                name: o.name.clone(),
                table: o.table.clone(),
                sql: o.sql.clone(),
                entries: Some(db.index_entries(o.root_page)?),
            },
            kind => Object::Other {
                kind: static_kind(kind),
                name: o.name.clone(),
                table: o.table.clone(),
                sql,
            },
        });
    }
    Ok((
        objects,
        Options {
            page_size: db.page_size(),
            application_id,
            user_version,
        },
    ))
}

/// Bulk-writes all objects in schema order.
fn write_objects(path: &Path, objects: Vec<Object>, options: Options) -> Result<()> {
    let mut w = Writer::create(path, options)?;
    // Autoindex numbering per table.
    let mut values: Vec<ValueRef> = Vec::new();
    for o in &objects {
        match o {
            Object::Table {
                name,
                sql,
                columns,
                rows,
            } => {
                let mut tw = w.table(name, sql);
                for (rowid, row) in rows {
                    values.clear();
                    values.extend(row.iter().zip(columns).map(|(v, c)| {
                        if c.rowid_alias {
                            ValueRef::Null
                        } else {
                            v.as_value_ref()
                        }
                    }));
                    tw.push(*rowid, &values)?;
                }
                tw.finish()?;
            }
            Object::Index {
                name,
                table,
                sql,
                entries,
            } => {
                let entries = match entries {
                    Some(e) => e.clone(),
                    None => index_entries(&objects, name, table, sql.as_deref())?,
                };
                w.index(name, table, sql.as_deref(), entries)?;
            }
            Object::Other {
                kind,
                name,
                table,
                sql,
            } => w.schema_entry(kind, name, table, sql),
        }
    }
    w.finish()
}

/// Entries of an index computed from its table's rows.
fn index_entries(
    objects: &[Object],
    name: &str,
    table: &str,
    sql: Option<&str>,
) -> Result<Vec<(Vec<SqlValue>, i64)>> {
    let Some(Object::Table {
        sql: table_sql,
        columns,
        rows,
        ..
    }) = objects
        .iter()
        .find(|o| matches!(o, Object::Table { name: n, .. } if n.eq_ignore_ascii_case(table)))
    else {
        return Err(Error::format(
            "SQLite",
            format!("index {name} without table {table}"),
        ));
    };
    let cols = match sql {
        Some(sql) => sqlite::index_columns(sql, columns),
        None => name
            .rsplit('_')
            .next()
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| {
                sqlite::autoindex_columns(table_sql, columns)
                    .get(n - 1)
                    .cloned()
            }),
    }
    .ok_or_else(|| Error::Unsupported(format!("rebuilding index {name}")))?;
    Ok(rows
        .iter()
        .map(|(rowid, v)| (cols.iter().map(|&c| v[c].clone()).collect(), *rowid))
        .collect())
}

fn geometry_type_name(kind: GeometryKind) -> Result<&'static str> {
    Ok(match kind {
        GeometryKind::Point => "POINT",
        GeometryKind::Multipoint => "MULTIPOINT",
        GeometryKind::Polyline => "MULTILINESTRING",
        GeometryKind::Polygon => "MULTIPOLYGON",
        GeometryKind::MultiPatch => {
            return Err(Error::Unsupported("multipatch in a GeoPackage".into()));
        }
    })
}

/// ArcGIS's CREATE TABLE for a feature class: columns in `[brackets]`, each
/// with a check of its storage class (and range or length).
fn create_table_sql(table: &NewTable) -> Result<String> {
    let mut cols = vec!["[OBJECTID] INTEGER primary key autoincrement not null".to_string()];
    if let Some((kind, _, _)) = table.geometry {
        cols.push(format!("[Shape] {}", geometry_type_name(kind)?));
    }
    for f in table.fields {
        let n = &f.name;
        let typed = |t: &str| format!("typeof([{n}]) = '{t}' or typeof([{n}]) = 'null'");
        cols.push(match f.field_type {
            FieldType::SmallInteger => format!(
                "[{n}] SMALLINT check(({}) and [{n}] >= -32768 and [{n}] <= 32767)",
                typed("integer")
            ),
            FieldType::Integer => format!(
                "[{n}] MEDIUMINT check(({}) and [{n}] >= -2147483648 and [{n}] <= 2147483647)",
                typed("integer")
            ),
            FieldType::BigInteger => format!("[{n}] INTEGER check({})", typed("integer")),
            FieldType::Single => format!(
                "[{n}] FLOAT check(({}) and [{n}] >= -340282346638528860000000000000000000000.000000 \
                 and [{n}] <= 340282346638528860000000000000000000000.000000)",
                typed("real")
            ),
            FieldType::Double => format!("[{n}] DOUBLE check({})", typed("real")),
            FieldType::String => format!(
                "[{n}] TEXT({len}) check(({}) and not length([{n}]) > {len})",
                typed("text"),
                len = f.length
            ),
            FieldType::Date => format!(
                "[{n}] DATETIME check((typeof({n}) = 'text' or typeof({n}) = 'null') and \
                 strftime('%Y-%m-%dT%H:%M:%fZ',{n}))"
            ),
            FieldType::DateOnly => format!(
                "[{n}] DATE check((typeof({n}) = 'text' or typeof({n}) = 'null') and \
                 strftime('%Y-%m-%d',{n}))"
            ),
            FieldType::Guid | FieldType::GlobalId => format!(
                "[{n}] TEXT(38) collate nocase check(({}) and length([{n}]) = 38)",
                typed("text")
            ),
            FieldType::Blob => format!("[{n}] BLOB check({})", typed("blob")),
            other => {
                return Err(Error::Unsupported(format!(
                    "ERROR 000308: Invalid field type: field {n} ({other:?}) cannot be stored \
                     in a GeoPackage"
                )));
            }
        });
    }
    Ok(format!(
        "CREATE TABLE [{}] ({})",
        table.name,
        cols.join(", ")
    ))
}

/// A value as ArcGIS stores it: dates as text rounded to whole seconds.
fn value(v: &Value, field: &Field) -> Result<SqlValue> {
    Ok(match v {
        Value::Null => SqlValue::Null,
        Value::SmallInteger(i) => SqlValue::Integer(*i as i64),
        Value::Integer(i) => SqlValue::Integer(*i as i64),
        Value::BigInteger(i) => SqlValue::Integer(*i),
        Value::Single(f) => SqlValue::Real(*f as f64),
        Value::Double(f) => SqlValue::Real(*f),
        Value::String(s) | Value::Guid(s) => SqlValue::Text(s.clone()),
        Value::Date(d) => {
            let rounded = if d.nanosecond() >= 500_000_000 {
                *d + chrono::Duration::seconds(1)
            } else {
                *d
            };
            SqlValue::Text(rounded.format("%Y-%m-%dT%H:%M:%S.0Z").to_string())
        }
        Value::DateOnly(d) => SqlValue::Text(d.format("%Y-%m-%d").to_string()),
        Value::Blob(b) => SqlValue::Blob(b.clone()),
        other => {
            return Err(Error::Unsupported(format!(
                "value {other:?} for field {}",
                field.name
            )));
        }
    })
}

/// Coordinates snapped to the XY domain as ArcGIS stores them (integers of
/// the resolution from the origin), and Z and M to ArcGIS's default Z/M
/// domain (origin -100000, resolution 0.0001).
fn snapped(g: &Geometry, (x0, y0, resolution): (f64, f64, f64)) -> Geometry {
    let snap = |v: f64, origin: f64, scale: f64| {
        if v.is_finite() {
            ((v - origin) * scale).round() / scale + origin
        } else {
            v
        }
    };
    let scale = 1.0 / resolution;
    let mut g = g.clone();
    for p in &mut g.xy {
        *p = [snap(p[0], x0, scale), snap(p[1], y0, scale)];
    }
    for values in [&mut g.z, &mut g.m].into_iter().flatten() {
        for v in values.iter_mut() {
            *v = snap(*v, -100000.0, 10000.0);
        }
    }
    g
}

/// The envelope (min x, max x, min y, max y) of a non-empty shape.
fn bounds(g: &Geometry) -> Option<[f64; 4]> {
    let first = g.xy.first()?;
    let mut b = [first[0], first[0], first[1], first[1]];
    for p in &g.xy {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].max(p[0]);
        b[2] = b[2].min(p[1]);
        b[3] = b[3].max(p[1]);
    }
    Some(b)
}

/// SQLite's rounding of R*Tree bounds to single precision: minimums down,
/// maximums up.
fn round_down(d: f64) -> f32 {
    let f = d as f32;
    if (f as f64) > d {
        (d * if d < 0.0 {
            1.0 + 1.0 / 8388608.0
        } else {
            1.0 - 1.0 / 8388608.0
        }) as f32
    } else {
        f
    }
}

fn round_up(d: f64) -> f32 {
    let f = d as f32;
    if (f as f64) < d {
        (d * if d < 0.0 {
            1.0 - 1.0 / 8388608.0
        } else {
            1.0 + 1.0 / 8388608.0
        }) as f32
    } else {
        f
    }
}

/// An R*Tree cell: a feature or child node and its box (min x, max x, min y,
/// max y).
type RtreeCell = (i64, [f32; 4]);

/// The shadow tables of an SQLite R*Tree over `boxes` (rowid, envelope),
/// bulk-loaded by sort-tile-recursive packing: `_node` rows (node number,
/// blob), `_rowid` rows (feature, leaf node) and `_parent` rows (node, parent).
/// Node 1 is the root; it stores the tree's depth.
fn build_rtree(boxes: Vec<(i64, [f64; 4])>, node_size: usize) -> (Vec<Row>, Vec<Row>, Vec<Row>) {
    let capacity = (node_size - 4) / 24;
    let mut entries: Vec<RtreeCell> = boxes
        .into_iter()
        .map(|(id, b)| {
            (
                id,
                [
                    round_down(b[0]),
                    round_up(b[1]),
                    round_down(b[2]),
                    round_up(b[3]),
                ],
            )
        })
        .collect();
    let mut nodes: Vec<(i64, Vec<RtreeCell>)> = Vec::new();
    let mut rowids: Vec<(i64, i64)> = Vec::new();
    let mut parents: Vec<(i64, i64)> = Vec::new();
    let mut next_id = 2i64;
    let mut depth = 0u16;
    loop {
        if entries.len() <= capacity {
            // The root.
            if depth == 0 {
                rowids.extend(entries.iter().map(|e| (e.0, 1)));
            } else {
                parents.extend(entries.iter().map(|e| (e.0, 1)));
            }
            nodes.push((1, entries));
            break;
        }
        // Sort-tile-recursive: slices by x, then runs by y within each slice.
        let count = entries.len().div_ceil(capacity);
        let slices = (count as f64).sqrt().ceil() as usize;
        let per_slice = slices * capacity;
        let center = |b: &[f32; 4], i: usize| (b[2 * i] as f64 + b[2 * i + 1] as f64) / 2.0;
        entries.sort_by(|a, b| center(&a.1, 0).total_cmp(&center(&b.1, 0)));
        let mut level = Vec::new();
        for slice in entries.chunks_mut(per_slice) {
            slice.sort_by(|a, b| center(&a.1, 1).total_cmp(&center(&b.1, 1)));
            for run in slice.chunks(capacity) {
                let id = next_id;
                next_id += 1;
                let mut b = run[0].1;
                for e in run {
                    b[0] = b[0].min(e.1[0]);
                    b[1] = b[1].max(e.1[1]);
                    b[2] = b[2].min(e.1[2]);
                    b[3] = b[3].max(e.1[3]);
                }
                if depth == 0 {
                    rowids.extend(run.iter().map(|e| (e.0, id)));
                } else {
                    parents.extend(run.iter().map(|e| (e.0, id)));
                }
                nodes.push((id, run.to_vec()));
                level.push((id, b));
            }
        }
        entries = level;
        depth += 1;
    }
    nodes.sort_by_key(|n| n.0);
    rowids.sort_unstable();
    parents.sort_unstable();
    let blob = |id: i64, cells: &[RtreeCell]| {
        let mut b = vec![0u8; node_size];
        if id == 1 {
            b[..2].copy_from_slice(&depth.to_be_bytes());
        }
        b[2..4].copy_from_slice(&(cells.len() as u16).to_be_bytes());
        for (k, (cid, bx)) in cells.iter().enumerate() {
            let at = 4 + 24 * k;
            b[at..at + 8].copy_from_slice(&cid.to_be_bytes());
            for (j, v) in bx.iter().enumerate() {
                b[at + 8 + 4 * j..at + 12 + 4 * j].copy_from_slice(&v.to_be_bytes());
            }
        }
        b
    };
    let int = SqlValue::Integer;
    (
        nodes
            .iter()
            .map(|(id, cells)| (*id, vec![int(*id), SqlValue::Blob(blob(*id, cells))]))
            .collect(),
        rowids
            .into_iter()
            .map(|(r, n)| (r, vec![int(r), int(n)]))
            .collect(),
        parents
            .into_iter()
            .map(|(n, p)| (n, vec![int(n), int(p)]))
            .collect(),
    )
}

/// A GeoPackage geometry blob: header (version 0, little-endian, no envelope,
/// the empty flag for empty shapes) and ISO WKB.
pub fn encode_geometry(g: &Geometry, srs_id: i32, has_z: bool, has_m: bool) -> Result<Vec<u8>> {
    if g.has_curves() {
        return Err(Error::Unsupported("curves in a GeoPackage".into()));
    }
    let mut out = Vec::with_capacity(13 + g.xy.len() * 32);
    out.extend_from_slice(b"GP");
    out.push(0);
    out.push(if g.xy.is_empty() { 0x11 } else { 0x01 });
    out.extend_from_slice(&srs_id.to_le_bytes());
    let dims = match (has_z, has_m) {
        (false, false) => 0,
        (true, false) => 1000,
        (false, true) => 2000,
        (true, true) => 3000,
    };
    let coord = |out: &mut Vec<u8>, i: usize| {
        let (x, y) = (g.xy[i][0], g.xy[i][1]);
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
        if has_z {
            let z = g.z.as_ref().map_or(0.0, |z| z[i]);
            out.extend_from_slice(&z.to_le_bytes());
        }
        if has_m {
            let m = g.m.as_ref().map_or(f64::NAN, |m| m[i]);
            out.extend_from_slice(&m.to_le_bytes());
        }
    };
    let head = |out: &mut Vec<u8>, t: u32| {
        out.push(1);
        out.extend_from_slice(&(t + dims).to_le_bytes());
    };
    let count = |out: &mut Vec<u8>, n: usize| out.extend_from_slice(&(n as u32).to_le_bytes());
    let part = |k: usize| -> (usize, usize) {
        let s = g.parts[k] as usize;
        (s, g.parts.get(k + 1).map_or(g.xy.len(), |&e| e as usize))
    };
    match g.kind {
        GeometryKind::Point => {
            head(&mut out, 1);
            if g.xy.is_empty() {
                for _ in 0..2 + has_z as usize + has_m as usize {
                    out.extend_from_slice(&f64::NAN.to_le_bytes());
                }
            } else {
                coord(&mut out, 0);
            }
        }
        GeometryKind::Multipoint => {
            head(&mut out, 4);
            count(&mut out, g.xy.len());
            for i in 0..g.xy.len() {
                head(&mut out, 1);
                coord(&mut out, i);
            }
        }
        GeometryKind::Polyline => {
            head(&mut out, 5);
            count(&mut out, g.parts.len());
            for k in 0..g.parts.len() {
                let (s, e) = part(k);
                head(&mut out, 2);
                count(&mut out, e - s);
                for i in s..e {
                    coord(&mut out, i);
                }
            }
        }
        GeometryKind::Polygon => {
            // Esri rings: clockwise outer rings, each followed by its holes.
            let mut polygons: Vec<Vec<usize>> = Vec::new();
            for k in 0..g.parts.len() {
                let (s, e) = part(k);
                let area2: f64 = (s..e.saturating_sub(1))
                    .map(|i| g.xy[i][0] * g.xy[i + 1][1] - g.xy[i + 1][0] * g.xy[i][1])
                    .sum();
                if area2 <= 0.0 || polygons.is_empty() {
                    polygons.push(vec![k]);
                } else {
                    polygons.last_mut().unwrap().push(k);
                }
            }
            head(&mut out, 6);
            count(&mut out, polygons.len());
            for rings in polygons {
                head(&mut out, 3);
                count(&mut out, rings.len());
                for k in rings {
                    // OGC order: reversed, keeping the start vertex.
                    let (s, e) = part(k);
                    count(&mut out, e - s);
                    coord(&mut out, s);
                    for i in (s + 1..e - 1).rev() {
                        coord(&mut out, i);
                    }
                    if e - s > 1 {
                        coord(&mut out, e - 1);
                    }
                }
            }
        }
        GeometryKind::MultiPatch => {
            return Err(Error::Unsupported("multipatch in a GeoPackage".into()));
        }
    }
    Ok(out)
}
