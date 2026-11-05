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

/// The map's geometry model from a UniGIS shape (curves densified); a MultiPatch as its footprint
/// (as ArcGIS draws one on a 2D map); `None` for a shape with no vertices.
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
        GeometryKind::MultiPatch => multipatch_footprint(g),
    }
}

/// Most faces merged into one footprint; a larger patch is drawn by its faces' outline box.
const MAX_FOOTPRINT_FACES: usize = 20_000;

/// A MultiPatch's footprint: the union of its faces (triangle strips, fans, triangles and rings,
/// holes inside their outer ring) seen from above. Vertical faces have no area and drop out.
fn multipatch_footprint(g: &GfGeometry) -> Option<Geometry> {
    use unigis_core::PartType as T;
    let pt = |i: usize| g.xy.get(i).map(|&[x, y]| geo::Coord { x, y });
    let mut faces: Vec<geo::Polygon<f64>> = Vec::new();
    let ring_of = |faces: &mut Vec<geo::Polygon<f64>>, pts: Vec<geo::Coord<f64>>, holes: Vec<geo::LineString<f64>>| {
        // Faces wind either way (a strip alternates): the union needs them all the same way.
        let p = geo::Orient::orient(&geo::Polygon::new(geo::LineString::new(pts), holes), geo::orient::Direction::Default);
        if geo::Area::unsigned_area(&p) > 1e-12 {
            faces.push(p);
        }
    };
    let types = g.part_types.clone().unwrap_or_default();
    let ranges: Vec<std::ops::Range<usize>> = g.part_ranges().collect();
    let mut i = 0;
    while i < ranges.len() {
        let r = ranges[i].clone();
        let kind = types.get(i).copied().unwrap_or(T::Ring);
        let tri = |a: usize, b: usize, c: usize| -> Option<Vec<geo::Coord<f64>>> { Some(vec![pt(a)?, pt(b)?, pt(c)?]) };
        match kind {
            T::TriangleStrip => {
                for k in r.start..r.end.saturating_sub(2) {
                    if let Some(t) = tri(k, k + 1, k + 2) {
                        ring_of(&mut faces, t, Vec::new());
                    }
                }
            }
            T::TriangleFan => {
                for k in r.start + 1..r.end.saturating_sub(1) {
                    if let Some(t) = tri(r.start, k, k + 1) {
                        ring_of(&mut faces, t, Vec::new());
                    }
                }
            }
            T::Triangles => {
                for k in (r.start..r.end.saturating_sub(2)).step_by(3) {
                    if let Some(t) = tri(k, k + 1, k + 2) {
                        ring_of(&mut faces, t, Vec::new());
                    }
                }
            }
            T::OuterRing | T::FirstRing | T::Ring | T::InnerRing => {
                let outer: Vec<geo::Coord<f64>> = r.clone().filter_map(pt).collect();
                // Inner rings (and the `Ring`s after a `FirstRing`) are holes of this one.
                let mut holes = Vec::new();
                let hole_kind = if kind == T::FirstRing { T::Ring } else { T::InnerRing };
                while matches!(kind, T::OuterRing | T::FirstRing) && types.get(i + 1) == Some(&hole_kind) {
                    i += 1;
                    holes.push(geo::LineString::new(ranges[i].clone().filter_map(pt).collect()));
                }
                if outer.len() >= 3 {
                    ring_of(&mut faces, outer, holes);
                }
            }
        }
        if faces.len() > MAX_FOOTPRINT_FACES {
            break;
        }
        i += 1;
    }
    if faces.is_empty() {
        return None;
    }
    let merged: geo::MultiPolygon<f64> = if faces.len() > MAX_FOOTPRINT_FACES {
        let b = geo::BoundingRect::bounding_rect(&geo::MultiPolygon::new(faces))?;
        geo::MultiPolygon::new(vec![b.to_polygon()])
    } else {
        geo::unary_union(faces.iter())
    };
    let ring = |l: &geo::LineString<f64>| -> Vec<Coord> { l.coords().map(|c| Coord::new(c.x, c.y)).collect() };
    let polys: Vec<Vec<Vec<Coord>>> = merged
        .0
        .iter()
        .map(|p| std::iter::once(ring(p.exterior())).chain(p.interiors().iter().map(ring)).collect())
        .filter(|rings: &Vec<Vec<Coord>>| rings.first().is_some_and(|r| r.len() >= 4))
        .collect();
    (!polys.is_empty()).then_some(Geometry::Polygon(polys))
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

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::PartType as T;

    fn patch(parts: &[(T, &[[f64; 2]])]) -> GfGeometry {
        let mut g = GfGeometry::point(0.0, 0.0);
        g.kind = GeometryKind::MultiPatch;
        g.xy.clear();
        let mut types = Vec::new();
        for (t, pts) in parts {
            g.parts.push(g.xy.len() as u32);
            g.xy.extend_from_slice(pts);
            types.push(*t);
        }
        g.part_types = Some(types);
        g
    }

    fn area(g: &Geometry) -> f64 {
        let Geometry::Polygon(polys) = g else { panic!("{g:?}") };
        polys
            .iter()
            .map(|rings| rings.iter().enumerate().map(|(i, r)| signed_area(r).abs() / 2.0 * if i == 0 { 1.0 } else { -1.0 }).sum::<f64>())
            .sum()
    }

    #[test]
    fn multipatch_footprints() {
        let sq: &[[f64; 2]] = &[[0.0, 0.0], [0.0, 2.0], [2.0, 2.0], [2.0, 0.0], [0.0, 0.0]];
        let wall: &[[f64; 2]] = &[[0.0, 0.0], [0.0, 0.0], [2.0, 0.0], [2.0, 0.0], [0.0, 0.0]];
        // A cube of rings: the top and bottom overlap, the walls have no area from above.
        let cube = patch(&[(T::OuterRing, sq), (T::OuterRing, sq), (T::OuterRing, wall), (T::OuterRing, wall)]);
        let f = from_unigis(&cube).unwrap();
        assert!((area(&f) - 4.0).abs() < 1e-9, "{f:?}");
        let Geometry::Polygon(p) = &f else { panic!("{f:?}") };
        assert_eq!(p.len(), 1, "one footprint, not two cancelling squares");
        // A strip and a fan covering the same square, and separate triangles.
        let strip = patch(&[(T::TriangleStrip, &[[0.0, 0.0], [0.0, 2.0], [2.0, 0.0], [2.0, 2.0]])]);
        let s = from_unigis(&strip).unwrap();
        assert!((area(&s) - 4.0).abs() < 1e-9, "{} {s:?}", area(&s));
        let fan = patch(&[(T::TriangleFan, &[[1.0, 1.0], [0.0, 0.0], [0.0, 2.0], [2.0, 2.0], [2.0, 0.0], [0.0, 0.0]])]);
        assert!((area(&from_unigis(&fan).unwrap()) - 4.0).abs() < 1e-9);
        let tris = patch(&[(T::Triangles, &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [5.0, 5.0], [6.0, 5.0], [5.0, 6.0]])]);
        let Geometry::Polygon(p) = from_unigis(&tris).unwrap() else { panic!() };
        assert_eq!(p.len(), 2);
        // An outer ring with a hole.
        let hole: &[[f64; 2]] = &[[0.5, 0.5], [1.5, 0.5], [1.5, 1.5], [0.5, 1.5], [0.5, 0.5]];
        let holed = patch(&[(T::OuterRing, sq), (T::InnerRing, hole)]);
        assert!((area(&from_unigis(&holed).unwrap()) - 3.0).abs() < 1e-9);
        // Only vertical faces: nothing to draw.
        assert!(from_unigis(&patch(&[(T::OuterRing, wall)])).is_none());
        assert!(from_unigis(&patch(&[])).is_none());
    }
}
