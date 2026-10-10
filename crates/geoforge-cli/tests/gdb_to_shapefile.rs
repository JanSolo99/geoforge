//! End to end: `gf copy-features` every feature class of the test geodatabases to a
//! shapefile, then check the output against the source and against GDAL.

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::Value;
use geoforge_filegdb::Geodatabase;
use geoforge_shapefile::ShapefileReader;
use geoforge_testkit::{compare_feature, compare_geometries, ogr_layer, ogrinfo_available};

fn geodatabases(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.join("a00000001.gdbtable").is_file() {
            out.push(p);
        } else if p.is_dir() {
            out.extend(geodatabases(&p));
        }
    }
    out.sort();
    out
}

/// Source value as it should read back from the shapefile (as ArcGIS reads it).
fn expected(v: &Value) -> Value {
    match v {
        Value::Date(d) => Value::Date(d.date().and_hms_opt(0, 0, 0).unwrap()),
        Value::DateOnly(d) => Value::Date(d.and_hms_opt(0, 0, 0).unwrap()),
        Value::Guid(s) | Value::String(s) => {
            let mut end = s.len().min(254);
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            let t = s[..end].trim_end();
            // Empty text reads back as a single space (ArcGIS null substitution).
            Value::String(if t.is_empty() {
                " ".into()
            } else {
                t.to_owned()
            })
        }
        other => other.clone(),
    }
}

fn matches(src: &Value, out: &Value) -> bool {
    let num = |v: &Value| match v {
        Value::SmallInteger(n) => Some(f64::from(*n)),
        Value::Integer(n) => Some(f64::from(*n)),
        Value::Single(n) => Some(f64::from(*n)),
        Value::Double(n) => Some(*n),
        _ => None,
    };
    // F(13,11) keeps 6 significant digits, F(19,11) 12 (ArcGIS's formatting).
    let rel = if matches!(src, Value::Single(_)) {
        1e-5
    } else {
        1e-11
    };
    match (expected(src), out) {
        // ArcGIS null substitution: numbers 0, text " ", dates stay null.
        (Value::Null, Value::Null) => true,
        (Value::Null, o) => num(o) == Some(0.0) || *o == Value::String(" ".into()),
        (e, o) => match (num(&e), num(o)) {
            (Some(a), Some(b)) => a == b || (a - b).abs() <= rel * a.abs().max(1.0),
            _ => e == *o,
        },
    }
}

/// OBJECTIDs the CLI reported as skipped (WARNING 000595 [..]).
fn skipped_oids(stderr: &str) -> Vec<i64> {
    stderr
        .lines()
        .filter(|l| l.starts_with("WARNING 000595"))
        .filter_map(|l| {
            l.split_once('[')
                .map(|(_, r)| r.trim_end_matches(']').to_owned())
        })
        .flat_map(|list| {
            list.split(',')
                .filter_map(|n| n.trim().parse().ok())
                .collect::<Vec<i64>>()
        })
        .collect()
}

#[test]
fn geodatabase_feature_classes_export_to_shapefile() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/large/filegdb");
    if !ogrinfo_available() || !root.is_dir() {
        eprintln!("skipping: needs ogrinfo and scripts/fetch-testdata.sh");
        return;
    }
    let out_dir = std::env::temp_dir().join(format!("gf-gdb2shp-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let (mut exported, mut failures, mut rejected) = (0, 0, 0);
    for gdb_path in geodatabases(&root) {
        let Ok(gdb) = Geodatabase::open(&gdb_path) else {
            continue;
        };
        for entry in gdb.entries() {
            let Ok(table) = gdb.open_table(&entry.name) else {
                continue;
            };
            if table.geometry_kind().is_none() || table.features().any(|f| f.is_err()) {
                continue;
            }
            let label = format!(
                "{}/{}",
                gdb_path.file_name().unwrap().to_string_lossy(),
                entry.name
            );
            let out = out_dir.join(format!("{exported}_{failures}.shp"));
            let run = Command::new(env!("CARGO_BIN_EXE_gf"))
                .arg("copy-features")
                .arg(gdb_path.join(&entry.name))
                .arg(&out)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&run.stderr);
            if !run.status.success() {
                if stderr.contains("ERROR 000308") {
                    rejected += 1; // ArcGIS refuses these field types too
                } else {
                    failures += 1;
                    eprintln!("✗ {label}: {}", stderr.trim());
                }
                continue;
            }
            exported += 1;

            let mut diffs = Vec::new();
            let shp = ShapefileReader::open(&out).unwrap();
            let skipped = skipped_oids(&stderr);
            let src: Vec<_> = table
                .features()
                .map(Result::unwrap)
                .filter(|f| !skipped.contains(&f.oid))
                .collect();
            let dst: Vec<_> = shp.features().map(Result::unwrap).collect();
            if src.len() != dst.len() {
                diffs.push(format!("count {} -> {}", src.len(), dst.len()));
            }
            let kept: Vec<usize> = (0..table.fields().len())
                .filter(|&i| {
                    !stderr.contains(&format!("Field(s) {} will not", table.fields()[i].name))
                })
                .collect();
            for (a, b) in src.iter().zip(&dst) {
                let mut oriented = a.geometry.as_ref().map(|g| {
                    g.densify_curves(geoforge_core::curves::DEFAULT_MAX_ANGLE_STEP)
                        .unwrap()
                });
                if let Some(g) = &mut oriented {
                    g.orient_rings_esri();
                }
                // Shapefiles have no empty geometries: an empty shape becomes null.
                let oriented = oriented
                    .filter(|g| !g.xy.is_empty() || g.kind == geoforge_core::GeometryKind::Point);
                let written = b
                    .geometry
                    .clone()
                    .filter(|g| !g.xy.is_empty() || g.kind == geoforge_core::GeometryKind::Point);
                if let Some(d) = compare_geometries(oriented.as_ref(), written.as_ref()) {
                    diffs.push(format!("OID {}: {d}", a.oid));
                }
                for (j, &i) in kept.iter().enumerate() {
                    if !matches(&a.attributes[i], &b.attributes[j]) {
                        diffs.push(format!(
                            "OID {} field {}: {:?} -> {:?}",
                            a.oid,
                            table.fields()[i].name,
                            a.attributes[i],
                            b.attributes[j]
                        ));
                    }
                }
            }
            // And GDAL must read our output the same way we do.
            match ogr_layer(&out, None) {
                Ok(gdal) => {
                    let names: Vec<&str> = shp.fields().iter().map(|f| f.name.as_str()).collect();
                    for f in &dst {
                        match gdal.features.get(&f.oid) {
                            Some(g) => diffs.extend(compare_feature(f, &names, g)),
                            None => diffs.push(format!("GDAL lacks FID {}", f.oid)),
                        }
                    }
                }
                Err(e) => diffs.push(format!("GDAL: {e}")),
            }
            if !diffs.is_empty() {
                failures += 1;
                eprintln!("✗ {label}: {} differences", diffs.len());
                diffs.iter().take(4).for_each(|d| eprintln!("    {d}"));
            }
        }
    }
    eprintln!(
        "{exported} exported, {rejected} rejected like ArcGIS (ERROR 000308), {failures} with differences"
    );
    std::fs::remove_dir_all(&out_dir).ok();
    assert_eq!(failures, 0);
}
