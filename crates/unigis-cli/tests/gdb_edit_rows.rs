//! `ug delete-rows`, `delete-features`, `truncate-table` and the in-place tools (Add Field,
//! Calculate Field, Delete Field, Append) on a geodatabase ArcGIS Pro 3.7 wrote
//! (`testdata/gdb-reference/edit_arcgis.gdb`), checked with our reader and GDAL.

use std::path::{Path, PathBuf};
use std::process::Command;

use unigis_core::Value;
use unigis_filegdb::Geodatabase;
use unigis_testkit::{compare_feature, gdal_version, ogr_layer, ogrinfo_available};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gf(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "gf {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::remove_dir_all(to).ok();
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

fn oids(gdb: &Path, name: &str) -> Vec<i64> {
    Geodatabase::open(gdb)
        .unwrap()
        .open_table(name)
        .unwrap()
        .features()
        .map(|f| f.unwrap().oid)
        .collect()
}

fn assert_gdal_agrees(gdb: &Path, name: &str) {
    if !ogrinfo_available() || (name == "pts64" && gdal_version().is_none_or(|v| v < (3, 9))) {
        return;
    }
    let table = Geodatabase::open(gdb).unwrap().open_table(name).unwrap();
    let gdal = ogr_layer(gdb, Some(name)).unwrap();
    let names: Vec<&str> = table.fields().iter().map(|f| f.name.as_str()).collect();
    let mut n = 0;
    for f in table.features() {
        let f = f.unwrap();
        n += 1;
        let g = gdal.features.get(&f.oid).expect("GDAL has the row");
        let d = compare_feature(&f, &names, g);
        assert!(d.is_empty(), "{name} {}: {d:?}", f.oid);
    }
    assert_eq!(n, gdal.features.len(), "{name}");
}

#[test]
fn delete_rows_and_in_place_tools_on_an_arcgis_geodatabase() {
    let dir = std::env::temp_dir().join(format!("gf-gdb-edit-rows-{}", std::process::id()));
    let gdb = dir.join("e.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_arcgis.gdb"), &gdb);
    let path = |n: &str| gdb.join(n).to_str().unwrap().to_owned();

    // parcels: OBJECTIDs 1..20 without 4 and 9; CLASS = OBJECTID-1 mod 3.
    let out = gf(&[
        "delete-rows",
        &path("parcels"),
        "--where",
        "OBJECTID > 14 AND CLASS = 1",
    ]);
    assert!(out.contains("Deleted 2 rows"), "{out}"); // 17 and 20
    assert_eq!(
        oids(&gdb, "parcels"),
        [1, 2, 3, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 18, 19]
    );
    gf(&[
        "delete-features",
        &path("roads"),
        "--where",
        "RNAME = 'road 2'",
    ]);
    assert_eq!(oids(&gdb, "roads"), [1, 2, 4, 5, 6]);
    assert_gdal_agrees(&gdb, "parcels");
    assert_gdal_agrees(&gdb, "roads");

    // In-place tools rewrite the table; OBJECTIDs and the measures stay.
    gf(&[
        "add-field",
        &path("parcels"),
        "NOTE",
        "TEXT",
        "--length",
        "16",
    ]);
    gf(&[
        "calculate-field",
        &path("parcels"),
        "NOTE",
        "!NAME!.upper()",
    ]);
    gf(&["delete-field", &path("parcels"), "VALUE;SURVEYED"]);
    let t = Geodatabase::open(&gdb)
        .unwrap()
        .open_table("parcels")
        .unwrap();
    let names: Vec<&str> = t.fields().iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["Shape_Length", "Shape_Area", "NAME", "CLASS", "NOTE"]
    );
    for f in t.features() {
        let f = f.unwrap();
        assert_eq!(f.attributes[1], Value::Double(6400.0));
        let Value::String(name) = &f.attributes[2] else {
            panic!()
        };
        assert_eq!(f.attributes[4], Value::String(name.to_uppercase()));
    }
    drop(t);
    assert_eq!(oids(&gdb, "parcels").len(), 16);
    assert_gdal_agrees(&gdb, "parcels");

    // Append into the 64-bit OBJECTID class: new rows continue after the largest OBJECTID.
    let other = dir.join("other.gdb");
    copy_dir(
        &root().join("testdata/gdb-reference/edit_arcgis.gdb"),
        &other,
    );
    gf(&[
        "append",
        other.join("pts64").to_str().unwrap(),
        &path("pts64"),
    ]);
    assert_eq!(
        oids(&gdb, "pts64"),
        [1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    );
    assert_gdal_agrees(&gdb, "pts64");

    // Truncate Table empties the table and restarts its OBJECTIDs.
    gf(&["truncate-table", &path("log")]);
    assert!(oids(&gdb, "log").is_empty());
    let base = Geodatabase::open(&gdb).unwrap().table_base("log").unwrap();
    let mut ed = unigis_filegdb::TableEditor::open(base).unwrap();
    let row = unigis_core::Feature {
        oid: 0,
        geometry: None,
        attributes: vec![Value::String("again".into()), Value::Integer(1)],
    };
    assert_eq!(ed.insert(&row).unwrap(), 1);
    ed.finish().unwrap();
    assert_gdal_agrees(&gdb, "log");

    // A shapefile is rewritten without the rows.
    let shp_dir = dir.join("shp");
    std::fs::create_dir_all(&shp_dir).unwrap();
    for ext in ["shp", "shx", "dbf", "prj"] {
        let from = root().join(format!("testdata/geoproc/wells.{ext}"));
        if from.exists() {
            std::fs::copy(from, shp_dir.join(format!("wells.{ext}"))).unwrap();
        }
    }
    let shp = shp_dir.join("wells.shp");
    gf(&[
        "delete-features",
        shp.to_str().unwrap(),
        "--where",
        "FID < 10",
    ]);
    let n = shapefile_count(&shp);
    assert!(n > 0);
    assert!(
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["truncate-table", shp.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .code()
            != Some(0)
    );
    std::fs::remove_dir_all(&dir).ok();
}

fn shapefile_count(shp: &Path) -> usize {
    let before = std::fs::metadata(root().join("testdata/geoproc/wells.shx"))
        .unwrap()
        .len();
    let after = std::fs::metadata(shp.with_extension("shx")).unwrap().len();
    // 100-byte header, 8 bytes per record.
    let (b, a) = ((before - 100) / 8, (after - 100) / 8);
    assert_eq!(b - a, 10);
    a as usize
}
