//! Spatial Analyst Contour: isolines through cell centers.
//!
//! As ArcGIS does, each square of four cell centers is split into four triangles
//! around its center (whose value is the mean of the corners), and contours are
//! linear within each triangle, so they cross both diagonals. Squares touching a
//! NoData cell are skipped. Lines keep higher ground on their right; they come out
//! ordered by contour value descending, then by vertex count.

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
            Node::Middle(row, col) => {
                (r.bands[0][row * w + col]
                    + r.bands[0][row * w + col + 1]
                    + r.bands[0][(row + 1) * w + col]
                    + r.bands[0][(row + 1) * w + col + 1])
                    / 4.0
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
        let mut segments: Vec<(Edge, Edge)> = Vec::new();
        for row in (0..h - 1).rev() {
            for col in 0..w - 1 {
                let corners = [
                    (row, col),
                    (row, col + 1),
                    (row + 1, col),
                    (row + 1, col + 1),
                ];
                if corners
                    .iter()
                    .any(|&(y, x)| r.is_nodata(r.bands[0][y * w + x]))
                {
                    continue;
                }
                let tl = Node::Cell(row, col);
                let tr = Node::Cell(row, col + 1);
                let bl = Node::Cell(row + 1, col);
                let br = Node::Cell(row + 1, col + 1);
                let c = Node::Middle(row, col);
                for tri in [[tl, tr, c], [tr, br, c], [br, bl, c], [bl, tl, c]] {
                    let above = tri.map(|n| z(n) >= level);
                    if above.iter().all(|&a| a) || above.iter().all(|&a| !a) {
                        continue;
                    }
                    let crossed: Vec<Edge> = (0..3)
                        .filter(|&i| above[i] != above[(i + 1) % 3])
                        .map(|i| edge(tri[i], tri[(i + 1) % 3]))
                        .collect();
                    let (p0, p1) = (crossing(crossed[0]), crossing(crossed[1]));
                    let high = xy(tri[above.iter().position(|&a| a).unwrap_or(0)]);
                    // Higher ground on the right of p0 → p1.
                    let side =
                        (p1[0] - p0[0]) * (high[1] - p0[1]) - (p1[1] - p0[1]) * (high[0] - p0[0]);
                    segments.push(if side < 0.0 {
                        (crossed[0], crossed[1])
                    } else {
                        (crossed[1], crossed[0])
                    });
                }
            }
        }
        let by_entry: HashMap<Edge, usize> =
            segments.iter().enumerate().map(|(i, s)| (s.0, i)).collect();
        let by_exit: HashMap<Edge, usize> =
            segments.iter().enumerate().map(|(i, s)| (s.1, i)).collect();
        let mut used = vec![false; segments.len()];
        for first in 0..segments.len() {
            if used[first] {
                continue;
            }
            // Walk back to the start of an open line; a ring starts here.
            let mut start = first;
            while let Some(&p) = by_exit.get(&segments[start].0) {
                if p == first {
                    start = first;
                    break;
                }
                if used[p] {
                    break;
                }
                start = p;
            }
            let mut points = vec![crossing(segments[start].0)];
            let mut cur = start;
            loop {
                used[cur] = true;
                let p = crossing(segments[cur].1);
                if points.last() != Some(&p) {
                    points.push(p);
                }
                match by_entry.get(&segments[cur].1) {
                    Some(&n) if !used[n] => cur = n,
                    Some(&n) if n == start => {
                        // Closed ring: end exactly on its first vertex.
                        let first_pt = points[0];
                        if points.last() != Some(&first_pt) {
                            points.push(first_pt);
                        }
                        break;
                    }
                    _ => break,
                }
            }
            if points.len() >= 2 {
                lines.push(ContourLine {
                    value: level,
                    points,
                });
            }
        }
    }
    lines.sort_by(|a, b| {
        b.value
            .total_cmp(&a.value)
            .then(a.points.len().cmp(&b.points.len()))
    });
    lines
}
