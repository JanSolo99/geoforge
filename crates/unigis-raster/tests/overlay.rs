//! Overlay and local tools over several rasters against ArcGIS Pro 3.7
//! (testdata/arcgis-reference/overlay.json: 8 x 8 rasters with NoData, and integer rasters of many
//! ranges for the output types).

use std::path::{Path, PathBuf};

use serde_json::Value;
use unigis_raster::overlay::*;
use unigis_raster::{DataType, Raster};

const ND: f64 = -9999.0;

fn reference() -> Value {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/overlay.json")).unwrap(),
    )
    .unwrap()
}

/// A raster from rows of values (top row first) with the file's statistics.
fn raster(rows: &Value, t: DataType, nodata: f64) -> Raster {
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
    r.nodata = Some(nodata);
    let mut stats = (f64::INFINITY, f64::NEG_INFINITY);
    for (y, row) in rows.iter().enumerate() {
        for (x, v) in row.as_array().unwrap().iter().enumerate() {
            let v = v.as_f64().unwrap();
            r.bands[0][y * w + x] = if v == nodata { v } else { t.store(v) };
            if v != nodata {
                stats = (stats.0.min(v), stats.1.max(v));
            }
        }
    }
    r.statistics = Some(stats);
    r
}

fn matches(got: &Raster, want: &Value, tolerance: f64) -> Result<(), String> {
    let mut bad = 0;
    let mut first = None;
    for (y, row) in want["values"].as_array().unwrap().iter().enumerate() {
        for (x, w) in row.as_array().unwrap().iter().enumerate() {
            let w = w.as_f64().unwrap();
            let ok = match got.get(x, y) {
                None => w <= -9000.0 && w != 0.0 || w == -99_999_999.0,
                Some(g) => w > -9000.0 && (g - w).abs() <= tolerance + 1e-6 * w.abs(),
            };
            if !ok {
                bad += 1;
                first.get_or_insert((y, x, w, got.get(x, y)));
            }
        }
    }
    if bad > 0 {
        Err(format!("{bad} cells differ, first {first:?}"))
    } else {
        Ok(())
    }
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

#[test]
fn cell_statistics_weighted_sum_position_and_combine_equal_arcgis() {
    let r = reference();
    let p = &r["probe"];
    let a = raster(&p["in_A"]["values"], DataType::F32, ND);
    let b = raster(&p["in_B"]["values"], DataType::F32, ND);
    let c = raster(&p["in_C"]["values"], DataType::I32, ND);
    let d = raster(&p["in_D"]["values"], DataType::I32, ND);
    let mut failures = Vec::new();
    let mut check = |key: &str, out: Option<Raster>, nodata: Option<f64>| {
        let want = &p[key];
        let Some(out) = out else {
            failures.push(format!("{key}: no output"));
            return;
        };
        if out.data_type != pixel(want["pixel"].as_str().unwrap()) {
            failures.push(format!(
                "{key}: type {:?} but ArcGIS {}",
                out.data_type, want["pixel"]
            ));
        }
        if let Some(nd) = nodata
            && out.nodata != Some(nd)
        {
            failures.push(format!("{key}: NoData {:?} but ArcGIS {nd}", out.nodata));
        }
        if let Err(e) = matches(&out, want, 1e-4) {
            failures.push(format!("{key}: {e}"));
        }
    };
    for (name, stat) in [
        ("SUM", CellStat::Sum),
        ("MEAN", CellStat::Mean),
        ("MAXIMUM", CellStat::Maximum),
        ("MINIMUM", CellStat::Minimum),
        ("MEDIAN", CellStat::Median),
        ("MAJORITY", CellStat::Majority),
        ("MINORITY", CellStat::Minority),
        ("RANGE", CellStat::Range),
        ("STD", CellStat::Std),
        ("VARIETY", CellStat::Variety),
    ] {
        for (ig, ignore) in [("DATA", true), ("NODATA", false)] {
            check(
                &format!("cs_{name}_{ig}_f"),
                cell_statistics(&[&a, &b, &c], stat, ignore),
                None,
            );
            check(
                &format!("cs_{name}_{ig}_i"),
                cell_statistics(&[&c, &d], stat, ignore),
                None,
            );
        }
    }
    // Integer outputs carry ArcGIS's NoData for their value range.
    check(
        "cs_SUM_DATA_i",
        cell_statistics(&[&c, &d], CellStat::Sum, true),
        Some(15.0),
    );
    check(
        "cs_VARIETY_DATA_f",
        cell_statistics(&[&a, &b, &c], CellStat::Variety, true),
        Some(0.0),
    );
    check(
        "cs_VARIETY_DATA_i",
        cell_statistics(&[&c, &d], CellStat::Variety, true),
        Some(3.0),
    );
    check("wsum", weighted_sum(&[&a, &b, &c], &[0.5, 0.3, 1.2]), None);
    check("wsum_i", weighted_sum(&[&c, &d], &[2.0, 3.0]), None);
    check("hp", position(&[&a, &b, &c], true), Some(0.0));
    check("lp", position(&[&a, &b, &c], false), Some(0.0));
    check("hp_i", position(&[&c, &d], true), Some(3.0));
    check("combine", combine(&[&c, &d]), Some(-128.0));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn weighted_overlay_equals_arcgis() {
    let r = reference();
    let p = &r["probe"];
    let c = raster(&p["in_C"]["values"], DataType::I32, ND);
    let d = raster(&p["in_D"]["values"], DataType::I32, ND);
    let remap_c: Vec<(f64, Scale)> = vec![
        (1.0, Scale::Value(1.0)),
        (2.0, Scale::Value(1.0)),
        (3.0, Scale::Value(3.0)),
        (4.0, Scale::Value(5.0)),
        (5.0, Scale::Value(5.0)),
        (6.0, Scale::Value(7.0)),
        (7.0, Scale::Value(9.0)),
        (8.0, Scale::Value(9.0)),
        (9.0, Scale::NoData),
    ];
    let remap_d: Vec<(f64, Scale)> = vec![
        (1.0, Scale::Value(2.0)),
        (2.0, Scale::Value(4.0)),
        (3.0, Scale::Value(6.0)),
        (4.0, Scale::Value(8.0)),
    ];
    for (key, wc, wd) in [("wo", 60.0, 40.0), ("wo_ev", 70.0, 30.0)] {
        let out = weighted_overlay(
            &[
                Weighted {
                    raster: &c,
                    influence: wc,
                    remap: remap_c.clone(),
                },
                Weighted {
                    raster: &d,
                    influence: wd,
                    remap: remap_d.clone(),
                },
            ],
            (1.0, 9.0),
        )
        .unwrap();
        assert_eq!(out.data_type, DataType::I8);
        assert_eq!(out.nodata, Some(-128.0));
        matches(&out, &p[key], 1e-9).unwrap_or_else(|e| panic!("{key}: {e}"));
    }
    // Influences must add up to 100.
    assert!(
        weighted_overlay(
            &[Weighted {
                raster: &c,
                influence: 90.0,
                remap: remap_c
            }],
            (1.0, 9.0)
        )
        .is_none()
    );
}

#[test]
fn fuzzy_membership_and_overlay_equal_arcgis() {
    let r = reference();
    let p = &r["probe"];
    let a = raster(&p["in_A"]["values"], DataType::F32, ND);
    let b = raster(&p["in_B"]["values"], DataType::F32, ND);
    let c = raster(&p["in_C"]["values"], DataType::I32, ND);
    let classes: [(&str, Fuzzy); 12] = [
        ("linear", Fuzzy::Linear { min: 3.0, max: 7.0 }),
        ("linear_dec", Fuzzy::Linear { min: 7.0, max: 3.0 }),
        (
            "gaussian",
            Fuzzy::Gaussian {
                midpoint: 5.0,
                spread: 0.2,
            },
        ),
        (
            "gaussian_s",
            Fuzzy::Gaussian {
                midpoint: 5.0,
                spread: 0.6,
            },
        ),
        (
            "large",
            Fuzzy::Large {
                midpoint: 5.0,
                spread: 5.0,
            },
        ),
        (
            "large_s",
            Fuzzy::Large {
                midpoint: 4.0,
                spread: 8.0,
            },
        ),
        (
            "small",
            Fuzzy::Small {
                midpoint: 5.0,
                spread: 5.0,
            },
        ),
        (
            "small_s",
            Fuzzy::Small {
                midpoint: 6.0,
                spread: 2.0,
            },
        ),
        (
            "mslarge",
            Fuzzy::MsLarge {
                mean_multiplier: 1.0,
                std_multiplier: 1.5,
            },
        ),
        (
            "mssmall",
            Fuzzy::MsSmall {
                mean_multiplier: 1.0,
                std_multiplier: 1.5,
            },
        ),
        (
            "near",
            Fuzzy::Near {
                midpoint: 5.0,
                spread: 0.3,
            },
        ),
        (
            "near_s",
            Fuzzy::Near {
                midpoint: 5.0,
                spread: 0.05,
            },
        ),
    ];
    let mut failures = Vec::new();
    for (name, class) in classes {
        let out = fuzzy_membership(&a, class, Hedge::None);
        if let Err(e) = matches(&out, &p[format!("fm_{name}")], 1e-5) {
            failures.push(format!("fm_{name}: {e}"));
        }
    }
    let out = fuzzy_membership(
        &c,
        Fuzzy::Large {
            midpoint: 5.0,
            spread: 5.0,
        },
        Hedge::None,
    );
    if let Err(e) = matches(&out, &p["fm_c_large"], 1e-5) {
        failures.push(format!("fm_c_large: {e}"));
    }
    let m1 = fuzzy_membership(&a, Fuzzy::Linear { min: 3.0, max: 7.0 }, Hedge::None);
    let m2 = fuzzy_membership(
        &b,
        Fuzzy::Gaussian {
            midpoint: 5.0,
            spread: 0.2,
        },
        Hedge::None,
    );
    let m3 = fuzzy_membership(
        &c,
        Fuzzy::Large {
            midpoint: 5.0,
            spread: 5.0,
        },
        Hedge::None,
    );
    for (key, kind) in [
        ("AND", FuzzyOverlayType::And),
        ("OR", FuzzyOverlayType::Or),
        ("PRODUCT", FuzzyOverlayType::Product),
        ("SUM", FuzzyOverlayType::Sum),
        ("gamma_0.0", FuzzyOverlayType::Gamma(0.0)),
        ("gamma_0.3", FuzzyOverlayType::Gamma(0.3)),
        ("gamma_0.7", FuzzyOverlayType::Gamma(0.7)),
        ("gamma_1.0", FuzzyOverlayType::Gamma(1.0)),
    ] {
        let out = fuzzy_overlay(&[&m1, &m2, &m3], kind).unwrap();
        if let Err(e) = matches(&out, &p[format!("fo_{key}")], 1e-5) {
            failures.push(format!("fo_{key}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn output_types_follow_the_value_range_like_arcgis() {
    let r = reference();
    let t = &r["types"];
    let inputs = t["inputs"].as_object().unwrap();
    let ras = |name: &str| raster(&inputs[name], DataType::I32, -2_147_483_647.0);
    let stats = [
        ("SUM", CellStat::Sum),
        ("MAXIMUM", CellStat::Maximum),
        ("MINIMUM", CellStat::Minimum),
        ("RANGE", CellStat::Range),
        ("MEDIAN", CellStat::Median),
        ("MAJORITY", CellStat::Majority),
        ("VARIETY", CellStat::Variety),
    ];
    let mut failures = Vec::new();
    for (key, want) in t["types"].as_object().unwrap() {
        let parts: Vec<&str> = key.split('_').collect();
        let out = match parts[0] {
            "cs" => {
                let stat = stats.iter().find(|s| s.0 == parts[1]).unwrap().1;
                cell_statistics(&[&ras(parts[2]), &ras(parts[3])], stat, true)
            }
            "cs3" => {
                let stat = stats.iter().find(|s| s.0 == parts[1]).unwrap().1;
                let u8r = ras("u8");
                cell_statistics(&[&u8r, &u8r, &u8r], stat, true)
            }
            "hp" => position(&[&ras(parts[1]), &ras(parts[2])], true),
            "wsum" => weighted_sum(&[&ras(parts[1]), &ras(parts[2])], &[1.0, 1.0]),
            "comb" => combine(&[&ras(parts[1]), &ras(parts[2])]),
            other => panic!("{other}"),
        };
        let Some(out) = out else {
            failures.push(format!("{key}: no output"));
            continue;
        };
        let want_type = pixel(want["pixel"].as_str().unwrap());
        if out.data_type != want_type {
            failures.push(format!(
                "{key}: {:?} but ArcGIS {want_type:?}",
                out.data_type
            ));
        }
        // 32-bit NoData follows the inputs' in this run, and float NoData is the same to the last digit.
        if let Some(nd) = want["nodata"].as_f64()
            && !out.data_type.is_float()
            && out.data_type != DataType::I32
            && out.nodata.is_some_and(|n| n != nd)
        {
            failures.push(format!("{key}: NoData {:?} but ArcGIS {nd}", out.nodata));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn fuzzy_hedges_equal_arcgis() {
    let r = reference();
    let a = raster(&r["probe"]["in_A"]["values"], DataType::F32, ND);
    for (name, class) in [
        ("linear", Fuzzy::Linear { min: 3.0, max: 7.0 }),
        (
            "large",
            Fuzzy::Large {
                midpoint: 5.0,
                spread: 5.0,
            },
        ),
    ] {
        for (hedge, h) in [
            ("NONE", Hedge::None),
            ("SOMEWHAT", Hedge::Somewhat),
            ("VERY", Hedge::Very),
        ] {
            let out = fuzzy_membership(&a, class, h);
            let want = serde_json::json!({"values": r["hedge"][format!("{name}_{hedge}")]});
            matches(&out, &want, 1e-5).unwrap_or_else(|e| panic!("{name} {hedge}: {e}"));
        }
    }
}
