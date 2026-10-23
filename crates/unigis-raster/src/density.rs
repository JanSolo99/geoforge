//! Spatial Analyst density surfaces: Kernel Density, Point Density and Line Density.
//!
//! Findings from ArcGIS Pro 3.7 (see `scripts/research/euclidean-density/`):
//! - Kernel Density spreads each feature with the quartic kernel
//!   `3/π · (1 − (d/r)²)² / r²` (lines: integrated along the line); the grid starts at
//!   the lower-left corner of the features' extent widened by half a cell on every
//!   side and has round(extent / cell) columns and rows.
//! - The default radius is Silverman's rule of thumb:
//!   `0.9 · min(SD, √(1/ln 2) · Dm) · n^−0.2` with the (weighted) mean center, SD the
//!   standard distance, Dm the median distance to the center and n the sum of the
//!   weights; for lines the points are the line centroids.
//! - Densities are per output area unit (square kilometers unless asked otherwise);
//!   Line Density is length per area unit, so it scales with the square root.
//! - Point Density counts the points in the neighborhood around each cell center; its
//!   grid starts `(w + h) / 798` before the points' lower-left corner and has
//!   round((extent + 2 · that) / cell) columns and rows.

use unigis_core::{Geometry, GeometryKind};

use crate::{DataType, Raster};

/// A feature with its weight (the population field, or 1).
pub type Weighted = (Geometry, f64);

/// The shape Point Density counts points in around each cell center. Sizes are in map
/// units (the caller converts cell units).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Neighborhood {
    Circle {
        radius: f64,
    },
    /// Full width and height.
    Rectangle {
        width: f64,
        height: f64,
    },
    Annulus {
        inner: f64,
        outer: f64,
    },
    /// Angles in degrees counterclockwise from east.
    Wedge {
        radius: f64,
        start: f64,
        end: f64,
    },
}

/// The largest grid a density tool will make.
const MAX_CELLS: f64 = 2.0e8;

fn float_raster(x0: f64, y0: f64, cols: usize, rows: usize, cell: f64) -> Raster {
    let top = y0 + rows as f64 * cell;
    let mut r = Raster::new(cols, rows, DataType::F32, [x0, cell, 0.0, top, 0.0, -cell]);
    r.nodata = Some(f32::MIN as f64);
    r.bands[0] = vec![0.0; cols * rows];
    r
}

fn grid_size(width: f64, cell: f64) -> Option<usize> {
    let n = (width / cell).round();
    (n >= 1.0 && n.is_finite()).then_some(n as usize)
}

/// Every vertex of the weighted features, and the pieces (segment endpoints) of lines.
fn points_of(features: &[Weighted]) -> Vec<([f64; 2], f64)> {
    let mut out = Vec::new();
    for (g, w) in features {
        for p in &g.xy {
            out.push((*p, *w));
        }
    }
    out
}

fn segments_of(features: &[Weighted]) -> Vec<([f64; 2], [f64; 2], f64)> {
    let mut out = Vec::new();
    for (g, w) in features {
        if !matches!(g.kind, GeometryKind::Polyline | GeometryKind::Polygon) {
            continue;
        }
        let n = g.xy.len();
        let starts: Vec<usize> = if g.parts.is_empty() {
            vec![0]
        } else {
            g.parts.iter().map(|&s| s as usize).collect()
        };
        for (i, &s) in starts.iter().enumerate() {
            let e = starts.get(i + 1).map_or(n, |&e| e);
            for k in s..e.saturating_sub(1) {
                out.push((g.xy[k], g.xy[k + 1], *w));
            }
        }
    }
    out
}

/// Silverman's default search radius over weighted points.
fn silverman(points: &[([f64; 2], f64)]) -> Option<f64> {
    let total: f64 = points.iter().map(|p| p.1).sum();
    if points.is_empty() || total <= 0.0 {
        return None;
    }
    let mx = points.iter().map(|p| p.0[0] * p.1).sum::<f64>() / total;
    let my = points.iter().map(|p| p.0[1] * p.1).sum::<f64>() / total;
    let mut dist: Vec<(f64, f64)> = points
        .iter()
        .map(|p| ((p.0[0] - mx).hypot(p.0[1] - my), p.1))
        .collect();
    let sd = (dist.iter().map(|d| d.0 * d.0 * d.1).sum::<f64>() / total).sqrt();
    dist.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The (weighted) median; with an even split, the mean of the two middle values.
    let mut acc = 0.0;
    let mut median = dist[0].0;
    for (i, d) in dist.iter().enumerate() {
        acc += d.1;
        if acc >= total / 2.0 {
            median = if acc == total / 2.0 && i + 1 < dist.len() {
                (d.0 + dist[i + 1].0) / 2.0
            } else {
                d.0
            };
            break;
        }
    }
    let r = 0.9 * sd.min((1.0 / std::f64::consts::LN_2).sqrt() * median) * total.powf(-0.2);
    (r > 0.0).then_some(r)
}

/// Centroids of the lines (length-weighted segment midpoints) for the default radius.
fn line_centroids(features: &[Weighted], weighted: bool) -> Vec<([f64; 2], f64)> {
    let mut out = Vec::new();
    for f in features {
        let segs = segments_of(std::slice::from_ref(f));
        let len: f64 = segs
            .iter()
            .map(|s| (s.1[0] - s.0[0]).hypot(s.1[1] - s.0[1]))
            .sum();
        if len <= 0.0 {
            continue;
        }
        let (mut x, mut y) = (0.0, 0.0);
        for s in &segs {
            let l = (s.1[0] - s.0[0]).hypot(s.1[1] - s.0[1]);
            x += l * (s.0[0] + s.1[0]) / 2.0;
            y += l * (s.0[1] + s.1[1]) / 2.0;
        }
        out.push(([x / len, y / len], if weighted { f.1 } else { 1.0 }));
    }
    out
}

fn extent_of(points: &[([f64; 2], f64)]) -> Option<(f64, f64, f64, f64)> {
    let first = points.first()?.0;
    Some(points.iter().fold(
        (first[0], first[1], first[0], first[1]),
        |(a, b, c, d), (p, _)| (a.min(p[0]), b.min(p[1]), c.max(p[0]), d.max(p[1])),
    ))
}

/// Grid from the extent widened by half a cell on every side (Kernel and Line Density).
fn widened_grid(extent: (f64, f64, f64, f64), cell: f64) -> Result<Raster, String> {
    let (x0, y0, x1, y1) = extent;
    let (w, h) = (x1 - x0 + cell, y1 - y0 + cell);
    if (w / cell) * (h / cell) > MAX_CELLS {
        return Err("the density raster would have too many cells".into());
    }
    let (cols, rows) = grid_size(w, cell)
        .zip(grid_size(h, cell))
        .ok_or("the features' extent is empty")?;
    Ok(float_raster(
        x0 - cell / 2.0,
        y0 - cell / 2.0,
        cols,
        rows,
        cell,
    ))
}

/// Visits the cells whose centers are within `reach` of the box (`bx0`..`bx1`,
/// `by0`..`by1`).
#[allow(clippy::too_many_arguments)]
fn cells_near(
    t: [f64; 6],
    width: usize,
    height: usize,
    bx0: f64,
    by0: f64,
    bx1: f64,
    by1: f64,
    reach: f64,
    mut visit: impl FnMut(usize, usize, f64, f64),
) {
    let cell = t[1];
    let lo = |v: f64, o: f64| ((v - o) / cell - 0.5).floor().max(0.0) as usize;
    let c0 = lo(bx0 - reach, t[0]);
    let c1 =
        (((bx1 + reach - t[0]) / cell - 0.5).ceil().max(-1.0) + 1.0).min(width as f64) as usize;
    let top = t[3];
    let r0 = (((top - by1 - reach) / cell - 0.5).floor().max(0.0)) as usize;
    let r1 =
        (((top - (by0 - reach)) / cell - 0.5).ceil().max(-1.0) + 1.0).min(height as f64) as usize;
    for row in r0..r1 {
        let cy = top - (row as f64 + 0.5) * cell;
        for col in c0..c1 {
            visit(col, row, t[0] + (col as f64 + 0.5) * cell, cy);
        }
    }
}

/// Quartic kernel weight times 3/π, per unit of weight (before dividing by r²).
fn quartic(d2: f64, r2: f64) -> f64 {
    let u = 1.0 - d2 / r2;
    3.0 / std::f64::consts::PI * u * u
}

/// Kernel Density of points. `area` is the output area unit in squared map units
/// (1 for SQUARE_MAP_UNITS, 1e6 for SQUARE_KILOMETERS in meters); with
/// `expected_counts` the output is the density times the cell area instead.
pub fn kernel_density_points(
    features: &[Weighted],
    cell: f64,
    radius: Option<f64>,
    area: f64,
    expected_counts: bool,
) -> Result<Raster, String> {
    let pts = points_of(features);
    let ext = extent_of(&pts).ok_or("no features")?;
    let radius = match radius {
        Some(r) if r > 0.0 => r,
        Some(_) => return Err("the search radius must be positive".into()),
        None => silverman(&pts).ok_or("cannot work out a default search radius")?,
    };
    let mut out = widened_grid(ext, cell)?;
    let (w, r2) = (out.width, radius * radius);
    let scale = if expected_counts { cell * cell } else { 1.0 } * area_scale(area, expected_counts);
    for (p, wt) in &pts {
        cells_near(
            out.transform,
            out.width,
            out.height,
            p[0],
            p[1],
            p[0],
            p[1],
            radius,
            |c, r, x, y| {
                let d2 = (x - p[0]).powi(2) + (y - p[1]).powi(2);
                if d2 < r2 {
                    out.bands[0][r * w + c] += wt * quartic(d2, r2) / r2 * scale;
                }
            },
        );
    }
    store_f32(&mut out);
    Ok(out)
}

/// Densities are per map unit squared; the output unit has `area` map units squared.
/// Expected counts (density × cell area) are in cells, not per unit area.
fn area_scale(area: f64, expected_counts: bool) -> f64 {
    if expected_counts { 1.0 } else { area }
}

fn store_f32(r: &mut Raster) {
    for v in &mut r.bands[0] {
        *v = DataType::F32.store(*v);
    }
}

/// Integral of the quartic kernel along a segment as seen from (`x`, `y`), per unit of
/// weight and before dividing by r²: `∫ 3/π (1 − d²/r²)² ds`.
fn kernel_along(a: [f64; 2], b: [f64; 2], x: f64, y: f64, r: f64) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return 0.0;
    }
    let (ux, uy) = (dx / len, dy / len);
    let (qx, qy) = (x - a[0], y - a[1]);
    let t = qx * ux + qy * uy;
    let h2 = (qx * qx + qy * qy - t * t).max(0.0);
    let r2 = r * r;
    if h2 >= r2 {
        return 0.0;
    }
    let half = (r2 - h2).sqrt();
    let lo = (-half).max(-t);
    let hi = half.min(len - t);
    if hi <= lo {
        return 0.0;
    }
    let big_a = 1.0 - h2 / r2;
    let prim = |u: f64| {
        big_a * big_a * u - 2.0 * big_a * u.powi(3) / (3.0 * r2) + u.powi(5) / (5.0 * r2 * r2)
    };
    3.0 / std::f64::consts::PI * (prim(hi) - prim(lo))
}

/// Length of a segment inside the circle of `r` around (`x`, `y`).
fn length_in_circle(a: [f64; 2], b: [f64; 2], x: f64, y: f64, r: f64) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return 0.0;
    }
    let (qx, qy) = (x - a[0], y - a[1]);
    let t = (qx * dx + qy * dy) / len;
    let h2 = (qx * qx + qy * qy - t * t).max(0.0);
    if h2 >= r * r {
        return 0.0;
    }
    let half = (r * r - h2).sqrt();
    ((half).min(len - t) - (-half).max(-t)).max(0.0)
}

/// Kernel Density of lines: the kernel integrated along each line. Densities are length
/// per output area unit (so they scale with the square root of `area`).
pub fn kernel_density_lines(
    features: &[Weighted],
    cell: f64,
    radius: Option<f64>,
    area: f64,
    weighted: bool,
) -> Result<Raster, String> {
    let segs = segments_of(features);
    let ends: Vec<_> = segs.iter().flat_map(|s| [(s.0, 1.0), (s.1, 1.0)]).collect();
    let ext = extent_of(&ends).ok_or("no line features")?;
    let radius = match radius {
        Some(r) if r > 0.0 => r,
        Some(_) => return Err("the search radius must be positive".into()),
        None => silverman(&line_centroids(features, weighted))
            .ok_or("cannot work out a default search radius")?,
    };
    let mut out = widened_grid(ext, cell)?;
    let (w, r2) = (out.width, radius * radius);
    let scale = area.sqrt();
    for (a, b, wt) in &segs {
        let wt = if weighted { *wt } else { 1.0 };
        cells_near(
            out.transform,
            out.width,
            out.height,
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[0].max(b[0]),
            a[1].max(b[1]),
            radius,
            |c, r, x, y| {
                out.bands[0][r * w + c] += wt * kernel_along(*a, *b, x, y, radius) / r2 * scale;
            },
        );
    }
    store_f32(&mut out);
    Ok(out)
}

/// Line Density: weighted length of lines inside the circle around each cell center
/// divided by the circle's area, per output area unit.
pub fn line_density(
    features: &[Weighted],
    cell: f64,
    radius: f64,
    area: f64,
    weighted: bool,
) -> Result<Raster, String> {
    if radius <= 0.0 {
        return Err("the search radius must be positive".into());
    }
    let segs = segments_of(features);
    let ends: Vec<_> = segs.iter().flat_map(|s| [(s.0, 1.0), (s.1, 1.0)]).collect();
    let ext = extent_of(&ends).ok_or("no line features")?;
    let mut out = widened_grid(ext, cell)?;
    let w = out.width;
    let scale = area.sqrt() / (std::f64::consts::PI * radius * radius);
    for (a, b, wt) in &segs {
        let wt = if weighted { *wt } else { 1.0 };
        cells_near(
            out.transform,
            out.width,
            out.height,
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[0].max(b[0]),
            a[1].max(b[1]),
            radius,
            |c, r, x, y| {
                out.bands[0][r * w + c] += wt * length_in_circle(*a, *b, x, y, radius) * scale;
            },
        );
    }
    store_f32(&mut out);
    Ok(out)
}

fn neighborhood_reach(n: Neighborhood) -> f64 {
    match n {
        Neighborhood::Circle { radius } | Neighborhood::Wedge { radius, .. } => radius,
        Neighborhood::Rectangle { width, height } => {
            width.max(height) / 2.0 * std::f64::consts::SQRT_2
        }
        Neighborhood::Annulus { outer, .. } => outer,
    }
}

fn neighborhood_area(n: Neighborhood) -> f64 {
    use std::f64::consts::PI;
    match n {
        Neighborhood::Circle { radius } => PI * radius * radius,
        Neighborhood::Rectangle { width, height } => width * height,
        Neighborhood::Annulus { inner, outer } => PI * (outer * outer - inner * inner),
        Neighborhood::Wedge { radius, start, end } => {
            let span = (end - start).rem_euclid(360.0);
            PI * radius * radius * if span == 0.0 { 1.0 } else { span / 360.0 }
        }
    }
}

fn inside(n: Neighborhood, dx: f64, dy: f64) -> bool {
    let d = dx.hypot(dy);
    match n {
        Neighborhood::Circle { radius } => d <= radius,
        Neighborhood::Rectangle { width, height } => {
            dx.abs() <= width / 2.0 && dy.abs() <= height / 2.0
        }
        Neighborhood::Annulus { inner, outer } => d >= inner && d <= outer,
        Neighborhood::Wedge { radius, start, end } => {
            if d > radius {
                return false;
            }
            if d == 0.0 {
                return true;
            }
            let az = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            let span = (end - start).rem_euclid(360.0);
            span == 0.0 || (az - start).rem_euclid(360.0) <= span
        }
    }
}

/// Point Density: the weighted number of points in the neighborhood around each cell
/// center divided by the neighborhood's area, per output area unit.
pub fn point_density(
    features: &[Weighted],
    cell: f64,
    neighborhood: Neighborhood,
    area: f64,
) -> Result<Raster, String> {
    let pts = points_of(features);
    let (x0, y0, x1, y1) = extent_of(&pts).ok_or("no features")?;
    let off = ((x1 - x0) + (y1 - y0)) / 798.0;
    let (w, h) = (x1 - x0 + 2.0 * off, y1 - y0 + 2.0 * off);
    if (w / cell) * (h / cell) > MAX_CELLS {
        return Err("the density raster would have too many cells".into());
    }
    let (cols, rows) = grid_size(w, cell)
        .zip(grid_size(h, cell))
        .ok_or("the features' extent is too small for the cell size")?;
    let mut out = float_raster(x0 - off, y0 - off, cols, rows, cell);
    let reach = neighborhood_reach(neighborhood);
    let scale = area / neighborhood_area(neighborhood);
    for (p, wt) in &pts {
        cells_near(
            out.transform,
            out.width,
            out.height,
            p[0],
            p[1],
            p[0],
            p[1],
            reach,
            |c, r, x, y| {
                if inside(neighborhood, p[0] - x, p[1] - y) {
                    out.bands[0][r * cols + c] += wt * scale;
                }
            },
        );
    }
    store_f32(&mut out);
    Ok(out)
}

/// The area of an ArcGIS area unit in squared meters (the unit of the density tools'
/// `area_unit_scale_factor`); SQUARE_MAP_UNITS counts as square meters (map units are assumed to be meters).
pub fn area_unit(name: &str) -> Result<f64, String> {
    Ok(match name.to_ascii_uppercase().as_str() {
        "SQUARE_MAP_UNITS" | "SQUARE_METERS" => 1.0,
        "SQUARE_KILOMETERS" => 1e6,
        "HECTARES" => 1e4,
        "ACRES" => 4_046.856_422_4,
        "SQUARE_FEET" => 0.092_903_04,
        "SQUARE_YARDS" => 0.836_127_36,
        "SQUARE_INCHES" => 0.000_645_16,
        "SQUARE_MILES" => 2_589_988.110_336,
        "ARES" => 100.0,
        "SQUARE_CENTIMETERS" => 0.0001,
        "SQUARE_DECIMETERS" => 0.01,
        "SQUARE_MILLIMETERS" => 1e-6,
        other => return Err(format!("area unit {other}")),
    })
}

/// Parses the neighborhood string of Point Density: `Circle r`, `Rectangle w h`,
/// `Annulus inner outer`, `Wedge r start end`, each followed by `MAP` (or
/// `MAP_UNITS`) or `CELL`.
pub fn parse_neighborhood(text: &str, cell: f64) -> Result<Neighborhood, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let (kind, rest) = words.split_first().ok_or("name a neighborhood")?;
    let (units, nums) = match rest.split_last() {
        Some((u, n)) if u.parse::<f64>().is_err() => (*u, n),
        _ => ("MAP", rest),
    };
    let k = match units.to_ascii_uppercase().as_str() {
        "MAP" | "MAP_UNITS" => 1.0,
        "CELL" => cell,
        other => return Err(format!("neighborhood units {other}")),
    };
    let n: Vec<f64> = nums
        .iter()
        .map(|w| {
            w.parse::<f64>()
                .map_err(|_| format!("neighborhood number {w}"))
        })
        .collect::<Result<_, _>>()?;
    let need = |c: usize| {
        if n.len() == c && n.iter().all(|v| v.is_finite() && *v >= 0.0) {
            Ok(())
        } else {
            Err(format!("{kind} takes {c} non-negative numbers"))
        }
    };
    Ok(match kind.to_ascii_uppercase().as_str() {
        "CIRCLE" => {
            need(1)?;
            Neighborhood::Circle { radius: n[0] * k }
        }
        "RECTANGLE" => {
            need(2)?;
            Neighborhood::Rectangle {
                width: n[0] * k,
                height: n[1] * k,
            }
        }
        "ANNULUS" => {
            need(2)?;
            Neighborhood::Annulus {
                inner: n[0] * k,
                outer: n[1] * k,
            }
        }
        "WEDGE" => {
            need(3)?;
            Neighborhood::Wedge {
                radius: n[0] * k,
                start: n[1],
                end: n[2],
            }
        }
        other => return Err(format!("neighborhood {other}")),
    })
}
