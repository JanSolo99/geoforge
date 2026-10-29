//! Excel To Table: native workbook reading, with schema observed in ArcGIS Pro.
use std::{collections::HashSet, path::Path};

use calamine::{Data, DataType, Reader, open_workbook_auto};
use unigis_core::{Error, Field, FieldType, Result, Value};

use crate::{
    TableOutput,
    registry::{Context, Kind, Outcome, Param, Runner, Tool},
};

pub const TOOL: Tool = Tool {
    id: "excel_to_table",
    name: "Excel To Table",
    toolbox: "Conversion › Excel",
    summary: "Converts an Excel worksheet or single named range to a table.",
    params: &[
        Param {
            name: "input",
            label: "Input Excel File",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "An .xls or .xlsx workbook.",
        },
        Param {
            name: "sheet",
            label: "Sheet Or Named Range",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Defaults to the first sheet; single rectangular .xlsx named ranges supported.",
        },
        Param {
            name: "field_names_row",
            label: "Row To Use As Field Names",
            kind: Kind::Number,
            required: false,
            default: "1",
            help: "One-based header row, or 0 for COL_A, COL_B, etc.",
        },
        Param {
            name: "cell_range",
            label: "Cell Range",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Inclusive rectangle, for example B2:D10. Header names still come from field_names_row.",
        },
    ],
    output_suffix: "ExcelToTable",
    measures: false,
    run: Runner::Dynamic(run),
};

fn error(message: impl Into<String>) -> Error {
    Error::Unsupported(message.into())
}
type Bounds = ((u32, u32), (u32, u32));

/// Absolute, zero-based (row, column), within Excel's worksheet limits.
fn address(text: &str) -> Option<(u32, u32)> {
    let text = text.replace('$', "");
    let split = text.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = text.split_at(split);
    if letters.is_empty()
        || !letters.bytes().all(|c| c.is_ascii_alphabetic())
        || !digits.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let col = letters.bytes().try_fold(0u32, |n, c| {
        n.checked_mul(26)?
            .checked_add(u32::from(c.to_ascii_uppercase() - b'A') + 1)
    })?;
    let row: u32 = digits.parse().ok()?;
    (row > 0 && row <= 1_048_576 && col > 0 && col <= 16_384).then(|| (row - 1, col - 1))
}
fn bounds(text: &str) -> Result<Bounds> {
    let fail = || {
        error(format!(
            "ERROR 002926: Invalid value for parameter cell_range: {text}"
        ))
    };
    let (a, b) = text.split_once(':').ok_or_else(fail)?;
    let a = address(a).ok_or_else(fail)?;
    let b = address(b).ok_or_else(fail)?;
    if a.0 > b.0 || a.1 > b.1 {
        return Err(fail());
    }
    Ok((a, b))
}
fn column(mut col: u32) -> String {
    let mut out = String::new();
    col += 1;
    while col > 0 {
        col -= 1;
        out.insert(0, (b'A' + (col % 26) as u8) as char);
        col /= 26;
    }
    out
}
fn text(cell: &Data) -> Result<String> {
    Ok(match cell {
        Data::Empty => String::new(),
        Data::String(s) if s == "#N/A" => String::new(),
        Data::String(s) => s.clone(),
        Data::Bool(b) => if *b { "True" } else { "False" }.into(),
        Data::Int(n) => n.to_string(),
        Data::Float(n) if n.is_finite() => n.to_string(),
        _ => {
            return Err(error(
                "Excel formatted dates, durations, nonfinite numbers and errors in text columns are not supported",
            ));
        }
    })
}
fn null(cell: &Data) -> bool {
    matches!(cell, Data::Empty | Data::Error(calamine::CellErrorType::NA))
        || matches!(cell, Data::String(s) if s == "#N/A")
}

fn field_name(raw: &str, dbf: bool, seen: &mut HashSet<String>) -> String {
    let limit = if dbf { 10 } else { 64 };
    let mut name: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        name.insert(0, if dbf { 'a' } else { 'F' });
    }
    if !dbf
        && [
            "SELECT", "FROM", "WHERE", "ORDER", "BY", "GROUP", "TABLE", "INSERT", "UPDATE",
            "DELETE", "ADD", "ALTER", "DROP",
        ]
        .contains(&name.to_ascii_uppercase().as_str())
    {
        name.push('_');
    }
    name.truncate(limit);
    let base = name.clone();
    let mut i = 1;
    while !seen.insert(name.to_ascii_uppercase()) {
        let suffix = format!("_{i}");
        name = format!("{}{suffix}", &base[..base.len().min(limit - suffix.len())]);
        i += 1;
    }
    name
}

fn run(ctx: &dyn Context) -> Result<Outcome> {
    let input = ctx.get("input").ok_or_else(|| error("input is required"))?;
    let path = Path::new(&input);
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "xls" | "xlsx") {
        return Err(error("Input Excel File must be .xls or .xlsx"));
    }
    let output = ctx
        .get("output")
        .ok_or_else(|| error("output is required"))?;
    let gdb = Path::new(&output)
        .parent()
        .is_some_and(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gdb")));
    let dbf = !gdb
        && Path::new(&output)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dbf"));
    if !dbf && !gdb {
        return Err(error(
            "Excel To Table supports .dbf or a table in a FileGDB",
        ));
    }
    let mut workbook =
        open_workbook_auto(path).map_err(|e| error(format!("Cannot read Excel workbook: {e}")))?;
    let sheets = workbook.sheet_names();
    let requested = ctx.get("sheet").unwrap_or_default();
    let mut header: u32 = ctx
        .get("field_names_row")
        .unwrap_or_else(|| "1".into())
        .parse()
        .map_err(|_| error("field_names_row must be a nonnegative integer"))?;
    if header > 1_048_576 {
        return Err(error("field_names_row is outside the worksheet"));
    }
    let mut selected = ctx.get("cell_range").map(|s| bounds(&s)).transpose()?;
    let sheet = if requested.is_empty() {
        sheets
            .first()
            .cloned()
            .ok_or_else(|| error("Workbook has no sheets"))?
    } else if sheets.contains(&requested) {
        requested
    } else if extension == "xlsx" {
        let formula = workbook
            .defined_names()
            .iter()
            .find(|(n, _)| n == &requested)
            .map(|(_, f)| f.clone())
            .ok_or_else(|| {
                error("ERROR 003958: Sheet or supported rectangular named range not found")
            })?;
        let (sheet, cells) = formula
            .rsplit_once('!')
            .ok_or_else(|| error("Only single rectangular named ranges are supported"))?;
        if cells.contains(',') || sheet.contains(['[', ']']) {
            return Err(error(
                "Only single rectangular named ranges in this workbook are supported",
            ));
        }
        let area = bounds(cells)?;
        if area.0 == area.1 {
            return Err(error(
                "ERROR 003958: Single-cell named ranges are not supported",
            ));
        }
        header = area.0.0 + 1;
        selected = Some(area);
        sheet.trim_matches('\'').replace("''", "'")
    } else {
        return Err(error("Sheet not found; named ranges require .xlsx"));
    };
    let data = workbook
        .worksheet_range(&sheet)
        .map_err(|e| error(format!("Cannot read worksheet: {e}")))?;
    let end = data.end().ok_or_else(|| error("Worksheet has no fields"))?;
    if header > end.0 + 1 {
        return Err(error("field_names_row is outside the used worksheet"));
    }
    let (start, stop) = selected.unwrap_or(((0, 0), end));
    let stop = (stop.0.min(end.0), stop.1.min(end.1));
    if start.0 > stop.0 || start.1 > stop.1 {
        return Err(error("Cell range does not intersect the worksheet"));
    }
    // Refuse uncached formulas rather than importing silently empty results.
    let formulas = workbook
        .worksheet_formula(&sheet)
        .map_err(|e| error(format!("Cannot read worksheet formulas: {e}")))?;
    let formula_start = formulas.start().unwrap_or((0, 0));
    for (r, c, f) in formulas.cells() {
        let (r, c) = (r as u32 + formula_start.0, c as u32 + formula_start.1);
        if !f.is_empty()
            && r >= start.0
            && r <= stop.0
            && c >= start.1
            && c <= stop.1
            && data
                .get_value((r, c))
                .is_none_or(|v| matches!(v, Data::Empty))
        {
            return Err(error(
                "Formula has no cached value; calculate and save the workbook in Excel first",
            ));
        }
    }
    let cell = |row, col| data.get_value((row, col)).unwrap_or(&Data::Empty);
    let row_numbers: Vec<_> = (start.0..=stop.0)
        .filter(|r| header == 0 || *r != header - 1)
        .collect();
    if row_numbers
        .len()
        .saturating_mul((stop.1 - start.1 + 1) as usize)
        > 10_000_000
    {
        return Err(error("Excel selection exceeds the supported cell limit"));
    }
    let mut seen = HashSet::from([
        "OBJECTID".into(),
        "OID".into(),
        "SHAPE".into(),
        "FID".into(),
    ]);
    let mut fields = Vec::new();
    let mut rows = vec![Vec::new(); row_numbers.len()];
    for col in start.1..=stop.1 {
        let alias = if header == 0 {
            String::new()
        } else {
            text(cell(header - 1, col))?
        };
        let alias = if alias.is_empty() {
            format!("COL_{}", column(col))
        } else {
            alias
        };
        let name = field_name(&alias, dbf, &mut seen);
        let cells: Vec<_> = row_numbers.iter().map(|r| cell(*r, col)).collect();
        let nonnull: Vec<_> = cells.iter().copied().filter(|v| !null(v)).collect();
        let all_number = !nonnull.is_empty()
            && nonnull
                .iter()
                .all(|c| matches!(c, Data::Int(_) | Data::Float(_)));
        let all_bool = !nonnull.is_empty() && nonnull.iter().all(|c| matches!(c, Data::Bool(_)));
        if nonnull
            .iter()
            .any(|c| matches!(c, Data::DateTime(d) if d.is_duration()))
        {
            return Err(error("Excel duration cells are not yet supported"));
        }
        let all_date =
            !nonnull.is_empty() && nonnull.iter().all(|c| matches!(c, Data::DateTime(_)));
        let kind = if all_number {
            let values: Vec<_> = nonnull.iter().map(|v| v.as_f64().unwrap()).collect();
            if values.iter().any(|v| !v.is_finite()) {
                return Err(error("Nonfinite Excel numbers are unsupported"));
            }
            if values.iter().any(|v| v.fract() != 0.0) {
                FieldType::Double
            } else if values
                .iter()
                .all(|v| *v >= i32::MIN as f64 && *v <= i32::MAX as f64)
            {
                FieldType::Integer
            } else if values.iter().all(|v| v.abs() <= 9_007_199_254_740_991.0) {
                if dbf {
                    FieldType::Double
                } else {
                    FieldType::BigInteger
                }
            } else {
                return Err(error(
                    "Excel integers outside the exact 53-bit range are unsupported",
                ));
            }
        } else if all_bool {
            FieldType::SmallInteger
        } else if all_date && dbf {
            FieldType::Date
        } else if nonnull.iter().any(|c| {
            matches!(
                c,
                Data::DateTime(_) | Data::DateTimeIso(_) | Data::DurationIso(_)
            )
        }) {
            return Err(error(
                "Excel dates in FileGDB and mixed date columns are not yet supported; date/time formatting must be retained",
            ));
        } else {
            FieldType::String
        };
        let mut field = Field::new(name, kind);
        if !dbf {
            field.alias = Some(alias);
        }
        if kind == FieldType::String {
            field.length = if dbf { 254 } else { 255 };
        }
        for (i, value) in cells.iter().enumerate() {
            let value = if null(value) {
                match kind {
                    FieldType::String if matches!(value, Data::Empty) => {
                        Value::String(String::new())
                    }
                    FieldType::Integer if dbf => Value::Integer(0),
                    FieldType::SmallInteger if dbf => Value::SmallInteger(0),
                    FieldType::Double if dbf => Value::Double(0.0),
                    _ => Value::Null,
                }
            } else {
                match kind {
                    FieldType::String => {
                        let s = text(value)?;
                        if s.len() > field.length as usize {
                            return Err(error(
                                "Excel text exceeding the default output field width is not yet supported",
                            ));
                        }
                        Value::String(s)
                    }
                    FieldType::Integer => Value::Integer(value.as_f64().unwrap() as i32),
                    FieldType::BigInteger => Value::BigInteger(value.as_f64().unwrap() as i64),
                    FieldType::Double => Value::Double(value.as_f64().unwrap()),
                    FieldType::SmallInteger => {
                        Value::SmallInteger(i16::from(matches!(value, Data::Bool(true))))
                    }
                    FieldType::Date => {
                        let date = value
                            .as_datetime()
                            .ok_or_else(|| error("Excel date outside supported range"))?;
                        // DBF drops times, and Excel-only times use 1899-12-30.
                        let day = if matches!(value, Data::DateTime(d) if d.as_f64() >= 0.0 && d.as_f64() < 1.0)
                        {
                            chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap()
                        } else {
                            date.date()
                        };
                        Value::Date(day.and_hms_opt(0, 0, 0).unwrap())
                    }
                    _ => unreachable!(),
                }
            };
            rows[i].push(value);
        }
        fields.push(field);
    }
    Ok(Outcome::Table(TableOutput { fields, rows }))
}
