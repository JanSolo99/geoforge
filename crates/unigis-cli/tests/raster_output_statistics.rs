//! Overwrites must not expose the old raster's cached statistics to later tools.
use std::path::{Path, PathBuf};
use std::process::Command;

fn copy(input: &Path, output: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "copy_raster"])
        .arg(format!("input={}", input.display()))
        .arg(format!("output={}", output.display()))
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn copy_raster_overwrites_drop_stale_stats_and_invalid_parameters_preserve_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folder =
        std::env::temp_dir().join(format!("unigis-raster-output-stats-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let output = folder.join("out.tif");
    let aux = PathBuf::from(format!("{}.aux.xml", output.display()));
    let input = root.join("testdata/raster/copy_statistics/small_cached.tif");
    let want = unigis_raster::read_geotiff(&input).unwrap();

    let result = copy(&root.join("testdata/raster/dem.tif"), &output, &[]);
    assert!(result.status.success(), "{:?}", result);
    assert!(aux.exists());
    let before = std::fs::read(&output).unwrap();
    let before_aux = std::fs::read(&aux).unwrap();
    let result = copy(&input, &output, &["pixel_type=INVALID"]);
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), before);
    assert_eq!(std::fs::read(&aux).unwrap(), before_aux);

    let result = copy(&input, &output, &[]);
    assert!(result.status.success(), "{:?}", result);
    assert!(
        !aux.exists(),
        "old DEM statistics survived a no-statistics copy"
    );
    let got = unigis_raster::read_geotiff(&output).unwrap();
    assert_eq!(got.bands, want.bands);
    assert_eq!(got.transform, want.transform);
    assert_eq!(got.nodata, want.nodata);
    assert_eq!(got.statistics, want.statistics);

    std::fs::copy(format!("{}.aux.xml", input.display()), &aux).unwrap();
    assert!(
        unigis_raster::read_geotiff(&output)
            .unwrap()
            .statistics
            .is_some()
    );
    let result = copy(&output, &output, &[]);
    assert!(result.status.success(), "{:?}", result);
    let got = unigis_raster::read_geotiff(&output).unwrap();
    assert_eq!(got.bands, want.bands);
    assert_eq!(got.statistics, want.statistics);
    assert!(!aux.exists());
    // A requested statistics write can also produce no sidecar for all-NoData.
    let result = copy(&root.join("testdata/raster/dem.tif"), &output, &[]);
    assert!(result.status.success(), "{:?}", result);
    assert!(aux.exists());
    let result = copy(
        &root.join("testdata/arcgis-reference/ascimp_all_nd_float.tif"),
        &output,
        &[],
    );
    assert!(result.status.success(), "{:?}", result);
    assert!(!aux.exists());
    let empty = unigis_raster::read_geotiff(&output).unwrap();
    assert!(empty.bands[0].iter().all(|&v| empty.is_nodata(v)));
    assert_eq!(empty.statistics, None);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn cached_copy_preserves_or_defaults_each_arcgis_statistics_field() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folder = std::env::temp_dir().join(format!("unigis-copy-cache-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    for case in [
        "minmax",
        "stale",
        "all",
        "missing_mean",
        "missing_stddev",
        "missing_count",
        "extra",
        "missing_mean_extra",
    ] {
        let output = folder.join(format!("{case}.tif"));
        let result = copy(
            &root.join(format!("testdata/raster/copy_cache/{case}.tif")),
            &output,
            &[],
        );
        assert!(result.status.success(), "{case}: {result:?}");
        assert!(
            !PathBuf::from(format!("{}.aux.xml", output.display())).exists(),
            "{case}"
        );
        let expected = unigis_raster::stats_metadata::read(
            &root.join(format!("testdata/arcgis-reference/cprcache_{case}.tif")),
        )
        .unwrap();
        let actual = unigis_raster::stats_metadata::read(&output).unwrap();
        assert_eq!(actual.len(), expected.len(), "{case}");
        for (got, want) in actual.iter().zip(&expected) {
            assert_eq!(got.len(), want.len(), "{case}: {got:?} vs {want:?}");
            for (key, value) in want {
                let stored = got
                    .get(key)
                    .unwrap_or_else(|| panic!("{case}: missing {key}"));
                match (stored.parse::<f64>(), value.parse::<f64>()) {
                    (Ok(a), Ok(b)) => assert!(
                        a == b || (a.is_nan() && b.is_nan()),
                        "{case} {key}: {a} vs {b}"
                    ),
                    _ => assert_eq!(stored, value, "{case} {key}"),
                }
            }
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}
