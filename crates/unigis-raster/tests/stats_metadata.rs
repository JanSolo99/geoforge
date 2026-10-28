//! Embedded statistics are visible without PAM and PAM overrides remain band-specific.
use std::path::Path;
use std::process::Command;
use unigis_raster::{DataType, Raster, WriteOptions, stats_metadata, write_geotiff_with};

#[test]
fn embedded_statistics_roundtrip_through_native_and_gdal_with_pam_overrides() {
    let folder = std::env::temp_dir().join(format!("unigis-stored-stats-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let mut raster = Raster::new(2, 2, DataType::F32, [0., 1., 0., 2., 0., -1.]);
    raster.nodata = Some(-9999.);
    raster.bands = vec![vec![1., -9999., 3., 4.], vec![10., 20., 30., 40.]];
    let metadata = stats_metadata::for_raster(&raster).unwrap();
    for big in [false, true] {
        let path = folder.join(format!("embedded-{big}.tif"));
        write_geotiff_with(
            &path,
            &raster,
            &WriteOptions {
                bigtiff: big,
                tile: big.then_some((16, 16)),
                gdal_metadata: Some(metadata.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        let read = stats_metadata::read(&path).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0]["STATISTICS_COUNT"], "3.000000");
        assert_eq!(read[1]["STATISTICS_COUNT"], "4.000000");
        assert_eq!(
            unigis_raster::read_geotiff(&path).unwrap().statistics,
            Some((1., 4.))
        );
        assert!(!Path::new(&format!("{}.aux.xml", path.display())).exists());
        let output = Command::new("gdalinfo")
            .arg("-json")
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let gdal: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            gdal["bands"][0]["metadata"][""]["STATISTICS_COUNT"],
            "3.000000"
        );
        assert_eq!(gdal["bands"][1]["metadata"][""]["STATISTICS_MEAN"], "25");
        let aux = format!("{}.aux.xml", path.display());
        std::fs::write(&aux, "<PAMDataset><PAMRasterBand band=\"2\"><Metadata><MDI key=\"STATISTICS_MEAN\">99</MDI></Metadata></PAMRasterBand><PAMRasterBand band=\"999999999\"><Metadata><MDI key=\"STATISTICS_MEAN\">1</MDI></Metadata></PAMRasterBand></PAMDataset>").unwrap();
        let read = stats_metadata::read(&path).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0]["STATISTICS_MEAN"], "2.6666666666667");
        assert_eq!(read[1]["STATISTICS_MEAN"], "99");
        assert_eq!(read[1]["STATISTICS_COUNT"], "4.000000");
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn all_nodata_has_no_cache_and_text_is_escaped_without_becoming_xml() {
    let mut raster = Raster::new(1, 1, DataType::I16, [0., 1., 0., 1., 0., -1.]);
    raster.nodata = Some(-1.);
    raster.bands[0][0] = -1.;
    assert_eq!(stats_metadata::for_raster(&raster), None);
    let mut band = stats_metadata::BandStatistics::new();
    band.insert("STATISTICS_EXCLUDEDVALUES".into(), "<bad>&\"'".into());
    let xml = stats_metadata::encode(&[band]).unwrap();
    assert!(xml.contains("&lt;bad&gt;&amp;&quot;&apos;"));
    assert!(!xml.contains("<bad>"));
}

#[test]
fn physical_nodata_override_keeps_logical_samples_and_statistics() {
    let folder = std::env::temp_dir().join(format!("unigis-raw-nodata-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let mut raster = Raster::new(3, 1, DataType::U8, [0., 1., 0., 1., 0., -1.]);
    raster.nodata = Some(0.);
    raster.bands[0] = vec![0., 3., 127.];
    for bigtiff in [false, true] {
        let path = folder.join(format!("raw-{bigtiff}.tif"));
        write_geotiff_with(
            &path,
            &raster,
            &WriteOptions {
                bigtiff,
                nodata_tag: Some(-0.25),
                gdal_metadata: stats_metadata::for_raster(&raster),
                ..Default::default()
            },
        )
        .unwrap();
        let read = unigis_raster::read_geotiff(&path).unwrap();
        assert_eq!(read.nodata, Some(-0.25));
        assert_eq!(read.bands, raster.bands);
        assert_eq!(
            stats_metadata::read(&path).unwrap()[0]["STATISTICS_COUNT"],
            "2.000000"
        );
        let output = Command::new("gdalinfo")
            .arg("-json")
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let gdal: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(gdal["bands"][0]["noDataValue"], -0.25);
        assert_eq!(gdal["bands"][0]["metadata"][""]["STATISTICS_MEAN"], "65");
    }
    std::fs::remove_dir_all(folder).unwrap();
}
