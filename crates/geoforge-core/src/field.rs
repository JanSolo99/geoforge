/// ArcGIS field types (`esriFieldType*`).
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

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub field_type: FieldType,
    /// Maximum length for text fields; storage width for formats that have one (dBASE).
    pub length: u32,
    pub precision: u32,
    pub scale: u32,
    pub nullable: bool,
    /// Name of the attribute domain constraining this field, if any.
    pub domain: Option<String>,
}

impl Field {
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            alias: None,
            field_type,
            length: 0,
            precision: 0,
            scale: 0,
            nullable: true,
            domain: None,
        }
    }
}
