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
pub const ENABLE: Tool = Tool {
    id: "enable_editor_tracking",
    name: "Enable Editor Tracking",
    toolbox: "Data Management › Database",
    summary: "Enables selected editor tracking roles, optionally adding missing fields; retains existing values.",
    params: &[
        Param {
            name: "input",
            label: "Input Dataset",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Simple FileGDB feature class or table.",
        },
        Param {
            name: "creator_field",
            label: "Creator Field",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "String field for creator.",
        },
        Param {
            name: "creation_date_field",
            label: "Creation Date Field",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Low precision date field.",
        },
        Param {
            name: "last_editor_field",
            label: "Last Editor Field",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "String field for last editor.",
        },
        Param {
            name: "last_edit_date_field",
            label: "Last Edit Date Field",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Low precision date field.",
        },
        Param {
            name: "add_fields",
            label: "Add Fields",
            kind: Kind::Bool,
            required: false,
            default: "false",
            help: "ADD_FIELDS or NO_ADD_FIELDS; true/false also accepted.",
        },
        Param {
            name: "record_dates_in",
            label: "Record Dates In",
            kind: Kind::Choice(&["UTC", "DATABASE_TIME"]),
            required: false,
            default: "UTC",
            help: "Time basis when initially enabling tracking.",
        },
    ],
    output_suffix: "tracking_enabled",
    measures: false,
    run: Runner::Report(enable),
};
fn enable(ctx: &dyn Context) -> Result<Report> {
    let fields = [
        "creator_field",
        "creation_date_field",
        "last_editor_field",
        "last_edit_date_field",
    ]
    .map(|p| ctx.get(p).filter(|v| v != "#").unwrap_or_default());
    let add = match ctx
        .get("add_fields")
        .unwrap_or_else(|| "NO_ADD_FIELDS".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "ADD_FIELDS" | "TRUE" | "1" => true,
        "NO_ADD_FIELDS" | "FALSE" | "0" => false,
        _ => {
            return Err(Error::Unsupported(
                "ERROR 000800: expected ADD_FIELDS or NO_ADD_FIELDS".into(),
            ));
        }
    };
    let utc = ctx
        .get("record_dates_in")
        .is_none_or(|v| v.eq_ignore_ascii_case("UTC"));
    ctx.enable_editor_tracking("input", fields, add, utc)?;
    Ok(Report {
        rows: vec![],
        messages: vec!["Editor tracking enabled".into()],
    })
}
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
            enable(&Host)
                .unwrap_err()
                .to_string()
                .contains("editor tracking edits are not supported by this host")
        );
        assert!(
            disable(&Host)
                .unwrap_err()
                .to_string()
                .contains("editor tracking edits are not supported by this host")
        );
    }
}
