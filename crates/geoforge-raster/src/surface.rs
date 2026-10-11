//! Spatial Analyst surface tools: Slope, Aspect and Hillshade (planar method).
//!
//! The gradient is Horn's 3×3 estimate as ArcGIS computes it: the center and its
//! four edge neighbors must have values (otherwise, and on the raster's border,
//! the output is NoData); a missing corner cell drops out of its column's or row's
//! weighted mean, so dz/dx = (mean of right column − mean of left column) / 2Δx with
//! weights 1, 2, 1.

use crate::{DataType, Raster};

/// Output NoData of float results: ArcGIS writes −FLT_MAX when the input has no
/// NoData and 3.4e38 when it has.
fn float_nodata(input: &Raster) -> f64 {
    if input.nodata.is_some() {
        3.4e38_f32 as f64
    } else {
        f32::MIN as f64
    }
}

/// Hillshade's NoData (the output is 8-bit unsigned).
const HILLSHADE_NODATA: f64 = 255.0;

/// Slope measurement unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlopeUnit {
    Degree,
    PercentRise,
}

/// (dz/dx, dz/dy) at a cell, x to the east and y down the rows (south), or `None`
/// where ArcGIS writes NoData. With `all_neighbors` (Hillshade) a missing corner
/// also gives NoData.
fn gradient(r: &Raster, col: usize, row: usize, all_neighbors: bool) -> Option<(f64, f64)> {
    if col == 0 || row == 0 || col + 1 >= r.width || row + 1 >= r.height {
        return None;
    }
    let at =
        |dx: isize, dy: isize| r.get((col as isize + dx) as usize, (row as isize + dy) as usize);
    at(0, 0)?;
    let (n, s, w, east) = (at(0, -1)?, at(0, 1)?, at(-1, 0)?, at(1, 0)?);
    let (nw, ne, sw, se) = (at(-1, -1), at(1, -1), at(-1, 1), at(1, 1));
    if all_neighbors && [nw, ne, sw, se].iter().any(Option::is_none) {
        return None;
    }
    let mean = |corner1: Option<f64>, middle: f64, corner2: Option<f64>| {
        let mut sum = 2.0 * middle;
        let mut weight = 2.0;
        for v in [corner1, corner2].into_iter().flatten() {
            sum += v;
            weight += 1.0;
        }
        sum / weight
    };
    let dzdx = (mean(ne, east, se) - mean(nw, w, sw)) / (2.0 * r.cell_width());
    let dzdy = (mean(sw, s, se) - mean(nw, n, ne)) / (2.0 * r.cell_height());
    Some((dzdx, dzdy))
}

fn map_cells(
    input: &Raster,
    data_type: DataType,
    nodata: f64,
    all_neighbors: bool,
    f: impl Fn(f64, f64) -> f64,
) -> Raster {
    let mut out = input.like(data_type);
    out.nodata = Some(nodata);
    for row in 0..input.height {
        for col in 0..input.width {
            out.bands[0][row * input.width + col] = match gradient(input, col, row, all_neighbors) {
                Some((dx, dy)) => data_type.store(f(dx, dy)),
                None => nodata,
            };
        }
    }
    out
}

/// Spatial Analyst Slope (planar): degrees or percent rise, 32-bit float.
pub fn slope(dem: &Raster, unit: SlopeUnit, z_factor: f64) -> Raster {
    map_cells(dem, DataType::F32, float_nodata(dem), false, |dx, dy| {
        let rise = z_factor * dx.hypot(dy);
        match unit {
            SlopeUnit::Degree => rise.atan().to_degrees(),
            SlopeUnit::PercentRise => 100.0 * rise,
        }
    })
}

/// Downslope direction in degrees clockwise from north (0–360); −1 on flat cells.
fn compass_aspect(dx: f64, dy: f64) -> f64 {
    if dx == 0.0 && dy == 0.0 {
        return -1.0;
    }
    let a = dy.atan2(-dx).to_degrees();
    if a < 0.0 {
        90.0 - a
    } else if a > 90.0 {
        450.0 - a
    } else {
        90.0 - a
    }
}

/// Spatial Analyst Aspect (planar), 32-bit float.
pub fn aspect(dem: &Raster) -> Raster {
    map_cells(dem, DataType::F32, float_nodata(dem), false, compass_aspect)
}

/// Spatial Analyst Hillshade: 0–254 illumination, 8-bit unsigned with NoData 255.
/// Values are truncated, as ArcGIS does. With `shadows`, cells whose view of the
/// sun is blocked are 0: the ray toward the sun is followed one cell at a time,
/// sampling the surface bilinearly (approximates ArcGIS; a few cells within a
/// few centimeters of the horizon differ).
pub fn hillshade_with(
    dem: &Raster,
    azimuth: f64,
    altitude: f64,
    z_factor: f64,
    shadows: bool,
) -> Raster {
    let mut out = hillshade(dem, azimuth, altitude, z_factor);
    if !shadows {
        return out;
    }
    let (w, h) = (dem.width, dem.height);
    let (dx, dy) = (azimuth.to_radians().sin(), -azimuth.to_radians().cos());
    let rise = altitude.to_radians().tan() * dem.cell_width();
    // Outside the raster: None; NoData under the ray: Some(None), skipped.
    let sample = |fx: f64, fy: f64| -> Option<Option<f64>> {
        if fx < 0.0 || fy < 0.0 || fx > (w - 1) as f64 || fy > (h - 1) as f64 {
            return None;
        }
        let (x0, y0) = ((fx as usize).min(w - 2), (fy as usize).min(h - 2));
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let mut sum = 0.0;
        for (x, y, wt) in [
            (x0, y0, (1.0 - tx) * (1.0 - ty)),
            (x0 + 1, y0, tx * (1.0 - ty)),
            (x0, y0 + 1, (1.0 - tx) * ty),
            (x0 + 1, y0 + 1, tx * ty),
        ] {
            match dem.get(x, y) {
                Some(v) => sum += wt * v,
                None => return Some(None),
            }
        }
        Some(Some(sum * z_factor))
    };
    for row in 0..h {
        for col in 0..w {
            let i = row * w + col;
            if out.bands[0][i] == HILLSHADE_NODATA || out.bands[0][i] == 0.0 {
                continue;
            }
            let Some(z0) = dem.get(col, row) else {
                continue;
            };
            let z0 = z0 * z_factor;
            let mut k = 1.0;
            while let Some(v) = sample(col as f64 + dx * k, row as f64 + dy * k) {
                if v.is_some_and(|v| v > z0 + k * rise) {
                    out.bands[0][i] = 0.0;
                    break;
                }
                k += 1.0;
            }
        }
    }
    out
}

/// Hillshade without shadows (see [`hillshade_with`]).
pub fn hillshade(dem: &Raster, azimuth: f64, altitude: f64, z_factor: f64) -> Raster {
    let zenith = (90.0 - altitude).to_radians();
    let azimuth_math = (360.0 - azimuth + 90.0).rem_euclid(360.0).to_radians();
    let (cz, sz) = (zenith.cos(), zenith.sin());
    map_cells(dem, DataType::U8, HILLSHADE_NODATA, true, |dx, dy| {
        let (dx, dy) = (dx * z_factor, dy * z_factor);
        let slope = dx.hypot(dy).atan();
        let aspect = dy.atan2(-dx);
        let v = 255.0 * (cz * slope.cos() + sz * slope.sin() * (azimuth_math - aspect).cos());
        v.floor().clamp(0.0, 254.0)
    })
}

/// Spatial Analyst Curvature (Zevenbergen and Thorne): (curvature, profile, plan),
/// each in hundredths of a z-unit per square cell unit. Neighbors that are NoData
/// or beyond the edge take the center value, so only NoData cells are NoData.
pub fn curvature(dem: &Raster, z_factor: f64) -> (Raster, Raster, Raster) {
    // Observed: −FLT_MAX for float input without NoData, otherwise 3.4e38.
    let nodata = if dem.data_type.is_float() && dem.nodata.is_none() {
        f32::MIN as f64
    } else {
        3.4e38_f32 as f64
    };
    let mut outs = [
        dem.like(DataType::F32),
        dem.like(DataType::F32),
        dem.like(DataType::F32),
    ];
    for o in &mut outs {
        o.nodata = Some(nodata);
    }
    let (l, w, h) = (dem.cell_width(), dem.width as isize, dem.height as isize);
    for row in 0..dem.height {
        for col in 0..dem.width {
            let i = row * dem.width + col;
            let Some(z5) = dem.get(col, row) else {
                for o in &mut outs {
                    o.bands[0][i] = nodata;
                }
                continue;
            };
            let at = |dx: isize, dy: isize| {
                let (x, y) = (col as isize + dx, row as isize + dy);
                if x < 0 || y < 0 || x >= w || y >= h {
                    return z5 * z_factor;
                }
                dem.get(x as usize, y as usize).unwrap_or(z5) * z_factor
            };
            let z5 = z5 * z_factor;
            let (z1, z2, z3) = (at(-1, -1), at(0, -1), at(1, -1));
            let (z4, z6) = (at(-1, 0), at(1, 0));
            let (z7, z8, z9) = (at(-1, 1), at(0, 1), at(1, 1));
            let d = ((z4 + z6) / 2.0 - z5) / (l * l);
            let e = ((z2 + z8) / 2.0 - z5) / (l * l);
            let f = (-z1 + z3 + z7 - z9) / (4.0 * l * l);
            let g = (-z4 + z6) / (2.0 * l);
            let hh = (z2 - z8) / (2.0 * l);
            let s = g * g + hh * hh;
            let (profile, plan) = if s == 0.0 {
                (0.0, 0.0)
            } else {
                (
                    2.0 * (d * g * g + e * hh * hh + f * g * hh) / s * 100.0,
                    -2.0 * (d * hh * hh + e * g * g - f * g * hh) / s * 100.0,
                )
            };
            for (o, v) in outs.iter_mut().zip([-2.0 * (d + e) * 100.0, profile, plan]) {
                o.bands[0][i] = DataType::F32.store(v);
            }
        }
    }
    let [c, p, q] = outs;
    (c, p, q)
}
