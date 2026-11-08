//! Get Cell Value against values ArcGIS Pro 3.7 returned (scripts/research/get_cell_value/).
use std::process::Command;

fn run(raster: &str, location: &str, band: &str) -> (bool, String) {
    let path = format!(
        "{}/../../testdata/raster/{raster}",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "get_cell_value"])
        .arg(format!("input={path}"))
        .arg(format!("location={location}"))
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
    let cases = [
        ("dem.tif", "500025.0 5001165.0", "1", "204.9949"),
        ("dem.tif", "500000.0 5001200.0", "1", "202.53319"),
        ("dem.tif", "501500.0 5000000.0", "1", "219.88638"),
        ("dem.tif", "501500.001 5000000.0", "1", "NoData"),
        ("dem.tif", "499000.0 5001165.0", "1", "NoData"),
        ("dem.tif", "500025.0 5001165.0", "2", "NoData"),
        ("classes_u8.tif", "500015.0 5001185.0", "1", "5"),
        ("dem_geographic.tif", "5.01495 51.98805", "1", "219.88638"),
        ("world_nonsquare.tif", "1070.0 2005.0", "1", "11"),
        (
            "copy_multicache/first_full_f64.tif",
            "500015 5000015",
            "1",
            "NoData",
        ),
        (
            "copy_multicache/first_full_f64.tif",
            "500015 5000015",
            "2",
            "2",
        ),
        (
            "copy_multicache/first_full_f64.tif",
            "500005 5000015",
            "1",
            "1",
        ),
        (
            "copy_multicache/first_full_f64.tif",
            "500005 5000015",
            "3",
            "NoData",
        ),
    ];
    for (r, loc, band, want) in cases {
        let (ok, got) = run(r, loc, band);
        assert!(
            ok && got == want,
            "{r} {loc} band {band}: {got:?} != {want:?}"
        );
    }
}

#[test]
fn bad_location_and_missing_raster_fail_like_arcgis() {
    let (ok, text) = run("dem.tif", "500005", "1");
    assert!(!ok && text.contains("ERROR 000628"), "{text}");
    let (ok, text) = run("nope.tif", "1 1", "1");
    assert!(!ok && text.contains("ERROR 000732"), "{text}");
}
