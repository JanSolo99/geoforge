//! In-place GlobalID edits supplied by writable hosts.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::Result;
pub const TOOL: Tool = Tool {
    id: "add_global_ids",
    name: "Add Global IDs",
    toolbox: "Data Management › Database",
    summary: "Adds global identifiers to geodatabase feature classes, tables and feature datasets.",
    params: &[Param {
        name: "in_datasets",
        label: "Input Datasets",
        kind: Kind::Text,
        required: true,
        default: "",
        help: "Semicolon-separated datasets in one FileGDB; updated in place.",
    }],
    output_suffix: "global_ids",
    measures: false,
    run: Runner::Report(run),
};
fn run(ctx: &dyn Context) -> Result<Report> {
    ctx.add_global_ids("in_datasets")?;
    Ok(Report {
        rows: vec![],
        messages: vec!["Global IDs added".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::FeatureClass;
    #[test]
    fn read_only_host_refuses_mutation() {
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
            run(&Host)
                .unwrap_err()
                .to_string()
                .contains("not supported by this host")
        );
    }
}
