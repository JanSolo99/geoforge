//! In-place raster metadata against saved ArcGIS controls, with file-preservation guards.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use unigis_raster::{read_geotiff, stats_metadata};
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn root(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ug-setprops-{}-{label}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}
fn beside(p: &Path, suffix: &str) -> PathBuf {
    let mut p = p.as_os_str().to_owned();
    p.push(suffix);
    p.into()
}
fn copy(source: &Path, target: &Path) {
    std::fs::copy(source, target).unwrap();
    let aux = beside(source, ".aux.xml");
    if aux.exists() {
        std::fs::copy(aux, beside(target, ".aux.xml")).unwrap();
    }
}
fn run(path: &Path, args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "set_raster_properties"])
        .arg(format!("in_raster={}", path.display()))
        .args(args)
        .output()
        .unwrap()
}
fn success(out: Output) {
    assert!(out.status.success(), "{out:?}");
}
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
#[test]
fn matches_all_saved_arcgis_metadata_and_pixels() {
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repo().join("testdata/arcgis-reference/manifest.json")).unwrap(),
    )
    .unwrap();
    let mut count = 0;
    for (name, case) in manifest["cases"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(n, _)| n.starts_with("srp_"))
    {
        count += 1;
        let dir = root(name);
        let input = dir.join("input.tif");
        let source = repo().join(case["run"]["params"]["in_raster"].as_str().unwrap());
        copy(&source, &input);
        let before = snapshot(&dir);
        let source_before = std::fs::read(&source).unwrap();
        let args: Vec<_> = case["run"]["params"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| *k != "in_raster")
            .map(|(k, v)| format!("{k}={}", v.as_str().unwrap()))
            .collect();
        let out = run(&input, &args);
        if case["status"] == "failed" {
            assert!(!out.status.success(), "{name}: {out:?}");
            assert_eq!(snapshot(&dir), before, "{name}");
        } else {
            success(out);
            let reference = repo().join(format!("testdata/arcgis-reference/{name}.tif"));
            let a = read_geotiff(&input).unwrap();
            let b = read_geotiff(&reference).unwrap();
            assert_eq!(
                (a.width, a.height, a.data_type, a.epsg, a.transform),
                (b.width, b.height, b.data_type, b.epsg, b.transform),
                "{name}"
            );
            assert_eq!(
                a.nodata.map(f64::to_bits),
                b.nodata.map(f64::to_bits),
                "{name}"
            );
            assert_eq!(a.bands, b.bands, "{name}");
            assert_eq!(
                stats_metadata::read(&input).unwrap(),
                stats_metadata::read(&reference).unwrap(),
                "{name}"
            );
            assert_eq!(
                unigis_raster::setprops::source_type(&input).unwrap(),
                unigis_raster::setprops::source_type(&reference).unwrap(),
                "{name}"
            );
            let original = read_geotiff(&source).unwrap();
            assert_eq!(a.bands, original.bands, "{name}: changed stored pixels");
            if !args.iter().any(|p| p.starts_with("nodata=")) {
                assert_eq!(
                    std::fs::read(&input).unwrap(),
                    source_before,
                    "{name}: changed TIFF bytes"
                );
            }
        }
        assert_eq!(std::fs::read(&source).unwrap(), source_before);
        std::fs::remove_dir_all(dir).unwrap();
    }
    assert!(count >= 25, "no saved Set Raster Properties cases");
}
#[test]
fn updates_retain_unrelated_xml_and_existing_ancillary_statistics() {
    let dir = root("retain");
    let input = dir.join("input.tif");
    copy(&repo().join("testdata/raster/classes_u8.tif"), &input);
    let xml = "<PAMDataset note='a>b'><Metadata domain='CUSTOM'><MDI key='DataType'>keep</MDI></Metadata><!-- retain --><PAMRasterBand band='1'><Histograms><Opaque token='keep'/></Histograms><Metadata><MDI key='STATISTICS_COUNT'>123.000000</MDI><MDI key='STATISTICS_MEDIAN'>7</MDI><MDI key='custom'>x&amp;y</MDI></Metadata></PAMRasterBand></PAMDataset>";
    std::fs::write(beside(&input, ".aux.xml"), xml).unwrap();
    let tif = std::fs::read(&input).unwrap();
    success(run(
        &input,
        &[
            "statistics=1 1 250 20 3".into(),
            "data_type=THEMATIC".into(),
        ],
    ));
    success(run(&input, &["data_type=ELEVATION".into()]));
    let out = std::fs::read_to_string(beside(&input, ".aux.xml")).unwrap();
    for fragment in [
        "note='a>b'",
        "<Metadata domain='CUSTOM'><MDI key='DataType'>keep</MDI></Metadata>",
        "<!-- retain -->",
        "<Histograms><Opaque token='keep'/></Histograms>",
        "<MDI key='custom'>x&amp;y</MDI>",
    ] {
        assert!(out.contains(fragment), "{fragment}: {out}");
    }
    let stats = stats_metadata::read(&input).unwrap();
    assert_eq!(stats[0]["STATISTICS_COUNT"], "123.000000");
    assert_eq!(stats[0]["STATISTICS_MEDIAN"], "7");
    assert_eq!(std::fs::read(&input).unwrap(), tif);
    assert_eq!(
        unigis_raster::setprops::source_type(&input).unwrap(),
        "Elevation"
    );
    let value = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "get_raster_properties"])
        .arg(format!("input={}", input.display()))
        .arg("property_type=SOURCETYPE")
        .output()
        .unwrap();
    assert!(value.status.success(), "{value:?}");
    assert_eq!(String::from_utf8_lossy(&value.stdout).trim(), "Elevation");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn all_invalid_or_unsupported_options_preserve_inputs_and_sidecars() {
    let dir = root("errors");
    let input = dir.join("input.tif");
    copy(&repo().join("testdata/raster/classes_u8.tif"), &input);
    std::fs::write(
        beside(&input, ".aux.xml"),
        "<PAMDataset><Metadata><MDI key='keep'>yes</MDI></Metadata></PAMDataset>",
    )
    .unwrap();
    let before = snapshot(&dir);
    for arg in [
        "statistics=1 1 2 3",
        "statistics=2 1 2 3 4",
        "statistics=1 NaN 2 3 4",
        "statistics=1 -1 2 3 4",
        "data_type=NO",
        "nodata=2 3",
        "nodata=1 NaN",
        "nodata=1 2 3",
        "nodata=1 #",
        "stats_file=stats.xml",
        "key_properties=SensorName demo",
        "multidimensional_info=t StdTime 2020 2021",
        "output=output.tif",
    ] {
        let out = run(
            &input,
            &[
                "data_type=ELEVATION".into(),
                "statistics=1 1 250 20 3".into(),
                arg.into(),
            ],
        );
        assert!(!out.status.success(), "{arg}: {out:?}");
        assert_eq!(snapshot(&dir), before, "{arg}");
    }
    for xml in [
        "malformed",
        "<PAMDataset><PAMRasterBand band='1'/><PAMRasterBand band='1'/></PAMDataset>",
        "<PAMDataset><Metadata/><Metadata/></PAMDataset>",
    ] {
        std::fs::write(beside(&input, ".aux.xml"), xml).unwrap();
        let before = snapshot(&dir);
        assert!(
            !run(&input, &["nodata=1 2".into(), "data_type=THEMATIC".into()])
                .status
                .success()
        );
        assert_eq!(snapshot(&dir), before);
    }
    std::fs::remove_file(beside(&input, ".aux.xml")).unwrap();
    std::fs::create_dir(beside(&input, ".aux.xml")).unwrap();
    let before = std::fs::read(&input).unwrap();
    assert!(
        !run(
            &input,
            &["nodata=1 2".into(), "statistics=1 1 2 3 4".into()]
        )
        .status
        .success()
    );
    assert_eq!(std::fs::read(&input).unwrap(), before);
    assert!(beside(&input, ".aux.xml").is_dir());
    std::fs::remove_dir_all(dir).unwrap();
    let dir = root("multi-errors");
    let input = dir.join("input.tif");
    copy(&repo().join("testdata/raster/rgb_pixel_u8.tif"), &input);
    let before = snapshot(&dir);
    assert!(
        !run(&input, &["nodata=1 2".into(), "data_type=THEMATIC".into()])
            .status
            .success()
    );
    assert_eq!(snapshot(&dir), before);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn classic_big_endian_nodata_and_bigtiff_failure_safety() {
    if Command::new("gdal_translate")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let dir = root("tiff-layouts");
    let source = repo().join("testdata/raster/classes_u8.tif");
    for (label, options, valid) in [
        ("be", vec!["-co", "ENDIANNESS=BIG"], true),
        ("big", vec!["-co", "BIGTIFF=YES"], false),
    ] {
        let input = dir.join(format!("{label}.tif"));
        let out = Command::new("gdal_translate")
            .arg("-q")
            .args(options)
            .arg(&source)
            .arg(&input)
            .output()
            .unwrap();
        success(out);
        let before = snapshot(&dir);
        let original = read_geotiff(&input).unwrap();
        let out = run(
            &input,
            &["nodata=1 2.5".into(), "statistics=1 1 250 20 3".into()],
        );
        if valid {
            success(out);
            let r = read_geotiff(&input).unwrap();
            assert_eq!(r.nodata, Some(2.));
            assert_eq!(r.bands, original.bands);
            let out = Command::new("gdalinfo")
                .arg("-json")
                .arg(&input)
                .output()
                .unwrap();
            assert!(out.status.success());
            let info: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(info["bands"][0]["noDataValue"], 2.);
        } else {
            assert!(!out.status.success(), "{out:?}");
            assert_eq!(snapshot(&dir), before);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
