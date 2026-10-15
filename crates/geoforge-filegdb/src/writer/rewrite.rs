//! Replacing the fields and rows of an existing table while it keeps its place in the
//! geodatabase (file number, geometry definition, surviving attribute indexes): what
//! adding or deleting a field, or a tool updating a table in place, amounts to.

use std::path::Path;

use geoforge_core::{Error, Feature, Field, FieldType, Result};

use super::btree::{spatial_entries, write_index};
use super::rebuild::read_raw;
use super::table::TableWriter;
use super::{to_gdb_field, write_gdbindexes};
use crate::bytes::err;
use crate::table::{GdbField, GdbFieldType, Table};

/// Rewrites the table at `base` (path of `aXXXXXXXX`, with or without extension) with the
/// attribute fields `attribute_fields` and `rows`. A row's OBJECTID is kept when given
/// (OBJECTIDs missing from `rows` stay deleted); rows without one are numbered after the
/// largest OBJECTID the table has had. Fields that match an existing field in name and type
/// keep their stored definition (defaults, domain, alias); attribute indexes on fields that
/// remain are rebuilt. Returns the new row count.
pub fn rewrite_table(
    base: impl AsRef<Path>,
    attribute_fields: &[Field],
    rows: &[(Option<u64>, Feature)],
) -> Result<u64> {
    let base = base.as_ref().with_extension("");
    let old = Table::open(&base)?;
    if old.is_version_4() {
        return Err(Error::Unsupported(
            "rewriting 64-bit OBJECTID tables (ArcGIS Pro 3.2 format) is not implemented".into(),
        ));
    }
    let is_attribute = |f: &GdbField| {
        !matches!(
            f.field_type,
            GdbFieldType::ObjectId | GdbFieldType::Geometry
        )
    };
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
            Some(g) => g.clone(),
            None => to_gdb_field(f)?,
        });
    }
    // Attribute indexes whose field is still there.
    let mut indexes: Vec<(String, String)> = Vec::new();
    let index_file = base.with_extension("gdbindexes");
    if index_file.is_file() {
        for d in read_raw(&index_file)?.into_iter().filter(|d| d.flag == 2) {
            if fields
                .iter()
                .any(|f| is_attribute(f) && f.name.eq_ignore_ascii_case(&d.expression))
            {
                indexes.push((d.name, d.expression));
            }
        }
    }

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

    // Write into a scratch directory, then move the files over the old ones.
    let dir = base
        .parent()
        .ok_or_else(|| err("table path has no directory"))?;
    let stem = base
        .file_name()
        .ok_or_else(|| err("table path has no name"))?
        .to_owned();
    let scratch = dir.join(format!(".gf-rewrite-{}", stem.to_string_lossy()));
    std::fs::remove_dir_all(&scratch).ok();
    std::fs::create_dir_all(&scratch).map_err(|e| Error::io(&scratch, e))?;
    let tmp = scratch.join(&stem);
    let result = (|| -> Result<u64> {
        let mut w = TableWriter::create(
            &tmp,
            fields.clone(),
            old.geometry_kind(),
            old.has_z(),
            old.has_m(),
        )?;
        for (name, field) in &indexes {
            w.add_index(name, field)?;
        }
        for (oid, f) in &numbered {
            w.push_at(*oid, f.geometry.as_ref(), &f.attributes)?;
        }
        let n = numbered.len() as u64;
        let finished = w.finish()?;
        let mut index_names = Vec::new();
        for ix in &finished.indexes {
            write_index(
                &tmp.with_extension(format!("{}.atx", ix.name)),
                ix.key_size,
                ix.kind,
                &ix.entries,
            )?;
            index_names.push((ix.name.clone(), ix.field.clone()));
        }
        match finished.grid {
            Some(grid) => {
                write_index(
                    &tmp.with_extension("spx"),
                    8,
                    0x40,
                    &spatial_entries(&finished.envelopes, grid),
                )?;
                write_gdbindexes(&tmp.with_extension("gdbindexes"), true, &index_names)?;
            }
            None => write_gdbindexes(&tmp.with_extension("gdbindexes"), false, &index_names)?,
        }
        Ok(n)
    })();
    let n = match result {
        Ok(n) => n,
        Err(e) => {
            std::fs::remove_dir_all(&scratch).ok();
            return Err(e);
        }
    };

    // Remove what belonged to the old table, then move the new files in.
    let prefix = format!("{}.", stem.to_string_lossy());
    for e in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let e = e.map_err(|e| Error::io(dir, e))?;
        let name = e.file_name().to_string_lossy().into_owned();
        let own = name.starts_with(&prefix)
            && (name.ends_with(".atx")
                || ["gdbtable", "gdbtablx", "gdbindexes", "spx", "freelist"]
                    .iter()
                    .any(|x| name == format!("{prefix}{x}")));
        if own {
            std::fs::remove_file(e.path()).map_err(|er| Error::io(e.path(), er))?;
        }
    }
    for e in std::fs::read_dir(&scratch).map_err(|e| Error::io(&scratch, e))? {
        let e = e.map_err(|er| Error::io(&scratch, er))?;
        let to = dir.join(e.file_name());
        std::fs::rename(e.path(), &to).map_err(|er| Error::io(&to, er))?;
    }
    std::fs::remove_dir_all(&scratch).ok();
    Ok(n)
}
