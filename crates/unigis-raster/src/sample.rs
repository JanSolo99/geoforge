//! Extract Values To Points: the raster value at point locations.
//!
//! Findings from ArcGIS Pro 3.7 (`evp_*` reference cases):
//! - Without INTERPOLATE a point is inside the raster when its column and row are in
//!   [0, width] and [0, height]; one exactly on a boundary between two cells takes
//!   the cell to its left (above). The value type is the raster's (integer, single or
//!   double).
//! - INTERPOLATE is bilinear between the four cell centers around the point (integer
//!   rasters give single precision). Taps with zero weight are ignored. With a NoData
//!   value in the raster the result is NoData when the cell containing the point
//!   (floor of column and row, in [0, width) and [0, height)) is NoData or outside;
//!   otherwise NoData and out-of-raster taps are dropped and the weights renormalized.
//!   Without a NoData value, taps beyond the edge count as 0, so a point up to half a
//!   cell outside still gets a (shrunken) value.
//! - Shapefile output holds -9999 for NoData when the raster has a NoData value, else 0.

use crate::Raster;

/// The value of band 0 at each point, `None` for NoData and for points outside.
pub fn extract_values(raster: &Raster, points: &[[f64; 2]], interpolate: bool) -> Vec<Option<f64>> {
    points
        .iter()
        .map(|p| extract_value(raster, *p, interpolate))
        .collect()
}

/// The value of band 0 at each point, `None` for NoData and for points outside.
fn extract_value(r: &Raster, p: [f64; 2], interpolate: bool) -> Option<f64> {
    let t = &r.transform;
    let c = (p[0] - t[0]) / t[1];
    let row = (p[1] - t[3]) / t[5];
    let (w, h) = (r.width as f64, r.height as f64);
    let at = |col: i64, row: i64| -> Option<f64> {
        (col >= 0 && row >= 0 && (col as usize) < r.width && (row as usize) < r.height)
            .then(|| r.bands[0][row as usize * r.width + col as usize])
    };
    if !interpolate {
        if !(c >= 0.0 && c <= w && row >= 0.0 && row <= h) {
            return None;
        }
        let v = at((c.ceil() as i64 - 1).max(0), (row.ceil() as i64 - 1).max(0))?;
        return (!r.is_nodata(v)).then_some(v);
    }
    if !c.is_finite() || !row.is_finite() {
        return None;
    }
    let has_nodata = r.nodata.is_some();
    if has_nodata {
        // The cell containing the point must be on the raster and have data.
        match at(c.floor() as i64, row.floor() as i64) {
            Some(v) if !r.is_nodata(v) => {}
            _ => return None,
        }
    }
    let (fx, fy) = (c - 0.5, row - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let taps = [
        (x0, y0, (1.0 - tx) * (1.0 - ty)),
        (x0 + 1, y0, tx * (1.0 - ty)),
        (x0, y0 + 1, (1.0 - tx) * ty),
        (x0 + 1, y0 + 1, tx * ty),
    ];
    let (mut sum, mut weight) = (0.0, 0.0);
    for (col, row, wt) in taps {
        if wt <= 0.0 {
            continue;
        }
        match at(col, row) {
            Some(v) if has_nodata && r.is_nodata(v) => {}
            Some(v) => {
                sum += v * wt;
                weight += wt;
            }
            None => {}
        }
    }
    if has_nodata {
        (weight > 0.0).then(|| sum / weight)
    } else {
        Some(sum)
    }
}
