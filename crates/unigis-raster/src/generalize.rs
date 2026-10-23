//! Spatial Analyst generalization tools: Expand, Shrink, Majority Filter and Region Group.
//!
//! Matched to ArcGIS Pro 3.7 cell for cell (`testdata/arcgis-reference/{expand,shrink,
//! majority,regiongroup}_*`, plus synthetic probes of ties, edges and NoData):
//! - The number of cells rounds half to even (2.5 is 2, 1.5 is 2) and must then be at least 1.
//! - Expand (MORPHOLOGICAL) grows the listed zones one cell per pass, all cells of a pass
//!   deciding on the previous pass: a cell outside the zones (NoData included) takes the zone
//!   value most frequent among its eight neighbors; a tie goes to the value met first reading
//!   the neighbors column by column (NW, W, SW, N, S, NE, E, SE).
//! - Shrink (MORPHOLOGICAL) is the reverse: a cell of a listed zone takes the most frequent
//!   value among its neighbors that are in no listed zone, NoData counting as a value, with the
//!   same tie order.
//! - DISTANCE: a cell within `n` cells (Euclidean, between centers) of the expanding cells takes
//!   the value of the nearest one; among equally near ones the smallest value wins (NoData
//!   below every value when shrinking).
//! - Majority Filter is one pass. NoData is a value like any other; the raster continues past
//!   its edges as its mirror image about the edge cell. A value replaces the center when it
//!   fills a run of contiguous ring cells of at least 5 of 8 (EIGHT, MAJORITY), 4 of 8 (HALF),
//!   3 of 4 (FOUR, MAJORITY) or 2 of 4 (HALF), and the center's own value has no such run. The
//!   run that completes first walking the ring clockwise from SE (EIGHT) or W (FOUR) wins; the
//!   center keeps its value when it fills such a run on a single clockwise walk from SE (EIGHT)
//!   or N (FOUR).
//! - Region Group numbers regions in the order of the provisional labels that survive a
//!   one-pass union-find labeling (each cell takes its W neighbor's label, else NW, N, NE;
//!   unions keep the set holding more provisional labels, on a tie the later neighbor's).
//!   CROSS joins cells whatever their values; the excluded value is 0 in the output.
//! - Integer output types: Expand, Shrink (MORPHOLOGICAL) and Majority Filter fit the input's
//!   value range ([`class_type`]); DISTANCE keeps the input type (unsigned: its NoData; signed:
//!   the type's maximum as NoData); Region Group fits 1 ..= (rows + 1) x (columns + 1).

use crate::{DataType, Raster};

/// Why a generalization tool refused its parameters (the message carries ArcGIS's code).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

fn integer_input(input: &Raster) -> Result<(), Refused> {
    if input.data_type.is_float() {
        return Err(Refused(
            "ERROR 000864: Input raster: The input is not within the defined domain. \
             ERROR 000969: The input raster is not integer type."
                .into(),
        ));
    }
    Ok(())
}

/// The number of cells as ArcGIS takes it: rounded half to even, between 1 and 2^31 - 1.
pub fn number_of_cells(n: f64) -> Result<usize, Refused> {
    let r = n.round_ties_even();
    if !(1.0..=i32::MAX as f64).contains(&r) {
        return Err(Refused(format!(
            "ERROR 000864: Number of cells: The input is not within the defined domain. \
             ERROR 000892: The numeric value is outside of the valid range ({n})."
        )));
    }
    Ok(r as usize)
}

/// Expand and Shrink methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Morphological,
    Distance,
}

impl Method {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "MORPHOLOGICAL" => Some(Self::Morphological),
            "DISTANCE" => Some(Self::Distance),
            _ => None,
        }
    }
}

/// The integer type ArcGIS gives Expand, Shrink and Majority Filter for input values in
/// `lo ..= hi`, with its NoData value (1-, 2- and 4-bit types are unsigned bytes here).
pub fn class_type(lo: f64, hi: f64) -> (DataType, f64) {
    if lo >= 0.0 {
        for bits in [1u32, 2, 4] {
            let top = ((1u64 << bits) - 1) as f64;
            if hi < top {
                return (DataType::U8, top);
            }
            if hi == top && lo > 0.0 {
                return (DataType::U8, 0.0);
            }
        }
        if hi <= 126.0 {
            return (DataType::I8, -128.0);
        }
        for (t, top) in [(DataType::U8, 255.0), (DataType::U16, 65_535.0)] {
            if hi < top {
                return (t, top);
            }
            if hi == top && lo > 0.0 {
                return (t, 0.0);
            }
        }
        if hi < i32::MAX as f64 {
            return (DataType::I32, -(i32::MAX as f64));
        }
        if hi < u32::MAX as f64 {
            return (DataType::U32, u32::MAX as f64);
        }
        return (DataType::U32, 0.0);
    }
    for (t, min, max) in [
        (DataType::I8, i8::MIN as f64, i8::MAX as f64),
        (DataType::I16, i16::MIN as f64, i16::MAX as f64),
    ] {
        if lo > min && hi <= max {
            return (t, min);
        }
        if lo == min && hi < max {
            return (t, max);
        }
    }
    if lo > -(i32::MAX as f64) {
        (DataType::I32, -(i32::MAX as f64))
    } else {
        (DataType::I32, i32::MAX as f64)
    }
}

/// The input's value range: its stored statistics, else its cells.
fn value_range(input: &Raster) -> (f64, f64) {
    if let Some(s) = input.statistics {
        return s;
    }
    let (lo, hi) = input.bands[0]
        .iter()
        .filter(|v| !input.is_nodata(**v))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    if lo.is_finite() { (lo, hi) } else { (0.0, 0.0) }
}

/// `cells` (raw values, the input's NoData among them) as a raster of [`class_type`].
fn class_raster(input: &Raster, cells: &[f64]) -> Raster {
    let (lo, hi) = value_range(input);
    let (t, nodata) = class_type(lo, hi);
    let mut out = input.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = cells
        .iter()
        .map(|&v| {
            if input.is_nodata(v) {
                nodata
            } else {
                t.store(v)
            }
        })
        .collect();
    out
}

/// `cells` as a raster of the input's type (the DISTANCE methods).
fn same_type_raster(input: &Raster, cells: &[f64]) -> Raster {
    let t = input.data_type;
    let nodata = match t {
        DataType::U8 | DataType::U16 | DataType::U32 => input
            .nodata
            .unwrap_or_else(|| crate::local::type_range_of(t).1),
        _ => crate::local::type_range_of(t).1,
    };
    let mut out = input.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = cells
        .iter()
        .map(|&v| if input.is_nodata(v) { nodata } else { v })
        .collect();
    out
}

/// Neighbors in the order ties are broken: column by column from the north-west.
const TIE_ORDER: [(isize, isize); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
]; // (dx, dy)

/// One morphological pass deciding `todo` cells from `cells`: each takes the most frequent
/// neighbor value that `counts` accepts. Returns the changed cells.
fn morph_pass(
    w: usize,
    h: usize,
    cells: &mut [f64],
    todo: &[usize],
    counts: &dyn Fn(f64, f64) -> bool,
) -> Vec<(usize, f64)> {
    let mut changes = Vec::new();
    let mut seen: Vec<(f64, u32)> = Vec::with_capacity(8);
    for &i in todo {
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        seen.clear();
        for (dx, dy) in TIE_ORDER {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                continue;
            }
            let v = cells[ny as usize * w + nx as usize];
            if !counts(cells[i], v) {
                continue;
            }
            match seen.iter_mut().find(|s| s.0.to_bits() == v.to_bits()) {
                Some(s) => s.1 += 1,
                None => seen.push((v, 1)),
            }
        }
        // The first of the most frequent (max_by_key would keep the last).
        let mut best: Option<(f64, u32)> = None;
        for &s in &seen {
            if best.is_none_or(|b| s.1 > b.1) {
                best = Some(s);
            }
        }
        if let Some((v, _)) = best {
            changes.push((i, v));
        }
    }
    for &(i, v) in &changes {
        cells[i] = v;
    }
    changes
}

fn neighbors8(w: usize, h: usize, i: usize) -> impl Iterator<Item = usize> {
    let (x, y) = ((i % w) as isize, (i / w) as isize);
    TIE_ORDER.into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        (nx >= 0 && ny >= 0 && nx < w as isize && ny < h as isize)
            .then(|| ny as usize * w + nx as usize)
    })
}

/// Repeats [`morph_pass`] up to `n` times; after a pass only the cells next to changed ones can
/// change (`eligible` says which cells may).
fn morph(
    input: &Raster,
    n: usize,
    eligible: &dyn Fn(f64) -> bool,
    counts: &dyn Fn(f64, f64) -> bool,
) -> Vec<f64> {
    let (w, h) = (input.width, input.height);
    let mut cells = input.bands[0].clone();
    let mut todo: Vec<usize> = (0..w * h).filter(|&i| eligible(cells[i])).collect();
    let mut mark = vec![false; w * h];
    for _ in 0..n {
        let changes = morph_pass(w, h, &mut cells, &todo, counts);
        if changes.is_empty() {
            break;
        }
        todo.clear();
        for &(i, _) in &changes {
            for j in neighbors8(w, h, i) {
                if !mark[j] && eligible(cells[j]) {
                    mark[j] = true;
                    todo.push(j);
                }
            }
        }
        todo.sort_unstable();
        for &j in &todo {
            mark[j] = false;
        }
    }
    cells
}

/// Squared distances (in cells) from every cell to the nearest cell where `source` holds.
fn squared_distances(w: usize, h: usize, source: &[bool]) -> Vec<f64> {
    let mut grid: Vec<f64> = source
        .iter()
        .map(|&s| if s { 0.0 } else { f64::INFINITY })
        .collect();
    let mut col = vec![0.0; h];
    for x in 0..w {
        for y in 0..h {
            col[y] = grid[y * w + x];
        }
        crate::resample::edt_1d(&mut col);
        for y in 0..h {
            grid[y * w + x] = col[y];
        }
    }
    for y in 0..h {
        crate::resample::edt_1d(&mut grid[y * w..(y + 1) * w]);
    }
    grid
}

/// The DISTANCE method: cells where `target` holds, within `n` cells of a `source` cell, take
/// the smallest value (by `key`) among their nearest source cells.
fn by_distance(
    input: &Raster,
    n: usize,
    source: &dyn Fn(f64) -> bool,
    key: &dyn Fn(f64) -> f64,
) -> Vec<f64> {
    let (w, h) = (input.width, input.height);
    let cells = &input.bands[0];
    let src: Vec<bool> = cells.iter().map(|&v| source(v)).collect();
    let d2 = squared_distances(w, h, &src);
    let limit = (n as f64) * (n as f64);
    let mut out = cells.clone();
    for i in 0..w * h {
        if src[i] || !d2[i].is_finite() || d2[i] > limit {
            continue;
        }
        let d = d2[i].round() as i64;
        let (x, y) = ((i % w) as i64, (i / w) as i64);
        let mut best: Option<f64> = None;
        let mut dx = 0i64;
        while dx * dx <= d {
            let rest = d - dx * dx;
            let dy = (rest as f64).sqrt().round() as i64;
            if dy * dy == rest {
                for (sx, sy) in [(dx, dy), (-dx, dy), (dx, -dy), (-dx, -dy)] {
                    let (cx, cy) = (x + sx, y + sy);
                    if cx < 0 || cy < 0 || cx >= w as i64 || cy >= h as i64 {
                        continue;
                    }
                    let j = cy as usize * w + cx as usize;
                    if src[j] && best.is_none_or(|b| key(cells[j]) < key(b)) {
                        best = Some(cells[j]);
                    }
                }
            }
            dx += 1;
        }
        if let Some(v) = best {
            out[i] = v;
        }
    }
    out
}

fn zone_test<'a>(input: &'a Raster, zones: &'a [f64]) -> impl Fn(f64) -> bool + 'a {
    move |v: f64| !input.is_nodata(v) && zones.contains(&v)
}

/// Spatial Analyst Expand: grows the zones `zones` by `n` cells (see [`number_of_cells`]).
pub fn expand(input: &Raster, n: usize, zones: &[f64], method: Method) -> Result<Raster, Refused> {
    integer_input(input)?;
    if zones.is_empty() {
        return Err(Refused(
            "ERROR 000859: The required parameter Zone values is empty".into(),
        ));
    }
    let zone = zone_test(input, zones);
    Ok(match method {
        Method::Morphological => {
            let cells = morph(input, n, &|v| !zone(v), &|_, v| zone(v));
            class_raster(input, &cells)
        }
        Method::Distance => {
            let cells = by_distance(input, n, &zone, &|v| v);
            same_type_raster(input, &cells)
        }
    })
}

/// Spatial Analyst Shrink: shrinks the zones `zones` by `n` cells.
pub fn shrink(input: &Raster, n: usize, zones: &[f64], method: Method) -> Result<Raster, Refused> {
    integer_input(input)?;
    if zones.is_empty() {
        return Err(Refused(
            "ERROR 000859: The required parameter Zone values is empty".into(),
        ));
    }
    let zone = zone_test(input, zones);
    Ok(match method {
        Method::Morphological => {
            let cells = morph(input, n, &zone, &|center, v| {
                v.to_bits() != center.to_bits() && !zone(v)
            });
            class_raster(input, &cells)
        }
        Method::Distance => {
            let key = |v: f64| {
                if input.is_nodata(v) {
                    f64::NEG_INFINITY
                } else {
                    v
                }
            };
            let cells = by_distance(input, n, &|v| !zone(v), &key);
            same_type_raster(input, &cells)
        }
    })
}

/// Ring of neighbors clockwise from the north, as (dx, dy).
const RING8: [(isize, isize); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];
const RING4: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Mirror index about the edge cell (a raster one cell wide repeats that cell).
fn mirror(i: isize, n: usize) -> usize {
    let n = n as isize;
    let j = if i < 0 {
        -i
    } else if i >= n {
        2 * (n - 1) - i
    } else {
        i
    };
    j.clamp(0, n - 1) as usize
}

/// Spatial Analyst Majority Filter (`eight`: EIGHT neighbors; `half`: HALF).
pub fn majority_filter(input: &Raster, eight: bool, half: bool) -> Result<Raster, Refused> {
    integer_input(input)?;
    let (w, h) = (input.width, input.height);
    let cells = &input.bands[0];
    let ring: &[(isize, isize)] = if eight { &RING8 } else { &RING4 };
    let len = ring.len();
    let need = match (eight, half) {
        (true, false) => 5,
        (true, true) => 4,
        (false, false) => 3,
        (false, true) => 2,
    };
    // The winning walk starts at SE (EIGHT) or W (FOUR); the center's at SE or N.
    let (start, center_start) = if eight { (3, 3) } else { (3, 0) };
    let mut out = cells.clone();
    let mut vals = [0u64; 8];
    for y in 0..h {
        for x in 0..w {
            for (k, (dx, dy)) in ring.iter().enumerate() {
                let (nx, ny) = (mirror(x as isize + dx, w), mirror(y as isize + dy, h));
                vals[k] = cells[ny * w + nx].to_bits();
            }
            let center = cells[y * w + x].to_bits();
            let mut win = None;
            let (mut cur, mut run) = (u64::MAX, 0);
            for step in 0..2 * len {
                let v = vals[(start + step) % len];
                if run > 0 && v == cur {
                    run += 1;
                } else {
                    cur = v;
                    run = 1;
                }
                if run >= need {
                    win = Some(cur);
                    break;
                }
            }
            let Some(win) = win else { continue };
            if win == center {
                continue;
            }
            let mut run = 0;
            let mut keep = false;
            for step in 0..len {
                if vals[(center_start + step) % len] == center {
                    run += 1;
                    keep |= run >= need;
                } else {
                    run = 0;
                }
            }
            if !keep {
                out[y * w + x] = f64::from_bits(win);
            }
        }
    }
    Ok(class_raster(input, &out))
}

/// Spatial Analyst Region Group (`eight`: EIGHT neighbors; `cross`: CROSS; cells holding
/// `excluded` are 0 and join no region).
pub fn region_group(input: &Raster, eight: bool, cross: bool, excluded: Option<f64>) -> Raster {
    let (w, h) = (input.width, input.height);
    let cells = &input.bands[0];
    let member = |i: usize| {
        let v = cells[i];
        !input.is_nodata(v) && excluded != Some(v)
    };
    let same = |i: usize, j: usize| cross || cells[i].to_bits() == cells[j].to_bits();
    let mut label = vec![0usize; w * h];
    // parent[0] is unused; size counts provisional labels.
    let mut parent = vec![0usize];
    let mut size = vec![0usize];
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let prior: &[(isize, isize)] = if eight {
        &[(-1, 0), (-1, -1), (0, -1), (1, -1)]
    } else {
        &[(-1, 0), (0, -1)]
    };
    let mut found = Vec::with_capacity(4);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !member(i) {
                continue;
            }
            found.clear();
            for &(dx, dy) in prior {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx >= w as isize {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if label[j] != 0 && same(i, j) {
                    found.push(label[j]);
                }
            }
            let Some(&first) = found.first() else {
                let l = parent.len();
                parent.push(l);
                size.push(1);
                label[i] = l;
                continue;
            };
            label[i] = first;
            for &other in &found[1..] {
                let (a, b) = (find(&mut parent, first), find(&mut parent, other));
                if a == b {
                    continue;
                }
                let (keep, gone) = if size[a] > size[b] { (a, b) } else { (b, a) };
                parent[gone] = keep;
                size[keep] += size[gone];
            }
        }
    }
    let mut number = vec![0u32; parent.len()];
    let mut count = 0u32;
    for (l, n) in number.iter_mut().enumerate().skip(1) {
        if find(&mut parent, l) == l {
            count += 1;
            *n = count;
        }
    }
    let (t, nodata) = class_type(1.0, ((h + 1) * (w + 1)) as f64);
    let mut out = input.like(t);
    out.nodata = Some(nodata);
    out.bands[0] = (0..w * h)
        .map(|i| {
            if label[i] != 0 {
                number[find(&mut parent, label[i])] as f64
            } else if !input.is_nodata(cells[i]) {
                0.0
            } else {
                nodata
            }
        })
        .collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_types() {
        let cases = [
            ((1.0, 1.0), (DataType::U8, 0.0)),
            ((0.0, 0.0), (DataType::U8, 1.0)),
            ((1.0, 2.0), (DataType::U8, 3.0)),
            ((2.0, 3.0), (DataType::U8, 0.0)),
            ((0.0, 3.0), (DataType::U8, 15.0)),
            ((0.0, 15.0), (DataType::I8, -128.0)),
            ((1.0, 15.0), (DataType::U8, 0.0)),
            ((0.0, 127.0), (DataType::U8, 255.0)),
            ((0.0, 255.0), (DataType::U16, 65_535.0)),
            ((1.0, 255.0), (DataType::U8, 0.0)),
            ((0.0, 65_535.0), (DataType::I32, -2_147_483_647.0)),
            ((-128.0, 0.0), (DataType::I8, 127.0)),
            ((-128.0, 127.0), (DataType::I16, -32_768.0)),
            ((-32_768.0, 32_767.0), (DataType::I32, -2_147_483_647.0)),
            ((-2_147_483_647.0, 5.0), (DataType::I32, 2_147_483_647.0)),
            ((0.0, 2_147_483_647.0), (DataType::U32, 4_294_967_295.0)),
        ];
        for ((lo, hi), want) in cases {
            assert_eq!(class_type(lo, hi), want, "{lo}..{hi}");
        }
    }

    #[test]
    fn cells_round_half_even() {
        assert_eq!(number_of_cells(2.5), Ok(2));
        assert_eq!(number_of_cells(1.5), Ok(2));
        assert_eq!(number_of_cells(0.6), Ok(1));
        assert!(number_of_cells(0.5).is_err());
        assert!(number_of_cells(-1.0).is_err());
        assert!(number_of_cells(1e12).is_err());
        assert!(number_of_cells(f64::NAN).is_err());
    }
}
