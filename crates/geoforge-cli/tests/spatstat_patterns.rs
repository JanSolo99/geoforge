//! Average Nearest Neighbor, Spatial Autocorrelation (Moran's I) and Hot Spot Analysis (Gi*) against
//! ArcGIS Pro 3.7 (testdata/arcgis-reference/spatstat_patterns.json: two point sets of 40 and 70
//! features with a weight field, every conceptualization, both distance methods).

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::Value;
use geoforge_shapefile::ShapefileReader;
use serde_json::Value as Json;

fn shapefile(dir: &Path, name: &str, ds: &Json) -> PathBuf {
    let features: Vec<String> = ds["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            format!(
                r#"{{"type":"Feature","properties":{{"W":{}}},"geometry":{{"type":"Point","coordinates":[{},{}]}}}}"#,
                r[2], r[0], r[1]
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
        .args(["-f", "ESRI Shapefile", "-a_srs", "EPSG:32632"])
        .arg(&shp)
        .arg(&json)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    shp
}

fn run(args: &[String]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_gf"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn report(stdout: &str) -> Vec<(String, f64)> {
    stdout
        .lines()
        .filter_map(|l| l.rsplit_once(": "))
        .filter_map(|(k, v)| v.trim().parse().ok().map(|v| (k.to_string(), v)))
        .collect()
}

#[test]
fn pattern_statistics_equal_arcgis() {
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let r: Json = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/spatstat_patterns.json")).unwrap(),
    )
    .unwrap();
    let d = std::env::temp_dir().join(format!("gf-patterns-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let mut failures = Vec::new();
    for ds in ["fa", "fb"] {
        let shp = shapefile(&d, ds, &r["datasets"][ds]);
        let input = format!("input={}", shp.display());
        let s = |x: &str| x.to_string();
        // Average Nearest Neighbor: ratio, z, p, expected, observed (ArcGIS's p is a few 1e-8 away).
        for (key, extra) in [
            ("ann", vec![]),
            ("ann_man", vec![s("distance_method=MANHATTAN_DISTANCE")]),
            ("ann_area", vec![s("area=400000")]),
        ] {
            let mut a = vec![s("run"), s("average_nearest_neighbor"), input.clone()];
            a.extend(extra);
            let got = report(&run(&a));
            let want: Vec<f64> = r["ann"][format!("{ds}:{key}")]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            if got.len() != 5
                || got
                    .iter()
                    .zip(&want)
                    .any(|((_, g), w)| (g - w).abs() > 1e-5 + 1e-6 * w.abs())
            {
                failures.push(format!("{ds}:{key}: {got:?} but ArcGIS {want:?}"));
            }
        }
        // Moran's I: index, z, p.
        for (key, conc, std, method, band) in [
            (
                "INVERSE_DISTANCE:NONE:EUCLIDEAN_DISTANCE",
                "INVERSE_DISTANCE",
                "NONE",
                "EUCLIDEAN_DISTANCE",
                "0",
            ),
            (
                "INVERSE_DISTANCE:ROW:EUCLIDEAN_DISTANCE",
                "INVERSE_DISTANCE",
                "ROW",
                "EUCLIDEAN_DISTANCE",
                "0",
            ),
            (
                "INVERSE_DISTANCE:ROW:MANHATTAN_DISTANCE",
                "INVERSE_DISTANCE",
                "ROW",
                "MANHATTAN_DISTANCE",
                "0",
            ),
            (
                "INVERSE_DISTANCE_SQUARED:NONE:EUCLIDEAN_DISTANCE",
                "INVERSE_DISTANCE_SQUARED",
                "NONE",
                "EUCLIDEAN_DISTANCE",
                "0",
            ),
            (
                "INVERSE_DISTANCE_SQUARED:ROW:EUCLIDEAN_DISTANCE",
                "INVERSE_DISTANCE_SQUARED",
                "ROW",
                "EUCLIDEAN_DISTANCE",
                "0",
            ),
            (
                "K_NEAREST_NEIGHBORS:ROW:MANHATTAN_DISTANCE",
                "K_NEAREST_NEIGHBORS",
                "ROW",
                "MANHATTAN_DISTANCE",
                "0",
            ),
            (
                "fixed250:NONE",
                "FIXED_DISTANCE_BAND",
                "NONE",
                "EUCLIDEAN_DISTANCE",
                "250",
            ),
            (
                "inv_band300",
                "INVERSE_DISTANCE",
                "NONE",
                "EUCLIDEAN_DISTANCE",
                "300",
            ),
            (
                "zoi250",
                "ZONE_OF_INDIFFERENCE",
                "ROW",
                "EUCLIDEAN_DISTANCE",
                "250",
            ),
        ] {
            let a = vec![
                s("run"),
                s("spatial_autocorrelation"),
                input.clone(),
                s("field=W"),
                format!("conceptualization={conc}"),
                format!("standardization={std}"),
                format!("distance_method={method}"),
                format!("distance_band={band}"),
            ];
            let got = report(&run(&a));
            let want: Vec<f64> = r["moran"][format!("{ds}:moran:{key}")]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let mine = [
                got.first().map(|g| g.1),
                got.get(3).map(|g| g.1),
                got.get(4).map(|g| g.1),
            ];
            if mine
                .iter()
                .zip(&want)
                .any(|(g, w)| g.is_none_or(|g| (g - w).abs() > 1e-5 + 1e-6 * w.abs()))
            {
                failures.push(format!("{ds} Moran {key}: {mine:?} but ArcGIS {want:?}"));
            }
        }
        // Hot Spot Analysis: schema, z, p (ArcGIS's is approximate), neighbors and bin.
        for (key, args) in [
            (
                "fixed250",
                vec![
                    s("conceptualization=FIXED_DISTANCE_BAND"),
                    s("distance_band=250"),
                ],
            ),
            (
                "fixed250_row",
                vec![
                    s("conceptualization=FIXED_DISTANCE_BAND"),
                    s("distance_band=250"),
                    s("standardization=ROW"),
                ],
            ),
            (
                "inv",
                vec![
                    s("conceptualization=INVERSE_DISTANCE"),
                    s("standardization=ROW"),
                ],
            ),
            (
                "zoi",
                vec![
                    s("conceptualization=ZONE_OF_INDIFFERENCE"),
                    s("distance_band=250"),
                    s("standardization=ROW"),
                ],
            ),
            (
                "fixed250_fdr",
                vec![
                    s("conceptualization=FIXED_DISTANCE_BAND"),
                    s("distance_band=250"),
                    s("apply_fdr=APPLY_FDR"),
                ],
            ),
            (
                "knn5",
                vec![
                    s("conceptualization=K_NEAREST_NEIGHBORS"),
                    s("number_of_neighbors=5"),
                ],
            ),
            (
                "knn12",
                vec![
                    s("conceptualization=K_NEAREST_NEIGHBORS"),
                    s("number_of_neighbors=12"),
                ],
            ),
            (
                "knn5_man",
                vec![
                    s("conceptualization=K_NEAREST_NEIGHBORS"),
                    s("number_of_neighbors=5"),
                    s("distance_method=MANHATTAN_DISTANCE"),
                ],
            ),
            (
                "invsq",
                vec![s("conceptualization=INVERSE_DISTANCE_SQUARED")],
            ),
            (
                "inv_band",
                vec![
                    s("conceptualization=INVERSE_DISTANCE"),
                    s("distance_band=200"),
                ],
            ),
        ] {
            let out = d.join(format!("{ds}_{key}.shp"));
            let mut a = vec![
                s("run"),
                s("hot_spot_analysis"),
                input.clone(),
                s("field=W"),
                format!("output={}", out.display()),
            ];
            a.extend(args);
            run(&a);
            let rd = ShapefileReader::open(&out).unwrap();
            let names: Vec<&str> = rd.fields().iter().map(|f| f.name.as_str()).collect();
            if names
                != [
                    "SOURCE_ID",
                    "W",
                    "GiZScore",
                    "GiPValue",
                    "NNeighbors",
                    "Gi_Bin",
                ]
            {
                failures.push(format!("{ds} hot {key}: fields {names:?}"));
                continue;
            }
            let want = r["hot"][format!("{ds}:{key}")]["rows"].as_array().unwrap();
            let ours: Vec<_> = rd.features().map(|f| f.unwrap()).collect();
            if ours.len() != want.len() {
                failures.push(format!(
                    "{ds} hot {key}: {} features but {}",
                    ours.len(),
                    want.len()
                ));
                continue;
            }
            for (i, (f, w)) in ours.iter().zip(want).enumerate() {
                let num = |k: usize| match &f.attributes[k] {
                    Value::Double(v) => *v,
                    Value::Integer(v) => *v as f64,
                    other => panic!("{other:?}"),
                };
                // The reference row: FID, SOURCE_ID, W, GiZScore, GiPValue, NNeighbors, Gi_Bin.
                let bad = (num(2) - w[3].as_f64().unwrap()).abs() > 1e-9
                    || (num(3) - w[4].as_f64().unwrap()).abs() > 5e-8
                    || num(4) != w[5].as_f64().unwrap()
                    || num(5) != w[6].as_f64().unwrap()
                    || num(0) != w[1].as_f64().unwrap();
                if bad {
                    failures.push(format!(
                        "{ds} hot {key} row {i}: {:?} but ArcGIS {w}",
                        f.attributes
                    ));
                    break;
                }
            }
        }
        // Distance bands that are needed but missing are errors.
        let o = Command::new(env!("CARGO_BIN_EXE_gf"))
            .args(["run", "spatial_autocorrelation"])
            .arg(&input)
            .args(["field=W", "conceptualization=FIXED_DISTANCE_BAND"])
            .output()
            .unwrap();
        assert!(!o.status.success());
    }
    std::fs::remove_dir_all(&d).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
