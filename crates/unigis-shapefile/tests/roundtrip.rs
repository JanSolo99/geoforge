//! Copies every test shapefile with `ShapefileWriter`, then checks the copy against
//! GDAL and against the original (allowing for ArcGIS null substitution).

mod common;

use std::path::Path;

use common::{diff_file, gdal_divergence, shapefiles, testdata};
use unigis_core::Value;
use unigis_shapefile::{ShapefileReader, ShapefileWriter};
use unigis_testkit::{compare_geometries, ogrinfo_available};

fn copy(src: &ShapefileReader, dst: &Path) -> unigis_core::Result<()> {
    let st = src.shape_type();
    let kind = st.kind().expect("caller skips Null-type files");
    let mut w = ShapefileWriter::create(
        dst,
        kind,
        st.has_z(),
        st.has_m(),
        src.fields(),
        src.spatial_reference_wkt(),
    )?;
    for f in src.features() {
        w.write(&f?)?;
    }
    w.finish()
}

/// Equal as ArcGIS would read the copy back: numbers compare by value across
/// integer/float types (a `L` field becomes `N(5)`, read as Long — ArcGIS's own
/// Copy Features does the same), `F` columns keep 6 or 12 significant digits.
fn values_match(orig: &Value, copy: &Value) -> bool {
    let num = |v: &Value| match v {
        Value::SmallInteger(n) => Some(f64::from(*n)),
        Value::Integer(n) => Some(f64::from(*n)),
        Value::Single(n) => Some(f64::from(*n)),
        Value::Double(n) => Some(*n),
        _ => None,
    };
    match (num(orig), num(copy)) {
        (Some(a), Some(b)) => {
            let rel = if matches!(orig, Value::Single(_)) {
                1e-5
            } else {
                1e-11
            };
            a == b || (a - b).abs() <= rel * a.abs().max(1.0)
        }
        _ => orig == copy,
    }
}

fn diff_copy(orig_path: &Path, copy_path: &Path) -> Vec<String> {
    let orig = ShapefileReader::open(orig_path).unwrap();
    let copy = ShapefileReader::open(copy_path).unwrap();
    let mut diffs = Vec::new();
    let a: Vec<_> = orig.features().collect::<Result<_, _>>().unwrap();
    let b: Vec<_> = copy.features().collect::<Result<_, _>>().unwrap();
    if a.len() != b.len() {
        return vec![format!("feature count {} -> {}", a.len(), b.len())];
    }
    for (fa, fb) in a.iter().zip(&b) {
        // The writer orients rings the Esri way; compare against the oriented input.
        let oriented = fa.geometry.clone().map(|mut g| {
            g.orient_rings_esri();
            g
        });
        if let Some(d) = compare_geometries(oriented.as_ref(), fb.geometry.as_ref()) {
            diffs.push(format!("FID {}: {d}", fa.oid));
        }
        for (i, (va, vb)) in fa.attributes.iter().zip(&fb.attributes).enumerate() {
            if !values_match(va, vb) {
                diffs.push(format!(
                    "FID {} field {}: {va:?} -> {vb:?}",
                    fa.oid,
                    orig.fields()[i].name
                ));
            }
        }
    }
    diffs
}

#[test]
fn copies_match_original_and_gdal() {
    if !ogrinfo_available() {
        eprintln!("skipping: ogrinfo not installed");
        return;
    }
    let out_dir = std::env::temp_dir().join(format!("gf-roundtrip-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut failures = 0;
    let files = shapefiles(&testdata());
    for (i, path) in files.iter().enumerate() {
        let name = path
            .strip_prefix(testdata())
            .unwrap_or(path)
            .display()
            .to_string();
        let Ok(src) = ShapefileReader::open(path) else {
            continue;
        };
        let kind = src.shape_type().kind();
        let conformant = src
            .features()
            .all(|f| f.is_ok_and(|f| f.geometry.is_none_or(|g| Some(g.kind) == kind)));
        if kind.is_none() || !conformant {
            continue; // Null-type, corrupt or mixed-type inputs cannot be copied faithfully
        }
        let dst = out_dir.join(format!("{i}.shp"));
        if let Err(e) = copy(&src, &dst) {
            failures += 1;
            eprintln!("✗ {name}: copy failed: {e}");
            continue;
        }
        let mut diffs = diff_copy(path, &dst);
        match diff_file(&dst) {
            _ if gdal_divergence(path).is_some() => {}
            Ok(d) => diffs.extend(d.into_iter().map(|d| format!("GDAL on copy: {d}"))),
            Err(e) => diffs.push(format!("GDAL on copy: {e}")),
        }
        if !diffs.is_empty() {
            failures += 1;
            eprintln!("✗ {name}: {} differences", diffs.len());
            diffs.iter().take(5).for_each(|d| eprintln!("    {d}"));
        }
    }
    eprintln!("{} files, {failures} with differences", files.len());
    std::fs::remove_dir_all(&out_dir).ok();
    assert_eq!(failures, 0);
}
