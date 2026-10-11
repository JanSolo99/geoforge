//! Spatial Analyst hydrology: Fill, Flow Direction (D8) and Flow Accumulation.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::{DataType, Raster};

/// D8 codes clockwise from east, with their (dx, dy) offsets.
pub const D8: [(u8, isize, isize); 8] = [
    (1, 1, 0),
    (2, 1, 1),
    (4, 0, 1),
    (8, -1, 1),
    (16, -1, 0),
    (32, -1, -1),
    (64, 0, -1),
    (128, 1, -1),
];

/// Output NoData of float results whose input had none (Fill, Flow Accumulation).
const FLOAT_NODATA: f64 = 3.4e38_f32 as f64;

#[derive(Clone, Copy, PartialEq)]
struct Key(f64);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

fn neighbors(r: &Raster, i: usize) -> impl Iterator<Item = (u8, usize, bool)> + '_ {
    let (x, y) = ((i % r.width) as isize, (i / r.width) as isize);
    D8.iter().filter_map(move |&(code, dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        if nx < 0 || ny < 0 || nx >= r.width as isize || ny >= r.height as isize {
            return None;
        }
        Some((
            code,
            ny as usize * r.width + nx as usize,
            dx != 0 && dy != 0,
        ))
    })
}

fn value(r: &Raster, i: usize) -> Option<f64> {
    let v = r.bands[0][i];
    (!r.is_nodata(v)).then_some(v)
}

/// Spatial Analyst Fill: raises every sink to its spill elevation (priority flood
/// from the raster edge and NoData). With `z_limit`, a sink deeper than the limit
/// (spill elevation − its lowest cell) is left as it is.
pub fn fill(dem: &Raster, z_limit: Option<f64>) -> Raster {
    let n = dem.width * dem.height;
    let mut out = dem.clone();
    out.bands.truncate(1);
    if dem.nodata.is_none() {
        out.nodata = Some(if dem.data_type.is_float() {
            FLOAT_NODATA
        } else {
            crate::focal::type_max(dem.data_type)
        });
    }
    let z = &dem.bands[0];
    let mut filled = z.clone();
    let mut done = vec![false; n];
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        if value(dem, i).is_none() {
            done[i] = true;
            continue;
        }
        let (x, y) = (i % dem.width, i / dem.width);
        let edge = x == 0 || y == 0 || x + 1 == dem.width || y + 1 == dem.height;
        if edge || neighbors(dem, i).any(|(_, j, _)| value(dem, j).is_none()) {
            done[i] = true;
            heap.push(Reverse((Key(z[i]), i)));
        }
    }
    // Cells raised by the flood, grouped into sinks by their spill elevation.
    let mut raised = Vec::new();
    while let Some(Reverse((Key(level), i))) = heap.pop() {
        for (_, j, _) in neighbors(dem, i) {
            if done[j] {
                continue;
            }
            done[j] = true;
            if filled[j] < level {
                filled[j] = level;
                raised.push(j);
            }
            heap.push(Reverse((Key(filled[j]), j)));
        }
    }
    if let Some(limit) = z_limit {
        // Connected groups of raised cells at one spill level form a sink.
        let mut seen = vec![false; n];
        let mut is_raised = vec![false; n];
        for &i in &raised {
            is_raised[i] = true;
        }
        for &start in &raised {
            if seen[start] {
                continue;
            }
            let level = filled[start];
            let mut group = vec![start];
            seen[start] = true;
            let mut k = 0;
            while k < group.len() {
                let i = group[k];
                k += 1;
                for (_, j, _) in neighbors(dem, i) {
                    if is_raised[j] && !seen[j] && filled[j] == level {
                        seen[j] = true;
                        group.push(j);
                    }
                }
            }
            let bottom = group.iter().map(|&i| z[i]).fold(f64::INFINITY, f64::min);
            if level - bottom > limit {
                for &i in &group {
                    filled[i] = z[i];
                }
            }
        }
    }
    out.bands[0] = filled.into_iter().map(|v| dem.data_type.store(v)).collect();
    out
}

/// Picks one direction among tied steepest descents as ArcGIS does: of two, the
/// larger code; of more, the middle one of the shortest arc covering them.
fn pick(codes: &[u8]) -> u8 {
    match codes.len() {
        0 => 0,
        1 => codes[0],
        2 => codes[0].max(codes[1]),
        _ => {
            let mut idx: Vec<usize> = codes.iter().map(|c| c.trailing_zeros() as usize).collect();
            idx.sort_unstable();
            let k = idx.len();
            // The arc starts after the largest gap between consecutive directions.
            let start = (0..k)
                .max_by_key(|&i| ((idx[(i + 1) % k] + 8 - idx[i]) % 8, i))
                .map_or(0, |i| (i + 1) % k);
            1 << idx[(start + k / 2) % k]
        }
    }
}

/// Spatial Analyst Flow Direction, D8. Edge cells flow outward unless (with
/// `force_edges` false) they have a steeper inward descent. Flat cells look two
/// cells out, then drain through level neighbors that already have a direction.
/// ArcGIS's flat resolution is only approximated: large flats (integer DEMs)
/// differ in places.
pub fn flow_direction(dem: &Raster, force_edges: bool) -> Raster {
    let (w, h) = (dem.width, dem.height);
    let n = w * h;
    let mut out = dem.like(DataType::U8);
    out.nodata = Some(0.0);
    let dirs = &mut out.bands[0];
    let mut z: Vec<Option<f64>> = (0..n).map(|i| value(dem, i)).collect();
    // A single cell lower than all its neighbors is noise: raise it to the
    // lowest neighbor first.
    for i in 0..n {
        let Some(v) = z[i] else { continue };
        let (x, y) = (i % w, i / w);
        if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
            continue;
        }
        let around: Vec<Option<f64>> = neighbors(dem, i).map(|(_, j, _)| z[j]).collect();
        if around.iter().all(|a| a.is_some_and(|a| a > v)) {
            z[i] = around.into_iter().flatten().reduce(f64::min);
        }
    }
    let outward = |x: usize, y: usize| -> u8 {
        match (x == 0, x + 1 == w, y == 0, y + 1 == h) {
            (true, _, true, _) => 32,
            (_, true, true, _) => 128,
            (true, _, _, true) => 8,
            (_, true, _, true) => 2,
            (true, ..) => 16,
            (_, true, ..) => 1,
            (_, _, true, _) => 64,
            _ => 4,
        }
    };
    let mut unresolved = Vec::new();
    for i in 0..n {
        let Some(v) = z[i] else {
            dirs[i] = 0.0;
            continue;
        };
        let (x, y) = (i % w, i / w);
        let edge = x == 0 || y == 0 || x + 1 == w || y + 1 == h;
        let mut best = 0.0;
        let mut codes: Vec<u8> = Vec::new();
        for (code, j, diagonal) in neighbors(dem, i) {
            let Some(nv) = z[j] else { continue };
            let drop = (v - nv)
                / if diagonal {
                    std::f64::consts::SQRT_2
                } else {
                    1.0
                };
            if drop > best {
                best = drop;
                codes.clear();
                codes.push(code);
            } else if drop == best && best > 0.0 {
                codes.push(code);
            }
        }
        dirs[i] = if edge && (force_edges || codes.is_empty()) {
            outward(x, y) as f64
        } else if codes.is_empty() {
            // On a flat, look two cells out along each direction that stays level.
            let mut best = 0.0;
            let mut far: Vec<u8> = Vec::new();
            for &(code, dx, dy) in &D8 {
                let (x1, y1) = (x as isize + dx, y as isize + dy);
                let (x2, y2) = (x as isize + 2 * dx, y as isize + 2 * dy);
                if x2 < 0 || y2 < 0 || x2 >= w as isize || y2 >= h as isize {
                    continue;
                }
                if z[y1 as usize * w + x1 as usize] != Some(v) {
                    continue;
                }
                let Some(v2) = z[y2 as usize * w + x2 as usize] else {
                    continue;
                };
                let run = if dx != 0 && dy != 0 {
                    2.0 * std::f64::consts::SQRT_2
                } else {
                    2.0
                };
                let drop = (v - v2) / run;
                if drop > best {
                    best = drop;
                    far.clear();
                    far.push(code);
                } else if drop == best && best > 0.0 {
                    far.push(code);
                }
            }
            if far.is_empty() {
                unresolved.push(i);
                0.0
            } else {
                pick(&far) as f64
            }
        } else {
            pick(&codes) as f64
        };
    }
    // Remaining flat cells drain, pass by pass, into level neighbors that already
    // have a direction (Jenson and Domingue), preferring edge neighbors to corners.
    loop {
        let before = dirs.clone();
        let mut still = Vec::new();
        for &i in &unresolved {
            let codes: Vec<u8> = neighbors(dem, i)
                .filter(|&(_, j, _)| z[j] == z[i] && before[j] > 0.0)
                .map(|(c, _, _)| c)
                .collect();
            let straight: Vec<u8> = codes
                .iter()
                .copied()
                .filter(|c| matches!(c, 1 | 4 | 16 | 64))
                .collect();
            if codes.is_empty() {
                still.push(i);
            } else if straight.is_empty() {
                dirs[i] = pick(&codes) as f64;
            } else {
                dirs[i] = pick(&straight) as f64;
            }
        }
        if still.len() == unresolved.len() {
            break;
        }
        unresolved = still;
    }
    // Cells in sinks that drain nowhere: the sum of their lowest-neighbor codes.
    for &i in &unresolved {
        let v = z[i].unwrap_or(0.0);
        let codes: u8 = neighbors(dem, i)
            .filter(|&(_, j, _)| z[j] == Some(v))
            .fold(0, |acc, (c, _, _)| acc | c);
        dirs[i] = codes as f64;
    }
    out
}

/// Spatial Analyst Flow Accumulation (D8): the number (or the summed weight) of
/// cells flowing into each cell. `integer` gives a 32-bit integer raster.
pub fn flow_accumulation(directions: &Raster, weight: Option<&Raster>, integer: bool) -> Raster {
    let (w, h) = (directions.width, directions.height);
    let n = w * h;
    let target = |i: usize| -> Option<usize> {
        let code = value(directions, i)? as u8;
        let &(_, dx, dy) = D8.iter().find(|d| d.0 == code)?;
        let (x, y) = ((i % w) as isize + dx, (i / w) as isize + dy);
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            return None;
        }
        let j = y as usize * w + x as usize;
        value(directions, j).map(|_| j)
    };
    let downstream: Vec<Option<usize>> = (0..n).map(target).collect();
    let mut inflow = vec![0u32; n];
    for j in downstream.iter().flatten() {
        inflow[*j] += 1;
    }
    let cell_weight = |i: usize| match weight {
        Some(wr) => value(wr, i).unwrap_or(0.0),
        None => 1.0,
    };
    let mut acc = vec![0.0f64; n];
    let mut stack: Vec<usize> = (0..n).filter(|&i| inflow[i] == 0).collect();
    while let Some(i) = stack.pop() {
        if let Some(j) = downstream[i] {
            acc[j] += acc[i] + cell_weight(i);
            inflow[j] -= 1;
            if inflow[j] == 0 {
                stack.push(j);
            }
        }
    }
    let (data_type, nodata) = if integer {
        (DataType::I32, i32::MAX as f64)
    } else {
        (DataType::F32, FLOAT_NODATA)
    };
    let mut out = directions.like(data_type);
    out.nodata = Some(nodata);
    for (i, (cell, a)) in out.bands[0].iter_mut().zip(&acc).enumerate() {
        *cell = if value(directions, i).is_some() {
            data_type.store(*a)
        } else {
            nodata
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::pick;

    #[test]
    fn tie_breaking() {
        assert_eq!(pick(&[1, 4]), 4);
        assert_eq!(pick(&[4, 16]), 16);
        assert_eq!(pick(&[1, 64]), 64);
        assert_eq!(pick(&[1, 4, 16]), 4);
        assert_eq!(pick(&[1, 4, 64]), 1);
    }
}
