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
