//! Replacing the fields and rows of an existing table while it keeps its place in the
//! geodatabase (file number, geometry definition, surviving attribute indexes): what
//! adding or deleting a field, or a tool updating a table in place, amounts to.

use std::path::Path;

use unigis_core::{Error, Feature, Field, FieldType, Result};

use super::btree::{spatial_entries, write_index_for};
use super::rebuild::read_raw;
use super::table::TableWriter;
use super::txn;
use super::{to_gdb_field, write_gdbindexes_for};
use crate::bytes::err;
use crate::table::{GdbField, GdbFieldType, Table};

/// Rewrites the table at `base` (path of `aXXXXXXXX`, with or without extension) with the
/// attribute fields `attribute_fields` and `rows`. A row's OBJECTID is kept when given
/// (OBJECTIDs missing from `rows` stay deleted); rows without one are numbered after the
/// largest OBJECTID the table has had. Fields that match an existing field in name and type
/// keep their stored definition (defaults, domain, alias); attribute indexes on fields that
/// remain are rebuilt. Returns the new row count.
///
/// The OBJECTID sequence continues where it was (deleted OBJECTIDs are not reused), as in
/// ArcGIS; [`truncate_table`] starts it again.
pub fn rewrite_table(
    base: impl AsRef<Path>,
    attribute_fields: &[Field],
    rows: &[(Option<u64>, Feature)],
) -> Result<u64> {
    rewrite(base.as_ref(), attribute_fields, rows, true)
}

/// Deletes every row of a table and restarts its OBJECTIDs at 1 (`arcpy.management.
/// TruncateTable`); fields, indexes and the geometry definition stay.
pub fn truncate_table(base: impl AsRef<Path>) -> Result<()> {
    let base = base.as_ref().with_extension("");
    let fields = Table::open(&base)?.fields().to_vec();
    rewrite(&base, &fields, &[], false).map(|_| ())
}

/// Adds `field` after the existing fields of a table (`arcpy.management.AddField`); the
/// existing rows get its default value, or null. Rewrites the table.
pub fn add_field(base: impl AsRef<Path>, field: &Field) -> Result<()> {
    let base = base.as_ref().with_extension("");
    super::validate_field_name(&field.name)?;
    let table = Table::open(&base)?;
    if table
        .all_fields()
        .iter()
        .any(|f| f.name.eq_ignore_ascii_case(&field.name))
    {
        return Err(Error::Unsupported(format!(
            "ERROR 000012: {} already exists",
            field.name
        )));
    }
    if !field.nullable && field.default.is_none() && table.row_count() > 0 {
        return Err(Error::Unsupported(format!(
            "field {} is not nullable and has no default, but the table has rows",
            field.name
        )));
    }
    let mut fields = table.fields().to_vec();
    fields.push(field.clone());
    let fill = field.default.clone().unwrap_or(unigis_core::Value::Null);
    let rows = table
        .features()
        .map(|f| {
            let mut f = f?;
            f.attributes.push(fill.clone());
            Ok((Some(f.oid as u64), f))
        })
        .collect::<Result<Vec<_>>>()?;
    drop(table);
    rewrite(&base, &fields, &rows, true).map(|_| ())
}

/// Removes fields from a table (`arcpy.management.DeleteField`). OBJECTID, the geometry
/// and ArcGIS's required `Shape_Length` / `Shape_Area` fields cannot be removed.
/// Rewrites the table.
pub fn delete_fields(base: impl AsRef<Path>, names: &[&str]) -> Result<()> {
    let base = base.as_ref().with_extension("");
    let table = Table::open(&base)?;
    let shape = table.shape_field().map(|f| f.name.to_ascii_lowercase());
    let mut keep = Vec::new();
    for (i, f) in table.fields().iter().enumerate() {
        if names.iter().any(|n| n.eq_ignore_ascii_case(&f.name)) {
            let n = f.name.to_ascii_lowercase();
            if shape
                .as_ref()
                .is_some_and(|s| n == format!("{s}_length") || n == format!("{s}_area"))
            {
                return Err(Error::Unsupported(format!(
                    "ERROR 000370: {} is a required field and cannot be deleted",
                    f.name
                )));
            }
        } else {
            keep.push(i);
        }
    }
    for n in names {
        let known = table
            .all_fields()
            .iter()
            .any(|f| f.name.eq_ignore_ascii_case(n));
        if !known {
            return Err(Error::Unsupported(format!("no field {n}")));
        }
        if !table
            .fields()
            .iter()
            .any(|f| f.name.eq_ignore_ascii_case(n))
        {
            return Err(Error::Unsupported(format!(
                "ERROR 000370: {n} is a required field and cannot be deleted"
            )));
        }
    }
    let fields: Vec<Field> = keep.iter().map(|&i| table.fields()[i].clone()).collect();
    let rows = table
        .features()
        .map(|f| {
            let mut f = f?;
            f.attributes = keep.iter().map(|&i| f.attributes[i].clone()).collect();
            Ok((Some(f.oid as u64), f))
        })
        .collect::<Result<Vec<_>>>()?;
    drop(table);
    rewrite(&base, &fields, &rows, true).map(|_| ())
}

fn is_attribute(f: &GdbField) -> bool {
    !matches!(
        f.field_type,
        GdbFieldType::ObjectId | GdbFieldType::Geometry
    )
}

fn rewrite(
    base: &Path,
    attribute_fields: &[Field],
    rows: &[(Option<u64>, Feature)],
    keep_max_oid: bool,
) -> Result<u64> {
    let base = base.with_extension("");
    let old = Table::open(&base)?;
    let mut fields: Vec<GdbField> = old
        .all_fields()
        .iter()
        .filter(|f| !is_attribute(f))
        .cloned()
        .collect();
    // The spatial grid is chosen again from the new rows.
    for g in fields.iter_mut().filter_map(|f| f.geometry.as_mut()) {
        g.grid_sizes.clear();
    }
    for f in attribute_fields {
        let kept = old.all_fields().iter().find(|g| {
            is_attribute(g)
                && g.name.eq_ignore_ascii_case(&f.name)
                && g.field_type.esri() == f.field_type
                && (f.field_type != FieldType::String || f.length == 0 || g.length == f.length)
        });
        fields.push(match kept {
            Some(g) => {
                let mut g = g.clone();
                // A changed default replaces the stored one; the rest of the definition stays.
                let stored = g
                    .default
                    .as_deref()
                    .and_then(|d| crate::table::decode_default(g.field_type, d));
                if stored != f.default {
                    g.default = f
                        .default
                        .as_ref()
                        .and_then(|v| crate::table::encode_default(g.field_type, v));
                }
                g
            }
            None => to_gdb_field(f)?,
        });
    }
    rewrite_core(&base, old, fields, &[], &|_| {}, rows, keep_max_oid)
}

/// Writes the table again with `fields` (the object ID and geometry fields followed by the
/// attribute fields, as [`Table::all_fields`] lists them) and `rows`. `renames` lists
/// (old, new) names of attribute fields whose attribute indexes follow them.
pub(super) fn rewrite_core(
    base: &Path,
    old: Table,
    fields: Vec<GdbField>,
    renames: &[(String, String)],
    edit_indexes: &dyn Fn(&mut Vec<(String, String)>),
    rows: &[(Option<u64>, Feature)],
    keep_max_oid: bool,
) -> Result<u64> {
    let v4 = old.is_version_4();
    // Attribute indexes whose field is still there.
    let mut indexes: Vec<(String, String)> = Vec::new();
    let index_file = base.with_extension("gdbindexes");
    if index_file.is_file() {
        for d in read_raw(&index_file)?.into_iter().filter(|d| d.flag == 2) {
            let field = renames
                .iter()
                .find(|(old, _)| old.eq_ignore_ascii_case(&d.expression))
                .map_or(d.expression.clone(), |(_, new)| new.clone());
            if fields
                .iter()
                .any(|f| is_attribute(f) && f.name.eq_ignore_ascii_case(&field))
            {
                indexes.push((d.name, field));
            }
        }
    }

    edit_indexes(&mut indexes);
    let (geometry_kind, has_z, has_m) = (old.geometry_kind(), old.has_z(), old.has_m());
    let old_max_oid = old.max_oid();
    // The index names follow the table's own field names (`FDO_Shape` in ArcGIS's tables).
    let oid_name = old.oid_field_name().unwrap_or("OBJECTID").to_owned();
    let shape_name = old
        .shape_field()
        .map_or("SHAPE".to_owned(), |f| f.name.clone());
    // Number the rows.
    let mut next = old
        .max_oid()
        .max(rows.iter().filter_map(|r| r.0).max().unwrap_or(0));
    let mut numbered: Vec<(u64, &Feature)> = rows
        .iter()
        .map(|(oid, f)| match oid {
            Some(o) => (*o, f),
            None => {
                next += 1;
                (next, f)
            }
        })
        .collect();
    numbered.sort_by_key(|r| r.0);
    if numbered.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err(err("two rows with the same OBJECTID"));
    }
    // Rows whose geometry is new or changed get ArcGIS's measures recomputed.
    let mut changed = std::collections::HashSet::new();
    if old.shape_field().is_some() {
        for (oid, f) in &numbered {
            let same = old.get(*oid)?.is_some_and(|o| o.geometry == f.geometry);
            if !same {
                changed.insert(*oid);
            }
        }
    }

    // Write into a scratch directory, then move the files over the old ones, all in one
    // transaction (a crash leaves the old table).
    let dir = base
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let stem = base
        .file_name()
        .ok_or_else(|| err("table path has no name"))?
        .to_owned();
    let guard = txn::begin(&dir)?;
    let scratch = txn::scratch_dir(&dir, &stem.to_string_lossy())?;
    let tmp = scratch.join(&stem);
    // The old table's handle is closed before its files are moved away.
    drop(old);
    let result = (|| -> Result<u64> {
        let mut w =
            TableWriter::create_versioned(&tmp, fields.clone(), geometry_kind, has_z, has_m, v4)?;
        for (name, field) in &indexes {
            w.add_index(name, field)?;
        }
        for (oid, f) in &numbered {
            w.measure_next = changed.contains(oid);
            w.push_at(*oid, f.geometry.as_ref(), &f.attributes)?;
        }
        if keep_max_oid {
            w.reserve_oids(old_max_oid);
        }
        let n = numbered.len() as u64;
        let finished = w.finish()?;
        let mut index_names = Vec::new();
        for ix in &finished.indexes {
            write_index_for(
                &tmp.with_extension(format!("{}.atx", ix.name)),
                ix.key_size,
                ix.kind,
                &ix.entries,
                v4,
            )?;
            index_names.push((ix.name.clone(), ix.field.clone()));
        }
        match finished.grid {
            Some(grid) => {
                write_index_for(
                    &tmp.with_extension("spx"),
                    8,
                    0x40,
                    &spatial_entries(&finished.envelopes, grid),
                    v4,
                )?;
                write_gdbindexes_for(
                    &tmp.with_extension("gdbindexes"),
                    &oid_name,
                    Some(&shape_name),
                    &index_names,
                )?;
            }
            None => write_gdbindexes_for(
                &tmp.with_extension("gdbindexes"),
                &oid_name,
                None,
                &index_names,
            )?,
        }

        // Remove what belonged to the old table, then move the new files in.
        let prefix = format!("{}.", stem.to_string_lossy());
        for e in std::fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))? {
            let e = e.map_err(|e| Error::io(&dir, e))?;
            let name = e.file_name().to_string_lossy().into_owned();
            let own = name.starts_with(&prefix)
                && (name.ends_with(".atx")
                    || ["gdbtable", "gdbtablx", "gdbindexes", "spx", "freelist"]
                        .iter()
                        .any(|x| name == format!("{prefix}{x}")));
            if own {
                txn::remove_file(&e.path())?;
            }
        }
        for e in std::fs::read_dir(&scratch).map_err(|e| Error::io(&scratch, e))? {
            let e = e.map_err(|er| Error::io(&scratch, er))?;
            txn::install(&e.path(), &dir.join(e.file_name()))?;
        }
        Ok(n)
    })();
    std::fs::remove_dir_all(&scratch).ok();
    let n = result?;
    guard.commit()?;
    Ok(n)
}
