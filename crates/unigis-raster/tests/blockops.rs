//! Aggregate, Block Statistics, Slice, Pick and Region Group against ArcGIS Pro 3.7
//! (testdata/arcgis-reference/blockops.json).

use std::path::{Path, PathBuf};

use serde_json::Value;
use unigis_raster::blockops::*;
use unigis_raster::focal::{FocalStatistic, Neighborhood};
use unigis_raster::local::Operand;
use unigis_raster::{DataType, Raster};

const ND: f64 = -9999.0;

fn reference() -> Value {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/blockops.json")).unwrap(),
    )
    .unwrap()
}

fn raster(rows: &Value, t: DataType) -> Raster {
    let rows = rows.as_array().unwrap();
    let (h, w) = (rows.len(), rows[0].as_array().unwrap().len());
    let mut r = Raster::new(
        w,
        h,
        t,
        [
            500_000.0,
            30.0,
            0.0,
            5_000_000.0 + h as f64 * 30.0,
            0.0,
            -30.0,
        ],
    );
    r.nodata = Some(ND);
    let mut stats = (f64::INFINITY, f64::NEG_INFINITY);
    for (y, row) in rows.iter().enumerate() {
        for (x, v) in row.as_array().unwrap().iter().enumerate() {
            let v = v.as_f64().unwrap();
            let v = if v == ND { v } else { t.store(v) };
            r.bands[0][y * w + x] = v;
            if v != ND {
                stats = (stats.0.min(v), stats.1.max(v));
            }
        }
    }
    r.statistics = Some(stats);
    r
}

fn pixel(name: &str) -> DataType {
    match name {
        "U8" => DataType::U8,
        "S8" => DataType::I8,
        "U16" => DataType::U16,
        "S16" => DataType::I16,
        "U32" => DataType::U32,
        "S32" => DataType::I32,
        "F32" => DataType::F32,
        other => panic!("{other}"),
    }
}

fn judge(key: &str, got: &Raster, want: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    if got.data_type != pixel(want["pixel"].as_str().unwrap()) {
        failures.push(format!(
            "{key}: type {:?} but ArcGIS {}",
            got.data_type, want["pixel"]
        ));
    }
    if let Some(nd) = want["nodata"].as_f64()
        && got
            .nodata
            .is_none_or(|n| (n - nd).abs() > 1e-3 * nd.abs().max(1.0))
    {
        failures.push(format!("{key}: NoData {:?} but ArcGIS {nd}", got.nodata));
    }
    if (got.width, got.height)
        != (
            want["cols"].as_u64().unwrap() as usize,
            want["rows"].as_u64().unwrap() as usize,
        )
    {
        failures.push(format!(
            "{key}: {}x{} but ArcGIS {}x{}",
            got.width, got.height, want["cols"], want["rows"]
        ));
        return failures;
    }
    let e = got.extent();
    let we: Vec<f64> = want["extent"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    if [e.0, e.1, e.2, e.3]
        .iter()
        .zip(&we)
        .any(|(a, b)| (a - b).abs() > 1e-6)
    {
        failures.push(format!("{key}: extent {e:?} but ArcGIS {we:?}"));
    }
    let (mut bad, mut first) = (0, None);
    for (y, row) in want["values"].as_array().unwrap().iter().enumerate() {
        for (x, w) in row.as_array().unwrap().iter().enumerate() {
            let w = w.as_f64().unwrap();
            let ok = match got.get(x, y) {
                None => (w + 123_456_789.0).abs() < 10.0,
                Some(g) => {
                    (w + 123_456_789.0).abs() >= 10.0 && (g - w).abs() <= 1e-4 * w.abs().max(1.0)
                }
            };
            if !ok {
                bad += 1;
                first.get_or_insert((y, x, w, got.get(x, y)));
            }
        }
    }
    if bad > 0 {
        failures.push(format!("{key}: {bad} cells differ, first {first:?}"));
    }
    failures
}

#[test]
fn block_tools_equal_arcgis() {
    let r = reference();
    let fl = raster(&r["fl"], DataType::F32);
    let it = raster(&r["it"], DataType::I32);
    let cls = raster(&r["cls"], DataType::I32);
    let mut failures = Vec::new();
    // Aggregate.
    for (tag, src) in [("f", &fl), ("i", &it)] {
        for (name, stat) in [
            ("MEAN", AggregateStat::Mean),
            ("MAXIMUM", AggregateStat::Maximum),
            ("MEDIAN", AggregateStat::Median),
            ("MINIMUM", AggregateStat::Minimum),
            ("SUM", AggregateStat::Sum),
        ] {
            for (ig, ignore) in [("DATA", true), ("NODATA", false)] {
                for (ex, expand) in [("exp", true), ("trunc", false)] {
                    let key = format!("agg_{tag}_{name}_{ig}_{ex}");
                    match aggregate(src, 3, stat, expand, ignore) {
                        Some(o) => failures.extend(judge(&key, &o, &r[&key])),
                        None => failures.push(format!("{key}: no output")),
                    }
                }
            }
        }
        // Block Statistics.
        for (name, stat) in [
            ("MEAN", FocalStatistic::Mean),
            ("MAJORITY", FocalStatistic::Majority),
            ("MAXIMUM", FocalStatistic::Maximum),
            ("MEDIAN", FocalStatistic::Median),
            ("MINIMUM", FocalStatistic::Minimum),
            ("MINORITY", FocalStatistic::Minority),
            ("RANGE", FocalStatistic::Range),
            ("STD", FocalStatistic::Std),
            ("SUM", FocalStatistic::Sum),
            ("VARIETY", FocalStatistic::Variety),
        ] {
            for (nb, shape) in [
                (
                    "Rectangle",
                    Neighborhood::Rectangle {
                        width: 3,
                        height: 3,
                    },
                ),
                ("Circle", Neighborhood::Circle { radius: 2.0 }),
            ] {
                let key = format!("blk_{tag}_{name}_{nb}");
                if r.get(&key).is_none() {
                    continue; // ArcGIS refuses these statistics on floats
                }
                match block_statistics(src, shape, stat, true) {
                    Some(o) => failures.extend(judge(&key, &o, &r[&key])),
                    None => failures.push(format!("{key}: no output")),
                }
            }
        }
    }
    // Slice.
    failures.extend(judge(
        "slice_EQUAL_INTERVAL",
        &slice_equal_interval(&fl, 5, 1).unwrap(),
        &r["slice_EQUAL_INTERVAL"],
    ));
    failures.extend(judge(
        "slice_i_EQUAL_INTERVAL",
        &slice_equal_interval(&it, 3, 1).unwrap(),
        &r["slice_i_EQUAL_INTERVAL"],
    ));
    failures.extend(judge(
        "slice_base",
        &slice_equal_interval(&fl, 4, 10).unwrap(),
        &r["slice_base"],
    ));
    // Pick.
    let rasters = [
        Operand::Raster(fl.clone()),
        Operand::Raster(fl.clone()),
        Operand::Raster(it.clone()),
        Operand::Raster(it.clone()),
    ];
    failures.extend(judge("pick", &pick(&it, &rasters).unwrap(), &r["pick"]));
    let consts = [10.0, 20.0, 30.0, 40.0].map(Operand::Constant);
    failures.extend(judge(
        "pick_const",
        &pick(&it, &consts).unwrap(),
        &r["pick_const"],
    ));
    // Region Group.
    failures.extend(judge("rg4", &region_group_four(&cls), &r["rg4"]));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// New GeoTIFF probes exercise real pixel types, negative integers, edge NoData,
/// circular masks and all bands, independently of the older JSON fixtures.
#[test]
fn block_statistics_geotiffs_equal_arcgis() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/manifest.json")).unwrap(),
    )
    .unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (key, case) in manifest["cases"].as_object().unwrap() {
        if !key.starts_with("blk_") || case["status"] != "succeeded" {
            continue;
        }
        let args = &case["args"];
        let text = args[2].as_str().unwrap();
        let src = unigis_raster::read_geotiff(&root.join(args[0].as_str().unwrap())).unwrap();
        let expected =
            unigis_raster::read_geotiff(&root.join(format!("testdata/arcgis-reference/{key}.tif")))
                .unwrap();
        let nb = block_neighborhood(text, src.cell_width(), src.cell_height()).unwrap();
        let stat = FocalStatistic::parse(args[3].as_str().unwrap()).unwrap();
        let got = block_statistics(&src, nb, stat, args[4] == "DATA").unwrap();
        if got.data_type != expected.data_type || got.nodata != expected.nodata {
            failures.push(format!(
                "{key}: type/nodata {:?}/{:?}, expected {:?}/{:?}",
                got.data_type, got.nodata, expected.data_type, expected.nodata
            ));
        }
        assert_eq!(
            (got.width, got.height),
            (expected.width, expected.height),
            "{key}"
        );
        assert_eq!(got.transform, expected.transform, "{key}");
        assert_eq!(got.epsg, expected.epsg, "{key}");
        assert_eq!(got.bands.len(), expected.bands.len(), "{key}");
        for (b, (g, e)) in got.bands.iter().zip(&expected.bands).enumerate() {
            let bad: Vec<_> = g
                .iter()
                .zip(e)
                .enumerate()
                .filter(|(_, (g, e))| {
                    if got.is_nodata(**g) || expected.is_nodata(**e) {
                        got.is_nodata(**g) != expected.is_nodata(**e)
                    } else {
                        (**g - **e).abs()
                            > if got.data_type.is_float() {
                                1e-5 * e.abs().max(1.0)
                            } else {
                                0.0
                            }
                    }
                })
                .collect();
            if !bad.is_empty() {
                failures.push(format!(
                    "{key} band {b}: {} cells differ; first {:?}",
                    bad.len(),
                    bad[0]
                ));
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 74, "reference coverage changed");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn block_statistics_rejects_invalid_inputs() {
    // The zero-width case is ArcGIS ERROR 000628 in the probe manifest.
    for text in [
        "Rectangle 0 3 CELL",
        "Rectangle -1 3 CELL",
        "Circle NaN CELL",
        "Circle inf MAP",
        "Annulus 3 1 CELL",
        "Rectangle 99999999 9 CELL",
        "Circle 2 CELL extra",
        "Rectangle 3 3 UNKNOWN",
    ] {
        assert!(block_neighborhood(text, 30.0, 30.0).is_none(), "{text}");
    }
    assert!(block_neighborhood("Circle 30 MAP", 0.0, 30.0).is_none());
    assert!(block_neighborhood("Circle 30 MAP", 30.0, 60.0).is_none());
    let src = Raster::new(3, 3, DataType::F32, [0.0, 1.0, 0.0, 3.0, 0.0, -1.0]);
    for stat in [
        FocalStatistic::Majority,
        FocalStatistic::Median,
        FocalStatistic::Minority,
        FocalStatistic::Variety,
    ] {
        assert!(
            block_check(stat, src.data_type)
                .unwrap_err()
                .contains("000864")
        );
        assert!(
            block_statistics(
                &src,
                Neighborhood::Rectangle {
                    width: 3,
                    height: 3
                },
                stat,
                true
            )
            .is_none()
        );
    }
}
