//! Vector to raster conversion: Feature To Raster, Polygon To Raster and Point
//! To Raster.
//!
//! The output grid starts at the lower-left corner of the features' extent (for
//! points and lines first widened by half a cell on every side) and has
//! round(extent / cell size) columns and rows.

use geoforge_core::{Geometry, GeometryKind};

use crate::{DataType, Raster};

/// How a polygon cell gets its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonAssignment {
    /// The polygon containing the cell center.
    CellCenter,
    /// The polygon covering most of the cell.
    MaximumArea,
}

/// How a point cell combines several points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointAssignment {
    MostFrequent,
    Sum,
    Mean,
    Maximum,
    Minimum,
    Count,
}

/// The value type of the source field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Integer,
    Single,
    Double,
}

fn extent(features: &[(Geometry, f64)]) -> Option<(f64, f64, f64, f64)> {
    let mut e: Option<(f64, f64, f64, f64)> = None;
    for (g, _) in features {
        for p in &g.xy {
            e = Some(match e {
                None => (p[0], p[1], p[0], p[1]),
                Some((a, b, c, d)) => (a.min(p[0]), b.min(p[1]), c.max(p[0]), d.max(p[1])),
            });
        }
    }
    e
}

/// An empty grid for `features` at `cell` size, NoData everywhere.
fn grid(features: &[(Geometry, f64)], cell: f64, widen: bool) -> Option<Raster> {
    let (mut x0, mut y0, mut x1, mut y1) = extent(features)?;
    if widen {
        x0 -= cell / 2.0;
        y0 -= cell / 2.0;
        x1 += cell / 2.0;
        y1 += cell / 2.0;
    }
    let cols = (((x1 - x0) / cell).round() as usize).max(1);
    let rows = (((y1 - y0) / cell).round() as usize).max(1);
    let top = y0 + rows as f64 * cell;
    let mut r = Raster::new(cols, rows, DataType::F64, [x0, cell, 0.0, top, 0.0, -cell]);
    r.bands[0] = vec![f64::NAN; cols * rows];
    Some(r)
}

/// Picks the cell type and NoData as ArcGIS does and fills NoData cells.
fn finish(mut r: Raster, kind: ValueKind) -> Raster {
    let (data_type, nodata) = match kind {
        ValueKind::Double => (DataType::F64, f64::MIN),
        ValueKind::Single => (DataType::F32, f32::MIN as f64),
        ValueKind::Integer => {
            let (min, max) = r.bands[0]
                .iter()
                .filter(|v| !v.is_nan())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
                    (a.min(v), b.max(v))
                });
            let (min, max) = if min.is_finite() {
                (min, max)
            } else {
                (0.0, 0.0)
            };
            crate::zonal::integer_output(min, max, true)
        }
    };
    r.data_type = data_type;
    r.nodata = Some(nodata);
    for v in &mut r.bands[0] {
        *v = if v.is_nan() {
            nodata
        } else {
            data_type.store(*v)
        };
    }
    r
}

fn rings(g: &Geometry) -> Vec<&[[f64; 2]]> {
    let n = g.parts.len();
    (0..n)
        .map(|i| {
            let s = g.parts[i] as usize;
            let e = if i + 1 < n {
                g.parts[i + 1] as usize
            } else {
                g.xy.len()
            };
            &g.xy[s..e]
        })
        .collect()
}

fn contains(g: &Geometry, x: f64, y: f64) -> bool {
    let mut inside = false;
    for ring in rings(g) {
        for w in ring.windows(2) {
            let (a, b) = (w[0], w[1]);
            if (a[1] > y) != (b[1] > y) && x < a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Area of the polygon inside an axis-aligned box (rings clipped one by one;
/// holes run the other way, so signed areas add up).
fn area_in_box(g: &Geometry, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let mut total = 0.0;
    for ring in rings(g) {
        let mut pts: Vec<[f64; 2]> = ring.to_vec();
        let clips: [(usize, f64, bool); 4] =
            [(0, x0, true), (0, x1, false), (1, y0, true), (1, y1, false)];
        for (axis, bound, keep_above) in clips {
            if pts.is_empty() {
                break;
            }
            let inside = |p: &[f64; 2]| {
                if keep_above {
                    p[axis] >= bound
                } else {
                    p[axis] <= bound
                }
            };
            let mut out = Vec::new();
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                let (ia, ib) = (inside(&a), inside(&b));
                if ia {
                    out.push(a);
                }
                if ia != ib {
                    let t = (bound - a[axis]) / (b[axis] - a[axis]);
                    out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                }
            }
            pts = out;
        }
        let n = pts.len();
        total += (0..n)
            .map(|i| pts[i][0] * pts[(i + 1) % n][1] - pts[(i + 1) % n][0] * pts[i][1])
            .sum::<f64>()
            / 2.0;
    }
    total.abs()
}

/// Polygon To Raster (Feature To Raster uses the cell center).
pub fn polygons_to_raster(
    features: &[(Geometry, f64)],
    cell: f64,
    assignment: PolygonAssignment,
    kind: ValueKind,
) -> Option<Raster> {
    let mut r = grid(features, cell, false)?;
    let (w, h) = (r.width, r.height);
    let t = r.transform;
    let mut best = vec![0.0f64; w * h];
    let mut covered = vec![0.0f64; w * h];
    for (g, value) in features {
        if g.kind != GeometryKind::Polygon || g.xy.is_empty() {
            continue;
        }
        let (gx0, gy0, gx1, gy1) = extent(&[(g.clone(), 0.0)])?;
        let c0 = (((gx0 - t[0]) / cell).floor().max(0.0)) as usize;
        let c1 = ((((gx1 - t[0]) / cell).ceil()) as usize).min(w);
        let r0 = (((t[3] - gy1) / cell).floor().max(0.0)) as usize;
        let r1 = ((((t[3] - gy0) / cell).ceil()) as usize).min(h);
        for row in r0..r1 {
            for col in c0..c1 {
                let i = row * w + col;
                let x = t[0] + (col as f64 + 0.5) * cell;
                let y = t[3] - (row as f64 + 0.5) * cell;
                match assignment {
                    PolygonAssignment::CellCenter => {
                        if r.bands[0][i].is_nan() && contains(g, x, y) {
                            r.bands[0][i] = *value;
                        }
                    }
                    PolygonAssignment::MaximumArea => {
                        let half = cell / 2.0;
                        let a = area_in_box(g, x - half, y - half, x + half, y + half);
                        covered[i] += a;
                        if a > best[i] {
                            best[i] = a;
                            r.bands[0][i] = *value;
                        }
                    }
                }
            }
        }
    }
    if assignment == PolygonAssignment::MaximumArea {
        // The uncovered part of a cell competes too: NoData when it is larger.
        for i in 0..w * h {
            if best[i] <= cell * cell - covered[i] {
                r.bands[0][i] = f64::NAN;
            }
        }
    }
    Some(finish(r, kind))
}

/// Point To Raster (Feature To Raster uses the most frequent value).
pub fn points_to_raster(
    features: &[(Geometry, f64)],
    cell: f64,
    assignment: PointAssignment,
    kind: ValueKind,
) -> Option<Raster> {
    let mut r = grid(features, cell, true)?;
    let (w, h) = (r.width, r.height);
    let t = r.transform;
    let mut cells: Vec<Vec<f64>> = vec![Vec::new(); w * h];
    for (g, value) in features {
        for p in &g.xy {
            // A point on a cell boundary belongs to the cell to its left / above.
            let col = ((p[0] - t[0]) / cell).ceil() as isize - 1;
            let row = ((t[3] - p[1]) / cell).ceil() as isize - 1;
            if col >= 0 && row >= 0 && (col as usize) < w && (row as usize) < h {
                cells[row as usize * w + col as usize].push(*value);
            }
        }
    }
    for (i, vals) in cells.iter_mut().enumerate() {
        if vals.is_empty() {
            continue;
        }
        let n = vals.len() as f64;
        r.bands[0][i] = match assignment {
            PointAssignment::Sum => vals.iter().sum(),
            PointAssignment::Mean => vals.iter().sum::<f64>() / n,
            PointAssignment::Maximum => vals.iter().copied().fold(f64::MIN, f64::max),
            PointAssignment::Minimum => vals.iter().copied().fold(f64::MAX, f64::min),
            PointAssignment::Count => n,
            PointAssignment::MostFrequent => {
                // Ties go to the value seen first in feature order.
                let mut best = (vals[0], 0usize);
                for (k, v) in vals.iter().enumerate() {
                    let count = vals.iter().filter(|&&u| u == *v).count();
                    if count > best.1 && !vals[..k].contains(v) {
                        best = (*v, count);
                    }
                }
                best.0
            }
        };
    }
    // Points keep a 32-bit integer field type (polygons and lines narrow it);
    // counts are signed 8-bit or wider, and means of integers single (observed).
    if kind == ValueKind::Integer {
        let filled = r.bands[0].iter().filter(|v| !v.is_nan());
        let max = filled.copied().fold(0.0, f64::max);
        let data_type = match assignment {
            PointAssignment::Mean => return Some(finish(r, ValueKind::Single)),
            PointAssignment::Count if max <= i8::MAX as f64 => DataType::I8,
            PointAssignment::Count if max <= i16::MAX as f64 => DataType::I16,
            _ => DataType::I32,
        };
        let nodata = match data_type {
            DataType::I8 => i8::MIN as f64,
            DataType::I16 => i16::MIN as f64,
            _ => i32::MIN as f64,
        };
        r.data_type = data_type;
        r.nodata = Some(nodata);
        for v in &mut r.bands[0] {
            *v = if v.is_nan() {
                nodata
            } else {
                data_type.store(*v)
            };
        }
        return Some(r);
    }
    Some(finish(r, kind))
}

/// Polyline To Raster: every cell whose interior a line passes through gets its
/// value (touching a cell's edge or corner is not enough); where lines share a
/// cell the feature with the most length in it wins (MAXIMUM_LENGTH).
pub fn lines_to_raster(features: &[(Geometry, f64)], cell: f64, kind: ValueKind) -> Option<Raster> {
    let mut r = grid(features, cell, true)?;
    let (w, h) = (r.width, r.height);
    let t = r.transform;
    // Length inside each cell per feature.
    let mut lengths: std::collections::HashMap<usize, Vec<(usize, f64)>> = Default::default();
    // The cell holding a point, with points on a grid line going to the cell
    // left of / above it.
    let cell_of = |v: f64| -> isize {
        let k = v.round();
        if (v - k).abs() < 1e-9 {
            k as isize - 1
        } else {
            v.floor() as isize
        }
    };
    for (fi, (g, _)) in features.iter().enumerate() {
        for part in rings(g) {
            for seg in part.windows(2) {
                let seg_len = (seg[1][0] - seg[0][0]).hypot(seg[1][1] - seg[0][1]);
                let to_grid = |p: [f64; 2]| [(p[0] - t[0]) / cell, (t[3] - p[1]) / cell];
                let (a, b) = (to_grid(seg[0]), to_grid(seg[1]));
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                if dx == 0.0 && dy == 0.0 {
                    continue;
                }
                // Cells just inside the segment's ends.
                let e = 1e-7;
                let (mut cx, mut cy) = (
                    if dx > 0.0 {
                        a[0].floor() as isize
                    } else if dx < 0.0 {
                        a[0].ceil() as isize - 1
                    } else {
                        cell_of(a[0])
                    },
                    if dy > 0.0 {
                        a[1].floor() as isize
                    } else if dy < 0.0 {
                        a[1].ceil() as isize - 1
                    } else {
                        cell_of(a[1])
                    },
                );
                let (ex, ey) = (
                    if dx > 0.0 {
                        (b[0] - e).floor() as isize
                    } else if dx < 0.0 {
                        (b[0] + e).ceil() as isize - 1
                    } else {
                        cell_of(b[0])
                    },
                    if dy > 0.0 {
                        (b[1] - e).floor() as isize
                    } else if dy < 0.0 {
                        (b[1] + e).ceil() as isize - 1
                    } else {
                        cell_of(b[1])
                    },
                );
                let (sx, sy) = (dx.signum() as isize, dy.signum() as isize);
                let boundary = |c: isize, s: isize| (c + if s > 0 { 1 } else { 0 }) as f64;
                let mut tx = if dx != 0.0 {
                    (boundary(cx, sx) - a[0]) / dx
                } else {
                    f64::INFINITY
                };
                let mut ty = if dy != 0.0 {
                    (boundary(cy, sy) - a[1]) / dy
                } else {
                    f64::INFINITY
                };
                let (stx, sty) = (1.0 / dx.abs(), 1.0 / dy.abs());
                let limit = (w + h) * 4;
                let mut entered = 0.0f64;
                for _ in 0..limit {
                    let left = tx.min(ty).min(1.0);
                    if cx >= 0 && cy >= 0 && (cx as usize) < w && (cy as usize) < h {
                        let per = lengths.entry(cy as usize * w + cx as usize).or_default();
                        let add = (left - entered).max(0.0) * seg_len;
                        match per.iter_mut().find(|p| p.0 == fi) {
                            Some(p) => p.1 += add,
                            None => per.push((fi, add)),
                        }
                    }
                    entered = left;
                    if (cx == ex && cy == ey) || (tx >= 1.0 - 1e-12 && ty >= 1.0 - 1e-12) {
                        break;
                    }
                    if (tx - ty).abs() < 1e-12 {
                        // Through a grid corner: straight into the diagonal cell.
                        cx += sx;
                        cy += sy;
                        tx += stx;
                        ty += sty;
                    } else if tx < ty {
                        cx += sx;
                        tx += stx;
                    } else {
                        cy += sy;
                        ty += sty;
                    }
                }
            }
        }
    }
    for (i, per) in lengths {
        // The longest; ties go to the earlier feature.
        let mut best = per[0];
        for &p in &per[1..] {
            if p.1 > best.1 + 1e-9 || ((p.1 - best.1).abs() <= 1e-9 && p.0 < best.0) {
                best = p;
            }
        }
        r.bands[0][i] = features[best.0].1;
    }
    Some(finish(r, kind))
}
