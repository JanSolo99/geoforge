//! Projection assignment changes metadata only; invalid requests preserve files.
use std::path::Path;
use std::process::{Command, Output};
use unigis_raster::{Compression, DataType, Raster, read_geotiff, write_geotiff};

fn assign(input: &Path, output: &Path, code: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "define_projection"])
        .arg(format!("input={}", input.display()))
        .arg(format!("output={}", output.display()))
        .arg(format!("coordinate_system={code}"))
        .output()
        .unwrap()
}

#[test]
fn raster_assignment_preserves_all_bands_and_rejects_unsafe_requests() {
    let dir = std::env::temp_dir().join(format!("ug-raster-projection-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("source.TIFF");
    let output = dir.join("out.tif");
    let mut raster = Raster::new(2, 2, DataType::F64, [140., 0.25, 0., -30., 0., -0.25]);
    raster.epsg = Some(4326);
    raster.nodata = Some(-9999.);
    raster.bands = vec![
        vec![-9999., 1.234567890123, 2., 3.],
        vec![4., 5., 6., -9999.],
    ];
    write_geotiff(&input, &raster, Compression::None).unwrap();
    let original = std::fs::read(&input).unwrap();
    let result = assign(&input, &output, "epsg:7844");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let back = read_geotiff(&output).unwrap();
    assert_eq!(back.epsg, Some(7844));
    assert_eq!(back.transform, raster.transform);
    assert_eq!(back.bands, raster.bands);
    assert_eq!(back.data_type, raster.data_type);
    assert_eq!(back.nodata, raster.nodata);
    let saved = std::fs::read(&output).unwrap();
    for code in ["0", "65535", "999999", "EPSG:12345", "garbage", "EPSG:4978"] {
        let result = assign(&input, &output, code);
        assert!(!result.status.success(), "accepted {code}");
        assert_eq!(std::fs::read(&input).unwrap(), original);
        assert_eq!(std::fs::read(&output).unwrap(), saved);
    }
    for path in [&input, &output] {
        let aux = format!("{}.aux.xml", path.display());
        std::fs::write(&aux, "<PAMDataset><SRS>EPSG:4326</SRS></PAMDataset>").unwrap();
        let result = assign(&input, &output, "3857");
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("auxiliary SRS"));
        assert_eq!(std::fs::read(&input).unwrap(), original);
        assert_eq!(std::fs::read(&output).unwrap(), saved);
        std::fs::remove_file(aux).unwrap();
    }
    let result = assign(&input, &input, "32633");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(read_geotiff(&input).unwrap().bands, raster.bands);
    assert_eq!(read_geotiff(&input).unwrap().epsg, Some(32633));
    std::fs::remove_dir_all(dir).unwrap();
}
