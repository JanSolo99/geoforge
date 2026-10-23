//! Keys of attribute indexes (`.atx`), as ArcGIS Pro writes them: integers and floats
//! in their stored form, dates as f64 days, text as space-padded UTF-16 (case
//! sensitive), GUIDs as their 38 ASCII characters. Nulls are not indexed; equal keys
//! are ordered by OBJECTID.

use std::cmp::Ordering;

use chrono::{NaiveDate, NaiveTime};
use unigis_core::Value;

use crate::table::{GdbField, GdbFieldType};

/// Text keys hold at most this many characters.
const MAX_TEXT_KEY: usize = 80;

/// How a field's values are stored in an index.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    pub key_size: usize,
    /// The byte after the key size in the file trailer: 0x20 text, 0x40 integers,
    /// 0x60 floating point and dates, 0 GUIDs.
    pub kind: u8,
    form: Form,
}

#[derive(Debug, Clone, Copy)]
enum Form {
    I16,
    I32,
    I64,
    F32,
    F64,
    Text(usize),
    Guid,
}

/// An index key, comparable in the order the index sorts by.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Key {
    Int(i64),
    Float(f64),
    Units(Vec<u16>),
    Ascii(Vec<u8>),
}

impl Key {
    pub fn cmp(&self, other: &Key) -> Ordering {
        match (self, other) {
            (Key::Int(a), Key::Int(b)) => a.cmp(b),
            (Key::Float(a), Key::Float(b)) => a.total_cmp(b),
            (Key::Units(a), Key::Units(b)) => a.cmp(b),
            (Key::Ascii(a), Key::Ascii(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }
}

/// The index layout of a field, or `None` if the type cannot be indexed.
pub(crate) fn layout(f: &GdbField) -> Option<Layout> {
    use GdbFieldType as G;
    let (key_size, kind, form) = match f.field_type {
        G::Int16 => (2, 0x40, Form::I16),
        G::Int32 => (4, 0x40, Form::I32),
        G::Int64 => (8, 0x40, Form::I64),
        G::Float32 => (4, 0x60, Form::F32),
        G::Float64 | G::DateTime | G::DateOnly | G::TimeOnly | G::DateTimeOffset => {
            (8, 0x60, Form::F64)
        }
        G::String => {
            let chars = (f.length as usize).clamp(1, MAX_TEXT_KEY);
            (2 * chars, 0x20, Form::Text(chars))
        }
        G::Guid | G::GlobalId => (38, 0, Form::Guid),
        _ => return None,
    };
    Some(Layout {
        key_size,
        kind,
        form,
    })
}

fn days(t: chrono::NaiveDateTime) -> f64 {
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    (t - epoch).num_milliseconds() as f64 / 86_400_000.0
}

fn number(v: &Value) -> Option<f64> {
    Some(match v {
        Value::SmallInteger(n) => f64::from(*n),
        Value::Integer(n) => f64::from(*n),
        Value::BigInteger(n) => *n as f64,
        Value::Single(n) => f64::from(*n),
        Value::Double(n) => *n,
        _ => return None,
    })
}

fn integer(v: &Value) -> Option<i64> {
    Some(match v {
        Value::SmallInteger(n) => i64::from(*n),
        Value::Integer(n) => i64::from(*n),
        Value::BigInteger(n) => *n,
        _ => return None,
    })
}

/// The key for `v`, or `None` for null (and values the field cannot hold).
pub(crate) fn key_of(l: &Layout, v: &Value) -> Option<Key> {
    if v.is_null() {
        return None;
    }
    Some(match l.form {
        Form::I16 | Form::I32 | Form::I64 => Key::Int(integer(v)?),
        Form::F32 => Key::Float(f64::from(number(v)? as f32)),
        Form::F64 => Key::Float(match v {
            Value::Date(t) => days(*t),
            Value::DateOnly(d) => days(d.and_hms_opt(0, 0, 0)?),
            Value::TimeOnly(t) => (*t - NaiveTime::MIN).num_milliseconds() as f64 / 86_400_000.0,
            Value::TimestampOffset(t) => days(t.naive_utc()),
            other => number(other)?,
        }),
        Form::Text(chars) => {
            let Value::String(s) = v else { return None };
            let mut units: Vec<u16> = s
                .chars()
                .take(chars)
                .flat_map(|c| {
                    let mut b = [0u16; 2];
                    c.encode_utf16(&mut b).to_vec()
                })
                .collect();
            units.resize(chars, u16::from(b' '));
            Key::Units(units)
        }
        Form::Guid => {
            let (Value::Guid(s) | Value::String(s)) = v else {
                return None;
            };
            Key::Ascii(s.to_ascii_uppercase().into_bytes())
        }
    })
}

/// The bytes stored for a key.
pub(crate) fn encode(l: &Layout, k: &Key) -> Vec<u8> {
    let mut out = match (l.form, k) {
        (Form::I16, Key::Int(n)) => (*n as i16).to_le_bytes().to_vec(),
        (Form::I32, Key::Int(n)) => (*n as i32).to_le_bytes().to_vec(),
        (Form::I64, Key::Int(n)) => n.to_le_bytes().to_vec(),
        (Form::F32, Key::Float(f)) => (*f as f32).to_le_bytes().to_vec(),
        (Form::F64, Key::Float(f)) => f.to_le_bytes().to_vec(),
        (Form::Text(_), Key::Units(u)) => u.iter().flat_map(|c| c.to_le_bytes()).collect(),
        (Form::Guid, Key::Ascii(a)) => a.clone(),
        _ => Vec::new(),
    };
    out.resize(l.key_size, 0);
    out
}
