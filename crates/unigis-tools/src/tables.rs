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

/// The field type a Field Statistics To Table run summarizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldStatisticsType {
    Numeric,
    Text,
    Date,
}

/// Field Statistics To Table (one field type per run): a row per field with its null count,
/// count, unique values, mode and least common value, for numeric fields the usual moments and
/// quartiles, and for date fields minimum, maximum, mean, median, quartiles and a Range text
/// (see [`date_range_text`]). Sample standard deviation, population skewness and (non-excess) kurtosis,
/// quartiles by linear interpolation at `(n-1)p`, outliers beyond 1.5 IQR (observed, ArcGIS Pro
/// 3.7). With `group_by` (Short, Long, BigInteger, Text or a date type) the rows repeat per group
/// value, ascending (text by byte order), led by the group field; rows with a null group value
/// are left out. `statistics` (keyword, output name) picks and renames the columns, in its
/// order; keywords are the default column names in upper case, unknown ones are ignored, and if
/// none is known every column is written.
pub fn field_statistics_to_table(
    input: &dyn FeatureClass,
    field_names: &[String],
    kind: FieldStatisticsType,
    group_by: Option<&str>,
    statistics: &[(String, String)],
) -> Result<TableOutput> {
    let numeric = kind == FieldStatisticsType::Numeric;
    let date = kind == FieldStatisticsType::Date;
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
        let wanted = match kind {
            FieldStatisticsType::Numeric => is_numeric,
            FieldStatisticsType::Text => fields[i].field_type == FieldType::String,
            FieldStatisticsType::Date => fields[i].field_type == FieldType::Date,
        };
        if wanted {
            picked.push(i);
        }
    }
    let group = group_by
        .map(|g| {
            let i = fields
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(g))
                .ok_or_else(|| err(format!("ERROR 000728: Field {g} does not exist")))?;
            if matches!(
                fields[i].field_type,
                FieldType::SmallInteger
                    | FieldType::Integer
                    | FieldType::BigInteger
                    | FieldType::String
                    | FieldType::Date
                    | FieldType::DateOnly
                    | FieldType::TimeOnly
            ) {
                Ok(i)
            } else {
                Err(err(
                    "ERROR 003911: The field is not of type SHORT | TEXT | LONG | DATE | TIMEONLY | DATEONLY | BIGINTEGER",
                ))
            }
        })
        .transpose()?;
    let mut all_rows: Vec<Vec<Value>> = Vec::new();
    for f in input.features() {
        all_rows.push(f?.attributes);
    }
    let mut groups: Vec<(Option<Value>, Vec<&Vec<Value>>)> = Vec::new();
    if let Some(g) = group {
        let mut keys: Vec<&Value> = all_rows
            .iter()
            .map(|r| &r[g])
            .filter(|v| !v.is_null())
            .collect();
        keys.sort_by(|a, b| group_order(a, b));
        keys.dedup();
        for k in keys {
            let members = all_rows.iter().filter(|r| &r[g] == k).collect();
            groups.push((Some(k.clone()), members));
        }
    } else {
        groups.push((None, all_rows.iter().collect()));
    }
    let mut out_rows: Vec<Vec<Value>> = Vec::new();
    for (key, rows_in) in &groups {
        for &i in &picked {
            let f = &fields[i];
            let alias = f.alias.clone().unwrap_or_else(|| f.name.clone());
            let type_name = match f.field_type {
                FieldType::SmallInteger => "Short".to_owned(),
                FieldType::Integer => "Long".to_owned(),
                FieldType::BigInteger => "BigInteger".to_owned(),
                FieldType::Single => "Float".to_owned(),
                FieldType::Double => "Double".to_owned(),
                FieldType::Date => "Date".to_owned(),
                _ => format!("Text ({})", f.length),
            };
            let nulls = rows_in
                .iter()
                .filter(|r| matches!(r[i], Value::Null))
                .count();
            let mut row: Vec<Value> = key.iter().cloned().collect();
            row.extend([
                Value::String(f.name.clone()),
                Value::String(alias),
                Value::String(type_name),
                Value::Integer(nulls as i32),
            ]);
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
            } else if date {
                // Milliseconds since the epoch.
                let mut v: Vec<f64> = rows_in
                    .iter()
                    .filter_map(|r| match &r[i] {
                        Value::Date(d) => Some(d.and_utc().timestamp_millis() as f64),
                        _ => None,
                    })
                    .collect();
                v.sort_by(f64::total_cmp);
                let n = v.len();
                let (counts, mode, least) =
                    value_counts(&v.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
                let q = |p: f64| {
                    let pos = (n as f64 - 1.0) * p;
                    let lo = pos.floor() as usize;
                    let hi = (lo + 1).min(n - 1);
                    v[lo] + (v[hi] - v[lo]) * (pos - lo as f64)
                };
                let at = |ms: Option<f64>| {
                    ms.and_then(|ms| chrono::DateTime::from_timestamp_millis(ms.round() as i64))
                        .map_or(Value::Null, |d| Value::Date(d.naive_utc()))
                };
                let some = |f: &dyn Fn() -> f64| (n > 0).then(f);
                row.extend([
                    at(v.first().copied()),
                    at(v.last().copied()),
                    at(some(&|| v.iter().sum::<f64>() / n as f64)),
                    at(some(&|| q(0.5))),
                    Value::Integer(n as i32),
                    Value::Integer(counts as i32),
                    at(mode.map(f64::from_bits)),
                    at(least.map(f64::from_bits)),
                    v.first().zip(v.last()).map_or(Value::Null, |(a, b)| {
                        Value::String(date_range_text((b - a) / 1000.0))
                    }),
                    at(some(&|| q(0.25))),
                    at(some(&|| q(0.75))),
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
    }
    let lead = usize::from(group.is_some());
    let text_width = |k: usize| {
        out_rows
            .iter()
            .map(|r| match &r[k] {
                Value::String(s) => s.chars().count() as u32,
                _ => 0,
            })
            .max()
            .unwrap_or(1)
            .max(1)
    };
    let mut cols: Vec<Field> = Vec::new();
    if let Some(g) = group {
        // A new field of the group field's type: ArcGIS keeps neither its width nor its alias.
        let mut fld = Field::new(fields[g].name.clone(), fields[g].field_type);
        if fld.field_type == FieldType::String {
            fld.length = text_width(0);
        }
        cols.push(fld);
    }
    for (k, name) in ["FieldName", "Alias", "FieldType"].iter().enumerate() {
        let mut fld = Field::new(*name, FieldType::String);
        fld.length = out_rows
            .iter()
            .map(|r| match &r[lead + k] {
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
    } else if date {
        let day = |n: &str| Field::new(n, FieldType::Date);
        let mut range = Field::new("Range", FieldType::String);
        range.length = text_width(lead + 12);
        cols.extend([
            day("Minimum"),
            day("Maximum"),
            day("Mean"),
            day("Median"),
            Field::new("Count", FieldType::Integer),
            Field::new("NumberofUniqueValues", FieldType::Integer),
            day("Mode"),
            day("LeastCommon"),
            range,
            day("FirstQuartile"),
            day("ThirdQuartile"),
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
    // The statistics to keep, as (column index, output name). ArcGIS validates the keywords
    // against the full list of all field types and rejects those of other types (e.g. SUM in
    // a TEXT or DATE run).
    let mut picked_stats: Vec<(usize, String)> = Vec::new();
    for (k, (key, name)) in statistics.iter().enumerate() {
        if statistics[..k].iter().any(|(earlier, _)| earlier == key) {
            return Err(err(format!(
                "ERROR 110415: The following values {key} have duplicates."
            )));
        }
        if let Some(c) = (lead..cols.len()).find(|&c| cols[c].name.to_uppercase() == *key) {
            let name = if name.is_empty() {
                cols[c].name.clone()
            } else {
                valid_field_name(name)
            };
            picked_stats.push((c, name));
        } else if !numeric && NUMERIC_STATISTICS.contains(&key.as_str()) {
            let text: Vec<String> = cols[lead..].iter().map(|f| f.name.to_uppercase()).collect();
            return Err(err(format!(
                "ERROR 000800: The value is not a member of {}.",
                text.join(" | ")
            )));
        }
    }
    if picked_stats.is_empty() {
        return Ok(TableOutput {
            fields: cols,
            rows: out_rows,
        });
    }
    let mut keep: Vec<usize> = (0..lead).collect();
    keep.extend(picked_stats.iter().map(|(c, _)| *c));
    let mut fields_out: Vec<Field> = cols[..lead].to_vec();
    for (c, name) in &picked_stats {
        if fields_out.iter().any(|f| f.name.eq_ignore_ascii_case(name)) {
            return Err(err(
                "ERROR 000210: Cannot create output table (duplicate field name)",
            ));
        }
        let mut f = cols[*c].clone();
        f.name = name.clone();
        fields_out.push(f);
    }
    let rows = out_rows
        .into_iter()
        .map(|r| keep.iter().map(|&k| r[k].clone()).collect())
        .collect();
    Ok(TableOutput {
        fields: fields_out,
        rows,
    })
}

/// A date range as Field Statistics To Table writes it: in years (365 days) beyond 365 days,
/// months (30 days) beyond 30, weeks beyond 7, days from one day, then hours, minutes, seconds
/// and milliseconds. Whole amounts have no decimals; others one decimal when that is within 1%,
/// else two (`200.1 years`, `1.33 months`, `3.7 months`, `60 seconds`; ArcGIS Pro 3.7).
fn date_range_text(seconds: f64) -> String {
    let days = seconds / 86_400.0;
    let (v, unit) = if days > 365.0 {
        (days / 365.0, "years")
    } else if days > 30.0 {
        (days / 30.0, "months")
    } else if days > 7.0 {
        (days / 7.0, "weeks")
    } else if days >= 1.0 {
        (days, "days")
    } else if seconds > 3600.0 {
        (seconds / 3600.0, "hours")
    } else if seconds > 60.0 {
        (seconds / 60.0, "minutes")
    } else if seconds >= 1.0 {
        (seconds, "seconds")
    } else {
        (seconds * 1000.0, "milliseconds")
    };
    let amount = if v.fract() == 0.0 {
        format!("{v:.0}")
    } else if (((v * 10.0).round() / 10.0) - v).abs() / v < 0.01 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    format!("{amount} {unit}")
}

/// The Field Statistics To Table keywords only a NUMERIC run accepts.
const NUMERIC_STATISTICS: [&str; 14] = [
    "MINIMUM",
    "MAXIMUM",
    "MEAN",
    "STANDARDDEVIATION",
    "MEDIAN",
    "OUTLIERS",
    "SUM",
    "RANGE",
    "INTERQUARTILERANGE",
    "FIRSTQUARTILE",
    "THIRDQUARTILE",
    "COEFFICIENTOFVARIATION",
    "SKEWNESS",
    "KURTOSIS",
];

/// A field name as arcpy.ValidateFieldName makes it: other characters than letters, digits and
/// `_` become `_`, and a leading digit gets an `F` in front (`My Med` → `My_Med`, `1sum` →
/// `F1sum`).
fn valid_field_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'F');
    }
    out
}

/// Ascending order of Field Statistics To Table group values: numbers by value, text by bytes.
fn group_order(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
        (Value::DateOnly(x), Value::DateOnly(y)) => x.cmp(y),
        (Value::TimeOnly(x), Value::TimeOnly(y)) => x.cmp(y),
        _ => crate::number(a)
            .unwrap_or(f64::NAN)
            .total_cmp(&crate::number(b).unwrap_or(f64::NAN)),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The memory table probed in ArcGIS Pro 3.7 (G text, V double, S short, with nulls).
    fn probe_table() -> Mem {
        let s = |x: &str| Value::String(x.into());
        let rows = [
            (s("b"), Value::Double(1.0), Value::SmallInteger(1)),
            (s("B"), Value::Double(2.0), Value::SmallInteger(2)),
            (s("a"), Value::Double(3.0), Value::Null),
            (Value::Null, Value::Double(4.0), Value::SmallInteger(4)),
            (s("b"), Value::Double(5.0), Value::SmallInteger(5)),
            (s(""), Value::Double(6.0), Value::SmallInteger(6)),
            (s("a"), Value::Null, Value::SmallInteger(7)),
            (Value::Null, Value::Double(8.0), Value::SmallInteger(8)),
        ];
        let mut g = Field::new("G", FieldType::String);
        g.length = 8;
        Mem(ToolOutput {
            kind: GeometryKind::Point,
            has_z: false,
            has_m: false,
            fields: vec![
                g,
                Field::new("V", FieldType::Double),
                Field::new("S", FieldType::SmallInteger),
            ],
            spatial_reference_wkt: None,
            features: rows
                .into_iter()
                .enumerate()
                .map(|(i, (a, b, c))| Feature {
                    oid: i as i64,
                    geometry: None,
                    attributes: vec![a, b, c],
                })
                .collect(),
            messages: Vec::new(),
        })
    }

    #[test]
    fn field_statistics_groups_skip_nulls_and_sort_by_bytes() {
        let names = ["V".to_owned(), "S".to_owned()];
        let t = field_statistics_to_table(
            &probe_table(),
            &names,
            FieldStatisticsType::Numeric,
            Some("G"),
            &[],
        )
        .unwrap();
        assert_eq!(t.fields[0].name, "G");
        assert_eq!(t.fields[0].length, 1);
        let keys: Vec<_> = t.rows.iter().map(|r| r[0].clone()).collect();
        let s = |x: &str| Value::String(x.into());
        assert_eq!(
            keys,
            [s(""), s(""), s("B"), s("B"), s("a"), s("a"), s("b"), s("b")]
        );
        // Group "a": V has one null (min 3), S has one null (min 7); group "b": V 1..5.
        assert_eq!(t.rows[4][4], Value::Integer(1));
        assert_eq!(t.rows[4][5], Value::Double(3.0));
        assert_eq!(t.rows[5][5], Value::Double(7.0));
        assert_eq!(t.rows[6][5], Value::Double(1.0));
        assert_eq!(t.rows[6][6], Value::Double(5.0));
    }

    #[test]
    fn field_statistics_group_by_short_drops_null_group_rows() {
        let names = ["V".to_owned()];
        let t = field_statistics_to_table(
            &probe_table(),
            &names,
            FieldStatisticsType::Numeric,
            Some("S"),
            &[],
        )
        .unwrap();
        let keys: Vec<_> = t.rows.iter().map(|r| r[0].clone()).collect();
        let k: Vec<_> = [1, 2, 4, 5, 6, 7, 8].map(Value::SmallInteger).into();
        assert_eq!(keys, k);
        // S = 7 holds only a null V: no statistics.
        assert_eq!(t.rows[5][4], Value::Integer(1));
        assert_eq!(t.rows[5][5], Value::Null);
    }

    #[test]
    fn field_statistics_rejects_a_double_group_field() {
        let names = ["S".to_owned()];
        let e = field_statistics_to_table(
            &probe_table(),
            &names,
            FieldStatisticsType::Numeric,
            Some("V"),
            &[],
        )
        .unwrap_err();
        assert!(e.to_string().contains("003911"), "{e}");
    }

    #[test]
    fn date_ranges_match_arcgis() {
        for (days, want) in [
            (29.0, "4.14 weeks"),
            (30.0, "4.3 weeks"),
            (31.0, "1.03 months"),
            (60.0, "2 months"),
            (100.0, "3.33 months"),
            (110.0, "3.7 months"),
            (365.0, "12.2 months"),
            (366.0, "1.0 years"),
            (550.0, "1.5 years"),
            (851.0, "2.33 years"),
            (1100.0, "3.0 years"),
            (73049.0, "200.1 years"),
            (7.0, "7 days"),
            (7.5, "1.07 weeks"),
            (1.0, "1 days"),
        ] {
            assert_eq!(date_range_text(days * 86_400.0), want, "{days} days");
        }
        for (seconds, want) in [
            (0.0, "0 milliseconds"),
            (60.0, "60 seconds"),
            (61.0, "1.02 minutes"),
            (3599.0, "60.0 minutes"),
            (3600.0, "60 minutes"),
            (3601.0, "1.0 hours"),
            (86_399.0, "24.0 hours"),
        ] {
            assert_eq!(date_range_text(seconds), want, "{seconds} s");
        }
    }

    fn stats(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(k, n)| ((*k).to_owned(), (*n).to_owned()))
            .collect()
    }

    #[test]
    fn field_statistics_picks_and_renames_columns_in_list_order() {
        let names = ["V".to_owned()];
        let picks = stats(&[
            ("MEDIAN", "My Med"),
            ("BOGUS", "B"),
            ("SUM", "1sum"),
            ("COUNT", ""),
        ]);
        let t = field_statistics_to_table(
            &probe_table(),
            &names,
            FieldStatisticsType::Numeric,
            Some("S"),
            &picks,
        )
        .unwrap();
        let cols: Vec<_> = t.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(cols, ["S", "My_Med", "F1sum", "Count"]);
        assert_eq!(
            t.rows[0],
            [
                Value::SmallInteger(1),
                Value::Double(1.0),
                Value::Double(1.0),
                Value::Integer(1)
            ]
        );
        // Only unknown or lower-case keywords: every column.
        let all = field_statistics_to_table(
            &probe_table(),
            &names,
            FieldStatisticsType::Numeric,
            None,
            &stats(&[("median", "m")]),
        )
        .unwrap();
        assert_eq!(all.fields.len(), 22);
    }

    #[test]
    fn field_statistics_rejects_bad_statistic_lists() {
        let run = |numeric: bool, group: Option<&str>, items: &[(&str, &str)]| {
            let n = [if numeric { "V" } else { "G" }.to_owned()];
            let kind = if numeric {
                FieldStatisticsType::Numeric
            } else {
                FieldStatisticsType::Text
            };
            field_statistics_to_table(&probe_table(), &n, kind, group, &stats(items))
                .unwrap_err()
                .to_string()
        };
        assert!(run(true, None, &[("MEDIAN", "A"), ("MEDIAN", "B")]).contains("110415"));
        assert!(run(true, None, &[("MEDIAN", "X"), ("SUM", "x")]).contains("000210"));
        assert!(run(true, Some("S"), &[("MEDIAN", "s")]).contains("000210"));
        let text = run(false, None, &[("MEAN", "M")]);
        assert!(text.contains("ERROR 000800: The value is not a member of FIELDNAME | ALIAS | FIELDTYPE | NULLS | COUNT | NUMBEROFUNIQUEVALUES | MODE | LEASTCOMMON."), "{text}");
    }
}
