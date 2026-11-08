//! Get Raster Properties against values ArcGIS Pro 3.7 returned (scripts/research/get_raster_properties/).
use std::process::Command;

fn run(raster: &str, property: &str, band: &str) -> (bool, String) {
    let path = format!(
        "{}/../../testdata/raster/{raster}",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "get_raster_properties"])
        .arg(format!("input={path}"))
        .arg(format!("property_type={property}"))
        .arg(format!("band_index={band}"))
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text.trim().to_string())
}

#[test]
fn matches_arcgis_values() {
    let m = "copy_multicache/first_full_f64.tif";
    let cases = [
        ("dem.tif", "MINIMUM", "", "188.10171508789"),
        ("dem.tif", "MAXIMUM", "", "324.82815551758"),
        ("dem.tif", "MEAN", "", "238.346012569"),
        ("dem.tif", "STD", "Band_1", "30.479163071878"),
        ("dem.tif", "TOP", "", "5001200"),
        ("dem.tif", "RIGHT", "", "501500"),
        ("dem.tif", "BOTTOM", "", "5000000"),
        ("dem.tif", "CELLSIZEX", "", "10"),
        ("dem.tif", "VALUETYPE", "", "9"),
        ("dem.tif", "COLUMNCOUNT", "", "150"),
        ("dem.tif", "ROWCOUNT", "", "120"),
        ("dem.tif", "ANYNODATA", "", "0"),
        ("dem.tif", "SENSORNAME", "", "UNKNOWN"),
        ("dem.tif", "SOURCETYPE", "", "Generic"),
        ("dem_geographic.tif", "TOP", "", "52"),
        ("dem_geographic.tif", "RIGHT", "", "5.015"),
        ("dem_geographic.tif", "BOTTOM", "", "51.988"),
        (
            "dem_geographic.tif",
            "CELLSIZEX",
            "",
            "9.99999999999979E-05",
        ),
        (
            "dem_geographic.tif",
            "CELLSIZEY",
            "",
            "0.000100000000000004",
        ),
        ("world_nonsquare.tif", "CELLSIZEX", "", "20"),
        ("world_nonsquare.tif", "CELLSIZEY", "", "10"),
        ("classes_u8.tif", "VALUETYPE", "", "3"),
        ("classes_u8.tif", "ANYNODATA", "", "1"),
        (m, "MINIMUM", "", "1"),
        (m, "MEAN", "", "2.5"),
        (m, "STD", "Band_1", "1.2"),
        (m, "BANDCOUNT", "", "2"),
        (m, "VALUETYPE", "", "10"),
        (m, "ANYNODATA", "Band_1", "1"),
        (m, "ANYNODATA", "Band_2", "0"),
        (m, "ALLNODATA", "", "0"),
        ("copy_raster/later_nodata.tif", "ANYNODATA", "", "1"),
        ("copy_raster/later_nodata.tif", "ANYNODATA", "Band_1", "0"),
    ];
    for (r, prop, band, want) in cases {
        let (ok, got) = run(r, prop, band);
        assert!(ok && got == want, "{r} {prop} {band}: {got:?} != {want:?}");
    }
}

#[test]
fn failures_match_arcgis_errors() {
    for (r, prop, band, err) in [
        ("classes_u8.tif", "MINIMUM", "", "ERROR 001100"),
        (
            "copy_multicache/first_full_f64.tif",
            "MINIMUM",
            "Band_2",
            "ERROR 001100",
        ),
        ("dem.tif", "FOO", "", "ERROR 000864"),
        ("dem.tif", "UNIQUEVALUECOUNT", "", "ERROR 000864"),
        ("dem.tif", "TOP", "Band_2", "ERROR 000864"),
        ("nope.tif", "TOP", "", "ERROR 000732"),
    ] {
        let (ok, text) = run(r, prop, band);
        assert!(!ok && text.contains(err), "{r} {prop} {band}: {text}");
    }
}
