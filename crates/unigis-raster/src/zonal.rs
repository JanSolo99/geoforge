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

/// `ZonalFill(in_zone_raster, in_weight_raster)`: every cell of a zone gets the zone's spill level
/// — the lowest, over pairs of 8-neighbours with one cell inside the zone and one outside it, of
/// the higher weight of the two (a NoData weight drops out; NoData zone cells count as another
/// zone). Cells on the bottom and right raster edges also spill at their own weight; the top and
/// left edges do not (ArcGIS Pro 3.7, 30 random grids). Float weights give Float32, integer weights
/// the narrowest integer type for the values.
pub fn zonal_fill(zones: &Raster, weight: &Raster) -> Option<Raster> {
    if (zones.width, zones.height) != (weight.width, weight.height) {
        return None;
    }
    let (w, h) = (zones.width, zones.height);
    let zone = |c: usize, r: usize| zones.get(c, r).map(|v| v as i64);
    let mut level: std::collections::HashMap<Option<i64>, f64> = std::collections::HashMap::new();
    let mut lower = |key: Option<i64>, v: f64| {
        let e = level.entry(key).or_insert(f64::INFINITY);
        if v < *e {
            *e = v;
        }
    };
    for r in 0..h {
        for c in 0..w {
            let Some(z) = zone(c, r) else { continue };
            let here = weight.get(c, r);
            for (dc, dr) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let (nc, nr) = (c as i64 + dc, r as i64 + dr);
                if nc < 0 || nr < 0 || nc >= w as i64 || nr >= h as i64 {
                    // Beyond the bottom or right edge (not only beyond the top or left).
                    if (nr >= h as i64 || nc >= w as i64)
                        && let Some(v) = here
                    {
                        lower(Some(z), v);
                    }
                    continue;
                }
                let (nc, nr) = (nc as usize, nr as usize);
                if zone(nc, nr) == Some(z) {
                    continue;
                }
                let v = match (here, weight.get(nc, nr)) {
                    (Some(a), Some(b)) => a.max(b),
                    (Some(a), None) | (None, Some(a)) => a,
                    (None, None) => continue,
                };
                lower(Some(z), v);
            }
        }
    }
    let float = weight.data_type.is_float();
    let values: Vec<f64> = level.values().copied().filter(|v| v.is_finite()).collect();
    let (min, max) = values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    let (t, nodata) = if float {
        (DataType::F32, f64::from(f32::MIN))
    } else if values.is_empty() {
        (DataType::U8, 255.0)
    } else {
        integer_output(min, max, true)
    };
    let mut out = zones.like(t);
    out.nodata = Some(nodata);
    out.bands = vec![
        (0..h)
            .flat_map(|r| (0..w).map(move |c| (c, r)))
            .map(|(c, r)| {
                zone(c, r)
                    .and_then(|z| level.get(&Some(z)).copied())
                    .filter(|v| v.is_finite())
                    .map_or(nodata, |v| t.store(v))
            })
            .collect(),
    ];
    Some(out)
}

#[cfg(test)]
mod zonal_fill_tests {
    use super::*;

    fn grid(values: &[Vec<f64>], t: DataType) -> Raster {
        let (h, w) = (values.len(), values[0].len());
        let mut r = Raster::new(w, h, t, [0.0, 1.0, 0.0, h as f64, 0.0, -1.0]);
        r.nodata = Some(-9999.0);
        r.bands[0] = values.concat();
        r
    }

    #[test]
    fn spill_levels_as_arcgis_gives_them() {
        // ArcGIS Pro 3.7 on a 6 x 6 grid: zone 0 spills into zone 3 at 15, zone 1 at its own
        // boundary minimum 17, zone 2 at 37.
        let z: Vec<Vec<f64>> = [
            [0, 0, 0, 0, 0, 3],
            [0, 1, 1, 1, 0, 0],
            [0, 1, 1, 1, 0, 0],
            [0, 1, 1, 1, 0, 0],
            [0, 0, 0, 2, 2, 2],
            [0, 0, 0, 2, 2, 2],
        ]
        .iter()
        .map(|r| r.iter().map(|&v| f64::from(v)).collect())
        .collect();
        let mut w: Vec<Vec<f64>> = (0..6)
            .map(|r| (0..6).map(|c| f64::from(r * 6 + c) + 10.0).collect())
            .collect();
        w[2][2] = 1.0;
        w[0][0] = 0.0;
        let out = zonal_fill(&grid(&z, DataType::I32), &grid(&w, DataType::F32)).unwrap();
        let at = |r: usize, c: usize| out.bands[0][r * 6 + c];
        assert_eq!(
            (at(0, 0), at(2, 2), at(5, 5), at(0, 5)),
            (15.0, 17.0, 37.0, 15.0)
        );
        // Bottom and right edges spill, the top and left do not.
        let z = vec![vec![1.0, 1.0, 0.0], vec![1.0, 1.0, 0.0]];
        let w = vec![vec![5.0, 6.0, 1.0], vec![7.0, 8.0, 2.0]];
        let out = zonal_fill(&grid(&z, DataType::I32), &grid(&w, DataType::I32)).unwrap();
        assert_eq!(out.bands[0], vec![6.0, 6.0, 1.0, 6.0, 6.0, 1.0]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterizationStatistic {
    Mean,
    Majority,
    MajorityCount,
    MajorityPercent,
    Maximum,
    Median,
    Minimum,
    Minority,
    MinorityCount,
    MinorityPercent,
    Percentile,
    Range,
    Std,
    Sum,
    Variety,
}

impl CharacterizationStatistic {
    pub fn parse(s: &str) -> Option<Self> {
        use CharacterizationStatistic::*;
        Some(match s.to_ascii_uppercase().as_str() {
            "MEAN" => Mean,
            "MAJORITY" => Majority,
            "MAJORITY_COUNT" => MajorityCount,
            "MAJORITY_PERCENT" => MajorityPercent,
            "MAXIMUM" => Maximum,
            "MEDIAN" => Median,
            "MINIMUM" => Minimum,
            "MINORITY" => Minority,
            "MINORITY_COUNT" => MinorityCount,
            "MINORITY_PERCENT" => MinorityPercent,
            "PERCENTILE" => Percentile,
            "RANGE" => Range,
            "STD" => Std,
            "SUM" => Sum,
            "VARIETY" => Variety,
            _ => return None,
        })
    }

    fn suffix(self) -> &'static str {
        use CharacterizationStatistic::*;
        match self {
            Mean => "MEAN",
            Majority => "MAJORITY",
            MajorityCount => "MAJORITY_COUNT",
            MajorityPercent => "MAJORITY_PERCENT",
            Maximum => "MAXIMUM",
            Median => "MEDIAN",
            Minimum => "MINIMUM",
            Minority => "MINORITY",
            MinorityCount => "MINORITY_COUNT",
            MinorityPercent => "MINORITY_PERCENT",
            Percentile => "PERCENTILE",
            Range => "RANGE",
            Std => "STD",
            Sum => "SUM",
            Variety => "VARIETY",
        }
    }
}

pub struct CharacterizationRequest {
    pub name: String,
    pub values: Raster,
    pub statistic: CharacterizationStatistic,
    pub field_name: Option<String>,
}

/// Zonal Characterization on aligned rasters. Each request contributes a count and one statistic
/// column (one per requested percentile). Strict NoData mode omits a zone affected in any input.
pub fn zonal_characterization(
    zones: &Raster,
    requests: &[CharacterizationRequest],
    ignore_nodata: bool,
    percentiles: &[f64],
    percentile_linear: Option<bool>,
    circular_wrap: Option<f64>,
) -> unigis_core::Result<(Vec<unigis_core::Field>, Vec<Vec<unigis_core::Value>>)> {
    use CharacterizationStatistic::*;
    use unigis_core::{Error, Field, FieldType, Value};

    let mut order = Vec::new();
    let mut per_zone: Vec<BTreeMap<i64, (Vec<f64>, bool)>> = vec![BTreeMap::new(); requests.len()];
    for i in 0..zones.width * zones.height {
        let z = zones.bands[0][i];
        if zones.is_nodata(z) {
            continue;
        }
        let key = z as i64;
        if !per_zone[0].contains_key(&key) {
            order.push(key);
        }
        for (request, column) in requests.iter().zip(&mut per_zone) {
            let entry = column.entry(key).or_default();
            let v = request.values.bands[0][i];
            if request.values.is_nodata(v) {
                entry.1 = true;
            } else {
                entry.0.push(v);
            }
        }
    }

    let mut fields = vec![Field::new("VALUE", FieldType::Integer)];
    for request in requests {
        let integer = !request.values.data_type.is_float();
        if !integer
            && matches!(
                request.statistic,
                Majority
                    | MajorityCount
                    | MajorityPercent
                    | Minority
                    | MinorityCount
                    | MinorityPercent
                    | Variety
            )
        {
            return Err(Error::Unsupported(format!(
                "{} requires an integer value raster",
                request.statistic.suffix()
            )));
        }
        if circular_wrap.is_some()
            && !matches!(
                request.statistic,
                Mean | Std | Majority | Minority | Variety
            )
        {
            return Err(Error::Unsupported(format!(
                "circular {} is not supported",
                request.statistic.suffix()
            )));
        }
        fields.push(Field::new(
            format!("{}_COUNT", request.name),
            FieldType::Double,
        ));
        let base = request.field_name.clone().unwrap_or_else(|| {
            let name = if request
                .name
                .get(..10)
                .is_some_and(|prefix| prefix.ends_with('_'))
            {
                format!("{}1", &request.name[..9])
            } else {
                request.name.clone()
            };
            format!(
                "{}_{}{}",
                name,
                if circular_wrap.is_some() { "C_" } else { "" },
                request.statistic.suffix()
            )
        });
        let statistic_type = match request.statistic {
            MajorityCount | MinorityCount | Variety => FieldType::Integer,
            MajorityPercent | MinorityPercent => FieldType::Single,
            Majority | Minority | Maximum | Median | Minimum | Range if integer => {
                FieldType::Integer
            }
            Percentile if integer && percentile_linear != Some(true) => FieldType::Integer,
            _ => FieldType::Double,
        };
        let count = if request.statistic == Percentile {
            percentiles.len()
        } else {
            1
        };
        for i in 0..count {
            fields.push(Field::new(
                if i == 0 {
                    base.clone()
                } else {
                    format!("{base}_{i}")
                },
                statistic_type,
            ));
        }
    }

    let mut rows = Vec::new();
    for key in order {
        if !ignore_nodata
            && per_zone
                .iter()
                .any(|column| column.get(&key).is_some_and(|entry| entry.1))
        {
            continue;
        }
        let mut row = vec![Value::Integer(key as i32)];
        for (request, column) in requests.iter().zip(&mut per_zone) {
            let entry = column.remove(&key).unwrap_or_default();
            let integer = !request.values.data_type.is_float();
            let n = entry.0.len();
            row.push(Value::Double(n as f64));
            let outputs = if request.statistic == Percentile {
                percentiles.len()
            } else {
                1
            };
            if n == 0 {
                row.extend((0..outputs).map(|_| Value::Null));
                continue;
            }
            let mut values = entry.0;
            if let Some(wrap) = circular_wrap {
                for value in &mut values {
                    *value = value.rem_euclid(wrap);
                }
            }
            values.sort_by(f64::total_cmp);
            let sum: f64 = values.iter().sum();
            let mean = sum / n as f64;
            let mut runs: Vec<(f64, usize)> = Vec::new();
            for &value in &values {
                match runs.last_mut() {
                    Some((last, count)) if *last == value => *count += 1,
                    _ => runs.push((value, 1)),
                }
            }
            let majority = runs.iter().fold(
                runs[0],
                |best, run| if run.1 > best.1 { *run } else { best },
            );
            let minority = runs.iter().fold(
                runs[0],
                |best, run| if run.1 < best.1 { *run } else { best },
            );
            let circular_moments = || {
                let wrap = circular_wrap.unwrap_or(360.0);
                let scale = std::f64::consts::TAU / wrap;
                let sin = values.iter().map(|v| (v * scale).sin()).sum::<f64>() / n as f64;
                let cos = values.iter().map(|v| (v * scale).cos()).sum::<f64>() / n as f64;
                (sin, cos, wrap)
            };
            let numeric = |value: f64, integral: bool| {
                if integral {
                    Value::Integer(value as i32)
                } else {
                    Value::Double(value)
                }
            };
            match request.statistic {
                Mean => {
                    let value = if circular_wrap.is_some() {
                        let (sin, cos, wrap) = circular_moments();
                        sin.atan2(cos).rem_euclid(std::f64::consts::TAU) * wrap
                            / std::f64::consts::TAU
                    } else {
                        mean
                    };
                    row.push(Value::Double(value));
                }
                Std => {
                    let value = if circular_wrap.is_some() {
                        let (sin, cos, wrap) = circular_moments();
                        (-2.0 * sin.hypot(cos).ln()).sqrt() * wrap / std::f64::consts::TAU
                    } else {
                        (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n as f64)
                            .sqrt()
                    };
                    row.push(Value::Double(value));
                }
                Sum => row.push(Value::Double(sum)),
                Maximum => row.push(numeric(values[n - 1], integer)),
                Minimum => row.push(numeric(values[0], integer)),
                Range => row.push(numeric(values[n - 1] - values[0], integer)),
                Median => {
                    let median = (values[(n - 1) / 2] + values[n / 2]) / 2.0;
                    row.push(numeric(
                        if integer { median.floor() } else { median },
                        integer,
                    ));
                }
                Majority => row.push(numeric(majority.0, integer)),
                Minority => row.push(numeric(minority.0, integer)),
                MajorityCount => row.push(Value::Integer(majority.1 as i32)),
                MinorityCount => row.push(Value::Integer(minority.1 as i32)),
                MajorityPercent | MinorityPercent => {
                    let count = if request.statistic == MajorityPercent {
                        majority.1
                    } else {
                        minority.1
                    };
                    let percent = (count as f64 / n as f64 * 100.0 * 1e4).round() / 1e4;
                    row.push(Value::Single(percent as f32));
                }
                Variety => row.push(Value::Integer(runs.len() as i32)),
                Percentile => {
                    let linear = percentile_linear.unwrap_or(!integer);
                    for percentile in percentiles {
                        let position = percentile / 100.0 * (n - 1) as f64;
                        let value = if linear {
                            let low = position.floor() as usize;
                            let high = (low + 1).min(n - 1);
                            values[low] + (values[high] - values[low]) * (position - low as f64)
                        } else {
                            values[position.round() as usize]
                        };
                        row.push(numeric(value, integer && !linear));
                    }
                }
            }
        }
        rows.push(row);
    }
    Ok((fields, rows))
}

/// What Zonal Geometry measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneGeometry {
    Area,
    Perimeter,
    Thickness,
    Centroid,
}

/// The chamfer distance (axis 1, diagonal 1.414) from a cell centre to the nearest point of a cell
/// `dx`, `dy` cells away.
fn chamfer(dx: i64, dy: i64) -> f64 {
    let a = (dx.abs() as f64 - 0.5).max(0.0);
    let b = (dy.abs() as f64 - 0.5).max(0.0);
    1.414 * a.min(b) + (a - b).abs()
}

/// `ZonalGeometry(in_zone_data, "Value", geometry_type)` (ArcGIS Pro 3.7, observed):
/// - AREA: the zone's cell count times the cell area; PERIMETER: its cell edges facing another zone
///   or the raster border, times the cell size (Float32).
/// - THICKNESS: the largest distance from a zone cell's centre to the nearest cell of another zone
///   (NoData counts as another zone, the raster border does not), measured with the chamfer metric
///   above, times the cell size; 10000 for a zone with no other zone in the raster.
/// - CENTROID: the zone value in the cell holding the mean of its cell centres, NoData elsewhere
///   (32-bit signed).
pub fn zonal_geometry(zones: &Raster, what: ZoneGeometry) -> Raster {
    use std::collections::HashMap;
    let (w, h) = (zones.width, zones.height);
    let cell = zones.transform[1].abs();
    let zone = |c: usize, r: usize| zones.get(c, r).map(|v| v as i64);
    let mut out = zones.like(if what == ZoneGeometry::Centroid {
        DataType::I32
    } else {
        DataType::F32
    });
    let nodata = if what == ZoneGeometry::Centroid {
        -2_147_483_647.0
    } else {
        f64::from(f32::MIN)
    };
    out.nodata = Some(nodata);
    let mut value: HashMap<i64, f64> = HashMap::new();
    match what {
        ZoneGeometry::Area | ZoneGeometry::Perimeter => {
            for r in 0..h {
                for c in 0..w {
                    let Some(z) = zone(c, r) else { continue };
                    let add = if what == ZoneGeometry::Area {
                        cell * cell
                    } else {
                        let edges = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                            .iter()
                            .filter(|(dc, dr)| {
                                let (nc, nr) = (c as i64 + dc, r as i64 + dr);
                                nc < 0
                                    || nr < 0
                                    || nc >= w as i64
                                    || nr >= h as i64
                                    || zone(nc as usize, nr as usize) != Some(z)
                            })
                            .count();
                        edges as f64 * cell
                    };
                    *value.entry(z).or_insert(0.0) += add;
                }
            }
        }
        ZoneGeometry::Thickness => {
            for r in 0..h {
                for c in 0..w {
                    let Some(z) = zone(c, r) else { continue };
                    // Search square rings outwards until no nearer cell can be found.
                    let mut best = f64::INFINITY;
                    let limit = w.max(h) as i64;
                    for k in 1..=limit {
                        if k as f64 - 0.5 > best {
                            break;
                        }
                        for dr in -k..=k {
                            for dc in -k..=k {
                                if dr.abs() != k && dc.abs() != k {
                                    continue;
                                }
                                let (nc, nr) = (c as i64 + dc, r as i64 + dr);
                                if nc < 0 || nr < 0 || nc >= w as i64 || nr >= h as i64 {
                                    continue;
                                }
                                if zone(nc as usize, nr as usize) != Some(z) {
                                    best = best.min(chamfer(dc, dr));
                                }
                            }
                        }
                    }
                    let e = value.entry(z).or_insert(f64::NEG_INFINITY);
                    let d = if best.is_finite() {
                        best * cell
                    } else {
                        10_000.0
                    };
                    *e = e.max(d);
                }
            }
        }
        ZoneGeometry::Centroid => {
            let mut sums: HashMap<i64, (f64, f64, f64)> = HashMap::new();
            for r in 0..h {
                for c in 0..w {
                    if let Some(z) = zone(c, r) {
                        let e = sums.entry(z).or_insert((0.0, 0.0, 0.0));
                        e.0 += c as f64 + 0.5;
                        e.1 += r as f64 + 0.5;
                        e.2 += 1.0;
                    }
                }
            }
            let mut band = vec![nodata; w * h];
            let mut keys: Vec<_> = sums.keys().copied().collect();
            keys.sort_unstable();
            for z in keys {
                let (sx, sy, n) = sums[&z];
                let (c, r) = ((sx / n).floor() as usize, (sy / n).floor() as usize);
                if c < w && r < h {
                    band[r * w + c] = z as f64;
                }
            }
            out.bands = vec![band];
            return out;
        }
    }
    out.bands = vec![
        (0..h)
            .flat_map(|r| (0..w).map(move |c| (c, r)))
            .map(|(c, r)| {
                zone(c, r)
                    .and_then(|z| value.get(&z))
                    .map_or(nodata, |&v| DataType::F32.store(v))
            })
            .collect(),
    ];
    out
}

#[cfg(test)]
mod zonal_geometry_tests {
    use super::*;

    fn grid(rows: &[&[i32]], cell: f64) -> Raster {
        let (h, w) = (rows.len(), rows[0].len());
        let mut r = Raster::new(
            w,
            h,
            DataType::I32,
            [0.0, cell, 0.0, h as f64 * cell, 0.0, -cell],
        );
        r.nodata = Some(-9999.0);
        r.bands[0] = rows
            .iter()
            .flat_map(|r| r.iter().map(|&v| f64::from(v)))
            .collect();
        r
    }

    #[test]
    fn thickness_as_arcgis_measures_it() {
        let t = |rows: &[&[i32]], cell: f64| {
            zonal_geometry(&grid(rows, cell), ZoneGeometry::Thickness).bands[0].clone()
        };
        let f = |v: f64| f64::from(v as f32);
        assert_eq!(
            t(&[&[0, 1, 0], &[1, 1, 1], &[0, 1, 0]], 1.0)[..2],
            [0.5, f(0.707)]
        );
        assert_eq!(
            t(
                &[
                    &[2, 1, 1, 1, 1, 1],
                    &[1, 1, 1, 1, 1, 1],
                    &[1, 1, 1, 1, 1, 1]
                ],
                2.0
            )[1],
            f(10.242)
        );
        assert_eq!(t(&[&[1, 1], &[1, 1]], 1.0)[0], 10_000.0);
    }

    #[test]
    fn area_perimeter_centroid() {
        let z = grid(
            &[
                &[0, 0, 0, 0, 0, 0, 0, 2],
                &[0, 1, 1, 1, 1, 1, 0, 0],
                &[0, 1, 1, 1, 1, 1, 0, 0],
                &[0, 1, 3, 1, 1, 1, 0, 0],
                &[0, 1, 1, 1, 1, 1, 0, 0],
                &[0, 0, 0, 0, 0, 0, 2, 2],
            ],
            2.0,
        );
        let a = zonal_geometry(&z, ZoneGeometry::Area).bands[0].clone();
        assert_eq!((a[0], a[9], a[26], a[7]), (100.0, 76.0, 4.0, 12.0));
        let p = zonal_geometry(&z, ZoneGeometry::Perimeter).bands[0].clone();
        assert_eq!((p[0], p[9], p[26], p[7]), (92.0, 44.0, 8.0, 20.0));
        let c = zonal_geometry(&z, ZoneGeometry::Centroid).bands[0].clone();
        assert_eq!(
            (c[2 * 8 + 3], c[2 * 8 + 4], c[3 * 8 + 2], c[3 * 8 + 7]),
            (1.0, 0.0, 3.0, 2.0)
        );
    }
}
