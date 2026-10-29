//! Random coordinates vary even across ArcGIS repetitions. Compare the public
//! schema/count contract, then independently verify membership and spacing.
use serde_json::Value as Json;
use std::{collections::BTreeMap, path::Path};
use unigis_core::{FeatureClass, Geometry, GeometryKind, Value};
use unigis_shapefile::ShapefileReader;
fn cid(v: &Value) -> i64 {
    match v {
        Value::Integer(v) => i64::from(*v),
        Value::SmallInteger(v) => i64::from(*v),
        _ => panic!("invalid CID"),
    }
}
fn points(fc: &ShapefileReader) -> Vec<(i64, [f64; 2])> {
    fc.features()
        .flat_map(|f| {
            let f = f.unwrap();
            let id = cid(&f.attributes[0]);
            f.geometry.unwrap().xy.into_iter().map(move |p| (id, p))
        })
        .collect()
}
pub fn compare(ours: &Path, theirs: &Path, info: &Json, root: &Path) -> Vec<String> {
    let a = ShapefileReader::open(ours).unwrap();
    let b = ShapefileReader::open(theirs).unwrap();
    let mut diffs = Vec::new();
    if a.fields() != b.fields() {
        diffs.push(format!("fields {:?} != {:?}", a.fields(), b.fields()));
    }
    if a.geometry_kind() != b.geometry_kind() {
        diffs.push("shape type differs".into());
    }
    if a.spatial_reference_wkt() != b.spatial_reference_wkt() {
        diffs.push("CRS differs".into());
    }
    let ap = points(&a);
    let bp = points(&b);
    if ap.len() != bp.len() {
        diffs.push(format!("point count {} != {}", ap.len(), bp.len()));
    }
    let chunk_sizes = |fc: &ShapefileReader| {
        fc.features()
            .map(|r| r.unwrap().geometry.unwrap().xy.len())
            .collect::<Vec<_>>()
    };
    if chunk_sizes(&a) != chunk_sizes(&b) {
        diffs.push("multipoint chunk sizes differ".into());
    }
    let params = &info["run"]["params"];
    let multi = params["create_multipoint_output"] == "MULTIPOINT";
    let source = params["constraining_feature_class"]
        .as_str()
        .map(|p| ShapefileReader::open(root.join(p)).unwrap());
    let point_input = source.as_ref().is_some_and(|s| {
        matches!(
            s.geometry_kind(),
            Some(GeometryKind::Point | GeometryKind::Multipoint)
        )
    });
    if !point_input {
        let counts = |ps: &[(i64, [f64; 2])]| {
            let mut map = BTreeMap::new();
            for (id, _) in ps {
                *map.entry(*id).or_insert(0usize) += 1;
            }
            map
        };
        if counts(&ap) != counts(&bp) {
            diffs.push("counts per CID differ".into());
        }
    }
    let source_rows = source
        .as_ref()
        .map(|s| s.features().collect::<Result<Vec<_>, _>>().unwrap());
    let mut grouped: BTreeMap<i64, Vec<[f64; 2]>> = BTreeMap::new();
    for (id, p) in &ap {
        if p.iter().any(|v| !v.is_finite()) {
            diffs.push("nonfinite point".into());
            continue;
        }
        let point = Geometry::point(p[0], p[1]);
        let mut group = *id;
        let mut distance = 0.0;
        if let (Some(source), Some(rows)) = (&source, &source_rows) {
            let candidates = rows
                .iter()
                .filter(|r| multi || r.oid == *id)
                .filter_map(|r| r.geometry.as_ref().map(|g| (r, g)));
            let mut matches = Vec::new();
            for (r, g) in candidates {
                // Polyline storage may snap in the final format; use the public XY tolerance.
                let inside = if g.kind == GeometryKind::Point || g.kind == GeometryKind::Multipoint
                {
                    g.xy.iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-8)
                } else {
                    unigis_geometry::within(&point, g).unwrap()
                };
                if inside {
                    matches.push(r);
                }
            }
            if let Some(r) = matches.first() {
                group = r.oid;
                let text = params["minimum_allowed_distance"].as_str().unwrap_or("0");
                distance = if let Some(i) = source
                    .fields()
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(text))
                {
                    match r.attributes[i] {
                        Value::Double(v) => v,
                        Value::Integer(v) => f64::from(v),
                        _ => 0.0,
                    }
                } else {
                    unigis_tools::registry::distance_in_units(text, source.spatial_reference_wkt())
                        .unwrap()
                };
            } else {
                diffs.push(format!("point {p:?} outside its CID constraint {id}"));
            }
        } else {
            let extent = params["constraining_extent"]
                .as_str()
                .unwrap_or("0 0 250 250");
            let values = extent
                .split_whitespace()
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>();
            let bounds = if let Ok(v) = values {
                v
            } else {
                let fc = ShapefileReader::open(root.join(extent)).unwrap();
                let e = fc.stored_extent().unwrap();
                vec![e.xmin, e.ymin, e.xmax, e.ymax]
            };
            if p[0] < bounds[0] || p[0] > bounds[2] || p[1] < bounds[1] || p[1] > bounds[3] {
                diffs.push("point outside constraining extent".into());
            }
            if *id != 0 {
                diffs.push("extent Id must be zero".into());
            }
        }
        let previous = grouped.entry(group).or_default();
        if previous
            .iter()
            .any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) + 1e-8 < distance)
        {
            diffs.push("minimum spacing violated".into());
        }
        if previous.contains(p) {
            diffs.push("duplicate random point".into());
        }
        previous.push(*p);
    }
    if multi && let Some(rows) = &source_rows {
        let mut expected_groups = BTreeMap::new();
        for (_, p) in &bp {
            let point = Geometry::point(p[0], p[1]);
            let row = rows
                .iter()
                .find(|r| {
                    r.geometry
                        .as_ref()
                        .is_some_and(|g| unigis_geometry::within(&point, g).unwrap())
                })
                .unwrap();
            *expected_groups.entry(row.oid).or_insert(0usize) += 1;
        }
        let actual_groups: BTreeMap<_, _> =
            grouped.iter().map(|(id, ps)| (*id, ps.len())).collect();
        if actual_groups != expected_groups {
            diffs.push("multipoint counts per constraining feature differ".into());
        }
    }
    diffs
}
