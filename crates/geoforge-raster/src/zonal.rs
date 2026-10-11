//! Spatial Analyst Reclassify and Zonal Statistics.

use std::collections::BTreeMap;

use crate::{DataType, Raster};

/// The integer cell type and NoData ArcGIS gives an integer result whose values
/// run from `min` to `max` (observed): results from 8-bit input may be stored with
/// 1, 2 or 4 bits (NoData 1, 3 or 15, or 0 when the top value is used and 0 is not) or as signed then unsigned 8-bit; wider
/// input gives unsigned 16-bit when there are no negative values, else signed
/// 16-bit, then 32-bit.
pub fn integer_output(min: f64, max: f64, eight_bit_input: bool) -> (DataType, f64) {
    if eight_bit_input {
        if min >= 0.0 {
            for bits in [1u32, 2, 4] {
                let top = ((1u32 << bits) - 1) as f64;
                if max < top {
                    return (DataType::U8, top);
                }
                // Every value up to the top is possible but 0 is unused.
                if max == top && min > 0.0 {
                    return (DataType::U8, 0.0);
                }
            }
        }
        if min >= -127.0 && max <= 127.0 {
            return (DataType::I8, -128.0);
        }
        if min >= 0.0 && max < 255.0 {
            return (DataType::U8, 255.0);
        }
    }
    if min >= 0.0 && max < u16::MAX as f64 {
        (DataType::U16, u16::MAX as f64)
    } else if min > i16::MIN as f64 && max <= i16::MAX as f64 {
        (DataType::I16, i16::MIN as f64)
    } else if min >= 0.0 && max < u32::MAX as f64 {
        (DataType::U32, u32::MAX as f64)
    } else {
        (DataType::I32, i32::MIN as f64)
    }
}

fn is_eight_bit(t: DataType) -> bool {
    matches!(t, DataType::U8 | DataType::I8)
}

/// Writes `values` (None = NoData) into a raster like `grid` with ArcGIS's
/// integer type choice.
fn integer_raster(grid: &Raster, values: &[Option<f64>], eight_bit_input: bool) -> Raster {
    let (min, max) = values
        .iter()
        .flatten()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    let (min, max) = if min.is_finite() {
        (min, max)
    } else {
        (0.0, 0.0)
    };
    let (data_type, nodata) = integer_output(min, max, eight_bit_input);
    let mut out = grid.like(data_type);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .iter()
        .map(|v| v.map_or(nodata, |v| data_type.store(v)))
        .collect();
    out
}

/// One Reclassify remap entry: an exact old value or an inclusive range, and the
/// new value (`None` = NoData).
#[derive(Debug, Clone, PartialEq)]
pub struct Remap {
    pub from: f64,
    pub to: f64,
    pub new: Option<f64>,
}

/// Parses ArcGIS's remap text: "old new;…" or "from to new;…", NODATA allowed as
/// the new value.
pub fn parse_remap(text: &str) -> Option<Vec<Remap>> {
    text.split(';')
        .filter(|e| !e.trim().is_empty())
        .map(|entry| {
            let parts: Vec<&str> = entry.split_whitespace().collect();
            let new = |s: &str| -> Option<Option<f64>> {
                if s.eq_ignore_ascii_case("NODATA") {
                    Some(None)
                } else {
                    s.parse().ok().map(Some)
                }
            };
            match parts.as_slice() {
                [old, n] => {
                    let v: f64 = old.parse().ok()?;
                    Some(Remap {
                        from: v,
                        to: v,
                        new: new(n)?,
                    })
                }
                [from, to, n] => Some(Remap {
                    from: from.parse().ok()?,
                    to: to.parse().ok()?,
                    new: new(n)?,
                }),
                _ => None,
            }
        })
        .collect()
}

/// Spatial Analyst Reclassify on cell values. The first matching entry wins;
/// unmatched cells keep their value (`keep_missing`, DATA) or become NoData.
pub fn reclassify(input: &Raster, remap: &[Remap], keep_missing: bool) -> Raster {
    let values: Vec<Option<f64>> = input.bands[0]
        .iter()
        .map(|&v| {
            if input.is_nodata(v) {
                return None;
            }
            match remap.iter().find(|r| v >= r.from && v <= r.to) {
                Some(r) => r.new,
                None if keep_missing => Some(v),
                None => None,
            }
        })
        .collect();
    integer_raster(input, &values, is_eight_bit(input.data_type))
}

/// Zonal statistic types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZonalStatistic {
    Mean,
    Majority,
    Maximum,
    Median,
    Minimum,
    Minority,
    Range,
    Std,
    Sum,
    Variety,
}

impl ZonalStatistic {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "MEAN" => Self::Mean,
            "MAJORITY" => Self::Majority,
            "MAXIMUM" => Self::Maximum,
            "MEDIAN" => Self::Median,
            "MINIMUM" => Self::Minimum,
            "MINORITY" => Self::Minority,
            "RANGE" => Self::Range,
            "STD" => Self::Std,
            "SUM" => Self::Sum,
            "VARIETY" => Self::Variety,
            _ => return None,
        })
    }
}

fn zone_value(stat: ZonalStatistic, values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let n = values.len() as f64;
    let max = values.iter().copied().fold(f64::MIN, f64::max);
    let min = values.iter().copied().fold(f64::MAX, f64::min);
    Some(match stat {
        ZonalStatistic::Mean => values.iter().sum::<f64>() / n,
        ZonalStatistic::Sum => values.iter().sum(),
        ZonalStatistic::Maximum => max,
        ZonalStatistic::Minimum => min,
        ZonalStatistic::Range => max - min,
        ZonalStatistic::Std => {
            let mean = values.iter().sum::<f64>() / n;
            (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n).sqrt()
        }
        ZonalStatistic::Median => {
            values.sort_by(f64::total_cmp);
            let m = values.len() / 2;
            if values.len() % 2 == 1 {
                values[m]
            } else {
                (values[m - 1] + values[m]) / 2.0
            }
        }
        ZonalStatistic::Variety | ZonalStatistic::Majority | ZonalStatistic::Minority => {
            let mut sorted = values.to_vec();
            sorted.sort_by(f64::total_cmp);
            let mut runs: Vec<(f64, usize)> = Vec::new();
            for v in sorted {
                match runs.last_mut() {
                    Some((last, c)) if *last == v => *c += 1,
                    _ => runs.push((v, 1)),
                }
            }
            match stat {
                ZonalStatistic::Variety => runs.len() as f64,
                ZonalStatistic::Majority => {
                    let best = runs.iter().map(|r| r.1).max()?;
                    runs.iter().find(|r| r.1 == best)?.0
                }
                _ => {
                    let best = runs.iter().map(|r| r.1).min()?;
                    runs.iter().find(|r| r.1 == best)?.0
                }
            }
        }
    })
}

/// Spatial Analyst Zonal Statistics (raster output) with zones and values on the
/// same grid. With `ignore_nodata` false a zone holding any NoData value cell is
/// NoData throughout.
pub fn zonal_statistics(
    zones: &Raster,
    values: &Raster,
    stat: ZonalStatistic,
    ignore_nodata: bool,
) -> Raster {
    let mut per_zone: BTreeMap<i64, (Vec<f64>, bool)> = BTreeMap::new();
    for i in 0..zones.width * zones.height {
        let z = zones.bands[0][i];
        if zones.is_nodata(z) {
            continue;
        }
        let entry = per_zone.entry(z as i64).or_default();
        let v = values.bands[0][i];
        if values.is_nodata(v) {
            entry.1 = true;
        } else {
            entry.0.push(v);
        }
    }
    let results: BTreeMap<i64, Option<f64>> = per_zone
        .into_iter()
        .map(|(z, (mut vals, missing))| {
            let r = if missing && !ignore_nodata {
                None
            } else {
                zone_value(stat, &mut vals)
            };
            (z, r)
        })
        .collect();
    let cells: Vec<Option<f64>> = zones.bands[0]
        .iter()
        .map(|&z| {
            if zones.is_nodata(z) {
                None
            } else {
                results.get(&(z as i64)).copied().flatten()
            }
        })
        .collect();
    let float_out = values.data_type.is_float()
        || matches!(
            stat,
            ZonalStatistic::Mean | ZonalStatistic::Std | ZonalStatistic::Sum
        );
    if float_out {
        let mut out = values.like(DataType::F32);
        out.nodata = Some(f32::MIN as f64);
        out.bands[0] = cells
            .iter()
            .map(|v| v.map_or(f32::MIN as f64, |v| DataType::F32.store(v)))
            .collect();
        out
    } else {
        // Range is stored like an 8-bit result (observed).
        let eight = is_eight_bit(values.data_type) || stat == ZonalStatistic::Range;
        integer_raster(values, &cells, eight)
    }
}
