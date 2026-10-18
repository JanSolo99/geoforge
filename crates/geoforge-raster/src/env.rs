//! Spatial Analyst environments: cell size, extent, snap raster and mask (`arcpy.env`).
//!
//! Observed in ArcGIS Pro 3.7 through the bridge (`testdata/arcgis-reference/raster_env.json`):
//! - The processing grid is anchored at the lower-left corner of the extent (the environment's, else
//!   the inputs' default), not at the inputs' grid, and has `round(extent / cell)` columns and rows.
//! - A snap raster moves the extent outward onto its lattice (the lower-left corner down, the
//!   upper-right up, in the snap raster's cell size); the cell size stays the environment's or the
//!   inputs'.
//! - Inputs are sampled at the output cell centers, nearest (a center on a cell edge takes the cell
//!   to its right/below). Slope at a coarser cell size reads the DEM resampled bilinearly.
//! - Tools read their input in one of two ways: most crop it to the processing extent first (a
//!   focal statistic or hillshade has no neighbors beyond it), while Slope, Aspect, Curvature and
//!   Flow Direction read one more cell around it.
//! - The mask does not change the extent: cells outside it (NoData in a mask raster, centers outside
//!   the mask polygons) are NoData.

use geoforge_core::Geometry;

use crate::Raster;

/// A snap raster's lattice: the corner of one of its cells and its cell size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snap {
    pub x: f64,
    pub y: f64,
    pub cell_w: f64,
    pub cell_h: f64,
}

impl Snap {
    pub fn of(r: &Raster) -> Snap {
        Snap {
            x: r.transform[0],
            y: r.transform[3],
            cell_w: r.cell_width(),
            cell_h: r.cell_height(),
        }
    }
}

/// The processing mask.
#[derive(Debug, Clone)]
pub enum Mask {
    /// Cells where this raster has data (it is sampled at the output cell centers).
    Raster(Raster),
    /// Cells whose centers lie inside these polygons (coordinates in the rasters' system).
    Polygons(Vec<Geometry>),
}

/// The environments that shape a raster tool's output. All unset: the tool's own defaults.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    /// Output cell size (square cells).
    pub cell_size: Option<f64>,
    /// `[xmin, ymin, xmax, ymax]`.
    pub extent: Option<[f64; 4]>,
    pub snap: Option<Snap>,
    pub mask: Option<Mask>,
}

impl Environment {
    pub fn is_empty(&self) -> bool {
        self.cell_size.is_none()
            && self.extent.is_none()
            && self.snap.is_none()
            && self.mask.is_none()
    }
}

/// A north-up grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub transform: [f64; 6],
    pub cols: usize,
    pub rows: usize,
}

impl Grid {
    fn cell(&self) -> f64 {
        self.transform[1]
    }

    /// `[xmin, ymin, xmax, ymax]`.
    pub fn extent(&self) -> [f64; 4] {
        let t = &self.transform;
        [
            t[0],
            t[3] + t[5] * self.rows as f64,
            t[0] + t[1] * self.cols as f64,
            t[3],
        ]
    }

    /// This grid with `n` more cells on every side.
    fn padded(&self, n: usize) -> Grid {
        let t = &self.transform;
        let k = n as f64;
        Grid {
            transform: [t[0] - k * t[1], t[1], 0.0, t[3] - k * t[5], 0.0, t[5]],
            cols: self.cols + 2 * n,
            rows: self.rows + 2 * n,
        }
    }
}

/// How a tool treats input beyond the processing extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reads {
    /// The input is cropped to the extent first: no neighbors beyond it.
    Cropped,
    /// The tool reads one cell around the extent (Slope, Aspect, Curvature, Flow Direction).
    Context,
}

/// How inputs are sampled when the cell size changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampling {
    Nearest,
    Bilinear,
}

const EPS: f64 = 1e-9;

fn lattice_floor(v: f64, origin: f64, step: f64) -> f64 {
    origin + ((v - origin) / step + EPS).floor() * step
}

fn lattice_ceil(v: f64, origin: f64, step: f64) -> f64 {
    origin + ((v - origin) / step - EPS).ceil() * step
}

/// The processing grid for `inputs` under `env`; `None` when the inputs do not overlap and no
/// extent is set, or the grid would have more than [`crate::MAX_CELLS`] cells.
pub fn processing_grid(inputs: &[&Raster], env: &Environment) -> Option<Grid> {
    let (default, d_cols, d_rows) =
        crate::local::local_grid(inputs).map_or((None, 0, 0), |(t, c, r)| (Some(t), c, r));
    let default_extent = default.map(|t| {
        [
            t[0],
            t[3] + t[5] * d_rows as f64,
            t[0] + t[1] * d_cols as f64,
            t[3],
        ]
    });
    grid_from(default_extent, default.map(|t| t[1]), env)
}

/// The processing grid when the tool's own defaults are `default_extent` and `default_cell` (those of
/// its inputs, or of whatever it reads that is not a raster) and `env` may override them.
pub fn grid_from(
    default_extent: Option<[f64; 4]>,
    default_cell: Option<f64>,
    env: &Environment,
) -> Option<Grid> {
    let [mut x0, mut y0, mut x1, mut y1] = env.extent.or(default_extent)?;
    let cell = env.cell_size.or(default_cell)?;
    if cell <= 0.0 || x1 <= x0 || y1 <= y0 {
        return None;
    }
    if let Some(s) = env.snap {
        x0 = lattice_floor(x0, s.x, s.cell_w);
        x1 = lattice_ceil(x1, s.x, s.cell_w);
        // Rows count down from the snap corner: the lattice in y has the same spacing.
        y0 = lattice_floor(y0, s.y, s.cell_h);
        y1 = lattice_ceil(y1, s.y, s.cell_h);
    }
    let (cols_f, rows_f) = (
        ((x1 - x0) / cell).round().max(1.0),
        ((y1 - y0) / cell).round().max(1.0),
    );
    // Also rejects NaN and absurd sizes before they become indices.
    if cols_f.is_nan() || rows_f.is_nan() || cols_f * rows_f > crate::MAX_CELLS as f64 {
        return None;
    }
    let (cols, rows) = (cols_f as usize, rows_f as usize);
    Some(Grid {
        transform: [x0, cell, 0.0, y0 + rows as f64 * cell, 0.0, -cell],
        cols,
        rows,
    })
}

/// `r` sampled at the cell centers of `grid`, bilinearly (taps outside the raster or on NoData are
/// dropped and the remaining weights renormalized).
pub fn bilinear_onto(r: &Raster, grid: &Grid) -> Raster {
    let nodata = r.nodata.unwrap_or(if r.data_type.is_float() {
        f32::MIN as f64
    } else {
        crate::focal::type_max(r.data_type)
    });
    let mut out = Raster::new(grid.cols, grid.rows, r.data_type, grid.transform);
    out.epsg = r.epsg;
    out.nodata = Some(nodata);
    out.statistics = r.statistics;
    let t = &r.transform;
    let at = |x: isize, y: isize| -> Option<f64> {
        if x < 0 || y < 0 || x >= r.width as isize || y >= r.height as isize {
            None
        } else {
            r.get(x as usize, y as usize)
        }
    };
    for row in 0..grid.rows {
        let y = grid.transform[3] + grid.transform[5] * (row as f64 + 0.5);
        let gy = (y - t[3]) / t[5] - 0.5;
        for col in 0..grid.cols {
            let x = grid.transform[0] + grid.transform[1] * (col as f64 + 0.5);
            let gx = (x - t[0]) / t[1] - 0.5;
            let (x0, y0) = (gx.floor() as isize, gy.floor() as isize);
            let (mut sum, mut wsum) = (0.0, 0.0);
            for m in 0..2 {
                for n in 0..2 {
                    if let Some(v) = at(x0 + n, y0 + m) {
                        let w = (1.0 - (gx - (x0 + n) as f64).abs()).max(0.0)
                            * (1.0 - (gy - (y0 + m) as f64).abs()).max(0.0);
                        sum += w * v;
                        wsum += w;
                    }
                }
            }
            out.bands[0][row * grid.cols + col] = if wsum > 0.0 {
                r.data_type.store(sum / wsum)
            } else {
                nodata
            };
        }
    }
    out
}

fn onto(r: &Raster, grid: &Grid, sampling: Sampling) -> Raster {
    // The same grid is a copy; a different cell size with bilinear sampling interpolates; all else
    // is nearest.
    if (r.transform, r.width, r.height) == (grid.transform, grid.cols, grid.rows) {
        return r.clone();
    }
    let finer_or_coarser = (r.cell_width() - grid.cell()).abs() > 1e-9 * grid.cell();
    if sampling == Sampling::Bilinear && finer_or_coarser {
        bilinear_onto(r, grid)
    } else {
        crate::resample::onto_grid(r, grid.transform, grid.cols, grid.rows)
    }
}

/// Whether the point is inside any polygon, its boundary included (even-odd over all rings).
fn inside(polygons: &[Geometry], x: f64, y: f64) -> bool {
    polygons.iter().any(|g| {
        let mut odd = false;
        for range in g.part_ranges() {
            let ring = &g.xy[range];
            let mut j = ring.len().wrapping_sub(1);
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[j]);
                // On the edge: inside.
                let cross = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
                let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                if cross.abs() <= 1e-9 * len.max(1.0)
                    && x >= a[0].min(b[0]) - 1e-9
                    && x <= a[0].max(b[0]) + 1e-9
                    && y >= a[1].min(b[1]) - 1e-9
                    && y <= a[1].max(b[1]) + 1e-9
                {
                    return true;
                }
                if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]
                {
                    odd = !odd;
                }
                j = i;
            }
        }
        odd
    })
}

/// `out` with the cells outside the mask set to NoData.
pub fn apply_mask(out: &mut Raster, mask: &Mask) {
    let nodata = out.nodata.unwrap_or_else(|| {
        let nd = if out.data_type.is_float() {
            f32::MIN as f64
        } else {
            crate::focal::type_max(out.data_type)
        };
        out.nodata = Some(nd);
        nd
    });
    let grid = Grid {
        transform: out.transform,
        cols: out.width,
        rows: out.height,
    };
    let keep: Vec<bool> = match mask {
        Mask::Raster(m) => {
            let m = crate::resample::onto_grid(m, grid.transform, grid.cols, grid.rows);
            m.bands[0].iter().map(|v| !m.is_nodata(*v)).collect()
        }
        Mask::Polygons(polys) => (0..grid.rows)
            .flat_map(|row| (0..grid.cols).map(move |col| (row, col)))
            .map(|(row, col)| {
                let x = grid.transform[0] + grid.transform[1] * (col as f64 + 0.5);
                let y = grid.transform[3] + grid.transform[5] * (row as f64 + 0.5);
                inside(polys, x, y)
            })
            .collect(),
    };
    for band in &mut out.bands {
        for (v, k) in band.iter_mut().zip(&keep) {
            if !k {
                *v = nodata;
            }
        }
    }
}

/// Runs a raster tool under `env`: the inputs go onto the processing grid, `f` computes on them, and
/// the result is cropped back (for tools that read a cell of context) and masked. With no
/// environment set, `f` gets the inputs as they are.
pub fn run(
    inputs: &[&Raster],
    env: &Environment,
    reads: Reads,
    sampling: Sampling,
    f: &dyn Fn(&[Raster]) -> Raster,
) -> Option<Raster> {
    if env.is_empty() {
        let owned: Vec<Raster> = inputs.iter().map(|r| (*r).clone()).collect();
        return Some(f(&owned));
    }
    let grid = processing_grid(inputs, env)?;
    let work = if reads == Reads::Context {
        grid.padded(1)
    } else {
        grid
    };
    let prepared: Vec<Raster> = inputs.iter().map(|r| onto(r, &work, sampling)).collect();
    let mut out = f(&prepared);
    if reads == Reads::Context {
        let (w, h) = (out.width, out.height);
        let mut cropped = Raster::new(grid.cols, grid.rows, out.data_type, grid.transform);
        cropped.epsg = out.epsg;
        cropped.nodata = out.nodata;
        cropped.statistics = out.statistics;
        cropped.bands = out
            .bands
            .iter()
            .map(|b| {
                let mut v = Vec::with_capacity(grid.cols * grid.rows);
                for row in 1..=grid.rows {
                    for col in 1..=grid.cols {
                        v.push(if row < h && col < w {
                            b[row * w + col]
                        } else {
                            out.nodata.unwrap_or(0.0)
                        });
                    }
                }
                v
            })
            .collect();
        out = cropped;
    }
    if let Some(m) = &env.mask {
        apply_mask(&mut out, m);
    }
    Some(out)
}
