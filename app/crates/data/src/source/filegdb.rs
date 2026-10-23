//! File Geodatabases (`.gdb` folders) read in place through UniGIS's `geoforge-filegdb`.
//!
//! Modified for UniGIS (not part of upstream RustGIS).
//!
//! `open` picks a feature class or table by name (default: the first feature class); a read asks
//! the table's spatial index (`.spx`) for the rows near the extent, then reads just those rows.
//! Curves are densified, as the map draws straight segments. MultiPatch classes are not drawn.
//!
//! The files are hostile input: a row that doesn't decode is skipped and counted.

use std::path::Path;

use geoforge_core::{Envelope, GeometryKind, Value as GfValue};
use geoforge_filegdb::{Geodatabase, Table};

pub use super::gfshape::{from_geoforge, to_geoforge};
use super::{Batch, FeatureSource, Reproject, Request, SourceInfo, clamp_limit, stride};
use crate::{DataError, Dataset, Feature, Field, FieldType, Geometry, GeometryType, Result, Value};
use rustgis_geom::Extent;

fn bad(msg: impl std::fmt::Display) -> DataError {
    DataError::Format(format!("File Geodatabase: {msg}"))
}

/// A feature class or table of a File Geodatabase, read in place.
pub struct FileGdbSource {
    info: SourceInfo,
    table: Table,
    reproject: Reproject,
}

impl FileGdbSource {
    /// Opens `path` (the `.gdb` folder); `layer` names a table (default: the first feature class,
    /// else the first table).
    pub fn open(path: &Path, layer: Option<&str>) -> Result<FileGdbSource> {
        let gdb = Geodatabase::open(path).map_err(bad)?;
        let name = match layer {
            Some(n) => n.to_string(),
            None => {
                let mut first_table = None;
                let mut found = None;
                for e in gdb.entries() {
                    let t = gdb.open_table(&e.name).map_err(bad)?;
                    if t.geometry_kind().is_some() {
                        found = Some(e.name.clone());
                        break;
                    }
                    first_table.get_or_insert_with(|| e.name.clone());
                }
                found.or(first_table).ok_or_else(|| bad("the geodatabase holds no tables"))?
            }
        };
        let table = gdb.open_table(&name).map_err(bad)?;
        let name = gdb.entries().iter().find(|e| e.name.eq_ignore_ascii_case(&name)).map_or(name, |e| e.name.clone());
        let mut notes = Vec::new();
        let geometry_type = match table.geometry_kind() {
            None => GeometryType::None,
            Some(GeometryKind::Point | GeometryKind::Multipoint) => GeometryType::Point,
            Some(GeometryKind::Polyline) => GeometryType::Polyline,
            Some(GeometryKind::Polygon) => GeometryType::Polygon,
            Some(GeometryKind::MultiPatch) => return Err(bad("MultiPatch feature classes are not drawn")),
        };
        let def = table.geometry_def();
        let crs = match def.and_then(|d| d.srs_wkt.as_deref()) {
            None => crate::raster::Crs::LonLat,
            Some(wkt) => match crate::raster::wkt::read_wkt(wkt) {
                Ok((c, note)) => {
                    notes.extend(note.map(|n| format!("the coordinate system: {n}")));
                    c
                }
                Err(e) => {
                    notes.push(format!("the coordinate system: {e}; coordinates are read as longitude/latitude"));
                    crate::raster::Crs::LonLat
                }
            },
        };
        let reproject = Reproject::from_crs(crs, &mut notes);
        let extent = def
            .map(|d| Extent::new(d.extent.xmin, d.extent.ymin, d.extent.xmax, d.extent.ymax))
            .filter(|e| e.min_x.is_finite() && e.max_x.is_finite() && e.min_x <= e.max_x && e.min_y <= e.max_y)
            .map_or(Extent::EMPTY, |e| reproject.extent_to_lonlat(&e));
        let fields = table.fields().iter().map(|f| Field::new(&f.name, field_type(f.field_type))).collect();
        let indexed = def.is_some_and(|d| d.grid_sizes.iter().any(|&g| g > 0.0)) && path_has_spx(path, &gdb, &name);
        let info = SourceInfo {
            name,
            format: "File Geodatabase",
            geometry_type,
            fields,
            extent,
            count: Some(table.row_count()),
            crs: reproject.label(),
            indexed,
            notes,
        };
        Ok(FileGdbSource { info, table, reproject })
    }

    /// The OBJECTIDs a request has to look at, and whether they came from the spatial index (else
    /// every row).
    fn candidates(&self, env: Option<&Envelope>) -> (Vec<u64>, bool) {
        if let Some(env) = env
            && let Ok(Some(oids)) = self.table.spatial_index_candidates(env)
        {
            return (oids, true);
        }
        ((1..=self.table.max_oid()).collect(), false)
    }
}

fn path_has_spx(_path: &Path, gdb: &Geodatabase, name: &str) -> bool {
    gdb.table_base(name).is_ok_and(|b| b.with_extension("spx").is_file())
}

fn field_type(t: geoforge_core::FieldType) -> FieldType {
    use geoforge_core::FieldType as G;
    match t {
        G::SmallInteger | G::Integer | G::BigInteger => FieldType::Integer,
        G::Single | G::Double => FieldType::Double,
        _ => FieldType::Text,
    }
}

fn value(v: GfValue) -> Value {
    match v {
        GfValue::Null => Value::Null,
        GfValue::SmallInteger(i) => Value::Int(i64::from(i)),
        GfValue::Integer(i) => Value::Int(i64::from(i)),
        GfValue::BigInteger(i) => Value::Int(i),
        GfValue::Single(f) => Value::Float(f64::from(f)),
        GfValue::Double(f) => Value::Float(f),
        GfValue::String(s) | GfValue::Guid(s) | GfValue::Xml(s) => Value::Text(s),
        GfValue::Date(d) => Value::Text(d.to_string()),
        GfValue::DateOnly(d) => Value::Text(d.to_string()),
        GfValue::TimeOnly(t) => Value::Text(t.to_string()),
        GfValue::TimestampOffset(t) => Value::Text(t.to_rfc3339()),
        GfValue::Blob(b) => Value::Text(format!("<{} bytes>", b.len())),
    }
}

impl FeatureSource for FileGdbSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }

    fn read(&self, req: &Request) -> Result<Batch> {
        let limit = clamp_limit(req.limit);
        let native = req.extent.map(|e| self.reproject.extent_from_lonlat(&e));
        // An empty native extent (outside the projection's domain) matches nothing.
        if native.is_some_and(|e| e.is_empty()) {
            return Ok(Batch { complete: true, ..Batch::default() });
        }
        let env = native.map(|e| Envelope { xmin: e.min_x, ymin: e.min_y, xmax: e.max_x, ymax: e.max_y, z: None, m: None });
        let (mut oids, from_index) = self.candidates(env.as_ref());
        let mut skipped = 0;
        // Without a spatial index, or with an extent, the row's own envelope decides.
        let meets = |f: &geoforge_core::Feature| match (&env, &f.geometry) {
            (None, _) => true,
            (Some(e), Some(g)) => {
                let b = g.envelope();
                !b.is_empty() && b.xmin <= e.xmax && b.xmax >= e.xmin && b.ymin <= e.ymax && b.ymax >= e.ymin
            }
            (Some(_), None) => false,
        };
        let mut rows: Vec<geoforge_core::Feature> = Vec::new();
        let matched;
        let complete;
        if env.is_none() || from_index {
            // Candidates known up front: sample them, then read.
            oids.sort_unstable();
            oids.dedup();
            let step = stride(oids.len() as u64, limit).max(1) as usize;
            for oid in oids.iter().copied().step_by(step).take(limit) {
                match self.table.get(oid) {
                    Ok(Some(f)) if meets(&f) => rows.push(f),
                    Ok(_) => {}
                    Err(_) => skipped += 1,
                }
            }
            matched = oids.len() as u64;
            complete = step == 1;
        } else {
            // No usable index: scan, keep what meets the extent, then sample.
            for f in self.table.features() {
                match f {
                    Ok(f) if meets(&f) => rows.push(f),
                    Ok(_) => {}
                    Err(_) => skipped += 1,
                }
            }
            matched = rows.len() as u64;
            let step = stride(matched, limit).max(1) as usize;
            complete = step == 1;
            rows = rows.into_iter().step_by(step).take(limit).collect();
        }
        let has_geometry = self.info.geometry_type != GeometryType::None;
        let mut features = Vec::with_capacity(rows.len());
        for f in rows {
            let g = match (&f.geometry, has_geometry) {
                (Some(g), true) => match from_geoforge(g).and_then(|g| self.reproject.geometry(g)) {
                    Some(g) => Some(g),
                    None => {
                        skipped += 1;
                        continue;
                    }
                },
                _ => None,
            };
            features.push(Feature { id: f.oid as u64, geometry: g, attrs: f.attributes.into_iter().map(value).collect() });
        }
        Ok(Batch { features, matched, complete, skipped })
    }
}

/// The sanitised name of a table or field for the geodatabase: letters, digits and `_`, starting
/// with a letter.
fn gdb_name(raw: &str, max: usize) -> String {
    let mut n: String = raw.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if !n.chars().next().is_some_and(char::is_alphabetic) {
        n.insert(0, 'F');
    }
    n.chars().take(max).collect()
}

/// The feature classes of a File Geodatabase (MultiPatch ones are left out), by name.
pub fn layers(gdb: &Path) -> Result<Vec<String>> {
    let g = Geodatabase::open(gdb).map_err(bad)?;
    let mut out = Vec::new();
    for e in g.entries() {
        let t = g.open_table(&e.name).map_err(bad)?;
        if t.geometry_kind().is_some_and(|k| k != GeometryKind::MultiPatch) {
            out.push(e.name.clone());
        }
    }
    out.sort_by_key(|n| n.to_ascii_lowercase());
    Ok(out)
}

/// Writes `ds` as a feature class (or table) named `name` (default: the layer's name) in the
/// File Geodatabase `gdb`, creating the geodatabase if it doesn't exist and replacing a class of
/// that name. Coordinates are WGS 84, as the map holds them. Returns the number of features.
pub fn write(ds: &Dataset, gdb: &Path, name: Option<&str>) -> Result<usize> {
    use geoforge_core::{Feature as GfFeature, Field as GfField, FieldType as GfType, Value as GfValue};
    use geoforge_filegdb::GeodatabaseWriter;
    let mut table = gdb_name(name.unwrap_or(&ds.name), 160);
    if table.to_ascii_uppercase().starts_with("GDB_") {
        table.insert(0, 'T');
    }
    let exists = gdb.join("a00000001.gdbtable").is_file();
    if gdb.exists() && !exists && std::fs::read_dir(gdb).map_or(true, |mut d| d.next().is_some()) {
        return Err(bad(format!("{} exists and is not a geodatabase", gdb.display())));
    }
    if exists {
        // Replacing a class of the same name, as every other writer replaces its file.
        if Geodatabase::open(gdb).map_err(bad)?.entries().iter().any(|e| e.name.eq_ignore_ascii_case(&table)) {
            geoforge_filegdb::delete_item(gdb, &table).map_err(bad)?;
        }
    }
    // Fields: names the geodatabase accepts, unique, and a type that holds every value.
    let mut fields: Vec<GfField> = Vec::new();
    let mut types: Vec<GfType> = Vec::new();
    for (i, f) in ds.fields.iter().enumerate() {
        let mut n = gdb_name(&f.name, 64);
        if ["OBJECTID", "SHAPE"].iter().any(|r| n.eq_ignore_ascii_case(r)) {
            n.push('_');
        }
        let base = n.clone();
        let mut k = 1;
        while fields.iter().any(|g| g.name.eq_ignore_ascii_case(&n)) {
            let suffix = format!("_{k}");
            n = format!("{}{suffix}", base.chars().take(64 - suffix.len()).collect::<String>());
            k += 1;
        }
        let column = || ds.features.iter().filter_map(move |ft| ft.attrs.get(i));
        let ty = match f.ty {
            FieldType::Integer if column().all(|v| !matches!(v, Value::Int(x) if i32::try_from(*x).is_err())) => GfType::Integer,
            FieldType::Integer | FieldType::Double => GfType::Double,
            FieldType::Boolean => GfType::SmallInteger,
            FieldType::Text => GfType::String,
        };
        let mut g = GfField::new(n, ty);
        g.alias = f.alias.clone().or_else(|| (g.name != f.name).then(|| f.name.clone()));
        if ty == GfType::String {
            let longest = column().map(|v| if let Value::Text(s) = v { s.chars().count() } else { 0 }).max().unwrap_or(0);
            g.length = longest.max(255) as u32;
        }
        types.push(ty);
        fields.push(g);
    }
    let kind = match ds.geometry_type {
        GeometryType::None => None,
        GeometryType::Point if ds.features.iter().any(|f| matches!(f.geometry, Some(Geometry::MultiPoint(_)))) => {
            Some(GeometryKind::Multipoint)
        }
        GeometryType::Point => Some(GeometryKind::Point),
        GeometryType::Polyline => Some(GeometryKind::Polyline),
        GeometryType::Polygon => Some(GeometryKind::Polygon),
    };
    const WGS84: &str = "GEOGCS[\"GCS_WGS_1984\",DATUM[\"D_WGS_1984\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]]";
    let mut w = if exists { GeodatabaseWriter::open_existing(gdb) } else { GeodatabaseWriter::create(gdb) }.map_err(bad)?;
    let mut t = w.create_table(&table, kind, false, false, &fields, kind.map(|_| WGS84)).map_err(bad)?;
    for f in &ds.features {
        let attributes = f
            .attrs
            .iter()
            .zip(&types)
            .map(|(v, ty)| match (v, ty) {
                (Value::Null, _) => GfValue::Null,
                (Value::Int(i), GfType::Integer) => GfValue::Integer(*i as i32),
                (Value::Int(i), _) => GfValue::Double(*i as f64),
                (Value::Float(x), _) => GfValue::Double(*x),
                (Value::Text(s), _) => GfValue::String(s.clone()),
                (Value::Bool(b), _) => GfValue::SmallInteger(i16::from(*b)),
            })
            .collect();
        let mut geometry = f.geometry.as_ref().and_then(to_geoforge);
        // A class of multipoints holds its single points as one-point multipoints.
        if let Some(g) = geometry.as_mut().filter(|_| kind == Some(GeometryKind::Multipoint)) {
            g.kind = GeometryKind::Multipoint;
        }
        let geometry = geometry.filter(|g| Some(g.kind) == kind);
        t.write(&GfFeature { oid: 0, geometry, attributes }).map_err(bad)?;
    }
    w.finish_table(t).map_err(bad)?;
    w.finish().map_err(bad)?;
    Ok(ds.features.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_geom::Coord;

    fn reference() -> Option<std::path::PathBuf> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../testdata/gdb-reference/buffers.gdb");
        p.is_dir().then_some(p)
    }

    #[test]
    fn reads_a_feature_class_in_place() {
        let Some(p) = reference() else { return };
        let s = FileGdbSource::open(&p, Some("parcels_10")).unwrap();
        assert_eq!(s.info().geometry_type, GeometryType::Polygon);
        assert_eq!(s.info().format, "File Geodatabase");
        let all = s.read(&Request::all()).unwrap();
        assert!(all.complete);
        assert_eq!(all.features.len() as u64, s.info().count.unwrap());
        assert_eq!(all.skipped, 0);
        assert!(all.features.iter().all(|f| matches!(f.geometry, Some(Geometry::Polygon(_)))));
        assert!(all.features.iter().all(|f| f.attrs.len() == s.info().fields.len()));
        // The extent filter keeps the features meeting it and drops the rest.
        let e = s.info().extent;
        let corner = Extent::new(e.min_x, e.min_y, e.min_x + e.width() * 0.2, e.min_y + e.height() * 0.2);
        let part = s.read(&Request { extent: Some(corner), limit: 1000 }).unwrap();
        assert!(part.features.len() < all.features.len());
        for f in &part.features {
            assert!(f.geometry.as_ref().unwrap().extent().intersects(&corner));
        }
    }

    #[test]
    fn default_layer_is_the_first_feature_class() {
        let Some(p) = reference() else { return };
        let s = open_default(&p);
        assert_ne!(s.info().geometry_type, GeometryType::None);
    }

    fn open_default(p: &Path) -> FileGdbSource {
        FileGdbSource::open(p, None).unwrap()
    }

    fn parcels(name: &str, n: usize) -> Dataset {
        let mut d = Dataset::new(
            name,
            GeometryType::Polygon,
            vec![
                Field::new("Lot No", FieldType::Integer),
                Field::new("owner", FieldType::Text),
                Field::new("area", FieldType::Double),
                Field::new("Shape", FieldType::Text),
            ],
        );
        for i in 0..n {
            let (x, y) = (10.0 + i as f64 * 0.01, 50.0);
            let ring = vec![
                Coord::new(x, y),
                Coord::new(x, y + 0.005),
                Coord::new(x + 0.005, y + 0.005),
                Coord::new(x + 0.005, y),
                Coord::new(x, y),
            ];
            let hole = vec![
                Coord::new(x + 0.001, y + 0.001),
                Coord::new(x + 0.002, y + 0.001),
                Coord::new(x + 0.002, y + 0.002),
                Coord::new(x + 0.001, y + 0.002),
                Coord::new(x + 0.001, y + 0.001),
            ];
            let attrs = vec![
                Value::Int(i as i64 * 3_000_000_000),
                Value::Text(format!("Owner \u{e9} {i}")),
                Value::Float(i as f64 / 7.0),
                if i == 0 { Value::Null } else { Value::Text("s".into()) },
            ];
            d.push(Some(Geometry::Polygon(vec![vec![ring, hole]])), attrs);
        }
        d.push(None, vec![Value::Null, Value::Null, Value::Null, Value::Null]);
        d
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rustgis-gdbw-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn written_layers_read_back_and_gdal_agrees() {
        let gdb = tmp("rt").join("out.gdb");
        let ds = parcels("My Parcels", 40);
        write(&ds, &gdb, None).unwrap();
        // Name and field names were made valid; a field too wide for 32 bits became a double.
        let s = FileGdbSource::open(&gdb, None).unwrap();
        assert_eq!(s.info().name, "My_Parcels");
        let names: Vec<&str> = s.info().fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Lot_No", "owner", "area", "Shape_"]);
        assert_eq!(s.info().fields[0].ty, FieldType::Double);
        let all = s.read(&Request::all()).unwrap();
        assert_eq!(all.features.len(), 41);
        let f = &all.features[3];
        assert_eq!(f.attrs[1], Value::Text("Owner \u{e9} 3".into()));
        assert!(matches!(f.attrs[2], Value::Float(x) if (x - 3.0 / 7.0).abs() < 1e-12));
        let Some(Geometry::Polygon(p)) = &f.geometry else { panic!("polygon") };
        assert_eq!((p.len(), p[0].len()), (1, 2), "one polygon with a hole");
        assert!(all.features[40].geometry.is_none());
        // GDAL reads what we wrote.
        if let Ok(out) = std::process::Command::new("ogrinfo").args(["-so", "-al"]).arg(&gdb).output()
            && out.status.success()
        {
            let t = String::from_utf8_lossy(&out.stdout);
            assert!(t.contains("Layer name: My_Parcels") && t.contains("Feature Count: 41") && t.contains("Lot_No: Real"), "{t}");
        }
        // Writing again adds a class; the same name replaces it.
        write(&parcels("Other", 5), &gdb, None).unwrap();
        write(&parcels("My Parcels", 7), &gdb, None).unwrap();
        let again = FileGdbSource::open(&gdb, Some("My_Parcels")).unwrap();
        assert_eq!(again.read(&Request::all()).unwrap().features.len(), 8);
        assert_eq!(FileGdbSource::open(&gdb, Some("Other")).unwrap().info().count, Some(6));
        let _ = std::fs::remove_dir_all(gdb.parent().unwrap());
    }

    #[test]
    fn paths_name_the_class_and_foreign_folders_are_left_alone() {
        let dir = tmp("path");
        std::fs::create_dir_all(&dir).unwrap();
        crate::io::write_path(&parcels("x", 2), &dir.join("g.gdb/roads")).unwrap();
        assert_eq!(FileGdbSource::open(&dir.join("g.gdb"), None).unwrap().info().name, "roads");
        std::fs::create_dir_all(dir.join("notes.gdb")).unwrap();
        std::fs::write(dir.join("notes.gdb/keep.txt"), "x").unwrap();
        assert!(write(&parcels("x", 2), &dir.join("notes.gdb"), None).is_err());
        assert!(dir.join("notes.gdb/keep.txt").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tables_and_points_are_written() {
        let gdb = tmp("tp").join("t.gdb");
        let mut t =
            Dataset::new("people", GeometryType::None, vec![Field::new("n", FieldType::Integer), Field::new("ok", FieldType::Boolean)]);
        t.push(None, vec![Value::Int(7), Value::Bool(true)]);
        write(&t, &gdb, None).unwrap();
        let mut p = Dataset::new("pts", GeometryType::Point, vec![]);
        p.push(Some(Geometry::Point(Coord::new(1.5, 2.5))), vec![]);
        p.push(Some(Geometry::MultiPoint(vec![Coord::new(3.0, 4.0), Coord::new(5.0, 6.0)])), vec![]);
        write(&p, &gdb, None).unwrap();
        let pts = FileGdbSource::open(&gdb, Some("pts")).unwrap().read(&Request::all()).unwrap();
        assert_eq!(pts.features.len(), 2);
        assert!(matches!(&pts.features[1].geometry, Some(Geometry::MultiPoint(v)) if v.len() == 2));
        let people = FileGdbSource::open(&gdb, Some("people")).unwrap();
        assert_eq!(people.info().geometry_type, GeometryType::None);
        let rows = people.read(&Request::all()).unwrap();
        assert_eq!(rows.features[0].attrs, vec![Value::Int(7), Value::Int(1)]);
        let _ = std::fs::remove_dir_all(gdb.parent().unwrap());
    }

    #[test]
    fn missing_layer_is_an_error_not_a_panic() {
        let Some(p) = reference() else { return };
        assert!(FileGdbSource::open(&p, Some("no_such_layer")).is_err());
        assert!(FileGdbSource::open(Path::new("/nonexistent/x.gdb"), None).is_err());
    }
}
