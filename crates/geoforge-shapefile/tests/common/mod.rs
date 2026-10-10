//! Shared helpers for the oracle and round-trip tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use geoforge_shapefile::ShapefileReader;
use geoforge_testkit::{compare_feature, ogr_layer};

pub fn testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

pub fn shapefiles(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(shapefiles(&p));
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("shp")) {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Differences between our reader and GDAL for one file.
pub fn diff_file(path: &Path) -> Result<Vec<String>, String> {
    let gdal = ogr_layer(path)?;
    let reader = ShapefileReader::open(path).map_err(|e| format!("we failed to open: {e}"))?;
    let names: Vec<&str> = reader.fields().iter().map(|f| f.name.as_str()).collect();
    let mut diffs = Vec::new();
    let gdal_names: Vec<&str> = gdal.fields.iter().map(|f| f.name.as_str()).collect();
    if names != gdal_names {
        diffs.push(format!("fields: ours {names:?}, GDAL {gdal_names:?}"));
    }
    let mut ours = 0;
    for fid in 0..reader.record_count() {
        let feature = match reader.get(fid) {
            Ok(Some(f)) => f,
            Ok(None) => continue, // deleted
            // We reject corrupt records; GDAL logs them and returns no geometry.
            Err(e) => {
                ours += 1;
                if gdal
                    .features
                    .get(&(fid as i64))
                    .is_none_or(|g| g.wkt.is_some())
                {
                    diffs.push(format!(
                        "FID {fid}: we failed ({e}) but GDAL read a geometry"
                    ));
                }
                continue;
            }
        };
        ours += 1;
        match gdal.features.get(&feature.oid) {
            Some(g) => diffs.extend(compare_feature(&feature, &names, g)),
            None => diffs.push(format!("FID {} not reported by GDAL", feature.oid)),
        }
    }
    if ours != gdal.features.len() {
        diffs.push(format!(
            "feature count: ours {ours}, GDAL {}",
            gdal.features.len()
        ));
    }
    Ok(diffs)
}

/// Files where GDAL deliberately differs from the raw file contents (which is what
/// ArcGIS, and we, return).
pub const GDAL_DIVERGENCES: &[(&str, &str)] = &[(
    "issue_14385.shp",
    "GDAL repairs a polygon whose rings it considers invalid, emitting 3 rings for 2 stored",
)];

pub fn gdal_divergence(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy();
    GDAL_DIVERGENCES
        .iter()
        .find(|(f, _)| *f == name)
        .map(|(_, why)| *why)
}
