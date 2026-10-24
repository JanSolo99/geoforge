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
/// the most common value among the 4 × 4 input cells nearest the output cell
/// center, clipped to the output extent (ties to the smaller value), and is
/// NoData where the center falls in a NoData cell or outside the input.
/// The (columns, rows) `resample(r, cell, ..)` gives.
pub fn output_size(r: &Raster, cell: f64) -> (u64, u64) {
    let size =
        |n: usize, c: f64| ((n as f64 * c / cell).round().clamp(1.0, u64::MAX as f64)) as u64;
    (
        size(r.width, r.cell_width()),
        size(r.height, r.cell_height()),
    )
}

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
                    // The 4 × 4 input cells nearest the output cell center,
                    // clipped to the output extent; NoData where the center
                    // falls in a NoData cell or outside the input.
                    let (max_x, max_y) = (
                        (cols as f64 * cell / cw).min(r.width as f64),
                        (rows as f64 * cell / ch).min(r.height as f64),
                    );
                    let (cx, cy) = (fx.floor() as isize, fy.floor() as isize);
                    if at(cx, cy).is_none() {
                        None
                    } else {
                        let (x0, y0) = (
                            (fx - 0.5).floor() as isize - 1,
                            (fy - 0.5).floor() as isize - 1,
                        );
                        let mut vals: Vec<f64> = Vec::new();
                        for y in y0..y0 + 4 {
                            for x in x0..x0 + 4 {
                                if (x as f64) < max_x && (y as f64) < max_y {
                                    vals.extend(at(x, y));
                                }
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
                }
            };
            out.bands[0][row * cols + col] = v.map_or(nodata, |v| r.data_type.store(v));
        }
    }
    out
}

/// Squared 1-D distance transform (Felzenszwalb–Huttenlocher) of `f` in place;
/// infinite entries are cells without a source.
pub(crate) fn edt_1d(f: &mut [f64]) {
    let mut arg = vec![0; f.len()];
    edt_1d_arg(f, &mut arg);
}

/// [`edt_1d`] that also reports, per entry, the index of the nearest finite entry.
fn edt_1d_arg(f: &mut [f64], arg: &mut [usize]) {
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
        arg[q] = p;
        *out = if d >= FAR / 2.0 { f64::INFINITY } else { d };
    }
}

/// For every cell, the (column, row) of the nearest cell of `source` that has a
/// value, measured between cell centers; `None` where the source is empty.
///
/// Found by vector propagation (each cell offers its source to a neighbor), which
/// also reproduces ArcGIS's pick among equally near sources:
/// 1. each row from the east, then from the west (only strictly nearer sources);
/// 2. top to bottom, each row west to east taking the north-east, north and
///    north-west neighbors' sources, where an equally near one replaces the current
///    pick; then east to west from the east neighbor (strictly nearer only);
/// 3. bottom to top, each row west to east from the south-west, south, south-east
///    and west neighbors, then east to west from the east neighbor (strictly nearer
///    only).
///
/// This matches ArcGIS Pro 3.7 on every cell of more than 4,600 ties in the
/// reference and probe rasters, including its distances.
fn nearest_sources(source: &Raster) -> Vec<Option<(usize, usize)>> {
    let (w, h) = (source.width, source.height);
    let mut near: Vec<usize> = (0..w * h)
        .map(|i| {
            if source.is_nodata(source.bands[0][i]) {
                NO_SOURCE
            } else {
                i
            }
        })
        .collect();
    let mut d2: Vec<u64> = near
        .iter()
        .map(|&s| if s == NO_SOURCE { u64::MAX } else { 0 })
        .collect();
    let mut p = Propagation {
        w,
        near: &mut near,
        d2: &mut d2,
    };
    for y in 0..h {
        let row = y * w;
        for x in (0..w.saturating_sub(1)).rev() {
            p.offer(row + x, row + x + 1, false);
        }
        for x in 1..w {
            p.offer(row + x, row + x - 1, false);
        }
    }
    for y in 0..h {
        let row = y * w;
        if y > 0 {
            for x in 0..w {
                let k = row + x;
                if x + 1 < w {
                    p.offer(k, k - w + 1, true);
                }
                p.offer(k, k - w, true);
                if x > 0 {
                    p.offer(k, k - w - 1, true);
                }
            }
        }
        for x in (0..w.saturating_sub(1)).rev() {
            p.offer(row + x, row + x + 1, false);
        }
    }
    for y in (0..h).rev() {
        let row = y * w;
        for x in 0..w {
            let k = row + x;
            if y + 1 < h {
                if x > 0 {
                    p.offer(k, k + w - 1, false);
                }
                p.offer(k, k + w, false);
                if x + 1 < w {
                    p.offer(k, k + w + 1, false);
                }
            }
            if x > 0 {
                p.offer(k, k - 1, false);
            }
        }
        for x in (0..w.saturating_sub(1)).rev() {
            p.offer(row + x, row + x + 1, false);
        }
    }
    near.iter()
        .map(|&s| (s != NO_SOURCE).then(|| (s % w, s / w)))
        .collect()
}

const NO_SOURCE: usize = usize::MAX;

/// Per-cell nearest source (cell index) and its squared distance in cells.
struct Propagation<'a> {
    w: usize,
    near: &'a mut [usize],
    d2: &'a mut [u64],
}

impl Propagation<'_> {
    /// Offers cell `from`'s source to cell `to`: taken when strictly nearer, or when
    /// equally near and `ties` is set.
    fn offer(&mut self, to: usize, from: usize, ties: bool) {
        let s = self.near[from];
        if s == NO_SOURCE {
            return;
        }
        let dx = (s % self.w) as i64 - (to % self.w) as i64;
        let dy = (s / self.w) as i64 - (to / self.w) as i64;
        let d = (dx * dx + dy * dy) as u64;
        if d < self.d2[to] || (ties && d == self.d2[to]) {
            self.near[to] = s;
            self.d2[to] = d;
        }
    }
}

/// Spatial Analyst Euclidean Direction: the azimuth in whole degrees clockwise from
/// north (360 due north, 0 on source cells) from each cell to its nearest source
/// cell. 16-bit signed, NoData 32767 beyond `max_distance`.
pub fn euclidean_direction(source: &Raster, max_distance: Option<f64>) -> Raster {
    let near = nearest_sources(source);
    let cell = source.cell_width();
    let mut out = source.like(DataType::I16);
    out.nodata = Some(32767.0);
    out.bands[0] = near
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let Some((sx, sy)) = *n else { return 32767.0 };
            let (x, y) = ((i % source.width) as f64, (i / source.width) as f64);
            let (dx, dy) = (sx as f64 - x, sy as f64 - y);
            if max_distance.is_some_and(|m| dx.hypot(dy) * cell > m) {
                return 32767.0;
            }
            if dx == 0.0 && dy == 0.0 {
                return 0.0;
            }
            let a = (dx.atan2(-dy).to_degrees().rem_euclid(360.0) + 0.5).floor();
            if a == 0.0 || a == 360.0 { 360.0 } else { a }
        })
        .collect();
    out
}

/// Spatial Analyst Euclidean Allocation: the value of the nearest source cell,
/// NoData beyond `max_distance`. Integer output takes ArcGIS's type choice for the
/// values present (as Extract by Mask does); float output is 32-bit.
pub fn euclidean_allocation(source: &Raster, max_distance: Option<f64>) -> Raster {
    let near = nearest_sources(source);
    let cell = source.cell_width();
    let cells: Vec<Option<f64>> = near
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (sx, sy) = (*n)?;
            let (x, y) = ((i % source.width) as f64, (i / source.width) as f64);
            if max_distance.is_some_and(|m| (sx as f64 - x).hypot(sy as f64 - y) * cell > m) {
                return None;
            }
            Some(source.bands[0][sy * source.width + sx])
        })
        .collect();
    let (t, nodata) = if source.data_type.is_float() {
        (DataType::F32, f32::MIN as f64)
    } else {
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
        crate::zonal::integer_output(
            min,
            max,
            matches!(source.data_type, DataType::U8 | DataType::I8),
        )
    };
    let mut out = source.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = cells.iter().map(|v| v.unwrap_or(nodata)).collect();
    out
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

/// Spatial Analyst Nibble: the cells of `mask` that have no value take the value of the nearest
/// cell of `input` that lies under a mask value (input cells without value stay so with `preserve_nodata`) (Euclidean, ties as in Euclidean Allocation); the
/// others keep theirs. With `data_only`, input cells without value are no source and the nibbled
/// cells fill from the nearest cell with data; otherwise they are sources whose NoData spreads.
pub fn nibble(
    input: &Raster,
    mask: &Raster,
    data_only: bool,
    preserve_nodata: bool,
) -> Option<Raster> {
    if (input.width, input.height) != (mask.width, mask.height) {
        return None;
    }
    let n = input.width * input.height;
    let mut source = input.like(input.data_type);
    // Sources: unmasked cells; a marker value stands for input NoData when it counts as a source.
    let marker = f64::NEG_INFINITY;
    source.nodata = Some(f64::INFINITY);
    source.bands[0] = (0..n)
        .map(|i| {
            if mask.is_nodata(mask.bands[0][i]) {
                f64::INFINITY
            } else if input.is_nodata(input.bands[0][i]) {
                if data_only { f64::INFINITY } else { marker }
            } else {
                input.bands[0][i]
            }
        })
        .collect();
    let near = nearest_sources(&source);
    let nodata = input.nodata;
    let mut out = input.clone();
    for i in 0..n {
        if !mask.is_nodata(mask.bands[0][i])
            || (preserve_nodata && input.is_nodata(input.bands[0][i]))
        {
            continue;
        }
        // Among equally near sources ArcGIS takes the smallest value (NoData counts as the smallest).
        let best = near[i].map(|(sx, sy)| {
            let (row, col) = ((i / input.width) as i64, (i % input.width) as i64);
            let d2 = (sx as i64 - col).pow(2) + (sy as i64 - row).pow(2);
            let reach = (d2 as f64).sqrt().ceil() as i64;
            let value = |r: usize, c: usize| source.bands[0][r * input.width + c];
            let mut pick = (sy, sx);
            for r in (row - reach).max(0)..=(row + reach).min(input.height as i64 - 1) {
                for c in (col - reach).max(0)..=(col + reach).min(input.width as i64 - 1) {
                    let v = value(r as usize, c as usize);
                    if (c - col).pow(2) + (r - row).pow(2) == d2
                        && v != f64::INFINITY
                        && v < value(pick.0, pick.1)
                    {
                        pick = (r as usize, c as usize);
                    }
                }
            }
            (pick.1, pick.0)
        });
        let v = best.map(|(sx, sy)| source.bands[0][sy * input.width + sx]);
        out.bands[0][i] = match (v, nodata) {
            (Some(v), Some(nd)) if v == marker => nd,
            (Some(v), _) => v,
            (None, Some(nd)) => nd,
            (None, None) => 0.0,
        };
    }
    // The output's NoData is the highest value of the cell type.
    if let Some(nd) = nodata {
        let new = crate::focal::type_max(input.data_type);
        if new != nd {
            for v in &mut out.bands[0] {
                if *v == nd {
                    *v = new;
                }
            }
            out.nodata = Some(new);
        }
    }
    Some(out)
}
