//! Feature Class To Geodatabase: writable-host batch import.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, Result};
pub const TOOL: Tool = Tool {
    id: "feature_class_to_geodatabase",
    name: "Feature Class To Geodatabase",
    toolbox: "Conversion › To Geodatabase",
    summary: "Imports feature classes into an existing FileGDB or feature dataset, allocating unique names.",
    params: &[
        Param {
            name: "input_features",
            label: "Input Features",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated feature-class paths; optional single quotes around each path.",
        },
        Param {
            name: "output_geodatabase",
            label: "Output Geodatabase",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Existing FileGDB or feature dataset. Existing classes are preserved; names get _1, _2, etc.",
        },
    ],
    output_suffix: "FeatureClassToGeodatabase",
    measures: false,
    run: Runner::Report(run),
};
fn run(ctx: &dyn Context) -> Result<Report> {
    let input = ctx.get("input_features").unwrap_or_default();
    let inputs: Vec<_> = input
        .split(';')
        .map(|p| p.trim().trim_matches('\'').to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    if inputs.is_empty() {
        return Err(Error::Unsupported(
            "ERROR 000735: Input Features: Value is required".into(),
        ));
    }
    let target = ctx.get("output_geodatabase").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Output Geodatabase: Value is required".into())
    })?;
    let names = ctx.import_feature_classes(&inputs, &target)?;
    Ok(Report {
        rows: vec![("Feature classes imported".into(), names.len() as f64)],
        messages: names,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::FeatureClass;
    #[test]
    fn unsupported_host_refuses_before_reporting_success() {
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
            Host.import_feature_classes(&["source.shp".into()], "target.gdb")
                .unwrap_err()
                .to_string()
                .contains("not supported by this host")
        );
    }
}
