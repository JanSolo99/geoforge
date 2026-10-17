//! An in-memory R-tree over feature envelopes, so tools compare only features
//! whose envelopes can interact instead of every pair.

use geoforge_core::{Envelope, Geometry, Result};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};

use crate::proximity::{Nearest, nearest};

type Entry = GeomWithData<Rectangle<[f64; 2]>, usize>;

/// Envelopes of a set of geometries, indexed by their position in that set.
pub struct EnvelopeIndex {
    tree: RTree<Entry>,
}

fn rect(e: &Envelope) -> Rectangle<[f64; 2]> {
    Rectangle::from_corners([e.xmin, e.ymin], [e.xmax, e.ymax])
}

impl EnvelopeIndex {
    /// Indexes `geometries`; `None` entries (null shapes) are left out.
    pub fn new<'a>(geometries: impl IntoIterator<Item = Option<&'a Geometry>>) -> Self {
        let entries = geometries
            .into_iter()
            .enumerate()
            .filter_map(|(i, g)| {
                let e = g?.curve_envelope();
                (!e.is_empty()).then(|| GeomWithData::new(rect(&e), i))
            })
            .collect();
        Self {
            tree: RTree::bulk_load(entries),
        }
    }

    /// Positions of the geometries whose envelopes come within `pad` of `env`,
    /// in ascending order (so callers keep FID order).
    pub fn candidates(&self, env: &Envelope, pad: f64) -> Vec<usize> {
        if env.is_empty() {
            return Vec::new();
        }
        let query = AABB::from_corners(
            [env.xmin - pad, env.ymin - pad],
            [env.xmax + pad, env.ymax + pad],
        );
        let mut out: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(&query)
            .map(|e| e.data)
            .collect();
        out.sort_unstable();
        out
    }

    /// The indexed geometry (looked up by position with `geometry`) closest to `g`, within
    /// `radius` if given; the lowest position wins ties. Envelopes are visited
    /// nearest first and the search stops once none can be closer.
    pub fn closest<'a>(
        &self,
        g: &Geometry,
        geometry: impl Fn(usize) -> Option<&'a Geometry>,
        radius: Option<f64>,
    ) -> Result<Option<(usize, Nearest)>> {
        let env = g.curve_envelope();
        if env.is_empty() {
            return Ok(None);
        }
        let center = [(env.xmin + env.xmax) / 2.0, (env.ymin + env.ymax) / 2.0];
        let half_diagonal = (env.xmax - env.xmin).hypot(env.ymax - env.ymin) / 2.0;
        let mut best: Option<(usize, Nearest)> = None;
        for (entry, d2) in self.tree.nearest_neighbor_iter_with_distance_2(&center) {
            // Lower bound on the distance from `g` to anything in this envelope.
            let bound = d2.sqrt() - half_diagonal;
            if best.is_some_and(|(_, b)| bound > b.distance) || radius.is_some_and(|r| bound > r) {
                break;
            }
            let Some(other) = geometry(entry.data) else {
                continue;
            };
            let Some(n) = nearest(g, other)? else {
                continue;
            };
            if radius.is_some_and(|r| n.distance > r) {
                continue;
            }
            let better = match best {
                None => true,
                Some((i, b)) => {
                    n.distance < b.distance || (n.distance == b.distance && entry.data < i)
                }
            };
            if better {
                best = Some((entry.data, n));
            }
        }
        Ok(best)
    }
}

impl EnvelopeIndex {
    /// Like [`closest`](Self::closest), measuring on the WGS 84 ellipsoid (geometries in
    /// longitude/latitude; distances in metres).
    pub fn closest_geodesic<'a>(
        &self,
        g: &Geometry,
        geometry: impl Fn(usize) -> Option<&'a Geometry>,
        radius: Option<f64>,
    ) -> Result<Option<(usize, Nearest)>> {
        /// The shortest metres in a degree of latitude.
        const METRES_PER_DEGREE: f64 = 110_574.0;
        let env = g.curve_envelope();
        if env.is_empty() {
            return Ok(None);
        }
        let mut best: Option<(usize, Nearest)> = None;
        for entry in self.tree.iter() {
            let (lo, hi) = (entry.geom().lower(), entry.geom().upper());
            let gap = (env.ymin - hi[1]).max(lo[1] - env.ymax).max(0.0) * METRES_PER_DEGREE;
            if best.is_some_and(|(_, b)| gap > b.distance) || radius.is_some_and(|r| gap > r) {
                continue;
            }
            let Some(other) = geometry(entry.data) else {
                continue;
            };
            let Some(n) = crate::geodesic::nearest_geodesic(g, other)? else {
                continue;
            };
            if radius.is_some_and(|r| n.distance > r) {
                continue;
            }
            if best.is_none_or(|(i, b)| {
                n.distance < b.distance || (n.distance == b.distance && entry.data < i)
            }) {
                best = Some((entry.data, n));
            }
        }
        Ok(best)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_and_closest() {
        let pts: Vec<Geometry> = (0..100)
            .map(|i| Geometry::point(f64::from(i % 10) * 10.0, f64::from(i / 10) * 10.0))
            .collect();
        let refs: Vec<Option<&Geometry>> = pts.iter().map(Some).collect();
        let index = EnvelopeIndex::new(refs.iter().copied());
        let q = Geometry::point(21.0, 29.0);
        assert_eq!(index.candidates(&q.envelope(), 5.0), vec![32]);
        let (i, n) = index.closest(&q, |i| refs[i], None).unwrap().unwrap();
        assert_eq!(i, 32);
        assert!((n.distance - 2f64.sqrt()).abs() < 1e-12);
        // Equidistant from 0 and 1: the lower position wins.
        let (i, _) = index
            .closest(&Geometry::point(5.0, 0.0), |i| refs[i], None)
            .unwrap()
            .unwrap();
        assert_eq!(i, 0);
        assert!(index.closest(&q, |i| refs[i], Some(1.0)).unwrap().is_none());
    }
}
