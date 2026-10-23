//! Conversion between the map's geometry model and UniGIS's shapes (`unigis-core`); shared by
//! the File Geodatabase source and the UniGIS tools, and free of file access so it builds for
//! the browser too.
//!
//! Modified for UniGIS (not part of upstream RustGIS).

use unigis_core::{Geometry as GfGeometry, GeometryKind};

use crate::Geometry;
use crate::io::group_rings;
use rustgis_geom::Coord;

/// Largest angle between densified points of an arc, in radians.
const ARC_STEP: f64 = 0.05;

/// The map's geometry model from a UniGIS shape (curves densified); `None` for MultiPatch and for a shape with no
/// vertices.
pub fn from_unigis(g: &GfGeometry) -> Option<Geometry> {
    let dense;
    let g = if g.has_curves() {
        dense = g.densify_curves(ARC_STEP).ok()?;
        &dense
    } else {
        g
    };
    let coords = |range: std::ops::Range<usize>| -> Vec<Coord> { g.xy[range].iter().map(|&[x, y]| Coord::new(x, y)).collect() };
    let starts = |n: usize| -> Vec<std::ops::Range<usize>> {
        let mut at: Vec<usize> = g.parts.iter().map(|&p| p as usize).collect();
        at.push(n);
        at.windows(2).filter(|w| w[0] <= w[1] && w[1] <= n).map(|w| w[0]..w[1]).collect()
    };
    let n = g.xy.len();
    match g.kind {
        GeometryKind::Point => (n > 0).then(|| Geometry::Point(coords(0..1)[0])),
        GeometryKind::Multipoint => (n > 0).then(|| Geometry::MultiPoint(coords(0..n))),
        GeometryKind::Polyline => {
            let parts: Vec<Vec<Coord>> = starts(n).into_iter().map(coords).filter(|p| p.len() >= 2).collect();
            (!parts.is_empty()).then_some(Geometry::Line(parts))
        }
        GeometryKind::Polygon => {
            // Esri's outer rings run clockwise, holes counter-clockwise.
            let rings: Vec<(Vec<Coord>, bool)> = starts(n)
                .into_iter()
                .map(coords)
                .filter(|r| r.len() >= 3)
                .map(|r| {
                    let outer = signed_area(&r) <= 0.0;
                    (r, outer)
                })
                .collect();
            (!rings.is_empty()).then(|| group_rings(rings))
        }
        GeometryKind::MultiPatch => None,
    }
}

/// A ring with Esri's orientation (outer clockwise, hole counter-clockwise), closed.
fn oriented(ring: &[Coord], outer: bool) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = ring.iter().map(|c| [c.x, c.y]).collect();
    if pts.first() != pts.last()
        && let Some(&first) = pts.first()
    {
        pts.push(first);
    }
    // Positive area is counter-clockwise.
    if (signed_area(ring) > 0.0) == outer {
        pts.reverse();
    }
    pts
}

/// UniGIS's shape from the map's geometry; `None` when there is nothing to draw.
pub fn to_unigis(g: &Geometry) -> Option<GfGeometry> {
    let mut out = GfGeometry::point(0.0, 0.0);
    out.xy.clear();
    match g {
        Geometry::Point(c) => return Some(GfGeometry::point(c.x, c.y)),
        Geometry::MultiPoint(v) => {
            out.kind = GeometryKind::Multipoint;
            out.xy = v.iter().map(|c| [c.x, c.y]).collect();
        }
        Geometry::Line(parts) => {
            out.kind = GeometryKind::Polyline;
            for p in parts.iter().filter(|p| p.len() >= 2) {
                out.parts.push(out.xy.len() as u32);
                out.xy.extend(p.iter().map(|c| [c.x, c.y]));
            }
        }
        Geometry::Polygon(polys) => {
            out.kind = GeometryKind::Polygon;
            for poly in polys {
                for (i, ring) in poly.iter().enumerate().filter(|(_, r)| r.len() >= 3) {
                    out.parts.push(out.xy.len() as u32);
                    out.xy.extend(oriented(ring, i == 0));
                }
            }
        }
    }
    (!out.xy.is_empty()).then_some(out)
}

/// Twice the signed area of a ring (positive counter-clockwise).
fn signed_area(r: &[Coord]) -> f64 {
    let mut s = 0.0;
    for (i, a) in r.iter().enumerate() {
        let b = r[(i + 1) % r.len()];
        s += a.x * b.y - b.x * a.y;
    }
    s
}
