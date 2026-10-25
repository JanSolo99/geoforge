//! Densifying Esri curve segments (circular arcs, Bézier curves, elliptic arcs)
//! into straight-line vertices.

use std::f64::consts::{PI, TAU};

use crate::geometry::{
    ARC_CCW, ARC_EMPTY, ARC_INTERIOR_POINT, ARC_LINE, ARC_POINT, Geometry, SegmentKind,
};
use crate::{Envelope, Error, Result};

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

/// 0 stands for the default.
static ARC_ANGLE_STEP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The largest angle one segment of an arc may span (10° unless [`set_arc_angle_step`] says
/// otherwise).
pub fn arc_angle_step() -> f64 {
    match f64::from_bits(ARC_ANGLE_STEP.load(std::sync::atomic::Ordering::Relaxed)) {
        step if step > 0.0 => step,
        _ => PI / 18.0,
    }
}

/// Sets the largest angle of an arc segment, for tools that densify more finely (Pairwise Buffer
/// of lines and polygons uses 3.75°).
pub fn set_arc_angle_step(radians: f64) {
    ARC_ANGLE_STEP.store(radians.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Number of straight segments ArcGIS uses for an arc of `sweep` radians and
/// `radius`: at most 10° per segment, and no point of the chord more than
/// `max_deviation` from the arc. Observed in ArcGIS Pro 3.7 shapefile output
/// (36 segments up to r = 10 m, then 39, 71, 223, 703, 2222 for 30 m … 100 km
/// with a deviation of 0.1 m).
pub fn arcgis_arc_segments(sweep: f64, radius: f64, max_deviation: f64) -> usize {
    let by_angle = (sweep.abs() / arc_angle_step() - 1e-9).ceil();
    let by_deviation = if radius > max_deviation && max_deviation > 0.0 {
        (sweep.abs() / (2.0 * (1.0 - max_deviation / radius).acos()) - 1e-9).ceil()
    } else {
        0.0
    };
    (by_angle.max(by_deviation) as usize).max(1)
}

/// The deviation ArcGIS uses when densifying for shapefile output: 100 × the
/// default XY tolerance (0.001 m projected → 0.1 m; 8.98e-9° geographic).
pub fn arcgis_densify_deviation(geographic: bool) -> f64 {
    if geographic {
        100.0 * 8.983152841195215e-9
    } else {
        0.1
    }
}

/// How finely curves are replaced by straight segments.
#[derive(Debug, Clone, Copy)]
enum Density {
    AngleStep(f64),
    ArcGis { max_deviation: f64 },
}

impl Geometry {
    /// A copy with every curve segment replaced by straight segments, at most
    /// `max_angle_step` radians of arc apart. Z and M are interpolated linearly along
    /// each segment.
    pub fn densify_curves(&self, max_angle_step: f64) -> Result<Geometry> {
        self.densify_with(Density::AngleStep(max_angle_step.clamp(1e-4, PI / 4.0)))
    }

    /// Densifies curves the way ArcGIS does for shapefile output (see
    /// [`arcgis_arc_segments`]).
    pub fn densify_curves_arcgis(&self, max_deviation: f64) -> Result<Geometry> {
        self.densify_with(Density::ArcGis { max_deviation })
    }

    fn densify_with(&self, density: Density) -> Result<Geometry> {
        if self.curves.is_empty() {
            return Ok(self.clone());
        }
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
                for (p, t) in interior_points(p0, p1, &seg.kind, density)? {
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

/// A curve segment in parametric form, `t` in `[0, 1]` from start to end.
enum Curve {
    Line,
    /// Circle (radius interpolated from `r0` to `r1` for inconsistent centers).
    Arc {
        center: [f64; 2],
        r0: f64,
        r1: f64,
        a0: f64,
        sweep: f64,
    },
    Bezier([[f64; 2]; 4]),
    Ellipse {
        center: [f64; 2],
        a: f64,
        b: f64,
        sin_r: f64,
        cos_r: f64,
        t0: f64,
        sweep: f64,
    },
}

impl Curve {
    fn new(p0: [f64; 2], p1: [f64; 2], kind: &SegmentKind) -> Result<Self> {
        Ok(match *kind {
            SegmentKind::CircularArc { point, bits } => {
                if bits & (ARC_LINE | ARC_POINT | ARC_EMPTY) != 0 {
                    return Ok(Curve::Line);
                }
                let closed = same_point(p0, p1);
                let (center, sweep) = if bits & ARC_INTERIOR_POINT != 0 {
                    if closed {
                        // A full circle stores the point opposite the start.
                        let c = [(p0[0] + point[0]) / 2.0, (p0[1] + point[1]) / 2.0];
                        (c, if bits & ARC_CCW != 0 { TAU } else { -TAU })
                    } else {
                        let Some(c) = circumcenter(p0, point, p1) else {
                            return Ok(Curve::Line); // collinear
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
                Curve::Arc {
                    center,
                    r0: dist(center, p0),
                    r1: dist(center, p1),
                    a0: angle(center, p0),
                    sweep,
                }
            }
            SegmentKind::Bezier { controls: [c1, c2] } => Curve::Bezier([p0, c1, c2, p1]),
            SegmentKind::EllipticArc {
                center,
                rotation,
                semi_major,
                minor_major_ratio,
                bits,
            } => {
                if bits & (ELLIPSE_LINE | ELLIPSE_POINT | ELLIPSE_EMPTY) != 0 {
                    return Ok(Curve::Line);
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
                Curve::Ellipse {
                    center,
                    a,
                    b,
                    sin_r,
                    cos_r,
                    t0: param(p0),
                    sweep,
                }
            }
        })
    }

    fn at(&self, t: f64) -> [f64; 2] {
        match *self {
            Curve::Line => [f64::NAN; 2],
            Curve::Arc {
                center,
                r0,
                r1,
                a0,
                sweep,
            } => {
                let a = a0 + sweep * t;
                let r = r0 + (r1 - r0) * t;
                [center[0] + r * a.cos(), center[1] + r * a.sin()]
            }
            Curve::Bezier([p0, c1, c2, p1]) => {
                let u = 1.0 - t;
                let b = |i: usize| {
                    u * u * u * p0[i]
                        + 3.0 * u * u * t * c1[i]
                        + 3.0 * u * t * t * c2[i]
                        + t * t * t * p1[i]
                };
                [b(0), b(1)]
            }
            Curve::Ellipse {
                center,
                a,
                b,
                sin_r,
                cos_r,
                t0,
                sweep,
            } => {
                let th = t0 + sweep * t;
                let (u, v) = (a * th.cos(), b * th.sin());
                [
                    center[0] + u * cos_r - v * sin_r,
                    center[1] + u * sin_r + v * cos_r,
                ]
            }
        }
    }

    /// Parameters in (0, 1) where x or y reaches an extreme.
    fn extremes(&self) -> Vec<f64> {
        // Map angle `target` to t for a sweep starting at `start`.
        let angle_t = |start: f64, sweep: f64, target: f64| {
            let delta = if sweep >= 0.0 {
                (target - start).rem_euclid(TAU)
            } else {
                (start - target).rem_euclid(TAU)
            };
            delta / sweep.abs()
        };
        let mut ts = Vec::new();
        match *self {
            Curve::Line => {}
            Curve::Arc { a0, sweep, .. } => {
                for k in 0..4 {
                    ts.push(angle_t(a0, sweep, f64::from(k) * PI / 2.0));
                }
            }
            Curve::Ellipse {
                a,
                b,
                sin_r,
                cos_r,
                t0,
                sweep,
                ..
            } => {
                let tx = (-b * sin_r).atan2(a * cos_r);
                let ty = (b * cos_r).atan2(a * sin_r);
                for th in [tx, tx + PI, ty, ty + PI] {
                    ts.push(angle_t(t0, sweep, th));
                }
            }
            Curve::Bezier([p0, c1, c2, p1]) => {
                for i in 0..2 {
                    let (d0, d1, d2) = (c1[i] - p0[i], c2[i] - c1[i], p1[i] - c2[i]);
                    let (qa, qb, qc) = (d0 - 2.0 * d1 + d2, 2.0 * (d1 - d0), d0);
                    if qa.abs() < 1e-15 {
                        if qb.abs() > 1e-15 {
                            ts.push(-qc / qb);
                        }
                    } else {
                        let disc = qb * qb - 4.0 * qa * qc;
                        if disc >= 0.0 {
                            ts.push((-qb + disc.sqrt()) / (2.0 * qa));
                            ts.push((-qb - disc.sqrt()) / (2.0 * qa));
                        }
                    }
                }
            }
        }
        ts.retain(|t| *t > 0.0 && *t < 1.0);
        ts
    }

    /// Steps with ArcGIS's rule (≤ 10° and ≤ `max_deviation` per chord).
    fn steps_arcgis(&self, max_deviation: f64) -> usize {
        match *self {
            Curve::Line => 1,
            Curve::Arc { r0, r1, sweep, .. } => {
                arcgis_arc_segments(sweep, r0.max(r1), max_deviation)
            }
            Curve::Ellipse { a, sweep, .. } => arcgis_arc_segments(sweep, a, max_deviation),
            Curve::Bezier(p) => ((turning(&p) / (PI / 18.0)).ceil() as usize).clamp(8, 1024),
        }
    }

    /// Steps needed so consecutive samples are at most `step` radians of arc apart.
    fn steps(&self, step: f64) -> usize {
        match *self {
            Curve::Line => 1,
            Curve::Arc { sweep, .. } | Curve::Ellipse { sweep, .. } => {
                ((sweep.abs() / step).ceil() as usize).max(1)
            }
            Curve::Bezier(p) => ((turning(&p) / step).ceil() as usize).clamp(8, 1024),
        }
    }
}

/// Points strictly between `p0` and `p1` along the segment, each with its fraction
/// (0..1) of the way along.
fn interior_points(
    p0: [f64; 2],
    p1: [f64; 2],
    kind: &SegmentKind,
    density: Density,
) -> Result<Vec<([f64; 2], f64)>> {
    let curve = Curve::new(p0, p1, kind)?;
    if matches!(curve, Curve::Line) {
        return Ok(Vec::new());
    }
    let n = match density {
        Density::AngleStep(step) => curve.steps(step),
        Density::ArcGis { max_deviation } => curve.steps_arcgis(max_deviation),
    };
    Ok((1..n)
        .map(|k| k as f64 / n as f64)
        .map(|t| (curve.at(t), t))
        .collect())
}

impl Geometry {
    /// The exact XY bounding box, including the bulge of curve segments beyond
    /// their endpoints (the extent ArcGIS stores and indexes).
    pub fn curve_envelope(&self) -> Envelope {
        let mut env = self.envelope();
        for s in &self.curves {
            let i = s.start as usize;
            let (Some(&p0), Some(&p1)) = (self.xy.get(i), self.xy.get(i + 1)) else {
                continue;
            };
            let Ok(curve) = Curve::new(p0, p1, &s.kind) else {
                continue;
            };
            for t in curve.extremes() {
                let [x, y] = curve.at(t);
                env.expand_xy(x, y);
            }
        }
        env
    }
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
    // Relative to `a`: with absolute coordinates (millions of metres) both the
    // determinant and a threshold scaled by them would swamp short, flat arcs.
    let (b, c) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
    let d = 2.0 * (b[0] * c[1] - b[1] * c[0]);
    let scale = [b, c]
        .iter()
        .map(|p| p[0].abs().max(p[1].abs()))
        .fold(0.0, f64::max);
    if scale == 0.0 || d.abs() <= 1e-12 * scale * scale {
        return None;
    }
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let ux = (c[1] * sq(b) - b[1] * sq(c)) / d;
    let uy = (b[0] * sq(c) - c[0] * sq(b)) / d;
    Some([a[0] + ux, a[1] + uy])
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
    fn arcgis_segment_counts() {
        let full = |r: f64| arcgis_arc_segments(TAU, r, 0.1);
        let counts: Vec<usize> = [5.0, 10.0, 30.0, 100.0, 1000.0, 10000.0, 100000.0]
            .map(full)
            .to_vec();
        assert_eq!(counts, [36, 36, 39, 71, 223, 703, 2222]);
    }

    #[test]
    fn curve_envelope_includes_the_bulge() {
        // Half circle from (1,0) to (-1,0) counter-clockwise: reaches y = 1.
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
        let e = g.curve_envelope();
        assert!((e.ymax - 1.0).abs() < 1e-12 && e.ymin == 0.0);
        // Cubic Bézier bulging to y = 0.75.
        let b = line(
            vec![[0.0, 0.0], [3.0, 0.0]],
            vec![Segment {
                start: 0,
                kind: SegmentKind::Bezier {
                    controls: [[1.0, 1.0], [2.0, 1.0]],
                },
            }],
        );
        assert!((b.curve_envelope().ymax - 0.75).abs() < 1e-12);
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
