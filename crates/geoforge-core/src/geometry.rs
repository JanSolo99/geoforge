//! Geometry model. Z and M are optional per-vertex ordinates, matching Esri semantics.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
    pub m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    Point(Coord),
    MultiPoint(Vec<Coord>),
    /// One or more paths.
    Polyline(Vec<Vec<Coord>>),
    /// One or more rings; exterior rings are clockwise (Esri convention).
    Polygon(Vec<Vec<Coord>>),
}
