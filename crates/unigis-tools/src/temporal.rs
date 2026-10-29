//! Convert Temporal Field (Data Management): dates to text or numbers in a format, and text or
//! numbers in a format to dates.

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use unigis_core::{Error, FeatureClass, Field, FieldType, Result, Value};

use crate::ToolOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// A piece of a .NET-style date format: a run of one pattern letter, or literal text.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Pattern(char, usize),
    Literal(String),
}

fn tokens(format: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let chars: Vec<char> = format.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            let end = chars[i + 1..]
                .iter()
                .position(|&x| x == '\'')
                .map_or(chars.len(), |p| i + 1 + p);
            out.push(Token::Literal(chars[i + 1..end].iter().collect()));
            i = end + 1;
        } else if "yMdHhmsft".contains(c) {
            let n = chars[i..].iter().take_while(|&&x| x == c).count();
            out.push(Token::Pattern(c, n));
            i += n;
        } else {
            match out.last_mut() {
                Some(Token::Literal(s)) => s.push(c),
                _ => out.push(Token::Literal(c.to_string())),
            }
            i += 1;
        }
    }
    out
}

/// `d` written in `format` (`yyyy-MM-dd`, `dd MMMM yyyy`, `HHmmss`, ...).
fn format_date(d: &NaiveDateTime, format: &str) -> String {
    let mut s = String::new();
    for t in tokens(format) {
        match t {
            Token::Literal(l) => s.push_str(&l),
            Token::Pattern(c, n) => {
                let num = |v: u32, width: usize| format!("{v:0width$}");
                s.push_str(&match (c, n) {
                    ('y', 2) => num(d.year().rem_euclid(100) as u32, 2),
                    ('y', n) => format!("{:0n$}", d.year()),
                    ('M', 1 | 2) => num(d.month(), n),
                    ('M', 3) => MONTHS[d.month0() as usize][..3].to_owned(),
                    ('M', _) => MONTHS[d.month0() as usize].to_owned(),
                    ('d', 1 | 2) => num(d.day(), n),
                    ('d', 3) => DAYS[d.weekday().num_days_from_monday() as usize][..3].to_owned(),
                    ('d', _) => DAYS[d.weekday().num_days_from_monday() as usize].to_owned(),
                    ('H', n) => num(d.hour(), n.min(2)),
                    ('h', n) => num((d.hour() + 11) % 12 + 1, n.min(2)),
                    ('m', n) => num(d.minute(), n.min(2)),
                    ('s', n) => num(d.second(), n.min(2)),
                    ('f', n) => {
                        let ms = format!("{:09}", d.nanosecond());
                        ms[..n.min(9)].to_owned()
                    }
                    ('t', _) => (if d.hour() < 12 { "am" } else { "pm" }).to_owned(),
                    _ => String::new(),
                });
            }
        }
    }
    s
}

/// `text` read with `format`, leniently as ArcGIS does: numeric parts take up to their width in
/// digits (one digit is enough: `2001011` as `yyyyMMdd` is 2001-01-01), month names in full or
/// as three letters; an impossible date is `None`.
fn parse_date(text: &str, format: &str) -> Option<NaiveDateTime> {
    let chars: Vec<char> = text.trim().chars().collect();
    let mut i = 0;
    let (mut y, mut mo, mut d, mut h, mut mi, mut s) = (None, None, None, 0, 0, 0);
    let mut pm: Option<bool> = None;
    let digits = |i: &mut usize, max: usize| -> Option<u32> {
        let start = *i;
        while *i < chars.len() && *i - start < max && chars[*i].is_ascii_digit() {
            *i += 1;
        }
        (*i > start).then(|| chars[start..*i].iter().collect::<String>().parse().ok())?
    };
    for t in tokens(format) {
        match t {
            Token::Literal(l) => {
                for c in l.chars() {
                    if chars.get(i) != Some(&c) {
                        return None;
                    }
                    i += 1;
                }
            }
            Token::Pattern(c, n) => match (c, n) {
                ('y', 2) => {
                    let v = digits(&mut i, 2)? as i32;
                    y = Some(if v < 30 { 2000 + v } else { 1900 + v });
                }
                ('y', _) => y = Some(digits(&mut i, 4)? as i32),
                ('M', 1 | 2) => mo = Some(digits(&mut i, 2)?),
                ('M', _) | ('d', 3..) | ('t', _) => {
                    let word: String = chars[i..]
                        .iter()
                        .take_while(|c| c.is_alphabetic())
                        .collect();
                    i += word.chars().count();
                    let lower = word.to_lowercase();
                    match c {
                        'M' => {
                            let k = MONTHS.iter().position(|m| {
                                let m = m.to_lowercase();
                                if n == 3 { m[..3] == lower } else { m == lower }
                            })?;
                            mo = Some(k as u32 + 1);
                        }
                        't' => pm = Some(lower.starts_with('p')),
                        _ => {} // a day name: not checked
                    }
                }
                ('d', _) => d = Some(digits(&mut i, 2)?),
                ('H' | 'h', _) => h = digits(&mut i, 2)?,
                ('m', _) => mi = digits(&mut i, 2)?,
                ('s', _) => s = digits(&mut i, 2)?,
                ('f', n) => {
                    digits(&mut i, n)?;
                }
                _ => return None,
            },
        }
    }
    if i != chars.len() {
        return None;
    }
    if let Some(pm) = pm {
        h = h % 12 + if pm { 12 } else { 0 };
    }
    NaiveDate::from_ymd_opt(y.unwrap_or(1899), mo.unwrap_or(12), d.unwrap_or(30))?
        .and_hms_opt(h, mi, s)
}

/// The output field type keyword (`DATE`, `TEXT`, `LONG`, `SHORT`, `DOUBLE`, `FLOAT`,
/// `BIGINTEGER`).
fn output_field(name: &str, kind: &str) -> Result<Field> {
    let (t, length) = match kind.to_ascii_uppercase().as_str() {
        "DATE" | "" => (FieldType::Date, 8),
        "TEXT" => (FieldType::String, 255),
        "LONG" => (FieldType::Integer, 10),
        "SHORT" => (FieldType::SmallInteger, 5),
        "DOUBLE" | "FLOAT" => (FieldType::Double, 19),
        "BIGINTEGER" => (FieldType::BigInteger, 19),
        other => {
            return Err(err(format!(
                "output time type {other} is not supported yet"
            )));
        }
    };
    let mut f = Field::new(name, t);
    f.length = length;
    if t == FieldType::Double {
        (f.precision, f.scale) = (19, 11);
    }
    Ok(f)
}

/// Convert Temporal Field: `field` (a date, or text/numbers in `input_format`) written to a new
/// field `output_name` of `output_type`, in `output_format` for text and numeric outputs
/// (default `yyyyMMddHHmmss` for text). Values that do not convert, nulls included, are null
/// with WARNING 002125; when none converts the tool fails (ERROR 002126; observed against ArcGIS
/// Pro 3.7).
pub fn convert_time_field(
    input: &dyn FeatureClass,
    field: &str,
    input_format: Option<&str>,
    output_name: &str,
    output_type: &str,
    output_format: Option<&str>,
) -> Result<ToolOutput> {
    let fields = input.fields().to_vec();
    let fi = fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(field))
        .ok_or_else(|| err(format!("ERROR 000728: Field {field} does not exist")))?;
    let out = output_field(output_name, output_type)?;
    let out_format =
        output_format
            .filter(|f| !f.is_empty())
            .unwrap_or(if out.field_type == FieldType::String {
                "yyyyMMddHHmmss"
            } else {
                "yyyyMMdd"
            });
    let read = |v: &Value| -> Option<NaiveDateTime> {
        match v {
            Value::Date(d) => Some(*d),
            Value::DateOnly(d) => d.and_hms_opt(0, 0, 0),
            Value::Null => None,
            Value::String(s) => parse_date(s, input_format?),
            other => {
                let n = crate::number(other)?;
                parse_date(&format!("{}", n.trunc() as i64), input_format?)
            }
        }
    };
    let mut features = Vec::new();
    let (mut failed, mut total) = (0, 0);
    for f in input.features() {
        let mut f = f?;
        total += 1;
        let value = read(&f.attributes[fi]).and_then(|d| match out.field_type {
            FieldType::Date => Some(Value::Date(d)),
            FieldType::String => Some(Value::String(format_date(&d, out_format))),
            t => {
                let n: f64 = format_date(&d, out_format).parse().ok()?;
                Some(match t {
                    FieldType::Integer => Value::Integer(i32::try_from(n as i64).ok()?),
                    FieldType::SmallInteger => Value::SmallInteger(i16::try_from(n as i64).ok()?),
                    FieldType::BigInteger => Value::BigInteger(n as i64),
                    _ => Value::Double(n),
                })
            }
        });
        if value.is_none() {
            failed += 1;
        }
        f.attributes.push(value.unwrap_or(Value::Null));
        features.push(f);
    }
    if total > 0 && failed == total {
        return Err(err(format!(
            "ERROR 002126: Unable to convert all of the values from {field} to {output_name}."
        )));
    }
    let mut out_fields = fields;
    out_fields.push(out);
    Ok(ToolOutput {
        kind: input
            .geometry_kind()
            .unwrap_or(unigis_core::GeometryKind::Point),
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: out_fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: if failed > 0 {
            vec![format!(
                "WARNING 002125: Unable to convert part of the values from {field} to {output_name}."
            )]
        } else {
            Vec::new()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, mi, s)
            .unwrap()
    }

    #[test]
    fn formats_match_arcgis() {
        let d = dt(2024, 2, 29, 13, 5, 9);
        assert_eq!(format_date(&d, "dd MMMM yyyy"), "29 February 2024");
        assert_eq!(format_date(&d, "MMM d, yy"), "Feb 29, 24");
        assert_eq!(format_date(&d, "HHmmss"), "130509");
        assert_eq!(format_date(&d, "dddd"), "Thursday");
        assert_eq!(
            format_date(&d, "yyyy-MM-ddTHH:mm:ss"),
            "2024-02-29T13:05:09"
        );
        assert_eq!(format_date(&d, "yyyyMMddHHmmss"), "20240229130509");
    }

    #[test]
    fn parsing_is_lenient_like_arcgis() {
        let day = |y, m, d| Some(dt(y, m, d, 0, 0, 0));
        assert_eq!(parse_date("2001011", "yyyyMMdd"), day(2001, 1, 1));
        assert_eq!(parse_date("1/1/2001", "MM/dd/yyyy"), day(2001, 1, 1));
        assert_eq!(parse_date("13/40/2020", "MM/dd/yyyy"), None);
        assert_eq!(
            parse_date("2020-02-30 00:00:00", "yyyy-MM-dd HH:mm:ss"),
            None
        );
        assert_eq!(
            parse_date("2001-01-01 7:05:00", "yyyy-MM-dd HH:mm:ss"),
            Some(dt(2001, 1, 1, 7, 5, 0))
        );
        assert_eq!(
            parse_date("29 February 2024", "d MMMM yyyy"),
            day(2024, 2, 29)
        );
        assert_eq!(parse_date("2 Jan 2020", "d MMMM yyyy"), None);
        assert_eq!(parse_date("notadate", "yyyyMMdd"), None);
    }
}
