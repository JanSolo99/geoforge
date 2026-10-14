//! `gf` writing GeoPackages as ArcGIS Pro 3.7 does: the parcels, roads and wells
//! shapefiles copied into one GeoPackage must equal, table by table and row by
//! row, testdata/gpkg/arcgis_wells.gpkg (made by ArcGIS's Copy Features into a
//! GeoPackage from Create SQLite Database) apart from gpkg_contents.last_change,
//! and pass SQLite's integrity and R*Tree checks.

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_gpkg::sqlite::{Database, SqlValue};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn copy_features_into_geopackage_like_arcgis() {
    let dir = std::env::temp_dir().join(format!("geoforge-gpkg-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.gpkg");
    for layer in ["parcels", "roads", "wells"] {
        let run = Command::new(env!("CARGO_BIN_EXE_gf"))
            .arg("copy-features")
            .arg(root().join(format!("testdata/geoproc/{layer}.shp")))
            .arg(out.join(layer))
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let ours = Database::open(&out).unwrap();
    let arcgis = Database::open(&root().join("testdata/gpkg/arcgis_wells.gpkg")).unwrap();
    let schema = |db: &Database| -> Vec<(String, String, String, Option<String>)> {
        db.objects()
            .iter()
            .map(|o| {
                (
                    o.kind.clone(),
                    o.name.clone(),
                    o.table.clone(),
                    o.sql.clone(),
                )
            })
            .collect()
    };
    assert_eq!(schema(&ours), schema(&arcgis), "schema");
    for t in arcgis.tables() {
        if t.root_page == 0 {
            continue;
        }
        let mut a = arcgis.select(t).unwrap();
        let mut b = ours.select(ours.table(&t.name).unwrap()).unwrap();
        if t.name == "gpkg_contents" {
            for row in a.iter_mut().chain(b.iter_mut()) {
                row.1[4] = SqlValue::Null; // last_change
            }
        }
        assert_eq!(a.len(), b.len(), "{}", t.name);
        for (x, y) in a.iter().zip(&b) {
            assert!(x == y, "{} rowid {}: {:?} vs {:?}", t.name, x.0, x.1, y.1);
        }
    }
    if Command::new("sqlite3").arg("-version").output().is_ok() {
        let check = Command::new("sqlite3")
            .arg(&out)
            .arg(
                "PRAGMA integrity_check; SELECT rtreecheck('rtree_parcels_Shape'), \
                 rtreecheck('rtree_roads_Shape'), rtreecheck('rtree_wells_Shape');",
            )
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&check.stdout).trim(),
            "ok\nok|ok|ok"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Export Table into a GeoPackage: attribute tables (no geometry column, no R*Tree)
/// equal, row for row, testdata/gpkg/arcgis_tables.gpkg made by ArcGIS's Export Table.
#[test]
fn export_table_into_geopackage_like_arcgis() {
    let dir = std::env::temp_dir().join(format!("geoforge-gpkg-tab-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.gpkg");
    for layer in ["parcels", "roads", "wells"] {
        let run = Command::new(env!("CARGO_BIN_EXE_gf"))
            .arg("export-table")
            .arg(root().join(format!("testdata/geoproc/{layer}.shp")))
            .arg(out.join(format!("{layer}_tab")))
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let ours = Database::open(&out).unwrap();
    let arcgis = Database::open(&root().join("testdata/gpkg/arcgis_tables.gpkg")).unwrap();
    let schema = |db: &Database| -> Vec<(String, String, String, Option<String>)> {
        db.objects()
            .iter()
            .map(|o| {
                (
                    o.kind.clone(),
                    o.name.clone(),
                    o.table.clone(),
                    o.sql.clone(),
                )
            })
            .collect()
    };
    assert_eq!(schema(&ours), schema(&arcgis), "schema");
    for t in arcgis.tables() {
        if t.root_page == 0 {
            continue;
        }
        let mut a = arcgis.select(t).unwrap();
        let mut b = ours.select(ours.table(&t.name).unwrap()).unwrap();
        if t.name == "gpkg_contents" {
            for row in a.iter_mut().chain(b.iter_mut()) {
                row.1[4] = SqlValue::Null; // last_change
            }
        }
        assert_eq!(a.len(), b.len(), "{}", t.name);
        for (x, y) in a.iter().zip(&b) {
            assert!(x == y, "{} rowid {}: {:?} vs {:?}", t.name, x.0, x.1, y.1);
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Frequency and Statistics tables written into a GeoPackage equal ArcGIS's.
#[test]
fn statistics_tables_into_geopackage_like_arcgis() {
    let dir = std::env::temp_dir().join(format!("geoforge-gpkg-stat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.gpkg");
    let runs: [(&str, &[&str]); 3] = [
        (
            "freq_zone",
            &[
                "frequency",
                "testdata/geoproc/parcels.shp",
                "<out>",
                "ZONE",
                "--summary-fields",
                "VALUE",
            ],
        ),
        (
            "stats_zone",
            &[
                "statistics",
                "testdata/geoproc/parcels.shp",
                "<out>",
                "VALUE SUM;VALUE MEAN;VALUE STD;PARCEL_ID COUNT;NAME FIRST",
                "--case-field",
                "ZONE",
            ],
        ),
        (
            "stats_median",
            &[
                "statistics",
                "testdata/geoproc/wells.shp",
                "<out>",
                "DEPTH MEDIAN;DEPTH VARIANCE;DEPTH UNIQUE",
            ],
        ),
    ];
    for (name, args) in runs {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_gf"));
        for a in args {
            if *a == "<out>" {
                cmd.arg(out.join(name));
            } else if a.starts_with("testdata/") {
                cmd.arg(root().join(a));
            } else {
                cmd.arg(a);
            }
        }
        let run = cmd.output().unwrap();
        assert!(
            run.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let ours = Database::open(&out).unwrap();
    let arcgis = Database::open(&root().join("testdata/gpkg/arcgis_stat_tables.gpkg")).unwrap();
    for t in arcgis.tables() {
        if t.root_page == 0
            || !["freq_zone", "stats_zone", "stats_median"].contains(&t.name.as_str())
        {
            continue;
        }
        let ours_table = ours.table(&t.name).unwrap();
        assert_eq!(t.sql, ours_table.sql, "{} schema", t.name);
        let a = arcgis.select(t).unwrap();
        let b = ours.select(ours_table).unwrap();
        assert_eq!(a.len(), b.len(), "{}", t.name);
        for (x, y) in a.iter().zip(&b) {
            assert!(x == y, "{} rowid {}: {:?} vs {:?}", t.name, x.0, x.1, y.1);
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The same tables into a new file geodatabase: fields (names, types, lengths) and
/// rows equal those of ArcGIS's testdata/gdb-reference/stat_tables.gdb.
#[test]
fn statistics_tables_into_geodatabase_like_arcgis() {
    use geoforge_core::FeatureClass;
    use geoforge_filegdb::Geodatabase;
    let dir = std::env::temp_dir().join(format!("geoforge-gdb-stat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let runs: [(&str, &[&str]); 3] = [
        (
            "freq_zone",
            &[
                "frequency",
                "testdata/geoproc/parcels.shp",
                "<out>",
                "ZONE",
                "--summary-fields",
                "VALUE",
            ],
        ),
        (
            "stats_zone",
            &[
                "statistics",
                "testdata/geoproc/parcels.shp",
                "<out>",
                "VALUE SUM;VALUE MEAN;VALUE STD;PARCEL_ID COUNT;NAME FIRST",
                "--case-field",
                "ZONE",
            ],
        ),
        (
            "stats_median",
            &[
                "statistics",
                "testdata/geoproc/wells.shp",
                "<out>",
                "DEPTH MEDIAN;DEPTH VARIANCE;DEPTH UNIQUE",
            ],
        ),
    ];
    // A new geodatabase takes one table per run here: write each to its own and compare.
    for (name, args) in runs {
        let gdb = dir.join(format!("{name}.gdb"));
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_gf"));
        for a in args {
            if *a == "<out>" {
                cmd.arg(gdb.join(name));
            } else if a.starts_with("testdata/") {
                cmd.arg(root().join(a));
            } else {
                cmd.arg(a);
            }
        }
        let run = cmd.output().unwrap();
        assert!(
            run.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        let ours = Geodatabase::open(&gdb).unwrap().open_table(name).unwrap();
        let theirs = Geodatabase::open(root().join("testdata/gdb-reference/stat_tables.gdb"))
            .unwrap()
            .open_table(name)
            .unwrap();
        let describe = |t: &dyn FeatureClass| -> Vec<(String, String, u32)> {
            t.fields()
                .iter()
                .map(|f| (f.name.clone(), format!("{:?}", f.field_type), f.length))
                .collect()
        };
        assert_eq!(describe(&ours), describe(&theirs), "{name} fields");
        let rows = |t: &dyn FeatureClass| -> Vec<Vec<geoforge_core::Value>> {
            t.features().map(|f| f.unwrap().attributes).collect()
        };
        assert_eq!(rows(&ours), rows(&theirs), "{name} rows");
    }
    std::fs::remove_dir_all(&dir).ok();
}
