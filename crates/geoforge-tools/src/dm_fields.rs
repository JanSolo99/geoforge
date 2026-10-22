//! Data Management field and table tools: Calculate Fields, Standardize Field, Transform Field,
//! Reclassify Field, Transpose Fields and Pivot Table.
//!
//! Matched to ArcGIS Pro 3.7 on shapefiles and dBASE tables (`testdata/arcgis-reference/manifest.json`,
//! cases `cf_*`, `std_*`, `tf_*`, `rf_*`, `transpose_*`, `pivot_*`):
//! - Calculate Fields runs Calculate Field per entry in order, so later expressions see earlier
//!   results; a field that does not exist is added (TEXT unless the entry names a type); an entry's
//!   where clause limits the rows.
//! - Standardize Field (Z-SCORE with the population standard deviation, MIN-MAX, MAXABS, ROBUST with
//!   the median and the interquartile range, linearly interpolated quartiles) and Transform Field
//!   add `STAND<n>` / `TRAN<n>` double fields (n: the field's place in the list), or write the
//!   named field.
//! - Reclassify Field adds `RECLASS` (the class) and `RECLASS_R` (its range as text) for numbers,
//!   one field for text; ranges read `lo - hi` with six decimals, a class starting 0.000001 above
//!   the end of the one before it.
//! - Transpose Fields gives a row per field and input row (field by field), with the kept fields,
//!   the field's alias (empty when none is given) and its value as text.
//! - Pivot Table sorts rows by the key fields, the pivot value and the value, and starts a new
//!   output row of a key when a pivot column already holds a value.

use geoforge_core::{Error, FeatureClass, Field, FieldType, Result, Value};

use crate::tables::Mem;
use crate::{TableOutput, ToolOutput, WhereClause, add_field, calculate_field};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

fn copy_of(input: &dyn FeatureClass) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| err("ERROR 000735: Input Table: Value is required"))?;
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features: input.features().collect::<Result<_>>()?,
        messages: Vec::new(),
    })
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

/// One entry of Calculate Fields.
#[derive(Debug, Clone)]
pub struct FieldCalculation {
    pub field: String,
    pub expression: String,
    pub where_clause: Option<String>,
    pub field_type: Option<String>,
}

impl FieldCalculation {
    /// `NAME=EXPRESSION[|WHERE[|TYPE]]`.
    pub fn parse(item: &str) -> Result<Self> {
        let (name, rest) = item
            .split_once('=')
            .ok_or_else(|| err(format!("'{item}': expected NAME=EXPRESSION[|WHERE[|TYPE]]")))?;
        let mut it = rest.rsplitn(3, '|').collect::<Vec<_>>();
        it.reverse();
        let (expression, where_clause, field_type) = match it.as_slice() {
            [e] => (*e, "", ""),
            [e, w] => (*e, *w, ""),
            [e, w, t] => (*e, *w, *t),
            _ => unreachable!(),
        };
        let non_empty = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_owned());
        Ok(Self {
            field: name.trim().to_owned(),
            expression: expression.trim().to_owned(),
            where_clause: non_empty(where_clause),
            field_type: non_empty(field_type),
        })
    }
}

/// `arcpy.management.CalculateFields(in_table, "PYTHON3", fields, code_block)`.
pub fn calculate_fields(
    input: &dyn FeatureClass,
    calculations: &[FieldCalculation],
    code_block: &str,
) -> Result<ToolOutput> {
    let mut out = copy_of(input)?;
    for c in calculations {
        if !out
            .fields
            .iter()
            .any(|f| f.name.eq_ignore_ascii_case(&c.field))
        {
            let ty = c.field_type.as_deref().unwrap_or("TEXT");
            let added = add_field(&Mem(out.clone()), &c.field, ty, None, None, None)?;
            out = added;
        }
        let column = out
            .fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(&c.field))
            .ok_or_else(|| err("field missing"))?;
        match &c.where_clause {
            None => {
                let done =
                    calculate_field(&Mem(out.clone()), &c.field, &c.expression, code_block, None)?;
                out.messages.extend(done.messages.iter().cloned());
                out.features = done.features;
            }
            Some(w) => {
                let clause = WhereClause::parse(w, &out.fields)?;
                let selected: Vec<usize> = out
                    .features
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| clause.matches(&f.attributes))
                    .map(|(i, _)| i)
                    .collect();
                let mut subset = out.clone();
                subset.features = selected.iter().map(|&i| out.features[i].clone()).collect();
                let done =
                    calculate_field(&Mem(subset), &c.field, &c.expression, code_block, None)?;
                for (k, &i) in selected.iter().enumerate() {
                    out.features[i].attributes[column] =
                        done.features[k].attributes[column].clone();
                }
            }
        }
    }
    Ok(out)
}

/// `NAME [OUTPUT]` items separated by semicolons.
pub fn parse_field_list(text: &str) -> Vec<(String, Option<String>)> {
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|item| {
            let mut w = item.split_whitespace();
            let name = w.next().unwrap_or_default().to_owned();
            (name, w.next().map(str::to_owned))
        })
        .collect()
}

/// The numeric column `name` of `out`, and its index.
fn numeric_column(out: &ToolOutput, name: &str) -> Result<(usize, Vec<f64>)> {
    let i = out
        .fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            err(format!(
                "ERROR 000728: Field {name} does not exist within table"
            ))
        })?;
    if !matches!(
        out.fields[i].field_type,
        FieldType::SmallInteger
            | FieldType::Integer
            | FieldType::BigInteger
            | FieldType::Single
            | FieldType::Double
    ) {
        return Err(err(
            "ERROR 003911: The field is not of type SHORT | LONG | FLOAT | DOUBLE | BIGINTEGER",
        ));
    }
    let values = out
        .features
        .iter()
        .map(|f| f.attributes.get(i).and_then(number).unwrap_or(f64::NAN))
        .collect();
    Ok((i, values))
}

/// The column named `name` (added as a double when missing), and its index.
fn output_column(out: &mut ToolOutput, name: &str, ty: &str) -> Result<usize> {
    if !out.fields.iter().any(|f| f.name.eq_ignore_ascii_case(name)) {
        let added = add_field(&Mem(out.clone()), name, ty, None, None, None)?;
        *out = added;
    }
    Ok(out
        .fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(name))
        .unwrap_or(0))
}

fn store(out: &mut ToolOutput, column: usize, values: &[f64]) {
    for (f, v) in out.features.iter_mut().zip(values) {
        f.attributes[column] = if v.is_nan() {
            Value::Null
        } else {
            match out.fields[column].field_type {
                FieldType::Single => Value::Single(*v as f32),
                _ => Value::Double(*v),
            }
        };
    }
}

/// A percentile with linear interpolation between ranks (the usual `(n - 1) p` rule).
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let pos = (sorted.len() - 1) as f64 * p;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// `arcpy.management.StandardizeField(in_table, fields, method, min_value, max_value)`.
pub fn standardize_field(
    input: &dyn FeatureClass,
    fields: &[(String, Option<String>)],
    method: &str,
    min_value: f64,
    max_value: f64,
) -> Result<ToolOutput> {
    let mut out = copy_of(input)?;
    for (n, (name, target)) in fields.iter().enumerate() {
        let (_, v) = numeric_column(&out, name)?;
        let count = v.len() as f64;
        let mean = v.iter().sum::<f64>() / count;
        let min = v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let result: Vec<f64> = match method.to_ascii_uppercase().as_str() {
            "Z-SCORE" | "" => {
                let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / count).sqrt();
                v.iter().map(|x| (x - mean) / sd).collect()
            }
            "MIN-MAX" => v
                .iter()
                .map(|x| (x - min) / (max - min) * (max_value - min_value) + min_value)
                .collect(),
            "MAXABS" => {
                let m = v.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
                v.iter().map(|x| x / m).collect()
            }
            "ROBUST" => {
                let mut s = v.clone();
                s.sort_by(f64::total_cmp);
                let (q1, med, q3) = (
                    percentile(&s, 0.25),
                    percentile(&s, 0.5),
                    percentile(&s, 0.75),
                );
                v.iter().map(|x| (x - med) / (q3 - q1)).collect()
            }
            other => {
                return Err(err(format!(
                    "ERROR 000067: method {other} is not supported"
                )));
            }
        };
        let name = target.clone().unwrap_or_else(|| format!("STAND{}", n + 1));
        let c = output_column(&mut out, &name, "DOUBLE")?;
        store(&mut out, c, &result);
    }
    Ok(out)
}

/// `arcpy.management.TransformField(in_table, fields, method, power, shift)`. A shift is added
/// before LOG, SQRT, INVX and BOX-COX and subtracted after the inverse transformations. LOG shifts
/// the data above zero by default (1e-6 above the smallest value when that is zero or negative);
/// a division by zero gives 0 with a warning.
pub fn transform_field(
    input: &dyn FeatureClass,
    fields: &[(String, Option<String>)],
    method: &str,
    power: Option<f64>,
    shift: Option<f64>,
) -> Result<ToolOutput> {
    let mut out = copy_of(input)?;
    let method = method.to_ascii_uppercase();
    let mut failed: Vec<String> = Vec::new();
    for (n, (name, target)) in fields.iter().enumerate() {
        let (_, v) = numeric_column(&out, name)?;
        let min = v.iter().copied().fold(f64::INFINITY, f64::min);
        let forward = matches!(method.as_str(), "LOG" | "SQRT" | "INVX" | "BOX-COX");
        let default_shift = match method.as_str() {
            "LOG" if min <= 0.0 => 1e-6 - min,
            "SQRT" | "BOX-COX" if min < 0.0 => -min,
            _ => 0.0,
        };
        let s = shift.unwrap_or(0.0);
        let s = if forward && shift.is_none_or(|s| s == 0.0) {
            default_shift
        } else {
            s
        };
        let lambda = power;
        let mut bad = false;
        let result: Vec<f64> = v
            .iter()
            .map(|&x| {
                let y = match method.as_str() {
                    "LOG" => (x + s).ln(),
                    "SQRT" => (x + s).sqrt(),
                    "INVX" => 1.0 / (x + s),
                    "BOX-COX" => match lambda {
                        Some(l) if l != 0.0 => ((x + s).powf(l) - 1.0) / l,
                        Some(_) => (x + s).ln(),
                        None => f64::NAN,
                    },
                    "INV_LOG" => x.exp() - s,
                    "INV_SQRT" => x * x - s,
                    "INV_BOX-COX" => match lambda {
                        Some(l) if l != 0.0 => (l * x + 1.0).powf(1.0 / l) - s,
                        Some(_) => x.exp() - s,
                        None => f64::NAN,
                    },
                    _ => f64::NAN,
                };
                if y.is_finite() {
                    y
                } else {
                    bad = true;
                    0.0
                }
            })
            .collect();
        if method == "BOX-COX" && lambda.is_none() {
            return Err(err(
                "ERROR 000735: Power: a value is required (the maximum likelihood estimate is not supported)",
            ));
        }
        if method == "INV_BOX-COX" && lambda.is_none() {
            return Err(err("ERROR 000735: Power: Value is required"));
        }
        if !matches!(
            method.as_str(),
            "LOG" | "SQRT" | "INVX" | "BOX-COX" | "INV_LOG" | "INV_SQRT" | "INV_BOX-COX"
        ) {
            return Err(err(format!(
                "ERROR 000067: method {method} is not supported"
            )));
        }
        if bad {
            failed.push(name.clone());
        }
        let name = target.clone().unwrap_or_else(|| format!("TRAN{}", n + 1));
        let c = output_column(&mut out, &name, "DOUBLE")?;
        store(&mut out, c, &result);
    }
    if !failed.is_empty() {
        out.messages.push(format!(
            "WARNING 110344: The transformation could not be calculated in some rows in the field(s): {}",
            failed.join(", ")
        ));
    }
    Ok(out)
}

/// How Reclassify Field forms its classes.
#[derive(Debug, Clone)]
pub enum Reclass {
    EqualInterval(usize),
    DefinedInterval(f64),
    Quantile(usize),
    /// Upper bound and value of each class.
    Manual(Vec<(f64, i32)>),
    UniqueValues,
}

/// `arcpy.management.ReclassifyField(in_table, field, method, ..., reverse_values, output_field_name)`.
pub fn reclassify_field(
    input: &dyn FeatureClass,
    field: &str,
    method: &Reclass,
    descending: bool,
    output_name: Option<&str>,
) -> Result<ToolOutput> {
    let mut out = copy_of(input)?;
    let column = out
        .fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(field))
        .ok_or_else(|| {
            err(format!(
                "ERROR 000728: Field {field} does not exist within table"
            ))
        })?;
    if out.fields[column].field_type == FieldType::String {
        if !matches!(method, Reclass::UniqueValues) {
            return Err(err(
                "ERROR 003911: the field must be numeric for this method",
            ));
        }
        let texts: Vec<String> = out
            .features
            .iter()
            .map(|f| match &f.attributes[column] {
                Value::String(s) => s.clone(),
                _ => String::new(),
            })
            .collect();
        let mut unique = texts.clone();
        unique.sort();
        unique.dedup();
        let name = output_name.unwrap_or("RECLASS");
        let c = output_column(&mut out, name, "LONG")?;
        for (f, t) in out.features.iter_mut().zip(&texts) {
            let rank = unique.iter().position(|u| u == t).unwrap_or(0);
            let class = if descending {
                unique.len() - rank
            } else {
                rank + 1
            };
            f.attributes[c] = Value::Integer(class as i32);
        }
        return Ok(out);
    }
    let (_, v) = numeric_column(&out, field)?;
    let min = v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // Upper bounds of the classes, ascending; the class values in order.
    let (bounds, values, label_length): (Vec<f64>, Vec<i32>, u32) = match method {
        Reclass::EqualInterval(n) => {
            let w = (max - min) / *n as f64;
            let mut b: Vec<f64> = (1..*n).map(|k| min + w * k as f64).collect();
            b.push(max);
            (b, (1..=*n as i32).collect(), 100)
        }
        Reclass::DefinedInterval(step) => {
            let last = (max / step).ceil().max(1.0) as usize;
            let b: Vec<f64> = (1..=last).map(|k| step * k as f64).collect();
            let first = (min / step).ceil().max(1.0) as i32;
            let n = b.len();
            // Classes below the one holding the smallest value do not exist; numbering starts at 1
            // for the interval from zero.
            let _ = first;
            (b, (1..=n as i32).collect(), 104)
        }
        Reclass::Quantile(n) => {
            let mut s = v.clone();
            s.sort_by(f64::total_cmp);
            let count = s.len() as f64;
            let mut b: Vec<f64> = (1..=*n)
                .map(|k| {
                    let rank = (k as f64 * count / *n as f64).ceil() as usize;
                    s[rank.clamp(1, s.len()) - 1]
                })
                .collect();
            b.dedup();
            let m = b.len() as i32;
            (b, (1..=m).collect(), 100)
        }
        Reclass::Manual(table) => (
            table.iter().map(|t| t.0).collect(),
            table.iter().map(|t| t.1).collect(),
            104,
        ),
        Reclass::UniqueValues => {
            return Err(err("ERROR 000530: Classes: Value is required"));
        }
    };
    let class_of = |x: f64| {
        bounds
            .iter()
            .position(|&b| x <= b)
            .unwrap_or(bounds.len() - 1)
    };
    let label = |k: usize| {
        let lo = if k == 0 {
            min
        } else {
            bounds[k - 1] + 0.000001
        };
        format!("{lo:.6} - {:.6}", bounds[k])
    };
    let n = bounds.len();
    let name = output_name.unwrap_or("RECLASS");
    let c = output_column(&mut out, name, "LONG")?;
    let rname = format!("{name}_R");
    let r = output_column(&mut out, &rname, "TEXT")?;
    out.fields[r].length = label_length;
    for (f, &x) in out.features.iter_mut().zip(&v) {
        let k = class_of(x);
        let class = if descending {
            values[n - 1 - k]
        } else {
            values[k]
        };
        f.attributes[c] = Value::Integer(class);
        f.attributes[r] = Value::String(label(k));
    }
    Ok(out)
}

/// A value as ArcGIS writes it into a text field: whole numbers without a fraction.
fn value_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Double(d) if d.fract() == 0.0 && d.abs() < 1e15 => format!("{}", *d as i64),
        Value::Double(d) => format!("{d}"),
        Value::Single(d) if d.fract() == 0.0 && d.abs() < 1e15 => format!("{}", *d as i64),
        Value::Single(d) => format!("{d}"),
        Value::SmallInteger(n) => n.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::BigInteger(n) => n.to_string(),
        Value::String(s) | Value::Guid(s) | Value::Xml(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

fn text_field(name: &str) -> Field {
    let mut f = Field::new(name, FieldType::String);
    f.length = 254;
    f
}

/// `arcpy.management.TransposeFields(in_table, in_field, out_table, in_transposed_field_name,
/// in_value_field_name, attribute_fields)`: `fields` are the field names with their aliases.
pub fn transpose_fields(
    input: &dyn FeatureClass,
    fields: &[(String, Option<String>)],
    transposed_name: &str,
    value_name: &str,
    attribute_fields: &[String],
) -> Result<TableOutput> {
    let find = |n: &str| {
        input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(n))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {n} does not exist within table"
                ))
            })
    };
    let attr: Vec<usize> = attribute_fields
        .iter()
        .map(|n| find(n))
        .collect::<Result<_>>()?;
    let sources: Vec<(usize, String)> = fields
        .iter()
        .map(|(n, a)| find(n).map(|i| (i, a.clone().unwrap_or_default())))
        .collect::<Result<_>>()?;
    let rows: Vec<Vec<Value>> = input
        .features()
        .map(|f| f.map(|f| f.attributes))
        .collect::<Result<_>>()?;
    let mut out_fields: Vec<Field> = attr.iter().map(|&i| input.fields()[i].clone()).collect();
    out_fields.push(text_field(transposed_name));
    out_fields.push(text_field(value_name));
    let mut out_rows = Vec::new();
    for (i, alias) in &sources {
        for r in &rows {
            let mut row: Vec<Value> = attr.iter().map(|&a| r[a].clone()).collect();
            row.push(Value::String(alias.clone()));
            row.push(Value::String(value_text(&r[*i])));
            out_rows.push(row);
        }
    }
    Ok(TableOutput {
        fields: out_fields,
        rows: out_rows,
    })
}

fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        _ => value_text(a).cmp(&value_text(b)),
    }
}

/// `arcpy.management.PivotTable(in_table, fields, pivot_field, value_field, out_table)`.
pub fn pivot_table(
    input: &dyn FeatureClass,
    key_fields: &[String],
    pivot_field: &str,
    value_field: &str,
) -> Result<TableOutput> {
    let find = |n: &str| {
        input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(n))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {n} does not exist within table"
                ))
            })
    };
    let keys: Vec<usize> = key_fields.iter().map(|n| find(n)).collect::<Result<_>>()?;
    let (pivot, value) = (find(pivot_field)?, find(value_field)?);
    let mut rows: Vec<Vec<Value>> = input
        .features()
        .map(|f| f.map(|f| f.attributes))
        .collect::<Result<_>>()?;
    rows.sort_by(|a, b| {
        keys.iter()
            .map(|&k| compare_values(&a[k], &b[k]))
            .chain([
                compare_values(&a[pivot], &b[pivot]),
                compare_values(&a[value], &b[value]),
            ])
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // The pivot columns, in order of their values.
    let numeric_pivot = number(&rows.first().map_or(Value::Null, |r| r[pivot].clone())).is_some();
    let mut labels: Vec<(Value, String)> = Vec::new();
    for r in &rows {
        if !labels
            .iter()
            .any(|l| compare_values(&l.0, &r[pivot]).is_eq())
        {
            let text = value_text(&r[pivot]);
            labels.push((
                r[pivot].clone(),
                if numeric_pivot {
                    format!("{pivot_field}{text}")
                } else {
                    text
                },
            ));
        }
    }
    labels.sort_by(|a, b| compare_values(&a.0, &b.0));
    let value_type = input.fields()[value].field_type;
    let mut out_fields: Vec<Field> = keys.iter().map(|&k| input.fields()[k].clone()).collect();
    for (_, label) in &labels {
        let mut f = Field::new(label.as_str(), value_type);
        if value_type == FieldType::Double {
            f.length = 19;
            f.precision = 18;
            f.scale = 15;
        } else {
            f.length = input.fields()[value].length;
            f.precision = input.fields()[value].precision;
        }
        out_fields.push(f);
    }
    let zero = |t: FieldType| match t {
        FieldType::SmallInteger => Value::SmallInteger(0),
        FieldType::Integer => Value::Integer(0),
        FieldType::BigInteger => Value::BigInteger(0),
        FieldType::Single => Value::Single(0.0),
        _ => Value::Double(0.0),
    };
    let mut out_rows: Vec<Vec<Value>> = Vec::new();
    let mut filled: Vec<bool> = Vec::new();
    let mut last_key: Option<Vec<Value>> = None;
    for r in &rows {
        let key: Vec<Value> = keys.iter().map(|&k| r[k].clone()).collect();
        let col = labels
            .iter()
            .position(|l| compare_values(&l.0, &r[pivot]).is_eq())
            .unwrap_or(0);
        let same = last_key.as_ref().is_some_and(|k| {
            k.iter()
                .zip(&key)
                .all(|(a, b)| compare_values(a, b).is_eq())
        });
        let need_new = !same || filled.get(col).copied().unwrap_or(false);
        if need_new {
            let mut row = key.clone();
            row.extend(labels.iter().map(|_| zero(value_type)));
            out_rows.push(row);
            filled = vec![false; labels.len()];
        }
        last_key = Some(key);
        let row = out_rows.last_mut().expect("a row was pushed");
        row[keys.len() + col] = r[value].clone();
        filled[col] = true;
    }
    Ok(TableOutput {
        fields: out_fields,
        rows: out_rows,
    })
}

fn time_key(v: &Value) -> Option<chrono::NaiveDateTime> {
    match v {
        Value::Date(d) => Some(*d),
        Value::DateOnly(d) => d.and_hms_opt(0, 0, 0),
        _ => None,
    }
}

/// `arcpy.management.CalculateEndTime(in_table, start_field, end_field, fields)`: a row ends
/// where the next row (by start time, then input order, among those with equal `fields`) starts;
/// the last row of a group ends where it starts. A row without a start sorts first.
pub fn calculate_end_time(
    input: &dyn FeatureClass,
    start_field: &str,
    end_field: &str,
    group_fields: &[String],
) -> Result<ToolOutput> {
    let mut out = copy_of(input)?;
    let find = |out: &ToolOutput, n: &str| {
        out.fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(n))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {n} does not exist within table"
                ))
            })
    };
    let (start, end) = (find(&out, start_field)?, find(&out, end_field)?);
    let groups: Vec<usize> = group_fields
        .iter()
        .map(|n| find(&out, n))
        .collect::<Result<_>>()?;
    let key = |f: &geoforge_core::Feature| -> Vec<String> {
        groups
            .iter()
            .map(|&g| value_text(&f.attributes[g]))
            .collect()
    };
    let mut order: Vec<usize> = (0..out.features.len()).collect();
    order.sort_by(|&a, &b| {
        let (fa, fb) = (&out.features[a], &out.features[b]);
        key(fa)
            .cmp(&key(fb))
            .then(time_key(&fa.attributes[start]).cmp(&time_key(&fb.attributes[start])))
            .then(a.cmp(&b))
    });
    let mut ends: Vec<(usize, Value)> = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let next = order
            .get(k + 1)
            .filter(|&&j| key(&out.features[j]) == key(&out.features[i]));
        let source = match next {
            Some(&j) => &out.features[j].attributes[start],
            None => &out.features[i].attributes[start],
        };
        ends.push((i, source.clone()));
    }
    for (i, v) in ends {
        out.features[i].attributes[end] = v;
    }
    Ok(out)
}
