//! Conversion between the Esri geometry model and `geo` types (2D).

use geo::{Coord, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon};
use unigis_core::geometry::{point_in_ring, ring_signed_area2};
use unigis_core::{Geometry, GeometryKind};

fn coords(xy: &[[f64; 2]]) -> Vec<Coord<f64>> {
    xy.iter().map(|&[x, y]| Coord { x, y }).collect()
}

/// Esri polygon rings → polygons. Clockwise rings are shells, counter-clockwise
/// rings holes; each hole belongs to the smallest shell containing it (as ArcGIS
/// reads them). A hole outside every shell is kept as a shell.
pub fn to_multipolygon(g: &Geometry) -> MultiPolygon<f64> {
    let rings: Vec<&[[f64; 2]]> = g
        .part_ranges()
        .map(|r| &g.xy[r])
        .filter(|r| r.len() >= 3)
        .collect();
    let mut shells: Vec<(usize, f64)> = Vec::new();
    let mut holes = Vec::new();
    for (i, r) in rings.iter().enumerate() {
        let a = ring_signed_area2(r);
        if a < 0.0 {
            shells.push((i, -a));
        } else {
            holes.push(i);
        }
    }
    let mut owned: Vec<Vec<usize>> = vec![Vec::new(); shells.len()];
    let mut orphans = Vec::new();
    for h in holes {
        let probe = rings[h][0];
        let owner = shells
            .iter()
            .enumerate()
            .filter(|(_, (s, _))| point_in_ring(probe, rings[*s]))
            .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
            .map(|(k, _)| k);
        match owner {
            Some(k) => owned[k].push(h),
            None => orphans.push(h),
        }
    }
    let mut polys: Vec<Polygon<f64>> = shells
        .iter()
        .zip(owned)
        .map(|(&(s, _), hs)| {
            Polygon::new(
                LineString::new(coords(rings[s])),
                hs.into_iter()
                    .map(|h| LineString::new(coords(rings[h])))
                    .collect(),
            )
        })
        .collect();
    polys.extend(
        orphans
            .into_iter()
            .map(|h| Polygon::new(LineString::new(coords(rings[h])), vec![])),
    );
    MultiPolygon(polys)
}

/// Polygons → Esri polygon (clockwise shells, counter-clockwise holes), or `None`
/// when empty.
pub fn from_multipolygon(mp: &MultiPolygon<f64>) -> Option<Geometry> {
    let mut g = empty(GeometryKind::Polygon);
    for p in &mp.0 {
        for ring in std::iter::once(p.exterior()).chain(p.interiors()) {
            let Some(ring) = clean_ring(&ring.0) else {
                continue;
            };
            g.parts.push(g.xy.len() as u32);
            g.xy.extend(ring);
        }
    }
    if g.xy.is_empty() {
        return None;
    }
    g.orient_rings_esri();
    Some(g)
}

/// Distance below which a spike counts as degenerate: about 1 mm at projected
/// coordinate magnitudes (2e-10 relative), comparable to ArcGIS's XY tolerance.
fn spike_tolerance(pts: &[[f64; 2]]) -> f64 {
    let mag = pts
        .iter()
        .fold(1.0f64, |m, p| m.max(p[0].abs()).max(p[1].abs()));
    2e-10 * mag
}

/// `b` is the tip of a spike: `a` and `c` lie on the same side of it, and one of
/// them is within `tol` of the line through `b` and the other.
fn is_spike(a: [f64; 2], b: [f64; 2], c: [f64; 2], tol: f64) -> bool {
    let (u, v) = ([a[0] - b[0], a[1] - b[1]], [c[0] - b[0], c[1] - b[1]]);
    let dot = u[0] * v[0] + u[1] * v[1];
    if dot <= 0.0 {
        return false; // a real corner, not a fold-back
    }
    let cross = (u[0] * v[1] - u[1] * v[0]).abs();
    let (lu, lv) = (u[0].hypot(u[1]), v[0].hypot(v[1]));
    // Height of the shorter arm's end above the longer arm's line.
    cross / lu.max(lv) <= tol
}

/// Overlay can leave degenerate pieces where inputs touch exactly: repeated points,
/// spikes (A, B, A) and zero-area rings. Returns the closed, cleaned ring, or `None`
/// if nothing with area remains.
fn clean_ring(coords: &[Coord<f64>]) -> Option<Vec<[f64; 2]>> {
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(coords.len() + 1);
    for c in coords {
        if pts.last() != Some(&[c.x, c.y]) {
            pts.push([c.x, c.y]);
        }
    }
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    // Remove spikes until none are left (removing one can expose another). A spike
    // is a vertex whose neighbours lie in (nearly) the same direction from it:
    // overlay leaves these, sub-millimetre wide, where edges cross almost exactly.
    let tol = spike_tolerance(&pts);
    let mut changed = true;
    while changed && pts.len() >= 3 {
        changed = false;
        let n = pts.len();
        for i in 0..n {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            if a == c || is_spike(a, b, c, tol) {
                pts.remove(i);
                // Merge the neighbours if they coincide now.
                let n = pts.len();
                if n >= 2 {
                    let j = i % n;
                    let prev = (j + n - 1) % n;
                    if pts[prev] == pts[j] {
                        pts.remove(j);
                    }
                }
                changed = true;
                break;
            }
        }
    }
    if pts.len() < 3 {
        return None;
    }
    // Negligible area relative to the ring's own extent: a sliver, not a ring.
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &pts {
        (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
    }
    let o = pts[0];
    let area2: f64 = (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            (a[0] - o[0]) * (b[1] - o[1]) - (b[0] - o[0]) * (a[1] - o[1])
        })
        .sum();
    let span = (x1 - x0).max(y1 - y0);
    if area2.abs() / 2.0 <= 1e-9 * span * span {
        return None;
    }
    pts.push(pts[0]);
    Some(pts)
}

pub fn to_multilinestring(g: &Geometry) -> MultiLineString<f64> {
    MultiLineString(
        g.part_ranges()
            .map(|r| LineString::new(coords(&g.xy[r])))
            .filter(|l| l.0.len() >= 2)
            .collect(),
    )
}

pub fn from_multilinestring(ml: &MultiLineString<f64>) -> Option<Geometry> {
    let mut g = empty(GeometryKind::Polyline);
    for l in ml.0.iter().filter(|l| l.0.len() >= 2) {
        g.parts.push(g.xy.len() as u32);
        g.xy.extend(l.0.iter().map(|c| [c.x, c.y]));
    }
    (!g.xy.is_empty()).then_some(g)
}

pub fn to_multipoint(g: &Geometry) -> MultiPoint<f64> {
    MultiPoint(g.xy.iter().map(|&[x, y]| Point::new(x, y)).collect())
}

fn empty(kind: GeometryKind) -> Geometry {
    Geometry {
        kind,
        parts: Vec::new(),
        part_types: None,
        xy: Vec::new(),
        z: None,
        m: None,
        curves: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holes_attach_to_the_smallest_containing_shell() {
        let rect = |x0: f64, y0: f64, x1: f64, y1: f64, cw: bool| {
            let mut r = vec![[x0, y0], [x0, y1], [x1, y1], [x1, y0], [x0, y0]];
            if !cw {
                r.reverse();
            }
            r
        };
        let rings = [
            rect(0.0, 0.0, 100.0, 100.0, true),
            rect(10.0, 10.0, 90.0, 90.0, false),
            rect(20.0, 20.0, 80.0, 80.0, true),
            rect(40.0, 40.0, 60.0, 60.0, false),
        ];
        let mut g = empty(GeometryKind::Polygon);
        for r in rings {
            g.parts.push(g.xy.len() as u32);
            g.xy.extend(r);
        }
        let mp = to_multipolygon(&g);
        assert_eq!(mp.0.len(), 2);
        assert!(mp.0.iter().all(|p| p.interiors().len() == 1));
        let back = from_multipolygon(&mp).unwrap();
        assert_eq!(back.parts.len(), 4);
    }
}
