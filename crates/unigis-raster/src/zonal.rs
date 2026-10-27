//! Spatial Analyst Reclassify and Zonal Statistics.

use std::collections::BTreeMap;

use crate::{DataType, Raster};

/// The integer cell type and NoData ArcGIS gives an integer result whose values
/// run from `min` to `max` (observed): results from 8-bit input may be stored with
/// 1, 2 or 4 bits (NoData 1, 3 or 15, or 0 when the values run from 1 to the top) or
/// as signed then unsigned 8-bit; wider input gives unsigned 16-bit when there are no negative values, else signed
/// 16-bit, then 32-bit.
pub fn integer_output(min: f64, max: f64, eight_bit_input: bool) -> (DataType, f64) {
    if eight_bit_input {
        if min >= 0.0 {
            for bits in [1u32, 2, 4] {
                let top = ((1u32 << bits) - 1) as f64;
                if max < top {
                    return (DataType::U8, top);
                }
                // Values running exactly from 1 to the top keep 0 for NoData
                // (1..3 stays 2-bit, but 2..3 moves on to 4 bits).
                if min == 1.0 && max == top {
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
    reclassify_with(
        input,
        remap,
        keep_missing,
        is_eight_bit(input.data_type),
        false,
    )
}

/// Reclass by ASCII File: as [`reclassify`], but with NODATA for missing values the output type
/// is chosen as for 8-bit input whatever the input (observed on a float raster).
pub fn reclassify_ascii(input: &Raster, remap: &[Remap], keep_missing: bool) -> Raster {
    reclassify_with(
        input,
        remap,
        keep_missing,
        !keep_missing || is_eight_bit(input.data_type),
        false,
    )
}

/// Reclass by Table: as [`reclassify`], but the output type is always chosen as for 8-bit input
/// and the fractional values it keeps are truncated, not rounded.
pub fn reclassify_table(input: &Raster, remap: &[Remap], keep_missing: bool) -> Raster {
    reclassify_with(input, remap, keep_missing, true, true)
}

fn reclassify_with(
    input: &Raster,
    remap: &[Remap],
    keep_missing: bool,
    eight_bit_rules: bool,
    truncate_kept: bool,
) -> Raster {
    let values: Vec<Option<f64>> = input.bands[0]
        .iter()
        .map(|&v| {
            if input.is_nodata(v) {
                return None;
            }
            match remap.iter().find(|r| v >= r.from && v <= r.to) {
                Some(r) => r.new,
                None if keep_missing => Some(if truncate_kept { v.trunc() } else { v }),
                None => None,
            }
        })
        .collect();
    integer_raster(input, &values, eight_bit_rules)
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
/// same grid (see `resample::zones_on_value_grid`). With `ignore_nodata` false a
/// zone holding any NoData value cell is NoData throughout.
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
            // An integer median of an even count rounds down.
            let r = match stat {
                ZonalStatistic::Median if !values.data_type.is_float() => r.map(f64::floor),
                _ => r,
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
        let eight = is_eight_bit(values.data_type);
        let max = cells.iter().flatten().copied().fold(0.0, f64::max);
        if stat == ZonalStatistic::Range && !eight && max < 255.0 {
            // A range from wider input that fits is unsigned 8-bit (observed).
            let mut out = values.like(DataType::U8);
            out.nodata = Some(255.0);
            out.bands[0] = cells.iter().map(|v| v.unwrap_or(255.0)).collect();
            out
        } else {
            integer_raster(values, &cells, eight)
        }
    }
}

/// Statistics of Zonal Statistics as Table, in ArcGIS's column order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TableStatistic {
    Min,
    Max,
    Range,
    Mean,
    Std,
    Sum,
    Variety,
    Majority,
    Minority,
    Median,
    Percentile,
}

/// The statistics an ArcGIS `statistics_type` keyword asks for (integer-only
/// ones are dropped later for float values).
pub fn table_statistics(keyword: &str) -> Option<Vec<TableStatistic>> {
    use TableStatistic::*;
    Some(match keyword.to_ascii_uppercase().as_str() {
        "ALL" => vec![
            Min, Max, Range, Mean, Std, Sum, Variety, Majority, Minority, Median, Percentile,
        ],
        "MEAN" => vec![Mean],
        "MAJORITY" => vec![Majority],
        "MAXIMUM" => vec![Max],
        "MEDIAN" => vec![Median],
        "MINIMUM" => vec![Min],
        "MINORITY" => vec![Minority],
        "PERCENTILE" => vec![Percentile],
        "RANGE" => vec![Range],
        "STD" => vec![Std],
        "SUM" => vec![Sum],
        "VARIETY" => vec![Variety],
        "MIN_MAX" => vec![Min, Max],
        "MEAN_STD" => vec![Mean, Std],
        "MIN_MAX_MEAN" => vec![Min, Max, Mean],
        _ => return None,
    })
}

/// Spatial Analyst Zonal Statistics as Table (zones and values on one grid, see
/// `resample::zones_on_value_grid`): one row per zone in order of first appearance, with VALUE, COUNT and AREA
/// followed by the requested statistics. Median averages the middle pair and
/// PCT90 interpolates linearly; integer values keep integer MIN, MAX, RANGE,
/// MEDIAN and PCT90 and add VARIETY and MAJORITY/MINORITY with their counts and
/// percentages. With `ignore_nodata` false, zones holding NoData are left out.
pub fn zonal_table(
    zones: &Raster,
    values: &Raster,
    stats: &[TableStatistic],
    ignore_nodata: bool,
) -> (Vec<unigis_core::Field>, Vec<Vec<unigis_core::Value>>) {
    use TableStatistic::*;
    use unigis_core::{Field, FieldType, Value};
    let integer = !values.data_type.is_float();
    let mut stats: Vec<TableStatistic> = stats
        .iter()
        .copied()
        .filter(|s| integer || !matches!(s, Variety | Majority | Minority))
        .collect();
    stats.sort();
    stats.dedup();
    let num = |name: &str, int: bool| {
        Field::new(
            name,
            if int {
                FieldType::Integer
            } else {
                FieldType::Double
            },
        )
    };
    let mut fields = vec![
        Field::new("VALUE", FieldType::Integer),
        num("COUNT", false),
        num("AREA", false),
    ];
    for s in &stats {
        match s {
            Min => fields.push(num("MIN", integer)),
            Max => fields.push(num("MAX", integer)),
            Range => fields.push(num("RANGE", integer)),
            Mean => fields.push(num("MEAN", false)),
            Std => fields.push(num("STD", false)),
            Sum => fields.push(num("SUM", false)),
            Variety => fields.push(num("VARIETY", true)),
            Majority | Minority => {
                let base = if *s == Majority {
                    "MAJORITY"
                } else {
                    "MINORITY"
                };
                fields.push(num(base, true));
                fields.push(num(&format!("{base}_COUNT"), true));
                fields.push(Field::new(format!("{base}_PERCENT"), FieldType::Single));
            }
            Median => fields.push(num("MEDIAN", integer)),
            Percentile => fields.push(num("PCT90", integer)),
        }
    }
    let mut order: Vec<i64> = Vec::new();
    let mut per_zone: BTreeMap<i64, (Vec<f64>, bool)> = BTreeMap::new();
    for i in 0..zones.width * zones.height {
        let z = zones.bands[0][i];
        if zones.is_nodata(z) {
            continue;
        }
        let key = z as i64;
        let entry = per_zone.entry(key).or_insert_with(|| {
            order.push(key);
            (Vec::new(), false)
        });
        let v = values.bands[0][i];
        if values.is_nodata(v) {
            entry.1 = true;
        } else {
            entry.0.push(v);
        }
    }
    let cell_area = values.cell_width() * values.cell_height();
    let mut rows = Vec::new();
    for key in order {
        let (mut v, missing) = per_zone.remove(&key).unwrap_or_default();
        if v.is_empty() || (missing && !ignore_nodata) {
            continue;
        }
        v.sort_by(f64::total_cmp);
        let n = v.len();
        let sum: f64 = v.iter().sum();
        let mean = sum / n as f64;
        let std = (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64).sqrt();
        let val = |x: f64, int: bool| {
            if int {
                Value::Integer(x.round() as i32)
            } else {
                Value::Double(x)
            }
        };
        let mut runs: Vec<(f64, usize)> = Vec::new();
        for &x in &v {
            match runs.last_mut() {
                Some((last, c)) if *last == x => *c += 1,
                _ => runs.push((x, 1)),
            }
        }
        let mut row = vec![
            Value::Integer(key as i32),
            Value::Double(n as f64),
            Value::Double(n as f64 * cell_area),
        ];
        for s in &stats {
            match s {
                Min => row.push(val(v[0], integer)),
                Max => row.push(val(v[n - 1], integer)),
                Range => row.push(val(v[n - 1] - v[0], integer)),
                Mean => row.push(Value::Double(mean)),
                Std => row.push(Value::Double(std)),
                Sum => row.push(Value::Double(sum)),
                Variety => row.push(Value::Integer(runs.len() as i32)),
                Majority | Minority => {
                    // Ties go to the smallest value.
                    let pick = if *s == Majority {
                        runs.iter()
                            .fold(runs[0], |b, r| if r.1 > b.1 { *r } else { b })
                    } else {
                        runs.iter()
                            .fold(runs[0], |b, r| if r.1 < b.1 { *r } else { b })
                    };
                    row.push(Value::Integer(pick.0 as i32));
                    row.push(Value::Integer(pick.1 as i32));
                    // Percentages are rounded to 4 decimals.
                    let pct = (pick.1 as f64 / n as f64 * 100.0 * 1e4).round() / 1e4;
                    row.push(Value::Single(pct as f32));
                }
                Median => {
                    // An integer median of an even count rounds down.
                    let m = (v[(n - 1) / 2] + v[n / 2]) / 2.0;
                    row.push(val(if integer { m.floor() } else { m }, integer));
                }
                Percentile => {
                    let pos = 0.9 * (n - 1) as f64;
                    let lo = pos.floor() as usize;
                    let hi = (lo + 1).min(n - 1);
                    let p = v[lo] + (v[hi] - v[lo]) * (pos - lo as f64);
                    row.push(val(p, integer));
                }
            }
        }
        rows.push(row);
    }
    (fields, rows)
}
