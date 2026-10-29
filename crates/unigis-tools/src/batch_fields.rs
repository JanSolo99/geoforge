//! Batch Update Fields (Data Management): a copy of a table or feature class with the fields a
//! definition table lists.

use std::collections::HashMap;

use unigis_core::{Error, FeatureClass, Field, FieldType, Result, Value};

use crate::{ToolOutput, pycalc};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// One row of the field definition table.
#[derive(Debug, Clone, Default)]
pub struct FieldDefinition {
    pub name: String,
    pub source: Option<String>,
    /// `TEXT`, `SHORT`, `LONG`, `FLOAT`, `DOUBLE`, `DATE`, `BIGINTEGER`; `None` keeps the source
    /// field's type.
    pub field_type: Option<String>,
    /// Text length, or decimals of a FLOAT/DOUBLE field.
    pub size: Option<u32>,
    pub alias: Option<String>,
    /// A Python expression over the output fields (`!PID! * 10`).
    pub script: Option<String>,
}

/// The output field of `def`, from the source field when the type is not given (observed
/// against ArcGIS Pro 3.7 shapefile output: SHORT width 5, LONG 10, FLOAT/DOUBLE 19 wide with the
/// given decimals, TEXT the given length or the source field's width).
fn output_field(def: &FieldDefinition, source: Option<&Field>) -> Result<Field> {
    let kind = def.field_type.as_deref().map(str::to_ascii_uppercase);
    let mut f = match (kind.as_deref(), source) {
        (None | Some(""), Some(s)) => {
            let mut f = s.clone();
            f.name = def.name.clone();
            f.alias = None;
            f.domain = None;
            f.default = None;
            f
        }
        (None | Some(""), None) => {
            return Err(err(format!(
                "ERROR 080466: Failed to change the data type for target field '{}'",
                def.name
            )));
        }
        (Some(t), _) => {
            let (field_type, length, precision, scale) = match t {
                "SHORT" => (FieldType::SmallInteger, 5, 5, 0),
                "LONG" => (FieldType::Integer, 10, 10, 0),
                "BIGINTEGER" => (FieldType::BigInteger, 19, 19, 0),
                "FLOAT" | "DOUBLE" => match def.size {
                    Some(d) => (FieldType::Double, 19, 18, d),
                    None => (FieldType::Double, 19, 19, 0),
                },
                "TEXT" => (
                    FieldType::String,
                    def.size
                        .or(source.map(|s| s.length).filter(|&l| l > 0))
                        .unwrap_or(255),
                    0,
                    0,
                ),
                "DATE" => (FieldType::Date, 8, 0, 0),
                other => return Err(err(format!("unsupported field type {other}"))),
            };
            let mut f = Field::new(def.name.clone(), field_type);
            (f.length, f.precision, f.scale) = (length, precision, scale);
            f
        }
    };
    f.alias = def.alias.clone().filter(|a| !a.is_empty());
    Ok(f)
}

/// `v` rounded half away from zero to the field's decimals, as ArcGIS stores it (1823.35 at one
/// decimal is 1823.4); later scripts see the stored value.
fn round_to_scale(v: f64, field: &Field) -> f64 {
    if field.precision == 0 {
        return v; // no fixed decimals (geodatabase-style double)
    }
    let k = 10_f64.powi(field.scale as i32);
    (v * k).round() / k
}

/// `value` converted to `field`'s type; `None` when it cannot be (text that is not a number).
fn convert(value: &Value, field: &Field) -> Option<Value> {
    if value.is_null() {
        return Some(Value::Null);
    }
    let text = match value {
        Value::String(s) => Some(s.trim().to_owned()),
        Value::SmallInteger(n) => Some(n.to_string()),
        Value::Integer(n) => Some(n.to_string()),
        Value::BigInteger(n) => Some(n.to_string()),
        Value::Single(n) => Some(n.to_string()),
        Value::Double(n) => Some(n.to_string()),
        Value::Date(d) => Some(d.format("%Y-%m-%d %H:%M:%S").to_string()),
        _ => None,
    };
    let number = crate::number(value).or_else(|| text.as_deref()?.parse::<f64>().ok());
    Some(match field.field_type {
        FieldType::String => {
            let t = text?;
            Value::String(if field.length > 0 {
                t.chars().take(field.length as usize).collect()
            } else {
                t
            })
        }
        FieldType::SmallInteger => Value::SmallInteger(number?.round() as i16),
        FieldType::Integer => Value::Integer(number?.round() as i32),
        FieldType::BigInteger => Value::BigInteger(number?.round() as i64),
        FieldType::Single => Value::Single(number? as f32),
        FieldType::Double => Value::Double(round_to_scale(number?, field)),
        FieldType::Date => match value {
            Value::Date(_) => value.clone(),
            _ => return None,
        },
        _ => value.clone(),
    })
}

/// Batch Update Fields: the output has the defined fields only, in definition order, each from
/// its source field (converted to the defined type) or from a script over other output fields,
/// evaluated in dependency order. A script naming a missing output field fails (ERROR 080469),
/// as does a cycle including a self reference (ERROR 080464); a value that cannot take the new
/// type fails (ERROR 080466).
pub fn batch_update_fields(
    input: &dyn FeatureClass,
    defs: &[FieldDefinition],
) -> Result<ToolOutput> {
    let in_fields = input.fields();
    let find = |n: &str| {
        in_fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(n))
    };
    let mut sources = Vec::new();
    let mut fields = Vec::new();
    for d in defs {
        let src = match d.source.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(s) => Some(
                find(s.trim())
                    .ok_or_else(|| err(format!("ERROR 000728: Field {s} does not exist")))?,
            ),
            None => None,
        };
        fields.push(output_field(d, src.map(|i| &in_fields[i]))?);
        sources.push(src);
    }
    let index = |n: &str| {
        fields
            .iter()
            .position(|f: &Field| f.name.eq_ignore_ascii_case(n))
    };
    // Script dependencies on output fields, then an evaluation order.
    let mut deps: Vec<Vec<usize>> = Vec::new();
    for d in defs {
        let mut v = Vec::new();
        if let Some(s) = d.script.as_deref().filter(|s| !s.trim().is_empty()) {
            let missing: Vec<String> = pycalc::field_tokens(s)
                .into_iter()
                .filter(|t| {
                    let found = index(t);
                    if let Some(i) = found {
                        v.push(i);
                    }
                    found.is_none()
                })
                .collect();
            if !missing.is_empty() {
                return Err(err(format!(
                    "ERROR 080469: '{}' has a reference to a missing field(s): {}",
                    d.name,
                    missing.join(", ")
                )));
            }
        }
        deps.push(v);
    }
    let mut order = Vec::new();
    let mut state = vec![0u8; defs.len()]; // 0 new, 1 visiting, 2 done
    fn visit(i: usize, deps: &[Vec<usize>], state: &mut [u8], order: &mut Vec<usize>) -> bool {
        match state[i] {
            2 => true,
            1 => false,
            _ => {
                state[i] = 1;
                for &j in &deps[i] {
                    if !visit(j, deps, state, order) {
                        return false;
                    }
                }
                state[i] = 2;
                order.push(i);
                true
            }
        }
    }
    for (i, d) in defs.iter().enumerate() {
        if !visit(i, &deps, &mut state, &mut order) {
            return Err(err(format!(
                "ERROR 080464: Cyclic reference detected for Target Field '{}'.",
                d.name
            )));
        }
    }
    let interps: Vec<Option<pycalc::Interp>> = defs
        .iter()
        .map(|d| {
            d.script
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|_| pycalc::Interp::new(""))
                .transpose()
        })
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| err(format!("ERROR 000539: {e}")))?;
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        let mut row = vec![Value::Null; defs.len()];
        for &i in &order {
            let d = &defs[i];
            row[i] = match &interps[i] {
                Some(interp) => {
                    let mut vars = HashMap::new();
                    for &j in &deps[i] {
                        vars.insert(fields[j].name.to_uppercase(), crate::to_py(&row[j]));
                    }
                    let v = interp
                        .eval(d.script.as_deref().unwrap_or_default(), &vars)
                        .map_err(|e| err(format!("ERROR 000539: {e}")))?;
                    match crate::from_py(&v, &fields[i]).unwrap_or(Value::Null) {
                        Value::Double(x) => Value::Double(round_to_scale(x, &fields[i])),
                        other => other,
                    }
                }
                None => match sources[i] {
                    Some(s) => convert(&f.attributes[s], &fields[i]).ok_or_else(|| {
                        err(format!(
                            "ERROR 080466: Failed to change the data type for target field '{}'.",
                            d.name
                        ))
                    })?,
                    None => Value::Null,
                },
            };
        }
        f.attributes = row;
        features.push(f);
    }
    Ok(ToolOutput {
        kind: input
            .geometry_kind()
            .unwrap_or(unigis_core::GeometryKind::Point),
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}
