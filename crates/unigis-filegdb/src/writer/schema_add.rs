//! Additive field/domain edits, using one journal for the whole operation.
use super::{
    FieldAlteration, GeodatabaseWriter, alter_field, assign_domain_to_field, rewrite_table, txn,
};
use crate::{Domain, Geodatabase};
use std::path::Path;
use unigis_core::{Error, Field, Result, Value};

/// Adds absent fields, retaining existing definitions/values, and creates the requested
/// domains. Conflicting existing domains are refused before any edit. One journal covers
/// domains, rows, schema and relationships; the original data survives failure.
pub fn add_schema_fields(
    dir: &Path,
    item: &str,
    requested: &[Field],
    domains: &[Domain],
) -> Result<()> {
    txn::atomic(dir, || {
        let gdb = Geodatabase::open(dir)?;
        let base = gdb.table_base(item)?;
        let table = gdb.open_table(item)?;
        if table.geometry_kind().is_none() {
            return Err(Error::Unsupported(
                "ERROR 000732: Input Features: dataset is not supported".into(),
            ));
        }
        for domain in domains {
            if let Some(old) = gdb
                .domains()
                .iter()
                .find(|d| d.name.eq_ignore_ascii_case(&domain.name))
                && (old.field_type != domain.field_type || old.kind != domain.kind)
            {
                return Err(Error::Unsupported(
                    "ERROR 000355: Invalid coded value domain (existing metadata domain conflicts)"
                        .into(),
                ));
            }
        }
        let added: Vec<_> = requested
            .iter()
            .filter(|f| {
                !table
                    .all_fields()
                    .iter()
                    .any(|old| old.name.eq_ignore_ascii_case(&f.name))
            })
            .cloned()
            .collect();
        for f in &added {
            super::validate_field_name(&f.name)?;
            if !f.nullable || f.default.is_some() {
                return Err(Error::Unsupported(
                    "additive metadata fields must be nullable without defaults".into(),
                ));
            }
        }
        let missing_domains: Vec<_> = domains
            .iter()
            .filter(|d| {
                !gdb.domains()
                    .iter()
                    .any(|old| old.name.eq_ignore_ascii_case(&d.name))
            })
            .collect();
        if added.is_empty() && missing_domains.is_empty() {
            return Ok(());
        }
        let mut fields = table.fields().to_vec();
        fields.extend(added.iter().cloned());
        let rows = table
            .features()
            .map(|row| {
                let mut row = row?;
                row.attributes
                    .extend(std::iter::repeat_n(Value::Null, added.len()));
                Ok((Some(row.oid as u64), row))
            })
            .collect::<Result<Vec<_>>>()?;
        drop(table);
        drop(gdb);
        if !missing_domains.is_empty() {
            let mut writer = GeodatabaseWriter::open_existing(dir)?;
            for d in missing_domains {
                writer.add_domain(d)?;
            }
            writer.finish()?;
        }
        if !added.is_empty() {
            rewrite_table(&base, &fields, &rows)?;
            for f in &added {
                if let Some(domain) = &f.domain {
                    assign_domain_to_field(dir, item, &f.name, domain, &[])?;
                    // ArcGIS uses GPFieldInfoEx's AliasName once a domain entry exists,
                    // overriding the physical field alias. Keep both representations.
                    if let Some(alias) = &f.alias {
                        alter_field(
                            dir,
                            item,
                            &f.name,
                            &FieldAlteration {
                                alias: Some(alias),
                                ..Default::default()
                            },
                        )?;
                    }
                }
            }
        }
        Ok(())
    })
}
