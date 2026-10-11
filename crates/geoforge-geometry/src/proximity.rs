//! Distances and spatial relationships between features: the basis of Near,
//! Spatial Join and Select Layer By Location.

use geo::Relate;
use geoforge_core::{Geometry, GeometryKind, Result};

use crate::clip::linear;
use crate::convert::{to_multilinestring, to_multipoint, to_multipolygon};

/// The closest approach of two geometries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nearest {
    pub distance: f64,
    /// The closest point on the first geometry.
    pub from: [f64; 2],
    /// The closest point on the second geometry.
    pub to: [f64; 2],
}

/// Segments of a geometry (polygon rings closed); a point is a zero-length segment.
fn segments(g: &Geometry) -> Vec<([f64; 2], [f64; 2])> {
    match g.kind {
        GeometryKind::Point | GeometryKind::Multipoint => g.xy.iter().map(|&p| (p, p)).collect(),
        _ => g
            .part_ranges()
            .flat_map(|r| {
                let pts = &g.xy[r];
                let mut s: Vec<_> = pts.windows(2).map(|w| (w[0], w[1])).collect();
                if pts.len() == 1 {
                    s.push((pts[0], pts[0]));
                }
                s
            })
            .collect(),
    }
}

/// The point of segment `a`–`b` closest to `p`.
fn closest_on_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return a;
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    [a[0] + t * dx, a[1] + t * dy]
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Where segments `p`–`q` and `r`–`s` cross, if they do.
fn crossing(p: [f64; 2], q: [f64; 2], r: [f64; 2], s: [f64; 2]) -> Option<[f64; 2]> {
    let d1 = [q[0] - p[0], q[1] - p[1]];
    let d2 = [s[0] - r[0], s[1] - r[1]];
    let den = d1[0] * d2[1] - d1[1] * d2[0];
    if den == 0.0 {
        return None;
    }
    let w = [r[0] - p[0], r[1] - p[1]];
    let t = (w[0] * d2[1] - w[1] * d2[0]) / den;
    let u = (w[0] * d1[1] - w[1] * d1[0]) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u))
        .then(|| [p[0] + t * d1[0], p[1] + t * d1[1]])
}

/// Closest points of two segments.
fn segment_nearest(a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])) -> Nearest {
    if let Some(x) = crossing(a.0, a.1, b.0, b.1) {
        return Nearest {
            distance: 0.0,
            from: x,
            to: x,
        };
    }
    let candidates = [
        (a.0, closest_on_segment(a.0, b.0, b.1)),
        (a.1, closest_on_segment(a.1, b.0, b.1)),
        (closest_on_segment(b.0, a.0, a.1), b.0),
        (closest_on_segment(b.1, a.0, a.1), b.1),
    ];
    candidates
        .into_iter()
        .map(|(from, to)| Nearest {
            distance: dist(from, to),
            from,
            to,
        })
        .min_by(|x, y| x.distance.total_cmp(&y.distance))
        .unwrap()
}

/// A vertex of `g` inside polygon `poly` (interior or boundary), if any.
fn vertex_inside(g: &Geometry, poly: &Geometry) -> Option<[f64; 2]> {
    let mp = to_multipolygon(poly);
    g.xy.iter()
        .copied()
        .find(|&[x, y]| geo::Intersects::intersects(&mp, &geo::Point::new(x, y)))
}

/// The shortest distance between two geometries (planar, curves densified) and
/// where it is attained. Zero when they overlap: then both points are a shared
/// location (for a polygon containing the other geometry, a vertex of that one).
/// `None` if either is empty.
pub fn nearest(a: &Geometry, b: &Geometry) -> Result<Option<Nearest>> {
    let (a, b) = (linear(a)?, linear(b)?);
    if a.xy.is_empty() || b.xy.is_empty() {
        return Ok(None);
    }
    if b.kind == GeometryKind::Polygon
        && let Some(p) = vertex_inside(&a, &b)
    {
        return Ok(Some(Nearest {
            distance: 0.0,
            from: p,
            to: p,
        }));
    }
    if a.kind == GeometryKind::Polygon
        && let Some(p) = vertex_inside(&b, &a)
    {
        return Ok(Some(Nearest {
            distance: 0.0,
            from: p,
            to: p,
        }));
    }
    let sb = segments(&b);
    let mut best: Option<Nearest> = None;
    for sa in segments(&a) {
        for &s in &sb {
            let n = segment_nearest(sa, s);
            if best.is_none_or(|b| n.distance < b.distance) {
                best = Some(n);
                if n.distance == 0.0 {
                    return Ok(best);
                }
            }
        }
    }
    Ok(best)
}

fn to_geo(g: &Geometry) -> Result<geo::Geometry<f64>> {
    let g = linear(g)?;
    Ok(match g.kind {
        GeometryKind::Point | GeometryKind::Multipoint => to_multipoint(&g).into(),
        GeometryKind::Polyline => to_multilinestring(&g).into(),
        _ => to_multipolygon(&g).into(),
    })
}

/// No part of `a` lies outside `b` (Esri's WITHIN, not Clementini's: a point on
/// `b`'s boundary is within it).
pub fn within(a: &Geometry, b: &Geometry) -> Result<bool> {
    let (ga, gb) = (to_geo(a)?, to_geo(b)?);
    Ok(gb.relate(&ga).is_covers())
}

/// `a` lies in the interior of `b`, not touching its boundary.
pub fn completely_within(a: &Geometry, b: &Geometry) -> Result<bool> {
    let (ga, gb) = (to_geo(a)?, to_geo(b)?);
    Ok(gb.relate(&ga).matches("T**FF*FF*").unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, s: f64) -> Geometry {
        let mut g = Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Polygon;
        g.parts = vec![0];
        g.xy = vec![
            [x0, y0],
            [x0, y0 + s],
            [x0 + s, y0 + s],
            [x0 + s, y0],
            [x0, y0],
        ];
        g
    }

    #[test]
    fn nearest_between_point_and_polygon() {
        let sq = square(0.0, 0.0, 10.0);
        let n = nearest(&sq, &Geometry::point(13.0, 5.0)).unwrap().unwrap();
        assert_eq!(n.distance, 3.0);
        assert_eq!(n.from, [10.0, 5.0]);
        assert_eq!(n.to, [13.0, 5.0]);
        let inside = nearest(&sq, &Geometry::point(4.0, 5.0)).unwrap().unwrap();
        assert_eq!(inside.distance, 0.0);
        assert_eq!(inside.to, [4.0, 5.0]);
    }

    #[test]
    fn within_and_completely_within() {
        let sq = square(0.0, 0.0, 10.0);
        assert!(within(&Geometry::point(5.0, 5.0), &sq).unwrap());
        assert!(within(&Geometry::point(10.0, 5.0), &sq).unwrap());
        assert!(!completely_within(&Geometry::point(10.0, 5.0), &sq).unwrap());
        assert!(!within(&Geometry::point(11.0, 5.0), &sq).unwrap());
        assert!(within(&square(0.0, 0.0, 5.0), &sq).unwrap());
        assert!(!completely_within(&square(0.0, 0.0, 5.0), &sq).unwrap());
        assert!(completely_within(&square(1.0, 1.0, 5.0), &sq).unwrap());
    }
}
