use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};

use crate::Geometry;

/// An attribute value. Variants correspond to [`crate::FieldType`].
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    SmallInteger(i16),
    Integer(i32),
    BigInteger(i64),
    Single(f32),
    Double(f64),
    String(String),
    Date(NaiveDateTime),
    DateOnly(NaiveDate),
    TimeOnly(NaiveTime),
    TimestampOffset(DateTime<FixedOffset>),
    Blob(Vec<u8>),
    Guid(String),
    Xml(String),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    pub oid: i64,
    /// `None` is a null shape.
    pub geometry: Option<Geometry>,
    /// One value per field of the owning dataset, in field order.
    pub attributes: Vec<Value>,
}
