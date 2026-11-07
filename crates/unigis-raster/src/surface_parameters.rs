//! Spatial Analyst Surface Parameters: slope, aspect and curvatures from a local surface fitted
//! geodesically to each cell's 3×3 neighbourhood.
//!
//! As ArcGIS Pro 3.7 computes it (black-box probes, `scripts/research/surface_parameters`):
//! the cell centres become geocentric (ECEF) points on the ellipsoid, with the cell value as
//! ellipsoidal height, and are expressed in the centre cell's local east/north/up frame. The
//! biquadratic surface interpolates the nine points; the quadratic one is a weighted least
//! squares fit with weights `1 / (1 + x² + y²)²` (metres). The parameters follow Minár et al.
//! (2020) from the derivatives at the centre.

use crate::{DataType, Raster};

/// The parameter Surface Parameters writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceParameter {
    Slope,
    Aspect,
    Mean,
    Profile,
    Tangential,
    Contour,
    ContourGeodesicTorsion,
    Gaussian,
    Casorati,
}

impl SurfaceParameter {
    /// From ArcGIS's `parameter_type` keyword.
    pub fn from_keyword(k: &str) -> Option<Self> {
        Some(match k.to_ascii_uppercase().as_str() {
            "SLOPE" => Self::Slope,
            "ASPECT" => Self::Aspect,
            "MEAN_CURVATURE" => Self::Mean,
            "PROFILE_CURVATURE" => Self::Profile,
            "TANGENTIAL_CURVATURE" => Self::Tangential,
            "CONTOUR_CURVATURE" => Self::Contour,
            "CONTOUR_GEODESIC_TORSION" => Self::ContourGeodesicTorsion,
            "GAUSSIAN_CURVATURE" => Self::Gaussian,
            "CASORATI_CURVATURE" => Self::Casorati,
            _ => return None,
        })
    }
}

/// How the parameters are computed.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceOptions {
    pub parameter: SurfaceParameter,
    /// Biquadratic (exact) rather than quadratic (weighted least squares) surface.
    pub biquadratic: bool,
    /// Metres per z unit.
    pub z_metres: f64,
    /// Slope in percent rise rather than degrees.
    pub percent_rise: bool,
}

/// The ellipsoid the raster's coordinates refer to.
#[derive(Debug, Clone, Copy)]
pub struct Ellipsoid {
    pub semi_major: f64,
    /// Inverse flattening; 0 for a sphere.
    pub inverse_flattening: f64,
}

impl Ellipsoid {
    fn e2(&self) -> f64 {
        if self.inverse_flattening == 0.0 {
            0.0
        } else {
            let f = 1.0 / self.inverse_flattening;
            f * (2.0 - f)
        }
    }
}

/// Solves `a x = b` (n×n, row-major) by Gaussian elimination with partial pivoting.
/// A system whose pivot falls below 1e-10 of its largest entry counts as singular.
fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    let scale = a.iter().flatten().fold(0.0_f64, |m, v| m.max(v.abs()));
    for k in 0..N {
        let p = (k..N).max_by(|&i, &j| a[i][k].abs().total_cmp(&a[j][k].abs()))?;
        // NaN pivots count as singular too.
        if a[p][k].abs().is_nan() || a[p][k].abs() <= 1e-10 * scale {
            return None;
        }
        a.swap(k, p);
        b.swap(k, p);
        let pivot = a[k];
        for i in k + 1..N {
            let m = a[i][k] / pivot[k];
            for (v, p) in a[i][k..].iter_mut().zip(&pivot[k..]) {
                *v -= m * p;
            }
            b[i] -= m * b[k];
        }
    }
    let mut x = [0.0; N];
    for k in (0..N).rev() {
        let s: f64 = (k + 1..N).map(|j| a[k][j] * x[j]).sum();
        x[k] = (b[k] - s) / a[k][k];
    }
    Some(x)
}

/// The derivatives (r, t, s, p, q) = (z_xx, z_yy, z_xy, z_x, z_y) at the centre of a surface
/// through the local points `(x, y, z)` of the 3×3 window (`None`: NoData there). The
/// biquadratic surface needs all nine; the quadratic one is fitted to those present.
fn derivatives(pts: &[Option<[f64; 3]>; 9], biquadratic: bool) -> Option<[f64; 5]> {
    if biquadratic {
        let mut a = [[0.0; 9]; 9];
        let mut b = [0.0; 9];
        for (k, p) in pts.iter().enumerate() {
            let [x, y, z] = (*p)?;
            a[k] = [
                x * x * y * y,
                x * x * y,
                x * y * y,
                x * x,
                y * y,
                x * y,
                x,
                y,
                1.0,
            ];
            b[k] = z;
        }
        let c = solve(a, b)?;
        return Some([2.0 * c[3], 2.0 * c[4], c[5], c[6], c[7]]);
    }
    let mut n = [[0.0; 6]; 6];
    let mut rhs = [0.0; 6];
    for &[x, y, z] in pts.iter().flatten() {
        let d = 1.0 + x * x + y * y;
        let w = 1.0 / (d * d);
        let row = [x * x, y * y, x * y, x, y, 1.0];
        for i in 0..6 {
            for j in 0..6 {
                n[i][j] += w * row[i] * row[j];
            }
            rhs[i] += w * row[i] * z;
        }
    }
    let c = solve(n, rhs)?;
    Some([2.0 * c[0], 2.0 * c[1], c[2], c[3], c[4]])
}

/// The parameter from the derivatives (Minár et al. 2020; signs as ArcGIS writes them).
fn parameter(kind: SurfaceParameter, percent_rise: bool, [r, t, s, p, q]: [f64; 5]) -> f64 {
    let g = p * p + q * q;
    let w = 1.0 + g;
    let mean = || -((1.0 + q * q) * r - 2.0 * p * q * s + (1.0 + p * p) * t) / (2.0 * w.powf(1.5));
    let gaussian = || (r * t - s * s) / (w * w);
    let contour = q * q * r - 2.0 * p * q * s + p * p * t;
    // Directional curvatures are undefined on flat cells; 0 is written there (unverified).
    let flat = g == 0.0;
    match kind {
        SurfaceParameter::Slope if percent_rise => 100.0 * g.sqrt(),
        SurfaceParameter::Slope => g.sqrt().atan().to_degrees(),
        SurfaceParameter::Aspect if flat => -1.0,
        SurfaceParameter::Aspect => (-p).atan2(-q).to_degrees().rem_euclid(360.0),
        _ if flat
            && matches!(
                kind,
                SurfaceParameter::Profile
                    | SurfaceParameter::Tangential
                    | SurfaceParameter::Contour
                    | SurfaceParameter::ContourGeodesicTorsion
            ) =>
        {
            0.0
        }
        SurfaceParameter::Profile => -(p * p * r + 2.0 * p * q * s + q * q * t) / (g * w.powf(1.5)),
        SurfaceParameter::Tangential => -contour / (g * w.sqrt()),
        SurfaceParameter::Contour => -contour / g.powf(1.5),
        SurfaceParameter::ContourGeodesicTorsion => {
            (p * q * (r - t) - (p * p - q * q) * s) / (g * w)
        }
        SurfaceParameter::Mean => mean(),
        SurfaceParameter::Gaussian => gaussian(),
        SurfaceParameter::Casorati => (2.0 * mean().powi(2) - gaussian()).max(0.0).sqrt(),
    }
}

/// Surface Parameters with the default (one-cell) fixed neighbourhood, as 32-bit float.
/// `lonlat` holds each cell centre's geodetic longitude and latitude in degrees (row-major).
/// The one-cell border is NoData; so are cells whose centre or east neighbour is NoData, cells
/// with any NoData neighbour on the biquadratic surface, and fits without a solution (a whole
/// side of the window missing).
pub fn surface_parameters(
    dem: &Raster,
    lonlat: &[[f64; 2]],
    ellipsoid: Ellipsoid,
    opts: SurfaceOptions,
) -> Raster {
    let (w, h) = (dem.width, dem.height);
    let nodata = if dem.nodata.is_some() {
        3.4e38_f32 as f64
    } else {
        f32::MIN as f64
    };
    let mut out = dem.like(DataType::F32);
    out.nodata = Some(nodata);
    let e2 = ellipsoid.e2();
    let a = ellipsoid.semi_major;
    let ecef = |i: usize, z: f64| {
        let (lon, lat) = (lonlat[i][0].to_radians(), lonlat[i][1].to_radians());
        let (sp, cp) = lat.sin_cos();
        let n = a / (1.0 - e2 * sp * sp).sqrt();
        let hgt = z * opts.z_metres;
        [
            (n + hgt) * cp * lon.cos(),
            (n + hgt) * cp * lon.sin(),
            (n * (1.0 - e2) + hgt) * sp,
        ]
    };
    for row in 0..h {
        for col in 0..w {
            let i = row * w + col;
            out.bands[0][i] = nodata;
            if col == 0 || row == 0 || col + 1 >= w || row + 1 >= h {
                continue;
            }
            let window: [Option<[f64; 3]>; 9] = std::array::from_fn(|k| {
                let (c, r) = (col + k % 3 - 1, row + k / 3 - 1);
                dem.get(c, r).map(|z| ecef(r * w + c, z))
            });
            // The fit's x axis runs toward the east neighbour, so the centre and that
            // neighbour must have values.
            let (Some(centre), Some(east_cell)) = (window[4], window[5]) else {
                continue;
            };
            let (lon, lat) = (lonlat[i][0].to_radians(), lonlat[i][1].to_radians());
            let (sp, cp) = lat.sin_cos();
            let (sl, cl) = lon.sin_cos();
            let up = [cp * cl, cp * sl, sp];
            let true_east = [-sl, cl, 0.0];
            let true_north = [-sp * cl, -sp * sl, cp];
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let v = sub(east_cell, centre);
            let along = dot(v, up);
            let v = [
                v[0] - along * up[0],
                v[1] - along * up[1],
                v[2] - along * up[2],
            ];
            let len = dot(v, v).sqrt();
            let x_axis = [v[0] / len, v[1] / len, v[2] / len];
            let y_axis = [
                up[1] * x_axis[2] - up[2] * x_axis[1],
                up[2] * x_axis[0] - up[0] * x_axis[2],
                up[0] * x_axis[1] - up[1] * x_axis[0],
            ];
            let pts = window.map(|q| {
                q.map(|q| {
                    let d = sub(q, centre);
                    [dot(d, x_axis), dot(d, y_axis), dot(d, up)]
                })
            });
            let Some(mut d) = derivatives(&pts, opts.biquadratic) else {
                continue;
            };
            if opts.parameter == SurfaceParameter::Aspect {
                // Aspect is measured from true north: the gradient in the east/north frame.
                let (gx, gy) = (d[3], d[4]);
                let g = [
                    gx * x_axis[0] + gy * y_axis[0],
                    gx * x_axis[1] + gy * y_axis[1],
                    gx * x_axis[2] + gy * y_axis[2],
                ];
                d[3] = dot(g, true_east);
                d[4] = dot(g, true_north);
            }
            out.bands[0][i] = DataType::F32.store(parameter(opts.parameter, opts.percent_rise, d));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a plane the fitted gradient is the plane's, whatever the surface type.
    #[test]
    fn plane_gradient() {
        let pts: [Option<[f64; 3]>; 9] = std::array::from_fn(|k| {
            let (x, y) = ((k % 3) as f64 * 10.0 - 10.0, 10.0 - (k / 3) as f64 * 10.0);
            Some([x, y, 0.1 * x - 0.2 * y])
        });
        for bi in [false, true] {
            let [r, t, s, p, q] = derivatives(&pts, bi).unwrap();
            assert!((p - 0.1).abs() < 1e-12 && (q + 0.2).abs() < 1e-12);
            assert!(r.abs() < 1e-12 && t.abs() < 1e-12 && s.abs() < 1e-12);
        }
        // z falls to the east: an east-facing (90°) cell; flat cells are −1.
        let east = parameter(SurfaceParameter::Aspect, false, [0.0, 0.0, 0.0, -1.0, 0.0]);
        assert_eq!(east, 90.0);
        assert_eq!(parameter(SurfaceParameter::Aspect, false, [0.0; 5]), -1.0);
    }
}
