//! Rescale by Function: each cell through a transformation function, then onto an evaluation
//! scale. The functions and their defaults are Esri's published formulas ("Formulas for the
//! transformation functions"), checked against ArcGIS Pro 3.7:
//!
//! - Shape parameters default from the lower and upper thresholds, which default to the raster's
//!   minimum and maximum; MSSmall / MSLarge use the raster's mean and sample standard deviation.
//! - The function values over the threshold interval are stretched linearly onto `from`..`to`
//!   (`from + (y - min y) * (to - from) / (max y - min y)`).
//! - Cells outside the thresholds take the value below / above (a number or NoData); without one,
//!   the value at the threshold.
//! - LogisticDecay is LogisticGrowth's formula with a y intercept of 99 % (the published decay form
//!   gives the mirror image of ArcGIS's output).
//!
//! The output is Float32, NoData -3.4028235e38.

use crate::{DataType, Raster};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    MsSmall,
    MsLarge,
    Linear,
    Exponential,
    Gaussian,
    Large,
    Logarithm,
    LogisticDecay,
    LogisticGrowth,
    Near,
    Power,
    Small,
    SymmetricLinear,
}

/// What cells outside a threshold get.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outside {
    Value(f64),
    NoData,
}

/// A transformation function with its parameters (`None`: the default).
#[derive(Debug, Clone, PartialEq)]
pub struct Transformation {
    pub function: Function,
    /// The two shape parameters (minimum/maximum, shift/factor, midpoint/spread, multipliers).
    pub a: Option<f64>,
    pub b: Option<f64>,
    /// The logistic functions' y intercept percent.
    pub y_intercept: Option<f64>,
    pub lower: Option<f64>,
    pub below: Option<Outside>,
    pub upper: Option<f64>,
    pub above: Option<Outside>,
}

impl Transformation {
    /// ArcGIS's text form: `LINEAR 0 100 # # # #`, `LOGISTICGROWTH # # 1 # # # #`
    /// (`#` or nothing for a default; `NoData` for an outside value).
    pub fn parse(text: &str) -> Option<Transformation> {
        let mut words = text.split_whitespace();
        let name = words.next()?.to_ascii_uppercase();
        let name = name.strip_prefix("TF").unwrap_or(&name).to_string();
        let function = match name.as_str() {
            "MSSMALL" => Function::MsSmall,
            "MSLARGE" => Function::MsLarge,
            "LINEAR" => Function::Linear,
            "EXPONENTIAL" => Function::Exponential,
            "GAUSSIAN" => Function::Gaussian,
            "LARGE" => Function::Large,
            "LOGARITHM" => Function::Logarithm,
            "LOGISTICDECAY" => Function::LogisticDecay,
            "LOGISTICGROWTH" => Function::LogisticGrowth,
            "NEAR" => Function::Near,
            "POWER" => Function::Power,
            "SMALL" => Function::Small,
            "SYMMETRICLINEAR" => Function::SymmetricLinear,
            _ => return None,
        };
        let words: Vec<&str> = words.collect();
        let number = |k: usize| -> Option<Option<f64>> {
            match words.get(k) {
                None | Some(&"#") => Some(None),
                Some(w) => w.parse().ok().map(Some),
            }
        };
        let outside = |k: usize| -> Option<Option<Outside>> {
            match words.get(k) {
                None | Some(&"#") => Some(None),
                Some(w) if w.eq_ignore_ascii_case("NODATA") => Some(Some(Outside::NoData)),
                Some(w) => w.parse().ok().map(|v| Some(Outside::Value(v))),
            }
        };
        let logistic = matches!(function, Function::LogisticDecay | Function::LogisticGrowth);
        let o = usize::from(logistic);
        Some(Transformation {
            function,
            a: number(0)?,
            b: number(1)?,
            y_intercept: if logistic { number(2)? } else { None },
            lower: number(2 + o)?,
            below: outside(3 + o)?,
            upper: number(4 + o)?,
            above: outside(5 + o)?,
        })
    }
}

/// Mean and sample standard deviation, minimum and maximum of the valid cells.
fn statistics(values: &[f64]) -> Option<(f64, f64, f64, f64)> {
    if values.is_empty() {
        return None;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some((mean, var.sqrt(), min, max))
}

/// The function `t` with its defaults resolved, for thresholds `lo`..`hi`; and the points where it
/// may peak inside them.
fn function(
    single: bool,
    t: &Transformation,
    (lo, hi): (f64, f64),
    from: f64,
    to: f64,
    (mean, std): (f64, f64),
) -> (Box<dyn Fn(f64) -> f64>, Vec<f64>) {
    let mid_default = (hi + lo) / 2.0;
    let rising = to > from;
    // ArcGIS keeps the resolved parameters as single precision for the cells (observable where the
    // function is steep: Logarithm's default shift).
    let p = |v: f64| if single { f64::from(v as f32) } else { v };
    match t.function {
        Function::Linear => {
            let (min, max) = (t.a.unwrap_or(lo), t.b.unwrap_or(hi));
            let diff = max - min;
            let f = move |x: f64| {
                if min < max {
                    if x < min {
                        0.0
                    } else if x > max {
                        1.0
                    } else {
                        (x - min) / diff
                    }
                } else if x > min {
                    0.0
                } else if x < max {
                    1.0
                } else {
                    (x - min) / diff
                }
            };
            (Box::new(f), vec![])
        }
        Function::SymmetricLinear => {
            let (min, max) = (t.a.unwrap_or(lo), t.b.unwrap_or(hi));
            let hdiff = 0.5 * (max - min);
            let midp = min + hdiff;
            let f = move |x: f64| {
                if min < max {
                    if x < min {
                        0.0
                    } else if x < midp {
                        (x - min) / hdiff
                    } else if x > max {
                        0.0
                    } else {
                        (max - x) / hdiff
                    }
                } else if x < max {
                    1.0
                } else if x < midp {
                    (x - midp) / hdiff
                } else if x > min {
                    1.0
                } else {
                    (midp - x) / hdiff
                }
            };
            (Box::new(f), vec![midp])
        }
        Function::MsSmall | Function::MsLarge => {
            let n_mean = t.a.unwrap_or(1.0) * mean;
            let n_std = t.b.unwrap_or(1.0) * std;
            let small = t.function == Function::MsSmall;
            let f = move |x: f64| {
                if x > n_mean {
                    let s = n_std / (x - n_mean + n_std);
                    if small { s } else { 1.0 - s }
                } else if small {
                    1.0
                } else {
                    0.0
                }
            };
            (Box::new(f), vec![])
        }
        Function::Exponential => {
            let (lf, lt) = (from.ln(), to.ln());
            let shift = p(t.a.unwrap_or(if rising {
                (lo * lt - hi * lf) / (lt - lf)
            } else {
                (lo * lf - hi * lt) / (lf - lt)
            }));
            let base = p(t.b.unwrap_or(if rising {
                (lt - lf) / (hi - lo)
            } else {
                (lf - lt) / (hi - lo)
            }));
            (Box::new(move |x: f64| ((x - shift) * base).exp()), vec![])
        }
        Function::Logarithm => {
            let (ef, et) = (from.exp(), to.exp());
            let shift = p(t.a.unwrap_or(if rising {
                (lo * et - hi * ef) / (et - ef)
            } else {
                (lo * ef - hi * et) / (ef - et)
            }));
            let factor = p(t.b.unwrap_or(if rising {
                (et - ef) / (hi - lo)
            } else {
                (ef - et) / (hi - lo)
            }));
            (Box::new(move |x: f64| ((x - shift) * factor).ln()), vec![])
        }
        Function::Gaussian | Function::Near => {
            let mid = t.a.unwrap_or(mid_default);
            let gaussian = t.function == Function::Gaussian;
            let spread = p(t.b.unwrap_or(if gaussian {
                10f64.ln() * 4.0 / (mid - lo).powi(2)
            } else {
                36.0 / (mid - lo).powi(2)
            }));
            let f = move |x: f64| {
                if gaussian {
                    (-spread * (x - mid).powi(2)).exp()
                } else {
                    1.0 / (1.0 + spread * (x - mid).powi(2))
                }
            };
            (Box::new(f), vec![mid])
        }
        Function::Large | Function::Small => {
            let mid = t.a.unwrap_or(mid_default);
            let spread = p(t.b.unwrap_or(5.0));
            let sign = if t.function == Function::Large {
                -1.0
            } else {
                1.0
            };
            (
                Box::new(move |x: f64| 1.0 / (1.0 + (x / mid).powf(sign * spread))),
                vec![],
            )
        }
        Function::LogisticGrowth | Function::LogisticDecay => {
            let (min, max) = (t.a.unwrap_or(lo), t.b.unwrap_or(hi));
            let c = 100.0;
            let y = t
                .y_intercept
                .unwrap_or(if t.function == Function::LogisticGrowth {
                    1.0
                } else {
                    99.0
                });
            let a = c / y - 1.0;
            let b = a.ln() / (0.5 * (max + min) - min);
            (
                Box::new(move |x: f64| c / (1.0 + a * (-(x - min) * b).exp())),
                vec![],
            )
        }
        Function::Power => {
            let shift = p(t.a.unwrap_or(if rising {
                if from == 1.0 { lo - 1.0 } else { lo }
            } else if to == 1.0 {
                lo - 1.0
            } else {
                lo
            }));
            let exponent = p(t.b.unwrap_or(if rising {
                if from == 0.0 {
                    if to <= 1.0 {
                        1.0
                    } else {
                        to.ln() / (hi - shift)
                    }
                } else if from == 1.0 {
                    to.ln() / (hi - shift).ln()
                } else {
                    2.0
                }
            } else if to == 0.0 {
                if from <= 1.0 {
                    1.0
                } else {
                    from.ln() / (hi - shift)
                }
            } else if to == 1.0 {
                from.ln() / (hi - shift).ln()
            } else {
                2.0
            }));
            (Box::new(move |x: f64| (x - shift).powf(exponent)), vec![])
        }
    }
}

/// `RescaleByFunction(in_raster, transformation_function, from_scale, to_scale)`.
pub fn rescale_by_function(r: &Raster, t: &Transformation, from: f64, to: f64) -> Raster {
    let values: Vec<f64> = r.bands[0]
        .iter()
        .copied()
        .filter(|&v| !r.is_nodata(v) && v.is_finite())
        .collect();
    let mut out = r.like(DataType::F32);
    let nodata = f64::from(f32::MIN);
    out.nodata = Some(nodata);
    let Some((mean, std, min, max)) = statistics(&values) else {
        out.bands = vec![vec![nodata; r.width * r.height]];
        return out;
    };
    let (lo, hi) = (t.lower.unwrap_or(min), t.upper.unwrap_or(max));
    let (f, peaks) = function(false, t, (lo, hi), from, to, (mean, std));
    let (cell_f, _) = function(true, t, (lo, hi), from, to, (mean, std));
    // The function's range over the threshold interval.
    let mut ys: Vec<f64> = [lo, hi]
        .into_iter()
        .chain(peaks.into_iter().filter(|p| (lo..=hi).contains(p)))
        .map(&f)
        .filter(|y| y.is_finite())
        .collect();
    ys.extend(
        values
            .iter()
            .filter(|v| (lo..=hi).contains(*v))
            .map(|&v| f(v)),
    );
    let ymin = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let ymax = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let scale = |y: f64| {
        if ymax > ymin {
            from + (y - ymin) * (to - from) / (ymax - ymin)
        } else {
            from
        }
    };
    let outside = |o: Option<Outside>, at: f64| match o {
        Some(Outside::Value(v)) => v,
        Some(Outside::NoData) => nodata,
        None => scale(cell_f(at)),
    };
    out.bands = vec![
        r.bands[0]
            .iter()
            .map(|&v| {
                let value = if r.is_nodata(v) || !v.is_finite() {
                    return nodata;
                } else if v < lo {
                    outside(t.below, lo)
                } else if v > hi {
                    outside(t.above, hi)
                } else {
                    scale(cell_f(v))
                };
                if value == nodata {
                    nodata
                } else {
                    DataType::F32.store(value)
                }
            })
            .collect(),
    ];
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(values: &[f64]) -> Raster {
        let mut r = Raster::new(
            values.len(),
            1,
            DataType::F32,
            [0.0, 1.0, 0.0, 1.0, 0.0, -1.0],
        );
        r.nodata = Some(-9999.0);
        r.bands[0] = values.to_vec();
        r
    }

    fn run(text: &str, from: f64, to: f64) -> Vec<f64> {
        let r = grid(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, -9999.0]);
        let t = Transformation::parse(text).unwrap();
        rescale_by_function(&r, &t, from, to).bands[0].clone()
    }

    fn close(a: &[f64], b: &[f64]) {
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() <= 1e-5 * y.abs().max(1.0), "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn defaults_match_arcgis() {
        // ArcGIS Pro 3.7 on 0..8 (scale 1..10).
        close(
            &run("LINEAR", 1.0, 10.0),
            &[1.0, 2.125, 3.25, 4.375, 5.5, 6.625, 7.75, 8.875, 10.0],
        );
        close(
            &run("MSSMALL", 1.0, 10.0)[5..8],
            &[5.944517612457275, 3.6007094383239746, 2.0737576484680176],
        );
        close(
            &run("GAUSSIAN", 1.0, 10.0)[1..4],
            &[1.0497156381607056, 1.8991899490356445, 6.060678005218506],
        );
        close(
            &run("LOGISTICDECAY", 1.0, 10.0)[..4],
            &[10.0, 9.808262, 9.253135, 7.881216],
        );
        close(
            &run("EXPONENTIAL", 10.0, 1.0)[..4],
            &[10.0, 9.666478, 9.221721, 8.628626],
        );
        close(
            &run("POWER", 0.0, 5.0)[..4],
            &[0.0, 3.290688, 3.7831, 4.104628],
        );
        assert_eq!(run("LINEAR", 1.0, 10.0)[9], f64::from(f32::MIN));
    }

    #[test]
    fn thresholds_match_arcgis() {
        close(
            &run("LINEAR # # 2 # 6 #", 1.0, 10.0),
            &[1.0, 1.0, 1.0, 3.25, 5.5, 7.75, 10.0, 10.0, 10.0],
        );
        let v = run("LINEAR # # 2 0 6 NoData", 1.0, 10.0);
        close(&v[..7], &[0.0, 0.0, 1.0, 3.25, 5.5, 7.75, 10.0]);
        assert_eq!(v[7], f64::from(f32::MIN));
        close(
            &run("LINEAR # # -2 # 12 #", 1.0, 10.0)[..3],
            &[2.285715, 2.928572, 3.571429],
        );
        close(
            &run("GAUSSIAN # # 1 # 5 #", 1.0, 10.0)[..5],
            &[1.0, 1.0, 1.89919, 10.0, 1.89919],
        );
        close(
            &run("EXPONENTIAL # # 2 # 6 #", 1.0, 10.0),
            &[
                1.0, 1.0, 1.0, 1.778279, 3.162278, 5.623414, 10.0, 10.0, 10.0,
            ],
        );
    }
}
