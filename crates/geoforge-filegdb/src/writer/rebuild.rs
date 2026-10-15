//! Rebuilding the attribute indexes of an existing table from its rows, as needed
//! after the table (or a catalog table) has been edited.

use std::path::Path;

use geoforge_core::{Error, Result, Value};

use super::btree::write_index;
use super::keys;
use crate::bytes::{Reader, err};
use crate::table::{GdbFieldType, Table};

/// One entry of a `.gdbindexes` file.
struct RawIndex {
    name: String,
    flag: i32,
    expression: String,
}

fn read_raw(path: &Path) -> Result<Vec<RawIndex>> {
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    let mut r = Reader::new(&bytes);
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n.min(1024) {
        let len = r.u32()? as usize;
        let name = r.utf16(len)?;
        r.bytes(2)?;
        let flag = r.i32()?;
        r.bytes(2 + 4)?;
        let len = r.u32()? as usize;
        let expression = r.utf16(len)?;
        r.bytes(2)?;
        out.push(RawIndex {
            name,
            flag,
            expression,
        });
    }
    Ok(out)
}

/// Rewrites every attribute index (`.atx`) listed in `<table_base>.gdbindexes` from the
/// table's current rows: plain field names and `LOWER(field)` expressions. Returns the
/// names of the indexes written.
pub fn rebuild_attribute_indexes(table_base: impl AsRef<Path>) -> Result<Vec<String>> {
    let base = table_base.as_ref();
    // Tables written by GeoForge's own system-table writer have no index file.
    if !base.with_extension("gdbindexes").is_file() {
        return Ok(Vec::new());
    }
    let table = Table::open(base)?;
    let defs = read_raw(&base.with_extension("gdbindexes"))?;
    // The attribute fields in the order their values appear in a row.
    let value_fields: Vec<_> = table
        .all_fields()
        .iter()
        .filter(|f| {
            !matches!(
                f.field_type,
                GdbFieldType::ObjectId | GdbFieldType::Geometry
            )
        })
        .collect();
    let rows: Vec<_> = table.features().collect::<Result<_>>()?;
    let mut written = Vec::new();
    for d in defs.iter().filter(|d| d.flag == 2) {
        let (lower, field_name) = match d
            .expression
            .strip_prefix("LOWER(")
            .and_then(|e| e.strip_suffix(')'))
        {
            Some(inner) => (true, inner),
            None => (false, d.expression.as_str()),
        };
        let at = value_fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(field_name))
            .ok_or_else(|| err(format!("index {}: no field {field_name}", d.name)))?;
        let field = value_fields[at];
        let layout = keys::layout(field).ok_or_else(|| {
            err(format!(
                "index {}: field {field_name} cannot be indexed",
                d.name
            ))
        })?;
        let mut entries: Vec<(keys::Key, u32)> = Vec::new();
        for r in &rows {
            let mut v = r.attributes.get(at).cloned().unwrap_or(Value::Null);
            if lower && let Value::String(s) = &v {
                v = Value::String(s.to_lowercase());
            }
            if let Some(k) = keys::key_of(&layout, &v) {
                entries.push((k, r.oid as u32));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let encoded: Vec<(Vec<u8>, u32)> = entries
            .iter()
            .map(|(k, oid)| (keys::encode(&layout, k), *oid))
            .collect();
        write_index(
            &base.with_extension(format!("{}.atx", d.name)),
            layout.key_size,
            layout.kind,
            &encoded,
        )?;
        written.push(d.name.clone());
    }
    Ok(written)
}
