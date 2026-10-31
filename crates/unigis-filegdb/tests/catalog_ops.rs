//! Delete and Rename of geodatabase items leave the catalog as ArcGIS Pro leaves it
//! (testdata/gdb-reference/catalog_ops: ArcGIS's states after each operation, as JSON snapshots made
//! by scripts/gdb-snapshot.py through GDAL). Our result is snapshotted the same way and compared.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use unigis_filegdb::{Geodatabase, delete_domain, delete_item, rename_domain, rename_item};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn snapshot(gdb: &Path) -> Option<Value> {
    let out = Command::new("python3")
        .arg(root().join("scripts/gdb-snapshot.py"))
        .arg(gdb)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| serde_json::from_slice(&out.stdout).ok())?
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let name = e.file_name();
        if name.to_string_lossy().ends_with(".lock") {
            continue;
        }
        std::fs::copy(e.path(), to.join(name)).unwrap();
    }
}

fn run(op: &str, apply: impl Fn(&Path)) {
    let ref_dir = root().join("testdata/gdb-reference/catalog_ops");
    if !unigis_testkit::ogrinfo_available() {
        return;
    }
    let work = std::env::temp_dir().join(format!("gf-catalog-{op}-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&work).ok();
    copy_dir(&ref_dir.join("before.gdb"), &work);
    apply(&work);
    let ours = snapshot(&work);
    std::fs::remove_dir_all(&work).ok();
    let Some(ours) = ours else { return };
    let want: Value =
        serde_json::from_slice(&std::fs::read(ref_dir.join(format!("{op}.json"))).unwrap())
            .unwrap();
    for part in ["catalog", "tables", "rels", "items"] {
        assert_eq!(ours[part], want[part], "{op}: {part}");
    }
}

#[test]
fn delete_table_removes_its_relationship_class() {
    run("delTbl", |g| {
        assert_eq!(delete_item(g, "tbl").unwrap(), ["tbl", "rc"]);
    });
}

#[test]
fn delete_feature_class_in_a_dataset() {
    run("delInds", |g| {
        delete_item(g, "INDS").unwrap();
    });
}

#[test]
fn delete_relationship_class() {
    run("delRc", |g| {
        delete_item(g, "rc").unwrap();
    });
}

#[test]
fn delete_feature_dataset_deletes_its_classes() {
    run("delDs", |g| {
        delete_item(g, "ds").unwrap();
    });
}

#[test]
fn rename_feature_class_table_and_relationship_class() {
    run("renLoose", |g| rename_item(g, "loose", "loose2").unwrap());
    run("renTbl", |g| rename_item(g, "tbl", "tbl2").unwrap());
    run("renRc", |g| rename_item(g, "rc", "rc2").unwrap());
}

#[test]
fn rename_feature_dataset_moves_its_children() {
    run("renDs", |g| rename_item(g, "ds", "ds2").unwrap());
}

#[test]
fn rename_domain_updates_fields() {
    run("renDom", |g| rename_domain(g, "dom", "dom2").unwrap());
}

#[test]
fn refusals() {
    let ref_dir = root().join("testdata/gdb-reference/catalog_ops");
    let work = std::env::temp_dir().join(format!("gf-catalog-refuse-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&work).ok();
    copy_dir(&ref_dir.join("before.gdb"), &work);
    // A domain fields use can't be deleted (ArcGIS ERROR 002630); a name taken or missing is refused.
    assert!(
        delete_domain(&work, "dom")
            .unwrap_err()
            .to_string()
            .contains("002630")
    );
    assert!(
        rename_item(&work, "tbl", "LOOSE")
            .unwrap_err()
            .to_string()
            .contains("000725")
    );
    assert!(
        delete_item(&work, "nothing")
            .unwrap_err()
            .to_string()
            .contains("000732")
    );
    assert!(rename_item(&work, "tbl", "1bad").is_err());
    // Nothing above changed anything: the geodatabase opens and still lists everything.
    let gdb = Geodatabase::open(&work).unwrap();
    assert_eq!(gdb.entries().len(), 3);
    std::fs::remove_dir_all(&work).ok();
}

/// ArcGIS Pro's Delete Mosaic Dataset on testdata/geoproc/mosaic_seed.gdb (one mosaic dataset
/// `md` with a raster, one feature class `keep`) leaves `keep`, the shared domain, and exactly
/// these table files.
#[test]
fn delete_mosaic_dataset_removes_its_amd_tables() {
    let work = std::env::temp_dir().join(format!("gf-mosaic-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&work).ok();
    copy_dir(&root().join("testdata/geoproc/mosaic_seed.gdb"), &work);
    assert!(unigis_filegdb::delete_mosaic_dataset(&work, "keep").is_err());
    let mut gone = unigis_filegdb::delete_mosaic_dataset(&work, "md").unwrap();
    gone.sort();
    assert_eq!(
        gone,
        [
            "AMD_md_ART",
            "AMD_md_BND",
            "AMD_md_CAT",
            "AMD_md_CSL",
            "AMD_md_DEF",
            "AMD_md_LOG",
            "md"
        ]
    );
    let mut tables: Vec<String> = std::fs::read_dir(&work)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.ends_with(".gdbtable").then(|| n[..9].to_string())
        })
        .collect();
    tables.sort();
    let want = [
        "a00000001",
        "a00000002",
        "a00000003",
        "a00000004",
        "a00000005",
        "a00000006",
        "a00000007",
        "a00000009",
        "a0000000a",
        "a00000011",
    ];
    assert_eq!(tables, want);
    let gdb = Geodatabase::open(&work).unwrap();
    assert!(
        gdb.domains()
            .iter()
            .any(|d| d.name == "MosaicCatalogItemCategoryDomain")
    );
    std::fs::remove_dir_all(&work).ok();
}
