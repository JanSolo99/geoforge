//! Clip (ArcGIS `analysis.Clip`): the parts of input features inside the clip
//! polygons. Overlapping clip polygons act as one area; each feature is clipped
//! by the union of just the clip polygons whose envelopes reach it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use geo::{BooleanOps, BoundingRect, Intersects, MultiPolygon, Point, Rect, unary_union};
use geoforge_core::{Envelope, Error, Geometry, GeometryKind, Result};

use crate::convert::{
    from_multilinestring, from_multipolygon, to_multilinestring, to_multipolygon,
};
use crate::index::EnvelopeIndex;

/// The clip (or erase) polygons, indexed by envelope.
pub struct ClipArea {
    polygons: Vec<MultiPolygon<f64>>,
    index: EnvelopeIndex,
    bounds: Option<Rect<f64>>,
    /// Unions already built, by the clip polygons they combine (neighbouring
    /// features mostly reach the same ones).
    unions: RefCell<HashMap<Vec<usize>, Rc<MultiPolygon<f64>>>>,
}

impl ClipArea {
    pub fn new<'a>(polygons: impl IntoIterator<Item = &'a Geometry>) -> Result<Self> {
        let mut geoms = Vec::new();
        for g in polygons {
            if g.kind != GeometryKind::Polygon {
                return Err(Error::Unsupported("clip features must be polygons".into()));
            }
            geoms.push(linear(g)?);
        }
        let index = EnvelopeIndex::new(geoms.iter().map(Some));
        let polygons: Vec<MultiPolygon<f64>> = geoms.iter().map(to_multipolygon).collect();
        let bounds = polygons
            .iter()
            .filter_map(|p| p.bounding_rect())
            .reduce(|a, b| {
                Rect::new(
                    (a.min().x.min(b.min().x), a.min().y.min(b.min().y)),
                    (a.max().x.max(b.max().x), a.max().y.max(b.max().y)),
                )
            });
        Ok(Self {
            polygons,
            index,
            bounds,
            unions: RefCell::default(),
        })
    }

    pub(crate) fn touches(&self, env: &Envelope) -> bool {
        self.bounds.is_some_and(|b| {
            !env.is_empty()
                && env.xmax >= b.min().x
                && env.xmin <= b.max().x
                && env.ymax >= b.min().y
                && env.ymin <= b.max().y
        })
    }

    /// The clip area around `env`: the union of the clip polygons whose
    /// envelopes reach it. `None` if there are none.
    pub(crate) fn around(&self, env: &Envelope) -> Option<Local<'_>> {
        if !self.touches(env) {
            return None;
        }
        let near = self.index.candidates(env, 0.0);
        match near.as_slice() {
            [] => None,
            [i] if self.polygons[*i].0.len() == 1 => Some(Local::Borrowed(&self.polygons[*i])),
            _ => {
                let mut unions = self.unions.borrow_mut();
                let union = unions.entry(near).or_insert_with_key(|near| {
                    Rc::new(unary_union(
                        near.iter().flat_map(|&i| self.polygons[i].0.iter()),
                    ))
                });
                Some(Local::Shared(Rc::clone(union)))
            }
        }
    }

    pub(crate) fn contains_point(&self, p: [f64; 2]) -> bool {
        let env = Envelope {
            xmin: p[0],
            ymin: p[1],
            xmax: p[0],
            ymax: p[1],
            ..Envelope::EMPTY
        };
        let pt = Point::new(p[0], p[1]);
        self.touches(&env)
            && self
                .index
                .candidates(&env, 0.0)
                .into_iter()
                .any(|i| self.polygons[i].intersects(&pt))
    }
}

/// The clip area around a feature: one clip polygon, or a (cached) union.
pub(crate) enum Local<'a> {
    Borrowed(&'a MultiPolygon<f64>),
    Shared(Rc<MultiPolygon<f64>>),
}

impl std::ops::Deref for Local<'_> {
    type Target = MultiPolygon<f64>;

    fn deref(&self) -> &MultiPolygon<f64> {
        match self {
            Local::Borrowed(p) => p,
            Local::Shared(p) => p,
        }
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
    let env = g.curve_envelope();
    Ok(match g.kind {
        GeometryKind::Point | GeometryKind::Multipoint => {
            // Points on the clip boundary are kept.
            let keep: Vec<usize> = (0..g.xy.len())
                .filter(|&i| area.contains_point(g.xy[i]))
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
            let Some(local) = area.around(&env) else {
                return Ok(None);
            };
            let lines = to_multilinestring(&linear(g)?);
            from_multilinestring(&local.clip(&lines, false))
        }
        GeometryKind::Polygon => {
            let Some(local) = area.around(&env) else {
                return Ok(None);
            };
            let polys = to_multipolygon(&linear(g)?);
            from_multipolygon(&polys.intersection(&*local))
        }
        GeometryKind::MultiPatch => {
            return Err(Error::Unsupported("clipping multipatches".into()));
        }
    })
}
