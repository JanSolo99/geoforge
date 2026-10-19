//! A planar graph of linework, and the tools built on it: Polygon To Line (IDENTIFY_NEIGHBORS),
//! Feature To Polygon and Feature To Line.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/`, probes in
//! `scripts/research/planar/`):
//!
//! - Cracking: coordinates closer than the cluster distance (2√2 × the XY tolerance, 0.00283 m for
//!   0.001 m) are merged at their mean; a vertex that close to a segment splits it; crossing
//!   segments are split where they cross. Vertices, collinear ones included, are kept.
//! - Row order (all three tools): each output line or polygon by its top-right vertex (highest y,
//!   then highest x), lowest first; several lines with the same top-right vertex by the direction
//!   they leave it, counterclockwise from west (west, south, ..., just north of east); then by
//!   their attributes.
//! - Polygon To Line splits the boundaries at every node (where three or more boundaries meet or
//!   the polygons on either side change) and at the start of every ring. A line runs the way the
//!   lowest-FID polygon that has it on its boundary goes round (that polygon on its right). It
//!   gets one row per (left, right) pair of the polygons on either side — those it bounds and those
//!   it lies inside — with -1 for no polygon. Only `LEFT_FID` and `RIGHT_FID` are kept.
//! - Feature To Polygon makes a polygon of every area enclosed by the linework (dangles and
//!   bridges do not count; areas inside an area become its holes). The outer ring starts at the
//!   vertex after the top-right one (clockwise). Holes (counterclockwise) are tied to the outer ring
//!   by the diagonals of a monotone partition swept upwards, and listed in the order a walk round
//!   the outer ring along those diagonals reaches them, each from the vertex after the one it is
//!   reached at (for a lone hole: after its lowest vertex). With several holes side by side this
//!   order and start do not always match ArcGIS (see `docs/KNOWN_GAPS.md`).
//!   Attributes come from the first label point inside the polygon (a label on a line becomes a
//!   vertex of it and labels nothing); without label
//!   points the polygon inputs give `FID_<name>` (-1) and their fields (null).
//! - Feature To Line splits every line at every crossing, at every point where other lines join
//!   or leave it, and at its ends. Polygon rings count as lines, not split at their start (a ring
//!   with no node keeps its start; ArcGIS sometimes splits a ring once more, see
//!   `docs/KNOWN_GAPS.md`). Where several features share a stretch, it runs the way the first
//!   polygon (else the first line) does, and gets one row per combination of one feature of each input
//!   (-1 for an input with none there).

use std::collections::HashMap;

use geoforge_core::{
    Error, Feature, FeatureClass, Field, FieldType, Geometry, GeometryKind, Result, Value,
};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};

use crate::{NamedInput, ToolOutput, default_xy_tolerance, push_unique};

type Pt = [f64; 2];
/// A line's ordering key: top-right vertex y, x, then the direction it leaves it.
type Key = (f64, f64, f64);
/// Input features, their owners and their paths.
/// An output line: key, FIDs per input, attributes, vertices.
type LineRow = (Key, Vec<i32>, Vec<Value>, Vec<Pt>);
type Loaded = (Vec<Vec<Feature>>, Vec<Owner>, Vec<InPath>);

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// The distance within which ArcGIS merges two coordinates, for an XY tolerance.
fn cluster_distance(tolerance: f64) -> f64 {
    2.0 * std::f64::consts::SQRT_2 * tolerance
}

/// One input path (a polyline part or a polygon ring) with the index of what owns it.
struct InPath {
    pts: Vec<Pt>,
    owner: usize,
}

/// A segment of the cracked linework between two vertices `a < b`, with the paths running along it.
struct Seg {
    a: usize,
    b: usize,
    /// (path, the path runs a → b).
    occ: Vec<(usize, bool)>,
}

struct Graph {
    pts: Vec<Pt>,
    segs: Vec<Seg>,
    /// Vertex → incident segments.
    adj: Vec<Vec<usize>>,
    /// Each input path as graph vertices (no repeats in a row).
    paths: Vec<Vec<usize>>,
    owners: Vec<usize>,
}

/// Points grouped by grid cell, to find those near a location.
struct PointGrid {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl PointGrid {
    fn new(cell: f64) -> Self {
        Self {
            cell: cell.max(1e-12),
            cells: HashMap::new(),
        }
    }
    fn key(&self, p: Pt) -> (i64, i64) {
        (
            (p[0] / self.cell).floor() as i64,
            (p[1] / self.cell).floor() as i64,
        )
    }
    fn insert(&mut self, p: Pt, id: usize) {
        let k = self.key(p);
        self.cells.entry(k).or_default().push(id);
    }
    fn near(&self, p: Pt) -> impl Iterator<Item = usize> + '_ {
        let (cx, cy) = self.key(p);
        (-1..=1).flat_map(move |dx| {
            (-1..=1).flat_map(move |dy| {
                self.cells
                    .get(&(cx + dx, cy + dy))
                    .into_iter()
                    .flatten()
                    .copied()
            })
        })
    }
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Distance from `p` to segment `a`–`b` and the parameter of the closest point.
fn to_segment(p: Pt, a: Pt, b: Pt) -> (f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return (dist(p, a), 0.0);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    (dist(p, [a[0] + t * dx, a[1] + t * dy]), t)
}

/// The parameters (on both segments) where two segments cross, when they do.
fn crossing(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<(f64, f64)> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den == 0.0 {
        return None;
    }
    let q = [c[0] - a[0], c[1] - a[1]];
    let t = (q[0] * s[1] - q[1] * s[0]) / den;
    let u = (q[0] * r[1] - q[1] * r[0]) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

type SegEntry = GeomWithData<Rectangle<Pt>, usize>;

impl Graph {
    /// Cracks the paths with the XY tolerance `tol`.
    fn build(paths: Vec<InPath>, tol: f64) -> Graph {
        let radius = cluster_distance(tol);
        // 1. Merge coordinates within the cluster distance at their mean.
        let mut exact: HashMap<(u64, u64), usize> = HashMap::new();
        let mut distinct: Vec<Pt> = Vec::new();
        let mut path_ids: Vec<Vec<usize>> = Vec::with_capacity(paths.len());
        for p in &paths {
            let ids = p
                .pts
                .iter()
                .map(|q| {
                    let k = (q[0].to_bits(), q[1].to_bits());
                    *exact.entry(k).or_insert_with(|| {
                        distinct.push(*q);
                        distinct.len() - 1
                    })
                })
                .collect();
            path_ids.push(ids);
        }
        let mut parent: Vec<usize> = (0..distinct.len()).collect();
        let mut grid = PointGrid::new(radius);
        for (i, p) in distinct.iter().enumerate() {
            let near: Vec<usize> = grid.near(*p).collect();
            for j in near {
                if dist(*p, distinct[j]) <= radius {
                    let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                    if ri != rj {
                        parent[ri.max(rj)] = ri.min(rj);
                    }
                }
            }
            grid.insert(*p, i);
        }
        let mut cluster_of = vec![usize::MAX; distinct.len()];
        let mut sums: Vec<(f64, f64, f64)> = Vec::new();
        for i in 0..distinct.len() {
            let r = find(&mut parent, i);
            if cluster_of[r] == usize::MAX {
                cluster_of[r] = sums.len();
                sums.push((0.0, 0.0, 0.0));
            }
            let c = cluster_of[r];
            cluster_of[i] = c;
            sums[c].0 += distinct[i][0];
            sums[c].1 += distinct[i][1];
            sums[c].2 += 1.0;
        }
        let mut pts: Vec<Pt> = sums.iter().map(|s| [s.0 / s.2, s.1 / s.2]).collect();
        let mut vgrid = PointGrid::new(radius);
        for (i, p) in pts.iter().enumerate() {
            vgrid.insert(*p, i);
        }
        let mut seqs: Vec<Vec<usize>> = path_ids
            .iter()
            .map(|ids| {
                let mut v: Vec<usize> = Vec::with_capacity(ids.len());
                for &i in ids {
                    let c = cluster_of[i];
                    if v.last() != Some(&c) {
                        v.push(c);
                    }
                }
                v
            })
            .collect();

        // 2. Raw segments, and the points that split them.
        let mut raw: Vec<(usize, usize, usize)> = Vec::new(); // (path, from, to)
        for (pi, s) in seqs.iter().enumerate() {
            for w in s.windows(2) {
                raw.push((pi, w[0], w[1]));
            }
        }
        let env = |a: Pt, b: Pt| {
            Rectangle::from_corners(
                [a[0].min(b[0]) - radius, a[1].min(b[1]) - radius],
                [a[0].max(b[0]) + radius, a[1].max(b[1]) + radius],
            )
        };
        let tree: RTree<SegEntry> = RTree::bulk_load(
            raw.iter()
                .enumerate()
                .map(|(i, &(_, a, b))| GeomWithData::new(env(pts[a], pts[b]), i))
                .collect(),
        );
        let mut splits: Vec<Vec<(f64, usize)>> = vec![Vec::new(); raw.len()];
        for (i, &(_, a, b)) in raw.iter().enumerate() {
            let (pa, pb) = (pts[a], pts[b]);
            let q = AABB::from_corners(
                [pa[0].min(pb[0]) - radius, pa[1].min(pb[1]) - radius],
                [pa[0].max(pb[0]) + radius, pa[1].max(pb[1]) + radius],
            );
            let mut others: Vec<usize> = tree
                .locate_in_envelope_intersecting(&q)
                .map(|e| e.data)
                .filter(|&j| j > i)
                .collect();
            others.sort_unstable();
            for j in others {
                let (_, c, d) = raw[j];
                let (pc, pd) = (pts[c], pts[d]);
                // Vertices of one on the other.
                let mut touched = false;
                for (v, (s, e), seg) in [
                    (c, (a, b), i),
                    (d, (a, b), i),
                    (a, (c, d), j),
                    (b, (c, d), j),
                ] {
                    if v == s || v == e {
                        touched = true;
                        continue;
                    }
                    let (dd, t) = to_segment(pts[v], pts[s], pts[e]);
                    if dd <= radius && t > 0.0 && t < 1.0 {
                        splits[seg].push((t, v));
                        touched = true;
                    }
                }
                if touched {
                    continue;
                }
                if let Some((t, u)) = crossing(pa, pb, pc, pd) {
                    let x = [pa[0] + t * (pb[0] - pa[0]), pa[1] + t * (pb[1] - pa[1])];
                    let hit = vgrid.near(x).find(|&k| dist(pts[k], x) <= radius);
                    let v = match hit {
                        Some(k) => k,
                        None => {
                            pts.push(x);
                            vgrid.insert(x, pts.len() - 1);
                            pts.len() - 1
                        }
                    };
                    if v != a && v != b {
                        splits[i].push((t, v));
                    }
                    if v != c && v != d {
                        splits[j].push((u, v));
                    }
                }
            }
        }

        // Lone points (label points) on a segment split it.
        for s in seqs.iter().filter(|s| s.len() == 1) {
            let v = s[0];
            let pv = pts[v];
            let q = AABB::from_corners(
                [pv[0] - radius, pv[1] - radius],
                [pv[0] + radius, pv[1] + radius],
            );
            for e in tree.locate_in_envelope_intersecting(&q) {
                let (_, a, b) = raw[e.data];
                if v == a || v == b {
                    continue;
                }
                let (dd, t) = to_segment(pv, pts[a], pts[b]);
                if dd <= radius && t > 0.0 && t < 1.0 {
                    splits[e.data].push((t, v));
                }
            }
        }

        // 3. Split, and merge segments with the same ends.
        let mut segs: Vec<Seg> = Vec::new();
        let mut by_ends: HashMap<(usize, usize), usize> = HashMap::new();
        let mut new_seqs: Vec<Vec<usize>> = vec![Vec::new(); seqs.len()];
        for (pi, s) in seqs.iter().enumerate() {
            if let Some(&f) = s.first() {
                new_seqs[pi].push(f);
            }
        }
        for (i, &(pi, a, b)) in raw.iter().enumerate() {
            let mut sp = std::mem::take(&mut splits[i]);
            sp.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut chain = vec![a];
            for (_, v) in sp {
                if chain.last() != Some(&v) && v != b {
                    chain.push(v);
                }
            }
            chain.push(b);
            for w in chain.windows(2) {
                let (u, v) = (w[0], w[1]);
                if u == v {
                    continue;
                }
                let key = (u.min(v), u.max(v));
                let id = *by_ends.entry(key).or_insert_with(|| {
                    segs.push(Seg {
                        a: key.0,
                        b: key.1,
                        occ: Vec::new(),
                    });
                    segs.len() - 1
                });
                segs[id].occ.push((pi, u < v));
                if new_seqs[pi].last() != Some(&v) {
                    new_seqs[pi].push(v);
                }
            }
        }
        seqs = new_seqs;
        let mut adj = vec![Vec::new(); pts.len()];
        for (i, s) in segs.iter().enumerate() {
            adj[s.a].push(i);
            adj[s.b].push(i);
        }
        Graph {
            pts,
            segs,
            adj,
            paths: seqs,
            owners: paths.iter().map(|p| p.owner).collect(),
        }
    }

    fn other(&self, seg: usize, v: usize) -> usize {
        let s = &self.segs[seg];
        if s.a == v { s.b } else { s.a }
    }

    /// The owners along a segment, sorted and without repeats.
    fn owner_set(&self, seg: usize) -> Vec<usize> {
        let mut o: Vec<usize> = self.segs[seg]
            .occ
            .iter()
            .map(|&(p, _)| self.owners[p])
            .collect();
        o.sort_unstable();
        o.dedup();
        o
    }

    /// Vertices where lines meet or part: three or more segments, or a change of owners.
    fn base_nodes(&self) -> Vec<bool> {
        (0..self.pts.len())
            .map(|v| match self.adj[v].as_slice() {
                [s, t] => self.owner_set(*s) != self.owner_set(*t),
                _ => true,
            })
            .collect()
    }

    /// The chains of segments between nodes. Each edge is given as its vertices and segments in
    /// walking order (a closed edge repeats its first vertex at the end). A cycle with no node starts
    /// at its lowest vertex index.
    fn edges(&self, node: &[bool]) -> Vec<Edge> {
        let mut used = vec![false; self.segs.len()];
        let mut out = Vec::new();
        let walk = |start: usize, first: usize, used: &mut [bool]| -> Edge {
            let mut verts = vec![start];
            let mut segs = Vec::new();
            let (mut v, mut s) = (start, first);
            loop {
                used[s] = true;
                let w = self.other(s, v);
                segs.push(s);
                verts.push(w);
                if node[w] || w == start {
                    break;
                }
                match self.adj[w].iter().find(|&&t| !used[t]) {
                    Some(&t) => {
                        v = w;
                        s = t;
                    }
                    None => break,
                }
            }
            Edge { verts, segs }
        };
        for (v, _) in node.iter().enumerate().filter(|(_, n)| **n) {
            for &s in &self.adj[v] {
                if !used[s] {
                    out.push(walk(v, s, &mut used));
                }
            }
        }
        for s in 0..self.segs.len() {
            if !used[s] {
                let v = self.segs[s].a.min(self.segs[s].b);
                out.push(walk(v, s, &mut used));
            }
        }
        out
    }
}

struct Edge {
    verts: Vec<usize>,
    segs: Vec<usize>,
}

impl Edge {
    fn reverse(&mut self) {
        self.verts.reverse();
        self.segs.reverse();
    }
}

/// Angle of the direction `from` → `to` for ordering at a top-right vertex: [π, 2π), west first.
fn leave_angle(from: Pt, to: Pt) -> f64 {
    let a = (to[1] - from[1]).atan2(to[0] - from[0]);
    if a < 0.0 {
        a + 2.0 * std::f64::consts::PI
    } else {
        a
    }
}

/// The ordering key of a line: its top-right vertex, then the direction it leaves it.
fn line_key(pts: &[Pt]) -> (f64, f64, f64) {
    let n = if pts.len() > 1 && pts[0] == pts[pts.len() - 1] {
        pts.len() - 1
    } else {
        pts.len()
    };
    let t = (0..n)
        .max_by(|&i, &j| {
            pts[i][1]
                .total_cmp(&pts[j][1])
                .then(pts[i][0].total_cmp(&pts[j][0]))
        })
        .unwrap_or(0);
    let closed = n < pts.len();
    let mut angle = f64::INFINITY;
    let mut consider = |j: usize| angle = angle.min(leave_angle(pts[t], pts[j]));
    if t + 1 < pts.len() {
        consider(t + 1);
    }
    if t > 0 {
        consider(t - 1);
    } else if closed && n > 1 {
        consider(n - 1);
    }
    (pts[t][1], pts[t][0], angle)
}

fn cmp_key(a: &(f64, f64, f64), b: &(f64, f64, f64)) -> std::cmp::Ordering {
    a.0.total_cmp(&b.0)
        .then(a.1.total_cmp(&b.1))
        .then(a.2.total_cmp(&b.2))
}

fn line_geometry(pts: Vec<Pt>) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polyline;
    g.parts = vec![0];
    g.xy = pts;
    g
}

fn signed_area(r: &[Pt]) -> f64 {
    let n = r.len();
    (0..n)
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

/// Even-odd point in a ring (open or closed vertex list).
fn in_ring(p: Pt, r: &[Pt]) -> bool {
    let n = r.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn part_runs(g: &Geometry) -> Vec<Vec<Pt>> {
    g.part_ranges()
        .map(|r| g.xy[r].to_vec())
        .filter(|r| r.len() >= 2)
        .collect()
}

fn no_curves(g: &Geometry, tool: &str) -> Result<()> {
    if g.has_curves() {
        return Err(err(format!(
            "ERROR 000860: {tool} does not handle curves yet"
        )));
    }
    Ok(())
}

/// A polygon's rings with, for each, whether its interior is on the right of its stored direction.
fn rings_with_sides(g: &Geometry) -> Vec<(Vec<Pt>, bool)> {
    let rings = part_runs(g);
    rings
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let depth = rings
                .iter()
                .enumerate()
                .filter(|(j, o)| *j != i && in_ring(r[0], o))
                .count();
            let cw = signed_area(r) < 0.0;
            let hole = depth % 2 == 1;
            (r.clone(), cw != hole)
        })
        .collect()
}

fn require(input: &dyn FeatureClass, kinds: &[GeometryKind], what: &str) -> Result<()> {
    match input.geometry_kind() {
        Some(k) if kinds.contains(&k) => Ok(()),
        _ => Err(err(format!("ERROR 000365: the input must be {what}"))),
    }
}

fn integer_field(name: &str) -> Field {
    let mut f = Field::new(name, FieldType::Integer);
    f.length = 10;
    f.precision = 10;
    f
}

fn id_field() -> Field {
    let mut f = Field::new("Id", FieldType::Integer);
    f.length = 6;
    f.precision = 6;
    f
}

/// Polygon To Line with IDENTIFY_NEIGHBORS.
pub fn polygon_to_line_neighbors(input: &dyn FeatureClass) -> Result<ToolOutput> {
    require(input, &[GeometryKind::Polygon], "polygons")?;
    let wkt = input.spatial_reference_wkt().map(str::to_owned);
    let tol = default_xy_tolerance(wkt.as_deref());
    let features: Vec<Feature> = input.features().collect::<Result<_>>()?;
    let mut paths = Vec::new();
    let mut right_side = Vec::new();
    let mut polys: Vec<(i32, Vec<Vec<Pt>>)> = Vec::new();
    for f in &features {
        let Some(g) = &f.geometry else { continue };
        no_curves(g, "Polygon To Line")?;
        let owner = polys.len();
        let rings = rings_with_sides(g);
        polys.push((f.oid as i32, rings.iter().map(|r| r.0.clone()).collect()));
        for (r, right) in rings {
            paths.push(InPath { pts: r, owner });
            right_side.push(right);
        }
    }
    let graph = Graph::build(paths, tol);
    let mut node = graph.base_nodes();
    for p in &graph.paths {
        if let Some(&v) = p.first() {
            node[v] = true;
        }
    }
    let boxes: Vec<[f64; 4]> = polys
        .iter()
        .map(|(_, rs)| {
            let mut b = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            for p in rs.iter().flatten() {
                b = [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].max(p[0]),
                    b[3].max(p[1]),
                ];
            }
            b
        })
        .collect();
    let mut rows: Vec<(Key, i32, i32, Vec<Pt>)> = Vec::new();
    for mut e in graph.edges(&node) {
        let seg = &graph.segs[e.segs[0]];
        // Polygons on each side, from the rings along the edge (true: on its right).
        let fwd = e.verts[0] == seg.a;
        let mut bound: Vec<(usize, bool)> = seg
            .occ
            .iter()
            .map(|&(p, f)| (graph.owners[p], (f == fwd) == right_side[p]))
            .collect();
        bound.sort_unstable();
        bound.dedup();
        // The lowest-FID polygon it bounds goes on its right.
        if let Some(&(_, false)) = bound.first() {
            e.reverse();
            for b in &mut bound {
                b.1 = !b.1;
            }
        }
        let (pa, pb) = (graph.pts[e.verts[0]], graph.pts[e.verts[1]]);
        let mid = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0];
        let mut left: Vec<usize> = bound.iter().filter(|b| !b.1).map(|b| b.0).collect();
        let mut right: Vec<usize> = bound.iter().filter(|b| b.1).map(|b| b.0).collect();
        for (k, (_, rings)) in polys.iter().enumerate() {
            let bx = boxes[k];
            if bound.iter().any(|b| b.0 == k)
                || mid[0] < bx[0]
                || mid[0] > bx[2]
                || mid[1] < bx[1]
                || mid[1] > bx[3]
            {
                continue;
            }
            if rings.iter().filter(|r| in_ring(mid, r)).count() % 2 == 1 {
                left.push(k);
                right.push(k);
            }
        }
        let fids = |s: &[usize]| -> Vec<i32> {
            let mut v: Vec<i32> = s.iter().map(|&k| polys[k].0).collect();
            v.sort_unstable();
            v.dedup();
            if v.is_empty() {
                v.push(-1);
            }
            v
        };
        let (lf, rf) = (fids(&left), fids(&right));
        let pts: Vec<Pt> = e.verts.iter().map(|&v| graph.pts[v]).collect();
        let key = line_key(&pts);
        for &l in &lf {
            for &r in &rf {
                rows.push((key, l, r, pts.clone()));
            }
        }
    }
    rows.sort_by(|a, b| cmp_key(&a.0, &b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: vec![integer_field("LEFT_FID"), integer_field("RIGHT_FID")],
        spatial_reference_wkt: wkt,
        features: rows
            .into_iter()
            .enumerate()
            .map(|(i, (_, l, r, pts))| Feature {
                oid: i as i64,
                geometry: Some(line_geometry(pts)),
                attributes: vec![Value::Integer(l), Value::Integer(r)],
            })
            .collect(),
        messages: Vec::new(),
    })
}

/// What the tools take from each input feature: which input, its FID and whether it is a polygon.
struct Owner {
    input: usize,
    fid: i32,
    feature: usize,
    polygon: bool,
}

fn load_inputs(inputs: &[NamedInput], tool: &str) -> Result<Loaded> {
    let mut all = Vec::new();
    let mut owners = Vec::new();
    let mut paths = Vec::new();
    for (k, input) in inputs.iter().enumerate() {
        require(
            input.features,
            &[GeometryKind::Polyline, GeometryKind::Polygon],
            "lines or polygons",
        )?;
        let polygon = input.features.geometry_kind() == Some(GeometryKind::Polygon);
        let features: Vec<Feature> = input.features.features().collect::<Result<_>>()?;
        for (i, f) in features.iter().enumerate() {
            let Some(g) = &f.geometry else { continue };
            no_curves(g, tool)?;
            let owner = owners.len();
            owners.push(Owner {
                input: k,
                fid: f.oid as i32,
                feature: i,
                polygon,
            });
            for r in part_runs(g) {
                paths.push(InPath { pts: r, owner });
            }
        }
        all.push(features);
    }
    Ok((all, owners, paths))
}

fn tolerance(inputs: &[NamedInput], cluster_tolerance: Option<f64>) -> Result<f64> {
    if let Some(t) = cluster_tolerance {
        if !(t.is_finite() && t >= 0.0) {
            return Err(err(
                "ERROR 000800: the cluster tolerance must be zero or more",
            ));
        }
        if t > 0.0 {
            return Ok(t);
        }
    }
    Ok(default_xy_tolerance(
        inputs
            .first()
            .and_then(|i| i.features.spatial_reference_wkt()),
    ))
}

/// Feature To Line (`attributes`: ATTRIBUTES or NO_ATTRIBUTES).
pub fn feature_to_line(
    inputs: &[NamedInput],
    cluster_tolerance: Option<f64>,
    attributes: bool,
) -> Result<ToolOutput> {
    if inputs.is_empty() {
        return Err(err("ERROR 000735: Input Features: value is required"));
    }
    let tol = tolerance(inputs, cluster_tolerance)?;
    let (features, owners, paths) = load_inputs(inputs, "Feature To Line")?;
    let path_owner: Vec<usize> = paths.iter().map(|p| p.owner).collect();
    let graph = Graph::build(paths, tol);
    let mut node = graph.base_nodes();
    // Line ends are nodes.
    for (pi, p) in graph.paths.iter().enumerate() {
        if !owners[path_owner[pi]].polygon
            && let (Some(&a), Some(&b)) = (p.first(), p.last())
        {
            node[a] = true;
            node[b] = true;
        }
    }
    // A ring without nodes keeps its start (a ring with nodes is not split at its start).
    for (pi, p) in graph.paths.iter().enumerate() {
        if owners[path_owner[pi]].polygon && p.len() > 1 && !p.iter().any(|&v| node[v]) {
            node[p[0]] = true;
        }
    }
    // Fields.
    let mut fields: Vec<Field> = Vec::new();
    if attributes {
        for input in inputs {
            push_unique(&mut fields, integer_field(&format!("FID_{}", input.name)));
            for f in input.features.fields() {
                push_unique(&mut fields, f.clone());
            }
        }
    } else {
        fields.push(id_field());
    }
    let mut rows: Vec<LineRow> = Vec::new();
    for mut e in graph.edges(&node) {
        let first = e.segs[0];
        let seg = &graph.segs[first];
        // The direction of the first polygon, else the first line, running along the edge.
        let lead = seg
            .occ
            .iter()
            .min_by_key(|&&(p, _)| {
                let o = &owners[path_owner[p]];
                (!o.polygon, o.input, o.fid, p)
            })
            .copied();
        if let Some((_, fwd)) = lead {
            let runs_ab = e.verts[0] == seg.a;
            if fwd != runs_ab {
                e.reverse();
            }
        }
        let pts: Vec<Pt> = e.verts.iter().map(|&v| graph.pts[v]).collect();
        let key = line_key(&pts);
        if !attributes {
            rows.push((key, Vec::new(), vec![Value::Integer(0)], pts));
            continue;
        }
        // One row per combination of one feature of each input.
        let mut per_input: Vec<Vec<usize>> = vec![Vec::new(); inputs.len()];
        for &(p, _) in &seg.occ {
            let o = path_owner[p];
            per_input[owners[o].input].push(o);
        }
        for v in &mut per_input {
            v.sort_by_key(|&o| owners[o].fid);
            v.dedup();
        }
        let mut combos: Vec<Vec<Option<usize>>> = vec![Vec::new()];
        for v in &per_input {
            let choices: Vec<Option<usize>> = if v.is_empty() {
                vec![None]
            } else {
                v.iter().map(|&o| Some(o)).collect()
            };
            combos = combos
                .into_iter()
                .flat_map(|c| {
                    choices.iter().map(move |x| {
                        let mut c = c.clone();
                        c.push(*x);
                        c
                    })
                })
                .collect();
        }
        for c in combos {
            let mut attrs = Vec::with_capacity(fields.len());
            let mut fids = Vec::with_capacity(inputs.len());
            for (k, o) in c.iter().enumerate() {
                let n = inputs[k].features.fields().len();
                match o {
                    Some(o) => {
                        let ow = &owners[*o];
                        fids.push(ow.fid);
                        attrs.push(Value::Integer(ow.fid));
                        attrs.extend(features[k][ow.feature].attributes.iter().cloned());
                    }
                    None => {
                        fids.push(-1);
                        attrs.push(Value::Integer(-1));
                        attrs.extend(std::iter::repeat_n(Value::Null, n));
                    }
                }
            }
            rows.push((key, fids, attrs, pts.clone()));
        }
    }
    rows.sort_by(|a, b| cmp_key(&a.0, &b.0).then(a.1.cmp(&b.1)));
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: inputs[0]
            .features
            .spatial_reference_wkt()
            .map(str::to_owned),
        features: rows
            .into_iter()
            .enumerate()
            .map(|(i, (_, _, attrs, pts))| Feature {
                oid: i as i64,
                geometry: Some(line_geometry(pts)),
                attributes: attrs,
            })
            .collect(),
        messages: Vec::new(),
    })
}

/// The bounded faces of the linework: (outer ring, holes), each ring as an open vertex list in
/// ArcGIS's stored order (outer clockwise, holes counterclockwise) and start.
fn faces(graph: &Graph) -> Vec<(Vec<Pt>, Vec<Vec<Pt>>)> {
    let n = graph.pts.len();
    let mut alive = vec![true; graph.segs.len()];
    let mut degree: Vec<usize> = graph.adj.iter().map(Vec::len).collect();
    let prune = |alive: &mut Vec<bool>, degree: &mut Vec<usize>| {
        let mut stack: Vec<usize> = (0..n).filter(|&v| degree[v] == 1).collect();
        while let Some(v) = stack.pop() {
            if degree[v] != 1 {
                continue;
            }
            if let Some(&s) = graph.adj[v].iter().find(|&&s| alive[s]) {
                alive[s] = false;
                degree[v] -= 1;
                let w = graph.other(s, v);
                degree[w] -= 1;
                if degree[w] == 1 {
                    stack.push(w);
                }
            }
        }
    };
    prune(&mut alive, &mut degree);
    loop {
        // Half-edges: 2s is a → b, 2s+1 is b → a. At each vertex the live segments in
        // counterclockwise order.
        let order: Vec<Vec<usize>> = (0..n)
            .map(|v| {
                let mut s: Vec<usize> =
                    graph.adj[v].iter().copied().filter(|&s| alive[s]).collect();
                let p = graph.pts[v];
                s.sort_by(|&x, &y| {
                    let (qx, qy) = (graph.pts[graph.other(x, v)], graph.pts[graph.other(y, v)]);
                    let ax = (qx[1] - p[1]).atan2(qx[0] - p[0]);
                    let ay = (qy[1] - p[1]).atan2(qy[0] - p[0]);
                    ax.total_cmp(&ay)
                });
                s
            })
            .collect();
        let head = |h: usize| {
            let s = &graph.segs[h / 2];
            if h.is_multiple_of(2) {
                (s.a, s.b)
            } else {
                (s.b, s.a)
            }
        };
        let mut cycle_of = vec![usize::MAX; graph.segs.len() * 2];
        let mut cycles: Vec<Vec<usize>> = Vec::new();
        for h0 in 0..graph.segs.len() * 2 {
            if !alive[h0 / 2] || cycle_of[h0] != usize::MAX {
                continue;
            }
            let id = cycles.len();
            let mut c = Vec::new();
            let mut h = h0;
            loop {
                cycle_of[h] = id;
                c.push(h);
                let v = head(h).1;
                // Next: the first segment counterclockwise from v → u.
                let ord = &order[v];
                let back = h / 2;
                let i = ord.iter().position(|&s| s == back).unwrap_or(0);
                let s = ord[(i + 1) % ord.len()];
                let nh = if graph.segs[s].a == v {
                    2 * s
                } else {
                    2 * s + 1
                };
                h = nh;
                if h == h0 || cycle_of[h] != usize::MAX {
                    break;
                }
            }
            cycles.push(c);
        }
        // Bridges: both directions in one cycle.
        let mut bridges = false;
        for s in 0..graph.segs.len() {
            if alive[s] && cycle_of[2 * s] == cycle_of[2 * s + 1] {
                alive[s] = false;
                degree[graph.segs[s].a] -= 1;
                degree[graph.segs[s].b] -= 1;
                bridges = true;
            }
        }
        if bridges {
            prune(&mut alive, &mut degree);
            continue;
        }
        let rings: Vec<Vec<Pt>> = cycles
            .iter()
            .map(|c| c.iter().map(|&h| graph.pts[head(h).0]).collect())
            .collect();
        let areas: Vec<f64> = rings.iter().map(|r| signed_area(r)).collect();
        let outer: Vec<usize> = (0..rings.len()).filter(|&i| areas[i] < 0.0).collect();
        // Connected parts of the linework: a hole lies in a face of another part.
        let mut part: Vec<usize> = (0..n).collect();
        for (s, seg) in graph.segs.iter().enumerate() {
            if alive[s] {
                let (a, b) = (find(&mut part, seg.a), find(&mut part, seg.b));
                part[a.max(b)] = a.min(b);
            }
        }
        let part_of: Vec<usize> = cycles
            .iter()
            .map(|c| find(&mut part, head(c[0]).0))
            .collect();
        let mut holes: Vec<Vec<usize>> = vec![Vec::new(); rings.len()];
        for h in (0..rings.len()).filter(|&i| areas[i] > 0.0) {
            let p = rings[h][0];
            let host = outer
                .iter()
                .copied()
                .filter(|&o| part_of[o] != part_of[h] && in_ring(p, &rings[o]))
                .min_by(|&a, &b| areas[b].total_cmp(&areas[a]));
            if let Some(o) = host {
                holes[o].push(h);
            }
        }
        let mut out = Vec::new();
        for &o in &outer {
            let hs: Vec<&[Pt]> = holes[o].iter().map(|&h| rings[h].as_slice()).collect();
            out.push(place_holes(&rings[o], &hs));
        }
        return out;
    }
}

fn top_right(r: &[Pt]) -> usize {
    (0..r.len())
        .max_by(|&i, &j| {
            r[i][1]
                .total_cmp(&r[j][1])
                .then(r[i][0].total_cmp(&r[j][0]))
        })
        .unwrap_or(0)
}

fn lowest(r: &[Pt]) -> usize {
    (0..r.len())
        .min_by(|&i, &j| {
            r[i][1]
                .total_cmp(&r[j][1])
                .then(r[i][0].total_cmp(&r[j][0]))
        })
        .unwrap_or(0)
}

fn rotate_after_top_right(r: &[Pt]) -> Vec<Pt> {
    let t = top_right(r);
    let k = r.len();
    (0..k).map(|i| r[(t + 1 + i) % k]).collect()
}

/// A vertex of a face: (ring, index); ring 0 is the outer ring.
type V = (usize, usize);

/// The rings of one face in ArcGIS's stored order: the outer ring from the vertex after its
/// top-right one, the holes in the order a walk round the face reaches them, each from the vertex
/// after the one where it is reached.
///
/// The holes are tied to the outer ring by the diagonals of a monotone partition (sweeping up,
/// vertices by y then x: a hole's lowest vertex joins the helper of the boundary on its left, its
/// top-right vertex becomes that boundary's helper and is joined by the next vertex that has it on
/// its left). The walk follows the outer ring from its start; at each vertex it takes the
/// diagonals there (counterclockwise from where it came) to holes not yet reached, walks each such
/// hole from the vertex after the one it entered by (doing the same), and goes on.
fn place_holes(outer: &[Pt], holes: &[&[Pt]]) -> (Vec<Pt>, Vec<Vec<Pt>>) {
    let outer_start = rotate_after_top_right(outer);
    if holes.is_empty() {
        return (outer_start, Vec::new());
    }
    let rings: Vec<&[Pt]> = std::iter::once(outer)
        .chain(holes.iter().copied())
        .collect();
    let at = |v: V| rings[v.0][v.1];
    let len = |r: usize| rings[r].len();
    let prev = |v: V| (v.0, (v.1 + len(v.0) - 1) % len(v.0));
    let next = |v: V| (v.0, (v.1 + 1) % len(v.0));
    let before = |a: Pt, b: Pt| a[1] < b[1] || (a[1] == b[1] && a[0] < b[0]);
    let mut events: Vec<V> = (0..rings.len())
        .flat_map(|r| (0..len(r)).map(move |i| (r, i)))
        .collect();
    events.sort_by(|&a, &b| {
        let (p, q) = (at(a), at(b));
        p[1].total_cmp(&q[1]).then(p[0].total_cmp(&q[0]))
    });
    #[derive(Clone, Copy, PartialEq)]
    enum T {
        Start,
        Split,
        End,
        Merge,
        Regular,
    }
    let kind = |v: V| {
        let (a, b, c) = (at(prev(v)), at(v), at(next(v)));
        let turn = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        let (pa, nb) = (before(b, a), before(b, c));
        match (pa, nb) {
            (true, true) if turn < 0.0 => T::Start,
            (true, true) => T::Split,
            (false, false) if turn < 0.0 => T::End,
            (false, false) => T::Merge,
            _ => T::Regular,
        }
    };
    // Active edges (by their first vertex) with their helpers.
    let mut active: Vec<(V, V)> = Vec::new();
    let mut diagonals: Vec<(V, V)> = Vec::new();
    let x_at = |e: V, y: f64| {
        let (a, b) = (at(e), at(next(e)));
        if a[1] == b[1] {
            a[0].max(b[0])
        } else {
            a[0] + (y - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
        }
    };
    let left_of = |active: &[(V, V)], v: V| -> Option<usize> {
        let p = at(v);
        active
            .iter()
            .enumerate()
            .filter(|(_, (e, _))| *e != v && next(*e) != v)
            .map(|(i, (e, _))| (i, x_at(*e, p[1])))
            .filter(|(_, x)| *x <= p[0])
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let is_merge = |v: V| kind(v) == T::Merge;
    for &v in &events {
        let pv = prev(v);
        let take_prev = |active: &mut Vec<(V, V)>, diagonals: &mut Vec<(V, V)>| {
            if let Some(i) = active.iter().position(|(e, _)| *e == pv) {
                let h = active[i].1;
                if is_merge(h) {
                    diagonals.push((v, h));
                }
                active.remove(i);
            }
        };
        match kind(v) {
            T::Start => active.push((v, v)),
            T::End => take_prev(&mut active, &mut diagonals),
            T::Split => {
                if let Some(j) = left_of(&active, v) {
                    diagonals.push((v, active[j].1));
                    active[j].1 = v;
                }
                active.push((v, v));
            }
            T::Merge => {
                take_prev(&mut active, &mut diagonals);
                if let Some(j) = left_of(&active, v) {
                    if is_merge(active[j].1) {
                        diagonals.push((v, active[j].1));
                    }
                    active[j].1 = v;
                }
            }
            T::Regular => {
                if before(at(pv), at(v)) {
                    take_prev(&mut active, &mut diagonals);
                    active.push((v, v));
                } else if let Some(j) = left_of(&active, v) {
                    if is_merge(active[j].1) {
                        diagonals.push((v, active[j].1));
                    }
                    active[j].1 = v;
                }
            }
        }
    }
    let mut links: HashMap<V, Vec<V>> = HashMap::new();
    for &(a, b) in &diagonals {
        if a.0 != b.0 {
            links.entry(a).or_default().push(b);
            links.entry(b).or_default().push(a);
        }
    }
    // The walk.
    let mut reached = vec![false; rings.len()];
    reached[0] = true;
    let mut order: Vec<(usize, usize)> = Vec::new(); // (hole ring, entry index)
    let mut stack: Vec<(usize, usize, usize)> = vec![(0, top_right(outer), 0)]; // (ring, entry, step)
    while let Some(&mut (r, entry, ref mut step)) = stack.last_mut() {
        let n = len(r);
        if *step >= n {
            stack.pop();
            continue;
        }
        let v = (r, (entry + 1 + *step) % n);
        *step += 1;
        let Some(targets) = links.get(&v) else {
            continue;
        };
        let from = at(prev(v));
        let here = at(v);
        let back = (from[1] - here[1]).atan2(from[0] - here[0]);
        let mut ts: Vec<(f64, V)> = targets
            .iter()
            .map(|&t| {
                let q = at(t);
                let a = (q[1] - here[1]).atan2(q[0] - here[0]) - back;
                (a.rem_euclid(2.0 * std::f64::consts::PI), t)
            })
            .collect();
        ts.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Holes are walked one at a time: push the first unreached one; the others are taken
        // when the walk comes back to this vertex.
        if let Some(&(_, t)) = ts.iter().find(|(_, t)| !reached[t.0]) {
            reached[t.0] = true;
            order.push((t.0, t.1));
            if let Some(top) = stack.last_mut() {
                top.2 -= 1; // revisit this vertex for its other diagonals
            }
            stack.push((t.0, t.1, 0));
        }
    }
    for (r, done) in reached.iter().enumerate() {
        if !done {
            order.push((r, lowest(rings[r])));
        }
    }
    let holes_out = order
        .into_iter()
        .map(|(r, l)| {
            let k = len(r);
            (0..k).map(|i| rings[r][(l + 1 + i) % k]).collect()
        })
        .collect();
    (outer_start, holes_out)
}

/// Feature To Polygon. `labels`: optional label points whose attributes the polygons take.
pub fn feature_to_polygon(
    inputs: &[NamedInput],
    cluster_tolerance: Option<f64>,
    attributes: bool,
    labels: Option<&dyn FeatureClass>,
) -> Result<ToolOutput> {
    if inputs.is_empty() {
        return Err(err("ERROR 000735: Input Features: value is required"));
    }
    let tol = tolerance(inputs, cluster_tolerance)?;
    let (_, _, mut paths) = load_inputs(inputs, "Feature To Polygon")?;
    // Label points on the linework become vertices of it (with NO_ATTRIBUTES they are not used).
    if let Some(l) = labels.filter(|_| attributes) {
        for f in l.features() {
            if let Some(g) = &f?.geometry
                && let Some(p) = g.xy.first()
            {
                paths.push(InPath {
                    pts: vec![*p],
                    owner: usize::MAX,
                });
            }
        }
    }
    let graph = Graph::build(paths, tol);
    let mut found = faces(&graph);
    found.sort_by(|a, b| {
        let ka = face_key(&a.0);
        let kb = face_key(&b.0);
        cmp_key(&ka, &kb)
    });
    // Fields and the label points.
    let mut fields: Vec<Field> = Vec::new();
    let mut is_fid: Vec<bool> = Vec::new();
    let mut label_points: Vec<(Pt, Vec<Value>)> = Vec::new();
    match labels {
        Some(l) => {
            if l.geometry_kind() != Some(GeometryKind::Point) {
                return Err(err("ERROR 000365: the label features must be points"));
            }
            if attributes {
                fields = l.fields().to_vec();
            }
            for f in l.features() {
                let f = f?;
                if let Some(g) = &f.geometry
                    && let Some(p) = g.xy.first()
                {
                    label_points.push((*p, f.attributes));
                }
            }
        }
        None => {
            for input in inputs {
                if input.features.geometry_kind() == Some(GeometryKind::Polygon) {
                    push_unique(&mut fields, integer_field(&format!("FID_{}", input.name)));
                    is_fid.push(true);
                    for f in input.features.fields() {
                        push_unique(&mut fields, f.clone());
                        is_fid.push(false);
                    }
                }
            }
        }
    }
    let empty_fields = fields.is_empty();
    if empty_fields {
        fields.push(id_field());
    }
    let near_line = |p: Pt| {
        graph
            .segs
            .iter()
            .any(|s| to_segment(p, graph.pts[s.a], graph.pts[s.b]).0 <= tol)
    };
    let mut features = Vec::new();
    for (outer, holes) in found {
        let attrs: Vec<Value> = if empty_fields {
            vec![Value::Integer(0)]
        } else if labels.is_some() {
            let hit = label_points.iter().find(|(p, _)| {
                in_ring(*p, &outer) && !holes.iter().any(|h| in_ring(*p, h)) && !near_line(*p)
            });
            match hit {
                Some((_, a)) => a.clone(),
                None => vec![Value::Null; fields.len()],
            }
        } else {
            is_fid
                .iter()
                .map(|&fid| if fid { Value::Integer(-1) } else { Value::Null })
                .collect()
        };
        let mut g = Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Polygon;
        g.parts = Vec::new();
        g.xy = Vec::new();
        for r in std::iter::once(&outer).chain(&holes) {
            g.parts.push(g.xy.len() as u32);
            g.xy.extend(r.iter().copied());
            g.xy.push(r[0]);
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(g),
            attributes: attrs,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: inputs[0]
            .features
            .spatial_reference_wkt()
            .map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// A face's order: its top-right vertex (the last of its ring), then the direction it leaves it.
fn face_key(ring: &[Pt]) -> (f64, f64, f64) {
    let t = ring[ring.len() - 1];
    (t[1], t[0], leave_angle(t, ring[0]))
}
