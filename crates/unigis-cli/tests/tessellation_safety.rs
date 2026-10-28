//! Invalid tessellation parameters must fail without panicking or touching output.
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
fn invalid_tessellation_parameters_preserve_existing_output() {
    let folder = std::env::temp_dir().join(format!("unigis-tess-safety-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let output = folder.join("existing.shp");
    let run = |extent: &str, size: &str| {
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "generate_tessellation", "shape=SQUARE"])
            .arg(format!("extent={extent}"))
            .arg(format!("size={size}"))
            .arg(format!("output={}", output.display()))
            .output()
            .unwrap()
    };
    let initial = run("0 0 10 10", "10000");
    assert!(initial.status.success(), "{initial:?}");
    let before = snapshot(&folder);
    for (extent, size, error) in [
        ("0 0 10 10", "5e-324", "too many cells"),
        ("-1e308 -1 1e308 1", "1", "finite"),
        ("0 0 inf 10", "10000", "finite"),
        ("0 0 10 10 garbage", "10000", "Extent"),
        ("0 0 10 10", "10000 Bananas", "area unit"),
    ] {
        let result = run(extent, size);
        assert_eq!(
            result.status.code(),
            Some(1),
            "{extent} / {size}: {result:?}"
        );
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(error),
            "{result:?}"
        );
        assert_eq!(snapshot(&folder), before, "{extent} / {size}");
    }
    std::fs::remove_dir_all(folder).unwrap();
}
