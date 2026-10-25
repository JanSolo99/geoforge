//! Overlay operations behind Intersect, Erase, Union and Dissolve.

use geo::{BooleanOps, Intersects, MultiPolygon, Point, unary_union};
use unigis_core::{Envelope, Error, Geometry, GeometryKind, Result};

use crate::clip::{ClipArea, linear};
use crate::convert::{
    from_multilinestring, from_multipolygon, to_multilinestring, to_multipolygon,
};

fn envelopes_touch(a: &Envelope, b: &Envelope) -> bool {
    !a.is_empty()
        && !b.is_empty()
        && a.xmin <= b.xmax
        && a.xmax >= b.xmin
        && a.ymin <= b.ymax
        && a.ymax >= b.ymin
}

fn dimension(k: GeometryKind) -> u8 {
    match k {
        GeometryKind::Point | GeometryKind::Multipoint => 0,
        GeometryKind::Polyline => 1,
        GeometryKind::Polygon | GeometryKind::MultiPatch => 2,
    }
}

/// The common part of two geometries, with the lower of their dimensions
/// (ArcGIS Intersect `output_type="INPUT"`). `None` when they do not intersect.
/// Supported: anything with a polygon, points with lines, and lines with lines
/// (their shared stretches).
pub fn intersection(a: &Geometry, b: &Geometry) -> Result<Option<Geometry>> {
    if !envelopes_touch(&a.curve_envelope(), &b.curve_envelope()) {
        return Ok(None);
    }
    // Put the lower-dimensional geometry first.
    let (lo, hi) = if dimension(a.kind) <= dimension(b.kind) {
        (a, b)
    } else {
        (b, a)
    };
    match hi.kind {
        GeometryKind::Polygon => {
            let area = ClipArea::new(std::iter::once(hi))?;
            crate::clip(lo, &area)
        }
        GeometryKind::Polyline if dimension(lo.kind) == 0 => {
            let lines = to_multilinestring(&linear(hi)?);
            let env = lo.envelope();
            let tol = 1e-9 * (env.xmax.abs().max(env.ymax.abs())).max(1.0);
            let keep: Vec<usize> = (0..lo.xy.len())
                .filter(|&i| {
                    let p = Point::new(lo.xy[i][0], lo.xy[i][1]);
                    lines.intersects(&p)
                        || geo::Distance::distance(&geo::Euclidean, &p, &lines) <= tol
                })
                .collect();
            Ok(select_points(lo, &keep))
        }
        GeometryKind::Polyline if lo.kind == GeometryKind::Polyline => {
            let m = crate::linework::meeting(a, b, crate::linework::XY_TOLERANCE)?;
            Ok(paths_geometry(m.shared))
        }
        _ => Err(Error::Unsupported(format!(
            "intersecting {:?} with {:?}",
            a.kind, b.kind
        ))),
    }
}

fn select_points(g: &Geometry, keep: &[usize]) -> Option<Geometry> {
    if keep.is_empty() {
        return None;
    }
    let mut out = g.clone();
    out.xy = keep.iter().map(|&i| g.xy[i]).collect();
    out.z = g.z.as_ref().map(|z| keep.iter().map(|&i| z[i]).collect());
    out.m = g.m.as_ref().map(|m| keep.iter().map(|&i| m[i]).collect());
    Some(out)
}

/// What remains of `g` outside `area` (ArcGIS Erase). `None` when nothing does.
pub fn erase(g: &Geometry, area: &ClipArea) -> Result<Option<Geometry>> {
    if let GeometryKind::Point | GeometryKind::Multipoint = g.kind {
        let keep: Vec<usize> = (0..g.xy.len())
            .filter(|&i| !area.contains_point(g.xy[i]))
            .collect();
        return Ok(select_points(g, &keep));
    }
    let Some(local) = area.around(&g.curve_envelope()) else {
        return Ok(Some(g.clone()));
    };
    Ok(match g.kind {
        GeometryKind::Polyline => {
            from_multilinestring(&local.clip(&to_multilinestring(&linear(g)?), true))
        }
        GeometryKind::Polygon => {
            from_multipolygon(&to_multipolygon(&linear(g)?).difference(&*local))
        }
        _ => return Err(Error::Unsupported("erasing multipatches".into())),
    })
}

/// A face of a planar overlay and, per input, the features covering it.
#[derive(Debug, Clone)]
pub struct OverlayCell {
    pub geometry: MultiPolygon<f64>,
    /// `members[i]`: OBJECTIDs of input `i` covering this cell (possibly several when
    /// features of one input overlap each other, possibly none).
    pub members: Vec<Vec<i64>>,
}

/// Planar overlay of polygon inputs (ArcGIS Union): splits the plane into cells
/// covered by the same set of features.
pub fn polygon_overlay(inputs: &[Vec<(i64, Geometry)>]) -> Result<Vec<OverlayCell>> {
    let mut cells: Vec<(OverlayCell, Option<geo::Rect<f64>>)> = Vec::new();
    for (input, features) in inputs.iter().enumerate() {
        for (oid, g) in features {
            if g.kind != GeometryKind::Polygon {
                return Err(Error::Unsupported("Union needs polygon inputs".into()));
            }
            let mut remaining = to_multipolygon(&linear(g)?);
            let rb = geo::BoundingRect::bounding_rect(&remaining);
            let mut next = Vec::with_capacity(cells.len() + 1);
            for (cell, cb) in cells.drain(..) {
                let overlaps = match (rb, cb) {
                    (Some(a), Some(b)) => {
                        a.min().x <= b.max().x
                            && a.max().x >= b.min().x
                            && a.min().y <= b.max().y
                            && a.max().y >= b.min().y
                    }
                    _ => false,
                };
                if !overlaps {
                    next.push((cell, cb));
                    continue;
                }
                let inside = cell.geometry.intersection(&remaining);
                if is_empty(&inside) {
                    next.push((cell, cb));
                    continue;
                }
                let outside = cell.geometry.difference(&remaining);
                remaining = remaining.difference(&cell.geometry);
                if !is_empty(&outside) {
                    let b = geo::BoundingRect::bounding_rect(&outside);
                    next.push((
                        OverlayCell {
                            geometry: outside,
                            members: cell.members.clone(),
                        },
                        b,
                    ));
                }
                let mut members = cell.members;
                members[input].push(*oid);
                let b = geo::BoundingRect::bounding_rect(&inside);
                next.push((
                    OverlayCell {
                        geometry: inside,
                        members,
                    },
                    b,
                ));
            }
            if !is_empty(&remaining) {
                let mut members = vec![Vec::new(); inputs.len()];
                members[input].push(*oid);
                let b = geo::BoundingRect::bounding_rect(&remaining);
                next.push((
                    OverlayCell {
                        geometry: remaining,
                        members,
                    },
                    b,
                ));
            }
            cells = next;
        }
    }
    Ok(cells.into_iter().map(|(c, _)| c).collect())
}

/// Ignores slivers below this fraction of a cell's own extent (overlay noise).
fn is_empty(mp: &MultiPolygon<f64>) -> bool {
    from_multipolygon(mp).is_none()
}

/// Dissolves geometries of one kind into one (ArcGIS Dissolve, `MULTI_PART`).
/// Lines that meet end to end are merged into single paths (`UNSPLIT_LINES`
/// off = `DISSOLVE_LINES`: paths are joined only where exactly two meet).
pub fn dissolve<'a>(geoms: impl IntoIterator<Item = &'a Geometry>) -> Result<Option<Geometry>> {
    let geoms: Vec<&Geometry> = geoms.into_iter().collect();
    let Some(first) = geoms.first() else {
        return Ok(None);
    };
    match first.kind {
        GeometryKind::Polygon => {
            let mut polys = Vec::new();
            for g in &geoms {
                polys.extend(to_multipolygon(&linear(g)?).0);
            }
            Ok(from_multipolygon(&unary_union(polys.iter())))
        }
        GeometryKind::Polyline => {
            let mut paths: Vec<Vec<[f64; 2]>> = Vec::new();
            for g in &geoms {
                let g = linear(g)?;
                paths.extend(
                    g.part_ranges()
                        .map(|r| g.xy[r].to_vec())
                        .filter(|p| p.len() >= 2),
                );
            }
            let merged = merge_paths(paths);
            let mut out = Geometry {
                kind: GeometryKind::Polyline,
                parts: Vec::new(),
                part_types: None,
                xy: Vec::new(),
                z: None,
                m: None,
                curves: Vec::new(),
            };
            for p in merged {
                out.parts.push(out.xy.len() as u32);
                out.xy.extend(p);
            }
            Ok((!out.xy.is_empty()).then_some(out))
        }
        GeometryKind::Point | GeometryKind::Multipoint => {
            let mut out = Geometry {
                kind: GeometryKind::Multipoint,
                parts: Vec::new(),
                part_types: None,
                xy: Vec::new(),
                z: None,
                m: None,
                curves: Vec::new(),
            };
            for g in &geoms {
                out.xy.extend(g.xy.iter().copied());
            }
            Ok((!out.xy.is_empty()).then_some(out))
        }
        GeometryKind::MultiPatch => Err(Error::Unsupported("dissolving multipatches".into())),
    }
}

/// Areas enclosed by the polygons but covered by none (holes of their union, minus
/// any islands inside them): Union's NO_GAPS rows. One per hole, in the order of
/// the union's polygons and rings.
pub fn enclosed_gaps<'a>(
    polygons: impl IntoIterator<Item = &'a Geometry>,
) -> Result<Vec<Geometry>> {
    let mut parts = Vec::new();
    for g in polygons {
        if g.kind == GeometryKind::Polygon {
            parts.extend(to_multipolygon(&linear(g)?).0);
        }
    }
    let union = unary_union(parts.iter());
    let mut out = Vec::new();
    for poly in &union.0 {
        for hole in poly.interiors() {
            let ring = MultiPolygon(vec![geo::Polygon::new(hole.clone(), Vec::new())]);
            if let Some(g) = from_multipolygon(&ring.difference(&union)) {
                out.push(g);
            }
        }
    }
    Ok(out)
}

/// Joins paths sharing an endpoint where exactly two path ends meet.
pub(crate) fn merge_paths(mut paths: Vec<Vec<[f64; 2]>>) -> Vec<Vec<[f64; 2]>> {
    loop {
        let mut joined = false;
        'outer: for i in 0..paths.len() {
            for end_i in [false, true] {
                let pi = if end_i {
                    *paths[i].last().unwrap()
                } else {
                    paths[i][0]
                };
                // Count path ends at this node.
                let ends: Vec<(usize, bool)> = paths
                    .iter()
                    .enumerate()
                    .flat_map(|(k, p)| [(k, false, p[0]), (k, true, *p.last().unwrap())])
                    .filter(|&(_, _, q)| q == pi)
                    .map(|(k, e, _)| (k, e))
                    .collect();
                if ends.len() != 2 {
                    continue;
                }
                let (j, end_j) = if ends[0] == (i, end_i) {
                    ends[1]
                } else {
                    ends[0]
                };
                if j == i {
                    continue; // a closed loop
                }
                let mut a = paths[i].clone();
                let mut b = paths[j].clone();
                if !end_i {
                    a.reverse();
                }
                if end_j {
                    b.reverse();
                }
                a.extend(b.into_iter().skip(1));
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                paths.remove(hi);
                paths[lo] = a;
                joined = true;
                break 'outer;
            }
        }
        if !joined {
            return paths;
        }
    }
}

/// Splits a geometry into its single parts (ArcGIS `SINGLE_PART`): one polygon
/// (shell + its holes), one path, or one point each.
pub fn explode(g: &Geometry) -> Vec<Geometry> {
    match g.kind {
        GeometryKind::Polygon => to_multipolygon(g)
            .0
            .into_iter()
            .filter_map(|p| from_multipolygon(&MultiPolygon(vec![p])))
            .collect(),
        GeometryKind::Polyline => to_multilinestring(g)
            .0
            .into_iter()
            .filter_map(|l| from_multilinestring(&geo::MultiLineString(vec![l])))
            .collect(),
        GeometryKind::Point | GeometryKind::Multipoint => {
            g.xy.iter().map(|p| Geometry::point(p[0], p[1])).collect()
        }
        GeometryKind::MultiPatch => vec![g.clone()],
    }
}

/// Converts a cell to an Esri polygon.
pub fn cell_geometry(cell: &OverlayCell) -> Option<Geometry> {
    from_multipolygon(&cell.geometry)
}

/// Paths as a polyline; `None` if there are none.
pub fn paths_geometry(paths: Vec<Vec<[f64; 2]>>) -> Option<Geometry> {
    if paths.is_empty() {
        return None;
    }
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polyline;
    g.xy.clear();
    g.parts.clear();
    for p in paths {
        g.parts.push(g.xy.len() as u32);
        g.xy.extend(p);
    }
    Some(g)
}

/// Cuts a polygon into strips of equal area with lines parallel to the X axis, from the bottom up
/// (ArcGIS Subdivide Polygon, equal parts / equal areas). Each strip is one geometry (several
/// parts when the polygon is not convex). With `parts` the strips all have area / parts; with
/// `target` they have that area, and the top one the remainder.
pub fn horizontal_strips(
    g: &Geometry,
    parts: Option<usize>,
    target: Option<f64>,
) -> Result<Vec<Geometry>> {
    use geo::{Area, BoundingRect, Coord, Rect};
    if g.kind != GeometryKind::Polygon {
        return Err(Error::Unsupported("subdividing non-polygons".into()));
    }
    let mp = to_multipolygon(&linear(g)?);
    let total = mp.unsigned_area();
    let Some(bb) = mp.bounding_rect() else {
        return Ok(Vec::new());
    };
    let strip_area = match (parts, target) {
        (Some(n), _) if n > 0 => total / n as f64,
        (_, Some(t)) if t > 0.0 => t,
        _ => {
            return Err(Error::Unsupported(
                "a number of parts or an area is required".into(),
            ));
        }
    };
    let n = match parts {
        Some(n) if n > 0 => n,
        _ => (total / strip_area - 1e-9).ceil().max(1.0) as usize,
    };
    let below = |y: f64| -> f64 {
        let r = Rect::new(
            Coord {
                x: bb.min().x - 1.0,
                y: bb.min().y - 1.0,
            },
            Coord {
                x: bb.max().x + 1.0,
                y,
            },
        )
        .to_polygon();
        mp.intersection(&MultiPolygon(vec![r])).unsigned_area()
    };
    let mut levels = vec![bb.min().y - 1.0];
    for k in 1..n {
        let want = strip_area * k as f64;
        let (mut lo, mut hi) = (bb.min().y, bb.max().y);
        for _ in 0..80 {
            let mid = (lo + hi) / 2.0;
            if below(mid) < want {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        levels.push((lo + hi) / 2.0);
    }
    levels.push(bb.max().y + 1.0);
    let mut out = Vec::new();
    for w in levels.windows(2) {
        let r = Rect::new(
            Coord {
                x: bb.min().x - 1.0,
                y: w[0],
            },
            Coord {
                x: bb.max().x + 1.0,
                y: w[1],
            },
        )
        .to_polygon();
        if let Some(piece) = from_multipolygon(&mp.intersection(&MultiPolygon(vec![r]))) {
            out.push(piece);
        }
    }
    Ok(out)
}
