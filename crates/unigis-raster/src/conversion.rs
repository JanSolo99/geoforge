//! Raster to vector conversion: Raster To Point, Raster To Polyline and Raster To Polygon.

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

/// One arc of Raster To Polyline: its value, endpoint node IDs and cell-center vertices.
#[derive(Debug, Clone)]
pub struct RegionPolyline {
    pub value: f64,
    pub from_node: usize,
    pub to_node: usize,
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug)]
struct CellArc {
    cells: Vec<usize>,
}

/// Raster To Polyline: connect four-adjacent foreground cell centers, merging each pair of arcs
/// at a degree-two cell. Nodes and arcs follow ArcGIS's raster scan order.
pub fn raster_to_polylines(r: &Raster, include_zero: bool) -> Vec<RegionPolyline> {
    let (w, h) = (r.width, r.height);
    let foreground = |i: usize| {
        r.get(i % w, i / w)
            .is_some_and(|v| include_zero || v != 0.0)
    };
    let degree = |i: usize| {
        let (x, y) = (i % w, i / w);
        usize::from(x > 0 && foreground(i - 1))
            + usize::from(x + 1 < w && foreground(i + 1))
            + usize::from(y > 0 && foreground(i - w))
            + usize::from(y + 1 < h && foreground(i + w))
    };
    let mut node = vec![0; w * h];
    let mut nodes = 0;
    for (i, node_id) in node.iter_mut().enumerate() {
        if foreground(i) && degree(i) != 2 {
            nodes += 1;
            *node_id = nodes;
        }
    }

    let mut arcs: Vec<Option<CellArc>> = Vec::new();
    let mut endpoints: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..w * h {
        if !foreground(i) {
            continue;
        }
        let (x, y) = (i % w, i / w);
        for j in [
            x + 1 < w && foreground(i + 1),
            y + 1 < h && foreground(i + w),
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(axis, yes)| yes.then_some(if axis == 0 { i + 1 } else { i + w }))
        {
            let id = arcs.len();
            arcs.push(Some(CellArc { cells: vec![i, j] }));
            endpoints.entry(i).or_default().push(id);
            endpoints.entry(j).or_default().push(id);
        }
    }

    // 0/1 = left/right, 2/3 = up/down. Opposite pairs use their natural direction; corners
    // start at left, down, up, right, respectively, as in ArcGIS's output.
    let side = |center: usize, next: usize| match next {
        n if n + 1 == center => 0,
        n if n == center + 1 => 1,
        n if n + w == center => 2,
        _ => 3,
    };
    for center in 0..w * h {
        if !foreground(center) || degree(center) != 2 {
            continue;
        }
        let ids = endpoints
            .remove(&center)
            .expect("degree-two cell has two arcs");
        let a = arcs[ids[0]].take().expect("arc is active");
        let b = arcs[ids[1]].take().expect("arc is active");
        let outward = |mut arc: CellArc| {
            if arc.cells[0] != center {
                arc.cells.reverse();
            }
            arc
        };
        let (a, b) = (outward(a), outward(b));
        let sides = (side(center, a.cells[1]), side(center, b.cells[1]));
        let start_side = if sides.0 ^ sides.1 == 1 {
            sides.0.min(sides.1)
        } else {
            [sides.0, sides.1]
                .into_iter()
                .min_by_key(|s| match s {
                    0 => 0,
                    3 => 1,
                    2 => 2,
                    _ => 3,
                })
                .expect("two sides")
        };
        let (mut start, end) = if sides.0 == start_side {
            (a, b)
        } else {
            (b, a)
        };
        start.cells.reverse();
        start.cells.extend(end.cells.into_iter().skip(1));
        let first = start.cells[0];
        let last = *start.cells.last().expect("arc has an endpoint");
        let id = arcs.len();
        arcs.push(Some(start));
        endpoints.entry(first).or_default().push(id);
        endpoints.entry(last).or_default().push(id);
    }

    let mut arcs: Vec<CellArc> = arcs.into_iter().flatten().collect();
    arcs.sort_by_key(|arc| {
        let anchor = *arc.cells.iter().max().expect("arc has cells");
        let position = arc.cells.iter().position(|&i| i == anchor).expect("anchor");
        let neighbor = if position + 1 < arc.cells.len() {
            arc.cells[position + 1]
        } else {
            arc.cells[position - 1]
        };
        (anchor, usize::from(anchor % w == neighbor % w))
    });
    let map = |i: usize| {
        let (c, rr) = (i % w, i / w);
        let (c, rr) = (c as f64 + 0.5, rr as f64 + 0.5);
        let t = &r.transform;
        [t[0] + t[1] * c + t[2] * rr, t[3] + t[4] * c + t[5] * rr]
    };
    arcs.into_iter()
        .map(|arc| {
            let value_cell = if arc.cells.len() > 2 {
                arc.cells[1]
            } else {
                arc.cells[0]
            };
            RegionPolyline {
                value: r.bands[0][value_cell],
                from_node: node[arc.cells[0]],
                to_node: node[*arc.cells.last().expect("arc has an endpoint")],
                points: arc.cells.into_iter().map(map).collect(),
            }
        })
        .collect()
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

/// Raster To Float: the cells as little-endian 32-bit floats, NoData -9999, rows from the top.
pub fn to_float(r: &crate::Raster) -> Vec<u8> {
    r.bands[0]
        .iter()
        .flat_map(|&v| {
            let x = if r.is_nodata(v) {
                -9999.0_f32
            } else {
                v as f32
            };
            x.to_le_bytes()
        })
        .collect()
}

/// The `.hdr` beside a `.flt`: the ASCII grid's header and the byte order.
pub fn float_header(r: &crate::Raster) -> String {
    let ascii = to_ascii(r);
    let header: String = ascii
        .split("\r\n")
        .take(6)
        .map(|l| format!("{l}\r\n"))
        .collect();
    format!("{header}{:<14}LSBFIRST\r\n", "byteorder")
}
