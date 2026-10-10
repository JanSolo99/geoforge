//! dBASE III/IV attribute tables (`.dbf`).

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use geoforge_core::{Error, Field, FieldType, Result, Value};

use crate::encoding::{self, DbfEncoding};

const FIELD_DESCRIPTOR_LEN: usize = 32;
const HEADER_TERMINATOR: u8 = 0x0D;
const EOF_MARKER: u8 = 0x1A;
/// ArcGIS limit on the number of fields in a `.dbf`.
pub const MAX_FIELDS: usize = 255;
pub const MAX_FIELD_NAME_LEN: usize = 10;
pub const MAX_TEXT_WIDTH: u32 = 254;

fn err(message: impl Into<String>) -> Error {
    Error::format("dBASE", message)
}

/// A field as stored in the `.dbf` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbfField {
    pub name: String,
    /// `C`, `N`, `F`, `L`, `D`, `M`, ...
    pub kind: u8,
    pub width: u8,
    pub decimals: u8,
}

impl DbfField {
    /// The ArcGIS field type for this dBASE field.
    pub fn field_type(&self) -> FieldType {
        match self.kind {
            b'N' | b'F' if self.decimals == 0 => match self.width {
                0..=4 => FieldType::SmallInteger,
                5..=11 => FieldType::Integer,
                _ => FieldType::Double,
            },
            b'N' | b'F' => FieldType::Double,
            b'D' => FieldType::Date,
            _ => FieldType::String,
        }
    }

    pub fn to_field(&self) -> Field {
        let mut f = Field::new(&self.name, self.field_type());
        f.length = u32::from(self.width);
        if matches!(self.kind, b'N' | b'F') {
            f.precision = u32::from(self.width);
            f.scale = u32::from(self.decimals);
        }
        f
    }

    /// The dBASE field ArcGIS writes for `field` (name already made valid).
    pub fn from_field(name: String, field: &Field) -> Result<Self> {
        let explicit = |default_w: u8, default_d: u8| {
            if field.precision > 0 {
                (field.precision.min(254) as u8, field.scale.min(15) as u8)
            } else {
                (default_w, default_d)
            }
        };
        let (kind, (width, decimals)) = match field.field_type {
            // Wide enough for the full i16 range (Esri documents 4, which cannot hold -32768).
            FieldType::SmallInteger => (b'N', explicit(6, 0)),
            // 11 holds the full i32 range including the sign.
            FieldType::Integer | FieldType::ObjectId => (b'N', explicit(11, 0)),
            FieldType::BigInteger => (b'N', explicit(19, 0)),
            FieldType::Single => (b'N', explicit(13, 6)),
            FieldType::Double => (b'N', explicit(19, 11)),
            FieldType::String | FieldType::Guid | FieldType::GlobalId | FieldType::Xml => {
                let len = if field.length == 0 {
                    MAX_TEXT_WIDTH
                } else {
                    field.length
                };
                (b'C', (len.min(MAX_TEXT_WIDTH) as u8, 0))
            }
            FieldType::Date | FieldType::DateOnly => (b'D', (8, 0)),
            other => {
                return Err(Error::Unsupported(format!(
                    "field `{}` of type {other:?} cannot be stored in a shapefile",
                    field.name
                )));
            }
        };
        Ok(Self {
            name,
            kind,
            width,
            decimals,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DbfHeader {
    pub num_records: u32,
    pub header_len: u16,
    pub record_len: u16,
    pub ldid: u8,
    pub fields: Vec<DbfField>,
}

pub struct DbfReader {
    file: File,
    path: PathBuf,
    header: DbfHeader,
    offsets: Vec<usize>,
    encoding: DbfEncoding,
}

impl DbfReader {
    /// `cpg` is the contents of the companion `.cpg` file, if any.
    pub fn open(path: &Path, cpg: Option<&str>) -> Result<Self> {
        let file = File::open(path).map_err(|e| Error::io(path, e))?;
        let file_len = file.metadata().map_err(|e| Error::io(path, e))?.len();
        let mut fixed = [0u8; 32];
        file.read_exact_at(&mut fixed, 0)
            .map_err(|e| Error::io(path, e))?;
        let header_len = u16::from_le_bytes([fixed[8], fixed[9]]);
        let record_len = u16::from_le_bytes([fixed[10], fixed[11]]);
        let ldid = fixed[29];

        let encoding = match cpg
            .and_then(encoding::from_cpg)
            .or_else(|| encoding::from_ldid(ldid))
        {
            Some(enc) => DbfEncoding::Known(enc),
            None => DbfEncoding::Sniff,
        };

        // Descriptors run until 0x0D; trust that over header_len, which some writers get wrong.
        let desc_end = (header_len as u64).max(32).min(file_len) as usize;
        let mut desc = vec![0u8; desc_end.saturating_sub(32)];
        file.read_exact_at(&mut desc, 32)
            .map_err(|e| Error::io(path, e))?;
        let mut fields = Vec::new();
        let mut offsets = Vec::new();
        let mut offset = 1; // deletion flag
        for d in desc.chunks_exact(FIELD_DESCRIPTOR_LEN) {
            if d[0] == HEADER_TERMINATOR {
                break;
            }
            let raw_name = &d[..11];
            let name_end = raw_name.iter().position(|&b| b == 0).unwrap_or(11);
            let name = encoding.decode(&raw_name[..name_end]).trim().to_owned();
            // Fields that run past the declared record length are unreadable; drop them.
            if offset + usize::from(d[16]) > usize::from(record_len) {
                break;
            }
            fields.push(DbfField {
                name,
                kind: d[11].to_ascii_uppercase(),
                width: d[16],
                decimals: d[17],
            });
            offsets.push(offset);
            offset += usize::from(d[16]);
        }

        // Clamp the record count to what the file actually holds.
        let declared = u32::from_le_bytes(fixed[4..8].try_into().unwrap());
        let available =
            file_len.saturating_sub(u64::from(header_len)) / u64::from(record_len.max(1));
        let num_records = declared.min(available.min(u64::from(u32::MAX)) as u32);

        Ok(Self {
            file,
            path: path.to_owned(),
            header: DbfHeader {
                num_records,
                header_len,
                record_len,
                ldid,
                fields,
            },
            offsets,
            encoding,
        })
    }

    pub fn header(&self) -> &DbfHeader {
        &self.header
    }

    pub fn encoding(&self) -> DbfEncoding {
        self.encoding
    }

    pub fn num_records(&self) -> u32 {
        self.header.num_records
    }

    /// Reads record `i`. Returns the values and whether the record is marked deleted.
    pub fn read(&self, i: u32) -> Result<(Vec<Value>, bool)> {
        if i >= self.header.num_records {
            return Err(err(format!("record {i} out of range")));
        }
        let len = usize::from(self.header.record_len);
        let mut buf = vec![0u8; len];
        let pos = u64::from(self.header.header_len) + u64::from(i) * len as u64;
        self.file
            .read_exact_at(&mut buf, pos)
            .map_err(|e| Error::io(&self.path, e))?;
        let deleted = buf[0] == b'*';
        let values = self
            .header
            .fields
            .iter()
            .zip(&self.offsets)
            .map(|(f, &o)| self.parse_value(f, &buf[o..o + usize::from(f.width)]))
            .collect();
        Ok((values, deleted))
    }

    fn parse_value(&self, field: &DbfField, raw: &[u8]) -> Value {
        match field.kind {
            b'C' | b'M' => {
                let end = raw
                    .iter()
                    .rposition(|&b| b != b' ' && b != 0)
                    .map_or(0, |p| p + 1);
                Value::String(self.encoding.decode(&raw[..end]))
            }
            b'L' => match raw.first().map(u8::to_ascii_uppercase) {
                Some(b @ (b'T' | b'Y' | b'F' | b'N')) => Value::String((b as char).to_string()),
                _ => Value::Null,
            },
            _ => {
                let s = String::from_utf8_lossy(raw);
                let s = s.trim_matches(|c: char| c == ' ' || c == '\0');
                match field.field_type() {
                    FieldType::SmallInteger | FieldType::Integer => {
                        parse_int(s, field.field_type())
                    }
                    FieldType::Double => s.parse::<f64>().map_or(Value::Null, Value::Double),
                    FieldType::Date => parse_date(s),
                    _ => Value::String(s.to_owned()),
                }
            }
        }
    }
}

fn parse_int(s: &str, ty: FieldType) -> Value {
    let v = match s.parse::<i64>() {
        Ok(v) => v,
        // Non-conformant writers put decimals in integer fields.
        Err(_) => match s.parse::<f64>() {
            Ok(f) if f.is_finite() => f as i64,
            _ => return Value::Null,
        },
    };
    match ty {
        // Values too wide for the column's integer type fall back to Double rather than null.
        FieldType::SmallInteger => {
            i16::try_from(v).map_or(Value::Double(v as f64), Value::SmallInteger)
        }
        _ => i32::try_from(v).map_or(Value::Double(v as f64), Value::Integer),
    }
}

/// `YYYYMMDD`; blank, all-zero, or invalid dates are null. Some writers use `YYYY-MM-DD`.
fn parse_date(s: &str) -> Value {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 8 {
        return Value::Null;
    }
    let (y, m, d) = (&digits[..4], &digits[4..6], &digits[6..8]);
    match (y.parse(), m.parse(), d.parse()) {
        (Ok(y), Ok(m), Ok(d)) => NaiveDate::from_ymd_opt(y, m, d)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map_or(Value::Null, Value::Date),
        _ => Value::Null,
    }
}

/// Streams a `.dbf` to disk. ArcGIS null handling: numbers become 0, text blank,
/// dates zero (displayed as null).
pub struct DbfWriter {
    out: BufWriter<File>,
    path: PathBuf,
    fields: Vec<DbfField>,
    num_records: u32,
    len: u64,
    record: Vec<u8>,
}

impl DbfWriter {
    /// Creates the file; `today` is stamped in the header.
    pub fn create(path: &Path, fields: Vec<DbfField>, today: NaiveDate) -> Result<Self> {
        if fields.len() > MAX_FIELDS {
            return Err(Error::Limit(format!(
                "a .dbf holds at most {MAX_FIELDS} fields"
            )));
        }
        let record_len = 1 + fields.iter().map(|f| usize::from(f.width)).sum::<usize>();
        if record_len > usize::from(u16::MAX) {
            return Err(Error::Limit(
                "dBASE record length exceeds 65535 bytes".into(),
            ));
        }
        let header_len = 32 + 32 * fields.len() + 1;
        let mut h = Vec::with_capacity(header_len);
        h.push(0x03);
        h.push((today.year() - 1900).clamp(0, 255) as u8);
        h.push(today.month() as u8);
        h.push(today.day() as u8);
        h.extend_from_slice(&0u32.to_le_bytes()); // record count, patched in finish()
        h.extend_from_slice(&(header_len as u16).to_le_bytes());
        h.extend_from_slice(&(record_len as u16).to_le_bytes());
        h.extend_from_slice(&[0; 17]);
        h.push(0); // language driver ID: the encoding comes from the .cpg
        h.extend_from_slice(&[0; 2]);
        for f in &fields {
            let mut name = [0u8; 11];
            name[..f.name.len()].copy_from_slice(f.name.as_bytes());
            h.extend_from_slice(&name);
            h.push(f.kind);
            h.extend_from_slice(&[0; 4]);
            h.push(f.width);
            h.push(f.decimals);
            h.extend_from_slice(&[0; 14]);
        }
        h.push(HEADER_TERMINATOR);

        let file = File::create(path).map_err(|e| Error::io(path, e))?;
        let mut out = BufWriter::new(file);
        out.write_all(&h).map_err(|e| Error::io(path, e))?;
        Ok(Self {
            out,
            path: path.to_owned(),
            fields,
            num_records: 0,
            len: h.len() as u64 + 1, // + EOF marker
            record: Vec::with_capacity(record_len),
        })
    }

    /// Size the finished file will have, including everything written so far.
    pub fn len_bytes(&self) -> u64 {
        self.len
    }

    /// Size of one record in bytes.
    pub fn record_len(&self) -> u64 {
        1 + self.fields.iter().map(|f| u64::from(f.width)).sum::<u64>()
    }

    pub fn push(&mut self, values: &[Value]) -> Result<()> {
        let rec = &mut self.record;
        rec.clear();
        rec.push(b' ');
        for (i, f) in self.fields.iter().enumerate() {
            let v = values.get(i).unwrap_or(&Value::Null);
            let width = usize::from(f.width);
            let start = rec.len();
            match f.kind {
                b'C' => {
                    let text = value_text(v);
                    let bytes = text.as_bytes();
                    // Truncate on a character boundary: widths are in bytes.
                    let mut end = bytes.len().min(width);
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    rec.extend_from_slice(&bytes[..end]);
                }
                b'D' => {
                    let s = match v {
                        Value::Date(dt) => ymd(dt.date()),
                        Value::DateOnly(d) => ymd(*d),
                        _ => "00000000".into(),
                    };
                    rec.extend_from_slice(s.as_bytes());
                }
                _ => {
                    let s = fit_number(format_number(v, f.decimals), width);
                    if s.len() > width {
                        rec.extend(std::iter::repeat_n(b'*', width));
                    } else {
                        rec.extend(std::iter::repeat_n(b' ', width - s.len()));
                        rec.extend_from_slice(s.as_bytes());
                    }
                }
            }
            rec.resize(start + width, b' ');
        }
        self.out
            .write_all(rec)
            .map_err(|e| Error::io(&self.path, e))?;
        self.num_records += 1;
        self.len += rec.len() as u64;
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        let io = |e| Error::io(&self.path, e);
        self.out.write_all(&[EOF_MARKER]).map_err(io)?;
        self.out.seek(SeekFrom::Start(4)).map_err(io)?;
        self.out
            .write_all(&self.num_records.to_le_bytes())
            .map_err(io)?;
        self.out.flush().map_err(io)
    }
}

fn ymd(d: NaiveDate) -> String {
    format!("{:04}{:02}{:02}", d.year(), d.month(), d.day())
}

fn value_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) | Value::Guid(s) | Value::Xml(s) => s.clone(),
        Value::SmallInteger(n) => n.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::BigInteger(n) => n.to_string(),
        Value::Single(n) => n.to_string(),
        Value::Double(n) => n.to_string(),
        Value::Date(d) => d.to_string(),
        Value::DateOnly(d) => d.to_string(),
        Value::TimeOnly(t) => t.to_string(),
        Value::TimestampOffset(t) => t.to_rfc3339(),
        Value::Blob(_) => String::new(),
    }
}

/// Falls back to scientific notation when fixed-point does not fit the width
/// (e.g. 1.79e308 in an N(19,11) field). Integers that do not fit stay too long.
fn fit_number(fixed: String, width: usize) -> String {
    if fixed.len() <= width || !fixed.contains('.') {
        return fixed;
    }
    let Ok(v) = fixed.parse::<f64>() else {
        return fixed;
    };
    (0..=15)
        .rev()
        .map(|d| format!("{v:.d$e}"))
        .find(|s| s.len() <= width)
        .unwrap_or(fixed)
}

fn format_number(v: &Value, decimals: u8) -> String {
    let d = usize::from(decimals);
    let f = match v {
        Value::SmallInteger(n) => f64::from(*n),
        Value::Integer(n) => f64::from(*n),
        Value::BigInteger(n) if d == 0 => return n.to_string(),
        Value::BigInteger(n) => *n as f64,
        Value::Single(n) => f64::from(*n),
        Value::Double(n) => *n,
        Value::String(s) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    };
    if !f.is_finite() {
        return format!("{:.d$}", 0.0);
    }
    format!("{f:.d$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(fields: Vec<DbfField>, rows: &[Vec<Value>]) -> Vec<Vec<Value>> {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("gf-dbf-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.dbf");
        let mut w =
            DbfWriter::create(&path, fields, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()).unwrap();
        rows.iter().for_each(|r| w.push(r).unwrap());
        w.finish().unwrap();
        let r = DbfReader::open(&path, Some("UTF-8")).unwrap();
        (0..r.num_records()).map(|i| r.read(i).unwrap().0).collect()
    }

    #[test]
    fn values_roundtrip_with_arcgis_null_substitution() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        let fields = vec![
            DbfField::from_field("name".into(), &Field::new("name", FieldType::String)).unwrap(),
            DbfField::from_field("n".into(), &Field::new("n", FieldType::Integer)).unwrap(),
            DbfField::from_field("x".into(), &Field::new("x", FieldType::Double)).unwrap(),
            DbfField::from_field("d".into(), &Field::new("d", FieldType::Date)).unwrap(),
        ];
        let rows = roundtrip(
            fields,
            &[
                vec![
                    Value::String("Zürich ✓".into()),
                    Value::Integer(-42),
                    Value::Double(3.25),
                    Value::Date(date.and_hms_opt(0, 0, 0).unwrap()),
                ],
                vec![Value::Null, Value::Null, Value::Null, Value::Null],
            ],
        );
        assert_eq!(rows[0][0], Value::String("Zürich ✓".into()));
        assert_eq!(rows[0][1], Value::Integer(-42));
        assert_eq!(rows[0][2], Value::Double(3.25));
        assert_eq!(rows[0][3], Value::Date(date.and_hms_opt(0, 0, 0).unwrap()));
        assert_eq!(
            rows[1],
            vec![
                Value::String(String::new()),
                Value::Integer(0),
                Value::Double(0.0),
                Value::Null
            ]
        );
    }

    #[test]
    fn extreme_values_survive() {
        let fields = vec![
            DbfField::from_field("s".into(), &Field::new("s", FieldType::SmallInteger)).unwrap(),
            DbfField::from_field("d".into(), &Field::new("d", FieldType::Double)).unwrap(),
        ];
        let rows = roundtrip(
            fields,
            &[vec![Value::SmallInteger(-32768), Value::Double(f64::MAX)]],
        );
        assert_eq!(rows[0][0], Value::Integer(-32768));
        let Value::Double(d) = rows[0][1] else {
            panic!()
        };
        assert!((d - f64::MAX).abs() / f64::MAX < 1e-10);
    }

    #[test]
    fn text_truncates_on_char_boundary() {
        let f = DbfField {
            name: "t".into(),
            kind: b'C',
            width: 4,
            decimals: 0,
        };
        let rows = roundtrip(vec![f], &[vec![Value::String("aé✓".into())]]);
        assert_eq!(rows[0][0], Value::String("aé".into()));
    }
}
