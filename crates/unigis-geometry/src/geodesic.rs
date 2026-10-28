//! Geodesic Buffer (ArcGIS `analysis.Buffer` on geographic data with a distance in linear units).
//!
//! ArcGIS measures the distance on the WGS 84 ellipsoid (or the input's): a point's buffer is the
//! geodesic circle, starting due north and running clockwise in equal azimuth steps, and the number
//! of steps follows the planar chord-deviation rule with a deviation that depends on the distance
//! (0.1 m up to 1 km, 1 m up to 10 km, 5 m up to 100 km, 10 m beyond; observed in ArcGIS Pro 3.7).
//! Lines and polygons are built the way the planar buffer is, from the same pieces — a rectangle per
//! segment, an arc where a bend opens, half-discs at open ends — with every offset point found by
//! the direct geodesic problem, and unioned in longitude/latitude.
//!
//! Coordinates are `[longitude, latitude]` in degrees. A buffer that would cross the antimeridian or
//! reach a pole is refused: its pieces are not simple polygons in longitude/latitude.

use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon, unary_union};
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use unigis_core::curves::arcgis_arc_segments;
use unigis_core::{Error, Geometry, GeometryKind, Result};

use crate::buffer::{LineEnd, LineSide};
use crate::convert::{from_multipolygon, to_multipolygon};

/// The deviation ArcGIS allows between a geodesic arc and its chords, in metres, for a buffer
/// distance of `distance` metres.
pub fn max_deviation(distance: f64) -> f64 {
    if distance <= 1_000.0 {
        0.1
    } else if distance <= 10_000.0 {
        1.0
    } else if distance <= 100_000.0 {
        5.0
    } else {
        10.0
    }
}

/// The deviation ArcGIS allows when it builds the arcs of a line's or polygon's buffer (metres):
/// observed through the bridge, it differs from a point's above 50 km.
pub fn max_deviation_path(distance: f64) -> f64 {
    if distance <= 1_000.0 {
        0.1
    } else if distance <= 10_000.0 {
        1.0
    } else if distance <= 50_000.0 {
        5.0
    } else {
        100.0
    }
}

/// The azimuth step, in degrees, of the arcs of a line's or polygon's buffer: the largest
/// 10° / 2^k whose chords stay within the deviation. Arc vertices sit on multiples of it.
pub fn arc_step(distance: f64) -> f64 {
    let dev = max_deviation_path(distance);
    let theta = if distance > dev {
        2.0 * (1.0 - dev / distance).acos().to_degrees()
    } else {
        360.0
    };
    let mut step = 10.0;
    while step > theta && step > 1e-6 {
        step /= 2.0;
    }
    step
}

#[derive(Debug, Clone, Copy)]
pub struct GeodesicOptions {
    /// Metres on the ellipsoid; negative distances shrink polygons.
    pub distance: f64,
    pub line_end: LineEnd,
    pub side: LineSide,
}

/// Geodesic computations on an ellipsoid.
struct Earth {
    g: Geodesic,
}

fn wrap180(a: f64) -> f64 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

impl Earth {
    fn wgs84() -> Earth {
        Earth {
            g: Geodesic::wgs84(),
        }
    }

    /// The point `r` metres from `p` along azimuth `az` (degrees clockwise from north).
    fn direct(&self, p: [f64; 2], az: f64, r: f64) -> [f64; 2] {
        let (lat, lon): (f64, f64) = self.g.direct(p[1], p[0], az, r);
        [lon, lat]
    }

    /// Azimuths at `a` (leaving) and at `b` (arriving) of the geodesic from `a` to `b`.
    fn azimuths(&self, a: [f64; 2], b: [f64; 2]) -> Option<(f64, f64)> {
        let (s12, az1, az2, _): (f64, f64, f64, f64) = self.g.inverse(a[1], a[0], b[1], b[0]);
        (s12 > 0.0).then_some((az1, az2))
    }
}

fn ring(points: Vec<[f64; 2]>) -> Polygon<f64> {
    let mut pts: Vec<Coord<f64>> = points
        .into_iter()
        .map(|p| Coord { x: p[0], y: p[1] })
        .collect();
    if pts.first() != pts.last() {
        pts.push(pts[0]);
    }
    Polygon::new(LineString::new(pts), vec![])
}

/// The ring clockwise, as the union wants every piece.
fn clockwise(points: Vec<[f64; 2]>) -> Polygon<f64> {
    let mut pts = points;
    if pts.first() != pts.last() {
        pts.push(pts[0]);
    }
    let area: f64 = pts
        .windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum();
    if area > 0.0 {
        pts.reverse();
    }
    ring(pts)
}

/// The geodesic circle: starts due north, clockwise, in equal azimuth steps.
fn circle(e: &Earth, c: [f64; 2], r: f64) -> Polygon<f64> {
    let n = arcgis_arc_segments(std::f64::consts::TAU, r, max_deviation(r));
    let pts: Vec<[f64; 2]> = (0..n)
        .map(|k| e.direct(c, 360.0 * k as f64 / n as f64, r))
        .collect();
    ring(pts)
}

/// How the stretches of an arc between its ends and the first/last 10-degree anchor are divided:
/// by the planar chord rule (the arcs of a line's buffer) or in steps of the grid (a polygon's).
#[derive(Clone, Copy)]
enum Partials {
    Planar,
    Grid,
}

/// The points of an arc around `c` at distance `r` from azimuth `az0` sweeping `sweep` degrees
/// (clockwise positive), ends included. Between its ends the arc runs along the global grid of
/// multiples of [`arc_step`]; the stretches from each end to the nearest grid point are divided by
/// the planar rule — as ArcGIS builds the arcs of line and polygon buffers.
fn arc_points(
    e: &Earth,
    c: [f64; 2],
    r: f64,
    az0: f64,
    sweep: f64,
    rule: Partials,
) -> Vec<[f64; 2]> {
    let step = arc_step(r);
    let dev = max_deviation_path(r);
    let (a, b) = (az0, az0 + sweep);
    let (lo, hi) = (a.min(b), a.max(b));
    // The 10-degree anchors strictly inside the arc, in its direction.
    let anchor = 10.0;
    let (k0, k1) = ((lo / anchor).ceil() as i64, (hi / anchor).floor() as i64);
    let mut anchors: Vec<f64> = (k0..=k1)
        .map(|k| k as f64 * anchor)
        .filter(|g| *g > lo + 1e-9 && *g < hi - 1e-9)
        .collect();
    if sweep < 0.0 {
        anchors.reverse();
    }
    let mut azs = vec![a];
    // `from` to `to` in equal parts, at most the planar rule's step.
    let planar = |azs: &mut Vec<f64>, from: f64, to: f64| {
        let n = match rule {
            Partials::Planar => arcgis_arc_segments((to - from).abs().to_radians(), r, dev),
            // Polygons: the stretch is divided into steps no longer than the grid's.
            Partials::Grid => (((to - from).abs() / step) - 1e-9).ceil().max(1.0) as usize,
        };
        azs.extend((1..=n).map(|j| from + (to - from) * j as f64 / n as f64));
    };
    match (anchors.first().copied(), anchors.last().copied()) {
        (Some(g1), Some(g2)) => {
            planar(&mut azs, a, g1);
            // Between the anchors, the fine grid.
            let n = (((g2 - g1).abs() / step).round() as usize).max(1);
            azs.extend((1..=n).map(|j| g1 + (g2 - g1) * j as f64 / n as f64));
            planar(&mut azs, g2, b);
        }
        _ => planar(&mut azs, a, b),
    }
    azs.into_iter().map(|az| e.direct(c, az, r)).collect()
}

/// A pie slice at `c` from azimuth `az0` sweeping `sweep` degrees (clockwise positive), its arc in
/// ArcGIS's step count; the tip is `apex`, a point inside the buffer so neighbouring pieces overlap.
fn sector(
    e: &Earth,
    c: [f64; 2],
    r: f64,
    az0: f64,
    sweep: f64,
    apex: Option<[f64; 2]>,
    rule: Partials,
) -> Polygon<f64> {
    let mut pts: Vec<[f64; 2]> = apex.into_iter().collect();
    pts.extend(arc_points(e, c, r, az0, sweep, rule));
    pts.extend(apex);
    // Pieces are kept clockwise in the plane; a clockwise sweep around `c` is already.
    if sweep < 0.0 {
        pts.reverse();
    }
    ring(pts)
}

/// The corners of the rectangle beside segment `a`–`b`: its left and right offsets at both ends.
fn body(e: &Earth, a: [f64; 2], b: [f64; 2], r: f64) -> Option<Polygon<f64>> {
    let (az1, az2) = e.azimuths(a, b)?;
    Some(ring(vec![
        e.direct(a, az1 - 90.0, r),
        e.direct(b, az2 - 90.0, r),
        e.direct(b, az2 + 90.0, r),
        e.direct(a, az1 + 90.0, r),
    ]))
}

/// Azimuth of the vector sum, for pulling an apex toward the inside of a bend.
fn mean_azimuth(a: f64, b: f64) -> f64 {
    // Unit vectors in (east, north).
    let (ax, ay, bx, by) = (
        a.to_radians().sin(),
        a.to_radians().cos(),
        b.to_radians().sin(),
        b.to_radians().cos(),
    );
    (bx - ax).atan2(by - ay).to_degrees()
}

type Caps = Vec<(Polygon<f64>, Polygon<f64>)>;

fn path_pieces(
    e: &Earth,
    paths: &[&[[f64; 2]]],
    r: f64,
    rule: Partials,
    pieces: &mut Vec<Polygon<f64>>,
    caps: &mut Caps,
) {
    for path in paths {
        let mut pts: Vec<[f64; 2]> = Vec::with_capacity(path.len());
        for p in path.iter() {
            if pts.last() != Some(p) {
                pts.push(*p);
            }
        }
        if pts.len() == 1 {
            pieces.push(circle(e, pts[0], r));
            continue;
        }
        let closed = pts.len() > 3 && pts.first() == pts.last();
        // Azimuths leaving and arriving at each segment's ends.
        let az: Vec<(f64, f64)> = pts
            .windows(2)
            .filter_map(|w| e.azimuths(w[0], w[1]))
            .collect();
        pieces.extend(pts.windows(2).filter_map(|w| body(e, w[0], w[1], r)));
        let joins: Vec<(usize, usize, usize)> = if closed {
            (0..az.len())
                .map(|k| ((k + az.len() - 1) % az.len(), k, k))
                .collect()
        } else {
            (1..az.len()).map(|k| (k - 1, k, k)).collect()
        };
        for (d_in, d_out, v) in joins {
            let (arrive, leave) = (az[d_in].1, az[d_out].0);
            let turn = wrap180(leave - arrive);
            if turn.abs() <= 1e-9 {
                continue;
            }
            // A left turn (counter-clockwise, negative) opens the gap on the right, from the
            // incoming right normal sweeping with the turn; a right turn on the left. A U-turn
            // needs a half-disc.
            let (az0, sweep) = if turn < 0.0 {
                (arrive + 90.0, turn)
            } else {
                (arrive - 90.0, turn)
            };
            let (az0, sweep) = if (turn.abs() - 180.0).abs() <= 1e-9 {
                (arrive + 90.0, -180.0)
            } else {
                (az0, sweep)
            };
            let inward = mean_azimuth(arrive, leave);
            let apex = e.direct(pts[v], inward, r * 0.5);
            pieces.push(sector(e, pts[v], r, az0, sweep, Some(apex), rule));
        }
        if !closed {
            let (last, first) = (az[az.len() - 1].1, az[0].0 + 180.0);
            for (end, dir) in [(pts[pts.len() - 1], last), (pts[0], first)] {
                let tip = |back: f64| e.direct(end, dir + 180.0, back);
                // From the left normal clockwise through `dir` to the right normal.
                let overlapping = sector(e, end, r, dir - 90.0, 180.0, Some(tip(r * 0.5)), rule);
                // The exact half-disc has no apex: its flat side is the chord between the corners.
                let exact = sector(e, end, r, dir - 90.0, 180.0, None, rule);
                caps.push((overlapping, exact));
            }
        }
    }
}

/// One side of a path's buffer (`side` +1 left, -1 right): see the planar `one_side_pieces`.
fn one_side_pieces(
    e: &Earth,
    path: &[[f64; 2]],
    r: f64,
    side: f64,
    round: bool,
) -> Vec<Polygon<f64>> {
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(path.len());
    for p in path {
        if pts.last() != Some(p) {
            pts.push(*p);
        }
    }
    let mut out = Vec::new();
    let az: Vec<(f64, f64)> = pts
        .windows(2)
        .filter_map(|w| e.azimuths(w[0], w[1]))
        .collect();
    if az.is_empty() {
        return out;
    }
    // The normal on `side` of a direction.
    let normal = |a: f64| a - 90.0 * side;
    for (k, w) in pts.windows(2).enumerate() {
        let (a1, a2) = az[k];
        out.push(clockwise(vec![
            w[0],
            w[1],
            e.direct(w[1], normal(a2), r),
            e.direct(w[0], normal(a1), r),
        ]));
    }
    for k in 1..az.len() {
        let (arrive, leave) = (az[k - 1].1, az[k].0);
        let turn = wrap180(leave - arrive);
        // The bend opens on this side when it turns away from it: a left turn (negative) opens on
        // the right, a right turn on the left.
        let opens = if side > 0.0 {
            turn > 1e-9
        } else {
            turn < -1e-9
        };
        if !opens {
            continue;
        }
        let (na, v) = (normal(arrive), pts[k]);
        let mut pieces = vec![v];
        pieces.extend(arc_points(e, v, r, na, turn, Partials::Planar));
        out.push(clockwise(pieces));
    }
    if round {
        let quarter = |center: [f64; 2], from: f64, sweep: f64| {
            let mut pieces = vec![center];
            pieces.extend(arc_points(e, center, r, from, sweep, Partials::Planar));
            clockwise(pieces)
        };
        let (start, end) = (pts[0], pts[pts.len() - 1]);
        let (d0, d1) = (az[0].0, az[az.len() - 1].1);
        // At the start: from the backward direction to the normal; at the end: from the normal to
        // the forward direction. Both sweep 90 degrees toward the side (clockwise for the left).
        out.push(quarter(start, d0 + 180.0, 90.0 * side));
        out.push(quarter(end, normal(d1), 90.0 * side));
    }
    out
}

fn buffer_mp(e: &Earth, g: &Geometry, opts: &GeodesicOptions) -> Result<MultiPolygon<f64>> {
    let d = opts.distance;
    let paths: Vec<&[[f64; 2]]> = g.part_ranges().map(|r| &g.xy[r]).collect();
    Ok(match g.kind {
        GeometryKind::Point | GeometryKind::Multipoint => {
            if d <= 0.0 {
                return Ok(MultiPolygon(vec![]));
            }
            let circles: Vec<Polygon<f64>> = g.xy.iter().map(|p| circle(e, *p, d)).collect();
            unary_union(circles.iter())
        }
        GeometryKind::Polyline => {
            if d <= 0.0 {
                return Ok(MultiPolygon(vec![]));
            }
            if let LineSide::Left | LineSide::Right = opts.side {
                let side = if opts.side == LineSide::Left {
                    1.0
                } else {
                    -1.0
                };
                let round = opts.line_end == LineEnd::Round;
                let pieces: Vec<Polygon<f64>> = paths
                    .iter()
                    .flat_map(|p| one_side_pieces(e, p, d, side, round))
                    .collect();
                return Ok(unary_union(pieces.iter()));
            }
            let (mut pieces, mut caps) = (Vec::new(), Vec::new());
            path_pieces(e, &paths, d, Partials::Planar, &mut pieces, &mut caps);
            match opts.line_end {
                LineEnd::Round => {
                    pieces.extend(caps.into_iter().map(|c| c.0));
                    unary_union(pieces.iter())
                }
                LineEnd::Flat => {
                    let exact: Vec<Polygon<f64>> = caps.iter().map(|c| c.1.clone()).collect();
                    pieces.extend(caps.into_iter().map(|c| c.0));
                    unary_union(pieces.iter()).difference(&unary_union(exact.iter()))
                }
            }
        }
        GeometryKind::Polygon => {
            let poly = to_multipolygon(g);
            if d == 0.0 {
                return Ok(poly);
            }
            let (mut pieces, mut caps) = (Vec::new(), Vec::new());
            path_pieces(e, &paths, d.abs(), Partials::Grid, &mut pieces, &mut caps);
            pieces.extend(caps.into_iter().map(|c| c.0));
            let band = unary_union(pieces.iter());
            if d > 0.0 && opts.side == LineSide::OutsideOnly {
                poly.union(&band).difference(&poly)
            } else if d > 0.0 {
                poly.union(&band)
            } else {
                poly.difference(&band)
            }
        }
        GeometryKind::MultiPatch => {
            return Err(Error::Unsupported("buffering multipatches".into()));
        }
    })
}

/// Whether a result stays clear of the antimeridian and the poles, where the pieces stop being
/// simple polygons in longitude/latitude.
fn within_world(mp: &MultiPolygon<f64>) -> bool {
    mp.0.iter().all(|p| {
        let xs = p.exterior().coords().map(|c| c.x);
        let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
        // Longitudes wrap, so a ring across the antimeridian shows as one spanning most of the world.
        hi - lo < 180.0 && p.exterior().coords().all(|c| c.y.abs() < 90.0)
    })
}

/// The geodesic buffer of one geometry, as polygons; `None` when empty.
pub fn buffer_geodesic(g: &Geometry, opts: &GeodesicOptions) -> Result<Option<Geometry>> {
    let mp = buffer_mp_checked(g, opts)?;
    Ok(from_multipolygon(&mp))
}

fn buffer_mp_checked(g: &Geometry, opts: &GeodesicOptions) -> Result<MultiPolygon<f64>> {
    if g.has_curves() {
        return Err(Error::Unsupported("geodesic buffer of curves".into()));
    }
    let mp = buffer_mp(&Earth::wgs84(), g, opts)?;
    if !within_world(&mp) {
        return Err(Error::Unsupported(
            "the geodesic buffer crosses the antimeridian or reaches a pole".into(),
        ));
    }
    Ok(mp)
}

/// The geodesic buffers of many geometries, dissolved into one.
pub fn buffer_geodesic_union<'a>(
    geoms: impl IntoIterator<Item = &'a Geometry>,
    opts: &GeodesicOptions,
) -> Result<Option<Geometry>> {
    let mut polys = Vec::new();
    for g in geoms {
        polys.extend(buffer_mp_checked(g, opts)?.0);
    }
    Ok(from_multipolygon(&unary_union(polys.iter())))
}

/// Shortest geodesic distance (metres) from `p` to the segment `a`–`b`, which is straight in
/// longitude/latitude as ArcGIS stores it, and the point attained. Scans the segment, then refines
/// around the best sample by golden section.
fn point_segment(earth: &Earth, p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, [f64; 2]) {
    let at = |t: f64| [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    let dist = |t: f64| earth.distance(p, at(t));
    if a == b {
        return (dist(0.0), a);
    }
    const SAMPLES: usize = 16;
    let (mut best_i, mut best_d) = (0, f64::INFINITY);
    for i in 0..=SAMPLES {
        let d = dist(i as f64 / SAMPLES as f64);
        if d < best_d {
            (best_i, best_d) = (i, d);
        }
    }
    let step = 1.0 / SAMPLES as f64;
    let (mut lo, mut hi) = (
        (best_i as f64 * step - step).max(0.0),
        (best_i as f64 * step + step).min(1.0),
    );
    let phi = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..60 {
        let (m1, m2) = (hi - phi * (hi - lo), lo + phi * (hi - lo));
        if dist(m1) < dist(m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    let t = (lo + hi) / 2.0;
    let refined = (dist(t), at(t));
    if refined.0 <= best_d {
        refined
    } else {
        (best_d, at(best_i as f64 * step))
    }
}

impl Earth {
    fn distance(&self, a: [f64; 2], b: [f64; 2]) -> f64 {
        let (s12, _, _, _): (f64, f64, f64, f64) = self.g.inverse(a[1], a[0], b[1], b[0]);
        s12
    }
}

/// The closest approach of two geometries in longitude/latitude, measured on the WGS 84 ellipsoid
/// (`Near` with `GEODESIC`): the distance in metres, and the points attained. Segments are straight
/// in longitude/latitude. Zero when they overlap. `None` if either is empty.
///
/// Between points and for polygons against points the results equal ArcGIS's; ArcGIS finds the
/// closest point on a line only approximately (up to 0.08 % off in distance).
pub fn nearest_geodesic(a: &Geometry, b: &Geometry) -> Result<Option<crate::Nearest>> {
    let Some(planar) = crate::nearest(a, b)? else {
        return Ok(None);
    };
    if planar.distance == 0.0 {
        return Ok(Some(planar));
    }
    let earth = Earth::wgs84();
    let (a, b) = (crate::clip::linear(a)?, crate::clip::linear(b)?);
    let (sa, sb) = (
        crate::proximity::segments(&a),
        crate::proximity::segments(&b),
    );
    let mut best: Option<crate::Nearest> = None;
    let mut keep = |distance: f64, from: [f64; 2], to: [f64; 2]| {
        if best.is_none_or(|n| distance < n.distance) {
            best = Some(crate::Nearest { distance, from, to });
        }
    };
    // The closest approach of two segments is at an end of one of them.
    for &(p, q) in &sa {
        for &(r, s) in &sb {
            for end in [p, q] {
                let (d, at) = point_segment(&earth, end, r, s);
                keep(d, end, at);
            }
            for end in [r, s] {
                let (d, at) = point_segment(&earth, end, p, q);
                keep(d, at, end);
            }
        }
    }
    Ok(best)
}

/// The point `distance` metres from `from` (longitude, latitude) along the WGS 84 geodesic with
/// azimuth `azimuth_degrees` (clockwise from north).
pub fn direct(from: [f64; 2], azimuth_degrees: f64, distance: f64) -> [f64; 2] {
    Earth::wgs84().direct(from, azimuth_degrees, distance)
}

/// The azimuth in degrees (clockwise from north, -180..180] at `from` of the geodesic to `to`; 0
/// when they coincide.
pub fn azimuth(from: [f64; 2], to: [f64; 2]) -> f64 {
    Earth::wgs84().azimuths(from, to).map_or(0.0, |(az, _)| az)
}

/// The WGS 84 geodesic distance in metres between two points (longitude, latitude).
pub fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    Earth::wgs84().distance(a, b)
}
