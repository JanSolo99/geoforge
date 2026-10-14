//! Bounding geometries and representative points: Minimum Bounding Geometry,
//! Feature To Point.

use geo::{Centroid, ConvexHull, Coord, LineString, MultiPoint, Point, Polygon};
use geoforge_core::{Geometry, GeometryKind, Result};

use crate::clip::linear;
use crate::convert::{from_multipolygon, to_multilinestring, to_multipolygon};

type Pt = [f64; 2];

/// A bounding polygon and the measures ArcGIS reports with `MBG_FIELDS`.
#[derive(Debug, Clone)]
pub struct Bounding {
    pub geometry: Geometry,
    /// Shorter and longer side (rectangles, envelopes); for a convex hull the
    /// narrowest width and the longest distance between two vertices.
    pub width: f64,
    pub length: f64,
    /// Degrees clockwise from north (0–180) of the longer side / longest span.
    pub orientation: f64,
    pub diameter: f64,
}

/// ArcGIS `geometry_type` of Minimum Bounding Geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundingKind {
    Envelope,
    ConvexHull,
    RectangleByArea,
    RectangleByWidth,
    Circle,
}

fn polygon_geometry(ring: Vec<Pt>) -> Option<Geometry> {
    let coords: Vec<Coord<f64>> = ring.iter().map(|p| Coord { x: p[0], y: p[1] }).collect();
    from_multipolygon(&geo::MultiPolygon(vec![Polygon::new(
        LineString::new(coords),
        vec![],
    )]))
}

/// Orientation of the segment a–b: degrees clockwise from north, 0 to 180.
fn azimuth(a: Pt, b: Pt) -> f64 {
    let deg = (b[0] - a[0]).atan2(b[1] - a[1]).to_degrees();
    let d = deg.rem_euclid(180.0);
    if d == 180.0 { 0.0 } else { d }
}

fn hull(points: &[Pt]) -> Vec<Pt> {
    let mp = MultiPoint::new(points.iter().map(|p| Point::new(p[0], p[1])).collect());
    mp.convex_hull()
        .exterior()
        .0
        .iter()
        .map(|c| [c.x, c.y])
        .collect()
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Rectangles aligned with each hull edge: (corners, extent along, across, edge).
fn edge_rectangles(h: &[Pt]) -> Vec<([Pt; 4], f64, f64, Pt)> {
    let mut out = Vec::new();
    for w in h.windows(2) {
        let len = dist(w[0], w[1]);
        if len == 0.0 {
            continue;
        }
        let u = [(w[1][0] - w[0][0]) / len, (w[1][1] - w[0][1]) / len];
        let v = [-u[1], u[0]];
        let (mut a0, mut a1, mut b0, mut b1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for p in h {
            let d = [p[0] - w[0][0], p[1] - w[0][1]];
            let (a, b) = (d[0] * u[0] + d[1] * u[1], d[0] * v[0] + d[1] * v[1]);
            a0 = a0.min(a);
            a1 = a1.max(a);
            b0 = b0.min(b);
            b1 = b1.max(b);
        }
        let at = |a: f64, b: f64| [w[0][0] + u[0] * a + v[0] * b, w[0][1] + u[1] * a + v[1] * b];
        out.push((
            [at(a0, b0), at(a0, b1), at(a1, b1), at(a1, b0)],
            a1 - a0,
            b1 - b0,
            u,
        ));
    }
    out
}

/// The smallest circle around the points (Welzl, iterative).
fn enclosing_circle(points: &[Pt]) -> (Pt, f64) {
    let circle2 = |a: Pt, b: Pt| ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], dist(a, b) / 2.0);
    let circle3 = |a: Pt, b: Pt, c: Pt| {
        let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
        if d.abs() < 1e-12 {
            // Collinear: the circle on the farthest pair.
            let pairs = [(a, b), (a, c), (b, c)];
            let (p, q) = pairs
                .into_iter()
                .max_by(|x, y| dist(x.0, x.1).total_cmp(&dist(y.0, y.1)))
                .unwrap();
            return circle2(p, q);
        }
        let sq = |p: Pt| p[0] * p[0] + p[1] * p[1];
        let ux = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
        let uy = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
        let center = [ux, uy];
        (center, dist(center, a))
    };
    let inside = |c: &(Pt, f64), p: Pt| dist(c.0, p) <= c.1 * (1.0 + 1e-12) + 1e-9;
    let pts = points;
    let mut c = (pts[0], 0.0);
    for i in 1..pts.len() {
        if inside(&c, pts[i]) {
            continue;
        }
        c = (pts[i], 0.0);
        for j in 0..i {
            if inside(&c, pts[j]) {
                continue;
            }
            c = circle2(pts[i], pts[j]);
            for k in 0..j {
                if !inside(&c, pts[k]) {
                    c = circle3(pts[i], pts[j], pts[k]);
                }
            }
        }
    }
    c
}

fn farthest_pair(points: &[Pt]) -> (Pt, Pt) {
    // Along a line the extremes are those farthest from the first point and then
    // from that one.
    let far = |from: Pt| {
        *points
            .iter()
            .max_by(|a, b| dist(from, **a).total_cmp(&dist(from, **b)))
            .unwrap()
    };
    let a = far(points[0]);
    (a, far(a))
}

fn collinear(points: &[Pt]) -> bool {
    let (a, b) = farthest_pair(points);
    let len = dist(a, b);
    len == 0.0
        || points.iter().all(|p| {
            ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])).abs() / len
                <= 1e-9 * len.max(1.0)
        })
}

/// The bounding geometry of a set of geometries' vertices.
pub fn bounding<'a>(
    geoms: impl IntoIterator<Item = &'a Geometry>,
    kind: BoundingKind,
    max_deviation: f64,
) -> Result<Option<Bounding>> {
    let mut points: Vec<Pt> = Vec::new();
    for g in geoms {
        points.extend(linear(g)?.xy);
    }
    if points.is_empty() {
        return Ok(None);
    }
    let (x0, y0) = points.iter().fold((f64::INFINITY, f64::INFINITY), |a, p| {
        (a.0.min(p[0]), a.1.min(p[1]))
    });
    let (x1, y1) = points
        .iter()
        .fold((f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
            (a.0.max(p[0]), a.1.max(p[1]))
        });
    let result = match kind {
        BoundingKind::Envelope => {
            let (w, h) = (x1 - x0, y1 - y0);
            let ring = vec![[x0, y0], [x0, y1], [x1, y1], [x1, y0], [x0, y0]];
            polygon_geometry(ring).map(|geometry| Bounding {
                geometry,
                width: w.min(h),
                length: w.max(h),
                orientation: if w >= h { 90.0 } else { 0.0 },
                diameter: 0.0,
            })
        }
        // Collinear points (a straight line): ArcGIS widens the hull and the
        // rectangles 1 unit to each side of the line.
        _ if kind != BoundingKind::Circle && collinear(&points) => {
            let (a, b) = farthest_pair(&points);
            let len = dist(a, b);
            if len == 0.0 {
                return Ok(None);
            }
            let n = [-(b[1] - a[1]) / len, (b[0] - a[0]) / len];
            let off = |p: Pt, s: f64| [p[0] + n[0] * s, p[1] + n[1] * s];
            let ring = vec![
                off(a, 1.0),
                off(b, 1.0),
                off(b, -1.0),
                off(a, -1.0),
                off(a, 1.0),
            ];
            polygon_geometry(ring).map(|geometry| Bounding {
                geometry,
                width: 2.0,
                length: len,
                orientation: azimuth(a, b),
                diameter: 0.0,
            })
        }
        BoundingKind::ConvexHull => {
            let h = hull(&points);
            let (mut length, mut orientation) = (0.0, 0.0);
            for (i, a) in h.iter().enumerate() {
                for b in &h[i + 1..] {
                    if dist(*a, *b) > length {
                        length = dist(*a, *b);
                        orientation = azimuth(*a, *b);
                    }
                }
            }
            let width = edge_rectangles(&h)
                .iter()
                .map(|r| r.2)
                .fold(f64::INFINITY, f64::min);
            polygon_geometry(h).map(|geometry| Bounding {
                geometry,
                width: if width.is_finite() { width } else { 0.0 },
                length,
                orientation,
                diameter: 0.0,
            })
        }
        BoundingKind::RectangleByArea | BoundingKind::RectangleByWidth => {
            let h = hull(&points);
            let rects = edge_rectangles(&h);
            let key = |r: &([Pt; 4], f64, f64, Pt)| match kind {
                BoundingKind::RectangleByArea => r.1 * r.2,
                _ => r.1.min(r.2),
            };
            // Equal areas (or widths): ArcGIS takes the most axis-aligned rectangle.
            let skew = |r: &([Pt; 4], f64, f64, Pt)| {
                let a = r.3[1].atan2(r.3[0]).rem_euclid(std::f64::consts::FRAC_PI_2);
                a.min(std::f64::consts::FRAC_PI_2 - a)
            };
            let Some(best) = rects.into_iter().min_by(|a, b| {
                let (ka, kb) = (key(a), key(b));
                if (ka - kb).abs() <= 1e-9 * ka.max(kb) {
                    skew(a).total_cmp(&skew(b))
                } else {
                    ka.total_cmp(&kb)
                }
            }) else {
                return Ok(None);
            };
            let (c, along, across, u) = best;
            let mut ring = c.to_vec();
            ring.push(c[0]);
            let long_dir = if along >= across { u } else { [-u[1], u[0]] };
            polygon_geometry(ring).map(|geometry| Bounding {
                geometry,
                width: along.min(across),
                length: along.max(across),
                orientation: azimuth([0.0, 0.0], long_dir),
                diameter: 0.0,
            })
        }
        BoundingKind::Circle => {
            let (center, r) = enclosing_circle(&points);
            let poly = crate::buffer::circle(center, r, max_deviation);
            from_multipolygon(&geo::MultiPolygon(vec![poly])).map(|geometry| Bounding {
                geometry,
                width: 0.0,
                length: 0.0,
                orientation: 0.0,
                diameter: 2.0 * r,
            })
        }
    };
    Ok(result)
}

/// The centroid ArcGIS's Feature To Point (`CENTROID`) gives: area-weighted for
/// polygons, length-weighted for lines, the mean of points.
pub fn centroid(g: &Geometry) -> Result<Option<Pt>> {
    let g = linear(g)?;
    if g.xy.is_empty() {
        return Ok(None);
    }
    let c = match g.kind {
        GeometryKind::Polygon => to_multipolygon(&g).centroid(),
        GeometryKind::Polyline => to_multilinestring(&g).centroid(),
        _ => MultiPoint::new(g.xy.iter().map(|p| Point::new(p[0], p[1])).collect()).centroid(),
    };
    Ok(c.map(|p| [p.x(), p.y()]))
}

/// A point inside a polygon / on a line (Feature To Point `INSIDE`, arcpy's
/// label point). Lines: the point on the line nearest its envelope's center.
/// Polygons: see [`polygon_label_point`].
pub fn inside_point(g: &Geometry) -> Result<Option<Pt>> {
    let g = linear(g)?;
    if g.xy.is_empty() {
        return Ok(None);
    }
    match g.kind {
        GeometryKind::Polyline => {
            let env = g.envelope();
            let center = [(env.xmin + env.xmax) / 2.0, (env.ymin + env.ymax) / 2.0];
            let line = g.clone();
            let p = Geometry::point(center[0], center[1]);
            Ok(crate::proximity::nearest(&line, &p)?.map(|n| n.from))
        }
        GeometryKind::Polygon => Ok(polygon_label_point(&g)),
        _ => Ok(centroid(&g)?),
    }
}

type Ring = Vec<Pt>;

fn ring_area(r: &[Pt]) -> f64 {
    let n = r.len();
    (0..n)
        .map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1])
        .sum::<f64>()
        / 2.0
}

fn ring_centroid(r: &[Pt]) -> Pt {
    let n = r.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (r[i], r[(i + 1) % n]);
        let f = p[0] * q[1] - q[0] * p[1];
        a += f;
        cx += (p[0] + q[0]) * f;
        cy += (p[1] + q[1]) * f;
    }
    if a == 0.0 {
        r[0]
    } else {
        [cx / (3.0 * a), cy / (3.0 * a)]
    }
}

/// Area centroid with signed areas (holes subtract).
fn rings_centroid(rings: &[&Ring]) -> Pt {
    let (mut a, mut x, mut y) = (0.0, 0.0, 0.0);
    for r in rings {
        let area = ring_area(r);
        let c = ring_centroid(r);
        a += area;
        x += area * c[0];
        y += area * c[1];
    }
    if a == 0.0 {
        rings[0][0]
    } else {
        [x / a, y / a]
    }
}

fn ring_edges<'a>(rings: &'a [&'a Ring]) -> impl Iterator<Item = (Pt, Pt)> + 'a {
    rings.iter().flat_map(|r| {
        let n = r.len();
        (0..n).map(move |i| (r[i], r[(i + 1) % n]))
    })
}

/// Even-odd containment.
fn rings_contain(rings: &[&Ring], p: Pt) -> bool {
    let mut inside = false;
    for (a, b) in ring_edges(rings) {
        if (a[1] > p[1]) != (b[1] > p[1])
            && a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) > p[0]
        {
            inside = !inside;
        }
    }
    inside
}

fn boundary_distance(rings: &[&Ring], p: Pt) -> f64 {
    ring_edges(rings)
        .map(|(a, b)| {
            let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
            let l = ex * ex + ey * ey;
            let t = if l == 0.0 {
                0.0
            } else {
                (((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / l).clamp(0.0, 1.0)
            };
            (p[0] - a[0] - t * ex).hypot(p[1] - a[1] - t * ey)
        })
        .fold(f64::INFINITY, f64::min)
}

fn shares_vertex(a: &Ring, b: &Ring) -> bool {
    a.iter().any(|p| b.contains(p))
}

/// The midpoint of the lowest stretch of the vertical line `X = x` inside the part.
fn lowest_chord_midpoint(rings: &[&Ring], x: f64) -> Option<f64> {
    let mut ys: Vec<f64> = ring_edges(rings)
        .filter(|(a, b)| (a[0] > x) != (b[0] > x))
        .map(|(a, b)| a[1] + (x - a[0]) * (b[1] - a[1]) / (b[0] - a[0]))
        .collect();
    if ys.len() < 2 {
        return None;
    }
    ys.sort_by(f64::total_cmp);
    Some((ys[0] + ys[1]) / 2.0)
}

/// ArcGIS's polygon label point (arcpy `Polygon.labelPoint`, Feature To Point
/// `INSIDE`), reconstructed from its outputs (`docs/formats/label-point.md`).
/// Only the largest part counts. Candidates: the area centroid of its outer ring
/// (when inside the part; weight 6) and the midpoints of the lowest stretch of the
/// vertical lines at 1/2, 3/4 and 1/4 of the envelope width (weights 5, 4, 3 by
/// distance to the centroid). The candidate with the largest distance to the part's
/// boundary times its weight wins. Rings are expected as ArcGIS stores them: outer
/// rings clockwise, holes counter-clockwise.
fn polygon_label_point(g: &Geometry) -> Option<Pt> {
    let mut rings: Vec<Ring> = g
        .part_ranges()
        .map(|r| {
            let mut v = g.xy[r].to_vec();
            if v.len() > 1 && v.first() == v.last() {
                v.pop();
            }
            v
        })
        .filter(|v| v.len() >= 3)
        .collect();
    if rings.is_empty() {
        return None;
    }
    let mut outers: Vec<Ring> = rings
        .iter()
        .filter(|r| ring_area(r) < 0.0)
        .cloned()
        .collect();
    let holes: Vec<Ring> = rings
        .iter()
        .filter(|r| ring_area(r) > 0.0)
        .cloned()
        .collect();
    let holes = if outers.is_empty() {
        // Orientation not Esri's: every ring is a part of its own.
        outers = rings
            .drain(..)
            .map(|mut r| {
                r.reverse();
                r
            })
            .collect();
        Vec::new()
    } else {
        holes
    };
    let mut parts: Vec<Vec<Ring>> = outers.into_iter().map(|o| vec![o]).collect();
    for h in holes {
        let hc = ring_centroid(&h);
        if let Some(p) = parts
            .iter_mut()
            .find(|p| shares_vertex(&h, &p[0]) || rings_contain(&[&p[0]], hc))
        {
            p.push(h);
        }
    }
    // The part with the largest net area (the first on a tie).
    let mut best_part = 0;
    let mut best_area = f64::NEG_INFINITY;
    for (i, p) in parts.iter().enumerate() {
        let a: f64 = -p.iter().map(|r| ring_area(r)).sum::<f64>();
        if a > best_area {
            best_area = a;
            best_part = i;
        }
    }
    let part: Vec<&Ring> = parts[best_part].iter().collect();
    let outer = part[0];
    let touching: Vec<&Ring> = std::iter::once(outer)
        .chain(
            part[1..]
                .iter()
                .copied()
                .filter(|h| shares_vertex(h, outer)),
        )
        .collect();
    let c = rings_centroid(&touching);
    let (x0, x1) = part
        .iter()
        .flat_map(|r| r.iter())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[0]), hi.max(p[0]))
        });
    // 1/2, 3/4, 1/4: the order breaks distance ties (3/4 before 1/4).
    let mut lines: Vec<Pt> = [0.5, 0.75, 0.25]
        .iter()
        .filter_map(|f| {
            let x = x0 + (x1 - x0) * f;
            lowest_chord_midpoint(&part, x).map(|y| [x, y])
        })
        .collect();
    lines.sort_by(|a, b| {
        (a[0] - c[0])
            .hypot(a[1] - c[1])
            .total_cmp(&(b[0] - c[0]).hypot(b[1] - c[1]))
    });
    let mut candidates: Vec<(Pt, f64)> = Vec::new();
    if rings_contain(&part, c) {
        candidates.push((c, 6.0));
    }
    candidates.extend(lines.iter().enumerate().map(|(i, &q)| (q, 5.0 - i as f64)));
    let mut best: Option<(f64, Pt)> = None;
    for (q, w) in candidates {
        let score = boundary_distance(&part, q) * w;
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, q));
        }
    }
    Some(best.map_or(c, |(_, q)| q))
}

/// Area of a polygon (holes subtracted; curves densified).
pub fn area(g: &Geometry) -> Result<f64> {
    use geo::Area;
    Ok(to_multipolygon(&linear(g)?).unsigned_area())
}

/// Length of a line, or the perimeter of a polygon (all rings).
pub fn length(g: &Geometry) -> Result<f64> {
    use geo::Length;
    Ok(to_multilinestring(&linear(g)?)
        .0
        .iter()
        .map(|l| geo::Euclidean.length(l))
        .sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A polygon from closed rings in Esri orientation (outer clockwise).
    fn polygon(rings: &[&[Pt]]) -> Geometry {
        let mut g = Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Polygon;
        g.xy.clear();
        for r in rings {
            g.parts.push(g.xy.len() as u32);
            g.xy.extend_from_slice(r);
        }
        g
    }

    /// ArcGIS's label points for hand-made shapes (arcpy `labelPoint`).
    #[test]
    fn label_point_of_u_c_and_ring_shapes() {
        let u_up: &[Pt] = &[
            [0., 0.],
            [0., 30.],
            [10., 30.],
            [10., 10.],
            [20., 10.],
            [20., 30.],
            [30., 30.],
            [30., 0.],
            [0., 0.],
        ];
        let c_tall: &[Pt] = &[
            [0., 0.],
            [0., 100.],
            [30., 100.],
            [30., 90.],
            [10., 90.],
            [10., 10.],
            [30., 10.],
            [30., 0.],
            [0., 0.],
        ];
        let u_down: &[Pt] = &[
            [0., 0.],
            [0., 30.],
            [30., 30.],
            [30., 0.],
            [20., 0.],
            [20., 20.],
            [10., 20.],
            [10., 0.],
            [0., 0.],
        ];
        let outer: &[Pt] = &[[0., 0.], [0., 40.], [40., 40.], [40., 0.], [0., 0.]];
        let hole: &[Pt] = &[[10., 10.], [30., 10.], [30., 30.], [10., 30.], [10., 10.]];
        for (g, want) in [
            (polygon(&[u_up]), [15., 5.]),
            (polygon(&[c_tall]), [15., 5.]),
            (polygon(&[u_down]), [15., 25.]),
            (polygon(&[outer, hole]), [20., 5.]),
        ] {
            let p = inside_point(&g).unwrap().unwrap();
            assert!(
                (p[0] - want[0]).abs() < 1e-9 && (p[1] - want[1]).abs() < 1e-9,
                "{p:?}"
            );
        }
    }
}
