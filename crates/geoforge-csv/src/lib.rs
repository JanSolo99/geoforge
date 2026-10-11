//! CSV tables as ArcGIS Pro reads and writes them.
//!
//! Reading: RFC 4180 (quoted fields, doubled quotes), an optional UTF-8 BOM, the
//! first row naming the fields. Types are inferred per column like ArcGIS:
//! whole numbers only → Long, any decimal → Double, anything else → Text(254);
//! empty cells are null.
//!
//! Writing (Export Table): UTF-8 with BOM, CRLF line ends, no OID or shape
//! column, doubles with 15 decimals (`%.15f`), fields quoted when they hold a
//! comma, quote or line break.

use std::path::Path;

use geoforge_core::{
    Envelope, Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value,
};

/// Splits CSV text into records of fields.
fn records(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                out.push(std::mem::take(&mut row));
                any = false;
            }
            _ => field.push(c),
        }
    }
    if any {
        row.push(field);
        out.push(row);
    }
    out.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    out
}

/// A CSV file opened as a table (no geometry).
pub struct CsvTable {
    fields: Vec<Field>,
    rows: Vec<Vec<Value>>,
}

impl CsvTable {
    pub fn open(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut recs = records(text).into_iter();
        let header = recs
            .next()
            .ok_or_else(|| Error::format("CSV", "no header row"))?;
        let body: Vec<Vec<String>> = recs.collect();
        let fields: Vec<Field> = header
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let cells = body
                    .iter()
                    .filter_map(|r| r.get(i))
                    .filter(|c| !c.trim().is_empty());
                let mut kind = FieldType::Integer;
                for c in cells {
                    let c = c.trim();
                    if c.parse::<i32>().is_ok() {
                        continue;
                    }
                    if c.parse::<f64>().is_ok() {
                        kind = FieldType::Double;
                    } else {
                        kind = FieldType::String;
                        break;
                    }
                }
                let mut f = Field::new(name.trim(), kind);
                match kind {
                    FieldType::String => f.length = 254,
                    FieldType::Integer => f.precision = 10,
                    _ => {}
                }
                f
            })
            .collect();
        let rows = body
            .iter()
            .map(|r| {
                fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let c = r.get(i).map_or("", String::as_str);
                        if c.trim().is_empty() {
                            return Value::Null;
                        }
                        match f.field_type {
                            FieldType::Integer => {
                                c.trim().parse().map_or(Value::Null, Value::Integer)
                            }
                            FieldType::Double => {
                                c.trim().parse().map_or(Value::Null, Value::Double)
                            }
                            _ => Value::String(c.to_owned()),
                        }
                    })
                    .collect()
            })
            .collect();
        Ok(Self { fields, rows })
    }
}

impl FeatureClass for CsvTable {
    fn geometry_kind(&self) -> Option<GeometryKind> {
        None
    }
    fn has_z(&self) -> bool {
        false
    }
    fn has_m(&self) -> bool {
        false
    }
    fn fields(&self) -> &[Field] {
        &self.fields
    }
    fn oid_field_name(&self) -> &str {
        "ObjectID"
    }
    fn shape_field_name(&self) -> &str {
        ""
    }
    fn spatial_reference_wkt(&self) -> Option<&str> {
        None
    }
    fn stored_extent(&self) -> Option<Envelope> {
        None
    }
    fn features(&self) -> Box<dyn Iterator<Item = Result<Feature>> + '_> {
        Box::new(self.rows.iter().enumerate().map(|(i, r)| {
            Ok(Feature {
                oid: i as i64 + 1,
                geometry: None,
                attributes: r.clone(),
            })
        }))
    }
}

fn cell(v: &Value) -> String {
    let text = match v {
        Value::Null => return String::new(),
        Value::SmallInteger(n) => return n.to_string(),
        Value::Integer(n) => return n.to_string(),
        Value::BigInteger(n) => return n.to_string(),
        Value::Single(x) => return format!("{:.15}", f64::from(*x)),
        Value::Double(x) => return format!("{x:.15}"),
        Value::String(s) | Value::Guid(s) | Value::Xml(s) => s.clone(),
        Value::Date(d) => d.format("%Y-%m-%d %H:%M:%S").to_string(),
        Value::DateOnly(d) => d.format("%Y-%m-%d").to_string(),
        Value::TimeOnly(t) => t.format("%H:%M:%S").to_string(),
        Value::TimestampOffset(t) => t.to_rfc3339(),
        Value::Blob(_) => return String::new(),
    };
    if text.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

/// The CSV text ArcGIS's Export Table writes for these fields and rows.
pub fn write(fields: &[Field], rows: impl IntoIterator<Item = Vec<Value>>) -> String {
    let mut out = String::from("\u{feff}");
    let header: Vec<String> = fields
        .iter()
        .map(|f| cell(&Value::String(f.name.clone())))
        .collect();
    out.push_str(&header.join(","));
    out.push_str("\r\n");
    for row in rows {
        let cells: Vec<String> = row.iter().map(cell).collect();
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_like_arcgis() {
        let t = CsvTable::parse(
            "\u{feff}ID,X,NAME\r\n1,500.5,\"a, b\"\r\n2,7,\"say \"\"hi\"\"\"\r\n3,,c\r\n",
        )
        .unwrap();
        let types: Vec<FieldType> = t.fields().iter().map(|f| f.field_type).collect();
        assert_eq!(
            types,
            [FieldType::Integer, FieldType::Double, FieldType::String]
        );
        let rows: Vec<Vec<Value>> = t.features().map(|f| f.unwrap().attributes).collect();
        assert_eq!(rows[0][2], Value::String("a, b".into()));
        assert_eq!(rows[1][2], Value::String("say \"hi\"".into()));
        assert_eq!(rows[2][1], Value::Null);
        let csv = write(t.fields(), rows);
        assert_eq!(
            csv,
            "\u{feff}ID,X,NAME\r\n1,500.500000000000000,\"a, b\"\r\n2,7.000000000000000,\"say \"\"hi\"\"\"\r\n3,,c\r\n"
        );
    }
}
