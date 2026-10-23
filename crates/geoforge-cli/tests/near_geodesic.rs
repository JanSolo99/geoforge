//! `ug near` and `ug generate-near-table` with `--method GEODESIC` on longitude/latitude data, against
//! the ArcGIS values (point to point: 181171.41 m, azimuth 22.54 degrees).

use std::path::PathBuf;
use std::process::Command;

fn shapefile(dir: &std::path::Path, name: &str, x: f64, y: f64) -> PathBuf {
    let json = dir.join(format!("{name}.geojson"));
    std::fs::write(
        &json,
        format!(r#"{{"type":"FeatureCollection","features":[{{"type":"Feature","properties":{{"id":1}},"geometry":{{"type":"Point","coordinates":[{x},{y}]}}}}]}}"#),
    )
    .unwrap();
    let shp = dir.join(format!("{name}.shp"));
    let r = Command::new("ogr2ogr")
        .args(["-f", "ESRI Shapefile", "-a_srs", "EPSG:4326"])
        .arg(&shp)
        .arg(&json)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    shp
}

fn value(dump: &str, field: &str) -> f64 {
    let line = dump
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("{field} (")))
        .unwrap_or_else(|| panic!("{field} in {dump}"));
    line.split('=').nth(1).unwrap().trim().parse().unwrap()
}

#[test]
fn geodesic_method_measures_on_the_ellipsoid() {
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let d = std::env::temp_dir().join(format!("gf-neargeo-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let (a, b) = (
        shapefile(&d, "a", 10.0, 50.0),
        shapefile(&d, "b", 11.0, 51.5),
    );
    let table = d.join("t.dbf");
    let r = Command::new(env!("CARGO_BIN_EXE_ug"))
        .arg("generate-near-table")
        .arg(&a)
        .arg(&b)
        .arg(&table)
        .args([
            "--method",
            "GEODESIC",
            "--location",
            "LOCATION",
            "--angle",
            "ANGLE",
        ])
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let dump = String::from_utf8_lossy(
        &Command::new("ogrinfo")
            .args(["-al"])
            .arg(&table)
            .output()
            .unwrap()
            .stdout,
    )
    .into_owned();
    assert!(
        (value(&dump, "NEAR_DIST") - 181_171.41024908726).abs() < 1e-3,
        "{dump}"
    );
    assert!(
        (value(&dump, "NEAR_ANGLE") - 22.539376658454792).abs() < 1e-6,
        "{dump}"
    );
    // `ug near` with a radius in kilometres: found at 200 km, not at 100 km (distance -1).
    for (radius, found) in [("200 Kilometers", true), ("100 Kilometers", false)] {
        let copy = d.join(format!("c{}.shp", radius.len()));
        let ogr = Command::new("ogr2ogr")
            .args(["-f", "ESRI Shapefile"])
            .arg(&copy)
            .arg(&a)
            .output()
            .unwrap();
        assert!(ogr.status.success());
        let r = Command::new(env!("CARGO_BIN_EXE_ug"))
            .arg("near")
            .arg(&copy)
            .arg(&b)
            .args(["--method", "GEODESIC", "--search-radius", radius])
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let dump = String::from_utf8_lossy(
            &Command::new("ogrinfo")
                .args(["-al"])
                .arg(&copy)
                .output()
                .unwrap()
                .stdout,
        )
        .into_owned();
        let dist = value(&dump, "NEAR_DIST");
        assert_eq!(dist > 0.0, found, "{radius}: {dist}");
        if found {
            assert!((dist - 181_171.41024908726).abs() < 1e-3);
        }
    }
    let bad = Command::new(env!("CARGO_BIN_EXE_ug"))
        .arg("near")
        .arg(&a)
        .arg(&b)
        .args(["--method", "RHUMB"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
