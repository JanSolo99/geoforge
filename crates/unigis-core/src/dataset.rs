//! Format-independent access to a feature class or table, so geoprocessing tools
//! work the same on shapefiles, file geodatabases and later formats.

use crate::{Envelope, Feature, Field, GeometryKind, Result};

pub trait FeatureClass {
    /// `None` for a table without geometry.
    fn geometry_kind(&self) -> Option<GeometryKind>;
    fn has_z(&self) -> bool;
    fn has_m(&self) -> bool;
    /// Attribute fields, excluding the ObjectID and geometry fields.
    fn fields(&self) -> &[Field];
    /// Whether the source exposes a native ObjectID. Text tables may synthesize
    /// row identifiers for iteration without having an ObjectID in ArcGIS.
    fn has_object_id(&self) -> bool {
        true
    }
    fn oid_field_name(&self) -> &str;
    fn shape_field_name(&self) -> &str;
    /// Esri WKT of the coordinate system, if defined.
    fn spatial_reference_wkt(&self) -> Option<&str>;
    /// The extent stored in the dataset's metadata (may be stale or absent).
    fn stored_extent(&self) -> Option<Envelope>;
    fn features(&self) -> Box<dyn Iterator<Item = Result<Feature>> + '_>;
}
