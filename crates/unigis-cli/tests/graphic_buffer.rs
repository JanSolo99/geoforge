//! Explicit refusals for Graphic Buffer behavior outside the saved ArcGIS cases.

use std::path::{Path, PathBuf};
use std::process::Command;

fn refused(input: &str, distance: &str, tag: &str, message: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "ug-graphic-buffer-refused-{}-{tag}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("out.shp");
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "graphic_buffer"])
        .arg(format!("input={}", root.join(input).display()))
        .arg(format!("distance={distance}"))
        .arg(format!("output={}", output.display()))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{tag}: unexpectedly succeeded");
    assert!(stderr.contains(message), "{tag}: {stderr}");
    assert!(!output.exists(), "{tag}: failure wrote a partial output");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn geographic_distances_are_refused_before_buffering_degrees() {
    for (distance, tag) in [("20", "geographic-raw"), ("20 Meters", "geographic-units")] {
        refused(
            "testdata/gpkg/shapes.gpkg/main.geographic",
            distance,
            tag,
            "geographic input is not supported",
        );
    }
}

#[test]
fn malformed_or_null_text_distances_do_not_write_partial_outputs() {
    for (distance, tag, message) in [
        ("BAD", "malformed", "cannot read distance"),
        ("TEXT_NULL", "text-null", "cannot read distance"),
    ] {
        refused(
            "testdata/geoproc/graphic_buffer_fields.shp",
            distance,
            tag,
            message,
        );
    }
}

#[test]
fn known_unit_fields_require_a_coordinate_unit_conversion() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("ug-graphic-buffer-no-crs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for extension in ["shp", "shx", "dbf"] {
        std::fs::copy(
            root.join(format!(
                "testdata/geoproc/graphic_buffer_fields.{extension}"
            )),
            dir.join(format!("input.{extension}")),
        )
        .unwrap();
    }
    refused(
        dir.join("input.shp").to_str().unwrap(),
        "TEXT_DIST",
        "known-units-without-crs",
        "needs a projected coordinate system",
    );
    std::fs::remove_dir_all(dir).unwrap();
}
