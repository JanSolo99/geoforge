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

/// Whether (x, y) is inside the ring, scanline style: the spans between the ring's crossings of
/// the row include their right end and not their left end (a cell center on an edge belongs to
/// the polygon on the right edges only; matched ArcGIS).
fn in_ring(ring: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut crossings: Vec<f64> = Vec::new();
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        if (a.1 > y) != (b.1 > y) {
            crossings.push((b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0);
        }
    }
    crossings.sort_by(f64::total_cmp);
    crossings
        .chunks(2)
        .any(|c| c.len() == 2 && x > c[0] && x <= c[1])
}

/// Extract by Polygon: cells whose centers are inside the polygon keep their values (the grid
/// is the input's).
pub fn extract_by_polygon(input: &Raster, ring: &[(f64, f64)], inside: bool) -> Raster {
    keep_cells(input, |x, y| in_ring(ring, x, y) == inside)
}

/// Extract by Points: the cells that contain one of the points keep their values (with
/// `inside` false, all the others do).
pub fn extract_by_points(input: &Raster, points: &[(f64, f64)], inside: bool) -> Raster {
    let t = input.transform;
    let hit: std::collections::HashSet<(i64, i64)> = points
        .iter()
        .map(|&(x, y)| {
            // A point on a cell edge belongs to the cell to its right / above it.
            let r = (t[3] - y) / -t[5];
            (
                ((x - t[0]) / t[1]).floor() as i64,
                if r.fract() == 0.0 {
                    r as i64 - 1
                } else {
                    r.floor() as i64
                },
            )
        })
        .collect();
    let cells: Vec<Option<f64>> = (0..input.height)
        .flat_map(|row| (0..input.width).map(move |col| (col, row)))
        .map(|(col, row)| {
            let v = input.bands[0][row * input.width + col];
            let on = hit.contains(&(col as i64, row as i64));
            (!input.is_nodata(v) && on == inside).then_some(v)
        })
        .collect();
    typed_cells(input, &cells, input.data_type)
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

/// Copy Raster: the cells as another pixel type (rounded, saturating) with the NoData value
/// given, else the input's, else ArcGIS's default for the source pixel type.
/// All bands retain their order; integer defaults may lie outside the pixel range.
pub fn copy_raster(input: &Raster, pixel_type: Option<DataType>, nodata: Option<f64>) -> Raster {
    let t = pixel_type.unwrap_or(input.data_type);
    // Copy Raster exposes high UInt32 source markers through signed32 defaults;
    // keep source masking against the original unsigned tag.
    let signed_u32_nodata = input
        .nodata
        .filter(|&nd| {
            input.data_type == DataType::U32
                && (2147483648.0..=f64::from(u32::MAX)).contains(&nd)
                && nd.fract() == 0.0
        })
        .map(|nd| f64::from(nd as u32 as i32));
    let out_nodata =
        nodata
            .or(signed_u32_nodata)
            .or(input.nodata)
            .unwrap_or(match input.data_type {
                DataType::F32 if t != DataType::F32 => 3.4e38,
                DataType::F32 => f64::from(3.4e38_f32),
                DataType::F64 => 1.79e308,
                DataType::U8 | DataType::I8 => 256.0,
                DataType::U16 => 65536.0,
                DataType::I16 => f64::from(i16::MAX),
                DataType::U32 | DataType::I32 => f64::from(i32::MAX),
            });
    // Statistics must use the same sentinel the floating TIFF actually stores.
    // For example, the I32 default rounds from 2147483647 to 2147483648 in F32.
    let out_nodata = if t.is_float() {
        t.store(out_nodata)
    } else if t == DataType::U32 && nodata.is_none() && signed_u32_nodata.is_some() {
        // The default negative marker in U32 output follows the observed F32
        // precision, while I32/F64 destinations retain the signed integer exactly.
        f64::from(out_nodata as f32)
    } else {
        out_nodata
    };
    // Negative unsigned markers remain invalid even when they round to zero.
    let negative_unsigned_marker =
        matches!(t, DataType::U8 | DataType::U16 | DataType::U32) && out_nodata < 0.0;
    let out_nodata = if t.is_float() {
        out_nodata
    } else {
        out_nodata.round_ties_even()
    };
    let integer_marker_outside_storage = !t.is_float()
        && (negative_unsigned_marker
            || out_nodata < t.store(f64::NEG_INFINITY)
            || out_nodata > t.store(f64::INFINITY));
    let convert = |v: f64| {
        if t.is_float() {
            t.store(v)
        } else {
            t.store((v + 0.5).floor())
        }
    };
    let mut out = input.like(t);
    out.nodata = Some(out_nodata);
    out.bands = input
        .bands
        .iter()
        .map(|band| {
            band.iter()
                .map(|&v| {
                    if input.is_nodata(v) && !integer_marker_outside_storage {
                        out_nodata
                    } else {
                        // Copy Raster preserves the converted original payload
                        // when the integer destination cannot store its marker.
                        convert(v)
                    }
                })
                .collect()
        })
        .collect();
    out
}

/// How Mosaic To New Raster combines rasters where they overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MosaicMethod {
    First,
    Last,
    Mean,
    Minimum,
    Maximum,
}

/// Mosaic To New Raster: the union of the extents, on the grid of the first raster (its cell size
/// unless `cell` says otherwise), each cell taking the value at its center from the rasters that
/// have data there, combined by `method`. NoData is the largest input NoData plus one, or +3.4e38
/// when an input has none and the output is float (observed).
pub fn mosaic(
    rasters: &[Raster],
    pixel_type: DataType,
    cell: Option<f64>,
    method: MosaicMethod,
) -> Option<Raster> {
    let first = rasters.first()?;
    let cell = cell.unwrap_or(first.transform[1]);
    let extent = |r: &Raster| {
        let t = r.transform;
        (
            t[0],
            t[3] + t[5] * r.height as f64,
            t[0] + t[1] * r.width as f64,
            t[3],
        )
    };
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for r in rasters {
        let e = extent(r);
        x0 = x0.min(e.0);
        y0 = y0.min(e.1);
        x1 = x1.max(e.2);
        y1 = y1.max(e.3);
    }
    let fe = extent(first);
    let snap = |lo: f64, base: f64| base + ((lo - base) / cell + 1e-9).floor() * cell;
    let snap_up = |hi: f64, base: f64| base + ((hi - base) / cell - 1e-9).ceil() * cell;
    let (ox0, oy0) = (snap(x0, fe.0), snap(y0, fe.1));
    let (ox1, oy1) = (snap_up(x1, fe.0), snap_up(y1, fe.1));
    let width = ((ox1 - ox0) / cell).round() as usize;
    let height = ((oy1 - oy0) / cell).round() as usize;
    let nodata = if rasters.iter().all(|r| r.nodata.is_some()) {
        rasters
            .iter()
            .filter_map(|r| r.nodata)
            .fold(f64::MIN, f64::max)
            + 1.0
    } else if pixel_type.is_float() {
        f64::from(3.4e38_f32)
    } else {
        rasters.iter().filter_map(|r| r.nodata).fold(0.0, f64::max) + 1.0
    };
    let mut out = Raster::new(width, height, pixel_type, [ox0, cell, 0.0, oy1, 0.0, -cell]);
    out.epsg = first.epsg;
    out.nodata = Some(nodata);
    for row in 0..height {
        for col in 0..width {
            let x = ox0 + (col as f64 + 0.5) * cell;
            let y = oy1 - (row as f64 + 0.5) * cell;
            let mut values = Vec::new();
            for r in rasters {
                let t = r.transform;
                let c = ((x - t[0]) / t[1]).floor();
                let rr = ((t[3] - y) / -t[5]).floor();
                if c < 0.0 || rr < 0.0 || c >= r.width as f64 || rr >= r.height as f64 {
                    continue;
                }
                let v = r.bands[0][rr as usize * r.width + c as usize];
                if !r.is_nodata(v) {
                    values.push(v);
                }
            }
            let v = match method {
                MosaicMethod::First => values.first().copied(),
                MosaicMethod::Last => values.last().copied(),
                MosaicMethod::Mean => {
                    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
                }
                MosaicMethod::Minimum => values.iter().copied().reduce(f64::min),
                MosaicMethod::Maximum => values.iter().copied().reduce(f64::max),
            };
            // Cells without data are the background 0 in an integer mosaic, NoData in a float one.
            let gap = if pixel_type.is_float() { nodata } else { 0.0 };
            out.bands[0][row * width + col] = v.map_or(gap, |v| pixel_type.store(v));
        }
    }
    Some(out)
}

/// Test: 1 where the cell matches the clause, else 0, as 8-bit with NoData 2 (observed).
pub fn test(input: &Raster, clause: &ValueClause) -> Raster {
    let mut out = input.like(DataType::U8);
    out.nodata = Some(2.0);
    out.bands[0] = input.bands[0]
        .iter()
        .map(|&v| {
            if input.is_nodata(v) {
                2.0
            } else {
                f64::from(clause.matches(v))
            }
        })
        .collect();
    out
}

/// Create Constant Raster: every cell holds `value`, on the grid with the cell size over the
/// extent (columns and rows rounded). Integer output takes the smallest type holding the value
/// (the NoData of the 1/2/4/8-bit layouts as for other integer outputs; -2147483647 for 32-bit
/// signed), float output is 32-bit float.
/// `CreateNormalRaster(cell_size, extent)`: Float32 values drawn from the standard normal
/// distribution (mean 0, standard deviation 1), NoData 3.4e38, on the grid Create Constant Raster
/// uses. ArcGIS's generators (ACM599, Mersenne Twister) are seeded in a way not reproduced here,
/// so the values match ArcGIS's statistically, not cell by cell; `seed` fixes ours.
pub fn normal_raster(cell: f64, extent: (f64, f64, f64, f64), seed: u64) -> Option<Raster> {
    let mut out = constant_raster(0.0, false, cell, extent)?;
    out.nodata = Some(f64::from(3.4e38_f32));
    let mut state = seed;
    // SplitMix64: uniform in (0, 1].
    let mut uniform = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 11) as f64 + 1.0) / (1u64 << 53) as f64
    };
    let n = out.bands[0].len();
    let mut values = Vec::with_capacity(n + 1);
    while values.len() < n {
        // Box-Muller: two independent normal values from two uniform ones.
        let (u1, u2) = (uniform(), uniform());
        let r = (-2.0 * u1.ln()).sqrt();
        let a = std::f64::consts::TAU * u2;
        values.push(r * a.cos());
        values.push(r * a.sin());
    }
    values.truncate(n);
    out.bands[0] = values.into_iter().map(|v| DataType::F32.store(v)).collect();
    Some(out)
}

pub fn constant_raster(
    value: f64,
    integer: bool,
    cell: f64,
    extent: (f64, f64, f64, f64),
) -> Option<Raster> {
    let (x0, y0, x1, y1) = extent;
    if !(cell > 0.0 && x1 > x0 && y1 > y0) {
        return None;
    }
    let width = ((x1 - x0) / cell).round().max(1.0) as usize;
    let height = ((y1 - y0) / cell).round().max(1.0) as usize;
    let transform = [x0, cell, 0.0, y0 + height as f64 * cell, 0.0, -cell];
    let (t, nodata, stored) = if integer {
        let v = value.trunc();
        if v.abs() >= f64::from(i16::MAX) && v >= 65535.0 || v < f64::from(i16::MIN) {
            (DataType::I32, -2147483647.0, v)
        } else {
            let (t, nd) = crate::zonal::integer_output(v, v, true);
            (t, nd, v)
        }
    } else {
        (DataType::F32, f32::MIN as f64, DataType::F32.store(value))
    };
    let mut out = Raster::new(width, height, t, transform);
    out.nodata = Some(nodata);
    out.bands[0] = vec![stored; width * height];
    Some(out)
}

#[cfg(test)]
mod normal_tests {
    use super::*;

    #[test]
    fn normal_raster_is_standard_normal() {
        let r = normal_raster(1.0, (0.0, 0.0, 200.0, 100.0), 7).unwrap();
        assert_eq!((r.width, r.height), (200, 100));
        let v = &r.bands[0];
        let n = v.len() as f64;
        let mean = v.iter().sum::<f64>() / n;
        let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
        assert!(mean.abs() < 0.03 && (sd - 1.0).abs() < 0.03, "{mean} {sd}");
        let again = normal_raster(1.0, (0.0, 0.0, 200.0, 100.0), 7).unwrap();
        assert_eq!(again.bands, r.bands);
    }
}
