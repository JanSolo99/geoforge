//! Invalid/unsupported batches must not delete earlier valid inputs.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            out.insert(
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(p).unwrap(),
            );
        }
    }
    out
}
#[test]
fn malformed_or_unsupported_later_inputs_leave_valid_data_untouched() {
    let root = std::env::temp_dir().join(format!("ug-delete-preflight-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/delete_multiple");
    for e in std::fs::read_dir(fixture).unwrap() {
        let e = e.unwrap();
        if e.path().is_file() {
            std::fs::copy(e.path(), root.join(e.file_name())).unwrap();
        }
    }
    std::fs::create_dir(root.join("folder")).unwrap();
    std::fs::write(root.join("folder/sentinel.txt"), b"retain folder contents").unwrap();
    let before = snapshot(&root);
    let valid = format!("'{}'", root.join("points.shp").display());
    for tail in [
        format!("'{}'", root.join("folder").display()),
        "'unterminated".into(),
        "bad too many columns".into(),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "delete_multiple"])
            .arg(format!("in_data={valid};{tail}"))
            .output()
            .unwrap();
        assert!(!result.status.success(), "{result:?}");
        assert_eq!(snapshot(&root), before);
        assert_eq!(
            std::fs::read(root.join("folder/sentinel.txt")).unwrap(),
            b"retain folder contents"
        );
    }
    // Ancillary names must designate files, never directories to recursively remove.
    std::fs::create_dir(root.join("points.KEY.atx")).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "delete_multiple"])
        .arg(format!("in_data={valid}"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(snapshot(&root), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn field_index_cleanup_preserves_a_dotted_sibling_dataset() {
    let root = std::env::temp_dir().join(format!("ug-delete-sibling-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/delete_multiple");
    for e in std::fs::read_dir(fixture).unwrap() {
        let e = e.unwrap();
        if e.path().is_file() {
            std::fs::copy(e.path(), root.join(e.file_name())).unwrap();
        }
    }
    std::fs::write(root.join("points.KEY.atx"), b"own index").unwrap();
    std::fs::write(root.join("points.other.KEY.atx"), b"sibling index").unwrap();
    let sibling = std::fs::read(root.join("points.other.shp")).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "delete_multiple"])
        .arg(format!("in_data='{}'", root.join("points.shp").display()))
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    assert!(!root.join("points.KEY.atx").exists());
    assert_eq!(
        std::fs::read(root.join("points.other.KEY.atx")).unwrap(),
        b"sibling index"
    );
    assert_eq!(
        std::fs::read(root.join("points.other.shp")).unwrap(),
        sibling
    );
    std::fs::remove_dir_all(root).unwrap();
}
