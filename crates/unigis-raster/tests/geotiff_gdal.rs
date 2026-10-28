//! Compares GeoTIFF reading and writing with GDAL (`gdalinfo`, `gdal_translate`).

use std::path::{Path, PathBuf};
use std::process::Command;

use unigis_raster::{Compression, DataType, read_geotiff, write_geotiff};

fn testdata(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/raster")
        .join(name)
}

fn gdal_available() -> bool {
    Command::new("gdalinfo").arg("--version").output().is_ok()
}

struct GdalRaster {
    transform: Vec<f64>,
    nodata: Option<f64>,
    epsg: Option<u32>,
    values: Vec<f64>,
    size: (usize, usize),
}

fn gdal_read(path: &Path, dir: &Path) -> GdalRaster {
    let out = Command::new("gdalinfo")
        .arg("-json")
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success(), "gdalinfo {}", path.display());
    let info: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let transform = info["geoTransform"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let band = &info["bands"][0];
    let nodata = match &band["noDataValue"] {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) if s.eq_ignore_ascii_case("nan") => Some(f64::NAN),
        v => v.as_f64(),
    };
    let wkt = info["coordinateSystem"]["wkt"].as_str().unwrap_or("");
    let epsg = wkt
        .rsplit("ID[\"EPSG\",")
        .next()
        .filter(|_| wkt.contains("ID[\"EPSG\","))
        .and_then(|s| s.split(']').next())
        .and_then(|s| s.parse().ok());
    let size = info["size"].as_array().unwrap();
    let size = (
        size[0].as_u64().unwrap() as usize,
        size[1].as_u64().unwrap() as usize,
    );
    let raw = dir.join(format!(
        "{}.f64",
        path.file_stem().unwrap().to_string_lossy()
    ));
    let st = Command::new("gdal_translate")
        .args([
            "-q",
            "-ot",
            "Float64",
            "-of",
            "ENVI",
            "-co",
            "INTERLEAVE=BSQ",
        ])
        .arg(path)
        .arg(&raw)
        .status()
        .unwrap();
    assert!(st.success());
    let bytes = std::fs::read(&raw).unwrap();
    let values = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| f64::from_le_bytes(*c))
        .collect();
    GdalRaster {
        transform,
        nodata,
        epsg,
        values,
        size,
    }
}

fn same(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

fn compare(path: &Path, dir: &Path) {
    let ours = read_geotiff(path).unwrap();
    let gdal = gdal_read(path, dir);
    let name = path.display();
    assert_eq!((ours.width, ours.height), gdal.size, "{name}: size");
    assert_eq!(ours.transform.to_vec(), gdal.transform, "{name}: transform");
    assert!(
        match (ours.nodata, gdal.nodata) {
            (Some(a), Some(b)) => same(a, b),
            (None, None) => true,
            _ => false,
        },
        "{name}: nodata {:?} vs {:?}",
        ours.nodata,
        gdal.nodata
    );
    assert_eq!(ours.epsg, gdal.epsg, "{name}: epsg");
    let all: Vec<f64> = ours.bands.concat();
    assert_eq!(all.len(), gdal.values.len(), "{name}: band count");
    let bad = all
        .iter()
        .zip(&gdal.values)
        .position(|(a, b)| !same(*a, *b));
    assert_eq!(bad, None, "{name}: first differing cell");
}

#[test]
fn reads_like_gdal() {
    if !gdal_available() {
        eprintln!("GDAL not installed; skipping");
        return;
    }
    let dir = tempdir("read");
    for name in [
        "dem.tif",
        "dem_tiled_deflate.tif",
        "dem_lzw.tif",
        "dem_nodata.tif",
        "dem_int16_packbits.tif",
        "landuse_u8.tif",
        "dem_f64.tif",
        "dem_bigendian.tif",
        "dem_bigtiff_i32.tif",
        "dem_point.tif",
        "rgb_planar_u16.tif",
        "rgb_pixel_u8.tif",
        // Written by ArcGIS with 4-bit samples.
        "../arcgis-reference/f2r_roads.tif",
        "../arcgis-reference/f2r_parcels_zone.tif",
    ] {
        compare(&testdata(name), &dir);
    }
}

#[test]
fn writes_what_gdal_reads() {
    if !gdal_available() {
        eprintln!("GDAL not installed; skipping");
        return;
    }
    let dir = tempdir("write");
    for (name, comp) in [
        ("dem_nodata.tif", Compression::None),
        ("dem.tif", Compression::Lzw),
        ("dem_f64.tif", Compression::Deflate),
        ("dem_int16_packbits.tif", Compression::PackBits),
        ("landuse_u8.tif", Compression::Lzw),
    ] {
        let r = read_geotiff(&testdata(name)).unwrap();
        let out = dir.join(format!("out_{name}"));
        write_geotiff(&out, &r, comp).unwrap();
        let gdal = gdal_read(&out, &dir);
        assert_eq!(gdal.transform, r.transform.to_vec(), "{name}");
        assert_eq!(gdal.epsg, r.epsg, "{name}");
        assert_eq!(
            gdal.nodata.map(|v| v.to_bits()),
            r.nodata.map(|v| v.to_bits()),
            "{name}"
        );
        assert!(
            r.bands[0]
                .iter()
                .zip(&gdal.values)
                .all(|(a, b)| same(*a, *b)),
            "{name}"
        );
        // And our own reader round-trips it.
        let back = read_geotiff(&out).unwrap();
        assert_eq!(back.data_type, r.data_type);
        assert_eq!(back.bands, r.bands);
    }
}

#[test]
fn geographic_and_rotated() {
    if !gdal_available() {
        eprintln!("GDAL not installed; skipping");
        return;
    }
    let dir = tempdir("geo");
    let mut r =
        unigis_raster::Raster::new(5, 4, DataType::I32, [140.0, 0.25, 0.0, -30.0, 0.0, -0.25]);
    r.epsg = Some(4326);
    r.nodata = Some(-1.0);
    r.bands[0] = (0..20).map(|v| v as f64 - 1.0).collect();
    let out = dir.join("geo.tif");
    write_geotiff(&out, &r, Compression::None).unwrap();
    let g = gdal_read(&out, &dir);
    assert_eq!(g.epsg, Some(4326));
    assert_eq!(g.transform, r.transform.to_vec());
    assert_eq!(g.values, r.bands[0]);

    r.transform = [1000.0, 2.0, 0.5, 2000.0, 0.5, -2.0];
    r.epsg = None;
    let out = dir.join("rot.tif");
    write_geotiff(&out, &r, Compression::None).unwrap();
    let g = gdal_read(&out, &dir);
    assert_eq!(g.transform, r.transform.to_vec());
    assert_eq!(read_geotiff(&out).unwrap().transform, r.transform);
}

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("unigis-raster-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn gdal_json(path: &Path) -> serde_json::Value {
    let out = Command::new("gdalinfo")
        .arg("-json")
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success(), "gdalinfo {}", path.display());
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn tiled_writer_with_overviews() {
    use unigis_raster::{OverviewResampling, WriteOptions, write_geotiff_with};
    if !gdal_available() {
        eprintln!("GDAL not installed; skipping");
        return;
    }
    let dir = tempdir("tiled");
    let cases = [
        ("dem.tif", Compression::Deflate, Some((64, 32)), true, false),
        ("dem_f64.tif", Compression::Lzw, Some((16, 16)), true, true),
        (
            "dem_nodata.tif",
            Compression::None,
            Some((128, 128)),
            false,
            false,
        ),
        (
            "dem_int16_packbits.tif",
            Compression::PackBits,
            None,
            true,
            false,
        ),
        (
            "landuse_u8.tif",
            Compression::Lzw,
            Some((32, 16)),
            true,
            true,
        ),
        (
            "rgb_planar_u16.tif",
            Compression::Deflate,
            Some((32, 32)),
            true,
            false,
        ),
        ("rgb_pixel_u8.tif", Compression::Lzw, None, true, true),
        (
            "dem_bigtiff_i32.tif",
            Compression::Deflate,
            Some((48, 48)),
            true,
            false,
        ),
    ];
    for (name, compression, tile, predictor, bigtiff) in cases {
        let r = read_geotiff(&testdata(name)).unwrap();
        let out = dir.join(format!("t_{name}"));
        let opts = WriteOptions {
            compression,
            tile,
            predictor,
            overviews: vec![2, 4],
            overview_resampling: if r.data_type.is_float() {
                OverviewResampling::Average
            } else {
                OverviewResampling::Nearest
            },
            bigtiff,
            zero_out_of_range_nodata: false,
            gdal_metadata: None,
        };
        write_geotiff_with(&out, &r, &opts).unwrap();
        let g = gdal_read(&out, &dir);
        assert_eq!(g.size, (r.width, r.height), "{name}");
        assert_eq!(g.transform, r.transform.to_vec(), "{name}");
        assert_eq!(g.epsg, r.epsg, "{name}");
        assert_eq!(
            g.nodata.map(f64::to_bits),
            r.nodata.map(f64::to_bits),
            "{name}"
        );
        let all: Vec<f64> = r.bands.concat();
        assert_eq!(g.values.len(), all.len(), "{name}");
        let bad = all.iter().zip(&g.values).position(|(a, b)| !same(*a, *b));
        assert_eq!(
            bad,
            None,
            "{name}: values {:?}",
            bad.map(|i| (all[i], g.values[i]))
        );
        let info = gdal_json(&out);
        let band = &info["bands"][0];
        let block: Vec<u64> = band["block"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        if let Some((tw, th)) = tile {
            assert_eq!(block, vec![tw as u64, th as u64], "{name}: tiles");
        }
        let ovr = band["overviews"].as_array().unwrap();
        assert_eq!(ovr.len(), 2, "{name}: overviews");
        assert_eq!(
            ovr[0]["size"],
            serde_json::json!([r.width.div_ceil(2), r.height.div_ceil(2)]),
            "{name}"
        );
        // GDAL reads the first overview's cells as written.
        let ov = dir.join(format!("ov_{name}"));
        let st = Command::new("gdal_translate")
            .args([
                "-q", "-ovr", "0", "-ot", "Float64", "-of", "ENVI", "-b", "1",
            ])
            .arg(&out)
            .arg(&ov)
            .status()
            .unwrap();
        assert!(st.success());
        let bytes = std::fs::read(&ov).unwrap();
        let got: Vec<f64> = bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| f64::from_le_bytes(*c))
            .collect();
        let (w, h) = (r.width.div_ceil(2), r.height.div_ceil(2));
        let mut expect = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let cells: Vec<f64> = [(0, 0), (1, 0), (0, 1), (1, 1)]
                    .iter()
                    .filter(|(dx, dy)| 2 * x + dx < r.width && 2 * y + dy < r.height)
                    .map(|(dx, dy)| r.bands[0][(2 * y + dy) * r.width + 2 * x + dx])
                    .filter(|v| !r.is_nodata(*v))
                    .collect();
                expect.push(if r.data_type.is_float() {
                    if cells.is_empty() {
                        r.nodata.unwrap()
                    } else {
                        r.data_type
                            .store(cells.iter().sum::<f64>() / cells.len() as f64)
                    }
                } else {
                    r.bands[0][2 * y * r.width + 2 * x]
                });
            }
        }
        assert!(
            expect.iter().zip(&got).all(|(a, b)| same(*a, *b)),
            "{name}: overview"
        );
        // And our reader agrees with itself.
        let back = read_geotiff(&out).unwrap();
        assert_eq!(back.bands, r.bands, "{name}: read back");
    }
}

/// Check stored keys directly: GDAL can repair inconsistent keys by EPSG lookup.
#[test]
fn epsg_geographic_keys_outside_the_old_numeric_range() {
    let dir = tempdir("epsg-keys");
    let mut raster = unigis_raster::Raster::new(1, 1, DataType::U8, [140., 1., 0., -30., 0., -1.]);
    for (code, geographic) in [
        (4326, true),
        (7844, true),
        (6318, true),
        (32633, false),
        (7856, false),
    ] {
        raster.epsg = Some(code);
        let out = dir.join(format!("{code}.tif"));
        write_geotiff(&out, &raster, Compression::None).unwrap();
        let mut decoder = tiff::decoder::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(&out).unwrap(),
        ))
        .unwrap();
        let keys = decoder
            .find_tag(tiff::tags::Tag::GeoKeyDirectoryTag)
            .unwrap()
            .unwrap()
            .into_u16_vec()
            .unwrap();
        let entries: Vec<_> = keys[4..].as_chunks::<4>().0.iter().collect();
        let key = |id| {
            entries
                .iter()
                .find(|entry| entry[0] == id)
                .map(|entry| entry[3])
        };
        assert_eq!(
            key(1024),
            Some(if geographic { 2 } else { 1 }),
            "EPSG:{code}"
        );
        assert_eq!(key(if geographic { 2048 } else { 3072 }), Some(code as u16));
        assert_eq!(key(if geographic { 3072 } else { 2048 }), None);
        assert_eq!(read_geotiff(&out).unwrap().epsg, Some(code));
        if gdal_available() {
            assert_eq!(gdal_read(&out, &dir).epsg, Some(code));
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// A NoData tag is metadata; it need not fit the integer cell representation.
#[test]
fn integer_nodata_tags_preserve_values_outside_i64_range() {
    let dir = tempdir("large-integer-nodata");
    let mut raster = unigis_raster::Raster::new(2, 2, DataType::I16, [0., 1., 0., 2., 0., -1.]);
    raster.bands[0] = vec![0., 1., 2., 3.];
    for (i, nodata) in [3.4e38, 1.79e308, -1.79e308].into_iter().enumerate() {
        raster.nodata = Some(nodata);
        let out = dir.join(format!("{i}.tif"));
        write_geotiff(&out, &raster, Compression::None).unwrap();
        let back = read_geotiff(&out).unwrap();
        assert_eq!(back.nodata, Some(nodata));
        assert_eq!(back.bands, raster.bands);
        if gdal_available() {
            assert_eq!(
                gdal_json(&out)["bands"][0]["noDataValue"].as_f64(),
                Some(nodata)
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// ArcGIS GP output encoding is optional; logical cells remain available for stats.
#[test]
fn integer_nodata_zero_encoding_is_explicit_and_preserves_logical_cells() {
    let dir = tempdir("zero-nodata");
    for (i, (data_type, nodata, clamped)) in [
        (DataType::U8, 256., 255.),
        (DataType::I16, 3.4e38, 32767.),
        (DataType::I32, f64::MIN, i32::MIN as f64),
    ]
    .into_iter()
    .enumerate()
    {
        let mut raster = unigis_raster::Raster::new(2, 1, data_type, [0., 1., 0., 1., 0., -1.]);
        raster.nodata = Some(nodata);
        raster.bands[0] = vec![1., nodata];
        let default = dir.join(format!("default-{i}.tif"));
        write_geotiff(&default, &raster, Compression::None).unwrap();
        assert_eq!(read_geotiff(&default).unwrap().bands[0], vec![1., clamped]);
        for tiled in [false, true] {
            let path = dir.join(format!("{i}-{tiled}.tif"));
            unigis_raster::write_geotiff_with(
                &path,
                &raster,
                &unigis_raster::WriteOptions {
                    zero_out_of_range_nodata: true,
                    tile: tiled.then_some((16, 16)),
                    bigtiff: tiled,
                    compression: if tiled {
                        Compression::Lzw
                    } else {
                        Compression::None
                    },
                    ..Default::default()
                },
            )
            .unwrap();
            let back = read_geotiff(&path).unwrap();
            assert_eq!(back.nodata, Some(nodata));
            assert_eq!(back.bands[0], vec![1., 0.]);
            if gdal_available() {
                let observed = gdal_read(&path, &dir);
                assert_eq!(observed.nodata, Some(nodata));
                assert_eq!(observed.values, vec![1., 0.]);
            }
            assert_eq!(raster.bands[0], vec![1., nodata]);
            assert_eq!(unigis_raster::stats::statistics(&raster).unwrap().count, 1);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
