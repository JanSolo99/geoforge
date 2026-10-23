//! The overlay commands against the ArcGIS values (testdata/arcgis-reference/overlay.json).

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_raster::{Compression, DataType, Raster, read_geotiff, write_geotiff};
use serde_json::Value;

const ND: f64 = -9999.0;

fn write(dir: &Path, name: &str, rows: &Value, t: DataType) -> PathBuf {
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
            r.bands[0][y * w + x] = v;
            if v != ND {
                stats = (stats.0.min(v), stats.1.max(v));
            }
        }
    }
    r.statistics = Some(stats);
    let path = dir.join(format!("{name}.tif"));
    write_geotiff(&path, &r, Compression::None).unwrap();
    path
}

fn run(args: &[&str]) {
    let out = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn same(path: &Path, want: &Value, tolerance: f64) {
    let got = read_geotiff(path).unwrap();
    for (y, row) in want["values"].as_array().unwrap().iter().enumerate() {
        for (x, w) in row.as_array().unwrap().iter().enumerate() {
            let w = w.as_f64().unwrap();
            match got.get(x, y) {
                Some(g) => assert!(
                    w > -9000.0 && (g - w).abs() <= tolerance,
                    "{path:?} {y},{x}: {g} vs {w}"
                ),
                None => assert!(w <= -9000.0, "{path:?} {y},{x}: NoData vs {w}"),
            }
        }
    }
}

#[test]
fn overlay_commands_equal_arcgis() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let r: Value = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/overlay.json")).unwrap(),
    )
    .unwrap();
    let p = &r["probe"];
    let d = std::env::temp_dir().join(format!("gf-overlay-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let a = write(&d, "a", &p["in_A"]["values"], DataType::F32);
    let b = write(&d, "b", &p["in_B"]["values"], DataType::F32);
    let c = write(&d, "c", &p["in_C"]["values"], DataType::I32);
    let (a, b, c) = (
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        c.to_str().unwrap(),
    );
    let out = |n: &str| d.join(format!("{n}.tif"));
    let o = |n: &str| out(n).to_str().unwrap().to_string();
    run(&[
        "cell-statistics",
        a,
        b,
        c,
        &o("max"),
        "--statistics-type",
        "MAXIMUM",
    ]);
    same(&out("max"), &p["cs_MAXIMUM_DATA_f"], 1e-4);
    run(&[
        "cell-statistics",
        a,
        b,
        c,
        &o("med"),
        "--statistics-type",
        "MEDIAN",
        "--ignore-nodata",
        "NODATA",
    ]);
    same(&out("med"), &p["cs_MEDIAN_NODATA_f"], 1e-4);
    run(&[
        "weighted-sum",
        &format!("{a}=0.5"),
        &format!("{b}=0.3"),
        &format!("{c}=1.2"),
        &o("ws"),
    ]);
    same(&out("ws"), &p["wsum"], 1e-3);
    run(&["highest-position", a, b, c, &o("hp")]);
    same(&out("hp"), &p["hp"], 0.0);
    run(&["highest-position", a, b, c, &o("lp"), "--lowest"]);
    same(&out("lp"), &p["lp"], 0.0);
    run(&["fuzzy-membership", a, &o("fm"), "linear", "3", "7"]);
    same(&out("fm"), &p["fm_linear"], 1e-5);
    run(&["fuzzy-membership", b, &o("fg"), "gaussian", "5", "0.2"]);
    run(&["fuzzy-membership", c, &o("fl"), "large", "5", "5"]);
    run(&[
        "fuzzy-overlay",
        &o("fm"),
        &o("fg"),
        &o("fl"),
        &o("fo"),
        "--overlay-type",
        "GAMMA",
        "--gamma",
        "0.7",
    ]);
    same(&out("fo"), &p["fo_gamma_0.7"], 1e-5);
    // Bad arguments are errors.
    for bad in [
        vec![
            "cell-statistics",
            a,
            b,
            "x.tif",
            "--statistics-type",
            "AVERAGE",
        ],
        vec!["fuzzy-overlay", a, b, "x.tif", "--overlay-type", "GAMMA"],
        vec!["fuzzy-membership", a, "x.tif", "LINEAR", "3"],
        vec!["weighted-sum", a, b],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(&bad)
            .output()
            .unwrap();
        assert!(!r.status.success(), "{bad:?}");
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn math_fn_command_equals_arcgis() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let r: Value = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/mathfn.json")).unwrap(),
    )
    .unwrap();
    let d = std::env::temp_dir().join(format!("gf-mathfn-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let f = write(&d, "f", &r["inputs"]["F"], DataType::F32);
    let i = write(&d, "i", &r["inputs"]["I"], DataType::I32);
    let b = write(&d, "b", &r["inputs"]["B"], DataType::I32);
    let (f, i, b) = (
        f.to_str().unwrap(),
        i.to_str().unwrap(),
        b.to_str().unwrap(),
    );
    let o = |n: &str| d.join(format!("{n}.tif")).to_str().unwrap().to_string();
    let check = |key: &str, path: &str| {
        let got = read_geotiff(Path::new(path)).unwrap();
        let want = &r["cases"][key];
        for (y, row) in want["values"].as_array().unwrap().iter().enumerate() {
            for (x, w) in row.as_array().unwrap().iter().enumerate() {
                let w = w.as_f64().unwrap();
                match got.get(x, y) {
                    Some(g) => assert!(
                        (w + 123_456_789.0).abs() >= 10.0
                            && (g - w).abs() <= 1e-5 * w.abs().max(1.0),
                        "{key} {y},{x}: {g} vs {w}"
                    ),
                    None => assert!(
                        (w + 123_456_789.0).abs() < 10.0,
                        "{key} {y},{x}: NoData vs {w}"
                    ),
                }
            }
        }
    };
    run(&["math-fn", "Ln", f, &o("ln")]);
    check("Ln_F", &o("ln"));
    run(&["math-fn", "Square", i, &o("sq")]);
    check("Square_I", &o("sq"));
    run(&["math-fn", "Mod", i, b, &o("mod")]);
    check("Mod_IB", &o("mod"));
    run(&["math-fn", "GreaterThan", f, "0", &o("gt")]);
    check("GreaterThan_F_const", &o("gt"));
    run(&["math-fn", "BitwiseAnd", i, b, &o("and")]);
    check("BitwiseAnd_IB", &o("and"));
    for bad in [
        vec!["math-fn", "Nope", f, "x.tif"],
        vec!["math-fn", "Ln", "5", "x.tif"],
        vec!["math-fn", "BitwiseAnd", f, "1", "x.tif"],
        vec!["math-fn", "Power", f, "x.tif"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(&bad)
            .output()
            .unwrap();
        assert!(!out.status.success(), "{bad:?}");
    }
    std::fs::remove_dir_all(&d).ok();
}
