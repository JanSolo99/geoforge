//! Spatial Analyst Contour: isolines through cell centers.
//!
//! As ArcGIS does, each square of four cell centers is split into four triangles
//! around its center (whose value is the mean of the corners), and contours are
//! linear within each triangle, so they cross both diagonals. Next to NoData, a
//! square with one NoData corner keeps the triangles away from it (center = mean
//! of the other three). Lines keep higher ground on their right; they come out
//! ordered by contour value descending, then by length.

use std::collections::HashMap;

use crate::Raster;

/// One contour line: its value and vertices in map coordinates.
#[derive(Debug, Clone)]
pub struct ContourLine {
    pub value: f64,
    pub points: Vec<[f64; 2]>,
}

/// A triangle corner: a cell center (row, col) or the center of the square whose
/// top-left cell is (row, col).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Node {
    Cell(usize, usize),
    Middle(usize, usize),
}

type Edge = (Node, Node);

fn edge(a: Node, b: Node) -> Edge {
    if a < b { (a, b) } else { (b, a) }
}

/// The most contour levels one run draws; more is a mistake (an interval of a millionth of the range).
pub const MAX_LEVELS: i64 = 100_000;

/// How many contour levels `contour(r, interval, base)` would draw (0 for none).
pub fn level_count(r: &Raster, interval: f64, base: f64) -> i64 {
    let mut valid = r.bands[0].iter().copied().filter(|v| !r.is_nodata(*v));
    let Some(first) = valid.next() else { return 0 };
    if interval.is_nan() || interval <= 0.0 || !interval.is_finite() {
        return 0;
    }
    let (lo, hi) = valid.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let n = ((hi - base) / interval).floor() - ((lo - base) / interval).ceil() + 1.0;
    if n.is_finite() {
        n.clamp(0.0, i64::MAX as f64) as i64
    } else {
        i64::MAX
    }
}

/// Contours at `base + k * interval` between the raster's minimum and maximum.
pub fn contour(r: &Raster, interval: f64, base: f64) -> Vec<ContourLine> {
    let (w, h) = (r.width, r.height);
    let valid: Vec<f64> = r.bands[0]
        .iter()
        .copied()
        .filter(|v| !r.is_nodata(*v))
        .collect();
    if valid.is_empty() || interval <= 0.0 || w < 2 || h < 2 {
        return Vec::new();
    }
    let lo = valid.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = valid.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if level_count(r, interval, base) > MAX_LEVELS {
        return Vec::new();
    }
    let k0 = ((lo - base) / interval).ceil() as i64;
    let k1 = ((hi - base) / interval).floor() as i64;
    let t = &r.transform;
    let xy = |n: Node| -> [f64; 2] {
        let (row, col) = match n {
            Node::Cell(row, col) => (row as f64 + 0.5, col as f64 + 0.5),
            Node::Middle(row, col) => (row as f64 + 1.0, col as f64 + 1.0),
        };
        [
            t[0] + t[1] * col + t[2] * row,
            t[3] + t[4] * col + t[5] * row,
        ]
    };
    let z = |n: Node| -> f64 {
        match n {
            Node::Cell(row, col) => r.bands[0][row * w + col],
            // The mean of the square's corners that have values.
            Node::Middle(row, col) => {
                let vals: Vec<f64> = [
                    (row, col),
                    (row, col + 1),
                    (row + 1, col),
                    (row + 1, col + 1),
                ]
                .iter()
                .filter_map(|&(y, x)| r.get(x, y))
                .collect();
                vals.iter().sum::<f64>() / vals.len() as f64
            }
        }
    };
    let mut lines = Vec::new();
    for k in k0..=k1 {
        let level = base + k as f64 * interval;
        let crossing = |e: Edge| -> [f64; 2] {
            let (za, zb) = (z(e.0), z(e.1));
            let f = (level - za) / (zb - za);
            let (a, b) = (xy(e.0), xy(e.1));
            [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
        };
        // One oriented segment (entry edge → exit edge) per crossed triangle, in
        // scan order: squares from the bottom row up, west to east.
        let mut segments: Vec<([f64; 2], [f64; 2])> = Vec::new();
        for row in (0..h - 1).rev() {
            for col in 0..w - 1 {
                let corners = [
                    (row, col),
                    (row, col + 1),
                    (row + 1, col),
                    (row + 1, col + 1),
                ];
                // A square with two or more NoData corners is skipped; with one,
                // only the triangles away from it are used.
                let missing = |&(y, x): &(usize, usize)| r.get(x, y).is_none();
                if corners.iter().filter(|c| missing(c)).count() > 1 {
                    continue;
                }
                let tl = Node::Cell(row, col);
                let tr = Node::Cell(row, col + 1);
                let bl = Node::Cell(row + 1, col);
                let br = Node::Cell(row + 1, col + 1);
                let c = Node::Middle(row, col);
                for tri in [[tl, tr, c], [tr, br, c], [br, bl, c], [bl, tl, c]] {
                    if tri.iter().any(|n| match *n {
                        Node::Cell(y, x) => missing(&(y, x)),
                        Node::Middle(..) => false,
                    }) {
                        continue;
                    }
                    let above = tri.map(|n| z(n) > level);
                    if above.iter().all(|&a| a) || above.iter().all(|&a| !a) {
                        continue;
                    }
                    let crossed: Vec<Edge> = (0..3)
                        .filter(|&i| above[i] != above[(i + 1) % 3])
                        .map(|i| edge(tri[i], tri[(i + 1) % 3]))
                        .collect();
                    let (p0, p1) = (crossing(crossed[0]), crossing(crossed[1]));
                    // A contour through a corner exactly can cross a triangle
                    // in a single point.
                    if (p0[0] - p1[0]).abs() < 1e-9 && (p0[1] - p1[1]).abs() < 1e-9 {
                        continue;
                    }
                    let high = xy(tri[above.iter().position(|&a| a).unwrap_or(0)]);
                    // Higher ground on the right of p0 → p1.
                    let side =
                        (p1[0] - p0[0]) * (high[1] - p0[1]) - (p1[1] - p0[1]) * (high[0] - p0[0]);
                    segments.push(if side < 0.0 { (p0, p1) } else { (p1, p0) });
                }
            }
        }
        lines.extend(link(&segments).into_iter().map(|points| ContourLine {
            value: level,
            points: drop_collinear(points),
        }));
    }
    let length = |l: &ContourLine| {
        l.points
            .windows(2)
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum::<f64>()
    };
    lines.sort_by(|a, b| {
        b.value
            .total_cmp(&a.value)
            .then(length(a).total_cmp(&length(b)))
    });
    lines
}

/// Joins oriented segments end to start into lines (rings close on their first
/// point). Segments meet at shared points, matched to 1e-6.
fn link(segments: &[([f64; 2], [f64; 2])]) -> Vec<Vec<[f64; 2]>> {
    let key = |p: [f64; 2]| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
    let mut starting: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let mut ending: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in segments.iter().enumerate() {
        starting.entry(key(s.0)).or_default().push(i);
        ending.entry(key(s.1)).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        // Walk back to the start of an open line; a ring starts at `first`.
        let mut start = first;
        let mut seen = vec![first];
        while let Some(&p) = ending
            .get(&key(segments[start].0))
            .and_then(|c| c.iter().find(|&&p| !used[p]))
        {
            if p == first || seen.contains(&p) {
                break;
            }
            seen.push(p);
            start = p;
        }
        let mut points = vec![segments[start].0];
        let mut cur = start;
        loop {
            used[cur] = true;
            points.push(segments[cur].1);
            match starting
                .get(&key(segments[cur].1))
                .and_then(|c| c.iter().find(|&&n| !used[n]))
            {
                Some(&n) => cur = n,
                None => break,
            }
        }
        if points.len() >= 2 {
            lines.push(points);
        }
    }
    lines
}

/// Removes vertices exactly in line with their neighbors (contours through
/// cells holding the contour value run straight across several triangles).
fn drop_collinear(points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    if points.len() < 3 {
        return points;
    }
    let mut out = vec![points[0]];
    for i in 1..points.len() - 1 {
        let (a, b, c) = (out[out.len() - 1], points[i], points[i + 1]);
        let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        let len2 = (c[0] - a[0]).powi(2) + (c[1] - a[1]).powi(2);
        if cross.abs() > 1e-9 * len2 {
            out.push(b);
        }
    }
    out.push(points[points.len() - 1]);
    out
}
