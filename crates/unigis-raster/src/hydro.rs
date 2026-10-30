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

/// Downstream neighbor of every cell along valid D8 codes, or `None` at outlets
/// (flow off the raster, into NoData, or an undefined direction).
fn downstream_cells(directions: &Raster) -> Vec<Option<usize>> {
    let (w, h) = (directions.width, directions.height);
    (0..w * h)
        .map(|i| {
            let code = value(directions, i)? as u8;
            let &(_, dx, dy) = D8.iter().find(|d| d.0 == code)?;
            let (x, y) = ((i % w) as isize + dx, (i / w) as isize + dy);
            if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                return None;
            }
            let j = y as usize * w + x as usize;
            value(directions, j).map(|_| j)
        })
        .collect()
}

/// Spatial Analyst Sink: the cells that drain only into each other (flow directions forming a
/// loop), numbered from 1 in raster order of each loop's first cell; every other cell is NoData.
pub fn sink(directions: &Raster) -> Raster {
    let down = downstream_cells(directions);
    let n = down.len();
    // 0 = unseen, 1 = on the current path, 2 = done.
    let mut state = vec![0u8; n];
    let mut id = vec![0u32; n];
    let mut next = 0;
    for start in 0..n {
        let mut path = Vec::new();
        let mut at = Some(start);
        while let Some(i) = at {
            if state[i] != 0 {
                break;
            }
            state[i] = 1;
            path.push(i);
            at = down[i];
        }
        if let Some(i) = at.filter(|&i| state[i] == 1) {
            // `i` is on this path: the loop runs from it to the path's end.
            next += 1;
            let from = path.iter().position(|&p| p == i).unwrap_or(0);
            for &p in &path[from..] {
                id[p] = next;
            }
        }
        for p in path {
            state[p] = 2;
        }
    }
    let nodata = -2_147_483_647.0;
    let mut out = directions.like(crate::DataType::I32);
    out.nodata = Some(nodata);
    out.bands[0] = id
        .iter()
        .map(|&v| if v == 0 { nodata } else { f64::from(v) })
        .collect();
    out
}

/// Cells in an order where every cell comes after all cells flowing into it.
fn upstream_first(down: &[Option<usize>], valid: &[bool]) -> Vec<usize> {
    let mut inflow = vec![0u32; down.len()];
    for j in down.iter().flatten() {
        inflow[*j] += 1;
    }
    let mut order = Vec::with_capacity(down.len());
    let mut stack: Vec<usize> = (0..down.len())
        .filter(|&i| valid[i] && inflow[i] == 0)
        .collect();
    while let Some(i) = stack.pop() {
        order.push(i);
        if let Some(j) = down[i] {
            inflow[j] -= 1;
            if inflow[j] == 0 {
                stack.push(j);
            }
        }
    }
    order
}

fn int_result(like: &Raster, data_type: DataType, nodata: f64, values: Vec<Option<f64>>) -> Raster {
    let mut out = like.like(data_type);
    out.nodata = Some(nodata);
    out.bands[0] = values.into_iter().map(|v| v.unwrap_or(nodata)).collect();
    out
}

/// Spatial Analyst Basin: each cell labeled by the outlet it drains to. Outlet
/// cells that touch (8-connected) form one basin; basins are numbered in the
/// raster order of their first cell.
pub fn basin(directions: &Raster) -> Raster {
    let n = directions.width * directions.height;
    let down = downstream_cells(directions);
    let valid: Vec<bool> = (0..n).map(|i| value(directions, i).is_some()).collect();
    let outlet: Vec<bool> = (0..n).map(|i| valid[i] && down[i].is_none()).collect();
    // Group touching outlets.
    let mut group: Vec<Option<usize>> = vec![None; n];
    let mut groups = 0;
    for i in 0..n {
        if !outlet[i] || group[i].is_some() {
            continue;
        }
        group[i] = Some(groups);
        let mut stack = vec![i];
        while let Some(c) = stack.pop() {
            for (_, j, _) in neighbors(directions, c) {
                if outlet[j] && group[j].is_none() {
                    group[j] = Some(groups);
                    stack.push(j);
                }
            }
        }
        groups += 1;
    }
    let order = upstream_first(&down, &valid);
    for &i in order.iter().rev() {
        if let Some(j) = down[i] {
            group[i] = group[j];
        }
    }
    let mut number: Vec<Option<f64>> = vec![None; groups];
    let mut next = 0.0;
    let label = group
        .iter()
        .map(|g| {
            let g = (*g)?;
            Some(*number[g].get_or_insert_with(|| {
                next += 1.0;
                next
            }))
        })
        .collect();
    int_result(directions, DataType::U16, 32767.0, label)
}

/// Spatial Analyst Watershed: each cell labeled with the value of the first pour
/// point cell at or below it; cells that reach none are NoData.
pub fn watershed(directions: &Raster, pour_points: &Raster) -> Raster {
    let n = directions.width * directions.height;
    let down = downstream_cells(directions);
    let valid: Vec<bool> = (0..n).map(|i| value(directions, i).is_some()).collect();
    let mut label: Vec<Option<f64>> = (0..n)
        .map(|i| {
            if valid[i] {
                value(pour_points, i)
            } else {
                None
            }
        })
        .collect();
    let order = upstream_first(&down, &valid);
    for &i in order.iter().rev() {
        if label[i].is_none()
            && let Some(j) = down[i]
        {
            label[i] = label[j];
        }
    }
    let max = label.iter().flatten().copied().fold(0.0, f64::max);
    let (t, nd) = if max < 255.0 {
        (DataType::U8, 255.0)
    } else if max < 65535.0 {
        (DataType::U16, 65535.0)
    } else {
        (DataType::I32, i32::MAX as f64)
    };
    int_result(directions, t, nd, label)
}

/// Stream cells (non-zero, non-NoData) and their stream-only downstream links.
fn stream_network(streams: &Raster, directions: &Raster) -> (Vec<bool>, Vec<Option<usize>>) {
    let n = directions.width * directions.height;
    let is_stream: Vec<bool> = (0..n)
        .map(|i| value(streams, i).is_some_and(|v| v != 0.0) && value(directions, i).is_some())
        .collect();
    let down = downstream_cells(directions)
        .into_iter()
        .enumerate()
        .map(|(i, d)| d.filter(|&j| is_stream[i] && is_stream[j]))
        .collect();
    (is_stream, down)
}

/// Spatial Analyst Stream Link: a unique value for each stretch of stream between
/// junctions, numbered in raster order of the stretches' upstream cells.
pub fn stream_link(streams: &Raster, directions: &Raster) -> Raster {
    let n = directions.width * directions.height;
    let (is_stream, down) = stream_network(streams, directions);
    let mut inflow = vec![0u32; n];
    for j in down.iter().flatten() {
        inflow[*j] += 1;
    }
    let mut label: Vec<Option<f64>> = vec![None; n];
    let mut next = 0.0;
    for i in 0..n {
        // A stretch starts at a source or just below a junction.
        let head = is_stream[i] && inflow[i] != 1;
        if head {
            next += 1.0;
            let mut cur = i;
            loop {
                label[cur] = Some(next);
                match down[cur] {
                    Some(j) if inflow[j] == 1 => cur = j,
                    _ => break,
                }
            }
        }
    }
    int_result(directions, DataType::I32, i32::MAX as f64, label)
}

/// Stream ordering method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOrder {
    Strahler,
    Shreve,
}

/// Spatial Analyst Stream Order.
pub fn stream_order(streams: &Raster, directions: &Raster, method: StreamOrder) -> Raster {
    let n = directions.width * directions.height;
    let (is_stream, down) = stream_network(streams, directions);
    let order = upstream_first(&down, &is_stream);
    let mut ups: Vec<Vec<f64>> = vec![Vec::new(); n];
    let mut result: Vec<Option<f64>> = vec![None; n];
    for &i in &order {
        let v = if ups[i].is_empty() {
            1.0
        } else {
            match method {
                StreamOrder::Shreve => ups[i].iter().sum(),
                StreamOrder::Strahler => {
                    let max = ups[i].iter().copied().fold(0.0, f64::max);
                    if ups[i].iter().filter(|&&u| u == max).count() > 1 {
                        max + 1.0
                    } else {
                        max
                    }
                }
            }
        };
        result[i] = Some(v);
        if let Some(j) = down[i] {
            ups[j].push(v);
        }
    }
    int_result(directions, DataType::U16, 65535.0, result)
}

/// Spatial Analyst Flow Length (unweighted): downstream distance to the outlet or
/// the longest upstream path, along D8 steps of one cell or one diagonal.
pub fn flow_length(directions: &Raster, upstream: bool) -> Raster {
    let (w, n) = (directions.width, directions.width * directions.height);
    let down = downstream_cells(directions);
    let valid: Vec<bool> = (0..n).map(|i| value(directions, i).is_some()).collect();
    let step = |i: usize, j: usize| {
        let diagonal = i % w != j % w && i / w != j / w;
        if diagonal {
            directions.cell_width().hypot(directions.cell_height())
        } else if i % w != j % w {
            directions.cell_width()
        } else {
            directions.cell_height()
        }
    };
    let order = upstream_first(&down, &valid);
    let mut len = vec![0.0f64; n];
    if upstream {
        for &i in &order {
            if let Some(j) = down[i] {
                len[j] = len[j].max(len[i] + step(i, j));
            }
        }
    } else {
        for &i in order.iter().rev() {
            if let Some(j) = down[i] {
                len[i] = len[j] + step(i, j);
            }
        }
    }
    let mut out = directions.like(DataType::F32);
    let nodata = f32::MIN as f64;
    out.nodata = Some(nodata);
    out.bands[0] = (0..n)
        .map(|i| {
            if valid[i] {
                DataType::F32.store(len[i])
            } else {
                nodata
            }
        })
        .collect();
    out
}

/// Focal Flow: for each cell a bit per neighbor that is higher than it by more than `threshold`
/// (so water can flow in from it): east 1, south-east 2, south 4, south-west 8, west 16,
/// north-west 32, north 64, north-east 128. 16-bit unsigned, NoData 65535 (observed).
pub fn focal_flow(input: &Raster, threshold: f64) -> Raster {
    const DIRS: [(isize, isize, f64); 8] = [
        (1, 0, 1.0),
        (1, 1, 2.0),
        (0, 1, 4.0),
        (-1, 1, 8.0),
        (-1, 0, 16.0),
        (-1, -1, 32.0),
        (0, -1, 64.0),
        (1, -1, 128.0),
    ];
    let nodata = f64::from(u16::MAX);
    let mut out = input.like(DataType::U16);
    out.nodata = Some(nodata);
    for row in 0..input.height {
        for col in 0..input.width {
            let Some(z) = input.get(col, row) else {
                out.bands[0][row * input.width + col] = nodata;
                continue;
            };
            let mut bits = 0.0;
            for (dx, dy, bit) in DIRS {
                let (x, y) = (col as isize + dx, row as isize + dy);
                if x < 0 || y < 0 || x >= input.width as isize || y >= input.height as isize {
                    continue;
                }
                if input
                    .get(x as usize, y as usize)
                    .is_some_and(|n| n - z > threshold)
                {
                    bits += bit;
                }
            }
            out.bands[0][row * input.width + col] = bits;
        }
    }
    out
}

/// Snap Pour Point: each point moves to the cell of highest flow accumulation within `snap`
/// (distance between cell centres, the point taken at the centre of its own cell); the output
/// holds the point's value in that cell. Integer values take ArcGIS's type for 8-bit rasters.
pub fn snap_pour_point(points: &[(f64, f64, f64)], accumulation: &Raster, snap: f64) -> Raster {
    let t = accumulation.transform;
    let cell = t[1];
    let (min, max) = points
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.2), b.max(p.2)));
    let (dt, nodata) = crate::zonal::integer_output(min.min(max), max.max(min), true);
    let mut out = accumulation.like(dt);
    out.nodata = Some(nodata);
    out.bands[0] = vec![nodata; accumulation.width * accumulation.height];
    for &(x, y, value) in points {
        let (c0, r0) = (
            ((x - t[0]) / cell).floor() as isize,
            ((t[3] - y) / cell).floor() as isize,
        );
        if c0 < 0
            || r0 < 0
            || c0 >= accumulation.width as isize
            || r0 >= accumulation.height as isize
        {
            continue;
        }
        let reach = (snap / cell).floor() as isize;
        let mut best: Option<(f64, f64, usize, usize)> = None;
        for dr in -reach..=reach {
            for dc in -reach..=reach {
                let dist = (dr * dr + dc * dc) as f64 * cell * cell;
                if dist.sqrt() > snap + 1e-9 {
                    continue;
                }
                let (c, r) = (c0 + dc, r0 + dr);
                if c < 0
                    || r < 0
                    || c >= accumulation.width as isize
                    || r >= accumulation.height as isize
                {
                    continue;
                }
                let Some(a) = accumulation.get(c as usize, r as usize) else {
                    continue;
                };
                let better = match best {
                    None => true,
                    Some((ba, bd, ..)) => a > ba || (a == ba && dist < bd),
                };
                if better {
                    best = Some((a, dist, c as usize, r as usize));
                }
            }
        }
        if let Some((_, _, c, r)) = best {
            out.bands[0][r * accumulation.width + c] = dt.store(value);
        }
    }
    out
}

/// Flow Distance with a D8 flow direction raster: for each cell, the distance along its flow path
/// to the first stream cell (any non-NoData cell of `streams`), horizontal (path length) or
/// vertical (height drop on `surface`). Distances accumulate from the stream upwards in single
/// precision, one step at a time, as ArcGIS's agree to within its rounding; a path that leaves the
/// raster or ends without reaching a stream is NoData. Float32, NoData 3.4e38 (ArcGIS Pro 3.7).
pub fn flow_distance(
    streams: &Raster,
    surface: &Raster,
    directions: &Raster,
    vertical: bool,
) -> Option<Raster> {
    let (w, h) = (directions.width, directions.height);
    if (streams.width, streams.height) != (w, h) || (surface.width, surface.height) != (w, h) {
        return None;
    }
    let n = w * h;
    let down = downstream_cells(directions);
    let valid: Vec<bool> = (0..n).map(|i| value(directions, i).is_some()).collect();
    let stream = |i: usize| value(streams, i).is_some();
    let z = |i: usize| value(surface, i).map(|v| v as f32);
    let step = |i: usize, j: usize| -> f32 {
        let diagonal = i % w != j % w && i / w != j / w;
        (if diagonal {
            directions.cell_width().hypot(directions.cell_height())
        } else if i % w != j % w {
            directions.cell_width()
        } else {
            directions.cell_height()
        }) as f32
    };
    let order = upstream_first(&down, &valid);
    let mut dist: Vec<Option<f32>> = (0..n).map(|i| stream(i).then_some(0.0)).collect();
    for &i in order.iter().rev() {
        if dist[i].is_some() {
            continue;
        }
        let Some(j) = down[i] else { continue };
        let Some(dj) = dist[j] else { continue };
        dist[i] = if vertical {
            match (z(i), z(j)) {
                (Some(zi), Some(zj)) => Some(dj + (zi - zj)),
                _ => None,
            }
        } else {
            Some(dj + step(i, j))
        };
    }
    let mut out = directions.like(DataType::F32);
    let nodata = f64::from(3.4e38_f32);
    out.nodata = Some(nodata);
    out.bands[0] = dist.iter().map(|d| d.map_or(nodata, f64::from)).collect();
    Some(out)
}
