//! Trim Line (Editing): dangling line pieces shorter than a length removed.

use unigis_core::{Error, Feature, FeatureClass, Geometry, GeometryKind, Result};

use crate::ToolOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

type Pt = [f64; 2];

fn sub(a: Pt, b: Pt) -> Pt {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross(a: Pt, b: Pt) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dot(a: Pt, b: Pt) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// Parameter (0..=1) of the point of `a b` nearest `p`, and the distance to it.
fn project(p: Pt, a: Pt, b: Pt) -> (f64, f64) {
    let d = sub(b, a);
    let l2 = dot(d, d);
    let u = if l2 == 0.0 {
        0.0
    } else {
        (dot(sub(p, a), d) / l2).clamp(0.0, 1.0)
    };
    let q = [a[0] + u * d[0], a[1] + u * d[1]];
    (u, (p[0] - q[0]).hypot(p[1] - q[1]))
}

/// Parameters along `a b` where it meets `c d`: a crossing, the touches of either segment's
/// end points on the other (within `tol`), or the ends of a collinear overlap.
fn meet(a: Pt, b: Pt, c: Pt, d: Pt, tol: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let (r, s) = (sub(b, a), sub(d, c));
    let denom = cross(r, s);
    if denom.abs() > 1e-12 * (r[0].hypot(r[1]) * s[0].hypot(s[1])) {
        let w = sub(c, a);
        let t = cross(w, s) / denom;
        let u = cross(w, r) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            out.push(t);
        }
    }
    for p in [c, d] {
        let (u, dist) = project(p, a, b);
        if dist <= tol {
            out.push(u);
        }
    }
    for p in [a, b] {
        if project(p, c, d).1 <= tol {
            out.push(project(p, a, b).0);
        }
    }
    out
}

/// One part's cut positions (distance from its start) where other lines, other parts of the
/// feature or non-adjacent segments of the part itself meet it.
fn cuts(parts: &[Vec<Vec<Pt>>], fi: usize, pi: usize, tol: f64) -> Vec<f64> {
    let p = &parts[fi][pi];
    let mut out = Vec::new();
    let mut start = 0.0;
    for (si, w) in p.windows(2).enumerate() {
        let len = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        for (fj, f) in parts.iter().enumerate() {
            for (pj, q) in f.iter().enumerate() {
                for (sj, v) in q.windows(2).enumerate() {
                    if fj == fi && pj == pi && si.abs_diff(sj) <= 1 {
                        continue;
                    }
                    out.extend(
                        meet(w[0], w[1], v[0], v[1], tol)
                            .into_iter()
                            .map(|u| start + u * len),
                    );
                }
            }
        }
        start += len;
    }
    out
}

/// Whether the vertex `end` (first or last) of part `pi` of feature `fi` touches no other line.
fn free(parts: &[Vec<Vec<Pt>>], fi: usize, pi: usize, last: bool, tol: f64) -> bool {
    let p = &parts[fi][pi];
    let e = if last { p[p.len() - 1] } else { p[0] };
    let own = if last { p.len() - 2 } else { 0 };
    parts.iter().enumerate().all(|(fj, f)| {
        f.iter().enumerate().all(|(pj, q)| {
            q.windows(2).enumerate().all(|(sj, v)| {
                (fj == fi && pj == pi && sj == own) || project(e, v[0], v[1]).1 > tol
            })
        })
    })
}

/// The stretch of `p` between distances `from` and `to` from its start.
fn slice(p: &[Pt], from: f64, to: f64) -> Vec<Pt> {
    let mut out = Vec::new();
    let mut at = 0.0;
    for w in p.windows(2) {
        let len = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        let point = |s: f64| {
            let u = if len == 0.0 { 0.0 } else { (s - at) / len };
            [
                w[0][0] + u * (w[1][0] - w[0][0]),
                w[0][1] + u * (w[1][1] - w[0][1]),
            ]
        };
        if out.is_empty() && from <= at + len {
            out.push(point(from.max(at)));
        }
        if !out.is_empty() {
            if to <= at + len {
                out.push(point(to));
                break;
            }
            out.push(w[1]);
        }
        at += len;
    }
    out.dedup();
    out
}

/// Trim Line: a piece between a free end (one touching no other line within `tolerance`) and
/// the nearest place the line meets a line (another, another part or itself) is removed when
/// shorter than `length`; a line touching another at one end and free at the other is removed
/// whole when shorter. A line free at both ends whose pieces are all such dangles (a short
/// crossing, an isolated short line) is deleted with `delete_short`, else kept unchanged; this
/// holds per part of a multipart line.
/// Trimming looks at the input lines, not the trimmed ones (observed against ArcGIS Pro 3.7;
/// a dangle of exactly `length` is kept).
pub fn trim_line(
    input: &dyn FeatureClass,
    length: f64,
    delete_short: bool,
    tolerance: f64,
) -> Result<ToolOutput> {
    if input.geometry_kind() != Some(GeometryKind::Polyline) {
        return Err(err("ERROR 000368: Invalid input data: lines are required"));
    }
    let features: Vec<Feature> = input.features().collect::<Result<_>>()?;
    let mut parts: Vec<Vec<Vec<Pt>>> = Vec::with_capacity(features.len());
    for f in &features {
        parts.push(match &f.geometry {
            None => Vec::new(),
            Some(g) if g.z.is_some() || g.m.is_some() || g.has_curves() => {
                return Err(err("Trim Line does not handle Z, M or curves yet"));
            }
            Some(g) => g
                .part_ranges()
                .map(|r| g.xy[r].to_vec())
                .filter(|p| p.len() > 1)
                .collect(),
        });
    }
    let mut out = Vec::new();
    for (fi, mut f) in features.into_iter().enumerate() {
        let Some(geometry) = &f.geometry else {
            out.push(f);
            continue;
        };
        let mut kept: Vec<Vec<Pt>> = Vec::new();
        for (pi, p) in parts[fi].iter().enumerate() {
            let total: f64 = p
                .windows(2)
                .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                .sum();
            let mut c = cuts(&parts, fi, pi, tolerance);
            c.retain(|&s| s > tolerance && s < total - tolerance);
            let (start_free, end_free) = (
                free(&parts, fi, pi, false, tolerance),
                free(&parts, fi, pi, true, tolerance),
            );
            let first = c.iter().copied().fold(f64::INFINITY, f64::min);
            let last = c.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let (from, to) = if c.is_empty() {
                (total, 0.0)
            } else {
                (first, last)
            };
            let trim_start = start_free && from < length;
            let trim_end = end_free && total - to < length;
            let start = if trim_start { from } else { 0.0 };
            let end = if trim_end { to } else { total };
            if start_free && end_free && trim_start && trim_end && start >= end {
                if !delete_short {
                    kept.push(p.clone());
                }
                continue;
            }
            if c.is_empty() && (trim_start || trim_end) {
                continue; // the whole part is a dangle
            }
            if end - start > 0.0 {
                kept.push(slice(p, start, end));
            }
        }
        if kept.is_empty() {
            continue;
        }
        let mut xy = Vec::new();
        let mut starts = Vec::new();
        for p in kept {
            starts.push(xy.len() as u32);
            xy.extend(p);
        }
        f.geometry = Some(Geometry {
            parts: starts,
            xy,
            ..geometry.clone()
        });
        out.push(f);
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features: out,
        messages: Vec::new(),
    })
}
