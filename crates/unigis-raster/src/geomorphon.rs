//! Spatial Analyst Geomorphon Landforms (Jasiewicz & Stepinski 2013).
//!
//! Each cell looks along the 8 compass lines up to the search distance. In each direction the
//! largest and smallest elevation angles (ψ max, ψ min) of the cells past the skip distance
//! give the sign: `max + min` above the flatness threshold is "higher" (+), below its negative
//! "lower" (−), otherwise flat. The counts of + and − pick one of ten landforms; the ternary
//! pattern itself, reduced over rotations and reflections, is the geomorphon code.
//!
//! Matched against ArcGIS Pro 3.7: lines stop at the raster edge and at NoData (a direction
//! with no cell is flat); a cell is skipped while its distance is below the skip distance;
//! with a skip distance a border of cells is NoData. In CELLS units the line lengths count
//! cells (a diagonal step is √2 cells); in linear units they are ground distances. Rasters in
//! geographic coordinates use a sphere of radius 6 371 000 m (east-west scaled by the cosine
//! of the cell's latitude).

use crate::{DataType, Raster};

/// Landform codes as ArcGIS writes them (1 Flat … 10 Pit).
pub const LANDFORMS: [&str; 10] = [
    "Flat",
    "Peak",
    "Ridge",
    "Shoulder",
    "Spur",
    "Slope",
    "Hollow",
    "Footslope",
    "Valley",
    "Pit",
];

/// Landform for (number of − directions, number of + directions).
const FORMS: [[i8; 9]; 9] = {
    const FL: i8 = 1;
    const PK: i8 = 2;
    const RI: i8 = 3;
    const SH: i8 = 4;
    const SP: i8 = 5;
    const SL: i8 = 6;
    const HL: i8 = 7;
    const FS: i8 = 8;
    const VL: i8 = 9;
    const PT: i8 = 10;
    const NA: i8 = 0;
    [
        [FL, FL, FL, FS, FS, VL, VL, VL, PT],
        [FL, FL, FS, FS, FS, VL, VL, VL, NA],
        [FL, SH, SL, SL, HL, HL, VL, NA, NA],
        [SH, SH, SL, SL, SL, HL, NA, NA, NA],
        [SH, SH, SP, SL, SL, NA, NA, NA, NA],
        [RI, RI, SP, SP, NA, NA, NA, NA, NA],
        [RI, RI, RI, NA, NA, NA, NA, NA, NA],
        [RI, RI, NA, NA, NA, NA, NA, NA, NA],
        [PK, NA, NA, NA, NA, NA, NA, NA, NA],
    ]
};

/// N, NE, E, SE, S, SW, W, NW as (row, column) steps.
const DIRECTIONS: [(isize, isize); 8] = [
    (-1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
];

/// Search and skip distances, in cells or in ground units (metres).
#[derive(Debug, Clone, Copy)]
pub enum Distance {
    Cells(f64),
    Ground(f64),
}

pub struct Parameters {
    /// Flatness threshold in degrees.
    pub angle_threshold: f64,
    pub search: Distance,
    pub skip: Distance,
    /// Metres per elevation unit.
    pub z_factor: f64,
}

/// Smallest base-3 reading (− = 0, flat = 1, + = 2) over the 8 rotations of the pattern and
/// of its reflection.
fn code(pattern: &[u8; 8]) -> i16 {
    let mut best = i16::MAX;
    for reflect in [false, true] {
        for start in 0..8 {
            let mut v = 0i16;
            for k in 0..8 {
                let i = if reflect { 15 - start - k } else { start + k } % 8;
                v = v * 3 + pattern[i] as i16;
            }
            best = best.min(v);
        }
    }
    best
}

const EPS: f64 = 1e-9;

/// (landforms, geomorphons): Int8 (NoData −128) and Int16 (NoData −32768).
pub fn geomorphon_landforms(dem: &Raster, p: &Parameters) -> (Raster, Raster) {
    let (w, h) = (dem.width, dem.height);
    let t = dem.transform;
    let geographic = dem
        .epsg
        .is_some_and(|e| crate::geographic_crs::GEOGRAPHIC_EPSG.contains(&e));
    let (cx, cy) = (t[1].abs(), t[5].abs());
    // Ground size of one cell step east-west and north-south on `row`.
    let cell = |row: usize| -> (f64, f64) {
        if geographic {
            let r = 6_371_000.0;
            let lat = (t[3] + t[5] * (row as f64 + 0.5)).to_radians();
            (r * cx.to_radians() * lat.cos(), r * cy.to_radians())
        } else {
            (cx, cy)
        }
    };
    let threshold = p.angle_threshold.to_radians();
    // NoData border with a skip distance: ArcGIS rounds a cell count half down and a ground
    // distance up (a distance up to half a cell gives none).
    let border = match p.skip {
        Distance::Cells(c) => (c - 0.5 - EPS).ceil().max(0.0) as usize,
        Distance::Ground(d) => {
            let (ew, ns) = cell(h / 2);
            let c = d / ew.min(ns);
            if c <= 0.5 + EPS {
                0
            } else {
                (c - EPS).ceil() as usize
            }
        }
    };
    let mut land = dem.like(DataType::I8);
    land.nodata = Some(-128.0);
    let mut geo = dem.like(DataType::I16);
    geo.nodata = Some(-32768.0);
    for y in 0..h {
        let (ew, ns) = cell(y);
        for x in 0..w {
            let i = y * w + x;
            let Some(z) = dem
                .get(x, y)
                .filter(|_| y.min(x).min(h - 1 - y).min(w - 1 - x) >= border)
            else {
                land.bands[0][i] = -128.0;
                geo.bands[0][i] = -32768.0;
                continue;
            };
            let mut pattern = [1u8; 8];
            for (d, &(dy, dx)) in DIRECTIONS.iter().enumerate() {
                let step = (dx as f64 * ew).hypot(dy as f64 * ns);
                let cells = (dx as f64).hypot(dy as f64);
                let (mut high, mut low) = (f64::NEG_INFINITY, f64::INFINITY);
                for k in 1.. {
                    let (along, ground) = (k as f64 * cells, k as f64 * step);
                    let (reach, skipped) = match (p.search, p.skip) {
                        (Distance::Cells(s), Distance::Cells(k0)) => {
                            (along <= s + EPS, along < k0 - EPS)
                        }
                        (Distance::Ground(s), Distance::Ground(k0)) => {
                            (ground <= s + EPS, ground < k0 - EPS)
                        }
                        (Distance::Cells(s), Distance::Ground(k0)) => {
                            (along <= s + EPS, ground < k0 - EPS)
                        }
                        (Distance::Ground(s), Distance::Cells(k0)) => {
                            (ground <= s + EPS, along < k0 - EPS)
                        }
                    };
                    if !reach {
                        break;
                    }
                    let (yy, xx) = (y as isize + dy * k, x as isize + dx * k);
                    if yy < 0 || xx < 0 || yy >= h as isize || xx >= w as isize {
                        break;
                    }
                    let Some(v) = dem.get(xx as usize, yy as usize) else {
                        break;
                    };
                    if skipped {
                        continue;
                    }
                    let angle = ((v - z) * p.z_factor / ground).atan();
                    high = high.max(angle);
                    low = low.min(angle);
                }
                if high.is_finite() {
                    let sum = high + low;
                    pattern[d] = if sum > threshold {
                        2
                    } else if sum < -threshold {
                        0
                    } else {
                        1
                    };
                }
            }
            let minus = pattern.iter().filter(|&&s| s == 0).count();
            let plus = pattern.iter().filter(|&&s| s == 2).count();
            land.bands[0][i] = FORMS[minus][plus] as f64;
            geo.bands[0][i] = code(&pattern) as f64;
        }
    }
    if geographic {
        crate::geographic_crs::recompute_cell_size(&mut land);
        crate::geographic_crs::recompute_cell_size(&mut geo);
    }
    (land, geo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_rotation_and_reflection_minimal() {
        assert_eq!(code(&[0; 8]), 0);
        assert_eq!(code(&[1; 8]), 3280);
        // 1 flat, 7 higher: 12222222 in base 3.
        assert_eq!(code(&[2, 2, 2, 1, 2, 2, 2, 2]), 4373);
        assert_eq!(code(&[2, 2, 2, 2, 0, 0, 0, 0]), 80);
    }
}
