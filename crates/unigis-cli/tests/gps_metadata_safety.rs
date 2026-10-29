//! Schema edits preserve data on both validation and mid-operation errors.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use unigis_core::{Field, FieldType, Value};
use unigis_filegdb::{Domain, DomainKind, MergePolicy, SplitPolicy};

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
fn copy(name: &str) -> PathBuf {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/gps_metadata_base.gdb");
    let copied = std::env::temp_dir().join(format!("ug-gps-{name}-{}.gdb", std::process::id()));
    std::fs::create_dir(&copied).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), copied.join(e.file_name())).unwrap();
        }
    }
    copied
}
fn domain() -> Domain {
    Domain {
        name: "ESRI_FIX_TYPE_DOMAIN".into(),
        description: "conflict".into(),
        field_type: FieldType::SmallInteger,
        split_policy: SplitPolicy::DefaultValue,
        merge_policy: MergePolicy::DefaultValue,
        kind: DomainKind::Coded(vec![(Value::SmallInteger(9), "Custom".into())]),
    }
}
#[test]
fn conflicting_domain_is_refused_before_any_changes() {
    let copied = copy("conflict");
    let mut writer = unigis_filegdb::GeodatabaseWriter::open_existing(&copied).unwrap();
    writer.add_domain(&domain()).unwrap();
    writer.finish().unwrap();
    let before = snapshot(&copied);
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "add_gps_metadata_fields"])
        .arg(format!("input={}/point", copied.display()))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("000355"),
        "{result:?}"
    );
    assert_eq!(snapshot(&copied), before);
    std::fs::remove_dir_all(copied).unwrap();
}
#[test]
fn failed_assignment_rolls_back_domains_and_row_rewrite() {
    let copied = copy("rollback");
    let before = snapshot(&copied);
    let mut f = Field::new("NEW_FIELD", FieldType::SmallInteger);
    f.domain = Some("ABSENT_DOMAIN".into());
    // Domain creation and row rewrite succeed, then assignment fails. The outer
    // transaction must undo both changes, including system catalog/domain links.
    let result = unigis_filegdb::add_schema_fields(&copied, "point", &[f], &[domain()]);
    assert!(result.is_err());
    assert_eq!(snapshot(&copied), before);
    std::fs::remove_dir_all(copied).unwrap();
}
