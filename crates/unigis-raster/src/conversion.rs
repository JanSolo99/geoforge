//! Raster to vector conversion: Raster To Point and Raster To Polygon.

use std::collections::HashMap;

use crate::Raster;

/// Raster To Point: one point per cell with a value, at the cell center, row by
/// row from the top.
pub fn raster_to_points(r: &Raster) -> Vec<([f64; 2], f64)> {
    let t = &r.transform;
    let mut out = Vec::new();
    for row in 0..r.height {
        for col in 0..r.width {
            if let Some(v) = r.get(col, row) {
                let (c, rr) = (col as f64 + 0.5, row as f64 + 0.5);
                out.push((
                    [t[0] + t[1] * c + t[2] * rr, t[3] + t[4] * c + t[5] * rr],
                    v,
                ));
            }
        }
    }
    out
}

/// A directed edge between two grid corners (column, row).
type GridEdge = ((usize, usize), (usize, usize));

/// One polygon of Raster To Polygon: the region's value and its rings (outer
/// ring clockwise first, then holes counterclockwise), closed.
#[derive(Debug, Clone)]
pub struct RegionPolygon {
    pub value: f64,
    pub rings: Vec<Vec<[f64; 2]>>,
}

/// Raster To Polygon without simplification: one polygon per 4-connected region
/// of equal value, ordered by the region's first cell in raster order, outlines
/// following cell edges with collinear vertices removed.
pub fn raster_to_polygons(r: &Raster) -> Vec<RegionPolygon> {
    let (w, h) = (r.width, r.height);
    let mut region = vec![usize::MAX; w * h];
    let mut values = Vec::new();
    for start in 0..w * h {
        if region[start] != usize::MAX {
            continue;
        }
        let Some(v) = r.get(start % w, start / w) else {
            continue;
        };
        let id = values.len();
        values.push(v);
        region[start] = id;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if region[j] == usize::MAX && r.get(j % w, j / w) == Some(v) {
                    region[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
    }
    // Directed boundary edges between grid corners (cx, cy), cy down, with the
    // region on the right in map orientation (clockwise outer rings).
    let mut edges: Vec<Vec<GridEdge>> = vec![Vec::new(); values.len()];
    let at = |x: isize, y: isize| -> usize {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            usize::MAX
        } else {
            region[y as usize * w + x as usize]
        }
    };
    for y in 0..h {
        for x in 0..w {
            let id = region[y * w + x];
            if id == usize::MAX {
                continue;
            }
            let (xi, yi) = (x as isize, y as isize);
            // Walking clockwise on screen (y down) keeps the cell on the right
            // in map coordinates too, since both flip together.
            if at(xi, yi - 1) != id {
                edges[id].push(((x, y), (x + 1, y)));
            }
            if at(xi + 1, yi) != id {
                edges[id].push(((x + 1, y), (x + 1, y + 1)));
            }
            if at(xi, yi + 1) != id {
                edges[id].push(((x + 1, y + 1), (x, y + 1)));
            }
            if at(xi - 1, yi) != id {
                edges[id].push(((x, y + 1), (x, y)));
            }
        }
    }
    let t = &r.transform;
    let map = |(cx, cy): (usize, usize)| -> [f64; 2] {
        let (c, rr) = (cx as f64, cy as f64);
        [t[0] + t[1] * c + t[2] * rr, t[3] + t[4] * c + t[5] * rr]
    };
    values
        .iter()
        .zip(edges)
        .map(|(&value, edges)| {
            let mut from: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
            for (i, e) in edges.iter().enumerate() {
                from.entry(e.0).or_default().push(i);
            }
            let mut used = vec![false; edges.len()];
            let mut rings: Vec<Vec<(usize, usize)>> = Vec::new();
            for first in 0..edges.len() {
                if used[first] {
                    continue;
                }
                let mut ring = vec![edges[first].0];
                let mut cur = first;
                loop {
                    used[cur] = true;
                    let (a, b) = edges[cur];
                    ring.push(b);
                    let dir = (b.0 as isize - a.0 as isize, b.1 as isize - a.1 as isize);
                    // At a corner touched diagonally, turn right (on screen) so the
                    // 4-connected region's rings stay apart.
                    let next = from.get(&b).and_then(|c| {
                        let open: Vec<usize> = c.iter().copied().filter(|&n| !used[n]).collect();
                        let turn = |n: &usize| {
                            let (p, q) = edges[*n];
                            let d = (q.0 as isize - p.0 as isize, q.1 as isize - p.1 as isize);
                            dir.0 * d.1 - dir.1 * d.0
                        };
                        open.iter().copied().max_by_key(turn)
                    });
                    match next {
                        Some(n) => cur = n,
                        None => break,
                    }
                }
                rings.push(simplify(ring));
            }
            // The outer ring is the one with the largest area.
            let area = |ring: &Vec<(usize, usize)>| -> f64 {
                ring.windows(2)
                    .map(|p| p[0].0 as f64 * p[1].1 as f64 - p[1].0 as f64 * p[0].1 as f64)
                    .sum::<f64>()
                    .abs()
            };
            rings.sort_by(|a, b| area(b).total_cmp(&area(a)));
            RegionPolygon {
                value,
                rings: rings
                    .into_iter()
                    .map(|ring| ring.into_iter().map(map).collect())
                    .collect(),
            }
        })
        .collect()
}

/// Drops collinear vertices of a closed ring of grid corners.
fn simplify(ring: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let n = ring.len() - 1; // closed: last == first
    let pts = &ring[..n];
    let turns = |i: usize| {
        let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let d1 = (b.0 as isize - a.0 as isize, b.1 as isize - a.1 as isize);
        let d2 = (c.0 as isize - b.0 as isize, c.1 as isize - b.1 as isize);
        d1.0 * d2.1 - d1.1 * d2.0 != 0
    };
    let mut out: Vec<(usize, usize)> = (0..n).filter(|&i| turns(i)).map(|i| pts[i]).collect();
    out.push(out[0]);
    out
}

/// Raster To ASCII: the Esri ASCII grid ArcGIS writes. The header's names are padded to 14
/// characters, NoData is always -9999, rows end with a space, lines with CR LF; floats are written with 7 significant digits, as 32-bit floats.
pub fn to_ascii(r: &crate::Raster) -> String {
    use std::fmt::Write;
    let cell = r.transform[1];
    let lower = r.transform[3] + r.transform[5] * r.height as f64;
    let mut out = String::new();
    let mut header = |name: &str, value: String| {
        let _ = write!(out, "{name:<14}{value}\r\n");
    };
    header("ncols", r.width.to_string());
    header("nrows", r.height.to_string());
    header("xllcorner", format!("{}", r.transform[0]));
    header("yllcorner", format!("{lower}"));
    header("cellsize", format!("{cell}"));
    header("NODATA_value", "-9999".into());
    for row in 0..r.height {
        for col in 0..r.width {
            let v = r.bands[0][row * r.width + col];
            if r.is_nodata(v) {
                out.push_str("-9999 ");
            } else if r.data_type.is_float() {
                out.push_str(&format_g7(v as f32));
                out.push(' ');
            } else {
                let _ = write!(out, "{v} ");
            }
        }
        out.push_str("\r\n");
    }
    out
}

/// `%.7g` of a 32-bit float.
fn format_g7(v: f32) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let exp = (f64::from(v).abs().log10().floor()) as i32;
    if !(-5..7).contains(&exp) {
        let s = format!("{:.6e}", v);
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = m.trim_end_matches('0').trim_end_matches('.');
        let e: i32 = e.parse().unwrap_or(0);
        return format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs());
    }
    let decimals = (6 - exp).max(0) as usize;
    let s = format!("{:.*}", decimals, f64::from(v));
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}
