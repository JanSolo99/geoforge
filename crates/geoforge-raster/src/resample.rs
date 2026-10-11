//! Resample (Data Management), Euclidean Distance and Extract by Mask (Spatial
//! Analyst).

use crate::{DataType, Raster};

/// Resampling technique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Technique {
    Nearest,
    Bilinear,
    Cubic,
    Majority,
}

fn cubic_weight(t: f64) -> f64 {
    let a = -0.5;
    let t = t.abs();
    if t <= 1.0 {
        (a + 2.0) * t * t * t - (a + 3.0) * t * t + 1.0
    } else if t < 2.0 {
        a * t * t * t - 5.0 * a * t * t + 8.0 * a * t - 4.0 * a
    } else {
        0.0
    }
}

/// Resample to square cells of `cell` size. The grid keeps the input's top-left
/// corner and has round(extent / cell) columns and rows. Bilinear and cubic
/// (Keys, a = −0.5) interpolate between cell centers, dropping taps outside the
/// raster or on NoData and renormalizing the remaining weights; majority takes
/// the most common value among input cells centered in the output cell widened
/// by one input cell (ties to the smaller value).
pub fn resample(r: &Raster, cell: f64, technique: Technique) -> Raster {
    let (cw, ch) = (r.cell_width(), r.cell_height());
    let cols = ((r.width as f64 * cw / cell).round() as usize).max(1);
    let rows = ((r.height as f64 * ch / cell).round() as usize).max(1);
    let mut out = Raster::new(
        cols,
        rows,
        r.data_type,
        [r.transform[0], cell, 0.0, r.transform[3], 0.0, -cell],
    );
    out.epsg = r.epsg;
    out.nodata = r.nodata;
    let nodata = r.nodata.unwrap_or(f64::NAN);
    let (w, h) = (r.width as isize, r.height as isize);
    let at = |x: isize, y: isize| -> Option<f64> {
        if x < 0 || y < 0 || x >= w || y >= h {
            None
        } else {
            r.get(x as usize, y as usize)
        }
    };
    for row in 0..rows {
        for col in 0..cols {
            // Output cell center in input cell units.
            let fx = (col as f64 + 0.5) * cell / cw;
            let fy = (row as f64 + 0.5) * cell / ch;
            let v = match technique {
                Technique::Nearest => at(fx.floor() as isize, fy.floor() as isize),
                Technique::Bilinear | Technique::Cubic => {
                    let (gx, gy) = (fx - 0.5, fy - 0.5);
                    let (x0, y0) = (gx.floor() as isize, gy.floor() as isize);
                    let taps: &[isize] = if technique == Technique::Cubic {
                        &[-1, 0, 1, 2]
                    } else {
                        &[0, 1]
                    };
                    let weight = |d: f64| {
                        if technique == Technique::Cubic {
                            cubic_weight(d)
                        } else {
                            (1.0 - d.abs()).max(0.0)
                        }
                    };
                    let (mut sum, mut wsum) = (0.0, 0.0);
                    for &m in taps {
                        for &n in taps {
                            let (x, y) = (x0 + n, y0 + m);
                            if let Some(v) = at(x, y) {
                                let wt = weight(gx - x as f64) * weight(gy - y as f64);
                                sum += wt * v;
                                wsum += wt;
                            }
                        }
                    }
                    (wsum != 0.0).then(|| sum / wsum)
                }
                Technique::Majority => {
                    // Input cells centered within the output cell widened by one
                    // input cell on every side.
                    let (x0, x1) = (
                        (col as f64 * cell / cw - 1.5).ceil() as isize,
                        ((col + 1) as f64 * cell / cw + 0.5).ceil() as isize,
                    );
                    let (y0, y1) = (
                        (row as f64 * cell / ch - 1.5).ceil() as isize,
                        ((row + 1) as f64 * cell / ch + 0.5).ceil() as isize,
                    );
                    let mut vals: Vec<f64> = Vec::new();
                    for y in y0..y1 {
                        for x in x0..x1 {
                            vals.extend(at(x, y));
                        }
                    }
                    let mut best: Option<(f64, usize)> = None;
                    for (k, v) in vals.iter().enumerate() {
                        if vals[..k].contains(v) {
                            continue;
                        }
                        let n = vals.iter().filter(|&&u| u == *v).count();
                        // Ties go to the smaller value.
                        if best.is_none_or(|b| n > b.1 || (n == b.1 && *v < b.0)) {
                            best = Some((*v, n));
                        }
                    }
                    best.map(|b| b.0)
                }
            };
            out.bands[0][row * cols + col] = v.map_or(nodata, |v| r.data_type.store(v));
        }
    }
    out
}

/// Squared 1-D distance transform (Felzenszwalb–Huttenlocher) of `f` in place;
/// infinite entries are cells without a source.
fn edt_1d(f: &mut [f64]) {
    const FAR: f64 = 1e20;
    let n = f.len();
    if n == 0 {
        return;
    }
    let g: Vec<f64> = f
        .iter()
        .map(|&x| if x.is_finite() { x } else { FAR })
        .collect();
    let mut v = vec![0usize; n];
    let mut z = vec![0.0; n + 1];
    let mut k = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        loop {
            let p = v[k];
            let s = ((g[q] + (q * q) as f64) - (g[p] + (p * p) as f64)) / (2.0 * (q - p) as f64);
            if s <= z[k] {
                k -= 1;
                continue;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    k = 0;
    for (q, out) in f.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        let d = (q as f64 - p as f64).powi(2) + g[p];
        *out = if d >= FAR / 2.0 { f64::INFINITY } else { d };
    }
}

/// Spatial Analyst Euclidean Distance from the cells of `source` that have values,
/// in map units between cell centers (square cells); beyond `max_distance`,
/// NoData.
pub fn euclidean_distance(source: &Raster, max_distance: Option<f64>) -> Raster {
    let (w, h) = (source.width, source.height);
    let mut grid: Vec<f64> = (0..w * h)
        .map(|i| {
            if source.is_nodata(source.bands[0][i]) {
                f64::INFINITY
            } else {
                0.0
            }
        })
        .collect();
    let mut col = vec![0.0; h];
    for x in 0..w {
        for y in 0..h {
            col[y] = grid[y * w + x];
        }
        edt_1d(&mut col);
        for y in 0..h {
            grid[y * w + x] = col[y];
        }
    }
    for y in 0..h {
        edt_1d(&mut grid[y * w..(y + 1) * w]);
    }
    let nodata = 3.4e38_f32 as f64;
    let mut out = source.like(DataType::F32);
    out.nodata = Some(nodata);
    let cell = source.cell_width();
    out.bands[0] = grid
        .into_iter()
        .map(|d2| {
            let d = d2.sqrt() * cell;
            if !d.is_finite() || max_distance.is_some_and(|m| d > m) {
                nodata
            } else {
                DataType::F32.store(d)
            }
        })
        .collect();
    out
}

/// Spatial Analyst Extract by Mask: the input's cells where the mask has data.
/// As in ArcGIS, the output takes the larger of the two cell sizes, aligned to the
/// input raster over the intersection of both extents, and samples both rasters
/// at its cell centers. Float output is 32-bit with NoData −FLT_MAX; integer
/// output takes ArcGIS's type choice for the values present.
pub fn extract_by_mask(input: &Raster, mask: &Raster) -> Option<Raster> {
    let cell = input.cell_width().max(mask.cell_width());
    let (transform, cols, rows) = common_grid(mask, input, cell)?;
    let values = onto_grid(input, transform, cols, rows);
    let mask = onto_grid(mask, transform, cols, rows);
    let cells: Vec<Option<f64>> = (0..cols * rows)
        .map(|i| {
            let v = values.bands[0][i];
            (!mask.is_nodata(mask.bands[0][i]) && !values.is_nodata(v)).then_some(v)
        })
        .collect();
    if input.data_type.is_float() {
        let nodata = f32::MIN as f64;
        let mut out = values.like(DataType::F32);
        out.nodata = Some(nodata);
        out.bands[0] = cells.iter().map(|v| v.unwrap_or(nodata)).collect();
        return Some(out);
    }
    let (min, max) = cells
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
    let eight = matches!(input.data_type, DataType::U8 | DataType::I8);
    let (t, nodata) = crate::zonal::integer_output(min, max, eight);
    let mut out = values.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = cells.iter().map(|v| v.unwrap_or(nodata)).collect();
    Some(out)
}

/// Nearest-neighbor copy of `r` onto a north-up grid given by its transform and
/// size; cells outside `r` are NoData.
pub fn onto_grid(r: &Raster, transform: [f64; 6], cols: usize, rows: usize) -> Raster {
    let nodata = r.nodata.unwrap_or(if r.data_type.is_float() {
        f32::MIN as f64
    } else {
        crate::focal::type_max(r.data_type)
    });
    let mut out = Raster::new(cols, rows, r.data_type, transform);
    out.epsg = r.epsg;
    out.nodata = Some(nodata);
    out.statistics = r.statistics;
    let t = &r.transform;
    for row in 0..rows {
        let y = transform[3] + transform[5] * (row as f64 + 0.5);
        let fy = ((y - t[3]) / t[5]).floor();
        for col in 0..cols {
            let x = transform[0] + transform[1] * (col as f64 + 0.5);
            let fx = ((x - t[0]) / t[1]).floor();
            let inside = fx >= 0.0 && fy >= 0.0 && fx < r.width as f64 && fy < r.height as f64;
            out.bands[0][row * cols + col] = if inside {
                r.bands[0][fy as usize * r.width + fx as usize]
            } else {
                nodata
            };
        }
    }
    out
}

/// Zones and values on one grid, as Spatial Analyst's zonal tools line them up:
/// the value raster's cells over the intersection of both extents (widened to
/// whole value cells), with the zones sampled at the cell centers. `None` when
/// the extents do not overlap.
pub fn zones_on_value_grid(zones: &Raster, values: &Raster) -> Option<(Raster, Raster)> {
    let same = (zones.width, zones.height, zones.transform)
        == (values.width, values.height, values.transform);
    if same {
        return Some((zones.clone(), values.clone()));
    }
    let (transform, cols, rows) = common_grid(zones, values, values.cell_width())?;
    Some((
        onto_grid(zones, transform, cols, rows),
        onto_grid(values, transform, cols, rows),
    ))
}

/// A north-up grid of `cell`-sized cells aligned to `snap`'s origin covering the
/// intersection of both extents (widened to whole cells); `None` when the
/// extents do not overlap.
fn common_grid(other: &Raster, snap: &Raster, cell: f64) -> Option<([f64; 6], usize, usize)> {
    let (ax0, ay0, ax1, ay1) = other.extent();
    let (bx0, by0, bx1, by1) = snap.extent();
    let (x0, y0, x1, y1) = (ax0.max(bx0), ay0.max(by0), ax1.min(bx1), ay1.min(by1));
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let t = &snap.transform;
    // A 1e-9 cell margin keeps edges on grid lines from adding a sliver cell.
    let c0 = ((x0 - t[0]) / cell + 1e-9).floor();
    let c1 = ((x1 - t[0]) / cell - 1e-9).ceil();
    let r0 = ((t[3] - y1) / cell + 1e-9).floor();
    let r1 = ((t[3] - y0) / cell - 1e-9).ceil();
    let transform = [t[0] + cell * c0, cell, 0.0, t[3] - cell * r0, 0.0, -cell];
    Some((transform, (c1 - c0) as usize, (r1 - r0) as usize))
}
