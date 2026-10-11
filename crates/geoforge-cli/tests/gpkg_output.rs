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
