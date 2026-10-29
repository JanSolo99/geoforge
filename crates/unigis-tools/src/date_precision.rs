//! Host capability for in-place Date field precision migration.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::Result;
pub const MIGRATE: Tool = Tool {
    id: "migrate_date_field_to_high_precision",
    name: "Migrate Date Field To High Precision",
    toolbox: "Data Management › Date and Time",
    summary: "Migrates existing geodatabase Date fields to millisecond precision while retaining their values.",
    params: &[
        Param {
            name: "input",
            label: "Input Table",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "FileGDB table or feature class to update in place.",
        },
        Param {
            name: "date_fields",
            label: "Date Fields",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated Date field names.",
        },
    ],
    output_suffix: "dates_high_precision",
    measures: false,
    run: Runner::Report(migrate),
};
fn migrate(ctx: &dyn Context) -> Result<Report> {
    let fields: Vec<_> = ctx
        .get("date_fields")
        .unwrap_or_default()
        .split(';')
        .map(|s| s.trim().trim_matches(['\'', '"']).to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    ctx.migrate_date_fields("input", &fields)?;
    Ok(Report {
        rows: vec![],
        messages: vec!["Date fields migrated to high precision".into()],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::{FeatureClass, Result};
    #[test]
    fn readonly_host_does_not_claim_migration() {
        struct Host;
        impl Context for Host {
            fn get(&self, _: &str) -> Option<String> {
                None
            }
            fn features(&self, _: &str) -> Result<Box<dyn FeatureClass + '_>> {
                unreachable!()
            }
        }
        assert!(
            migrate(&Host)
                .unwrap_err()
                .to_string()
                .contains("date precision edits are not supported by this host")
        );
    }
}
