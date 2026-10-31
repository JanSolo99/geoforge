//! Spatial Analyst Multiscale Surface Deviation: the largest deviation from the mean elevation
//! (DEV = (z − mean) / standard deviation) over a range of square neighbourhoods, and the
//! neighbourhood distance at which it was found (Lindsay et al. 2015, integral images).
//!
//! Matched against ArcGIS Pro 3.7: neighbourhoods are clipped at the raster edge and skip NoData;
//! the variance is the population variance from window sums of z and of z² where each square is
//! rounded to single precision (as ArcGIS does); distances are rounded half up to whole cells;
//! scale k after the minimum n₀ is n₀ + ⌊(Δn·k)^p⌋ cells, up to the maximum.

use crate::{DataType, Raster};

/// Neighbourhood distances (in cells) from the minimum, maximum and base increment (in cells,
/// already rounded) and the nonlinearity factor.
pub fn scales(min: usize, max: usize, increment: usize, nonlinearity: f64) -> Vec<usize> {
    let mut out = vec![min];
    for k in 1.. {
        let n = min + (((increment * k) as f64).powf(nonlinearity) + 1e-9).floor() as usize;
        if n > max {
            break;
        }
        if n > *out.last().unwrap() {
            out.push(n);
        }
    }
    out
}

fn integral(w: usize, h: usize, value: impl Fn(usize) -> f64) -> Vec<f64> {
    let mut out = vec![0.0; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0.0;
        for x in 0..w {
            row += value(y * w + x);
            out[(y + 1) * (w + 1) + x + 1] = out[y * (w + 1) + x + 1] + row;
        }
    }
    out
}

/// (deviation, scale): Float32 rasters, NoData where the input is NoData. A scale of `k` cells
/// is written as `k * cell_value` (the neighbourhood distance in the chosen units).
pub fn multiscale_surface_deviation(
    dem: &Raster,
    scales: &[usize],
    cell_value: f64,
) -> (Raster, Raster) {
    let (w, h) = (dem.width, dem.height);
    let valid: Vec<Option<f64>> = (0..w * h)
        .map(|i| {
            let v = dem.bands[0][i];
            (!dem.is_nodata(v)).then_some(v)
        })
        .collect();
    let sum = integral(w, h, |i| valid[i].unwrap_or(0.0));
    let squares = integral(w, h, |i| {
        valid[i].map_or(0.0, |v| {
            let v = v as f32;
            (v * v) as f64
        })
    });
    let count = integral(w, h, |i| f64::from(u8::from(valid[i].is_some())));
    let box_sum = |img: &[f64], y0: usize, y1: usize, x0: usize, x1: usize| {
        let s = w + 1;
        img[y1 * s + x1] - img[y0 * s + x1] - img[y1 * s + x0] + img[y0 * s + x0]
    };
    let mut best = vec![0.0f64; w * h];
    let mut at = vec![scales.first().copied().unwrap_or(0); w * h];
    for &r in scales {
        for y in 0..h {
            let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
            for x in 0..w {
                let i = y * w + x;
                let Some(z) = valid[i] else { continue };
                let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
                let n = box_sum(&count, y0, y1, x0, x1);
                let mean = box_sum(&sum, y0, y1, x0, x1) / n;
                let variance = box_sum(&squares, y0, y1, x0, x1) / n - mean * mean;
                let dev = (z - mean) / variance.sqrt();
                if dev.is_finite() && dev.abs() > best[i].abs() {
                    best[i] = dev;
                    at[i] = r;
                }
            }
        }
    }
    // ArcGIS tags the deviation raster with 3.4e38 and the scale raster with −f32::MAX.
    let nodata = 3.4e38f32 as f64;
    let mut deviation = dem.like(DataType::F32);
    deviation.nodata = Some(nodata);
    let mut scale = dem.like(DataType::F32);
    scale.nodata = Some(f32::MIN as f64);
    for (i, v) in valid.iter().enumerate() {
        (deviation.bands[0][i], scale.bands[0][i]) = if v.is_some() {
            (
                best[i] as f32 as f64,
                (at[i] as f64 * cell_value) as f32 as f64,
            )
        } else {
            (nodata, f32::MIN as f64)
        };
    }
    (deviation, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_sequences_follow_arcgis() {
        assert_eq!(scales(4, 13, 1, 1.0), (4..=13).collect::<Vec<_>>());
        assert_eq!(scales(2, 20, 1, 1.5), [2, 3, 4, 7, 10, 13, 16, 20]);
        assert_eq!(scales(1, 30, 1, 2.0), [1, 2, 5, 10, 17, 26]);
        assert_eq!(scales(3, 15, 2, 1.0), [3, 5, 7, 9, 11, 13, 15]);
        assert_eq!(scales(2, 20, 1, 1.3), [2, 3, 4, 6, 8, 10, 12, 14, 16, 19]);
    }
}
