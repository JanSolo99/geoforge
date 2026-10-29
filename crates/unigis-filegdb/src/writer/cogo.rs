//! Disable COGO as observed through public ArcGIS tools and saved file states.
use super::{
    catalog::base,
    subtype_edit::{Splice, apply, bad, child_text, load_target, store},
    txn,
};
use crate::table::Table;
use roxmltree::Document;
use std::path::Path;
use unigis_core::{GeometryKind, Result};

/// COGO values/fields stay in place; only their catalog Required markers are removed.
/// No user table, geometry, index, field alias, default or domain is rewritten.
pub fn disable_cogo(dir: impl AsRef<Path>, item: &str) -> Result<()> {
    let dir = dir.as_ref();
    txn::atomic(dir, || {
        let target = load_target(dir, item)?;
        let table = Table::open(base(dir, target.table_id))?;
        match table.geometry_kind() {
            Some(GeometryKind::Polyline) => {}
            None => {
                return Err(bad(
                    "ERROR 000732: Input Line Features: dataset does not exist or is not supported",
                ));
            }
            _ => {
                return Err(bad(
                    "ERROR 003917: The feature class geometry is not of type polyline",
                ));
            }
        }
        drop(table);
        let doc = Document::parse(&target.def)
            .map_err(|e| bad(format!("invalid feature class definition: {e}")))?;
        let mut edits = Vec::new();
        for entry in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoEx")
        {
            if child_text(entry, "Name").is_some_and(|name| {
                ["Direction", "Distance", "Radius", "ArcLength", "Radius2"]
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&name))
            }) {
                for required in entry
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "Required")
                {
                    edits.push(Splice {
                        at: required.range(),
                        with: String::new(),
                    });
                }
            }
        }
        if edits.is_empty() {
            return Ok(());
        }
        store(dir, &target, apply(&target.def, edits))
    })
}
