//! Host-owned in-place editor tracking edits; common parameter parsing is shared.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, Result};

pub const DISABLE: Tool = Tool {
    id: "disable_editor_tracking",
    name: "Disable Editor Tracking",
    toolbox: "Data Management › Database",
    summary: "Disables selected editor tracking roles while retaining existing fields and values.",
    params: &[
        Param {
            name: "input",
            label: "Input Dataset",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "FileGDB feature class, table or feature dataset to update in place.",
        },
        Param {
            name: "creator",
            label: "Disable Creator Tracking",
            kind: Kind::Bool,
            required: false,
            default: "true",
            help: "DISABLE_CREATOR or NO_DISABLE_CREATOR; true/false also accepted.",
        },
        Param {
            name: "creation_date",
            label: "Disable Creation Date Tracking",
            kind: Kind::Bool,
            required: false,
            default: "true",
            help: "DISABLE_CREATION_DATE or NO_DISABLE_CREATION_DATE; true/false also accepted.",
        },
        Param {
            name: "last_editor",
            label: "Disable Last Editor Tracking",
            kind: Kind::Bool,
            required: false,
            default: "true",
            help: "DISABLE_LAST_EDITOR or NO_DISABLE_LAST_EDITOR; true/false also accepted.",
        },
        Param {
            name: "last_edit_date",
            label: "Disable Last Edit Date Tracking",
            kind: Kind::Bool,
            required: false,
            default: "true",
            help: "DISABLE_LAST_EDIT_DATE or NO_DISABLE_LAST_EDIT_DATE; true/false also accepted.",
        },
    ],
    output_suffix: "tracking_disabled",
    measures: false,
    run: Runner::Report(disable),
};
fn disable(ctx: &dyn Context) -> Result<Report> {
    let mut flags = [true; 4];
    for (i, param) in DISABLE.params[1..].iter().enumerate() {
        if let Some(value) = ctx.get(param.name) {
            let yes = format!("DISABLE_{}", param.name.to_ascii_uppercase());
            let no = format!("NO_{yes}");
            flags[i] = match value.trim().to_ascii_uppercase().as_str() {
                "TRUE" | "1" => true,
                "FALSE" | "0" => false,
                v if v == yes => true,
                v if v == no => false,
                _ => {
                    return Err(Error::Unsupported(format!(
                        "ERROR 000800: {}: expected {yes} or {no}",
                        param.label
                    )));
                }
            };
        }
    }
    ctx.disable_editor_tracking("input", flags)?;
    Ok(Report {
        rows: vec![],
        messages: vec!["Editor tracking updated".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::FeatureClass;
    struct Host;
    impl Context for Host {
        fn get(&self, _: &str) -> Option<String> {
            None
        }
        fn features(&self, _: &str) -> Result<Box<dyn FeatureClass + '_>> {
            unreachable!()
        }
    }
    #[test]
    fn unsupported_host_refuses_mutation() {
        assert!(
            disable(&Host)
                .unwrap_err()
                .to_string()
                .contains("editor tracking edits are not supported by this host")
        );
    }
}
