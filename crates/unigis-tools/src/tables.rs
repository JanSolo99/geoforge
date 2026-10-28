//! Table tools: Copy Rows, Table To Table, Join Field, Add Fields and Get Count.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/tables.json`):
//! - Copy Rows and Table To Table keep the attribute fields in order (the shape goes); Table To
//!   Table's where clause keeps the rows it selects.
//! - Join Field appends the join table's fields (all of them when none are named, the join field
//!   and the ObjectID aside only when named), renamed `_1`, `_2`, ... when the name is taken. A row
//!   takes the first join row with an equal key (numbers by value, text trimmed); a row with no
//!   match gets nulls (0 and empty text in a shapefile).
//! - Add Fields adds the listed fields one by one as Add Field does.

use unigis_core::{Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value};

use crate::{TableOutput, ToolOutput, WhereClause, add_field, push_unique};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// The rows of a table or feature class, as the attribute fields.
fn rows(input: &dyn FeatureClass, where_clause: Option<&str>) -> Result<TableOutput> {
    let clause = where_clause
        .filter(|w| !w.trim().is_empty())
        .map(|w| WhereClause::parse(w, input.fields()))
        .transpose()?;
    let mut out = Vec::new();
    for f in input.features() {
        let f = f?;
        if clause.as_ref().is_none_or(|c| c.matches(&f.attributes)) {
            out.push(f.attributes);
        }
    }
    Ok(TableOutput {
        fields: input.fields().to_vec(),
        rows: out,
    })
}

/// Copy Rows (no clause) and Table To Table (`where_clause`).
pub fn copy_rows(input: &dyn FeatureClass, where_clause: Option<&str>) -> Result<TableOutput> {
    rows(input, where_clause)
}

/// The number of rows (that a where clause selects).
pub fn get_count(input: &dyn FeatureClass, where_clause: Option<&str>) -> Result<usize> {
    Ok(rows(input, where_clause)?.rows.len())
}

/// A key that compares like ArcGIS's join: numbers by value, text trimmed.
fn key(v: &Value) -> Option<String> {
    Some(match v {
        Value::Null => return None,
        Value::SmallInteger(n) => format!("n{}", *n as f64),
        Value::Integer(n) => format!("n{}", *n as f64),
        Value::BigInteger(n) => format!("n{}", *n as f64),
        Value::Single(n) => format!("n{}", *n as f64),
        Value::Double(n) => format!("n{n}"),
        Value::String(s) => format!("s{}", s.trim()),
        other => format!("o{other:?}"),
    })
}

/// What Join Field gives: features with the joined fields, or a table.
pub enum Joined {
    Features(ToolOutput),
    Table(TableOutput),
}

/// Join Field. `fields` empty joins every field of the join table.
pub fn join_field(
    input: &dyn FeatureClass,
    in_field: &str,
    join: &dyn FeatureClass,
    join_field: &str,
    fields: &[String],
) -> Result<Joined> {
    let find = |fc: &dyn FeatureClass, name: &str| {
        fc.fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {name} does not exist within table"
                ))
            })
    };
    let ki = find(input, in_field)?;
    let kj = find(join, join_field)?;
    let wanted: Vec<usize> = if fields.is_empty() {
        (0..join.fields().len()).collect()
    } else {
        fields
            .iter()
            .map(|n| find(join, n))
            .collect::<Result<_>>()?
    };
    let usable = |i: &usize| {
        !matches!(
            join.fields()[*i].field_type,
            FieldType::ObjectId | FieldType::Geometry | FieldType::GlobalId
        )
    };
    let wanted: Vec<usize> = wanted.into_iter().filter(usable).collect();
    // The first join row of each key.
    let mut first: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
    for f in join.features() {
        let f = f?;
        if let Some(k) = f.attributes.get(kj).and_then(key) {
            first.entry(k).or_insert(f.attributes);
        }
    }
    let mut out_fields = input.fields().to_vec();
    for &i in &wanted {
        let mut f = join.fields()[i].clone();
        f.domain = None;
        push_unique(&mut out_fields, f);
    }
    let extend = |attrs: &mut Vec<Value>| {
        let row = attrs.get(ki).and_then(key).and_then(|k| first.get(&k));
        for &i in &wanted {
            attrs.push(row.and_then(|r| r.get(i).cloned()).unwrap_or(Value::Null));
        }
    };
    match input.geometry_kind() {
        Some(kind) => {
            let mut features: Vec<Feature> = input.features().collect::<Result<_>>()?;
            for f in &mut features {
                extend(&mut f.attributes);
            }
            Ok(Joined::Features(ToolOutput {
                kind,
                has_z: input.has_z(),
                has_m: input.has_m(),
                fields: out_fields,
                spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
                features,
                messages: Vec::new(),
            }))
        }
        None => {
            let mut table = rows(input, None)?;
            for r in &mut table.rows {
                extend(r);
            }
            table.fields = out_fields;
            Ok(Joined::Table(table))
        }
    }
}

/// A feature class held in memory, so tools can be chained.
pub(crate) struct Mem(pub(crate) ToolOutput);

impl FeatureClass for Mem {
    fn geometry_kind(&self) -> Option<GeometryKind> {
        Some(self.0.kind)
    }
    fn has_z(&self) -> bool {
        self.0.has_z
    }
    fn has_m(&self) -> bool {
        self.0.has_m
    }
    fn fields(&self) -> &[Field] {
        &self.0.fields
    }
    fn oid_field_name(&self) -> &str {
        "FID"
    }
    fn shape_field_name(&self) -> &str {
        "Shape"
    }
    fn spatial_reference_wkt(&self) -> Option<&str> {
        self.0.spatial_reference_wkt.as_deref()
    }
    fn stored_extent(&self) -> Option<unigis_core::Envelope> {
        None
    }
    fn features(&self) -> Box<dyn Iterator<Item = Result<Feature>> + '_> {
        Box::new(self.0.features.iter().cloned().map(Ok))
    }
}

/// Add Fields from `NAME TYPE [LENGTH]` items separated by semicolons.
pub fn add_fields(input: &dyn FeatureClass, spec: &str) -> Result<ToolOutput> {
    let items: Vec<&str> = spec
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if items.is_empty() {
        return Err(err("ERROR 000735: Field Description: value is required"));
    }
    let mut current: Option<ToolOutput> = None;
    let mut messages = Vec::new();
    for item in items {
        let w: Vec<&str> = item.split_whitespace().collect();
        let (Some(name), Some(ty)) = (w.first(), w.get(1)) else {
            return Err(err(format!("'{item}': expected NAME TYPE [LENGTH]")));
        };
        let length = w
            .get(2)
            .map(|l| {
                l.parse::<u32>()
                    .map_err(|_| err(format!("'{item}': the length is a number")))
            })
            .transpose()?;
        let next = match &current {
            None => add_field(input, name, ty, None, None, length)?,
            Some(c) => add_field(&Mem(c.clone()), name, ty, None, None, length)?,
        };
        messages.extend(next.messages.iter().cloned());
        current = Some(next);
    }
    let mut out = current.ok_or_else(|| err("no fields"))?;
    out.messages = messages;
    Ok(out)
}

/// Encode Field (One-hot / One-cold): one integer field per distinct value of a text or integer
/// field, named `OH_` / `OC_` plus the value (characters other than letters, digits and `_`
/// become `_`), ordered by value (text bytewise, numbers numerically). One-hot is 1 on the rows
/// that have the value, One-cold the reverse; rows with a null keep nulls (observed, ArcGIS Pro 3.7).
pub fn encode_field(input: &dyn FeatureClass, field: &str, method: &str) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| err("input has no geometry"))?;
    let one_hot = match method
        .to_ascii_uppercase()
        .replace(['-', '_', ' '], "")
        .as_str()
    {
        "ONEHOT" => true,
        "ONECOLD" => false,
        other => return Err(err(format!("encoding method {other}"))),
    };
    let fields = input.fields().to_vec();
    let fi = fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(field))
        .ok_or_else(|| err(format!("ERROR 000728: Field {field} does not exist")))?;
    if !matches!(
        fields[fi].field_type,
        FieldType::String | FieldType::SmallInteger | FieldType::Integer | FieldType::BigInteger
    ) {
        return Err(err(
            "ERROR 003911: The field is not of type SHORT | TEXT | LONG | BIGINTEGER",
        ));
    }
    let mut rows: Vec<Feature> = Vec::new();
    for f in input.features() {
        rows.push(f?);
    }
    let mut cats: Vec<Value> = Vec::new();
    for r in &rows {
        let v = &r.attributes[fi];
        if !matches!(v, Value::Null) && !cats.contains(v) {
            cats.push(v.clone());
        }
    }
    cats.sort_by(|a, b| match (a, b) {
        (Value::String(x), Value::String(y)) => x.as_bytes().cmp(y.as_bytes()),
        _ => crate::number(a)
            .partial_cmp(&crate::number(b))
            .unwrap_or(std::cmp::Ordering::Equal),
    });
    let prefix = if one_hot { "OH_" } else { "OC_" };
    let mut out_fields = fields.clone();
    for c in &cats {
        let text = match c {
            Value::String(s) => s.clone(),
            other => crate::number(other).map_or_else(String::new, |n| format!("{n}")),
        };
        let mut name: String = prefix.to_owned()
            + &text
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
        let mut k = 0;
        while out_fields
            .iter()
            .any(|f| f.name.eq_ignore_ascii_case(&name))
        {
            k += 1;
            name = format!("{prefix}{}_{k}", name.trim_start_matches(prefix));
        }
        out_fields.push(Field::new(name, FieldType::Integer));
    }
    let features = rows
        .into_iter()
        .map(|mut r| {
            let v = r.attributes[fi].clone();
            for c in &cats {
                r.attributes.push(if matches!(v, Value::Null) {
                    Value::Null
                } else {
                    Value::Integer(i32::from((&v == c) == one_hot))
                });
            }
            r
        })
        .collect();
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: out_fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Field Statistics To Table (one field type per run, ungrouped): a row per field with its null
/// count, count, unique values, mode and least common value, and for numeric fields the usual
/// moments and quartiles. Sample standard deviation, population skewness and (non-excess)
/// kurtosis, quartiles by linear interpolation at `(n-1)p`, outliers beyond 1.5 IQR (observed,
/// ArcGIS Pro 3.7).
pub fn field_statistics_to_table(
    input: &dyn FeatureClass,
    field_names: &[String],
    numeric: bool,
) -> Result<TableOutput> {
    let fields = input.fields().to_vec();
    let mut picked = Vec::new();
    for n in field_names {
        let i = fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(n))
            .ok_or_else(|| err(format!("ERROR 000728: Field {n} does not exist")))?;
        let is_numeric = matches!(
            fields[i].field_type,
            FieldType::SmallInteger
                | FieldType::Integer
                | FieldType::BigInteger
                | FieldType::Single
                | FieldType::Double
        );
        if is_numeric == numeric && (is_numeric || fields[i].field_type == FieldType::String) {
            picked.push(i);
        }
    }
    let mut rows_in: Vec<Vec<Value>> = Vec::new();
    for f in input.features() {
        rows_in.push(f?.attributes);
    }
    let mut out_rows: Vec<Vec<Value>> = Vec::new();
    for &i in &picked {
        let f = &fields[i];
        let alias = f.alias.clone().unwrap_or_else(|| f.name.clone());
        let type_name = match f.field_type {
            FieldType::SmallInteger => "Short".to_owned(),
            FieldType::Integer => "Long".to_owned(),
            FieldType::BigInteger => "BigInteger".to_owned(),
            FieldType::Single => "Float".to_owned(),
            FieldType::Double => "Double".to_owned(),
            _ => format!("Text ({})", f.length),
        };
        let nulls = rows_in
            .iter()
            .filter(|r| matches!(r[i], Value::Null))
            .count();
        let mut row = vec![
            Value::String(f.name.clone()),
            Value::String(alias),
            Value::String(type_name),
            Value::Integer(nulls as i32),
        ];
        if numeric {
            let mut v: Vec<f64> = rows_in
                .iter()
                .filter_map(|r| crate::number(&r[i]))
                .collect();
            v.sort_by(f64::total_cmp);
            let n = v.len();
            let (counts, mode, least) =
                value_counts(&v.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
            let nf = n as f64;
            let sum: f64 = v.iter().sum();
            let mean = sum / nf;
            let q = |p: f64| {
                let pos = (nf - 1.0) * p;
                let lo = pos.floor() as usize;
                let hi = (lo + 1).min(n - 1);
                v[lo] + (v[hi] - v[lo]) * (pos - lo as f64)
            };
            let moment = |k: i32| v.iter().map(|x| (x - mean).powi(k)).sum::<f64>() / nf;
            let (q1, med, q3) = if n > 0 {
                (q(0.25), q(0.5), q(0.75))
            } else {
                (0.0, 0.0, 0.0)
            };
            let iqr = q3 - q1;
            let sd = (n > 1).then(|| (moment(2) * nf / (nf - 1.0)).sqrt());
            let m2 = moment(2);
            let opt = |x: Option<f64>| {
                x.filter(|y| y.is_finite())
                    .map_or(Value::Null, Value::Double)
            };
            let outliers = v
                .iter()
                .filter(|&&x| x < q1 - 1.5 * iqr || x > q3 + 1.5 * iqr)
                .count();
            let bits = |b: Option<u64>| opt(b.map(f64::from_bits));
            row.extend([
                opt(v.first().copied()),
                opt(v.last().copied()),
                opt((n > 0).then_some(mean)),
                opt(sd),
                opt((n > 0).then_some(med)),
                Value::Integer(n as i32),
                Value::Integer(counts as i32),
                bits(mode),
                bits(least),
                Value::Integer(outliers as i32),
                opt((n > 0).then_some(sum)),
                opt(v.first().zip(v.last()).map(|(a, b)| b - a)),
                opt((n > 0).then_some(iqr)),
                opt((n > 0).then_some(q1)),
                opt((n > 0).then_some(q3)),
                opt(sd.map(|s| s / mean)),
                opt((n > 2 && m2 > 0.0).then(|| moment(3) / m2.powf(1.5))),
                opt((n > 2 && m2 > 0.0).then(|| moment(4) / (m2 * m2))),
            ]);
        } else {
            let strings: Vec<&str> = rows_in
                .iter()
                .filter_map(|r| match &r[i] {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect();
            let mut ids: Vec<usize> = Vec::new();
            let mut uniq: Vec<&str> = Vec::new();
            for s in &strings {
                let p = uniq.iter().position(|u| u == s).unwrap_or_else(|| {
                    uniq.push(s);
                    uniq.len() - 1
                });
                ids.push(p);
            }
            let (counts, mode, least) =
                value_counts(&ids.iter().map(|&x| x as u64).collect::<Vec<_>>());
            let all_unique = !uniq.is_empty() && uniq.len() > 1 && uniq.len() == strings.len();
            let label = |pick: Option<u64>, any: bool| {
                if all_unique {
                    Value::String("[All Unique Values]".into())
                } else if let Some(p) = pick {
                    Value::String(uniq[p as usize].to_owned())
                } else if any {
                    Value::String("[Multiple Values]".into())
                } else {
                    Value::Null
                }
            };
            row.extend([
                Value::Integer(strings.len() as i32),
                Value::Integer(counts as i32),
                label(mode, !strings.is_empty()),
                label(least, !strings.is_empty()),
            ]);
        }
        out_rows.push(row);
    }
    let mut cols: Vec<Field> = Vec::new();
    for (k, name) in ["FieldName", "Alias", "FieldType"].iter().enumerate() {
        let mut fld = Field::new(*name, FieldType::String);
        fld.length = out_rows
            .iter()
            .map(|r| match &r[k] {
                Value::String(s) => s.chars().count() as u32,
                _ => 0,
            })
            .max()
            .unwrap_or(1)
            .max(1);
        cols.push(fld);
    }
    cols.push(Field::new("Nulls", FieldType::Integer));
    let dbl = |n: &str| Field::new(n, FieldType::Double);
    if numeric {
        cols.extend([
            dbl("Minimum"),
            dbl("Maximum"),
            dbl("Mean"),
            dbl("StandardDeviation"),
            dbl("Median"),
            Field::new("Count", FieldType::Integer),
            Field::new("NumberofUniqueValues", FieldType::Integer),
            dbl("Mode"),
            dbl("LeastCommon"),
            Field::new("Outliers", FieldType::Integer),
            dbl("Sum"),
            dbl("Range"),
            dbl("InterquartileRange"),
            dbl("FirstQuartile"),
            dbl("ThirdQuartile"),
            dbl("CoefficientofVariation"),
            dbl("Skewness"),
            dbl("Kurtosis"),
        ]);
    } else {
        cols.push(Field::new("Count", FieldType::Integer));
        cols.push(Field::new("NumberofUniqueValues", FieldType::Integer));
        for n in ["Mode", "LeastCommon"] {
            let mut fld = Field::new(n, FieldType::String);
            fld.length = 19;
            cols.push(fld);
        }
    }
    Ok(TableOutput {
        fields: cols,
        rows: out_rows,
    })
}

/// (distinct values, the single most common value, the single least common value) of `keys`;
/// a tie for first place gives `None`.
fn value_counts(keys: &[u64]) -> (usize, Option<u64>, Option<u64>) {
    let mut counts: Vec<(u64, usize)> = Vec::new();
    for &k in keys {
        match counts.iter_mut().find(|c| c.0 == k) {
            Some(c) => c.1 += 1,
            None => counts.push((k, 1)),
        }
    }
    let single = |target: usize| {
        let hits: Vec<_> = counts.iter().filter(|c| c.1 == target).collect();
        (hits.len() == 1).then(|| hits[0].0)
    };
    let max = counts.iter().map(|c| c.1).max().unwrap_or(0);
    let min = counts.iter().map(|c| c.1).min().unwrap_or(0);
    let mode = if counts.len() > 1 && max == 1 {
        None
    } else {
        single(max)
    };
    (counts.len(), mode, single(min))
}
