//! Raster datasets in file geodatabases compared with GDAL's OpenFileGDB raster driver on GDAL's
//! own test geodatabases (`testdata/large/filegdb-raster`, from `scripts/fetch-testdata.sh`):
//! size, geotransform, bands, every valid pixel value, and which pixels are valid.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as Json;
use unigis_filegdb::{Compression, Geodatabase, PixelType};
use unigis_testkit::ogrinfo_available;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/large/filegdb-raster")
}

fn gdal(args: &[&str]) -> Option<String> {
    let out = Command::new(args[0]).args(&args[1..]).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// One band's values as GDAL reads them (`-b mask` for its validity mask).
fn gdal_band(name: &str, band: &str) -> Option<Vec<f64>> {
    let xyz = gdal(&[
        "gdal_translate",
        "-q",
        "-of",
        "XYZ",
        "-b",
        band,
        name,
        "/vsistdout/",
    ])?;
    xyz.lines()
        .map(|l| l.split_whitespace().nth(2).and_then(|v| v.parse().ok()))
        .collect()
}

/// Largest raster compared pixel by pixel.
const MAX_COMPARED: usize = 1 << 20;

#[test]
fn rasters_match_gdal() {
    if !ogrinfo_available() || !root().is_dir() {
        eprintln!("skipped: needs GDAL and scripts/fetch-testdata.sh");
        return;
    }
    let mut gdbs: Vec<PathBuf> = std::fs::read_dir(root())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("a00000001.gdbtable").is_file())
        .collect();
    gdbs.sort();
    let (mut datasets, mut pixels, unsupported) = (0, 0usize, Vec::<String>::new());
    let mut overview_checked = 0;
    for path in &gdbs {
        let g = Geodatabase::open_in(Default::default(), path.clone()).unwrap();
        for name in g.raster_datasets() {
            let label = format!("{}:{name}", path.file_name().unwrap().to_string_lossy());
            let spec = format!("OpenFileGDB:\"{}\":{name}", path.display());
            let Some(info) = gdal(&["gdalinfo", "-json", &spec]) else {
                eprintln!("~ {label}: GDAL doesn't open it");
                continue;
            };
            let info: Json = serde_json::from_str(&info).unwrap();
            let r = match g.open_raster(&name) {
                Ok(r) => r,
                Err(e) => panic!("{label}: {e}"),
            };
            assert_eq!(
                [r.width as i64, r.height as i64],
                [
                    info["size"][0].as_i64().unwrap(),
                    info["size"][1].as_i64().unwrap()
                ],
                "{label}"
            );
            let gt: Vec<f64> = info["geoTransform"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            for (a, b) in r.geotransform.iter().zip(&gt) {
                assert!(
                    (a - b).abs() <= 1e-6 * b.abs().max(1.0),
                    "{label}: {:?} vs {gt:?}",
                    r.geotransform
                );
            }
            // A coordinate system wherever GDAL has one.
            if info["coordinateSystem"]["wkt"]
                .as_str()
                .is_some_and(|w| !w.is_empty())
            {
                assert!(r.srs_wkt.is_some(), "{label}: no coordinate system");
            }
            let bands = info["bands"].as_array().unwrap();
            assert_eq!(r.band_count(), bands.len(), "{label}");
            // GDAL's widened type for the stored one.
            let gdal_type = bands[0]["type"].as_str().unwrap();
            let expected = match r.pixel_type {
                PixelType::U1 | PixelType::U2 | PixelType::U4 | PixelType::U8 => "Byte",
                PixelType::I8 => "Int8",
                PixelType::U16 => "UInt16",
                PixelType::I16 => "Int16",
                PixelType::U32 => "UInt32",
                PixelType::I32 => "Int32",
                PixelType::F32 => "Float32",
                PixelType::F64 => "Float64",
            };
            assert_eq!(gdal_type, expected, "{label}");
            // Pyramids: each level's size, and the coarsest level's pixels where it is small.
            let levels = bands[0]["overviews"].as_array().map_or(0, Vec::len);
            assert_eq!(r.overviews, levels, "{label}: pyramid levels");
            if levels > 0 {
                let top = levels;
                let (w, h) = r.level_size(top);
                let o = &bands[0]["overviews"][top - 1]["size"];
                assert_eq!(
                    [w as i64, h as i64],
                    [o[0].as_i64().unwrap(), o[1].as_i64().unwrap()],
                    "{label}: level {top}"
                );
                if w * h <= MAX_COMPARED
                    && !matches!(r.compression, Compression::Jpeg | Compression::Jpeg2000)
                {
                    let ovr = (top - 1).to_string();
                    let xyz = gdal(&[
                        "gdal_translate",
                        "-q",
                        "-ovr",
                        &ovr,
                        "-of",
                        "XYZ",
                        "-b",
                        "1",
                        &spec,
                        "/vsistdout/",
                    ]);
                    let theirs: Option<Vec<f64>> = xyz.and_then(|x| {
                        x.lines()
                            .map(|l| l.split_whitespace().nth(2).and_then(|v| v.parse().ok()))
                            .collect()
                    });
                    let ours = r
                        .read_band_at(0, top)
                        .unwrap_or_else(|e| panic!("{label} level {top}: {e}"));
                    if let Some(theirs) = theirs {
                        let nd = bands[0]["noDataValue"].as_f64();
                        for (i, (t, o)) in theirs.iter().zip(&ours.values).enumerate() {
                            let valid = ours.valid.as_ref().is_none_or(|v| v[i]);
                            if valid && nd.is_none_or(|nd| (t - nd).abs() > nd.abs() * 1e-6) {
                                assert!(
                                    (t - o).abs() <= 1e-6 * t.abs().max(1.0),
                                    "{label} level {top} pixel {i}: GDAL {t}, ours {o}"
                                );
                            }
                        }
                        pixels += theirs.len();
                        overview_checked += 1;
                    }
                }
            }
            if r.width * r.height > MAX_COMPARED {
                datasets += 1;
                continue;
            }
            for (b, gdal_info) in bands.iter().enumerate().take(r.band_count()) {
                let ours = match r.read_band(b) {
                    Ok(v) => v,
                    Err(e) => panic!("{label} band {}: {e}", b + 1),
                };
                let Some(theirs) = gdal_band(&spec, &(b + 1).to_string()) else {
                    eprintln!("~ {label}: GDAL can't read band {}", b + 1);
                    break;
                };
                assert_eq!(theirs.len(), ours.values.len(), "{label}");
                // Valid pixels: GDAL's mask band for integers, its NoData value for floats.
                let gdal_valid: Vec<bool> = if r.pixel_type.is_float() {
                    let nd = gdal_info["noDataValue"].as_f64();
                    theirs
                        .iter()
                        .map(|v| nd.is_none_or(|nd| (v - nd).abs() > nd.abs() * 1e-6))
                        .collect()
                } else {
                    gdal_band(&spec, &format!("mask,{}", b + 1)).map_or_else(
                        || vec![true; theirs.len()],
                        |m| m.iter().map(|v| *v != 0.0).collect(),
                    )
                };
                let (mut diff, mut n) = (0.0, 0usize);
                for (i, (t, o)) in theirs.iter().zip(&ours.values).enumerate() {
                    let valid = ours.valid.as_ref().is_none_or(|v| v[i]);
                    assert_eq!(
                        valid,
                        gdal_valid[i],
                        "{label} band {} pixel {i}: validity",
                        b + 1
                    );
                    if valid {
                        // JPEG decoders round their inverse DCT differently (GDAL's libjpeg, our
                        // zune-jpeg): a level or two apart, never more.
                        let tol = match (r.compression, r.pixel_type) {
                            (Compression::Jpeg | Compression::Jpeg2000, _) => 2.0,
                            (_, PixelType::F32) => 1e-6 * t.abs().max(1.0),
                            _ => 1e-9 * t.abs().max(1.0),
                        };
                        assert!(
                            (t - o).abs() <= tol,
                            "{label} band {} pixel {i}: GDAL {t}, ours {o}",
                            b + 1
                        );
                        diff += (t - o).abs();
                        n += 1;
                    }
                }
                if matches!(r.compression, Compression::Jpeg | Compression::Jpeg2000) {
                    assert!(
                        diff / n.max(1) as f64 <= 0.5,
                        "{label}: mean JPEG difference {}",
                        diff / n as f64
                    );
                }
                pixels += theirs.len();
            }
            datasets += 1;
        }
    }
    eprintln!(
        "{datasets} raster datasets match GDAL ({pixels} pixels compared, {overview_checked} pyramid levels); not read: {unsupported:?}"
    );
    assert!(datasets >= 50, "only {datasets} datasets");
    assert!(overview_checked >= 1, "no pyramid level compared");
    assert!(unsupported.is_empty());
}
