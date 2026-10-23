//! ArcGIS field mappings (`arcpy.FieldMappings` strings, as the `field_mapping`
//! parameter of Spatial Join takes them): output fields with their type, length,
//! merge rule and the input fields that feed them.
//!
//! ```text
//! NAME "alias" visible nullable required length Type scale precision,Rule,Delim,
//!     table,field,start,end[,table,field,start,end ...];NEXT ...
//! ```

use unigis_core::{Error, Field, FieldType, Result, Value};

use crate::MergeRule;

/// One input field feeding an output field.
#[derive(Debug, Clone, PartialEq)]
pub struct MapSource {
    /// The input table as written in the mapping (a path or a name; only its file
    /// stem is used to tell the inputs apart).
    pub table: String,
    pub field: String,
    /// The characters of a text value to use (zero-based, inclusive), or -1, -1 for all.
    pub start: i64,
    pub end: i64,
}

/// An output field and where its values come from.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldMapEntry {
    pub field: Field,
    pub rule: MergeRule,
    pub sources: Vec<MapSource>,
}

/// The file stem of a table path in either slash style (`Z:\data\parcels.shp` → `parcels`).
pub fn table_stem(table: &str) -> String {
    let name = table.rsplit(['/', '\\']).next().unwrap_or(table);
    // `main.points` is a GeoPackage table; only real file extensions are dropped.
    let name = match name.rsplit_once('.') {
        Some((stem, ext))
            if ["shp", "dbf", "csv", "txt", "tif", "img", "json", "geojson"]
                .contains(&ext.to_ascii_lowercase().as_str()) =>
        {
            stem
        }
        _ => name,
    };
    name.strip_prefix("main.")
        .unwrap_or(name)
        .to_ascii_lowercase()
}

/// Splits `text` at `sep` outside double quotes; the quotes stay in the pieces.
fn split_unquoted(text: &str, sep: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                parts.last_mut().unwrap().push(c);
            }
            c if c == sep && !quoted => parts.push(String::new()),
            c => parts.last_mut().unwrap().push(c),
        }
    }
    parts
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches('"')
}

fn bad(entry: &str, why: &str) -> Error {
    Error::Unsupported(format!("field mapping `{entry}`: {why}"))
}

fn field_type(name: &str) -> Option<FieldType> {
    Some(match name.to_ascii_lowercase().as_str() {
        "text" => FieldType::String,
        "long" => FieldType::Integer,
        "short" => FieldType::SmallInteger,
        "bigint" | "biginteger" | "big integer" => FieldType::BigInteger,
        "float" => FieldType::Single,
        "double" => FieldType::Double,
        "date" => FieldType::Date,
        _ => return None,
    })
}

/// Parses a field mapping string.
pub fn parse_field_mapping(text: &str) -> Result<Vec<FieldMapEntry>> {
    let mut entries = Vec::new();
    for entry in split_unquoted(text, ';') {
        if entry.trim().is_empty() {
            continue;
        }
        let pieces = split_unquoted(&entry, ',');
        // The header: name "alias" visible nullable required length Type scale precision
        let header = split_unquoted(pieces[0].trim(), ' ')
            .into_iter()
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>();
        if header.len() < 9 {
            return Err(bad(
                &entry,
                "expected `name \"alias\" visible nullable required length Type scale precision`",
            ));
        }
        let name = header[0].clone();
        let kind = field_type(&header[6]).ok_or_else(|| bad(&entry, "unknown field type"))?;
        let length: u32 = header[5].parse().map_err(|_| bad(&entry, "length"))?;
        let mut field = Field::new(name, kind);
        if kind == FieldType::String {
            field.length = length;
        }
        let rest = &pieces[1..];
        if rest.len() < 2 {
            return Err(bad(&entry, "merge rule and delimiter missing"));
        }
        let delimiter = unquote(&rest[1]);
        let delimiter = if delimiter == "#" || delimiter.is_empty() {
            None
        } else {
            Some(delimiter)
        };
        let rule = MergeRule::parse(rest[0].trim(), delimiter)
            .ok_or_else(|| bad(&entry, "unknown merge rule"))?;
        let mut sources = Vec::new();
        for s in rest[2..].chunks(4) {
            if s.len() < 4 {
                return Err(bad(&entry, "a source needs table, field, start and end"));
            }
            sources.push(MapSource {
                table: unquote(&s[0]).to_owned(),
                field: unquote(&s[1]).to_owned(),
                start: s[2]
                    .trim()
                    .parse()
                    .map_err(|_| bad(&entry, "start position"))?,
                end: s[3]
                    .trim()
                    .parse()
                    .map_err(|_| bad(&entry, "end position"))?,
            });
        }
        entries.push(FieldMapEntry {
            field,
            rule,
            sources,
        });
    }
    Ok(entries)
}

/// A text value restricted to the characters `start..=end` of a source.
pub fn substring(value: &Value, start: i64, end: i64) -> Value {
    match value {
        Value::String(s) if start >= 0 && end >= start => Value::String(
            s.chars()
                .skip(start as usize)
                .take((end - start + 1) as usize)
                .collect(),
        ),
        other => other.clone(),
    }
}

/// A value stored in `field`'s type, as ArcGIS converts it when the mapped output
/// field's type differs from the source's: numbers round to integers, text that is
/// not a number reads as 0, numbers become their text, text is cut to the field length.
pub fn coerce(value: &Value, field: &Field) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let as_number = |v: &Value| -> f64 {
        crate::number(v).unwrap_or_else(|| match v {
            Value::String(s) => s.trim().parse().unwrap_or(0.0),
            _ => 0.0,
        })
    };
    match field.field_type {
        FieldType::SmallInteger => Value::SmallInteger(as_number(value).round() as i16),
        FieldType::Integer => Value::Integer(as_number(value).round() as i32),
        FieldType::BigInteger => Value::BigInteger(as_number(value).round() as i64),
        FieldType::Single => Value::Single(as_number(value) as f32),
        FieldType::Double => Value::Double(as_number(value)),
        FieldType::String => {
            let text = match value {
                Value::String(s) => s.clone(),
                other => crate::number(other).map_or_else(String::new, |x| format!("{x}")),
            };
            if field.length > 0 {
                Value::String(text.chars().take(field.length as usize).collect())
            } else {
                Value::String(text)
            }
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_arcgis_strings() {
        let text = r#"AREA_ID "AREA_ID" true true false 10 Long 0 10,First,#,Z:\d\clip_areas.shp,AREA_ID,-1,-1;NM "NM" true true false 5 Text 0 0,Join,";",Z:\d\parcels.shp,NAME,0,4,Z:\d\clip_areas.shp,NAME,-1,-1"#;
        let m = parse_field_mapping(text).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].field.name, "AREA_ID");
        assert_eq!(m[0].field.field_type, FieldType::Integer);
        assert_eq!(m[0].rule, MergeRule::First);
        assert_eq!(m[1].field.length, 5);
        assert_eq!(m[1].rule, MergeRule::Join(";".into()));
        assert_eq!(m[1].sources.len(), 2);
        assert_eq!(m[1].sources[0].end, 4);
        assert_eq!(table_stem(&m[1].sources[1].table), "clip_areas");
    }

    #[test]
    fn converts_like_arcgis() {
        let long = Field::new("A", FieldType::Integer);
        assert_eq!(coerce(&Value::Double(5607.71), &long), Value::Integer(5608));
        assert_eq!(
            coerce(&Value::String("Lot 2".into()), &long),
            Value::Integer(0)
        );
        let mut text = Field::new("T", FieldType::String);
        text.length = 5;
        assert_eq!(
            coerce(&Value::String("Lot 10".into()), &text),
            Value::String("Lot 1".into())
        );
        assert_eq!(coerce(&Value::Integer(2), &text), Value::String("2".into()));
        assert_eq!(
            substring(&Value::String("Lot 10".into()), 2, 5),
            Value::String("t 10".into())
        );
    }
}
