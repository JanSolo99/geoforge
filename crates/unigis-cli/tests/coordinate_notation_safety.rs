//! Unresolved grid output behavior must fail before changing existing output files.
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
fn unsupported_grid_outputs_preserve_existing_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folder =
        std::env::temp_dir().join(format!("ug-coordinate-notation-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let output = folder.join("existing.shp");
    let run = |source: &str, target: &str| {
        let stem = format!(
            "testdata/geoproc/grid_input_{}",
            source.to_ascii_lowercase()
        );
        let csv = root.join(format!("{stem}.csv"));
        let input = if csv.is_file() {
            csv
        } else {
            root.join(format!("{stem}.dbf"))
        };
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "convert_coordinate_notation", "x_field=COORD"])
            .arg(format!("input={}", input.display()))
            .arg(format!("input_format={source}"))
            .arg(format!("output_format={target}"))
            .arg(format!("output={}", output.display()))
            .output()
            .unwrap()
    };
    let initial = run("GARS", "DD_NUMERIC");
    assert!(initial.status.success(), "{initial:?}");
    let before = snapshot(&folder);
    for source in ["GARS", "GEOREF"] {
        for target in ["UTM_BANDS", "UTM_ZONES", "MGRS", "USNG"] {
            let result = run(source, target);
            assert_eq!(result.status.code(), Some(1), "{source}: {result:?}");
            let message = String::from_utf8_lossy(&result.stderr);
            assert!(message.contains("grid input output format"), "{message}");
            assert_eq!(snapshot(&folder), before, "{source} -> {target}");
        }
    }
    // Projected-grid inputs: only the decimal-degree outputs are verified.
    for source in ["MGRS", "USNG", "UTM_BANDS", "UTM_ZONES"] {
        for target in ["DDM_1", "DMS_2", "GARS", "MGRS"] {
            let result = run(source, target);
            assert_eq!(result.status.code(), Some(1), "{source}: {result:?}");
            let message = String::from_utf8_lossy(&result.stderr);
            assert!(message.contains("grid input output format"), "{message}");
            assert_eq!(snapshot(&folder), before, "{source} -> {target}");
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}
