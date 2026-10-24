//! Adding and removing attribute indexes of an existing table (`arcpy.management.AddIndex`,
//! `RemoveIndex`).
//!
//! As observed through the bridge (`testdata/gdb-reference/index_ops`): UNIQUE and ASCENDING are
//! accepted and ignored in a file geodatabase; a new index goes last in the table's index list;
//! an index name that exists is error 000192, an empty one error 000530; the object ID and the
//! shape cannot be indexed (error 003911); the same field can carry several indexes; removing a
//! name that is not an attribute index (including `FDO_OBJECTID`, `FDO_Shape`) is error 000192.
//! Only single-field indexes are done.

use std::path::Path;

use unigis_core::Result;

use super::catalog::base;
use super::rewrite::{IndexEdit, rewrite_core};
use super::subtype_edit::{bad, field, load_target};
use crate::table::{GdbFieldType, Table};

fn rewrite_with(dir: &Path, table: &str, edit: &IndexEdit<'_>) -> Result<()> {
    let t = load_target(dir, table)?;
    let tab_base = base(dir, t.table_id);
    let old = Table::open(&tab_base)?;
    let mut fields = old.all_fields().to_vec();
    // The spatial grid is chosen again from the rows.
    for g in fields.iter_mut().filter_map(|f| f.geometry.as_mut()) {
        g.grid_sizes.clear();
    }
    let rows = old
        .features()
        .map(|f| {
            let f = f?;
            Ok((Some(f.oid as u64), f))
        })
        .collect::<Result<Vec<_>>>()?;
    rewrite_core(&tab_base, old, fields, &[], edit, &rows, true).map(|_| ())
}

/// `AddIndex` on one field.
pub fn add_attribute_index(
    dir: impl AsRef<Path>,
    table: &str,
    field_name: &str,
    index_name: &str,
) -> Result<()> {
    let dir = dir.as_ref();
    if index_name.trim().is_empty() {
        return Err(bad("ERROR 000530: Value is required"));
    }
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let (name, ty, _) = field(&t, field_name)?.clone();
        if matches!(ty, GdbFieldType::ObjectId | GdbFieldType::Geometry) {
            return Err(bad(
                "ERROR 003911: The field is not of type SHORT | LONG | BIGINTEGER | FLOAT | DOUBLE | TEXT | DATE | DATEONLY | TIMEONLY | TIMESTAMPOFFSET | GUID | GLOBALID",
            ));
        }
        let taken = super::rebuild::read_raw(&base(dir, t.table_id).with_extension("gdbindexes"))?
            .iter()
            .any(|d| d.name.eq_ignore_ascii_case(index_name));
        if taken {
            return Err(bad("ERROR 000192: Invalid value for Index Name"));
        }
        rewrite_with(dir, table, &|list| {
            list.push((index_name.to_owned(), name.clone()));
        })
    })
}

/// `RemoveIndex`: an attribute index by name.
pub fn remove_attribute_index(dir: impl AsRef<Path>, table: &str, index_name: &str) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let found = super::rebuild::read_raw(&base(dir, t.table_id).with_extension("gdbindexes"))?
            .iter()
            .any(|d| d.flag == 2 && d.name.eq_ignore_ascii_case(index_name));
        if !found {
            return Err(bad(
                "ERROR 000192: Invalid value for Index Name or Indexed Item",
            ));
        }
        rewrite_with(dir, table, &|list| {
            list.retain(|(n, _)| !n.eq_ignore_ascii_case(index_name));
        })
    })
}
