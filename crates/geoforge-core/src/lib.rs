//! Core types shared by every GeoForge crate.
//!
//! The geometry model mirrors Esri's internal layout (flat coordinate arrays plus
//! part offsets), which is what both Shapefile and File Geodatabase store on disk.

pub mod dataset;
pub mod envelope;
pub mod error;
pub mod feature;
pub mod field;
pub mod geometry;

pub use dataset::FeatureClass;
pub use envelope::Envelope;
pub use error::{Error, Result};
pub use feature::{Feature, Value};
pub use field::{Field, FieldType};
pub use geometry::{Geometry, GeometryKind, PartType, Segment, SegmentKind};
