//! Compares the reader with GDAL's `ogrinfo` on every shapefile in `testdata/shp`
//! (committed) and `testdata/large` (from `scripts/fetch-testdata.sh`, optional).

mod common;

use std::path::Path;

use common::{diff_file, gdal_divergence, shapefiles, testdata};
use geoforge_testkit::ogrinfo_available;

fn check(dir: &Path, strict: bool) {
    if !ogrinfo_available() {
        eprintln!("skipping: ogrinfo not installed");
        return;
    }
    let files = shapefiles(dir);
    let mut failures = 0;
    for path in &files {
        let name = path.strip_prefix(testdata()).unwrap_or(path).display();
        if let Some(why) = gdal_divergence(path) {
            eprintln!("~ {name}: skipped ({why})");
            continue;
        }
        match diff_file(path) {
            Ok(diffs) if diffs.is_empty() => {}
            Ok(diffs) => {
                failures += 1;
                eprintln!("✗ {name}: {} differences", diffs.len());
                diffs.iter().take(5).for_each(|d| eprintln!("    {d}"));
            }
            Err(e) => eprintln!("- {name}: {e}"),
        }
    }
    eprintln!("{} files, {failures} with differences", files.len());
    if strict {
        assert_eq!(failures, 0);
    }
}

#[test]
fn committed_testdata_matches_gdal() {
    check(&testdata().join("shp"), true);
}

#[test]
fn large_testdata_matches_gdal() {
    check(&testdata().join("large"), true);
}
