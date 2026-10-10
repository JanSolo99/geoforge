//! Core types shared by every GeoForge crate.

pub mod geometry;

/// Esri field types, as stored in File Geodatabases and mapped from DBF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    SmallInteger,
    Integer,
    BigInteger,
    Single,
    Double,
    String,
    Date,
    DateOnly,
    TimeOnly,
    TimestampOffset,
    ObjectId,
    Geometry,
    Blob,
    Raster,
    Guid,
    GlobalId,
    Xml,
}
