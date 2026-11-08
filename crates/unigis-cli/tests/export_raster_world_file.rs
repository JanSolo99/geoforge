//! Export Raster World File: a missing raster fails with ArcGIS's ERROR 000732.
use std::process::Command;

#[test]
fn a_missing_raster_fails_like_arcgis() {
    let dir = std::env::temp_dir().join(format!("ug-world-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let missing = dir.join("nope.tif");
    let out = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "export_raster_world_file"])
        .arg(format!("input={}", missing.display()))
        .arg(format!("output={}", dir.join("out.tif").display()))
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success() && text.contains("ERROR 000732"),
        "{text}"
    );
    assert!(!dir.join("out.tfw").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
