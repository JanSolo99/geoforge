//! Spatial Analyst math functions against ArcGIS Pro 3.7 (testdata/arcgis-reference/mathfn.json).

use std::path::{Path, PathBuf};

use geoforge_raster::local::Operand;
use geoforge_raster::mathfn::{Binary, Unary, binary, unary};
use geoforge_raster::{DataType, Raster};
use serde_json::Value;

const ND: f64 = -9999.0;
const SENTINEL: f64 = -123_456_789.0;

fn reference() -> Value {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/mathfn.json")).unwrap(),
    )
    .unwrap()
}

fn raster(rows: &Value, t: DataType, shift: f64) -> Raster {
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
            let v = if v == ND { v } else { t.store(v + shift) };
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
    // NoData of integer outputs (float NoData is f32::MIN everywhere).
    if !got.data_type.is_float()
        && let Some(nd) = want["nodata"].as_f64()
        && got.nodata != Some(nd)
    {
        failures.push(format!("{key}: NoData {:?} but ArcGIS {nd}", got.nodata));
    }
    let mut bad = 0;
    let mut first = None;
    for (y, row) in want["values"].as_array().unwrap().iter().enumerate() {
        for (x, w) in row.as_array().unwrap().iter().enumerate() {
            let w = w.as_f64().unwrap();
            let ok = match got.get(x, y) {
                None => (w - SENTINEL).abs() < 10.0,
                Some(g) => (w - SENTINEL).abs() >= 10.0 && (g - w).abs() <= 1e-5 * w.abs().max(1.0),
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
fn math_functions_equal_arcgis() {
    let r = reference();
    let inputs = &r["inputs"];
    let f = raster(&inputs["F"], DataType::F32, 0.0);
    let g = raster(&inputs["F"], DataType::F32, 0.5);
    let i = raster(&inputs["I"], DataType::I32, 0.0);
    let b = raster(&inputs["B"], DataType::I32, 0.0);
    let by = |t: &str| match t {
        "F" => f.clone(),
        "G" => g.clone(),
        "I" => i.clone(),
        "B" => b.clone(),
        other => panic!("{other}"),
    };
    let mut failures = Vec::new();
    let mut ran = 0;
    for (key, want) in r["cases"].as_object().unwrap() {
        let (op, tag) = key.rsplit_once('_').unwrap();
        let out: Option<Raster> = if let Some(u) = Unary::parse(op).filter(|_| tag.len() == 1) {
            Some(unary(u, &by(tag)))
        } else {
            // Binary: two letters of rasters, or a case with a constant.
            let operands: Option<(Operand, Operand)> = match key.as_str() {
                "Power_F_const" => Some((Operand::Raster(f.clone()), Operand::Constant(2.0))),
                "Power_I_const2" => Some((Operand::Raster(i.clone()), Operand::Constant(2.0))),
                "Power_const_I" => Some((Operand::Constant(2.0), Operand::Raster(b.clone()))),
                "Mod_I_const" => Some((Operand::Raster(i.clone()), Operand::Constant(3.0))),
                "Mod_F_const" => Some((Operand::Raster(f.clone()), Operand::Constant(0.7))),
                "Mod_I_zero" => Some((Operand::Raster(i.clone()), Operand::Constant(0.0))),
                "GreaterThan_F_const" => Some((Operand::Raster(f.clone()), Operand::Constant(0.0))),
                "EqualTo_I_const" => Some((Operand::Raster(i.clone()), Operand::Constant(3.0))),
                "BitwiseLeftShift_B" | "BitwiseRightShift_B" => {
                    Some((Operand::Raster(b.clone()), Operand::Constant(2.0)))
                }
                "BitwiseLeftShift_BB" | "BitwiseRightShift_BB" => {
                    Some((Operand::Raster(i.clone()), Operand::Raster(b.clone())))
                }
                _ if tag.len() == 2 && Binary::parse(op).is_some() => {
                    let t: Vec<char> = tag.chars().collect();
                    let pick = |c: char| match c {
                        'F' => "F",
                        'G' => "G",
                        'I' => "I",
                        'B' => "B",
                        other => panic!("{other}"),
                    };
                    Some((
                        Operand::Raster(by(pick(t[0]))),
                        Operand::Raster(by(pick(t[1]))),
                    ))
                }
                _ => None,
            };
            operands.and_then(|(a, c)| Binary::parse(op).and_then(|bo| binary(bo, &a, &c)))
        };
        let Some(out) = out else { continue };
        ran += 1;
        failures.extend(judge(key, &out, want));
    }
    assert!(ran >= 125, "ran {ran} cases");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
