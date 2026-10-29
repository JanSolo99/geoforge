//! Validate Join (Data Management): how many rows an attribute join would match, and the
//! message table ArcGIS writes about it.

use unigis_core::{Error, FeatureClass, Field, FieldType, Result, Value};

use crate::TableOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// One table of the join: its rows' key values and the name ArcGIS reports for it
/// (`<folder>.<file>`, e.g. `geoproc.parcels.shp`).
pub struct JoinSide<'a> {
    pub table: &'a dyn FeatureClass,
    pub field: &'a str,
    pub name: String,
}

fn keys(side: &JoinSide) -> Result<(String, Vec<Value>)> {
    let i = side
        .table
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(side.field))
        .ok_or_else(|| err(format!("ERROR 000728: Field {} does not exist", side.field)))?;
    let name = side.table.fields()[i].name.clone();
    let mut v = Vec::new();
    for f in side.table.features() {
        v.push(f?.attributes[i].clone());
    }
    Ok((name, v))
}

/// Whether two key values join: numbers by value, text exactly; text never matches a number.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, _) | (_, Value::Null) => false,
        (Value::String(x), Value::String(y)) => x.trim_end() == y.trim_end(),
        (Value::String(_), _) | (_, Value::String(_)) => false,
        _ => match (crate::number(a), crate::number(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}

/// Validate Join: the number of input rows a one-to-one join would match (rows whose key has a
/// join row with the same value), and ArcGIS's message table: a row per join field without an
/// attribute index (TYPE is the field name), `GPM_NO_MATCH_JOIN` when nothing matches and
/// `GPM_ROW_COUNTS`; FIELD_NAME stays blank (observed against ArcGIS Pro 3.7). Shapefile and
/// dBASE fields are taken as unindexed. Joining a table to itself fails (ERROR 001706).
pub fn validate_join(
    input: &JoinSide,
    join: &JoinSide,
    same_table: bool,
) -> Result<(usize, TableOutput)> {
    if same_table {
        return Err(err("ERROR 001706: Cannot join a table to itself."));
    }
    let (in_name, in_keys) = keys(input)?;
    let (join_name, join_keys) = keys(join)?;
    let matched = in_keys
        .iter()
        .filter(|k| join_keys.iter().any(|j| same(k, j)))
        .count();
    let text = |name: &str| {
        let mut f = Field::new(name, FieldType::String);
        f.length = 254;
        f
    };
    let fields = vec![
        text("TYPE"),
        text("TABLE_NAME"),
        text("FIELD_NAME"),
        text("DESC_"),
    ];
    let row = |kind: &str, table: &str, desc: String| {
        vec![
            Value::String(kind.to_owned()),
            Value::String(table.to_owned()),
            Value::String(" ".to_owned()),
            Value::String(desc),
        ]
    };
    let mut rows = Vec::new();
    for (field, table) in [(&in_name, &input.name), (&join_name, &join.name)] {
        rows.push(row(
            field,
            table,
            format!(
                "The join field {field} in the table {table} is not indexed. To improve performance, we recommend that an index be created."
            ),
        ));
    }
    if matched == 0 {
        rows.push(row(
            "GPM_NO_MATCH_JOIN",
            &join.name,
            "Join had no matches.".to_owned(),
        ));
    }
    rows.push(row(
        "GPM_ROW_COUNTS",
        &join.name,
        format!(
            "The input table has {} and the join table has {} records.",
            in_keys.len(),
            join_keys.len()
        ),
    ));
    Ok((matched, TableOutput { fields, rows }))
}

/// The name Validate Join reports for a table at `path`: its folder's name, a dot, its file
/// name (`testdata/geoproc/parcels.shp` -> `geoproc.parcels.shp`).
pub fn table_name(path: &str) -> String {
    let p = std::path::Path::new(path);
    let file = p
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let folder = p
        .parent()
        .and_then(|d| {
            std::fs::canonicalize(d)
                .ok()
                .or_else(|| Some(d.to_path_buf()))
        })
        .and_then(|d| d.file_name().map(|f| f.to_string_lossy().into_owned()))
        .unwrap_or_default();
    format!("{folder}.{file}")
}
