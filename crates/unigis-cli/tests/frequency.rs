//! Unsupported frequency grids/counts and empty raster lists must preserve files.
use std::path::Path;
use std::process::Command;
use unigis_raster::{Compression, DataType, Raster, write_geotiff};

#[test]
fn frequency_refusals_preserve_sources_and_existing_output() {
    let dir = std::env::temp_dir().join(format!("ug-frequency-refuse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("source.tif");
    let mismatch = dir.join("mismatch.tif");
    let output = dir.join("out.tif");
    let mut raster = Raster::new(2, 2, DataType::U8, [0., 1., 0., 2., 0., -1.]);
    raster.epsg = Some(32633);
    raster.nodata = Some(1.);
    raster.bands[0] = vec![0., 1., 2., 3.];
    write_geotiff(&input, &raster, Compression::None).unwrap();
    raster.epsg = Some(3857);
    write_geotiff(&mismatch, &raster, Compression::None).unwrap();
    let original = std::fs::read(&input).unwrap();
    let other = std::fs::read(&mismatch).unwrap();
    let existing = b"existing output";
    std::fs::write(&output, existing).unwrap();
    let repeat = |n| vec![input.display().to_string(); n].join(";");
    for tool in [
        "equal_to_frequency",
        "greater_than_frequency",
        "less_than_frequency",
    ] {
        for (list, message) in [
            (";".to_owned(), "at least one"),
            (mismatch.display().to_string(), "matching grids"),
            (repeat(257), "256 comparison"),
        ] {
            let result = Command::new(env!("CARGO_BIN_EXE_ug"))
                .args(["run", tool])
                .arg(format!("input={}", input.display()))
                .arg(format!("rasters={list}"))
                .arg(format!("output={}", output.display()))
                .output()
                .unwrap();
            let error = String::from_utf8_lossy(&result.stderr);
            assert!(!result.status.success(), "accepted {tool}: {message}");
            assert!(error.contains(message), "{tool}: {error}");
            assert!(!error.contains("panicked"), "{error}");
            assert_eq!(std::fs::read(&input).unwrap(), original);
            assert_eq!(std::fs::read(&mismatch).unwrap(), other);
            assert_eq!(std::fs::read(&output).unwrap(), existing);
            assert!(!Path::new(&format!("{}.aux.xml", output.display())).exists());
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn empty_rank_and_popularity_lists_return_parameter_errors() {
    let dir = std::env::temp_dir().join(format!("ug-empty-rank-list-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("out.tif");
    for tool in ["rank", "popularity"] {
        let result = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", tool, "rasters=;", "rank=1"])
            .arg(format!("output={}", output.display()))
            .output()
            .unwrap();
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success());
        assert!(error.contains("000735"), "{tool}: {error}");
        assert!(!error.contains("panicked"), "{error}");
        assert!(!output.exists());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
