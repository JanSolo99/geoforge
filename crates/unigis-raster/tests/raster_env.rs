//! Spatial Analyst environments against ArcGIS Pro 3.7 (testdata/arcgis-reference/raster_env.json: the output
//! grid and every cell of 23 runs with an environment set, made through the bridge on small synthetic rasters
//! that are rebuilt here).

use std::path::{Path, PathBuf};

use serde_json::Value;
use unigis_core::{Geometry, GeometryKind};
use unigis_raster::env::{self, Environment, Mask, Reads, Sampling, Snap};
use unigis_raster::focal::{FocalStatistic, Neighborhood, focal_statistics};
use unigis_raster::local::{MathOp, Operand, math};
use unigis_raster::surface::{SlopeUnit, aspect, curvature, hillshade, slope};
use unigis_raster::{DataType, Raster, hydro};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A 20 x 20 raster of 30 m cells with its lower-left corner at (500000, 5000000), `f(row, col)` top-down.
fn grid20(f: impl Fn(usize, usize) -> f64) -> Raster {
    let mut r = Raster::new(
        20,
        20,
        DataType::F32,
        [500_000.0, 30.0, 0.0, 5_000_600.0, 0.0, -30.0],
    );
    r.nodata = Some(-9999.0);
    for row in 0..20 {
        for col in 0..20 {
            r.bands[0][row * 20 + col] = f(row, col) as f32 as f64;
        }
    }
    r
}

fn input(name: &str) -> Raster {
    match name {
        // value = column * 100 + row
        "ramp" => grid20(|r, c| c as f64 * 100.0 + r as f64),
        // a cone: 100 - 4 * distance from the middle, in cells
        "dem" => grid20(|r, c| {
            100.0 - (((c as f64 - 9.5).powi(2) + (r as f64 - 9.5).powi(2)).sqrt()) * 4.0
        }),
        other => panic!("input {other}"),
    }
}

/// The snap raster: 30 m cells, lower-left corner (500007, 4999911), 10 x 10.
fn snap_raster() -> Raster {
    let mut r = Raster::new(
        10,
        10,
        DataType::F32,
        [500_007.0, 30.0, 0.0, 4_999_911.0 + 300.0, 0.0, -30.0],
    );
    r.nodata = Some(-9999.0);
    r
}

/// The mask raster: 6 x 6 cells at columns 5-10 and rows 4-9 of the DEM grid, one NoData hole.
fn mask_raster() -> Raster {
    let mut r = Raster::new(
        6,
        6,
        DataType::F32,
        [
            500_000.0 + 5.0 * 30.0,
            30.0,
            0.0,
            5_000_600.0 - 4.0 * 30.0,
            0.0,
            -30.0,
        ],
    );
    r.nodata = Some(-9999.0);
    r.bands[0] = vec![1.0; 36];
    r.bands[0][2 * 6 + 2] = -9999.0;
    r
}

fn triangle(points: &Value) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polygon;
    g.parts = vec![0];
    g.xy = points
        .as_array()
        .unwrap()
        .iter()
        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
        .collect();
    g
}

#[allow(clippy::field_reassign_with_default)]
fn environment(c: &Value, tri: &Value) -> Environment {
    let e = &c["env"];
    let mut out = Environment::default();
    out.cell_size = e["cell_size"].as_f64();
    out.extent = e["extent"].as_array().map(|v| {
        [
            v[0].as_f64().unwrap(),
            v[1].as_f64().unwrap(),
            v[2].as_f64().unwrap(),
            v[3].as_f64().unwrap(),
        ]
    });
    if e["snap"] == "snap" {
        out.snap = Some(Snap::of(&snap_raster()));
    }
    match e["mask"].as_str() {
        Some("mask") => out.mask = Some(Mask::Raster(mask_raster())),
        Some("triangle") => out.mask = Some(Mask::Polygons(vec![triangle(tri)])),
        _ => {}
    }
    out
}

/// The tool, with how it reads its input and samples it.
type Tool = (Reads, Sampling, Box<dyn Fn(&[Raster]) -> Raster>);

fn tool(name: &str, plus: f64) -> Tool {
    match name {
        "plus" => (
            Reads::Cropped,
            Sampling::Nearest,
            Box::new(move |r| {
                math(
                    MathOp::Plus,
                    &Operand::Raster(r[0].clone()),
                    &Operand::Constant(plus),
                )
                .unwrap()
            }),
        ),
        "slope" => (
            Reads::Context,
            Sampling::Bilinear,
            Box::new(|r| slope(&r[0], SlopeUnit::Degree, 1.0)),
        ),
        "aspect" => (
            Reads::Context,
            Sampling::Bilinear,
            Box::new(|r| aspect(&r[0])),
        ),
        "curvature" => (
            Reads::Context,
            Sampling::Bilinear,
            Box::new(|r| curvature(&r[0], 1.0).0),
        ),
        "flow_direction" => (
            Reads::Context,
            Sampling::Nearest,
            Box::new(|r| hydro::flow_direction(&r[0], false)),
        ),
        "hillshade" => (
            Reads::Cropped,
            Sampling::Bilinear,
            Box::new(|r| hillshade(&r[0], 315.0, 45.0, 1.0)),
        ),
        "focal_mean3" => (
            Reads::Cropped,
            Sampling::Nearest,
            Box::new(|r| {
                focal_statistics(
                    &r[0],
                    Neighborhood::parse("Rectangle 3 3 CELL").unwrap(),
                    FocalStatistic::parse("MEAN").unwrap(),
                    true,
                )
            }),
        ),
        "fill" => (
            Reads::Cropped,
            Sampling::Nearest,
            Box::new(|r| hydro::fill(&r[0], None)),
        ),
        other => panic!("tool {other}"),
    }
}

#[test]
fn environments_equal_arcgis() {
    let reference: Value = serde_json::from_slice(
        &std::fs::read(root().join("testdata/arcgis-reference/raster_env.json")).unwrap(),
    )
    .unwrap();
    let tri = &reference["triangle"];
    let mut failures = Vec::new();
    for c in reference["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let inputs: Vec<Raster> = c["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| input(i.as_str().unwrap()))
            .collect();
        // The constant each Plus run added.
        let plus = if name == "plus_ext_in" { 1.0 } else { 0.0 };
        let (reads, sampling, f) = tool(c["tool"].as_str().unwrap(), plus);
        let env = environment(c, tri);
        let refs: Vec<&Raster> = inputs.iter().collect();
        let Some(out) = env::run(&refs, &env, reads, sampling, &*f) else {
            failures.push(format!("{name}: no output"));
            continue;
        };
        let extent = out.extent();
        let want_extent: Vec<f64> = c["extent"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let (cols, rows) = (
            c["cols"].as_u64().unwrap() as usize,
            c["rows"].as_u64().unwrap() as usize,
        );
        if (out.width, out.height) != (cols, rows)
            || [extent.0, extent.1, extent.2, extent.3]
                .iter()
                .zip(&want_extent)
                .any(|(a, b)| (a - b).abs() > 1e-6)
        {
            failures.push(format!(
                "{name}: grid {}x{} {:?} but ArcGIS {cols}x{rows} {want_extent:?}",
                out.width, out.height, extent
            ));
            continue;
        }
        let want = c["values"].as_array().unwrap();
        let mut bad = 0;
        let mut first = None;
        for (i, w) in want.iter().enumerate() {
            let got = out.get(i % cols, i / cols);
            let ok = match (w.as_f64(), got) {
                (None, None) => true,
                (Some(a), Some(b)) => (a - b).abs() <= 1e-3 + 1e-5 * a.abs(),
                _ => false,
            };
            if !ok {
                bad += 1;
                first.get_or_insert((i, w.clone(), got));
            }
        }
        if bad > 0 {
            failures.push(format!(
                "{name}: {bad} of {} cells differ, first {:?}",
                want.len(),
                first
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn no_environment_changes_nothing() {
    let dem = input("dem");
    let out = env::run(
        &[&dem],
        &Environment::default(),
        Reads::Context,
        Sampling::Bilinear,
        &|r| slope(&r[0], SlopeUnit::Degree, 1.0),
    )
    .unwrap();
    assert_eq!(out.bands[0], slope(&dem, SlopeUnit::Degree, 1.0).bands[0]);
}
