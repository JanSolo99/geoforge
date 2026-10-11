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
/// Polygons: the centroid when it lies inside; otherwise, on the horizontal
/// line through the envelope's center, the exterior span is split at the center
/// and the midpoint of the half that lies inside and farthest from any edge is
/// taken. (ArcGIS also searches vertically for shapes like a U; not reproduced.)
pub fn inside_point(g: &Geometry) -> Result<Option<Pt>> {
    let g = linear(g)?;
    if g.xy.is_empty() {
        return Ok(None);
    }
    let env = g.envelope();
    let center = [(env.xmin + env.xmax) / 2.0, (env.ymin + env.ymax) / 2.0];
    match g.kind {
        GeometryKind::Polyline => {
            let line = g.clone();
            let p = Geometry::point(center[0], center[1]);
            Ok(crate::proximity::nearest(&line, &p)?.map(|n| n.from))
        }
        GeometryKind::Polygon => {
            let mp = to_multipolygon(&g);
            if let Some(c) = mp.centroid()
                && geo::Contains::contains(&mp, &c)
            {
                return Ok(Some([c.x(), c.y()]));
            }
            let y = center[1];
            // Crossings of the horizontal line with exterior rings, and all rings.
            let crossings = |exterior_only: bool| {
                let mut xs: Vec<f64> = Vec::new();
                for poly in &mp.0 {
                    let rings = std::iter::once(poly.exterior())
                        .chain(poly.interiors().iter().filter(|_| !exterior_only));
                    for ring in rings {
                        for w in ring.0.windows(2) {
                            let (a, b) = (w[0], w[1]);
                            if (a.y > y) != (b.y > y) {
                                xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                            }
                        }
                    }
                }
                xs.sort_by(f64::total_cmp);
                xs
            };
            let outer = crossings(true);
            let all = crossings(false);
            let edge_distance = |x: f64| {
                all.iter()
                    .map(|&c| (c - x).abs())
                    .fold(f64::INFINITY, f64::min)
            };
            let is_inside = |x: f64| all.iter().filter(|&&c| c < x).count() % 2 == 1;
            let mut candidates: Vec<f64> = Vec::new();
            for span in outer.chunks(2).filter(|s| s.len() == 2) {
                if span[0] < center[0] && center[0] < span[1] {
                    candidates.push((span[0] + center[0]) / 2.0);
                    candidates.push((center[0] + span[1]) / 2.0);
                } else {
                    candidates.push((span[0] + span[1]) / 2.0);
                }
            }
            let best = candidates.into_iter().filter(|&x| is_inside(x)).fold(
                None::<(f64, f64)>,
                |best, x| {
                    let d = edge_distance(x);
                    match best {
                        Some((_, bd)) if bd >= d => best,
                        _ => Some((x, d)),
                    }
                },
            );
            Ok(best.map(|(x, _)| [x, y]).or_else(|| Some(g.xy[0])))
        }
        _ => Ok(centroid(&g)?),
    }
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
