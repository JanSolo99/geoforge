//! A conversion failure must preserve the source file and its header.

use std::path::Path;
use std::process::Command;

#[test]
fn truncated_float_import_preserves_inputs_and_writes_no_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("ug-import-preserve-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for suffix in ["flt", "hdr"] {
        std::fs::copy(
            root.join(format!("testdata/raster/grid_import/float_short.{suffix}")),
            dir.join(format!("short.{suffix}")),
        )
        .unwrap();
    }
    let source = dir.join("short.flt");
    let header = dir.join("short.hdr");
    let original_source = std::fs::read(&source).unwrap();
    let original_header = std::fs::read(&header).unwrap();
    let output = dir.join("out.tif");
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "float_to_raster"])
        .arg(format!("input={}", source.display()))
        .arg(format!("output={}", output.display()))
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{error}");
    assert!(error.contains("010067"), "{error}");
    assert_eq!(std::fs::read(source).unwrap(), original_source);
    assert_eq!(std::fs::read(header).unwrap(), original_header);
    assert!(!output.exists());
    std::fs::remove_dir_all(dir).unwrap();
}
