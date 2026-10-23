//! Spatial Analyst Aggregate, Block Statistics, Slice, Pick and Region Group.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/blockops.json`, 10 x 13 rasters with NoData):
//! - Aggregate cuts the raster into blocks of `factor` x `factor` cells counted from its lower-left
//!   corner. EXPAND keeps a partial block at the top and right (the extent grows), TRUNCATE drops
//!   it. With NODATA, a block with a NoData cell, or one cut short by the edge, is NoData; with DATA
//!   the cells that have values count. Float input gives 32-bit float; integer input gives float for
//!   MEAN and MEDIAN and signed 32-bit integers for the rest.
//! - Block Statistics cuts the raster into blocks of the neighborhood's size counted from its upper-left
//!   corner and gives every cell of a block (a circle's block holds the cells inside the circle) the
//!   block's statistic, NoData cells included. Output types are Focal Statistics' with ArcGIS's
//!   own choices for integers (RANGE unsigned 32-bit, SUM NoData +2147483647).
//! - Slice (EQUAL_INTERVAL): zones of equal value width from the minimum to the maximum (an integer
//!   raster's range counts one wider), numbered from `base`. Equal area, natural breaks and the others are not done.
//! - Pick takes each cell from the input its position raster names (1-based): float with a float
//!   input, else an integer fitting the inputs' ranges.
//! - Region Group: see [`crate::generalize`].

use crate::focal::{FocalStatistic, Neighborhood, compute};
use crate::local::Operand;
use crate::zonal::integer_output;
use crate::{DataType, Raster};

/// What Aggregate computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateStat {
    Sum,
    Mean,
    Median,
    Maximum,
    Minimum,
}

impl AggregateStat {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "SUM" => Self::Sum,
            "MEAN" => Self::Mean,
            "MEDIAN" => Self::Median,
            "MAXIMUM" => Self::Maximum,
            "MINIMUM" => Self::Minimum,
            _ => return None,
        })
    }

    fn focal(self) -> FocalStatistic {
        match self {
            Self::Sum => FocalStatistic::Sum,
            Self::Mean => FocalStatistic::Mean,
            Self::Median => FocalStatistic::Median,
            Self::Maximum => FocalStatistic::Maximum,
            Self::Minimum => FocalStatistic::Minimum,
        }
    }
}

/// Aggregate by `factor` (at least 2). `expand`: keep a partial block at the top and right;
/// `ignore_nodata`: DATA.
pub fn aggregate(
    input: &Raster,
    factor: usize,
    stat: AggregateStat,
    expand: bool,
    ignore_nodata: bool,
) -> Option<Raster> {
    if factor < 2 {
        return None;
    }
    let (w, h) = (input.width, input.height);
    let (cols, rows) = if expand {
        (w.div_ceil(factor), h.div_ceil(factor))
    } else {
        (w / factor, h / factor)
    };
    if cols == 0 || rows == 0 {
        return None;
    }
    let float =
        input.data_type.is_float() || matches!(stat, AggregateStat::Mean | AggregateStat::Median);
    let (t, nodata) = if float {
        (DataType::F32, f32::MIN as f64)
    } else {
        (DataType::I32, i32::MIN as f64)
    };
    let cell_w = input.transform[1] * factor as f64;
    let cell_h = input.transform[5] * factor as f64; // negative
    let bottom = input.transform[3] + input.transform[5] * h as f64;
    let mut out = Raster::new(
        cols,
        rows,
        t,
        [
            input.transform[0],
            cell_w,
            0.0,
            bottom - cell_h * rows as f64,
            0.0,
            cell_h,
        ],
    );
    out.epsg = input.epsg;
    out.nodata = Some(nodata);
    let mut values = Vec::new();
    for br in 0..rows {
        for bc in 0..cols {
            // Blocks count from the bottom; the array's rows run from the top.
            let r1 = h.saturating_sub(br * factor);
            let r0 = r1.saturating_sub(factor);
            let (c0, c1) = (bc * factor, (bc * factor + factor).min(w));
            values.clear();
            let mut cells = 0;
            for r in r0..r1 {
                for c in c0..c1 {
                    cells += 1;
                    if let Some(v) = input.get(c, r) {
                        values.push(v);
                    }
                }
            }
            let complete = values.len() == factor * factor;
            let v = if values.is_empty() || (!ignore_nodata && (!complete || cells != values.len()))
            {
                None
            } else {
                compute(stat.focal(), &mut values, None)
            };
            out.bands[0][(rows - 1 - br) * cols + bc] = v.map_or(nodata, |v| t.store(v));
        }
    }
    Some(out)
}

/// Block Statistics (`ignore_nodata`: DATA).
pub fn block_statistics(
    input: &Raster,
    nb: Neighborhood,
    stat: FocalStatistic,
    ignore_nodata: bool,
) -> Option<Raster> {
    // A block larger than any raster this reads would only be a huge list of cells.
    if nb.cell_count() > 4_000_000 {
        return None;
    }
    // Block size and the cells of a block the statistic reads.
    let (bw, bh, mask): (usize, usize, Vec<(usize, usize)>) = match nb {
        Neighborhood::Rectangle { width, height } if width > 0 && height > 0 => (
            width,
            height,
            (0..height)
                .flat_map(|y| (0..width).map(move |x| (x, y)))
                .collect(),
        ),
        Neighborhood::Circle { .. } | Neighborhood::Annulus { .. } => {
            let r = match nb {
                Neighborhood::Circle { radius } => radius.floor() as isize,
                Neighborhood::Annulus { outer, .. } => outer.floor() as isize,
                _ => 0,
            };
            let side = (2 * r + 1).max(1) as usize;
            let cells = nb
                .offsets()
                .into_iter()
                .map(|(dx, dy)| ((dx + r) as usize, (dy + r) as usize))
                .collect();
            (side, side, cells)
        }
        _ => return None,
    };
    let float = input.data_type.is_float();
    let (t, nodata) = match stat {
        FocalStatistic::Mean | FocalStatistic::Std => (DataType::F32, f32::MAX as f64),
        _ if float => (DataType::F32, f32::MAX as f64),
        FocalStatistic::Sum => (DataType::I32, i32::MAX as f64),
        FocalStatistic::Range => (DataType::U32, u32::MAX as f64),
        FocalStatistic::Variety => {
            let bits = [1u32, 2, 4, 8]
                .into_iter()
                .find(|b| mask.len() < (1usize << b) - 1)
                .unwrap_or(8);
            (DataType::U8, ((1u32 << bits) - 1) as f64)
        }
        _ => (DataType::I32, i32::MIN as f64),
    };
    let mut out = input.like(t);
    out.nodata = Some(nodata);
    let (w, h) = (input.width, input.height);
    let mut values = Vec::new();
    for r0 in (0..h).step_by(bh) {
        for c0 in (0..w).step_by(bw) {
            values.clear();
            let mut missing = false;
            for &(x, y) in &mask {
                let (c, r) = (c0 + x, r0 + y);
                if c >= w || r >= h {
                    missing = true;
                    continue;
                }
                match input.get(c, r) {
                    Some(v) => values.push(v),
                    None => missing = true,
                }
            }
            let v = if values.is_empty() || (missing && !ignore_nodata) {
                nodata
            } else {
                t.store(compute(stat, &mut values, None)?)
            };
            for r in r0..(r0 + bh).min(h) {
                for c in c0..(c0 + bw).min(w) {
                    out.bands[0][r * w + c] = v;
                }
            }
        }
    }
    Some(out)
}

/// Slice into `zones` equal intervals of the value range, the lowest numbered `base`.
pub fn slice_equal_interval(input: &Raster, zones: usize, base: i64) -> Option<Raster> {
    if zones == 0 {
        return None;
    }
    let valid: Vec<f64> = input.bands[0]
        .iter()
        .copied()
        .filter(|v| !input.is_nodata(*v))
        .collect();
    let min = valid.iter().copied().fold(f64::INFINITY, f64::min);
    let max = valid.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if valid.is_empty() {
        return None;
    }
    // An integer raster's values are cells of width 1, so its range is one wider (observed).
    let span = if input.data_type.is_float() {
        max - min
    } else {
        max - min + 1.0
    };
    let interval = span / zones as f64;
    let (t, nodata) = integer_output(base as f64, (base + zones as i64 - 1) as f64, true);
    let mut out = input.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = input.bands[0]
        .iter()
        .map(|&v| {
            if input.is_nodata(v) {
                return nodata;
            }
            let z = if interval == 0.0 {
                1.0
            } else {
                (((v - min) / interval).floor() + 1.0).min(zones as f64)
            };
            t.store(z + base as f64 - 1.0)
        })
        .collect();
    Some(out)
}

/// Pick: each cell of `position` (1-based) names the input its value comes from.
pub fn pick(position: &Raster, inputs: &[Operand]) -> Option<Raster> {
    if inputs.is_empty() {
        return None;
    }
    let n = position.width * position.height;
    let at = |i: usize| -> Option<f64> {
        let p = position.get(i % position.width, i / position.width)?;
        let k = p as usize;
        if p.fract() != 0.0 || k == 0 || k > inputs.len() {
            return None;
        }
        match &inputs[k - 1] {
            Operand::Constant(c) => Some(*c),
            Operand::Raster(r) => {
                if (r.width, r.height) != (position.width, position.height) {
                    return None;
                }
                r.get(i % r.width, i / r.width)
            }
        }
    };
    let float = inputs.iter().any(|o| o.is_float());
    let values: Vec<Option<f64>> = (0..n).map(at).collect();
    if float {
        let mut out = position.like(DataType::F32);
        out.nodata = Some(f32::MIN as f64);
        out.bands[0] = values
            .iter()
            .map(|v| v.map_or(f32::MIN as f64, |v| DataType::F32.store(v)))
            .collect();
        return Some(out);
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for o in inputs {
        let (a, b) = match o {
            Operand::Constant(c) => (*c, *c),
            Operand::Raster(r) => r
                .statistics
                .unwrap_or_else(|| crate::local::type_range_of(r.data_type)),
        };
        lo = lo.min(a);
        hi = hi.max(b);
    }
    let (t, nodata) = integer_output(lo, hi, true);
    let mut out = position.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = values
        .iter()
        .map(|v| v.map_or(nodata, |v| t.store(v)))
        .collect();
    Some(out)
}

/// Region Group with four-neighbor connectivity (WITHIN); see [`crate::generalize::region_group`].
pub fn region_group_four(input: &Raster) -> Raster {
    crate::generalize::region_group(input, false, false, None)
}
