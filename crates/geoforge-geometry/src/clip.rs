//! Clip (ArcGIS `analysis.Clip`): the parts of input features inside the clip
//! polygons. Clip polygons are dissolved into one area first.

use geo::{BooleanOps, BoundingRect, Intersects, MultiPolygon, Point, Rect, unary_union};
use geoforge_core::{Error, Geometry, GeometryKind, Result};

use crate::convert::{
    from_multilinestring, from_multipolygon, to_multilinestring, to_multipolygon,
};

/// The dissolved clip area.
pub struct ClipArea {
    area: MultiPolygon<f64>,
    bounds: Option<Rect<f64>>,
}

impl ClipArea {
    /// Dissolves the clip polygons (ArcGIS treats overlapping clip features as one).
    pub fn new<'a>(polygons: impl IntoIterator<Item = &'a Geometry>) -> Result<Self> {
        let mut parts = Vec::new();
        for g in polygons {
            if g.kind != GeometryKind::Polygon {
                return Err(Error::Unsupported("clip features must be polygons".into()));
            }
            let g = linear(g)?;
            parts.extend(to_multipolygon(&g).0);
        }
        let area = unary_union(parts.iter());
        let bounds = area.bounding_rect();
        Ok(Self { area, bounds })
    }
}

impl ClipArea {
    pub(crate) fn polygons(&self) -> &MultiPolygon<f64> {
        &self.area
    }

    pub(crate) fn touches(&self, env: &geoforge_core::Envelope) -> bool {
        self.bounds.is_some_and(|b| {
            !env.is_empty()
                && env.xmax >= b.min().x
                && env.xmin <= b.max().x
                && env.ymax >= b.min().y
                && env.ymin <= b.max().y
        })
    }

    pub(crate) fn contains_point(&self, p: [f64; 2]) -> bool {
        self.area.intersects(&Point::new(p[0], p[1]))
    }
}

/// Curves are densified before overlay (the result is linear, as in a shapefile).
pub(crate) fn linear(g: &Geometry) -> Result<Geometry> {
    if g.has_curves() {
        g.densify_curves_arcgis(geoforge_core::curves::arcgis_densify_deviation(false))
    } else {
        Ok(g.clone())
    }
}

/// Clips one geometry. `None` when nothing of it lies inside the clip area.
/// Z and M are dropped for lines and polygons (2D overlay); points keep them.
pub fn clip(g: &Geometry, area: &ClipArea) -> Result<Option<Geometry>> {
    let Some(bounds) = area.bounds else {
        return Ok(None);
    };
    let env = g.curve_envelope();
    if env.is_empty()
        || env.xmax < bounds.min().x
        || env.xmin > bounds.max().x
        || env.ymax < bounds.min().y
        || env.ymin > bounds.max().y
    {
        return Ok(None);
    }
    Ok(match g.kind {
        GeometryKind::Point | GeometryKind::Multipoint => {
            // Points on the clip boundary are kept.
            let keep: Vec<usize> = (0..g.xy.len())
                .filter(|&i| area.area.intersects(&Point::new(g.xy[i][0], g.xy[i][1])))
                .collect();
            if keep.is_empty() {
                None
            } else {
                let mut out = g.clone();
                out.xy = keep.iter().map(|&i| g.xy[i]).collect();
                out.z = g.z.as_ref().map(|z| keep.iter().map(|&i| z[i]).collect());
                out.m = g.m.as_ref().map(|m| keep.iter().map(|&i| m[i]).collect());
                Some(out)
            }
        }
        GeometryKind::Polyline => {
            let lines = to_multilinestring(&linear(g)?);
            from_multilinestring(&area.area.clip(&lines, false))
        }
        GeometryKind::Polygon => {
            let polys = to_multipolygon(&linear(g)?);
            from_multipolygon(&polys.intersection(&area.area))
        }
        GeometryKind::MultiPatch => {
            return Err(Error::Unsupported("clipping multipatches".into()));
        }
    })
}
