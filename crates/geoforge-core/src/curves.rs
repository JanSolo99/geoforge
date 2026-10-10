//! Densifying Esri curve segments (circular arcs, Bézier curves, elliptic arcs)
//! into straight-line vertices.

use std::f64::consts::{PI, TAU};

use crate::geometry::{
    ARC_CCW, ARC_EMPTY, ARC_INTERIOR_POINT, ARC_LINE, ARC_POINT, Geometry, SegmentKind,
};
use crate::{Error, Result};

/// Elliptic-arc flag bits (Esri extended shape buffer format).
pub const ELLIPSE_EMPTY: u32 = 0x1;
pub const ELLIPSE_LINE: u32 = 0x40;
pub const ELLIPSE_POINT: u32 = 0x80;
pub const ELLIPSE_CIRCULAR: u32 = 0x100;
pub const ELLIPSE_CENTER_TO: u32 = 0x200;
pub const ELLIPSE_CENTER_FROM: u32 = 0x400;
pub const ELLIPSE_CCW: u32 = 0x800;
pub const ELLIPSE_MINOR: u32 = 0x1000;
pub const ELLIPSE_COMPLETE: u32 = 0x2000;

/// Default angular step when densifying: one vertex per degree of arc.
pub const DEFAULT_MAX_ANGLE_STEP: f64 = PI / 180.0;

impl Geometry {
    /// A copy with every curve segment replaced by straight segments, at most
    /// `max_angle_step` radians of arc apart. Z and M are interpolated linearly along
    /// each segment.
    pub fn densify_curves(&self, max_angle_step: f64) -> Result<Geometry> {
        if self.curves.is_empty() {
            return Ok(self.clone());
        }
        let step = max_angle_step.clamp(1e-4, PI / 4.0);
        let mut out = Geometry {
            kind: self.kind,
            parts: Vec::with_capacity(self.parts.len()),
            part_types: self.part_types.clone(),
            xy: Vec::new(),
            z: self.z.as_ref().map(|_| Vec::new()),
            m: self.m.as_ref().map(|_| Vec::new()),
            curves: Vec::new(),
        };
        let push = |out: &mut Geometry, p: [f64; 2], z: Option<f64>, m: Option<f64>| {
            out.xy.push(p);
            if let (Some(zs), Some(z)) = (&mut out.z, z) {
                zs.push(z);
            }
            if let (Some(ms), Some(m)) = (&mut out.m, m) {
                ms.push(m);
            }
        };
        let z_at = |i: usize| self.z.as_ref().map(|z| z[i]);
        let m_at = |i: usize| self.m.as_ref().map(|m| m[i]);

        for range in self.part_ranges() {
            out.parts.push(out.xy.len() as u32);
            for i in range.clone() {
                push(&mut out, self.xy[i], z_at(i), m_at(i));
                if i + 1 >= range.end {
                    continue;
                }
                let Some(seg) = self.curves.iter().find(|s| s.start as usize == i) else {
                    continue;
                };
                let (p0, p1) = (self.xy[i], self.xy[i + 1]);
                for (p, t) in interior_points(p0, p1, &seg.kind, step)? {
                    let lerp =
                        |a: Option<f64>, b: Option<f64>| a.zip(b).map(|(a, b)| a + (b - a) * t);
                    push(
                        &mut out,
                        p,
                        lerp(z_at(i), z_at(i + 1)),
                        lerp(m_at(i), m_at(i + 1)),
                    );
                }
            }
        }
        if self.parts.is_empty() {
            out.parts.clear();
        }
        Ok(out)
    }
}

/// Points strictly between `p0` and `p1` along the segment, each with its fraction
/// (0..1) of the way along.
fn interior_points(
    p0: [f64; 2],
    p1: [f64; 2],
    kind: &SegmentKind,
    step: f64,
) -> Result<Vec<([f64; 2], f64)>> {
    match *kind {
        SegmentKind::CircularArc { point, bits } => {
            if bits & (ARC_LINE | ARC_POINT | ARC_EMPTY) != 0 {
                return Ok(Vec::new());
            }
            let closed = same_point(p0, p1);
            let (center, sweep) = if bits & ARC_INTERIOR_POINT != 0 {
                if closed {
                    // A full circle stores the point opposite the start.
                    let c = [(p0[0] + point[0]) / 2.0, (p0[1] + point[1]) / 2.0];
                    (c, if bits & ARC_CCW != 0 { TAU } else { -TAU })
                } else {
                    let Some(c) = circumcenter(p0, point, p1) else {
                        return Ok(Vec::new()); // collinear: a straight line
                    };
                    let ccw = cross(p0, point, p1) > 0.0;
                    (c, sweep_between(angle(c, p0), angle(c, p1), ccw))
                }
            } else {
                let ccw = bits & ARC_CCW != 0;
                let sweep = if closed {
                    if ccw { TAU } else { -TAU }
                } else {
                    sweep_between(angle(point, p0), angle(point, p1), ccw)
                };
                (point, sweep)
            };
            // The radius is interpolated from start to end so that an inconsistent
            // center (unequal distances to the endpoints) still meets both ends.
            let (r0, r1) = (dist(center, p0), dist(center, p1));
            let a0 = angle(center, p0);
            Ok(sample(sweep, step, |t| {
                let a = a0 + sweep * t;
                let r = r0 + (r1 - r0) * t;
                [center[0] + r * a.cos(), center[1] + r * a.sin()]
            }))
        }
        SegmentKind::Bezier { controls: [c1, c2] } => {
            // Enough steps that the control polygon turns at most `step` per step.
            let turn = turning(&[p0, c1, c2, p1]);
            let n = ((turn / step).ceil() as usize).clamp(8, 1024);
            Ok((1..n)
                .map(|k| {
                    let t = k as f64 / n as f64;
                    let u = 1.0 - t;
                    let b = |i: usize| {
                        u * u * u * p0[i]
                            + 3.0 * u * u * t * c1[i]
                            + 3.0 * u * t * t * c2[i]
                            + t * t * t * p1[i]
                    };
                    ([b(0), b(1)], t)
                })
                .collect())
        }
        SegmentKind::EllipticArc {
            center,
            rotation,
            semi_major,
            minor_major_ratio,
            bits,
        } => {
            if bits & (ELLIPSE_LINE | ELLIPSE_POINT | ELLIPSE_EMPTY) != 0 {
                return Ok(Vec::new());
            }
            if bits & (ELLIPSE_CENTER_TO | ELLIPSE_CENTER_FROM) != 0 {
                return Err(Error::Unsupported(
                    "elliptic arcs defined by center-to/from angles".into(),
                ));
            }
            let a = semi_major;
            let b = if bits & ELLIPSE_CIRCULAR != 0 {
                a
            } else {
                a * minor_major_ratio
            };
            let (sin_r, cos_r) = rotation.sin_cos();
            // Parametric angle of a point on the (rotated) ellipse.
            let param = |p: [f64; 2]| {
                let (dx, dy) = (p[0] - center[0], p[1] - center[1]);
                let (u, v) = (dx * cos_r + dy * sin_r, -dx * sin_r + dy * cos_r);
                (v / b).atan2(u / a)
            };
            let ccw = bits & ELLIPSE_CCW != 0;
            let sweep = if bits & ELLIPSE_COMPLETE != 0 || same_point(p0, p1) {
                if ccw { TAU } else { -TAU }
            } else {
                sweep_between(param(p0), param(p1), ccw)
            };
            let t0 = param(p0);
            Ok(sample(sweep, step, |t| {
                let th = t0 + sweep * t;
                let (u, v) = (a * th.cos(), b * th.sin());
                [
                    center[0] + u * cos_r - v * sin_r,
                    center[1] + u * sin_r + v * cos_r,
                ]
            }))
        }
    }
}

/// `n - 1` interior samples of a sweep, `n = ceil(|sweep| / step)`.
fn sample(sweep: f64, step: f64, at: impl Fn(f64) -> [f64; 2]) -> Vec<([f64; 2], f64)> {
    let n = ((sweep.abs() / step).ceil() as usize).max(1);
    (1..n)
        .map(|k| k as f64 / n as f64)
        .map(|t| (at(t), t))
        .collect()
}

/// Signed sweep from angle `a0` to `a1`: positive in (0, 2π] if counter-clockwise.
fn sweep_between(a0: f64, a1: f64, ccw: bool) -> f64 {
    let mut d = (a1 - a0).rem_euclid(TAU);
    if ccw {
        if d == 0.0 {
            d = TAU;
        }
        d
    } else {
        if d == 0.0 {
            d = TAU;
        }
        d - TAU
    }
}

fn angle(c: [f64; 2], p: [f64; 2]) -> f64 {
    (p[1] - c[1]).atan2(p[0] - c[0])
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn same_point(a: [f64; 2], b: [f64; 2]) -> bool {
    dist(a, b) <= 1e-12 * a[0].abs().max(a[1].abs()).max(1.0)
}

fn circumcenter(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<[f64; 2]> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    let scale = [a, b, c]
        .iter()
        .map(|p| p[0].abs().max(p[1].abs()))
        .fold(1.0, f64::max);
    if d.abs() <= 1e-12 * scale * scale {
        return None;
    }
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let ux = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
    let uy = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
    Some([ux, uy])
}

/// Total absolute turning angle along a polyline.
fn turning(p: &[[f64; 2]]) -> f64 {
    p.windows(3)
        .map(|w| {
            let a1 = angle(w[0], w[1]);
            let a2 = angle(w[1], w[2]);
            let d = (a2 - a1).rem_euclid(TAU);
            d.min(TAU - d)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GeometryKind, Segment};

    fn line(xy: Vec<[f64; 2]>, curves: Vec<Segment>) -> Geometry {
        Geometry {
            kind: GeometryKind::Polyline,
            parts: vec![0],
            part_types: None,
            xy,
            z: None,
            m: None,
            curves,
        }
    }

    #[test]
    fn half_circle_by_center_stays_on_radius() {
        // From (1,0) to (-1,0) around the origin, counter-clockwise: through (0,1).
        let g = line(
            vec![[1.0, 0.0], [-1.0, 0.0]],
            vec![Segment {
                start: 0,
                kind: SegmentKind::CircularArc {
                    point: [0.0, 0.0],
                    bits: ARC_CCW,
                },
            }],
        );
        let d = g.densify_curves(DEFAULT_MAX_ANGLE_STEP).unwrap();
        assert_eq!(d.xy.len(), 181);
        assert!(d.xy.iter().all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-12));
        assert!(d.xy[90][1] > 0.999);
    }

    #[test]
    fn arc_by_interior_point_passes_through_it() {
        let g = line(
            vec![[1.0, 0.0], [-1.0, 0.0]],
            vec![Segment {
                start: 0,
                kind: SegmentKind::CircularArc {
                    point: [0.0, -1.0],
                    bits: ARC_INTERIOR_POINT,
                },
            }],
        );
        let d = g.densify_curves(DEFAULT_MAX_ANGLE_STEP).unwrap();
        assert!(d.xy[90][1] < -0.999, "clockwise through (0,-1)");
    }

    #[test]
    fn z_and_m_are_interpolated() {
        let mut g = line(
            vec![[1.0, 0.0], [-1.0, 0.0]],
            vec![Segment {
                start: 0,
                kind: SegmentKind::CircularArc {
                    point: [0.0, 0.0],
                    bits: ARC_CCW,
                },
            }],
        );
        g.z = Some(vec![0.0, 10.0]);
        g.m = Some(vec![100.0, 200.0]);
        let d = g.densify_curves(PI / 2.0 / 2.0).unwrap();
        assert_eq!(d.z.unwrap(), [0.0, 2.5, 5.0, 7.5, 10.0]);
        assert_eq!(d.m.unwrap(), [100.0, 125.0, 150.0, 175.0, 200.0]);
    }
}
