//! Unsupported multiband unary requests must preserve existing GIS outputs.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn snapshot(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn unsupported_multiband_unary_preserves_existing_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folder = std::env::temp_dir().join(format!("ug-unary-bands-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let raw = folder.join("raw.tif");
    let cached = folder.join("cached.tif");
    let float = folder.join("float.tif");
    let output = folder.join("existing.tif");
    std::fs::copy(root.join("testdata/raster/unary_multiband/i16.tif"), &raw).unwrap();
    std::fs::copy(&raw, &cached).unwrap();
    std::fs::copy(
        root.join("testdata/raster/unary_multiband/i16_cached_statistics.xml"),
        folder.join("cached.tif.aux.xml"),
    )
    .unwrap();
    std::fs::copy(
        root.join("testdata/raster/copy_multicache/first_full_f64.tif"),
        &float,
    )
    .unwrap();
    let run = |tool: &str, input: &Path| {
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", tool])
            .arg(format!("input={}", input.display()))
            .arg(format!("output={}", output.display()))
            .output()
            .unwrap()
    };
    let initial = run("abs", &raw);
    assert!(initial.status.success(), "{initial:?}");
    let before = snapshot(&folder);
    for (tool, input) in [
        ("abs", &cached),
        ("negate", &cached),
        ("square", &cached),
        ("log2", &float),
        ("sin", &float),
        ("round_up", &float),
    ] {
        let result = run(tool, input);
        assert_eq!(result.status.code(), Some(1), "{tool}: {result:?}");
        let error = String::from_utf8_lossy(&result.stderr).to_ascii_lowercase();
        assert!(
            error.contains("multiband") && error.contains("not yet supported"),
            "{tool}: {result:?}"
        );
        assert_eq!(snapshot(&folder), before, "{tool}");
    }
    std::fs::remove_dir_all(folder).unwrap();
}
