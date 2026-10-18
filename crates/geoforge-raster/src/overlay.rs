//! Spatial Analyst Overlay and Local tools over several rasters: Cell Statistics, Weighted Sum,
//! Highest/Lowest Position, Combine, Weighted Overlay, Fuzzy Membership and Fuzzy Overlay.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/overlay.json` and `overlay_types.json`,
//! runs through the bridge on 8 x 8 rasters with NoData):
//! - Inputs go onto the local grid (the intersection, the largest cell) like the other local
//!   tools; a cell that is NoData in any input is NoData unless a tool says otherwise.
//! - Cell Statistics with `ignore_nodata` (DATA) uses the cells that have values and is NoData
//!   only where none has; with NODATA, one NoData input makes the cell NoData. STD is the
//!   population deviation; MAJORITY and MINORITY give the smallest value among the ties; MEDIAN of
//!   an even count averages the middle two in float output and takes the lower one in integer
//!   output. MEAN and STD are float; VARIETY is integer; the rest are float with any float input,
//!   else integer with ArcGIS's integer type choice.
//! - Weighted Sum adds `weight * value` and is always float. Highest and Lowest Position give the
//!   1-based index of the first raster holding the largest/smallest value.
//! - Combine numbers the distinct value combinations in the order the cells first show them,
//!   reading rows from the top.
//! - Weighted Overlay: each input's values map to the evaluation scale (or NoData), the scaled
//!   values are weighted by the influence (percent) and the sum is rounded half up; the output is
//!   an 8-bit signed integer (-128 is NoData).
//! - Fuzzy Membership classes follow Esri's formulas (the mean and standard deviation of MS Large
//!   and MS Small are the input's, the deviation the sample one); Fuzzy Overlay ignores nothing:
//!   any NoData input is NoData.

use crate::local::local_grid;
use crate::{DataType, Raster};

/// The inputs on their shared local grid, plus that grid's size. `None` when they do not overlap.
fn aligned(rasters: &[&Raster]) -> Option<(Vec<Raster>, usize, usize)> {
    let (t, cols, rows) = local_grid(rasters)?;
    Some((
        rasters.iter().map(|r| r.onto(t, cols, rows)).collect(),
        cols,
        rows,
    ))
}

fn float_output(grid: &Raster, values: impl Iterator<Item = Option<f64>>) -> Raster {
    let nodata = f32::MIN as f64;
    let mut out = grid.like(DataType::F32);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .map(|v| {
            v.filter(|v| v.is_finite())
                .map_or(nodata, |v| DataType::F32.store(v))
        })
        .collect();
    out
}

/// What Cell Statistics computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStat {
    Sum,
    Mean,
    Maximum,
    Minimum,
    Median,
    Majority,
    Minority,
    Range,
    Std,
    Variety,
}

impl CellStat {
    pub fn parse(s: &str) -> Option<CellStat> {
        Some(match s.to_ascii_uppercase().as_str() {
            "SUM" => CellStat::Sum,
            "MEAN" => CellStat::Mean,
            "MAXIMUM" => CellStat::Maximum,
            "MINIMUM" => CellStat::Minimum,
            "MEDIAN" => CellStat::Median,
            "MAJORITY" => CellStat::Majority,
            "MINORITY" => CellStat::Minority,
            "RANGE" => CellStat::Range,
            "STD" => CellStat::Std,
            "VARIETY" => CellStat::Variety,
            _ => return None,
        })
    }
}

/// The statistic of one cell's values (never empty).
fn statistic(stat: CellStat, v: &mut [f64], float: bool) -> f64 {
    let n = v.len() as f64;
    match stat {
        CellStat::Sum => v.iter().sum(),
        CellStat::Mean => v.iter().sum::<f64>() / n,
        CellStat::Maximum => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        CellStat::Minimum => v.iter().copied().fold(f64::INFINITY, f64::min),
        CellStat::Range => {
            v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - v.iter().copied().fold(f64::INFINITY, f64::min)
        }
        CellStat::Std => {
            let m = v.iter().sum::<f64>() / n;
            (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n).sqrt()
        }
        CellStat::Median => {
            v.sort_by(|a, b| a.total_cmp(b));
            let k = v.len();
            if k % 2 == 1 {
                v[k / 2]
            } else if float {
                (v[k / 2 - 1] + v[k / 2]) / 2.0
            } else {
                v[k / 2 - 1]
            }
        }
        CellStat::Variety | CellStat::Majority | CellStat::Minority => {
            v.sort_by(|a, b| a.total_cmp(b));
            // Runs of equal values, smallest first.
            let mut runs: Vec<(f64, usize)> = Vec::new();
            for &x in v.iter() {
                match runs.last_mut() {
                    Some((y, c)) if *y == x => *c += 1,
                    _ => runs.push((x, 1)),
                }
            }
            match stat {
                CellStat::Variety => runs.len() as f64,
                CellStat::Majority => {
                    let top = runs.iter().map(|r| r.1).max().unwrap_or(0);
                    runs.iter().find(|r| r.1 == top).map_or(0.0, |r| r.0)
                }
                _ => {
                    let low = runs.iter().map(|r| r.1).min().unwrap_or(0);
                    runs.iter().find(|r| r.1 == low).map_or(0.0, |r| r.0)
                }
            }
        }
    }
}

/// Cell Statistics. `ignore_nodata` is ArcGIS's DATA (true) or NODATA (false).
pub fn cell_statistics(rasters: &[&Raster], stat: CellStat, ignore_nodata: bool) -> Option<Raster> {
    let (ins, cols, rows) = aligned(rasters)?;
    let float_in = ins.iter().any(|r| r.data_type.is_float());
    let wide_signed = ins.iter().all(|r| r.data_type == DataType::I32);
    let mut values: Vec<Option<f64>> = Vec::with_capacity(cols * rows);
    let mut cell: Vec<f64> = Vec::with_capacity(ins.len());
    for i in 0..cols * rows {
        cell.clear();
        let mut missing = false;
        for r in &ins {
            let v = r.bands[0][i];
            if r.is_nodata(v) {
                missing = true;
            } else {
                cell.push(v);
            }
        }
        values.push(if cell.is_empty() || (missing && !ignore_nodata) {
            None
        } else {
            let float_out = float_in || matches!(stat, CellStat::Mean | CellStat::Std);
            Some(statistic(stat, &mut cell, float_out))
        });
    }
    let float_out =
        (float_in && stat != CellStat::Variety) || matches!(stat, CellStat::Mean | CellStat::Std);
    if float_out {
        return Some(float_output(&ins[0], values.into_iter()));
    }
    let (lo, hi) = predicted_range(&ins, stat);
    Some(integer_output_raster(&ins[0], &values, lo, hi, wide_signed))
}

/// The value range an integer result is typed for: what the inputs' statistics (else their types)
/// allow, not the values that turn up (observed: Variety of two identical rasters still gets the
/// 2-bit layout).
fn predicted_range(ins: &[Raster], stat: CellStat) -> (f64, f64) {
    let ranges: Vec<(f64, f64)> = ins
        .iter()
        .map(|r| {
            r.statistics
                .unwrap_or_else(|| crate::local::type_range_of(r.data_type))
        })
        .collect();
    let lo = ranges.iter().map(|r| r.0).fold(f64::INFINITY, f64::min);
    let hi = ranges.iter().map(|r| r.1).fold(f64::NEG_INFINITY, f64::max);
    match stat {
        CellStat::Sum => (
            ranges.iter().map(|r| r.0).sum(),
            ranges.iter().map(|r| r.1).sum(),
        ),
        CellStat::Range => (0.0, hi - lo),
        CellStat::Variety => (1.0, ins.len() as f64),
        _ => (lo, hi),
    }
}

/// `values` as an integer raster typed for `lo..=hi`; wide signed input keeps its signed 32-bit
/// type once the range outgrows 16 bits.
fn integer_output_raster(
    grid: &Raster,
    values: &[Option<f64>],
    lo: f64,
    hi: f64,
    wide_signed: bool,
) -> Raster {
    let (mut t, mut nodata) = crate::zonal::integer_output(lo, hi, true);
    // Values 0 and 1 only (observed): an unsigned byte with NoData 16.
    if lo == 0.0 && hi == 1.0 {
        (t, nodata) = (DataType::U8, 16.0);
    }
    if wide_signed && t == DataType::U32 {
        (t, nodata) = (DataType::I32, i32::MIN as f64);
    }
    let mut out = grid.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .iter()
        .map(|v| v.map_or(nodata, |v| t.store(v)))
        .collect();
    out
}

/// Weighted Sum of rasters' values (`weights` pairs with `rasters`): float, NoData where any input is.
pub fn weighted_sum(rasters: &[&Raster], weights: &[f64]) -> Option<Raster> {
    if rasters.len() != weights.len() {
        return None;
    }
    let (ins, cols, rows) = aligned(rasters)?;
    let values = (0..cols * rows).map(|i| {
        let mut sum = 0.0;
        for (r, w) in ins.iter().zip(weights) {
            let v = r.bands[0][i];
            if r.is_nodata(v) {
                return None;
            }
            sum += w * v;
        }
        Some(sum)
    });
    Some(float_output(&ins[0], values))
}

/// Highest (`highest`) or Lowest Position: the 1-based index of the first raster holding the
/// extreme value; NoData where any input is.
pub fn position(rasters: &[&Raster], highest: bool) -> Option<Raster> {
    let (ins, cols, rows) = aligned(rasters)?;
    let values: Vec<Option<f64>> = (0..cols * rows)
        .map(|i| {
            let mut best: Option<(usize, f64)> = None;
            for (k, r) in ins.iter().enumerate() {
                let v = r.bands[0][i];
                if r.is_nodata(v) {
                    return None;
                }
                if best.is_none_or(|(_, b)| if highest { v > b } else { v < b }) {
                    best = Some((k, v));
                }
            }
            best.map(|(k, _)| (k + 1) as f64)
        })
        .collect();
    Some(integer_output_raster(
        &ins[0],
        &values,
        1.0,
        ins.len() as f64,
        false,
    ))
}

/// Combine: a new value per distinct combination of the inputs' values, numbered from 1 in the
/// order the cells (top row first) first show them; NoData where any input is.
pub fn combine(rasters: &[&Raster]) -> Option<Raster> {
    let (ins, cols, rows) = aligned(rasters)?;
    let mut seen: std::collections::HashMap<Vec<u64>, f64> = std::collections::HashMap::new();
    let values: Vec<Option<f64>> = (0..cols * rows)
        .map(|i| {
            let mut key = Vec::with_capacity(ins.len());
            for r in &ins {
                let v = r.bands[0][i];
                if r.is_nodata(v) {
                    return None;
                }
                key.push(v.to_bits());
            }
            let next = (seen.len() + 1) as f64;
            Some(*seen.entry(key).or_insert(next))
        })
        .collect();
    // Signed 8-bit while the numbers fit (NoData -128), then wider.
    let top = seen.len() as f64;
    let (t, nodata) = if top <= 127.0 {
        (DataType::I8, -128.0)
    } else if top <= i16::MAX as f64 {
        (DataType::I16, i16::MIN as f64)
    } else {
        (DataType::I32, i32::MIN as f64)
    };
    let mut out = ins[0].like(t);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .iter()
        .map(|v| v.map_or(nodata, |v| t.store(v)))
        .collect();
    Some(out)
}

/// How a Weighted Overlay input value maps onto the evaluation scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    Value(f64),
    NoData,
}

/// One Weighted Overlay input: the raster, its influence in percent and the value → scale pairs
/// (a value not listed becomes NoData).
pub struct Weighted<'a> {
    pub raster: &'a Raster,
    pub influence: f64,
    pub remap: Vec<(f64, Scale)>,
}

/// Weighted Overlay. `None` when the inputs do not overlap or the influences do not sum to 100.
pub fn weighted_overlay(inputs: &[Weighted], scale: (f64, f64)) -> Option<Raster> {
    if inputs.is_empty() || (inputs.iter().map(|w| w.influence).sum::<f64>() - 100.0).abs() > 1e-9 {
        return None;
    }
    let rasters: Vec<&Raster> = inputs.iter().map(|w| w.raster).collect();
    let (ins, cols, rows) = aligned(&rasters)?;
    let values: Vec<Option<f64>> = (0..cols * rows)
        .map(|i| {
            let mut sum = 0.0;
            for (r, w) in ins.iter().zip(inputs) {
                let v = r.bands[0][i];
                if r.is_nodata(v) {
                    return None;
                }
                match w.remap.iter().find(|(from, _)| *from == v)?.1 {
                    Scale::Value(s) => sum += (w.influence / 100.0) * s,
                    Scale::NoData => return None,
                }
            }
            Some((sum + 0.5).floor())
        })
        .collect();
    // 8-bit signed unless the scale needs more.
    let (lo, hi) = scale;
    let t = if lo >= -127.0 && hi <= 127.0 {
        DataType::I8
    } else if lo >= i16::MIN as f64 + 1.0 && hi <= i16::MAX as f64 {
        DataType::I16
    } else {
        DataType::I32
    };
    let nodata = match t {
        DataType::I8 => -128.0,
        DataType::I16 => i16::MIN as f64,
        _ => i32::MIN as f64,
    };
    let mut out = ins[0].like(t);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .iter()
        .map(|v| v.map_or(nodata, |v| t.store(v)))
        .collect();
    Some(out)
}

/// A fuzzification function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fuzzy {
    /// 0 up to `min`, rising linearly to 1 at `max` (decreasing when `min > max`).
    Linear {
        min: f64,
        max: f64,
    },
    Gaussian {
        midpoint: f64,
        spread: f64,
    },
    Large {
        midpoint: f64,
        spread: f64,
    },
    Small {
        midpoint: f64,
        spread: f64,
    },
    /// Large around the input's mean and deviation, scaled by the multipliers.
    MsLarge {
        mean_multiplier: f64,
        std_multiplier: f64,
    },
    MsSmall {
        mean_multiplier: f64,
        std_multiplier: f64,
    },
    Near {
        midpoint: f64,
        spread: f64,
    },
}

/// A hedge applied to the membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hedge {
    None,
    Somewhat,
    Very,
}

/// Fuzzy Membership of `r`'s values in `class`.
pub fn fuzzy_membership(r: &Raster, class: Fuzzy, hedge: Hedge) -> Raster {
    let valid: Vec<f64> = r.bands[0]
        .iter()
        .copied()
        .filter(|v| !r.is_nodata(*v))
        .collect();
    let n = valid.len() as f64;
    let mean = if n > 0.0 {
        valid.iter().sum::<f64>() / n
    } else {
        0.0
    };
    let std = if n > 1.0 {
        (valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        0.0
    };
    let f = |x: f64| -> f64 {
        match class {
            Fuzzy::Linear { min, max } => {
                if min == max {
                    if x < min { 0.0 } else { 1.0 }
                } else if min < max {
                    ((x - min) / (max - min)).clamp(0.0, 1.0)
                } else {
                    ((x - max) / (min - max)).clamp(0.0, 1.0).mul_add(-1.0, 1.0)
                }
            }
            Fuzzy::Gaussian { midpoint, spread } => (-spread * (x - midpoint).powi(2)).exp(),
            Fuzzy::Large { midpoint, spread } => 1.0 / (1.0 + (x / midpoint).powf(-spread)),
            Fuzzy::Small { midpoint, spread } => 1.0 / (1.0 + (x / midpoint).powf(spread)),
            Fuzzy::Near { midpoint, spread } => 1.0 / (1.0 + spread * (x - midpoint).powi(2)),
            Fuzzy::MsLarge {
                mean_multiplier,
                std_multiplier,
            } => {
                let (m, s) = (mean_multiplier * mean, std_multiplier * std);
                if x <= m { 0.0 } else { (x - m) / (x - m + s) }
            }
            Fuzzy::MsSmall {
                mean_multiplier,
                std_multiplier,
            } => {
                let (m, s) = (mean_multiplier * mean, std_multiplier * std);
                if x <= m { 1.0 } else { s / (x - m + s) }
            }
        }
    };
    float_output(
        r,
        r.bands[0].iter().map(|&v| {
            if r.is_nodata(v) {
                return None;
            }
            let m = f(v);
            Some(match hedge {
                Hedge::None => m,
                Hedge::Somewhat => m.sqrt(),
                Hedge::Very => m * m,
            })
        }),
    )
}

/// How Fuzzy Overlay combines memberships.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FuzzyOverlayType {
    And,
    Or,
    Product,
    Sum,
    Gamma(f64),
}

/// Fuzzy Overlay of membership rasters; NoData where any input is.
pub fn fuzzy_overlay(rasters: &[&Raster], kind: FuzzyOverlayType) -> Option<Raster> {
    let (ins, cols, rows) = aligned(rasters)?;
    let values = (0..cols * rows).map(|i| {
        let mut xs = Vec::with_capacity(ins.len());
        for r in &ins {
            let v = r.bands[0][i];
            if r.is_nodata(v) {
                return None;
            }
            xs.push(v);
        }
        let product: f64 = xs.iter().product();
        let sum = 1.0 - xs.iter().map(|x| 1.0 - x).product::<f64>();
        Some(match kind {
            FuzzyOverlayType::And => xs.iter().copied().fold(f64::INFINITY, f64::min),
            FuzzyOverlayType::Or => xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            FuzzyOverlayType::Product => product,
            FuzzyOverlayType::Sum => sum,
            FuzzyOverlayType::Gamma(g) => sum.powf(g) * product.powf(1.0 - g),
        })
    });
    Some(float_output(&ins[0], values))
}
