//! Enable Editing Templates: a file geodatabase is left byte-identical (as ArcGIS Pro 3.7 does)
//! and other workspaces fail with ArcGIS's errors.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn snapshot(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(folder)
        .unwrap()
        .map(|e| {
            let path = e.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect()
}

fn run(workspace: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "enable_editing_templates"])
        .arg(format!("in_workspace={}", workspace.display()))
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn a_file_geodatabase_is_unchanged_and_other_workspaces_fail_like_arcgis() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("ug-enable-templates-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let gdb = dir.join("edit_base.gdb");
    std::fs::create_dir_all(&gdb).unwrap();
    for e in std::fs::read_dir(root.join("testdata/gdb-reference/edit_base.gdb")).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), gdb.join(e.file_name())).unwrap();
    }
    let before = snapshot(&gdb);
    let (ok, text) = run(&gdb);
    assert!(ok, "{text}");
    assert_eq!(snapshot(&gdb), before, "a file geodatabase stays as it is");
    // Twice is fine, too.
    assert!(run(&gdb).0);
    // A folder, a missing path: ERROR 000584; a file: ERROR 160632.
    let folder = dir.join("plain");
    std::fs::create_dir_all(&folder).unwrap();
    for missing in [folder.clone(), dir.join("nope.gdb")] {
        let (ok, text) = run(&missing);
        assert!(!ok && text.contains("ERROR 000584"), "{text}");
    }
    let file = dir.join("a.shp");
    std::fs::write(&file, b"x").unwrap();
    let (ok, text) = run(&file);
    assert!(!ok && text.contains("ERROR 160632"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
