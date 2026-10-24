//! Spatial Analyst extraction and reclassification tools that keep the input grid: Extract by
//! Attributes / Rectangle / Circle, Slice and Lookup.

use crate::local::ValueClause;
use crate::resample::typed_cells;
use crate::{DataType, Raster};

/// The cells of `input` where `keep(col, row)` holds and that have data, typed as Extract by
/// Rectangle / Circle give them (the grid is the input's: ArcGIS does not trim it).
fn keep_cells(input: &Raster, keep: impl Fn(f64, f64) -> bool) -> Raster {
    let cells: Vec<Option<f64>> = (0..input.height)
        .flat_map(|row| (0..input.width).map(move |col| (col, row)))
        .map(|(col, row)| {
            let v = input.bands[0][row * input.width + col];
            let (x, y) = cell_center(input, col, row);
            (!input.is_nodata(v) && keep(x, y)).then_some(v)
        })
        .collect();
    typed_cells(input, &cells, input.data_type)
}

fn cell_center(r: &Raster, col: usize, row: usize) -> (f64, f64) {
    let t = r.transform;
    let (c, w) = (col as f64 + 0.5, row as f64 + 0.5);
    (t[0] + c * t[1] + w * t[2], t[3] + c * t[4] + w * t[5])
}

/// Extract by Rectangle: `rect` is (xmin, ymin, xmax, ymax); cells whose centers are inside it
/// (or outside, with `inside` false) keep their values.
pub fn extract_by_rectangle(input: &Raster, rect: (f64, f64, f64, f64), inside: bool) -> Raster {
    let (x0, y0, x1, y1) = rect;
    keep_cells(input, |x, y| {
        (x >= x0 && x <= x1 && y >= y0 && y <= y1) == inside
    })
}

/// Extract by Circle: cells whose centers are within `radius` of `center`.
pub fn extract_by_circle(input: &Raster, center: (f64, f64), radius: f64, inside: bool) -> Raster {
    keep_cells(input, |x, y| {
        let (dx, dy) = (x - center.0, y - center.1);
        (dx * dx + dy * dy <= radius * radius) == inside
    })
}

/// Extract by Attributes: the cells matching `clause` keep their values; the type and NoData
/// are the input's (32-bit float with NoData −FLT_MAX when it has none).
pub fn extract_by_attributes(input: &Raster, clause: &ValueClause) -> Raster {
    let mut out = input.clone();
    let nodata = input.nodata.unwrap_or(if input.data_type.is_float() {
        f32::MIN as f64
    } else {
        match input.data_type {
            DataType::U8 => 255.0,
            DataType::I8 => -128.0,
            DataType::U16 => f64::from(u16::MAX),
            DataType::I16 => f64::from(i16::MIN),
            DataType::U32 => f64::from(u32::MAX),
            _ => f64::from(i32::MIN),
        }
    });
    out.nodata = Some(nodata);
    out.bands[0] = input.bands[0]
        .iter()
        .map(|&v| {
            if input.is_nodata(v) || !clause.matches(v) {
                nodata
            } else {
                v
            }
        })
        .collect();
    out
}

/// How Slice cuts the value range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceType {
    EqualInterval,
    EqualArea,
}

/// Slice: `zones` classes from the lowest value up, numbered from `base`. Equal interval cuts
/// the value range into equal parts; equal area puts about as many cells in each class.
pub fn slice(input: &Raster, zones: usize, kind: SliceType, base: i64) -> Raster {
    let valid: Vec<f64> = input.bands[0]
        .iter()
        .copied()
        .filter(|&v| !input.is_nodata(v))
        .collect();
    let (lo, hi) = valid
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    let cuts: Vec<f64> = match kind {
        SliceType::EqualInterval => (1..zones)
            .map(|k| lo + (hi - lo) * k as f64 / zones as f64)
            .collect(),
        SliceType::EqualArea => {
            let mut sorted = valid.clone();
            sorted.sort_by(f64::total_cmp);
            (1..zones)
                .map(|k| sorted[(sorted.len() * k / zones).min(sorted.len() - 1)])
                .collect()
        }
    };
    let cells: Vec<Option<f64>> = input.bands[0]
        .iter()
        .map(|&v| {
            (!input.is_nodata(v)).then(|| (cuts.partition_point(|&c| c <= v) as i64 + base) as f64)
        })
        .collect();
    let (min, max) = (base as f64, (base + zones as i64 - 1) as f64);
    let present_min = cells.iter().flatten().fold(max, |a, &v| a.min(v));
    let present_max = cells.iter().flatten().fold(min, |a, &v| a.max(v));
    let (t, nodata) = crate::zonal::integer_output(present_min, present_max, true);
    let mut out = input.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = cells.iter().map(|v| v.unwrap_or(nodata)).collect();
    out
}

/// Lookup: each cell takes the `field` value of the row of the raster's attribute table whose
/// `VALUE` is the cell's value. The `VALUE` field itself gives an integer raster; any other
/// field gives 32-bit float.
pub fn lookup(input: &Raster, table: &[(f64, f64)], is_value_field: bool) -> Raster {
    let map: std::collections::HashMap<u64, f64> =
        table.iter().map(|&(v, f)| (v.to_bits(), f)).collect();
    let cells: Vec<Option<f64>> = input.bands[0]
        .iter()
        .map(|&v| {
            if input.is_nodata(v) {
                None
            } else {
                map.get(&v.to_bits()).copied()
            }
        })
        .collect();
    if is_value_field && !input.data_type.is_float() {
        return typed_cells(input, &cells, input.data_type);
    }
    let nodata = f32::MIN as f64;
    let mut out = input.like(DataType::F32);
    out.nodata = Some(nodata);
    out.bands[0] = cells.iter().map(|v| v.unwrap_or(nodata)).collect();
    out
}

/// Flip: the rows in reverse order (top to bottom), on the same extent.
pub fn flip(input: &Raster) -> Raster {
    let mut out = input.clone();
    for row in 0..input.height {
        let from = (input.height - 1 - row) * input.width;
        out.bands[0][row * input.width..(row + 1) * input.width]
            .copy_from_slice(&input.bands[0][from..from + input.width]);
    }
    out
}

/// Mirror: the columns in reverse order (left to right), on the same extent.
pub fn mirror(input: &Raster) -> Raster {
    let mut out = input.clone();
    for row in 0..input.height {
        let line = &input.bands[0][row * input.width..(row + 1) * input.width];
        for (col, &v) in line.iter().rev().enumerate() {
            out.bands[0][row * input.width + col] = v;
        }
    }
    out
}

/// Shift: the same cells with the origin moved by (`dx`, `dy`).
pub fn shift(input: &Raster, dx: f64, dy: f64) -> Raster {
    let mut out = input.clone();
    out.transform[0] += dx;
    out.transform[3] += dy;
    out
}

/// Clip (Data Management) to a rectangle: the cells whose centers are inside it, on the input's
/// grid (or, with `maintain_extent`, with the origin moved to the rectangle's corner). Float
/// output has NoData +3.4e38 unless `nodata` says otherwise; other types keep the input's.
pub fn clip_rectangle(
    input: &Raster,
    rect: (f64, f64, f64, f64),
    nodata: Option<f64>,
    maintain_extent: bool,
) -> Option<Raster> {
    let (x0, y0, x1, y1) = rect;
    let t = input.transform;
    let (cw, ch) = (t[1], -t[5]);
    // The window is the cells touched by the rectangle, snapped outward from the left and
    // bottom edges and downward from the top edge (observed).
    let clamp = |v: f64, max: usize| v.clamp(0.0, max as f64) as usize;
    let c0 = clamp(((x0 - t[0]) / cw).floor(), input.width);
    let c1 = clamp(((x1 - t[0]) / cw).ceil(), input.width);
    let r0 = clamp(((t[3] - y1) / ch).ceil(), input.height);
    let r1 = clamp(((t[3] - y0) / ch).ceil(), input.height);
    if c1 <= c0 || r1 <= r0 {
        return None;
    }
    let (width, height) = (c1 - c0, r1 - r0);
    let out_nodata = nodata
        .or(input.nodata)
        .unwrap_or(if input.data_type.is_float() {
            f64::from(3.4e38_f32)
        } else {
            255.0
        });
    let mut transform = input.transform;
    transform[0] += c0 as f64 * input.transform[1];
    transform[3] += r0 as f64 * input.transform[5];
    if maintain_extent {
        // The cells are stretched to fill the rectangle exactly.
        transform[0] = x0;
        transform[3] = y1;
        transform[1] = (x1 - x0) / width as f64;
        transform[5] = -(y1 - y0) / height as f64;
    }
    let mut out = Raster::new(width, height, input.data_type, transform);
    out.epsg = input.epsg;
    out.nodata = Some(out_nodata);
    out.bands[0] = (0..height)
        .flat_map(|r| (0..width).map(move |c| (r, c)))
        .map(|(r, c)| {
            let v = input.bands[0][(r0 + r) * input.width + c0 + c];
            if input.is_nodata(v) { out_nodata } else { v }
        })
        .collect();
    Some(out)
}
