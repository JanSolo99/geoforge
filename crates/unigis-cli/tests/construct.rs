//! Densify, Generate Points Along Lines, Create Fishnet and Polygon To Line (run through `ug run`)
//! against ArcGIS Pro 3.7 (testdata/arcgis-reference/construct.json).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as Json;
use unigis_core::Value;
use unigis_shapefile::ShapefileReader;

/// A shapefile of an ArcGIS input: its geometry exactly as stored (ring order and start), an `Id`
/// field of 0 and the other fields.
fn shapefile(dir: &Path, name: &str, input: &Json) -> PathBuf {
    let names: Vec<&str> = input["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f[0].as_str().unwrap())
        .filter(|n| *n != "FID" && *n != "Shape")
        .collect();
    let features: Vec<String> = input["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let props: Vec<String> = names
                .iter()
                .enumerate()
                .map(|(i, n)| format!("\"{n}\":{}", r["attrs"][i + 1]))
                .collect();
            let g = &r["geom"];
            let geometry = if let Some(rings) = g["rings"].as_array() {
                format!(
                    r#"{{"type":"Polygon","coordinates":{}}}"#,
                    serde_json::to_string(rings).unwrap()
                )
            } else {
                let paths = g["paths"].as_array().unwrap();
                if paths.len() == 1 {
                    format!(
                        r#"{{"type":"LineString","coordinates":{}}}"#,
                        serde_json::to_string(&paths[0]).unwrap()
                    )
                } else {
                    format!(
                        r#"{{"type":"MultiLineString","coordinates":{}}}"#,
                        serde_json::to_string(paths).unwrap()
                    )
                }
            };
            format!(
                r#"{{"type":"Feature","properties":{{{}}},"geometry":{geometry}}}"#,
                props.join(",")
            )
        })
        .collect();
    let json = dir.join(format!("{name}.geojson"));
    std::fs::write(
        &json,
        format!(
            r#"{{"type":"FeatureCollection","features":[{}]}}"#,
            features.join(",")
        ),
    )
    .unwrap();
    let shp = dir.join(format!("{name}.shp"));
    let r = Command::new("ogr2ogr")
        .args([
            "-f",
            "ESRI Shapefile",
            "-a_srs",
            "EPSG:32632",
            "-lco",
            "ENCODING=UTF-8",
        ])
        .arg(&shp)
        .arg(&json)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    shp
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(v) => Some(*v as f64),
        Value::SmallInteger(v) => Some(*v as f64),
        Value::BigInteger(v) => Some(*v as f64),
        Value::Double(v) => Some(*v),
        Value::Single(v) => Some(*v as f64),
        _ => None,
    }
}

fn parts_of(g: &Json) -> Vec<Vec<[f64; 2]>> {
    let rings = g["rings"].as_array().or_else(|| g["paths"].as_array());
    match rings {
        Some(rs) => rs
            .iter()
            .map(|r| {
                r.as_array()
                    .unwrap()
                    .iter()
                    .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
                    .collect()
            })
            .collect(),
        None => vec![vec![[g["x"].as_f64().unwrap(), g["y"].as_f64().unwrap()]]],
    }
}

fn compare(key: &str, out: &Path, want: &Json) -> Vec<String> {
    let mut failures = Vec::new();
    let r = ShapefileReader::open(out).unwrap();
    let ours: Vec<_> = r.features().map(|f| f.unwrap()).collect();
    let rows = want["rows"].as_array().unwrap();
    if ours.len() != rows.len() {
        return vec![format!(
            "{key}: {} features but ArcGIS {}",
            ours.len(),
            rows.len()
        )];
    }
    let want_fields: Vec<&str> = want["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f[0].as_str().unwrap())
        .filter(|n| *n != "FID" && *n != "Shape")
        .collect();
    let our_fields: Vec<&str> = r.fields().iter().map(|f| f.name.as_str()).collect();
    if our_fields != want_fields {
        return vec![format!(
            "{key}: fields {our_fields:?} but ArcGIS {want_fields:?}"
        )];
    }
    for (i, (f, w)) in ours.iter().zip(rows).enumerate() {
        for (k, name) in want_fields.iter().enumerate() {
            match (&w["attrs"][k + 1], f.attributes.get(k)) {
                (Json::Number(n), Some(v)) => {
                    if num(v).is_none_or(|g| (g - n.as_f64().unwrap()).abs() > 1e-9) {
                        failures.push(format!("{key} row {i} {name}: {v:?} but ArcGIS {n}"));
                    }
                }
                (Json::String(s), Some(Value::String(g))) if s == g.trim_end() => {}
                (a, v) => failures.push(format!("{key} row {i} {name}: {v:?} but ArcGIS {a}")),
            }
        }
        let g = f.geometry.as_ref().unwrap();
        let theirs = parts_of(&w["geom"]);
        let starts: Vec<usize> = if g.parts.is_empty() {
            vec![0]
        } else {
            g.parts.iter().map(|p| *p as usize).collect()
        };
        let mine: Vec<&[[f64; 2]]> = starts
            .iter()
            .enumerate()
            .map(|(i, s)| &g.xy[*s..starts.get(i + 1).copied().unwrap_or(g.xy.len())])
            .collect();
        if mine.len() != theirs.len() || mine.iter().zip(&theirs).any(|(a, b)| a.len() != b.len()) {
            failures.push(format!(
                "{key} row {i}: parts {:?} but ArcGIS {:?}",
                mine.iter().map(|p| p.len()).collect::<Vec<_>>(),
                theirs.iter().map(Vec::len).collect::<Vec<_>>()
            ));
            continue;
        }
        let worst = mine
            .iter()
            .zip(&theirs)
            .flat_map(|(a, b)| {
                a.iter()
                    .zip(b)
                    .map(|(p, q)| (p[0] - q[0]).abs().max((p[1] - q[1]).abs()))
            })
            .fold(0.0, f64::max);
        if worst > 1e-3 {
            failures.push(format!("{key} row {i}: geometry differs by {worst}"));
        }
    }
    failures
}

#[test]
fn construction_tools_equal_arcgis() {
    if !unigis_testkit::ogrinfo_available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let r: Json = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/construct.json")).unwrap(),
    )
    .unwrap();
    let d = std::env::temp_dir().join(format!("gf-construct-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let input = |n: &str| {
        shapefile(&d, n, &r["inputs"][n])
            .to_str()
            .unwrap()
            .to_string()
    };
    let (vp, vl, vl2, vq2) = (input("vp"), input("vl"), input("vl2"), input("vq2"));
    let s = |x: &str| x.to_string();
    let origin = |x: f64, y: f64| format!("{x} {y}");
    let (x0, y0) = (500_000.0, 5_000_000.0);
    // (case key, tool, parameters)
    let mut jobs: Vec<(String, &str, Vec<String>)> = Vec::new();
    for (tag, dist) in [("d15", "15"), ("d7_5", "7.5"), ("d40", "40")] {
        jobs.push((
            format!("densify_lines_{tag}"),
            "densify",
            vec![format!("input={vl}"), format!("distance={dist}")],
        ));
        jobs.push((
            format!("densify_polys_{tag}"),
            "densify",
            vec![format!("input={vp}"), format!("distance={dist}")],
        ));
    }
    let along = |input: &str, rest: &[&str]| -> Vec<String> {
        let mut v = vec![format!("input={input}")];
        v.extend(rest.iter().map(|x| x.to_string()));
        v
    };
    jobs.push((
        "gpal_d25".into(),
        "generate_points_along_lines",
        along(&vl, &["distance=25 Meters"]),
    ));
    jobs.push((
        "gpal_d25_end".into(),
        "generate_points_along_lines",
        along(
            &vl,
            &["distance=25 Meters", "include_end_points=END_POINTS"],
        ),
    ));
    jobs.push((
        "gpal_p20".into(),
        "generate_points_along_lines",
        along(&vl, &["point_placement=PERCENTAGE", "percentage=20"]),
    ));
    jobs.push((
        "gpal_p20_end".into(),
        "generate_points_along_lines",
        along(
            &vl,
            &[
                "point_placement=PERCENTAGE",
                "percentage=20",
                "include_end_points=END_POINTS",
            ],
        ),
    ));
    jobs.push((
        "gpal_d70_polys".into(),
        "generate_points_along_lines",
        along(&vp, &["distance=70 Meters"]),
    ));
    for e in ["NO_END_POINTS", "END_POINTS"] {
        jobs.push((
            format!("v2:d25_{e}"),
            "generate_points_along_lines",
            along(
                &vl2,
                &["distance=25 Meters", &format!("include_end_points={e}")],
            ),
        ));
        jobs.push((
            format!("v2:d25_polys_{e}"),
            "generate_points_along_lines",
            along(
                &vq2,
                &["distance=25 Meters", &format!("include_end_points={e}")],
            ),
        ));
        jobs.push((
            format!("v2:p25_{e}"),
            "generate_points_along_lines",
            along(
                &vl2,
                &[
                    "point_placement=PERCENTAGE",
                    "percentage=25",
                    &format!("include_end_points={e}"),
                ],
            ),
        ));
        jobs.push((
            format!("v2:p50_polys_{e}"),
            "generate_points_along_lines",
            along(
                &vq2,
                &[
                    "point_placement=PERCENTAGE",
                    "percentage=50",
                    &format!("include_end_points={e}"),
                ],
            ),
        ));
    }
    jobs.push((
        "v2:d0_5km".into(),
        "generate_points_along_lines",
        along(&vl2, &["distance=0.025 Kilometers"]),
    ));
    let fish = |extra: &[String]| -> Vec<String> {
        let mut v = vec![format!("template={vl}")];
        v.extend(extra.iter().cloned());
        v
    };
    jobs.push((
        "fish".into(),
        "create_fishnet",
        fish(&[
            format!("origin_coord={}", origin(x0, y0)),
            format!("y_axis_coord={}", origin(x0, y0 + 10.0)),
            s("cell_width=50"),
            s("cell_height=40"),
            s("number_rows=4"),
            s("number_columns=5"),
        ]),
    ));
    jobs.push((
        "fish_ext".into(),
        "create_fishnet",
        fish(&[
            format!("origin_coord={}", origin(x0, y0)),
            format!("y_axis_coord={}", origin(x0, y0 + 10.0)),
            s("number_rows=4"),
            s("number_columns=5"),
            format!("corner_coord={}", origin(x0 + 200.0, y0 + 100.0)),
            s("geometry_type=POLYLINE"),
        ]),
    ));
    jobs.push((
        "fish_corner".into(),
        "create_fishnet",
        fish(&[
            format!("origin_coord={}", origin(x0, y0)),
            format!("y_axis_coord={}", origin(x0 + 10.0, y0)),
            s("cell_width=30"),
            s("cell_height=30"),
            s("number_rows=3"),
            s("number_columns=3"),
            format!("corner_coord={}", origin(x0 + 90.0, y0 + 90.0)),
        ]),
    ));
    jobs.push((
        "p2l_ign".into(),
        "polygon_to_line",
        vec![format!("input={vp}"), s("neighbor_option=IGNORE_NEIGHBORS")],
    ));
    let mut failures = Vec::new();
    for (n, (key, tool, args)) in jobs.iter().enumerate() {
        let out = d.join(format!("o{n}.shp"));
        let mut a = vec![s("run"), tool.to_string()];
        a.extend(args.iter().cloned());
        a.push(format!("output={}", out.display()));
        let o = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(&a)
            .output()
            .unwrap();
        if !o.status.success() {
            failures.push(format!("{key}: {}", String::from_utf8_lossy(&o.stderr)));
            continue;
        }
        failures.extend(compare(key, &out, &r["cases"][key.as_str()]));
    }
    // Bad parameters are errors.
    for bad in [
        vec![
            s("run"),
            s("densify"),
            format!("input={vl}"),
            s("distance=0"),
            format!("output={}", d.join("x.shp").display()),
        ],
        vec![
            s("run"),
            s("densify"),
            format!("input={vl}"),
            s("distance=abc"),
            format!("output={}", d.join("x.shp").display()),
        ],
        vec![
            s("run"),
            s("generate_points_along_lines"),
            format!("input={vl}"),
            s("distance=5 Parsecs"),
            format!("output={}", d.join("x.shp").display()),
        ],
        vec![
            s("run"),
            s("generate_points_along_lines"),
            format!("input={vl}"),
            s("point_placement=PERCENTAGE"),
            s("percentage=150"),
            format!("output={}", d.join("x.shp").display()),
        ],
        vec![
            s("run"),
            s("polygon_to_line"),
            format!("input={vl}"),
            format!("output={}", d.join("x.shp").display()),
        ],
        vec![
            s("run"),
            s("polygon_to_line"),
            format!("input={vp}"),
            s("neighbor_option=SOME_NEIGHBORS"),
            format!("output={}", d.join("x.shp").display()),
        ],
        vec![
            s("run"),
            s("create_fishnet"),
            s("origin_coord=1 1"),
            s("y_axis_coord=1 1"),
            s("number_rows=2"),
            s("number_columns=2"),
            s("cell_width=1"),
            s("cell_height=1"),
            format!("output={}", d.join("x.shp").display()),
        ],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(&bad)
            .output()
            .unwrap();
        assert!(!o.status.success(), "{bad:?}");
    }
    std::fs::remove_dir_all(&d).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
