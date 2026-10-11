//! Where two outlines meet: crossing/touching points and shared segments. The
//! basis of line×line Intersect and Intersect's LINE/POINT output types, where a
//! polygon takes part through its boundary.

use geoforge_core::{Geometry, GeometryKind, Result};

use crate::clip::linear;

/// ArcGIS's default XY tolerance for projected data in meters.
pub const XY_TOLERANCE: f64 = 0.001;

/// What two outlines have in common.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meeting {
    /// Isolated points where they cross or touch (not on a shared stretch).
    pub points: Vec<[f64; 2]>,
    /// Stretches they share, merged into paths (in the first geometry's direction).
    pub shared: Vec<Vec<[f64; 2]>>,
}

type Seg = ([f64; 2], [f64; 2]);

fn segments(g: &Geometry) -> Vec<Seg> {
    g.part_ranges()
        .flat_map(|r| {
            g.xy[r]
                .windows(2)
                .map(|w| (w[0], w[1]))
                .filter(|(a, b)| a != b)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn at(s: Seg, t: f64) -> [f64; 2] {
    if t <= 0.0 {
        s.0
    } else if t >= 1.0 {
        s.1
    } else {
        [
            s.0[0] + t * (s.1[0] - s.0[0]),
            s.0[1] + t * (s.1[1] - s.0[1]),
        ]
    }
}

/// Distance from `p` to segment `s`, and the parameter of the closest point.
fn to_segment(p: [f64; 2], s: Seg) -> (f64, f64) {
    let d = sub(s.1, s.0);
    let t = (dot(sub(p, s.0), d) / dot(d, d)).clamp(0.0, 1.0);
    let q = at(s, t);
    ((p[0] - q[0]).hypot(p[1] - q[1]), t)
}

enum SegMeet {
    None,
    Point([f64; 2]),
    Shared(Seg),
}

fn meet(a: Seg, b: Seg, tol: f64) -> SegMeet {
    let (da, db) = (sub(a.1, a.0), sub(b.1, b.0));
    let la = dot(da, da).sqrt();
    // Collinear (within tolerance): both ends of b near a's line.
    let off = |p: [f64; 2]| cross(da, sub(p, a.0)).abs() / la;
    if off(b.0) <= tol && off(b.1) <= tol {
        // Ends of the shared stretch as (parameter on a, point), using existing
        // vertices exactly: a's ends, else b's (which lie on a within tolerance).
        let snap = |p: [f64; 2]| {
            let t = dot(sub(p, a.0), da) / (la * la);
            if t * la <= tol {
                (0.0, a.0)
            } else if (1.0 - t) * la <= tol {
                (1.0, a.1)
            } else {
                (t, p)
            }
        };
        let (mut lo, mut hi) = (snap(b.0), snap(b.1));
        if lo.0 > hi.0 {
            std::mem::swap(&mut lo, &mut hi);
        }
        if lo.0 < 0.0 {
            lo = (0.0, a.0);
        }
        if hi.0 > 1.0 {
            hi = (1.0, a.1);
        }
        if (hi.0 - lo.0) * la > tol {
            return SegMeet::Shared((lo.1, hi.1));
        }
        if hi.0 >= lo.0 {
            return SegMeet::Point(lo.1);
        }
        return SegMeet::None;
    }
    let den = cross(da, db);
    if den != 0.0 {
        let w = sub(b.0, a.0);
        let t = cross(w, db) / den;
        let u = cross(w, da) / den;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            return SegMeet::Point(at(a, t));
        }
    }
    // Touching within tolerance (an end of one on the other).
    for (p, s, on_a) in [
        (a.0, b, false),
        (a.1, b, false),
        (b.0, a, true),
        (b.1, a, true),
    ] {
        let (d, t) = to_segment(p, s);
        if d <= tol {
            return SegMeet::Point(if on_a { at(a, t) } else { p });
        }
    }
    SegMeet::None
}

/// Whether `p` lies on a path (within `tol`); with `interior`, not at its ends.
pub fn on_path(p: [f64; 2], path: &[[f64; 2]], tol: f64, interior: bool) -> bool {
    let near = |q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) <= tol;
    if interior && (near(path[0]) || near(*path.last().unwrap())) {
        return false;
    }
    path.windows(2)
        .any(|w| w[0] != w[1] && to_segment(p, (w[0], w[1])).0 <= tol)
}

/// The outline of a line or polygon (a polygon's rings), as linear paths.
fn outline(g: &Geometry) -> Result<Option<Geometry>> {
    Ok(match g.kind {
        GeometryKind::Polyline | GeometryKind::Polygon => Some(linear(g)?),
        _ => None,
    })
}

/// Where the outlines of `a` and `b` meet, within `tol`.
pub fn meeting(a: &Geometry, b: &Geometry, tol: f64) -> Result<Meeting> {
    let (Some(a), Some(b)) = (outline(a)?, outline(b)?) else {
        return Err(geoforge_core::Error::Unsupported(
            "meeting points of point geometries".into(),
        ));
    };
    let (ea, eb) = (a.envelope(), b.envelope());
    if ea.xmin > eb.xmax + tol
        || eb.xmin > ea.xmax + tol
        || ea.ymin > eb.ymax + tol
        || eb.ymin > ea.ymax + tol
    {
        return Ok(Meeting::default());
    }
    let sb = segments(&b);
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut shared: Vec<Vec<[f64; 2]>> = Vec::new();
    for sa in segments(&a) {
        for &s in &sb {
            match meet(sa, s, tol) {
                SegMeet::None => {}
                SegMeet::Point(p) => {
                    if !points
                        .iter()
                        .any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) <= tol)
                    {
                        points.push(p);
                    }
                }
                SegMeet::Shared((p, q)) => shared.push(vec![p, q]),
            }
        }
    }
    let shared = crate::overlay::merge_paths(shared);
    points.retain(|&p| !shared.iter().any(|s| on_path(p, s, tol, false)));
    Ok(Meeting { points, shared })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(pts: &[[f64; 2]]) -> Geometry {
        let mut g = Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Polyline;
        g.parts = vec![0];
        g.xy = pts.to_vec();
        g
    }

    #[test]
    fn crossing_touching_and_shared() {
        let a = line(&[[0.0, 0.0], [10.0, 10.0], [20.0, 0.0]]);
        let b = line(&[[0.0, 10.0], [10.0, 0.0]]);
        let m = meeting(&a, &b, 0.001).unwrap();
        assert_eq!(m.points, vec![[5.0, 5.0]]);
        assert!(m.shared.is_empty());

        // Runs along a from (2,2) to (10,10) and on to (12,8), then leaves.
        let c = line(&[
            [-5.0, 3.0],
            [2.0, 2.0],
            [10.0, 10.0],
            [12.0, 8.0],
            [12.0, 20.0],
        ]);
        let m = meeting(&a, &c, 0.001).unwrap();
        assert_eq!(m.shared, vec![vec![[2.0, 2.0], [10.0, 10.0], [12.0, 8.0]]]);
        assert!(m.points.is_empty());

        // An end touching the middle of a segment.
        let d = line(&[[15.0, 5.0], [15.0, -5.0]]);
        let m = meeting(&a, &d, 0.001).unwrap();
        assert_eq!(m.points, vec![[15.0, 5.0]]);
    }
}
