//! Check Geometry and Repair Geometry: the problems ArcGIS reports for a shape
//! under the Esri or the OGC simple-feature rules, and its repair.
//!
//! Worked out against ArcGIS Pro 3.7 (black box) with the XY tolerance `tol`
//! (0.001 m for a shapefile in meters). ArcGIS reports one problem per feature,
//! the first of these that applies:
//!
//! - null geometry: no shape, or a line without a part of two or more points;
//! - unclosed rings;
//! - (OGC) non simple: a ring of fewer than four points, a one-point line part;
//! - short segments: a zero-length segment; under OGC any segment up to `tol`;
//! - (OGC) duplicate vertex: a segment up to 2√2·`tol` (two vertices that share
//!   a tolerance cell);
//! - (Esri) empty parts: a one-point line part;
//! - self intersections: segments that cross, overlap, or come within √2·`tol`
//!   of each other without sharing a vertex; for polygons under the Esri rules
//!   also any segment up to 2√2·`tol`. Lines only under OGC, where two parts may
//!   only meet at their ends;
//! - incorrect ring ordering: a clockwise ring at an odd depth or a
//!   counter-clockwise one at an even depth;
//! - (OGC) non simple: a ring that touches itself at a vertex.
//!
//! Repair closes rings, drops zero-length segments and one-point parts, merges
//! vertices closer than 2√2·`tol` (polygons: at their midpoint; OGC lines: the
//! first stays) and pulls a vertex within √2·`tol` of a segment to the midpoint
//! between them. Polygons are then rebuilt with the even-odd rule (Esri counts a
//! ring repeated exactly once, OGC cancels it out), OGC lines are split where
//! they meet.

use std::collections::HashMap;

use geoforge_core::{Geometry, GeometryKind, Result};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::ContourDirection;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::{FloatOverlay, OverlayOptions};

type Pt = [f64; 2];

/// Which simple-feature rules to check against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMethod {
    Esri,
    Ogc,
}

/// A problem Check Geometry reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    NullGeometry,
    UnclosedRings,
    NonSimple,
    ShortSegments,
    DuplicateVertex,
    EmptyParts,
    SelfIntersections,
    IncorrectRingOrdering,
}

impl Problem {
    /// The `PROBLEM` text of Check Geometry's output table.
    pub fn description(self) -> &'static str {
        match self {
            Problem::NullGeometry => "null geometry",
            Problem::UnclosedRings => "unclosed rings",
            Problem::NonSimple => "non simple",
            Problem::ShortSegments => "short segments",
            Problem::DuplicateVertex => "Duplicate vertex",
            Problem::EmptyParts => "empty parts",
            Problem::SelfIntersections => "self intersections",
            Problem::IncorrectRingOrdering => "incorrect ring ordering",
        }
    }

    /// The reason Repair Geometry gives ("Repaired feature N because of ...").
    pub fn repair_reason(self) -> &'static str {
        match self {
            Problem::NonSimple => "Non simple.",
            Problem::DuplicateVertex => "Duplicate vertex.",
            p => p.description(),
        }
    }
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Distance from `p` to segment `a`–`b`, and the closest point's parameter.
fn to_segment(p: Pt, a: Pt, b: Pt) -> (f64, f64) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
    };
    (dist(p, [a[0] + t * d[0], a[1] + t * d[1]]), t)
}

fn cross(o: Pt, a: Pt, b: Pt) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// Where segments `a` and `b` cross (parameters on each), if they do.
fn crossing(a: (Pt, Pt), b: (Pt, Pt)) -> Option<(f64, f64)> {
    let da = [a.1[0] - a.0[0], a.1[1] - a.0[1]];
    let db = [b.1[0] - b.0[0], b.1[1] - b.0[1]];
    let den = da[0] * db[1] - da[1] * db[0];
    if den == 0.0 {
        return None;
    }
    let w = [b.0[0] - a.0[0], b.0[1] - a.0[1]];
    let t = (w[0] * db[1] - w[1] * db[0]) / den;
    let u = (w[0] * da[1] - w[1] * da[0]) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

fn segment_distance(a: (Pt, Pt), b: (Pt, Pt)) -> f64 {
    if crossing(a, b).is_some() {
        return 0.0;
    }
    [
        to_segment(a.0, b.0, b.1).0,
        to_segment(a.1, b.0, b.1).0,
        to_segment(b.0, a.0, a.1).0,
        to_segment(b.1, a.0, a.1).0,
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

fn parts_of(g: &Geometry) -> Vec<Vec<Pt>> {
    g.part_ranges().map(|r| g.xy[r].to_vec()).collect()
}

/// Shoelace area: negative for a clockwise ring.
fn signed_area(ring: &[Pt]) -> f64 {
    ring.windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum::<f64>()
        / 2.0
}

fn on_boundary(p: Pt, ring: &[Pt]) -> bool {
    ring.windows(2).any(|w| to_segment(p, w[0], w[1]).0 == 0.0)
}

/// Even-odd point in closed ring.
fn inside(p: Pt, ring: &[Pt]) -> bool {
    let mut c = false;
    for w in ring.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                c = !c;
            }
        }
    }
    c
}

/// A segment of a part, for the pairwise tests.
#[derive(Clone, Copy)]
struct Seg {
    a: Pt,
    b: Pt,
    part: usize,
    idx: usize,
}

fn segments(parts: &[Vec<Pt>]) -> Vec<Seg> {
    let mut out = Vec::new();
    for (part, p) in parts.iter().enumerate() {
        for (idx, w) in p.windows(2).enumerate() {
            out.push(Seg {
                a: w[0],
                b: w[1],
                part,
                idx,
            });
        }
    }
    out
}

/// Pairs of segments whose envelopes come within `pad` (sweep over x).
fn near_pairs(segs: &[Seg], pad: f64, mut f: impl FnMut(&Seg, &Seg) -> bool) -> bool {
    let mut order: Vec<usize> = (0..segs.len()).collect();
    let minx = |s: &Seg| s.a[0].min(s.b[0]);
    order.sort_by(|&i, &j| minx(&segs[i]).total_cmp(&minx(&segs[j])));
    for (k, &i) in order.iter().enumerate() {
        let s = &segs[i];
        let (x1, y0, y1) = (s.a[0].max(s.b[0]), s.a[1].min(s.b[1]), s.a[1].max(s.b[1]));
        for &j in &order[k + 1..] {
            let t = &segs[j];
            if minx(t) > x1 + pad {
                break;
            }
            if t.a[1].min(t.b[1]) > y1 + pad || t.a[1].max(t.b[1]) < y0 - pad {
                continue;
            }
            let (p, q) = if i < j { (s, t) } else { (t, s) };
            if f(p, q) {
                return true;
            }
        }
    }
    false
}

/// Whether the outline crosses, overlaps or nearly touches itself. Segments may
/// share a vertex exactly; for lines only where it ends a part for both (or the
/// two follow each other along a part).
fn self_intersects(parts: &[Vec<Pt>], polygon: bool, tol: f64) -> bool {
    let near = std::f64::consts::SQRT_2 * tol;
    let segs = segments(parts);
    let part_end = |s: &Seg, v: Pt| {
        let p = &parts[s.part];
        (s.idx == 0 && v == s.a) || (s.idx + 2 == p.len() && v == s.b)
    };
    near_pairs(&segs, near, |s, t| {
        let shared = [s.a, s.b].into_iter().find(|&v| v == t.a || v == t.b);
        let Some(v) = shared else {
            return segment_distance((s.a, s.b), (t.a, t.b)) <= near;
        };
        // Touching at a common vertex: no overlap beyond it.
        let far_s = if s.a == v { s.b } else { s.a };
        let far_t = if t.a == v { t.b } else { t.a };
        if to_segment(far_s, t.a, t.b).0 <= near || to_segment(far_t, s.a, s.b).0 <= near {
            return true;
        }
        if polygon {
            return false;
        }
        let consecutive = s.part == t.part && s.idx.abs_diff(t.idx) == 1;
        !consecutive && !(part_end(s, v) && part_end(t, v))
    })
}

/// Rings must alternate: clockwise at an even nesting depth, counter-clockwise
/// at an odd one.
fn ring_order_ok(rings: &[Vec<Pt>]) -> bool {
    rings.iter().enumerate().all(|(i, ring)| {
        let depth = rings
            .iter()
            .enumerate()
            .filter(|&(j, other)| {
                if i == j {
                    return false;
                }
                let probe = ring
                    .iter()
                    .copied()
                    .chain(
                        ring.windows(2)
                            .map(|w| [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0]),
                    )
                    .find(|&p| !on_boundary(p, other));
                probe.is_some_and(|p| inside(p, other))
            })
            .count();
        (signed_area(ring) < 0.0) == (depth % 2 == 0)
    })
}

fn touches_itself(ring: &[Pt]) -> bool {
    let mut v: Vec<Pt> = ring[..ring.len() - 1].to_vec();
    v.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    v.windows(2).any(|w| w[0] == w[1])
}

fn has_zero_length(parts: &[Vec<Pt>]) -> bool {
    parts.iter().any(|p| p.windows(2).any(|w| w[0] == w[1]))
}

fn shortest(parts: &[Vec<Pt>]) -> f64 {
    parts
        .iter()
        .flat_map(|p| p.windows(2).map(|w| dist(w[0], w[1])))
        .fold(f64::INFINITY, f64::min)
}

/// The problem ArcGIS's Check Geometry reports for a shape, if any.
pub fn check(g: Option<&Geometry>, method: ValidationMethod, tol: f64) -> Option<Problem> {
    let ogc = method == ValidationMethod::Ogc;
    let Some(g) = g.filter(|g| !g.xy.is_empty()) else {
        return Some(Problem::NullGeometry);
    };
    // Curves are taken as valid: they have no segments to compare.
    if g.has_curves() {
        return None;
    }
    let cell = 2.0 * std::f64::consts::SQRT_2 * tol;
    match g.kind {
        GeometryKind::Polygon => {
            let rings: Vec<Vec<Pt>> = parts_of(g).into_iter().filter(|r| !r.is_empty()).collect();
            if rings.is_empty() {
                return Some(Problem::NullGeometry);
            }
            if rings.iter().any(|r| r[0] != r[r.len() - 1]) {
                return Some(Problem::UnclosedRings);
            }
            if ogc && rings.iter().any(|r| r.len() < 4) {
                return Some(Problem::NonSimple);
            }
            if has_zero_length(&rings) {
                return Some(Problem::ShortSegments);
            }
            let short = shortest(&rings);
            if ogc && short <= tol {
                return Some(Problem::ShortSegments);
            }
            if short <= cell {
                return Some(if ogc {
                    Problem::DuplicateVertex
                } else {
                    Problem::SelfIntersections
                });
            }
            if rings.iter().any(|r| signed_area(r) == 0.0) || self_intersects(&rings, true, tol) {
                return Some(Problem::SelfIntersections);
            }
            if !ring_order_ok(&rings) {
                return Some(Problem::IncorrectRingOrdering);
            }
            if ogc && rings.iter().any(|r| touches_itself(r)) {
                return Some(Problem::NonSimple);
            }
            None
        }
        GeometryKind::Polyline => {
            let parts = parts_of(g);
            if parts.iter().all(|p| p.len() < 2) {
                return Some(Problem::NullGeometry);
            }
            let lonely = parts.iter().any(|p| p.len() < 2);
            if !ogc {
                if has_zero_length(&parts) {
                    return Some(Problem::ShortSegments);
                }
                return lonely.then_some(Problem::EmptyParts);
            }
            if lonely {
                return Some(Problem::NonSimple);
            }
            let short = shortest(&parts);
            if short <= tol {
                return Some(Problem::ShortSegments);
            }
            if short <= cell {
                return Some(Problem::DuplicateVertex);
            }
            self_intersects(&parts, false, tol).then_some(Problem::SelfIntersections)
        }
        _ => None,
    }
}

/// Merges vertices closer than `cell`: polygons (cyclic) at their midpoint,
/// lines keep the first.
fn merge_close(path: &[Pt], cell: f64, cyclic: bool) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(path.len());
    for &p in path {
        match out.last_mut() {
            Some(last) if dist(*last, p) <= cell => {
                if cyclic {
                    *last = [(last[0] + p[0]) / 2.0, (last[1] + p[1]) / 2.0];
                }
            }
            _ => out.push(p),
        }
    }
    if cyclic {
        while out.len() > 1 && dist(out[0], out[out.len() - 1]) <= cell {
            let last = out.pop().unwrap();
            out[0] = [(out[0][0] + last[0]) / 2.0, (out[0][1] + last[1]) / 2.0];
        }
    }
    out
}

/// Pulls each vertex within √2·`tol` of a segment it is not an end of to the
/// midpoint between them, adding that point to the segment as a vertex.
fn snap_to_segments(parts: &mut [Vec<Pt>], tol: f64) {
    let near = std::f64::consts::SQRT_2 * tol;
    let segs = segments(parts);
    // (part, vertex) → new position; (part, segment) → points to insert.
    let mut moved: HashMap<(usize, usize), Pt> = HashMap::new();
    let mut inserts: HashMap<(usize, usize), Vec<(f64, Pt)>> = HashMap::new();
    for (pi, p) in parts.iter().enumerate() {
        for (vi, &v) in p.iter().enumerate() {
            for s in &segs {
                if v == s.a || v == s.b {
                    continue;
                }
                let (d, t) = to_segment(v, s.a, s.b);
                if d > near || t <= 0.0 || t >= 1.0 {
                    continue;
                }
                let q = [
                    s.a[0] + t * (s.b[0] - s.a[0]),
                    s.a[1] + t * (s.b[1] - s.a[1]),
                ];
                let m = [(v[0] + q[0]) / 2.0, (v[1] + q[1]) / 2.0];
                moved.insert((pi, vi), m);
                inserts.entry((s.part, s.idx)).or_default().push((t, m));
                break;
            }
        }
    }
    if moved.is_empty() {
        return;
    }
    for (pi, p) in parts.iter_mut().enumerate() {
        let mut out = Vec::with_capacity(p.len());
        for (vi, &v) in p.iter().enumerate() {
            out.push(moved.get(&(pi, vi)).copied().unwrap_or(v));
            if vi + 1 < p.len()
                && let Some(ins) = inserts.get_mut(&(pi, vi))
            {
                ins.sort_by(|a, b| a.0.total_cmp(&b.0));
                out.extend(ins.iter().map(|&(_, m)| m));
            }
        }
        // A closed ring keeps its closing vertex equal to the first.
        if p.first() == p.last() && p.len() > 1 {
            let first = out[0];
            *out.last_mut().unwrap() = first;
        }
        *p = out;
    }
}

/// The same ring (as a cycle, either direction).
fn same_ring(a: &[Pt], b: &[Pt]) -> bool {
    let (a, b) = (&a[..a.len() - 1], &b[..b.len() - 1]);
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    let n = a.len();
    (0..n).filter(|&k| b[k] == a[0]).any(|k| {
        (0..n).all(|i| a[i] == b[(k + i) % n]) || (0..n).all(|i| a[i] == b[(k + n - i) % n])
    })
}

fn repair_polygon(g: &Geometry, method: ValidationMethod, tol: f64) -> Option<Vec<Vec<Pt>>> {
    let cell = 2.0 * std::f64::consts::SQRT_2 * tol;
    let mut rings: Vec<Vec<Pt>> = parts_of(g)
        .into_iter()
        .filter_map(|r| {
            let mut r = merge_close(&r, cell, true);
            if r.len() < 3 {
                return None;
            }
            r.push(r[0]);
            Some(r)
        })
        .collect();
    snap_to_segments(&mut rings, tol);
    if method == ValidationMethod::Esri {
        let mut kept: Vec<Vec<Pt>> = Vec::new();
        for r in rings {
            if !kept.iter().any(|k| same_ring(k, &r)) {
                kept.push(r);
            }
        }
        rings = kept;
    }
    let contours: Vec<Vec<Pt>> = rings.iter().map(|r| r[..r.len() - 1].to_vec()).collect();
    let options = OverlayOptions {
        preserve_input_collinear: true,
        output_direction: ContourDirection::Clockwise,
        preserve_output_collinear: true,
        min_output_area: 0.0,
        ogc: method == ValidationMethod::Ogc,
        clean_result: true,
    };
    let shapes = FloatOverlay::with_subj_custom(&contours, options, Default::default())
        .overlay(OverlayRule::Subject, FillRule::EvenOdd);
    // Back onto the input's own coordinates where the overlay's grid moved them.
    let known: Vec<Pt> = rings.iter().flatten().copied().collect();
    let snap = |p: Pt| {
        known
            .iter()
            .copied()
            .filter(|&k| dist(k, p) <= 1e-6)
            .min_by(|&a, &b| dist(a, p).total_cmp(&dist(b, p)))
            .unwrap_or(p)
    };
    let out: Vec<Vec<Pt>> = shapes
        .into_iter()
        .flatten()
        .filter(|c| c.len() >= 3)
        .map(|c| {
            let mut r: Vec<Pt> = c.into_iter().map(snap).collect();
            r.push(r[0]);
            r
        })
        .collect();
    (!out.is_empty()).then_some(out)
}

fn key(p: Pt) -> (u64, u64) {
    (p[0].to_bits(), p[1].to_bits())
}

/// Splits lines where they cross, touch or overlap, keeps each stretch once and
/// joins consecutive pieces through vertices where nothing else meets (without
/// closing a loop).
fn node_lines(parts: &[Vec<Pt>]) -> Vec<Vec<Pt>> {
    let segs = segments(parts);
    let mut splits: Vec<Vec<(f64, Pt)>> = vec![Vec::new(); segs.len()];
    let eps = 1e-9;
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            let (s, t) = (segs[i], segs[j]);
            // Ends of one lying on the other (also covers collinear overlaps).
            for (from, onto, k) in [(t, s, i), (s, t, j)] {
                for v in [from.a, from.b] {
                    let (d, u) = to_segment(v, onto.a, onto.b);
                    if d <= eps && u > 0.0 && u < 1.0 && v != onto.a && v != onto.b {
                        splits[k].push((u, v));
                    }
                }
            }
            if let Some((u, w)) = crossing((s.a, s.b), (t.a, t.b))
                && u > 0.0
                && u < 1.0
                && w > 0.0
                && w < 1.0
                && cross(s.a, s.b, t.a).abs() > 0.0
            {
                let x = [
                    s.a[0] + u * (s.b[0] - s.a[0]),
                    s.a[1] + u * (s.b[1] - s.a[1]),
                ];
                splits[i].push((u, x));
                splits[j].push((w, x));
            }
        }
    }
    // Pieces in input order.
    let mut pieces: Vec<(Pt, Pt)> = Vec::new();
    let mut seen: std::collections::HashSet<_> = std::collections::HashSet::new();
    for (s, sp) in segs.iter().zip(&mut splits) {
        sp.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut pts = vec![s.a];
        pts.extend(sp.iter().map(|&(_, p)| p));
        pts.push(s.b);
        pts.dedup();
        for w in pts.windows(2) {
            let (ka, kb) = (key(w[0]), key(w[1]));
            let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
            if seen.insert(k) {
                pieces.push((w[0], w[1]));
            }
        }
    }
    let mut degree: HashMap<(u64, u64), usize> = HashMap::new();
    for &(a, b) in &pieces {
        *degree.entry(key(a)).or_default() += 1;
        *degree.entry(key(b)).or_default() += 1;
    }
    let mut out: Vec<Vec<Pt>> = Vec::new();
    for (a, b) in pieces {
        if let Some(last) = out.last_mut() {
            let end = *last.last().unwrap();
            if end == a && degree[&key(a)] == 2 && b != last[0] {
                last.push(b);
                continue;
            }
        }
        out.push(vec![a, b]);
    }
    out
}

fn repair_line(g: &Geometry, method: ValidationMethod, tol: f64) -> Option<Vec<Vec<Pt>>> {
    let cell = 2.0 * std::f64::consts::SQRT_2 * tol;
    let mut parts: Vec<Vec<Pt>> = parts_of(g)
        .into_iter()
        .map(|mut p| {
            match method {
                ValidationMethod::Esri => p.dedup(),
                ValidationMethod::Ogc => p = merge_close(&p, cell, false),
            }
            p
        })
        .filter(|p| p.len() >= 2)
        .collect();
    if method == ValidationMethod::Ogc && !parts.is_empty() {
        snap_to_segments(&mut parts, tol);
        parts = node_lines(&parts);
    }
    (!parts.is_empty()).then_some(parts)
}

/// Z and M of the output vertices, from the input vertex at (or nearest to) each.
fn carry_zm(src: &Geometry, xy: &[Pt], values: Option<&Vec<f64>>) -> Option<Vec<f64>> {
    let values = values?;
    let exact: HashMap<(u64, u64), f64> = src
        .xy
        .iter()
        .zip(values)
        .map(|(&p, &v)| (key(p), v))
        .collect();
    Some(
        xy.iter()
            .map(|&p| {
                exact.get(&key(p)).copied().unwrap_or_else(|| {
                    src.xy
                        .iter()
                        .zip(values)
                        .min_by(|a, b| dist(*a.0, p).total_cmp(&dist(*b.0, p)))
                        .map_or(0.0, |(_, &v)| v)
                })
            })
            .collect(),
    )
}

/// Repair Geometry's result for a shape with a problem; `None` when nothing is
/// left of it. Shapes without problems should be kept as they are.
pub fn repair(g: &Geometry, method: ValidationMethod, tol: f64) -> Result<Option<Geometry>> {
    if g.has_curves() {
        return Ok(Some(g.clone()));
    }
    let parts = match g.kind {
        GeometryKind::Polygon => repair_polygon(g, method, tol),
        GeometryKind::Polyline => repair_line(g, method, tol),
        _ => return Ok((!g.xy.is_empty()).then(|| g.clone())),
    };
    let Some(parts) = parts else {
        return Ok(None);
    };
    let mut starts = Vec::with_capacity(parts.len());
    let mut xy = Vec::new();
    for p in parts {
        starts.push(xy.len() as u32);
        xy.extend(p);
    }
    Ok(Some(Geometry {
        kind: g.kind,
        parts: starts,
        part_types: None,
        z: carry_zm(g, &xy, g.z.as_ref()),
        m: carry_zm(g, &xy, g.m.as_ref()),
        xy,
        curves: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 0.001;

    fn shape(kind: GeometryKind, parts: &[&[Pt]]) -> Geometry {
        let mut g = Geometry::point(0.0, 0.0);
        g.kind = kind;
        g.xy.clear();
        for p in parts {
            g.parts.push(g.xy.len() as u32);
            g.xy.extend_from_slice(p);
        }
        g
    }

    fn poly(parts: &[&[Pt]]) -> Geometry {
        shape(GeometryKind::Polygon, parts)
    }

    fn line(parts: &[&[Pt]]) -> Geometry {
        shape(GeometryKind::Polyline, parts)
    }

    const SQ: [Pt; 5] = [
        [0.0, 0.0],
        [0.0, 50.0],
        [50.0, 50.0],
        [50.0, 0.0],
        [0.0, 0.0],
    ];

    fn both(g: &Geometry) -> (Option<Problem>, Option<Problem>) {
        (
            check(Some(g), ValidationMethod::Esri, TOL),
            check(Some(g), ValidationMethod::Ogc, TOL),
        )
    }

    #[test]
    fn polygon_problems() {
        use Problem::*;
        assert_eq!(both(&poly(&[&SQ])), (None, None));
        let mut ccw = SQ;
        ccw.reverse();
        assert_eq!(
            both(&poly(&[&ccw])),
            (Some(IncorrectRingOrdering), Some(IncorrectRingOrdering))
        );
        let bowtie = [
            [0.0, 0.0],
            [0.0, 50.0],
            [50.0, 0.0],
            [50.0, 50.0],
            [0.0, 0.0],
        ];
        assert_eq!(both(&poly(&[&bowtie])).0, Some(SelfIntersections));
        assert_eq!(both(&poly(&[&SQ[..4]])).0, Some(UnclosedRings));
        let corner = |d: f64| {
            [
                [0.0, 0.0],
                [0.0, 50.0],
                [d, 50.0],
                [50.0, 50.0],
                [50.0, 0.0],
                [0.0, 0.0],
            ]
        };
        assert_eq!(
            both(&poly(&[&corner(0.0)])),
            (Some(ShortSegments), Some(ShortSegments))
        );
        assert_eq!(
            both(&poly(&[&corner(0.0005)])),
            (Some(SelfIntersections), Some(ShortSegments))
        );
        assert_eq!(
            both(&poly(&[&corner(0.0028)])),
            (Some(SelfIntersections), Some(DuplicateVertex))
        );
        assert_eq!(both(&poly(&[&corner(0.0029)])), (None, None));
        let notch = |g: f64| {
            [
                [0.0, 0.0],
                [0.0, 50.0],
                [50.0, 50.0],
                [50.0, 30.0],
                [g, 25.0],
                [50.0, 20.0],
                [50.0, 0.0],
                [0.0, 0.0],
            ]
        };
        assert_eq!(both(&poly(&[&notch(0.0014)])).0, Some(SelfIntersections));
        assert_eq!(both(&poly(&[&notch(0.00142)])).0, None);
        let touching = [
            [0.0, 0.0],
            [0.0, 50.0],
            [25.0, 25.0],
            [50.0, 50.0],
            [50.0, 0.0],
            [25.0, 25.0],
            [0.0, 0.0],
        ];
        assert_eq!(both(&poly(&[&touching])), (None, Some(NonSimple)));
        assert_eq!(
            both(&poly(&[&[[0.0, 0.0], [0.0, 50.0], [0.0, 0.0]]])),
            (Some(SelfIntersections), Some(NonSimple))
        );
        let hole = [
            [10.0, 10.0],
            [30.0, 10.0],
            [30.0, 30.0],
            [10.0, 30.0],
            [10.0, 10.0],
        ];
        assert_eq!(both(&poly(&[&SQ, &hole])), (None, None));
        let mut cw_hole = hole;
        cw_hole.reverse();
        assert_eq!(both(&poly(&[&SQ, &cw_hole])).0, Some(IncorrectRingOrdering));
        assert_eq!(check(None, ValidationMethod::Esri, TOL), Some(NullGeometry));
    }

    #[test]
    fn line_problems() {
        use Problem::*;
        let ok = [[0.0, 0.0], [50.0, 50.0]];
        assert_eq!(both(&line(&[&ok])), (None, None));
        assert_eq!(
            both(&line(&[&ok, &[[60.0, 60.0]]])),
            (Some(EmptyParts), Some(NonSimple))
        );
        assert_eq!(
            both(&line(&[&[[60.0, 60.0]]])),
            (Some(NullGeometry), Some(NullGeometry))
        );
        let cross = [[0.0, 0.0], [50.0, 50.0], [50.0, 0.0], [0.0, 50.0]];
        assert_eq!(both(&line(&[&cross])), (None, Some(SelfIntersections)));
        let t = [[25.0, 0.0], [25.0, 50.0]];
        assert_eq!(
            both(&line(&[&[[0.0, 0.0], [50.0, 0.0]], &t])).1,
            Some(SelfIntersections)
        );
        let ends = [[50.0, 0.0], [50.0, 50.0]];
        assert_eq!(both(&line(&[&[[0.0, 0.0], [50.0, 0.0]], &ends])).1, None);
        let closed = [[0.0, 0.0], [0.0, 50.0], [50.0, 50.0], [0.0, 0.0]];
        assert_eq!(both(&line(&[&closed])).1, None);
        let short = [[0.0, 0.0], [25.0, 25.0], [25.0004, 25.0], [50.0, 50.0]];
        assert_eq!(both(&line(&[&short])), (None, Some(ShortSegments)));
    }

    fn area(g: &Geometry) -> f64 {
        -g.part_ranges().map(|r| signed_area(&g.xy[r])).sum::<f64>()
    }

    #[test]
    fn repairs_polygons() {
        let esri = ValidationMethod::Esri;
        let bowtie = [
            [0.0, 0.0],
            [0.0, 50.0],
            [50.0, 0.0],
            [50.0, 50.0],
            [0.0, 0.0],
        ];
        let r = repair(&poly(&[&bowtie]), esri, TOL).unwrap().unwrap();
        assert_eq!(r.num_parts(), 2);
        assert!((area(&r) - 1250.0).abs() < 1e-9);
        let mut ccw = SQ;
        ccw.reverse();
        let r = repair(&poly(&[&ccw]), esri, TOL).unwrap().unwrap();
        assert!((area(&r) - 2500.0).abs() < 1e-9);
        // Overlapping rings: even-odd.
        let sq2: Vec<Pt> = SQ.iter().map(|p| [p[0] + 25.0, p[1] + 25.0]).collect();
        let r = repair(&poly(&[&SQ, &sq2]), esri, TOL).unwrap().unwrap();
        assert!((area(&r) - 3750.0).abs() < 1e-9);
        // A hole given twice: Esri keeps it, OGC cancels it.
        let hole = [
            [10.0, 10.0],
            [30.0, 10.0],
            [30.0, 30.0],
            [10.0, 30.0],
            [10.0, 10.0],
        ];
        let g = poly(&[&SQ, &hole, &hole]);
        assert!((area(&repair(&g, esri, TOL).unwrap().unwrap()) - 2100.0).abs() < 1e-9);
        let ogc = repair(&g, ValidationMethod::Ogc, TOL).unwrap().unwrap();
        assert!((area(&ogc) - 2500.0).abs() < 1e-9);
        // A short segment at a corner collapses to its midpoint.
        let corner = [
            [0.0, 0.0],
            [0.0, 50.0],
            [0.002, 50.0],
            [50.0, 50.0],
            [50.0, 0.0],
            [0.0, 0.0],
        ];
        let r = repair(&poly(&[&corner]), esri, TOL).unwrap().unwrap();
        assert!((area(&r) - 2499.975).abs() < 1e-6, "{}", area(&r));
        assert!(
            repair(&poly(&[&[[0.0, 0.0], [0.0, 50.0], [0.0, 0.0]]]), esri, TOL)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn repairs_lines() {
        let ogc = ValidationMethod::Ogc;
        let cross = [[0.0, 0.0], [50.0, 50.0], [50.0, 0.0], [0.0, 50.0]];
        let r = repair(&line(&[&cross]), ogc, TOL).unwrap().unwrap();
        let parts: Vec<_> = r.part_ranges().map(|p| r.xy[p].to_vec()).collect();
        assert_eq!(
            parts,
            vec![
                vec![[0.0, 0.0], [25.0, 25.0]],
                vec![[25.0, 25.0], [50.0, 50.0], [50.0, 0.0]],
                vec![[50.0, 0.0], [25.0, 25.0]],
                vec![[25.0, 25.0], [0.0, 50.0]],
            ]
        );
        let overlapping = line(&[&[[0.0, 0.0], [50.0, 0.0]], &[[25.0, 0.0], [75.0, 0.0]]]);
        let r = repair(&overlapping, ogc, TOL).unwrap().unwrap();
        assert_eq!(
            r.xy,
            vec![[0.0, 0.0], [25.0, 0.0], [50.0, 0.0], [75.0, 0.0]]
        );
        let esri = repair(
            &line(&[&[[10.0, 10.0], [10.0, 10.0]]]),
            ValidationMethod::Esri,
            TOL,
        );
        assert!(esri.unwrap().is_none());
    }
}
