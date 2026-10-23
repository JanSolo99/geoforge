//! `ug idw` against the ArcGIS Idw run on the same 16 points (testdata/arcgis-reference/idw.json).

use std::path::Path;
use std::process::Command;

#[test]
fn idw_command_equals_arcgis() {
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reference: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/idw.json")).unwrap(),
    )
    .unwrap();
    let d = std::env::temp_dir().join(format!("gf-idw-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    // The points as GeoJSON in UTM 32N, made a shapefile by GDAL (a test oracle for reading only).
    let features: Vec<String> = reference["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            format!(
                r#"{{"type":"Feature","properties":{{"V":{}}},"geometry":{{"type":"Point","coordinates":[{},{}]}}}}"#,
                p[2], p[0], p[1]
            )
        })
        .collect();
    let json = d.join("p.geojson");
    std::fs::write(
        &json,
        format!(
            r#"{{"type":"FeatureCollection","features":[{}]}}"#,
            features.join(",")
        ),
    )
    .unwrap();
    let shp = d.join("p.shp");
    let r = Command::new("ogr2ogr")
        .args(["-f", "ESRI Shapefile", "-a_srs", "EPSG:32632"])
        .arg(&shp)
        .arg(&json)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    for (name, args) in [
        ("default", vec![]),
        ("power3", vec!["--power", "3"]),
        (
            "var5_max",
            vec!["--num-points", "5", "--max-distance", "150"],
        ),
        (
            "fixed_min3",
            vec![
                "--search-radius",
                "FIXED",
                "--distance",
                "150",
                "--min-points",
                "3",
            ],
        ),
    ] {
        let out = d.join(format!("{name}.tif"));
        let r = Command::new(env!("CARGO_BIN_EXE_ug"))
            .arg("idw")
            .arg(&shp)
            .arg("V")
            .arg(&out)
            .args(["--cellsize", "20"])
            .args(&args)
            .output()
            .unwrap();
        assert!(
            r.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&r.stderr)
        );
        let got = geoforge_raster::read_geotiff(&out).unwrap();
        let want = &reference["param_cases"][name];
        assert_eq!(got.width, want["cols"].as_u64().unwrap() as usize, "{name}");
        assert_eq!(
            got.height,
            want["rows"].as_u64().unwrap() as usize,
            "{name}"
        );
        for (r, row) in want["values"].as_array().unwrap().iter().enumerate() {
            for (c, w) in row.as_array().unwrap().iter().enumerate() {
                let w = w.as_f64().unwrap();
                match got.get(c, r) {
                    Some(g) => assert!(
                        w > -9000.0 && (g - w).abs() <= 1e-3,
                        "{name} {r},{c}: {g} vs {w}"
                    ),
                    None => assert!(w < -9000.0, "{name} {r},{c}"),
                }
            }
        }
    }
    // Bad parameters are errors.
    let bad = Command::new(env!("CARGO_BIN_EXE_ug"))
        .arg("idw")
        .arg(&shp)
        .arg("V")
        .arg(d.join("x.tif"))
        .args(["--power", "-1"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    std::fs::remove_dir_all(&d).ok();
}
