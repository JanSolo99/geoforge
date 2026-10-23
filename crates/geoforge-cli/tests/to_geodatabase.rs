//! End to end: `ug copy-features` every test shapefile and geodatabase feature
//! class into a new .gdb, then check the copy against the source, against GDAL,
//! and that its spatial index never misses a feature.

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::{Envelope, Feature, FeatureClass, GeometryKind, Value};
use geoforge_filegdb::{Geodatabase, Table};
use geoforge_shapefile::ShapefileReader;
use geoforge_testkit::{
    compare_feature_tol, compare_geometries_tol, gdal_version, ogr_layer, ogrinfo_available,
};

fn inputs(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.join("a00000001.gdbtable").is_file() {
            if let Ok(gdb) = Geodatabase::open(&p) {
                out.extend(gdb.entries().iter().map(|e| p.join(&e.name)));
            }
        } else if p.is_dir() {
            inputs(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("shp")) {
            out.push(p);
        }
    }
}

fn open(path: &Path) -> Option<Box<dyn FeatureClass>> {
    let parent = path.parent()?;
    if parent.join("a00000001.gdbtable").is_file() {
        let t = Geodatabase::open(parent)
            .ok()?
            .open_table(path.file_name()?.to_str()?)
            .ok()?;
        Some(Box::new(t))
    } else {
        Some(Box::new(ShapefileReader::open(path).ok()?))
    }
}

fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Single(x), Value::Single(y)) => x == y || (x.is_nan() && y.is_nan()),
        (Value::Double(x), Value::Double(y)) => x == y || (x.is_nan() && y.is_nan()),
        (Value::Date(x), Value::Date(y)) => (*x - *y).num_milliseconds().abs() <= 1,
        // Legacy dates are read rounded to the second, so a re-read can shift by one.
        _ => a == b,
    }
}

#[test]
fn copies_into_new_geodatabases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    if !ogrinfo_available() {
        eprintln!("skipping: needs ogrinfo");
        return;
    }
    let mut paths = Vec::new();
    inputs(&root, &mut paths);
    let out_dir = std::env::temp_dir().join(format!("gf-to-gdb-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let (mut copied, mut unsupported, mut failures, mut domains_checked) = (0, 0, 0, 0);
    for (n, src_path) in paths.iter().enumerate() {
        let Some(src) = open(src_path) else { continue };
        let Some(kind) = src.geometry_kind() else {
            continue;
        };
        let Ok(src_features) = src.features().collect::<Result<Vec<Feature>, _>>() else {
            continue;
        };
        if src_features
            .iter()
            .any(|f| f.geometry.as_ref().is_some_and(|g| g.kind != kind))
        {
            continue; // mixed geometry types (non-conformant input)
        }
        let label = src_path
            .strip_prefix(&root)
            .unwrap_or(src_path)
            .display()
            .to_string();
        let gdb = out_dir.join(format!("{n}.gdb"));
        let run = Command::new(env!("CARGO_BIN_EXE_ug"))
            .arg("copy-features")
            .arg(src_path)
            .arg(gdb.join("copy"))
            .output()
            .unwrap();
        if !run.status.success() {
            let stderr = String::from_utf8_lossy(&run.stderr);
            if stderr.contains("unsupported") {
                unsupported += 1;
                eprintln!("~ {label}: {}", stderr.trim());
            } else {
                failures += 1;
                eprintln!("✗ {label}: {}", stderr.trim());
            }
            continue;
        }
        copied += 1;
        let mut diffs = Vec::new();
        let out = Geodatabase::open(&gdb).and_then(|g| g.open_table("copy"));
        let Ok(out) = out else {
            failures += 1;
            eprintln!("✗ {label}: cannot reopen copy");
            continue;
        };
        let dst: Vec<Feature> = Table::features(&out).map(Result::unwrap).collect();
        // Coordinates are snapped to the storage resolution, as in ArcGIS.
        let tol = out.geometry_def().map_or(1e-9, |d| 1.0 / d.xy_scale);
        if dst.len() != src_features.len() {
            diffs.push(format!("count {} -> {}", src_features.len(), dst.len()));
        }
        for (a, b) in src_features.iter().zip(&dst) {
            let mut g = a.geometry.clone();
            if let Some(g) = &mut g
                && g.kind == GeometryKind::Polygon
                && g.curves.is_empty()
            {
                g.orient_rings_esri();
            }
            if let Some(d) = compare_geometries_tol(g.as_ref(), b.geometry.as_ref(), tol) {
                diffs.push(format!("OID {}: {d}", a.oid));
            }
            for (i, (x, y)) in a.attributes.iter().zip(&b.attributes).enumerate() {
                if !same_value(x, y) {
                    diffs.push(format!(
                        "OID {} field {}: {x:?} -> {y:?}",
                        a.oid,
                        src.fields()[i].name
                    ));
                }
            }
        }
        // GDAL must read the copy as we do. Exception: GDAL repairs this polygon (drops
        // repeated vertices and splits rings); we, like ArcGIS, keep what is stored.
        let gdal_repairs = src_path.ends_with("issue_14385.shp");
        let gdal_too_old = out.requires_pro_3_2() && !gdal_version().is_some_and(|v| v >= (3, 9));
        match ogr_layer(&gdb, Some("copy")) {
            _ if gdal_repairs || gdal_too_old => {}
            Ok(gdal) => {
                let names: Vec<&str> = out.fields().iter().map(|f| f.name.as_str()).collect();
                for f in &dst {
                    match gdal.features.get(&f.oid) {
                        Some(g) => diffs.extend(compare_feature_tol(f, &names, g, Some(tol))),
                        None => diffs.push(format!("GDAL lacks OID {}", f.oid)),
                    }
                }
            }
            Err(e) => diffs.push(format!("GDAL: {e}")),
        }
        // Attribute domains used by the copied fields come along unchanged.
        if let Some(parent) = src_path
            .parent()
            .filter(|p| p.join("a00000001.gdbtable").is_file())
        {
            let (src_gdb, out_gdb) = (
                Geodatabase::open(parent).unwrap(),
                Geodatabase::open(&gdb).unwrap(),
            );
            for name in src.fields().iter().filter_map(|f| f.domain.as_deref()) {
                if src_gdb.domain(name) != out_gdb.domain(name) {
                    diffs.push(format!("domain {name} not carried over intact"));
                }
            }
            domains_checked += src.fields().iter().filter(|f| f.domain.is_some()).count();
        }
        // The spatial index we wrote must find every feature by its own envelope.
        for f in &dst {
            let Some(e) = f
                .geometry
                .as_ref()
                .map(|g| g.envelope())
                .filter(|e: &Envelope| !e.is_empty())
            else {
                continue;
            };
            match out.spatial_index_candidates(&e) {
                Ok(Some(c)) if c.binary_search(&(f.oid as u64)).is_ok() => {}
                other => diffs.push(format!("spatial index misses OID {} ({other:?})", f.oid)),
            }
        }
        if !diffs.is_empty() {
            failures += 1;
            eprintln!("✗ {label}: {} differences", diffs.len());
            diffs
                .iter()
                .take(4)
                .for_each(|d| eprintln!("    {}", d.chars().take(300).collect::<String>()));
        }
    }
    eprintln!(
        "{copied} copied, {unsupported} unsupported, {failures} with differences, {domains_checked} field domains carried over"
    );
    std::fs::remove_dir_all(&out_dir).ok();
    assert!(copied > 100 && domains_checked > 0);
    assert_eq!(failures, 0);
}
