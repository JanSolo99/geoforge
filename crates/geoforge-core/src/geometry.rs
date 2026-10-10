//! Esri-style geometry: flat coordinate arrays with part start offsets.
//!
//! Z and M are per-geometry (all vertices have them or none do). A vertex with no
//! measure stores `f64::NAN` in `m`.

use std::ops::Range;

use crate::Envelope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryKind {
    Point,
    Multipoint,
    Polyline,
    Polygon,
    MultiPatch,
}

/// MultiPatch part types. Discriminants match the Shapefile / FileGDB encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PartType {
    TriangleStrip = 0,
    TriangleFan = 1,
    OuterRing = 2,
    InnerRing = 3,
    FirstRing = 4,
    Ring = 5,
    /// FileGDB only; not part of the Shapefile specification.
    Triangles = 6,
}

impl PartType {
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            0 => Self::TriangleStrip,
            1 => Self::TriangleFan,
            2 => Self::OuterRing,
            3 => Self::InnerRing,
            4 => Self::FirstRing,
            5 => Self::Ring,
            6 => Self::Triangles,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    pub kind: GeometryKind,
    /// Start index into `xy` of each part. Empty for Point and Multipoint.
    pub parts: Vec<u32>,
    /// One entry per part, MultiPatch only.
    pub part_types: Option<Vec<PartType>>,
    pub xy: Vec<[f64; 2]>,
    pub z: Option<Vec<f64>>,
    pub m: Option<Vec<f64>>,
    /// Non-linear segments of a Polyline or Polygon, in vertex order. Any segment not
    /// listed is a straight line between consecutive vertices.
    pub curves: Vec<Segment>,
}

/// A non-linear segment starting at vertex `start` and ending at vertex `start + 1`.
/// Stored as Esri defines them (extended shape buffer format).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: u32,
    pub kind: SegmentKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SegmentKind {
    /// `point` is the arc's center, or an interior point when `bits & ARC_INTERIOR_POINT`.
    CircularArc { point: [f64; 2], bits: u32 },
    /// Cubic Bézier with two control points.
    Bezier { controls: [[f64; 2]; 2] },
    EllipticArc {
        /// Center, or start/central angles depending on `bits`.
        center: [f64; 2],
        rotation: f64,
        semi_major: f64,
        minor_major_ratio: f64,
        bits: u32,
    },
}

/// Circular-arc flag bits.
pub const ARC_EMPTY: u32 = 0x1;
pub const ARC_CCW: u32 = 0x8;
pub const ARC_MINOR: u32 = 0x10;
pub const ARC_LINE: u32 = 0x20;
pub const ARC_POINT: u32 = 0x40;
pub const ARC_INTERIOR_POINT: u32 = 0x80;

impl Geometry {
    pub fn point(x: f64, y: f64) -> Self {
        Self {
            kind: GeometryKind::Point,
            parts: Vec::new(),
            part_types: None,
            xy: vec![[x, y]],
            z: None,
            m: None,
            curves: Vec::new(),
        }
    }

    pub fn has_curves(&self) -> bool {
        !self.curves.is_empty()
    }

    pub fn has_z(&self) -> bool {
        self.z.is_some()
    }

    pub fn has_m(&self) -> bool {
        self.m.is_some()
    }

    pub fn num_points(&self) -> usize {
        self.xy.len()
    }

    pub fn num_parts(&self) -> usize {
        match self.kind {
            GeometryKind::Point | GeometryKind::Multipoint => usize::from(!self.xy.is_empty()),
            _ => self.parts.len(),
        }
    }

    /// Vertex index ranges of each part. Point and Multipoint yield one range.
    pub fn part_ranges(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        let starts: &[u32] = match self.kind {
            GeometryKind::Point | GeometryKind::Multipoint if !self.xy.is_empty() => &[0],
            _ => &self.parts,
        };
        let n = self.xy.len();
        starts.iter().enumerate().map(move |(i, &s)| {
            let end = starts.get(i + 1).map_or(n, |&e| e as usize);
            s as usize..end
        })
    }

    pub fn envelope(&self) -> Envelope {
        let mut env = Envelope::EMPTY;
        for &[x, y] in &self.xy {
            env.expand_xy(x, y);
        }
        if let Some(z) = &self.z {
            z.iter().for_each(|&v| env.expand_z(v));
        }
        if let Some(m) = &self.m {
            m.iter().for_each(|&v| env.expand_m(v));
        }
        env
    }
}

impl Geometry {
    /// Orients polygon rings the Esri way: exterior rings clockwise, holes
    /// counter-clockwise. A ring is a hole when an odd number of other rings contain it.
    pub fn orient_rings_esri(&mut self) {
        if self.kind != GeometryKind::Polygon {
            return;
        }
        let ranges: Vec<_> = self.part_ranges().collect();
        let flips: Vec<bool> = ranges
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let ring = &self.xy[r.clone()];
                let Some(&probe) = ring.first() else {
                    return false;
                };
                let depth = ranges
                    .iter()
                    .enumerate()
                    .filter(|&(j, other)| j != i && point_in_ring(probe, &self.xy[other.clone()]))
                    .count();
                let clockwise = ring_signed_area2(ring) < 0.0;
                let is_hole = depth % 2 == 1;
                clockwise == is_hole
            })
            .collect();
        for (r, flip) in ranges.into_iter().zip(flips) {
            if flip {
                self.xy[r.clone()].reverse();
                if let Some(z) = &mut self.z {
                    z[r.clone()].reverse();
                }
                if let Some(m) = &mut self.m {
                    m[r].reverse();
                }
            }
        }
    }
}

/// Even-odd point-in-ring test. Points on the boundary may go either way.
pub fn point_in_ring(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let [xi, yi] = ring[i];
        let [xj, yj] = ring[(i + n - 1) % n];
        if (yi > p[1]) != (yj > p[1]) && p[0] < (xj - xi) * (p[1] - yi) / (yj - yi) + xi {
            inside = !inside;
        }
    }
    inside
}

/// Twice the signed area of a ring (shoelace). Negative means clockwise.
pub fn ring_signed_area2(ring: &[[f64; 2]]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for w in ring.windows(2) {
        sum += w[0][0] * w[1][1] - w[1][0] * w[0][1];
    }
    let (first, last) = (ring[0], ring[ring.len() - 1]);
    sum + (last[0] * first[1] - first[0] * last[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_ranges_split_on_offsets() {
        let g = Geometry {
            kind: GeometryKind::Polyline,
            parts: vec![0, 2],
            part_types: None,
            xy: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0], [3.0, 3.0], [4.0, 4.0]],
            z: None,
            m: None,
            curves: Vec::new(),
        };
        assert_eq!(g.part_ranges().collect::<Vec<_>>(), vec![0..2, 2..5]);
    }

    #[test]
    fn clockwise_ring_has_negative_area() {
        let cw = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        assert!(ring_signed_area2(&cw) < 0.0);
    }

    #[test]
    fn orient_rings_makes_shells_clockwise_and_holes_counter_clockwise() {
        // OGC orientation (CCW shell, CW hole), as GeoJSON writes it.
        let mut g = Geometry {
            kind: GeometryKind::Polygon,
            parts: vec![0, 5],
            part_types: None,
            xy: vec![
                [0.0, 0.0],
                [10.0, 0.0],
                [10.0, 10.0],
                [0.0, 10.0],
                [0.0, 0.0],
                [2.0, 2.0],
                [2.0, 4.0],
                [4.0, 4.0],
                [4.0, 2.0],
                [2.0, 2.0],
            ],
            z: None,
            m: Some((0..10).map(f64::from).collect()),
            curves: Vec::new(),
        };
        g.orient_rings_esri();
        let rings: Vec<_> = g.part_ranges().collect();
        assert!(ring_signed_area2(&g.xy[rings[0].clone()]) < 0.0);
        assert!(ring_signed_area2(&g.xy[rings[1].clone()]) > 0.0);
        assert_eq!(g.m.as_ref().unwrap()[..5], [4.0, 3.0, 2.0, 1.0, 0.0]);
    }

    #[test]
    fn envelope_ignores_nan_measures() {
        let mut g = Geometry::point(1.0, 2.0);
        g.m = Some(vec![f64::NAN]);
        assert_eq!(g.envelope().m, None);
    }
}
