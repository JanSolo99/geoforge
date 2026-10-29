//! Sampling coordinates vary; the schema, allocation, membership and spacing do not.
use serde_json::Value as Json;
use std::{collections::BTreeMap, path::Path};
use unigis_core::{FeatureClass, Geometry, GeometryKind, Value};
use unigis_shapefile::ShapefileReader;
fn cid(v: &Value) -> i64 {
    match v {
        Value::Integer(n) => i64::from(*n),
        _ => panic!("invalid CID"),
    }
}
fn key(v: &Value) -> String {
    match v {
        Value::String(s) => format!("s:{s}"),
        Value::Integer(n) => format!("i:{n}"),
        Value::SmallInteger(n) => format!("i:{n}"),
        _ => format!("{v:?}"),
    }
}
pub fn compare(ours: &Path, theirs: &Path, info: &Json, root: &Path) -> Vec<String> {
    let a = ShapefileReader::open(ours).unwrap();
    let b = ShapefileReader::open(theirs).unwrap();
    let mut diffs = Vec::new();
    if a.fields() != b.fields() {
        diffs.push("sampling schema differs".into());
    }
    if a.geometry_kind() != Some(GeometryKind::Point) || a.geometry_kind() != b.geometry_kind() {
        diffs.push("sampling shape type differs".into());
    }
    if a.spatial_reference_wkt() != b.spatial_reference_wkt() {
        diffs.push("sampling CRS differs".into());
    }
    let aa = a.features().collect::<Result<Vec<_>, _>>().unwrap();
    let bb = b.features().collect::<Result<Vec<_>, _>>().unwrap();
    let counts = |rows: &[unigis_core::Feature]| {
        let mut map = BTreeMap::new();
        for row in rows {
            *map.entry(cid(&row.attributes[0])).or_insert(0usize) += 1;
        }
        map
    };
    if counts(&aa) != counts(&bb) {
        diffs.push(format!("allocation {:?} != {:?}", counts(&aa), counts(&bb)));
    }
    let p = &info["run"]["params"];
    let source = ShapefileReader::open(root.join(p["input"].as_str().unwrap())).unwrap();
    let method = p["sampling_method"].as_str().unwrap_or("RANDOM");
    let allocation = p["strata_count_method"].as_str().unwrap_or("EQUAL");
    let proportional = matches!(allocation, "PROP_AREA" | "PROP_FIELD");
    let source_rows = source.features().collect::<Result<Vec<_>, _>>().unwrap();
    let mut groups = Vec::new();
    if method == "RANDOM" {
        groups.push(source_rows.iter().collect::<Vec<_>>());
    } else if method == "STRAT_POLY" {
        groups.extend(source_rows.iter().map(|r| vec![r]));
    } else {
        let i = source
            .fields()
            .iter()
            .position(|f| {
                f.name
                    .eq_ignore_ascii_case(p["strata_id_field"].as_str().unwrap())
            })
            .unwrap();
        let mut by_key: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for row in &source_rows {
            by_key.entry(key(&row.attributes[i])).or_default().push(row);
        }
        groups.extend(by_key.into_values());
    }
    let gap = unigis_tools::registry::distance_in_units(
        p["min_distance"].as_str().unwrap_or("0"),
        source.spatial_reference_wkt(),
    )
    .unwrap();
    let mut points: BTreeMap<i64, Vec<[f64; 2]>> = BTreeMap::new();
    for row in aa {
        let id = cid(&row.attributes[0]);
        let index = if method == "RANDOM" {
            0
        } else {
            usize::try_from(id - i64::from(!proportional)).unwrap()
        };
        let g = row.geometry.unwrap();
        let xy = g.xy[0];
        if !xy.iter().all(|n| n.is_finite()) {
            diffs.push("nonfinite sample".into());
            continue;
        }
        let point = Geometry::point(xy[0], xy[1]);
        if !groups.get(index).is_some_and(|rs| {
            rs.iter().any(|r| {
                r.geometry
                    .as_ref()
                    .is_some_and(|g| unigis_geometry::within(&point, g).unwrap())
            })
        }) {
            diffs.push(format!("sample outside CID {id} stratum"));
        }
        let group = points.entry(id).or_default();
        if group
            .iter()
            .any(|q| (q[0] - xy[0]).hypot(q[1] - xy[1]) + 1e-8 < gap)
        {
            diffs.push("sample spacing violated".into());
        }
        if group.contains(&xy) {
            diffs.push("duplicate sample".into());
        }
        group.push(xy);
    }
    diffs
}
