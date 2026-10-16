//! File Geodatabases (`.gdb` folders) read in place through GeoForge's `geoforge-filegdb`.
//!
//! Modified for GeoForge (not part of upstream RustGIS).
//!
//! `open` picks a feature class or table by name (default: the first feature class); a read asks
//! the table's spatial index (`.spx`) for the rows near the extent, then reads just those rows.
//! Curves are densified, as the map draws straight segments. MultiPatch classes are not drawn.
//!
//! The files are hostile input: a row that doesn't decode is skipped and counted.

use std::path::Path;

use geoforge_core::{Envelope, Geometry as GfGeometry, GeometryKind, Value as GfValue};
use geoforge_filegdb::{Geodatabase, Table};

use super::{Batch, FeatureSource, Reproject, Request, SourceInfo, clamp_limit, stride};
use crate::io::group_rings;
use crate::{DataError, Feature, Field, FieldType, Geometry, GeometryType, Result, Value};
use rustgis_geom::{Coord, Extent};

fn bad(msg: impl std::fmt::Display) -> DataError {
    DataError::Format(format!("File Geodatabase: {msg}"))
}

/// Largest angle between densified points of an arc, in radians.
const ARC_STEP: f64 = 0.05;

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

/// The map's geometry model from a decoded shape; `None` for MultiPatch and for a shape with no
/// vertices.
fn geometry(g: &GfGeometry) -> Option<Geometry> {
    let dense;
    let g = if g.has_curves() {
        dense = g.densify_curves(ARC_STEP).ok()?;
        &dense
    } else {
        g
    };
    let coords = |range: std::ops::Range<usize>| -> Vec<Coord> { g.xy[range].iter().map(|&[x, y]| Coord::new(x, y)).collect() };
    let starts = |n: usize| -> Vec<std::ops::Range<usize>> {
        let mut at: Vec<usize> = g.parts.iter().map(|&p| p as usize).collect();
        at.push(n);
        at.windows(2).filter(|w| w[0] <= w[1] && w[1] <= n).map(|w| w[0]..w[1]).collect()
    };
    let n = g.xy.len();
    match g.kind {
        GeometryKind::Point => (n > 0).then(|| Geometry::Point(coords(0..1)[0])),
        GeometryKind::Multipoint => (n > 0).then(|| Geometry::MultiPoint(coords(0..n))),
        GeometryKind::Polyline => {
            let parts: Vec<Vec<Coord>> = starts(n).into_iter().map(coords).filter(|p| p.len() >= 2).collect();
            (!parts.is_empty()).then_some(Geometry::Line(parts))
        }
        GeometryKind::Polygon => {
            // Esri's outer rings run clockwise, holes counter-clockwise.
            let rings: Vec<(Vec<Coord>, bool)> = starts(n)
                .into_iter()
                .map(coords)
                .filter(|r| r.len() >= 3)
                .map(|r| {
                    let outer = signed_area(&r) <= 0.0;
                    (r, outer)
                })
                .collect();
            (!rings.is_empty()).then(|| group_rings(rings))
        }
        GeometryKind::MultiPatch => None,
    }
}

/// Twice the signed area of a ring (positive counter-clockwise).
fn signed_area(r: &[Coord]) -> f64 {
    let mut s = 0.0;
    for (i, a) in r.iter().enumerate() {
        let b = r[(i + 1) % r.len()];
        s += a.x * b.y - b.x * a.y;
    }
    s
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
                (Some(g), true) => match geometry(g).and_then(|g| self.reproject.geometry(g)) {
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn missing_layer_is_an_error_not_a_panic() {
        let Some(p) = reference() else { return };
        assert!(FileGdbSource::open(&p, Some("no_such_layer")).is_err());
        assert!(FileGdbSource::open(Path::new("/nonexistent/x.gdb"), None).is_err());
    }
}
