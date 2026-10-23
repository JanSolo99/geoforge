use unigis_core::GeometryKind;

/// Shapefile shape types, with their on-disk codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ShapeType {
    Null = 0,
    Point = 1,
    PolyLine = 3,
    Polygon = 5,
    MultiPoint = 8,
    PointZ = 11,
    PolyLineZ = 13,
    PolygonZ = 15,
    MultiPointZ = 18,
    PointM = 21,
    PolyLineM = 23,
    PolygonM = 25,
    MultiPointM = 28,
    MultiPatch = 31,
}

impl ShapeType {
    pub fn from_code(code: i32) -> Option<Self> {
        use ShapeType::*;
        Some(match code {
            0 => Null,
            1 => Point,
            3 => PolyLine,
            5 => Polygon,
            8 => MultiPoint,
            11 => PointZ,
            13 => PolyLineZ,
            15 => PolygonZ,
            18 => MultiPointZ,
            21 => PointM,
            23 => PolyLineM,
            25 => PolygonM,
            28 => MultiPointM,
            31 => MultiPatch,
            _ => return None,
        })
    }

    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn kind(self) -> Option<GeometryKind> {
        use ShapeType::*;
        Some(match self {
            Null => return None,
            Point | PointZ | PointM => GeometryKind::Point,
            MultiPoint | MultiPointZ | MultiPointM => GeometryKind::Multipoint,
            PolyLine | PolyLineZ | PolyLineM => GeometryKind::Polyline,
            Polygon | PolygonZ | PolygonM => GeometryKind::Polygon,
            MultiPatch => GeometryKind::MultiPatch,
        })
    }

    /// Z types and MultiPatch store Z.
    pub fn has_z(self) -> bool {
        use ShapeType::*;
        matches!(
            self,
            PointZ | PolyLineZ | PolygonZ | MultiPointZ | MultiPatch
        )
    }

    /// Z types also carry M (optionally present on disk), as ArcGIS reports.
    pub fn has_m(self) -> bool {
        use ShapeType::*;
        matches!(
            self,
            PointZ
                | PolyLineZ
                | PolygonZ
                | MultiPointZ
                | MultiPatch
                | PointM
                | PolyLineM
                | PolygonM
                | MultiPointM
        )
    }

    /// The shape type that stores `kind` with the requested dimensions.
    pub fn for_geometry(kind: GeometryKind, has_z: bool, has_m: bool) -> Self {
        use ShapeType::*;
        match (kind, has_z, has_m) {
            (GeometryKind::MultiPatch, _, _) => MultiPatch,
            (GeometryKind::Point, true, _) => PointZ,
            (GeometryKind::Point, false, true) => PointM,
            (GeometryKind::Point, false, false) => Point,
            (GeometryKind::Multipoint, true, _) => MultiPointZ,
            (GeometryKind::Multipoint, false, true) => MultiPointM,
            (GeometryKind::Multipoint, false, false) => MultiPoint,
            (GeometryKind::Polyline, true, _) => PolyLineZ,
            (GeometryKind::Polyline, false, true) => PolyLineM,
            (GeometryKind::Polyline, false, false) => PolyLine,
            (GeometryKind::Polygon, true, _) => PolygonZ,
            (GeometryKind::Polygon, false, true) => PolygonM,
            (GeometryKind::Polygon, false, false) => Polygon,
        }
    }

    /// ArcGIS `Describe.shapeType` name.
    pub fn esri_name(self) -> &'static str {
        match self.kind() {
            None => "Null",
            Some(GeometryKind::Point) => "Point",
            Some(GeometryKind::Multipoint) => "Multipoint",
            Some(GeometryKind::Polyline) => "Polyline",
            Some(GeometryKind::Polygon) => "Polygon",
            Some(GeometryKind::MultiPatch) => "MultiPatch",
        }
    }
}
