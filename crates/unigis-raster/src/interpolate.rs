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
