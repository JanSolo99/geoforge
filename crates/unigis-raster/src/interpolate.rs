//! Spatial Analyst interpolation from points: Inverse Distance Weighted.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/idw.json`, runs through the bridge):
//! - The default output grid has cell centers from the points' lower-left corner on: the extent
//!   starts half a cell below it and has `round(width / cell) + 1` columns and `round(height / cell)
//!   + 1` rows. With a snap raster the points' own extent is snapped outward instead. The default cell size is `min(width, height) / 250`.
//! - A cell takes the weighted mean of the points the search radius finds, each weighing
//!   `1 / distance^power`; a point on the cell center gives its value.
//! - A variable radius takes the nearest `points` points (12), none farther than `max_distance`
//!   when that is set (a cell with none is NoData).
//! - A variable radius never reaches farther than the output grid's width plus its height, even
//!   with no maximum set (a point 1 m beyond that is ignored).
//! - A fixed radius takes the points within `distance` (default five cells); a cell with fewer than
//!   `min_points` widens the radius one cell size at a time until it has enough (with none and no
//!   minimum, the cell is NoData).

use rstar::{AABB, PointDistance, RTree, RTreeObject};

use crate::env::{Environment, Grid, apply_mask, grid_from};
use crate::{DataType, Raster};

/// A point and its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Which points a cell is interpolated from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchRadius {
    Variable {
        points: usize,
        max_distance: Option<f64>,
    },
    Fixed {
        distance: Option<f64>,
        min_points: usize,
    },
}

impl Default for SearchRadius {
    fn default() -> Self {
        SearchRadius::Variable {
            points: 12,
            max_distance: None,
        }
    }
}

struct Indexed {
    xy: [f64; 2],
    z: f64,
}

impl RTreeObject for Indexed {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.xy)
    }
}

impl PointDistance for Indexed {
    fn distance_2(&self, p: &[f64; 2]) -> f64 {
        (self.xy[0] - p[0]).powi(2) + (self.xy[1] - p[1]).powi(2)
    }
}

/// The grid ArcGIS makes for `samples` at `cell` (default: a 250th of the shorter side).
fn default_grid(samples: &[Sample], env: &Environment) -> Option<Grid> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in samples {
        x0 = x0.min(s.x);
        y0 = y0.min(s.y);
        x1 = x1.max(s.x);
        y1 = y1.max(s.y);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let extent = env.extent.unwrap_or([x0, y0, x1, y1]);
    let cell = env
        .cell_size
        .unwrap_or_else(|| (extent[2] - extent[0]).min(extent[3] - extent[1]) / 250.0);
    if !(cell.is_finite() && cell > 0.0) {
        return None;
    }
    let natural = [
        x0 - cell / 2.0,
        y0 - cell / 2.0,
        x0 - cell / 2.0 + ((w / cell).round() + 1.0) * cell,
        y0 - cell / 2.0 + ((h / cell).round() + 1.0) * cell,
    ];
    let natural = if env.snap.is_some() {
        [x0, y0, x1, y1]
    } else {
        natural
    };
    grid_from(Some(natural), Some(cell), env)
}

/// Inverse Distance Weighted interpolation of `samples` (those with a finite value) under `env`.
/// `None` when there are no points or the grid cannot be made.
pub fn idw(
    samples: &[Sample],
    power: f64,
    search: SearchRadius,
    env: &Environment,
    epsg: Option<u32>,
) -> Option<Raster> {
    let samples: Vec<Sample> = samples
        .iter()
        .copied()
        .filter(|s| s.x.is_finite() && s.y.is_finite() && s.z.is_finite())
        .collect();
    if samples.is_empty() {
        return None;
    }
    let grid = default_grid(&samples, env)?;
    let cell = grid.transform[1];
    let tree = RTree::bulk_load(
        samples
            .iter()
            .map(|s| Indexed {
                xy: [s.x, s.y],
                z: s.z,
            })
            .collect(),
    );
    let nodata = f32::MIN as f64;
    let mut out = Raster::new(grid.cols, grid.rows, DataType::F32, grid.transform);
    out.epsg = epsg;
    out.nodata = Some(nodata);
    let mut found: Vec<(f64, f64)> = Vec::new();
    for row in 0..grid.rows {
        let y = grid.transform[3] + grid.transform[5] * (row as f64 + 0.5);
        for col in 0..grid.cols {
            let x = grid.transform[0] + cell * (col as f64 + 0.5);
            found.clear();
            let q = [x, y];
            match search {
                SearchRadius::Variable {
                    points,
                    max_distance,
                } => {
                    let reach = (grid.cols + grid.rows) as f64 * cell;
                    let max2 = max_distance.map_or(reach, |d| d.min(reach)).powi(2);
                    for (p, d2) in tree
                        .nearest_neighbor_iter_with_distance_2(&q)
                        .take(points.max(1))
                    {
                        if d2 > max2 {
                            break;
                        }
                        found.push((d2.sqrt(), p.z));
                    }
                }
                SearchRadius::Fixed {
                    distance,
                    min_points,
                } => {
                    let mut radius = distance.unwrap_or(5.0 * cell);
                    let want = min_points.min(samples.len());
                    if want > 0 {
                        // The radius grows a cell at a time until the minimum is inside it.
                        if let Some((p, d2)) =
                            tree.nearest_neighbor_iter_with_distance_2(&q).nth(want - 1)
                        {
                            let _ = p;
                            let farthest = d2.sqrt();
                            if farthest > radius {
                                radius +=
                                    ((farthest - radius) / cell - 1e-9).ceil().max(0.0) * cell;
                            }
                        }
                    }
                    for (p, d2) in tree.nearest_neighbor_iter_with_distance_2(&q) {
                        if d2 > radius * radius {
                            break;
                        }
                        found.push((d2.sqrt(), p.z));
                    }
                }
            }
            let v = match found.iter().find(|(d, _)| *d == 0.0) {
                Some((_, z)) => Some(*z),
                None if found.is_empty() => None,
                None => {
                    let (num, den) = found.iter().fold((0.0, 0.0), |(n, d), (dist, z)| {
                        let w = 1.0 / dist.powf(power);
                        (n + w * z, d + w)
                    });
                    Some(num / den)
                }
            };
            out.bands[0][row * grid.cols + col] = v
                .filter(|v| v.is_finite())
                .map_or(nodata, |v| v as f32 as f64);
        }
    }
    if let Some(m) = &env.mask {
        apply_mask(&mut out, m);
    }
    Some(out)
}

/// Solves the square system `a x = b` by Gaussian elimination with partial pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for r in col + 1..n {
            let f = a[r][col] / a[col][col];
            let pivot_row = a[col].clone();
            for (x, p) in a[r][col..].iter_mut().zip(&pivot_row[col..]) {
                *x -= f * p;
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let tail: f64 = (r + 1..n).map(|c| a[r][c] * x[c]).sum();
        x[r] = (b[r] - tail) / a[r][r];
    }
    Some(x)
}

/// Trend (LINEAR): the least-squares polynomial surface of the given `order` (1 to 12) through
/// the points, on IDW's default grid (observed to be the same). `None` when the points do not
/// determine the surface.
pub fn trend(
    samples: &[Sample],
    order: usize,
    env: &Environment,
    epsg: Option<u32>,
) -> Option<Raster> {
    let samples: Vec<Sample> = samples
        .iter()
        .copied()
        .filter(|s| s.x.is_finite() && s.y.is_finite() && s.z.is_finite())
        .collect();
    let terms: Vec<(usize, usize)> = (0..=order)
        .flat_map(|i| (0..=order - i).map(move |j| (i, j)))
        .collect();
    if samples.len() < terms.len() {
        return None;
    }
    let grid = default_grid(&samples, env)?;
    // Centre and scale the coordinates so the normal equations stay well conditioned.
    let n = samples.len() as f64;
    let (cx, cy) = (
        samples.iter().map(|s| s.x).sum::<f64>() / n,
        samples.iter().map(|s| s.y).sum::<f64>() / n,
    );
    let scale = samples
        .iter()
        .map(|s| (s.x - cx).abs().max((s.y - cy).abs()))
        .fold(0.0, f64::max)
        .max(1e-9);
    let basis = |x: f64, y: f64| -> Vec<f64> {
        let (u, v) = ((x - cx) / scale, (y - cy) / scale);
        terms
            .iter()
            .map(|&(i, j)| u.powi(i as i32) * v.powi(j as i32))
            .collect()
    };
    let m = terms.len();
    let mut ata = vec![vec![0.0; m]; m];
    let mut atz = vec![0.0; m];
    for s in &samples {
        let row = basis(s.x, s.y);
        for i in 0..m {
            atz[i] += row[i] * s.z;
            for j in 0..m {
                ata[i][j] += row[i] * row[j];
            }
        }
    }
    let coefficients = solve(ata, atz)?;
    let nodata = f32::MIN as f64;
    let mut out = Raster::new(grid.cols, grid.rows, DataType::F32, grid.transform);
    out.epsg = epsg;
    out.nodata = Some(nodata);
    let cell = grid.transform[1];
    for row in 0..grid.rows {
        let y = grid.transform[3] + grid.transform[5] * (row as f64 + 0.5);
        for col in 0..grid.cols {
            let x = grid.transform[0] + cell * (col as f64 + 0.5);
            let v: f64 = basis(x, y)
                .iter()
                .zip(&coefficients)
                .map(|(b, c)| b * c)
                .sum();
            out.bands[0][row * grid.cols + col] = DataType::F32.store(v);
        }
    }
    Some(out)
}

/// A point-statistics neighborhood in map units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointNeighborhood {
    Circle(f64),
    Rectangle(f64, f64),
    Annulus(f64, f64),
}

/// Statistics of Point Statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointStatistic {
    Mean,
    Maximum,
    Minimum,
    Range,
    Std,
    Sum,
}

/// Point Statistics: for each cell the statistic of the values of the points inside the
/// neighborhood around its centre (NoData where there are none). The grid starts
/// `(w + h) / 798` below and left of the points' extent and has round((w + 2 off) / cell)
/// columns and round((h + 2 off) / cell) rows (observed).
pub fn point_statistics(
    samples: &[Sample],
    cell: f64,
    neighborhood: PointNeighborhood,
    stat: PointStatistic,
    epsg: Option<u32>,
) -> Option<Raster> {
    let samples: Vec<Sample> = samples
        .iter()
        .copied()
        .filter(|s| s.x.is_finite() && s.y.is_finite() && s.z.is_finite())
        .collect();
    if samples.is_empty() || cell.is_nan() || cell <= 0.0 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in &samples {
        x0 = x0.min(s.x);
        y0 = y0.min(s.y);
        x1 = x1.max(s.x);
        y1 = y1.max(s.y);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let offset = (w + h) / 798.0;
    let (gx, gy) = (x0 - offset, y0 - offset);
    // The extent grown by the offset on both sides, in whole cells (rounded).
    let cols = (((w + 2.0 * offset) / cell).round() as usize).max(1);
    let rows = (((h + 2.0 * offset) / cell).round() as usize).max(1);
    let transform = [gx, cell, 0.0, gy + rows as f64 * cell, 0.0, -cell];
    let nodata = f32::MIN as f64;
    let mut out = Raster::new(cols, rows, DataType::F32, transform);
    out.epsg = epsg;
    out.nodata = Some(nodata);
    for row in 0..rows {
        let y = transform[3] - (row as f64 + 0.5) * cell;
        for col in 0..cols {
            let x = gx + (col as f64 + 0.5) * cell;
            let vals: Vec<f64> = samples
                .iter()
                .filter(|s| {
                    let (dx, dy) = ((s.x - x).abs(), (s.y - y).abs());
                    match neighborhood {
                        PointNeighborhood::Circle(r) => dx.hypot(dy) <= r,
                        PointNeighborhood::Rectangle(a, b) => dx <= a / 2.0 && dy <= b / 2.0,
                        PointNeighborhood::Annulus(a, b) => {
                            let d = dx.hypot(dy);
                            d >= a && d <= b
                        }
                    }
                })
                .map(|s| s.z)
                .collect();
            let v = if vals.is_empty() {
                nodata
            } else {
                let n = vals.len() as f64;
                let sum: f64 = vals.iter().sum();
                let v = match stat {
                    PointStatistic::Sum => sum,
                    PointStatistic::Mean => sum / n,
                    PointStatistic::Maximum => vals.iter().copied().fold(f64::MIN, f64::max),
                    PointStatistic::Minimum => vals.iter().copied().fold(f64::MAX, f64::min),
                    PointStatistic::Range => {
                        vals.iter().copied().fold(f64::MIN, f64::max)
                            - vals.iter().copied().fold(f64::MAX, f64::min)
                    }
                    PointStatistic::Std => {
                        let mean = sum / n;
                        (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n).sqrt()
                    }
                };
                DataType::F32.store(v)
            };
            out.bands[0][row * cols + col] = v;
        }
    }
    Some(out)
}

/// Line Statistics: for each cell the statistic of the field values of the line parts within
/// `radius` of the cell centre, weighted by the length inside the circle. LENGTH is the sum of
/// value x length (the field is a weight, observed). The grid is IDW's default one (the extent
/// of the lines grown half a cell, round(w / cell) + 1 columns); NoData where no line reaches.
/// `integer` says the field is integral (statistics other than MEAN and LENGTH then give 32-bit
/// integers with NoData -2147483647, VARIETY 0 for none).
pub fn line_statistics(
    lines: &[(Vec<Vec<[f64; 2]>>, f64)],
    integer: bool,
    cell: f64,
    radius: f64,
    stat: &str,
    epsg: Option<u32>,
) -> Option<Raster> {
    let stat = stat.to_ascii_uppercase();
    if !cell.is_finite()
        || cell <= 0.0
        || !radius.is_finite()
        || radius <= 0.0
        || !matches!(
            stat.as_str(),
            "MEAN"
                | "MAXIMUM"
                | "MINIMUM"
                | "RANGE"
                | "LENGTH"
                | "VARIETY"
                | "MAJORITY"
                | "MINORITY"
        )
        || (!integer && matches!(stat.as_str(), "VARIETY" | "MAJORITY" | "MINORITY"))
    {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (parts, value) in lines {
        if !value.is_finite() {
            return None;
        }
        for p in parts.iter().flatten() {
            if !p[0].is_finite() || !p[1].is_finite() {
                return None;
            }
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (gx, gy) = (x0 - cell / 2.0, y0 - cell / 2.0);
    let cols = ((x1 - x0) / cell).round() + 1.0;
    let rows = ((y1 - y0) / cell).round() + 1.0;
    if !cols.is_finite() || !rows.is_finite() || cols * rows > crate::MAX_CELLS as f64 {
        return None;
    }
    let (cols, rows) = (cols as usize, rows as usize);
    let transform = [gx, cell, 0.0, gy + rows as f64 * cell, 0.0, -cell];
    let float_out = !integer || stat == "MEAN" || stat == "LENGTH";
    let (dt, nodata) = if float_out {
        (DataType::F32, f32::MIN as f64)
    } else {
        (DataType::I32, -2147483647.0)
    };
    let mut out = Raster::new(cols, rows, dt, transform);
    out.epsg = epsg;
    out.nodata = Some(nodata);
    // ArcGIS stores line vertices as single-precision offsets from the lower-left
    // grid origin before clipping. Keep output world coordinates in double precision.
    let lines: Vec<_> = lines
        .iter()
        .map(|(parts, value)| {
            let parts: Vec<Vec<[f64; 2]>> = parts
                .iter()
                .map(|part| {
                    part.iter()
                        .map(|p| [((p[0] - gx) as f32) as f64, ((p[1] - gy) as f32) as f64])
                        .collect()
                })
                .collect();
            (parts, *value)
        })
        .collect();
    // A zero-length contact still participates in extrema, variety and modes.
    // None means the segment never meets the disc.
    let inside = |a: [f64; 2], b: [f64; 2], c: [f64; 2], r: f64| -> Option<f64> {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len == 0.0 {
            return None;
        }
        let (fx, fy) = (a[0] - c[0], a[1] - c[1]);
        let (qa, qb, qc) = (
            dx * dx + dy * dy,
            2.0 * (fx * dx + fy * dy),
            fx * fx + fy * fy - r * r,
        );
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        let t0 = ((-qb - sq) / (2.0 * qa)).max(0.0);
        let t1 = ((-qb + sq) / (2.0 * qa)).min(1.0);
        (t1 >= t0).then_some((t1 - t0) * len)
    };
    for row in 0..rows {
        let y = (rows as f64 - row as f64 - 0.5) * cell;
        for col in 0..cols {
            let x = (col as f64 + 0.5) * cell;
            let mut found: Vec<(f64, f64)> = Vec::new(); // (value, length inside)
            for (parts, value) in &lines {
                let mut l = 0.0;
                let mut touches = false;
                for part in parts {
                    for w in part.windows(2) {
                        if let Some(length) = inside(w[0], w[1], [x, y], radius) {
                            touches = true;
                            l += length;
                        }
                    }
                }
                if touches {
                    found.push((*value, l));
                }
            }
            let v = if found.is_empty() {
                if stat == "VARIETY" && !float_out {
                    0.0
                } else {
                    nodata
                }
            } else {
                let total: f64 = found.iter().map(|f| f.1).sum();
                let max = found.iter().map(|f| f.0).fold(f64::MIN, f64::max);
                let min = found.iter().map(|f| f.0).fold(f64::MAX, f64::min);
                // Length per distinct value.
                let mut per: Vec<(f64, f64)> = Vec::new();
                for (val, l) in &found {
                    match per.iter_mut().find(|p| p.0 == *val) {
                        Some(p) => p.1 += l,
                        None => per.push((*val, *l)),
                    }
                }
                match stat.as_str() {
                    "LENGTH" => found.iter().map(|f| f.0 * f.1).sum(),
                    "MEAN" => {
                        if total > 0.0 {
                            found.iter().map(|f| f.0 * f.1).sum::<f64>() / total
                        } else {
                            nodata
                        }
                    }
                    "MAXIMUM" => max,
                    "MINIMUM" => min,
                    "RANGE" => max - min,
                    "VARIETY" => per.len() as f64,
                    "MAJORITY" => per
                        .iter()
                        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.total_cmp(&a.0)))
                        .map_or(nodata, |p| p.0),
                    "MINORITY" => per
                        .iter()
                        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)))
                        .map_or(nodata, |p| p.0),
                    _ => return None,
                }
            };
            out.bands[0][row * cols + col] = if v == nodata { v } else { dt.store(v) };
        }
    }
    Some(out)
}
