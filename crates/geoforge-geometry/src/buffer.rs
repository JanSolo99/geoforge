//! Buffer (ArcGIS `analysis.Buffer`, planar). Built from simple pieces — a circle
//! per vertex, a rectangle per segment — unioned with `geo`, so the round parts
//! are densified with exactly ArcGIS's arc rule.

use std::f64::consts::TAU;

use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon, unary_union};
use geoforge_core::curves::arcgis_arc_segments;
use geoforge_core::{Error, Geometry, GeometryKind, Result};

use crate::convert::{from_multipolygon, to_multipolygon};

/// ArcGIS `line_end_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnd {
    Round,
    Flat,
}

/// ArcGIS `line_side`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineSide {
    #[default]
    Full,
    /// Lines: only the side left of the line's direction.
    Left,
    Right,
    /// Polygons: the buffer without the polygon itself.
    OutsideOnly,
}

#[derive(Debug, Clone, Copy)]
pub struct BufferOptions {
    /// Negative distances shrink polygons; they are ignored for points and lines.
    pub distance: f64,
    pub line_end: LineEnd,
    /// Maximum distance between an arc and its chords (ArcGIS: 0.1 m projected).
    pub max_deviation: f64,
    pub side: LineSide,
}

/// Circle with ArcGIS's vertex placement: starts due north, runs clockwise.
pub(crate) fn circle(c: [f64; 2], r: f64, max_deviation: f64) -> Polygon<f64> {
    let n = arcgis_arc_segments(TAU, r, max_deviation);
    let mut pts: Vec<Coord<f64>> = (0..n)
        .map(|k| {
            let a = TAU * k as f64 / n as f64;
            Coord {
                x: c[0] + r * a.sin(),
                y: c[1] + r * a.cos(),
            }
        })
        .collect();
    pts.push(pts[0]);
    Polygon::new(LineString::new(pts), vec![])
}

/// The rectangle around segment `a`–`b` at distance `r`.
fn capsule_body(a: [f64; 2], b: [f64; 2], r: f64) -> Option<Polygon<f64>> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return None;
    }
    let (nx, ny) = (-dy / len * r, dx / len * r);
    let c = |x: f64, y: f64| Coord { x, y };
    Some(Polygon::new(
        LineString::new(vec![
            c(a[0] + nx, a[1] + ny),
            c(b[0] + nx, b[1] + ny),
            c(b[0] - nx, b[1] - ny),
            c(a[0] - nx, a[1] - ny),
            c(a[0] + nx, a[1] + ny),
        ]),
        vec![],
    ))
}

/// A pie slice at `c` from angle `a0` sweeping `sweep` radians (signed), its arc
/// densified with ArcGIS's rule — the way ArcGIS densifies each arc of a buffer.
/// The slice's tip is `apex` rather than `c`: a point inside the line's buffer,
/// chosen so the slice overlaps its neighbours instead of merely touching them
/// (exactly touching pieces can fail to merge in floating point).
fn sector(c: [f64; 2], r: f64, a0: f64, sweep: f64, dev: f64, apex: [f64; 2]) -> Polygon<f64> {
    let n = arcgis_arc_segments(sweep, r, dev);
    let mut pts = vec![Coord {
        x: apex[0],
        y: apex[1],
    }];
    pts.extend((0..=n).map(|k| {
        let a = a0 + sweep * k as f64 / n as f64;
        Coord {
            x: c[0] + r * a.cos(),
            y: c[1] + r * a.sin(),
        }
    }));
    pts.push(Coord {
        x: apex[0],
        y: apex[1],
    });
    // The union treats ring direction as meaningful: keep every piece clockwise,
    // like the rectangles and circles.
    if sweep > 0.0 {
        pts.reverse();
    }
    Polygon::new(LineString::new(pts), vec![])
}

fn unit(a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy);
    (len > 0.0).then(|| [dx / len, dy / len])
}

/// Pieces whose union is the buffer of the paths (a ring is a closed path): a
/// rectangle per segment and an arc sector on the outer side of each bend, from
/// one segment's offset corner to the next. The half-discs at open ends go to
/// `caps` separately (FLAT ends are the round buffer minus these, as in ArcGIS).
fn path_pieces(
    paths: &[&[[f64; 2]]],
    r: f64,
    dev: f64,
    pieces: &mut Vec<Polygon<f64>>,
    caps: &mut Vec<(Polygon<f64>, Polygon<f64>)>,
) {
    for path in paths {
        // Repeated vertices have no direction.
        let mut pts: Vec<[f64; 2]> = Vec::with_capacity(path.len());
        for p in path.iter() {
            if pts.last() != Some(p) {
                pts.push(*p);
            }
        }
        if pts.len() == 1 {
            pieces.push(circle(pts[0], r, dev));
            continue;
        }
        let closed = pts.len() > 3 && pts.first() == pts.last();
        pieces.extend(pts.windows(2).filter_map(|w| capsule_body(w[0], w[1], r)));
        let dirs: Vec<[f64; 2]> = pts.windows(2).filter_map(|w| unit(w[0], w[1])).collect();
        // (incoming direction, outgoing direction, vertex) for every bend.
        let joins: Vec<(usize, usize, usize)> = if closed {
            (0..dirs.len())
                .map(|k| ((k + dirs.len() - 1) % dirs.len(), k, k))
                .collect()
        } else {
            (1..dirs.len()).map(|k| (k - 1, k, k)).collect()
        };
        for (d_in, d_out, v) in joins {
            let (a, b) = (dirs[d_in], dirs[d_out]);
            let cross = a[0] * b[1] - a[1] * b[0];
            let dot = a[0] * b[0] + a[1] * b[1];
            if cross.abs() <= 1e-12 && dot > 0.0 {
                continue; // straight on: no gap to fill
            }
            // Left turn: the gap opens on the right, from right normal (dy, -dx) of
            // `a` turning counter-clockwise. Right turn: on the left, from the left
            // normal (-dy, dx) turning clockwise. A U-turn needs a half-disc.
            let (a0, sweep) = if cross > 1e-12 {
                ((-a[0]).atan2(a[1]), cross.atan2(dot))
            } else if cross < -1e-12 {
                (a[0].atan2(-a[1]), cross.atan2(dot))
            } else {
                ((-a[0]).atan2(a[1]), std::f64::consts::PI)
            };
            // Tip pulled half a radius toward the inside of the bend.
            let (bx, by) = (b[0] - a[0], b[1] - a[1]);
            let blen = bx.hypot(by);
            let inward = if blen > 1e-12 {
                [bx / blen, by / blen]
            } else {
                [-a[0], -a[1]]
            };
            let apex = [
                pts[v][0] + inward[0] * r * 0.5,
                pts[v][1] + inward[1] * r * 0.5,
            ];
            pieces.push(sector(pts[v], r, a0, sweep, dev, apex));
        }
        if !closed {
            let last = dirs[dirs.len() - 1];
            let first = [-dirs[0][0], -dirs[0][1]];
            pieces_caps(caps, pts[pts.len() - 1], last, r, dev);
            pieces_caps(caps, pts[0], first, r, dev);
        }
    }
}

/// The half-disc beyond `end` in direction `dir`: from the left normal clockwise
/// through `dir` to the right normal.
/// Pushes an end's (overlapping cap, exact half-disc) pair.
fn pieces_caps(
    caps: &mut Vec<(Polygon<f64>, Polygon<f64>)>,
    end: [f64; 2],
    dir: [f64; 2],
    r: f64,
    dev: f64,
) {
    caps.push((
        end_cap(end, dir, r, dev, true),
        end_cap(end, dir, r, dev, false),
    ));
}

/// With `overlap`, the tip is pulled half a radius back along the line so the cap
/// overlaps the segment's rectangle (for unions); without, it is the exact
/// half-disc (for FLAT ends, which subtract it).
fn end_cap(end: [f64; 2], dir: [f64; 2], r: f64, dev: f64, overlap: bool) -> Polygon<f64> {
    let back = if overlap { r * 0.5 } else { 0.0 };
    let apex = [end[0] - dir[0] * back, end[1] - dir[1] * back];
    sector(
        end,
        r,
        dir[0].atan2(-dir[1]),
        -std::f64::consts::PI,
        dev,
        apex,
    )
}

/// One side of a path's buffer (`side` +1 left, -1 right of its direction): a
/// half-rectangle per segment, an arc wedge where a bend opens on that side, and
/// with `round` a quarter disc at each end. Neighbouring pieces share their corner
/// points exactly, so the union closes without slivers.
fn one_side_pieces(
    path: &[[f64; 2]],
    r: f64,
    side: f64,
    round: bool,
    dev: f64,
) -> Vec<Polygon<f64>> {
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(path.len());
    for p in path {
        if pts.last() != Some(p) {
            pts.push(*p);
        }
    }
    let mut out = Vec::new();
    if pts.len() < 2 {
        return out;
    }
    let c = |p: [f64; 2]| Coord { x: p[0], y: p[1] };
    let dirs: Vec<[f64; 2]> = pts.windows(2).map(|w| unit(w[0], w[1]).unwrap()).collect();
    let normal = |d: [f64; 2]| [-d[1] * side, d[0] * side];
    let off = |p: [f64; 2], n: [f64; 2]| [p[0] + n[0] * r, p[1] + n[1] * r];
    // A ring of `ring` points, oriented clockwise for the union.
    let polygon = |mut ring: Vec<[f64; 2]>| {
        ring.push(ring[0]);
        let area: f64 = ring
            .windows(2)
            .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
            .sum();
        if area > 0.0 {
            ring.reverse();
        }
        Polygon::new(LineString::new(ring.into_iter().map(c).collect()), vec![])
    };
    // Arc points from angle a0 sweeping `sweep`, without its ends.
    let arc = |center: [f64; 2], a0: f64, sweep: f64| -> Vec<[f64; 2]> {
        let n = arcgis_arc_segments(sweep.abs(), r, dev);
        (1..n)
            .map(|k| {
                let a = a0 + sweep * k as f64 / n as f64;
                [center[0] + r * a.cos(), center[1] + r * a.sin()]
            })
            .collect()
    };
    for (k, w) in pts.windows(2).enumerate() {
        let n = normal(dirs[k]);
        out.push(polygon(vec![w[0], w[1], off(w[1], n), off(w[0], n)]));
    }
    for k in 1..dirs.len() {
        let (a, b) = (dirs[k - 1], dirs[k]);
        let cross = a[0] * b[1] - a[1] * b[0];
        // The bend opens on this side when it turns away from it.
        if cross * side >= -1e-12 {
            continue;
        }
        let (na, nb) = (normal(a), normal(b));
        let a0 = na[1].atan2(na[0]);
        let sweep = (na[0] * nb[1] - na[1] * nb[0]).atan2(na[0] * nb[0] + na[1] * nb[1]);
        let mut ring = vec![pts[k], off(pts[k], na)];
        ring.extend(arc(pts[k], a0, sweep));
        ring.push(off(pts[k], nb));
        out.push(polygon(ring));
    }
    if round {
        // Quarter discs: at the start from the backward direction to the normal,
        // at the end from the normal to the forward direction.
        let (d0, d1) = (dirs[0], dirs[dirs.len() - 1]);
        let (start, end) = (pts[0], pts[pts.len() - 1]);
        let n0 = normal(d0);
        let back = [-d0[0], -d0[1]];
        let a0 = back[1].atan2(back[0]);
        let mut ring = vec![start, off(start, back)];
        ring.extend(arc(start, a0, -side * std::f64::consts::FRAC_PI_2));
        ring.push(off(start, n0));
        out.push(polygon(ring));
        let n1 = normal(d1);
        let a1 = n1[1].atan2(n1[0]);
        let mut ring = vec![end, off(end, n1)];
        ring.extend(arc(end, a1, -side * std::f64::consts::FRAC_PI_2));
        ring.push(off(end, d1));
        out.push(polygon(ring));
    }
    out
}

/// Buffers one geometry, returning polygons (Z/M dropped). `None` when the result
/// is empty (e.g. a polygon shrunk away by a negative distance).
pub fn buffer(g: &Geometry, opts: &BufferOptions) -> Result<Option<Geometry>> {
    let mp = buffer_mp(g, opts)?;
    Ok(from_multipolygon(&mp))
}

/// Buffers many geometries and dissolves the result into one (ArcGIS
/// `dissolve_option="ALL"`).
pub fn buffer_union<'a>(
    geoms: impl IntoIterator<Item = &'a Geometry>,
    opts: &BufferOptions,
) -> Result<Option<Geometry>> {
    let mut polys = Vec::new();
    for g in geoms {
        polys.extend(buffer_mp(g, opts)?.0);
    }
    Ok(from_multipolygon(&unary_union(polys.iter())))
}

fn buffer_mp(g: &Geometry, opts: &BufferOptions) -> Result<MultiPolygon<f64>> {
    let g = if g.has_curves() {
        g.densify_curves_arcgis(opts.max_deviation / 10.0)?
    } else {
        g.clone()
    };
    let d = opts.distance;
    let dev = opts.max_deviation;
    let paths: Vec<&[[f64; 2]]> = g.part_ranges().map(|r| &g.xy[r]).collect();
    Ok(match g.kind {
        GeometryKind::Point | GeometryKind::Multipoint => {
            if d <= 0.0 {
                return Ok(MultiPolygon(vec![]));
            }
            let circles: Vec<Polygon<f64>> = g.xy.iter().map(|p| circle(*p, d, dev)).collect();
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
                    .flat_map(|p| one_side_pieces(p, d, side, round, dev))
                    .collect();
                return Ok(unary_union(pieces.iter()));
            }
            let (mut pieces, mut caps) = (Vec::new(), Vec::new());
            path_pieces(&paths, d, dev, &mut pieces, &mut caps);
            match opts.line_end {
                LineEnd::Round => {
                    pieces.extend(caps.into_iter().map(|c| c.0));
                    unary_union(pieces.iter())
                }
                // ArcGIS FLAT: the round buffer minus the end half-discs, even where
                // other parts of the line's buffer reach into them.
                LineEnd::Flat => {
                    let exact: Vec<Polygon<f64>> = caps.iter().map(|c| c.1.clone()).collect();
                    pieces.extend(caps.into_iter().map(|c| c.0));
                    unary_union(pieces.iter()).difference(&unary_union(exact.iter()))
                }
            }
        }
        GeometryKind::Polygon => {
            let poly = to_multipolygon(&g);
            if d == 0.0 {
                return Ok(poly);
            }
            // The boundary's buffer, added for growth or removed for shrinking.
            let (mut pieces, mut caps) = (Vec::new(), Vec::new());
            path_pieces(&paths, d.abs(), dev, &mut pieces, &mut caps);
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

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Area;

    fn line(xy: Vec<[f64; 2]>) -> Geometry {
        Geometry {
            kind: GeometryKind::Polyline,
            parts: vec![0],
            part_types: None,
            xy,
            z: None,
            m: None,
            curves: Vec::new(),
        }
    }

    fn area(g: &Geometry, d: f64, end: LineEnd) -> f64 {
        let opts = BufferOptions {
            distance: d,
            line_end: end,
            max_deviation: 1e-6,
            side: LineSide::Full,
        };
        buffer_mp(g, &opts).unwrap().unsigned_area()
    }

    #[test]
    fn one_sided_buffers() {
        let side_area = |g: &Geometry, side: LineSide, end: LineEnd| {
            let opts = BufferOptions {
                distance: 1.0,
                line_end: end,
                max_deviation: 1e-6,
                side,
            };
            let mp = buffer_mp(g, &opts).unwrap();
            (mp.unsigned_area(), mp)
        };
        let pi = std::f64::consts::PI;
        let g = line(vec![[0.0, 0.0], [10.0, 0.0]]);
        let (a, mp) = side_area(&g, LineSide::Left, LineEnd::Flat);
        assert!((a - 10.0).abs() < 1e-9, "{a}");
        // Left of an eastward line is north.
        assert!(mp.0[0].exterior().0.iter().all(|c| c.y >= 0.0));
        let (a, _) = side_area(&g, LineSide::Right, LineEnd::Round);
        assert!((a - (10.0 + pi / 2.0)).abs() < 1e-3, "{a}");
        // A right-angle bend to the right: the left side gains a quarter disc.
        let bend = line(vec![[0.0, 0.0], [10.0, 0.0], [10.0, -10.0]]);
        let (a, _) = side_area(&bend, LineSide::Left, LineEnd::Flat);
        assert!((a - (20.0 + pi / 4.0)).abs() < 1e-3, "{a}");
        // On the inside of the bend the two strips overlap in a unit square.
        let (a, _) = side_area(&bend, LineSide::Right, LineEnd::Flat);
        assert!((a - 19.0).abs() < 1e-9, "{a}");
    }

    #[test]
    fn straight_line_round_and_flat() {
        let g = line(vec![[0.0, 0.0], [10.0, 0.0]]);
        let pi = std::f64::consts::PI;
        assert!(
            (area(&g, 1.0, LineEnd::Round) - (20.0 + pi)).abs() < 1e-3,
            "{}",
            area(&g, 1.0, LineEnd::Round)
        );
        assert!(
            (area(&g, 1.0, LineEnd::Flat) - 20.0).abs() < 1e-3,
            "{}",
            area(&g, 1.0, LineEnd::Flat)
        );
    }

    #[test]
    fn right_angle_bend() {
        // L shape: (0,0)-(10,0)-(10,10); round buffer of 1 = two 10x2 strips minus the
        // overlapping 1x1 inner square... computed: 20+20 - 1 (inner overlap square)
        // + quarter disc at the outer corner (pi/4) + 2 half discs (pi) - 1 (outer
        // corner square counted by neither strip) ... checked numerically below.
        let g = line(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        let pi = std::f64::consts::PI;
        let expected =
            2.0 * 10.0 * 2.0 - 1.0 /* overlap */ + pi / 4.0 /* outer quarter */ - 0.0 + pi;
        let got = area(&g, 1.0, LineEnd::Round);
        assert!(
            (got - expected).abs() < 1e-3,
            "got {got}, expected {expected}"
        );
    }

    #[test]
    fn square_grow_and_shrink() {
        let sq = Geometry {
            kind: GeometryKind::Polygon,
            parts: vec![0],
            part_types: None,
            xy: vec![
                [0.0, 0.0],
                [0.0, 10.0],
                [10.0, 10.0],
                [10.0, 0.0],
                [0.0, 0.0],
            ],
            z: None,
            m: None,
            curves: Vec::new(),
        };
        let pi = std::f64::consts::PI;
        assert!(
            (area(&sq, 1.0, LineEnd::Round) - (100.0 + 40.0 + pi)).abs() < 1e-3,
            "{}",
            area(&sq, 1.0, LineEnd::Round)
        );
        assert!(
            (area(&sq, -1.0, LineEnd::Round) - 64.0).abs() < 1e-3,
            "{}",
            area(&sq, -1.0, LineEnd::Round)
        );
    }
}
