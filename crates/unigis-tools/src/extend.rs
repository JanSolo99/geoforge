//! Extend Line (Editing): dangling line ends extended to the first line they reach.

use unigis_core::{Error, Feature, FeatureClass, Geometry, GeometryKind, Result};

use crate::ToolOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

type Parts = Vec<Vec<[f64; 2]>>;

/// One end of a line part: feature, part, and whether it is the part's last vertex.
#[derive(Clone, Copy, PartialEq)]
struct End {
    feature: usize,
    part: usize,
    last: bool,
}

/// The point and the unit direction away from the line at `end`, or `None` for a part without
/// two distinct vertices.
fn ray(geoms: &[Option<Parts>], end: End) -> Option<([f64; 2], [f64; 2])> {
    let p = &geoms[end.feature].as_ref()?[end.part];
    let e = if end.last { *p.last()? } else { p[0] };
    let other = if end.last {
        p.iter().rev().find(|v| **v != e)?
    } else {
        p.iter().find(|v| **v != e)?
    };
    let (dx, dy) = (e[0] - other[0], e[1] - other[1]);
    let len = dx.hypot(dy);
    Some((e, [dx / len, dy / len]))
}

/// Index of the segment (in the part) that ends at `end`: it is not a target of its own ray.
fn end_segment(p: &[[f64; 2]], last: bool) -> usize {
    if last { p.len() - 2 } else { 0 }
}

fn point_segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Every segment of the current geometries, except the end segment of `skip`.
fn segments(geoms: &[Option<Parts>], skip: End) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
    geoms.iter().enumerate().flat_map(move |(fi, g)| {
        g.iter().flatten().enumerate().flat_map(move |(pi, p)| {
            let own = (fi == skip.feature && pi == skip.part).then(|| end_segment(p, skip.last));
            p.windows(2)
                .enumerate()
                .filter(move |(si, _)| Some(*si) != own)
                .map(|(_, w)| (w[0], w[1]))
        })
    })
}

/// An end touching no other line (within the XY tolerance) can be extended.
fn dangling(geoms: &[Option<Parts>], end: End, tol: f64) -> bool {
    let Some((e, _)) = ray(geoms, end) else {
        return false;
    };
    segments(geoms, end).all(|(a, b)| point_segment_distance(e, a, b) > tol)
}

/// Distance along the ray `o + t d` to segment `a b`, for a crossing or touch; parallel and
/// collinear segments do not count.
fn ray_segment(o: [f64; 2], d: [f64; 2], a: [f64; 2], b: [f64; 2]) -> Option<f64> {
    let s = [b[0] - a[0], b[1] - a[1]];
    let denom = d[0] * s[1] - d[1] * s[0];
    if denom.abs() < 1e-12 * s[0].hypot(s[1]) {
        return None;
    }
    let w = [a[0] - o[0], a[1] - o[1]];
    let t = (w[0] * s[1] - w[1] * s[0]) / denom;
    let u = (w[0] * d[1] - w[1] * d[0]) / denom;
    (t > 0.0 && (-1e-12..=1.0 + 1e-12).contains(&u)).then_some(t)
}

/// Distance along the ray from `end` to the first line it reaches.
fn first_hit(geoms: &[Option<Parts>], end: End) -> Option<f64> {
    let (o, d) = ray(geoms, end)?;
    segments(geoms, end)
        .filter_map(|(a, b)| ray_segment(o, d, a, b))
        .min_by(f64::total_cmp)
}

/// Distances along two rays to their crossing.
fn ray_ray(o1: [f64; 2], d1: [f64; 2], o2: [f64; 2], d2: [f64; 2]) -> Option<(f64, f64)> {
    let denom = d1[0] * d2[1] - d1[1] * d2[0];
    if denom.abs() < 1e-12 {
        return None;
    }
    let w = [o2[0] - o1[0], o2[1] - o1[1]];
    let t1 = (w[0] * d2[1] - w[1] * d2[0]) / denom;
    let t2 = (w[0] * d1[1] - w[1] * d1[0]) / denom;
    (t1 > 0.0 && t2 > 0.0).then_some((t1, t2))
}

fn extend_to(geoms: &mut [Option<Parts>], end: End, point: [f64; 2]) {
    if let Some(parts) = &mut geoms[end.feature] {
        let p = &mut parts[end.part];
        if end.last {
            p.push(point);
        } else {
            p.insert(0, point);
        }
    }
}

/// Extend Line: each dangling end (one touching no other line within `tolerance`) is extended
/// along its end segment, adding a vertex, to the first line it reaches less than `length`
/// away (an extension of exactly `length` is not made, as when ArcGIS edits a shapefile), its
/// own line included; parallel and collinear lines are not targets. Features are processed in
/// order against the lines as already extended, part by part, start before end. With
/// `to_extensions` an end may instead stop where its ray crosses the potential extension of
/// another dangling end not extended yet: that end's ray up to `length` (inclusive) or its own
/// first line. Only the end being processed is extended (observed against ArcGIS Pro 3.7).
pub fn extend_line(
    input: &dyn FeatureClass,
    length: f64,
    to_extensions: bool,
    tolerance: f64,
) -> Result<ToolOutput> {
    if input.geometry_kind() != Some(GeometryKind::Polyline) {
        return Err(err("ERROR 000368: Invalid input data: lines are required"));
    }
    let mut features: Vec<Feature> = input.features().collect::<Result<_>>()?;
    let mut geoms: Vec<Option<Parts>> = Vec::with_capacity(features.len());
    for f in &features {
        geoms.push(match &f.geometry {
            None => None,
            Some(g) if g.z.is_some() || g.m.is_some() || g.has_curves() => {
                return Err(err("Extend Line does not handle Z, M or curves yet"));
            }
            Some(g) => Some(g.part_ranges().map(|r| g.xy[r].to_vec()).collect()),
        });
    }
    let ends: Vec<End> = geoms
        .iter()
        .enumerate()
        .flat_map(|(feature, g)| {
            g.iter().flatten().enumerate().flat_map(move |(part, _)| {
                [false, true].map(|last| End {
                    feature,
                    part,
                    last,
                })
            })
        })
        .collect();
    // Exactly `length` is out of reach; allow for rounding in the intersection.
    let limit = length * (1.0 - 1e-9);
    // An end extended to another end's potential extension still touches nothing, but no
    // longer offers its own extension to later ends.
    let mut extended: Vec<End> = Vec::new();
    for &end in &ends {
        if !dangling(&geoms, end, tolerance) {
            continue;
        }
        let Some((o, d)) = ray(&geoms, end) else {
            continue;
        };
        let mut best = first_hit(&geoms, end).filter(|&t| t < limit);
        if to_extensions {
            for &other in &ends {
                if other == end || extended.contains(&other) || !dangling(&geoms, other, tolerance)
                {
                    continue;
                }
                let Some((o2, d2)) = ray(&geoms, other) else {
                    continue;
                };
                let reach = first_hit(&geoms, other).map_or(length, |t| t.min(length));
                if let Some((t1, t2)) = ray_ray(o, d, o2, d2)
                    && t1 < limit
                    && t2 <= reach
                    && best.is_none_or(|t| t1 < t)
                {
                    best = Some(t1);
                }
            }
        }
        if let Some(t) = best {
            extend_to(&mut geoms, end, [o[0] + d[0] * t, o[1] + d[1] * t]);
            extended.push(end);
        }
    }
    for (f, g) in features.iter_mut().zip(geoms) {
        if let (Some(geometry), Some(parts)) = (&mut f.geometry, g) {
            let mut xy = Vec::new();
            let mut starts = Vec::new();
            for p in parts {
                starts.push(xy.len() as u32);
                xy.extend(p);
            }
            *geometry = Geometry {
                parts: starts,
                xy,
                ..geometry.clone()
            };
        }
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}
