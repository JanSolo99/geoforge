//! testdata/gpkg/sample.gpkg (written by GDAL) read as ArcGIS Pro 3.7 reads it:
//! table names, ObjectID, field types and lengths, every value, and geometries
//! (ArcGIS's WKT shows outer rings counterclockwise, so ours are written out
//! reversed with their start kept).

use std::path::Path;

use geoforge_core::{FeatureClass, FieldType, Geometry, GeometryKind, Value};
use geoforge_gpkg::GeoPackage;
use serde_json::Value as Json;

fn data(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/gpkg")
        .join(name)
}

fn type_name(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "SmallInteger",
        FieldType::Integer => "Integer",
        FieldType::BigInteger => "BigInteger",
        FieldType::Single => "Single",
        FieldType::Double => "Double",
        FieldType::String => "String",
        FieldType::Date => "Date",
        FieldType::DateOnly => "DateOnly",
        FieldType::Blob => "Blob",
        _ => "other",
    }
}

fn value_json(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::SmallInteger(i) => (*i).into(),
        Value::Integer(i) => (*i).into(),
        Value::BigInteger(i) => (*i).into(),
        Value::Single(f) => (*f as f64).into(),
        Value::Double(f) => (*f).into(),
        Value::String(s) => s.clone().into(),
        Value::DateOnly(d) => d.format("%Y-%m-%d").to_string().into(),
        Value::Date(d) => d.format("%Y-%m-%dT%H:%M:%S%.6f").to_string().into(),
        Value::Blob(b) => b
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>()
            .into(),
        other => panic!("unexpected {other:?}"),
    }
}

/// ArcGIS's SHAPE@WKT for one of our shapes.
fn wkt(g: &Geometry, has_z: bool, has_m: bool) -> String {
    let dims = match (has_z, has_m) {
        (true, true) => " ZM",
        (true, false) => " Z",
        (false, true) => " M",
        _ => "",
    };
    let coord = |i: usize| {
        let mut s = format!("{} {}", g.xy[i][0], g.xy[i][1]);
        if let Some(z) = &g.z {
            s += &format!(" {}", z[i]);
        }
        if let Some(m) = &g.m {
            s += &format!(" {}", m[i]);
        }
        s
    };
    let part = |k: usize| -> (usize, usize) {
        let s = g.parts[k] as usize;
        (s, g.parts.get(k + 1).map_or(g.xy.len(), |&e| e as usize))
    };
    match g.kind {
        GeometryKind::Point => format!("POINT{dims} ({})", coord(0)),
        GeometryKind::Multipoint => format!(
            "MULTIPOINT{dims} ({})",
            (0..g.xy.len())
                .map(|i| format!("({})", coord(i)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        GeometryKind::Polyline => format!(
            "MULTILINESTRING{dims} ({})",
            (0..g.parts.len())
                .map(|k| {
                    let (s, e) = part(k);
                    format!("({})", (s..e).map(coord).collect::<Vec<_>>().join(", "))
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        GeometryKind::Polygon => {
            // Rings reversed (start kept); an outer ring (clockwise here) opens
            // a new polygon.
            let mut polys: Vec<Vec<String>> = Vec::new();
            for k in 0..g.parts.len() {
                let (s, e) = part(k);
                let area2: f64 = (s..e - 1)
                    .map(|i| g.xy[i][0] * g.xy[i + 1][1] - g.xy[i + 1][0] * g.xy[i][1])
                    .sum();
                let order: Vec<usize> = std::iter::once(s)
                    .chain((s + 1..e - 1).rev())
                    .chain([e - 1])
                    .collect();
                let ring = format!(
                    "({})",
                    order.into_iter().map(coord).collect::<Vec<_>>().join(", ")
                );
                if area2 < 0.0 || polys.is_empty() {
                    polys.push(vec![ring]);
                } else {
                    polys.last_mut().unwrap().push(ring);
                }
            }
            format!(
                "MULTIPOLYGON{dims} ({})",
                polys
                    .iter()
                    .map(|p| format!("({})", p.join(", ")))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        GeometryKind::MultiPatch => unreachable!(),
    }
}

/// WKT with numbers replaced by `#`, plus the numbers.
fn split_numbers(s: &str) -> (String, Vec<f64>) {
    let mut skeleton = String::new();
    let mut nums = Vec::new();
    let mut cur = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || c == '.' || c == '-' || (c == 'e' && !cur.is_empty()) {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                nums.push(cur.parse().unwrap());
                skeleton.push('#');
                cur.clear();
            }
            skeleton.push(c);
        }
    }
    (skeleton, nums)
}

#[test]
fn reads_like_arcgis() {
    let arcgis: serde_json::Map<String, Json> =
        serde_json::from_str(&std::fs::read_to_string(data("sample.arcgis.json")).unwrap())
            .unwrap();
    let gpkg = GeoPackage::open(&data("sample.gpkg")).unwrap();
    let names: Vec<String> = gpkg
        .contents()
        .iter()
        .map(|c| format!("main.{}", c.table_name))
        .collect();
    let mut want_names: Vec<&String> = arcgis.keys().collect();
    want_names.sort();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(sorted.iter().collect::<Vec<_>>(), want_names);
    for (name, want) in &arcgis {
        let t = gpkg.open_table(name).unwrap();
        assert_eq!(t.name(), name);
        assert_eq!(Some(t.oid_field_name()), want["oid"].as_str(), "{name}");
        let kind = t.geometry_kind().map(|k| match k {
            GeometryKind::Point => "Point",
            GeometryKind::Multipoint => "Multipoint",
            GeometryKind::Polyline => "Polyline",
            GeometryKind::Polygon => "Polygon",
            GeometryKind::MultiPatch => "MultiPatch",
        });
        assert_eq!(kind, want["shapeType"].as_str(), "{name}");
        if kind.is_some() {
            assert_eq!(Some(t.has_z()), want["hasZ"].as_bool(), "{name}");
            assert_eq!(Some(t.has_m()), want["hasM"].as_bool(), "{name}");
        }
        // Fields after the ObjectID and geometry.
        let want_fields: Vec<&Json> = want["fields"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f[1] != "OID" && f[1] != "Geometry")
            .collect();
        assert_eq!(t.fields().len(), want_fields.len(), "{name}");
        for (f, w) in t.fields().iter().zip(want_fields) {
            assert_eq!(f.name, w[0].as_str().unwrap(), "{name}");
            assert_eq!(
                type_name(f.field_type),
                w[1].as_str().unwrap(),
                "{name}.{}",
                f.name
            );
            if f.field_type == FieldType::String {
                assert_eq!(f.length as u64, w[2].as_u64().unwrap(), "{name}.{}", f.name);
            }
            assert_eq!(f.nullable, w[5].as_bool().unwrap(), "{name}.{}", f.name);
        }
        let features: Vec<_> = t.features().map(Result::unwrap).collect();
        let rows = want["rows"].as_array().unwrap();
        assert_eq!(features.len(), rows.len(), "{name}");
        for (f, row) in features.iter().zip(rows) {
            let row = row.as_array().unwrap();
            assert_eq!(Some(f.oid), row[0].as_i64(), "{name}");
            for (k, v) in f.attributes.iter().enumerate() {
                let (ours, theirs) = (value_json(v), &row[k + 1]);
                let same = match (ours.as_f64(), theirs.as_f64()) {
                    (Some(a), Some(b)) => (a - b).abs() <= 1e-12 * b.abs().max(1.0),
                    _ => &ours == theirs,
                };
                assert!(same, "{name} oid {} field {k}: {ours} vs {theirs}", f.oid);
            }
            if kind.is_some() {
                let theirs = row.last().unwrap();
                // ArcGIS's WKT of an empty shape is None, like a null one.
                let geometry = f.geometry.as_ref().filter(|g| !g.xy.is_empty());
                match (geometry, theirs.as_str()) {
                    (None, None) => {}
                    (Some(g), Some(w)) => {
                        let ours = wkt(g, t.has_z(), t.has_m());
                        let ((s1, n1), (s2, n2)) = (split_numbers(&ours), split_numbers(w));
                        assert_eq!(s1, s2, "{name} oid {}: {ours} vs {w}", f.oid);
                        for (a, b) in n1.iter().zip(&n2) {
                            assert!((a - b).abs() <= 1e-9, "{name} oid {}: {ours} vs {w}", f.oid);
                        }
                    }
                    (g, w) => panic!("{name} oid {}: {g:?} vs {w:?}", f.oid),
                }
            }
        }
    }
}
