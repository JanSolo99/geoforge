//! Spatial Analyst Focal Statistics.

use crate::{DataType, Raster};

/// A focal neighborhood (sizes in cells).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Neighborhood {
    Rectangle { width: usize, height: usize },
    Circle { radius: f64 },
    Annulus { inner: f64, outer: f64 },
}

impl Neighborhood {
    /// Parses ArcGIS's text form: "Rectangle 3 3 CELL", "Circle 2 CELL",
    /// "Annulus 1 3 CELL" (sizes in cells only).
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let units = parts.last()?.to_ascii_uppercase();
        if units != "CELL" {
            return None;
        }
        let num = |i: usize| parts.get(i)?.parse::<f64>().ok();
        Some(match parts.first()?.to_ascii_uppercase().as_str() {
            "RECTANGLE" => Neighborhood::Rectangle {
                width: num(1)? as usize,
                height: num(2)? as usize,
            },
            "CIRCLE" => Neighborhood::Circle { radius: num(1)? },
            "ANNULUS" => Neighborhood::Annulus {
                inner: num(1)?,
                outer: num(2)?,
            },
            _ => return None,
        })
    }

    /// How many cells the neighborhood spans (an upper bound), for refusing absurd sizes.
    pub fn cell_count(&self) -> u64 {
        match *self {
            Neighborhood::Rectangle { width, height } => {
                (width as u64).saturating_mul(height as u64)
            }
            Neighborhood::Circle { radius } => {
                let d = (radius.abs().floor() * 2.0 + 1.0).min(u32::MAX as f64) as u64;
                d.saturating_mul(d)
            }
            Neighborhood::Annulus { outer, .. } => {
                let d = (outer.abs().floor() * 2.0 + 1.0).min(u32::MAX as f64) as u64;
                d.saturating_mul(d)
            }
        }
    }

    /// (dx, dy) offsets of the cells in the neighborhood, row by row.
    pub fn offsets(&self) -> Vec<(isize, isize)> {
        let mut out = Vec::new();
        match *self {
            Neighborhood::Rectangle { width, height } => {
                let (w, h) = (width as isize, height as isize);
                for dy in -(h - 1) / 2..=h / 2 {
                    for dx in -(w - 1) / 2..=w / 2 {
                        out.push((dx, dy));
                    }
                }
            }
            // ArcGIS truncates radii given in cells to whole cells.
            Neighborhood::Circle { radius } => {
                let r = radius.floor() as isize;
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy <= r * r {
                            out.push((dx, dy));
                        }
                    }
                }
            }
            Neighborhood::Annulus { inner, outer } => {
                let (r, i) = (outer.floor() as isize, inner.floor() as isize);
                for dy in -r..=r {
                    for dx in -r..=r {
                        let d2 = dx * dx + dy * dy;
                        if d2 <= r * r && d2 >= i * i {
                            out.push((dx, dy));
                        }
                    }
                }
            }
        }
        out
    }
}

/// Focal statistic types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocalStatistic {
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

impl FocalStatistic {
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

    /// The output cell type ArcGIS uses for an input of type `t`.
    fn output_type(self, t: DataType) -> DataType {
        match self {
            Self::Mean | Self::Std | Self::Median => DataType::F32,
            Self::Sum if t.is_float() => DataType::F32,
            Self::Sum => DataType::I32,
            _ if t == DataType::F64 => DataType::F32,
            _ => t,
        }
    }
}

/// The largest value of a cell type, which Focal Statistics and the hydrology tools
/// use as output NoData.
pub(crate) fn type_max(t: DataType) -> f64 {
    match t {
        DataType::U8 => u8::MAX as f64,
        DataType::I8 => i8::MAX as f64,
        DataType::U16 => u16::MAX as f64,
        DataType::I16 => i16::MAX as f64,
        DataType::U32 => u32::MAX as f64,
        DataType::I32 => i32::MAX as f64,
        DataType::F32 | DataType::F64 => f32::MAX as f64,
    }
}

/// Counts of distinct values, in first-seen order.
fn counts(values: &[f64]) -> Vec<(f64, usize)> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, usize)> = Vec::new();
    for v in sorted {
        match out.last_mut() {
            Some((last, n)) if *last == v => *n += 1,
            _ => out.push((v, 1)),
        }
    }
    out
}

pub(crate) fn compute(
    stat: FocalStatistic,
    values: &mut [f64],
    center: Option<f64>,
) -> Option<f64> {
    let n = values.len() as f64;
    Some(match stat {
        FocalStatistic::Mean => values.iter().sum::<f64>() / n,
        FocalStatistic::Sum => values.iter().sum(),
        FocalStatistic::Maximum => values.iter().copied().fold(f64::MIN, f64::max),
        FocalStatistic::Minimum => values.iter().copied().fold(f64::MAX, f64::min),
        FocalStatistic::Range => {
            values.iter().copied().fold(f64::MIN, f64::max)
                - values.iter().copied().fold(f64::MAX, f64::min)
        }
        FocalStatistic::Std => {
            let mean = values.iter().sum::<f64>() / n;
            (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n).sqrt()
        }
        FocalStatistic::Median => {
            values.sort_by(f64::total_cmp);
            let m = values.len() / 2;
            if values.len() % 2 == 1 {
                values[m]
            } else {
                (values[m - 1] + values[m]) / 2.0
            }
        }
        FocalStatistic::Variety => counts(values).len() as f64,
        FocalStatistic::Majority | FocalStatistic::Minority => {
            let c = counts(values);
            let best = if stat == FocalStatistic::Majority {
                c.iter().map(|x| x.1).max()?
            } else {
                c.iter().map(|x| x.1).min()?
            };
            // Ties go to the center cell's value when it is one of them, else the
            // smallest (ArcGIS warns 010666).
            let tied = c.iter().filter(|x| x.1 == best);
            match center {
                Some(v) if tied.clone().any(|x| x.0 == v) => v,
                _ => tied.clone().next()?.0,
            }
        }
    })
}

/// Spatial Analyst Focal Statistics. With `ignore_nodata` (DATA) only the cells
/// with values count; otherwise any NoData cell in the neighborhood, or any part of
/// it outside the raster, gives NoData.
pub fn focal_statistics(
    input: &Raster,
    neighborhood: Neighborhood,
    stat: FocalStatistic,
    ignore_nodata: bool,
) -> Raster {
    let offsets = neighborhood.offsets();
    let data_type = stat.output_type(input.data_type);
    let nodata = if stat == FocalStatistic::Variety {
        // A 1-, 2-, 4- or 8-bit raster just wide enough for the cell count,
        // whose largest value is NoData.
        let bits = [1u32, 2, 4, 8]
            .into_iter()
            .find(|b| offsets.len() < (1usize << b) - 1)
            .unwrap_or(8);
        ((1u32 << bits) - 1) as f64
    } else {
        type_max(data_type)
    };
    let mut out = input.like(data_type);
    out.nodata = Some(nodata);
    let mut values = Vec::with_capacity(offsets.len());
    for row in 0..input.height {
        for col in 0..input.width {
            values.clear();
            let mut missing = false;
            for &(dx, dy) in &offsets {
                let (x, y) = (col as isize + dx, row as isize + dy);
                if x < 0 || y < 0 || x >= input.width as isize || y >= input.height as isize {
                    missing = true; // outside counts as NoData
                    continue;
                }
                match input.get(x as usize, y as usize) {
                    Some(v) => values.push(v),
                    None => missing = true,
                }
            }
            let v = if values.is_empty() || (missing && !ignore_nodata) {
                None
            } else {
                compute(stat, &mut values, input.get(col, row))
            };
            out.bands[0][row * input.width + col] = v.map_or(nodata, |v| data_type.store(v));
        }
    }
    out
}

/// Spatial Analyst Filter: `low` is the 3x3 mean, otherwise the high-pass kernel
/// `-0.7 -1 -0.7 / -1 6.8 -1 / -0.7 -1 -0.7`. The output is 32-bit float. With `ignore_nodata`
/// (DATA) the cells with values count (a mean divides by how many there are, the high-pass sum
/// is not rescaled) and a cell without value is filled from its neighbours when it has any;
/// otherwise a cell with a missing neighbour (or beyond the raster) or without value is NoData.
pub fn filter(input: &Raster, low: bool, ignore_nodata: bool) -> Raster {
    const HIGH: [[f64; 3]; 3] = [[-0.7, -1.0, -0.7], [-1.0, 6.8, -1.0], [-0.7, -1.0, -0.7]];
    let nodata = f32::MAX as f64;
    let mut out = input.like(DataType::F32);
    out.nodata = Some(nodata);
    let (w, h) = (input.width as isize, input.height as isize);
    for row in 0..h {
        for col in 0..w {
            let (mut sum, mut count, mut missing) = (0.0, 0usize, false);
            for i in 0..3isize {
                for j in 0..3isize {
                    let (r, c) = (row + i - 1, col + j - 1);
                    let v = if r < 0 || c < 0 || r >= h || c >= w {
                        None
                    } else {
                        input.get(c as usize, r as usize)
                    };
                    match v {
                        Some(v) => {
                            sum += if low {
                                v
                            } else {
                                HIGH[i as usize][j as usize] * v
                            };
                            count += 1;
                        }
                        None => missing = true,
                    }
                }
            }
            let centre_missing = input.get(col as usize, row as usize).is_none();
            let v = if count == 0 || (!ignore_nodata && (missing || centre_missing)) {
                None
            } else if low {
                Some(sum / count as f64)
            } else {
                Some(sum)
            };
            out.bands[0][(row * w + col) as usize] = v.map_or(nodata, |v| DataType::F32.store(v));
        }
    }
    out
}
