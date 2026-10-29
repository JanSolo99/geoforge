//! Host dispatch for Extract Data From Geodatabase.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, Result};

pub struct Request {
    pub inputs: Vec<String>,
    pub destination: String,
    pub create_new: bool,
    pub schema_only: bool,
    pub expand: String,
    pub related: bool,
    pub all_table_rows: bool,
}

pub const TOOL: Tool = Tool {
    id: "extract_data_from_geodatabase",
    name: "Extract Data From Geodatabase",
    toolbox: "Data Management › Distributed Geodatabase",
    summary: "Extracts FileGDB classes, tables, domains and simple relationships, with data or schema only.",
    params: &[
        Param {
            name: "in_data",
            label: "Datasets to Extract",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated FileGDB dataset paths; quote paths containing spaces.",
        },
        Param {
            name: "extract_type",
            label: "Extract",
            kind: Kind::Choice(&["DATA", "SCHEMA_ONLY"]),
            required: false,
            default: "DATA",
            help: "Data and schema, or schema only.",
        },
        Param {
            name: "out_type",
            label: "Output Type",
            kind: Kind::Choice(&[
                "GEODATABASE",
                "NEW_FILE_GEODATABASE",
                "XML_FILE",
                "NEW_MOBILE_GEODATABASE",
            ]),
            required: false,
            default: "GEODATABASE",
            help: "Existing or new FileGDB; XML and mobile outputs are unsupported.",
        },
        Param {
            name: "out_geodatabase",
            label: "Geodatabase to Extract Data to",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Existing destination FileGDB.",
        },
        Param {
            name: "out_xml",
            label: "XML File to Extract Data to",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "XML output is unsupported.",
        },
        Param {
            name: "out_folder_path",
            label: "Geodatabase Location",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Existing folder for a new FileGDB.",
        },
        Param {
            name: "out_name",
            label: "Geodatabase Name",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Name of a new FileGDB, with optional .gdb suffix.",
        },
        Param {
            name: "expand_feature_classes_and_tables",
            label: "Expand Feature Classes and Tables",
            kind: Kind::Choice(&[
                "USE_DEFAULTS",
                "ADD_WITH_SCHEMA_ONLY",
                "ALL_ROWS",
                "DO_NOT_ADD",
            ]),
            required: false,
            default: "USE_DEFAULTS",
            help: "How related datasets are expanded.",
        },
        Param {
            name: "reuse_schema",
            label: "Reuse Schema",
            kind: Kind::Choice(&["DO_NOT_REUSE", "REUSE"]),
            required: false,
            default: "DO_NOT_REUSE",
            help: "REUSE is unsupported; destination dataset names must be unused.",
        },
        Param {
            name: "get_related_data",
            label: "Extract Related Data",
            kind: Kind::Choice(&["GET_RELATED", "DO_NOT_GET_RELATED"]),
            required: false,
            default: "GET_RELATED",
            help: "Extract rows linked by simple key relationships.",
        },
        Param {
            name: "extract_using_geometry_features",
            label: "Extract Using Geometry Features",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Geometry filters are unsupported.",
        },
        Param {
            name: "geometry_filter_type",
            label: "Geometry Filter Type",
            kind: Kind::Choice(&["INTERSECTS", "CONTAINS"]),
            required: false,
            default: "INTERSECTS",
            help: "Used only with a geometry filter.",
        },
        Param {
            name: "all_records_for_tables",
            label: "All records for tables",
            kind: Kind::Choice(&[
                "SCHEMA_ONLY_FOR_TABLES",
                "ALL_RECORDS_FOR_TABLES",
                "true",
                "false",
            ]),
            required: false,
            default: "SCHEMA_ONLY_FOR_TABLES",
            help: "Include all unfiltered table rows; otherwise schema only, plus related rows.",
        },
    ],
    output_suffix: "extracted",
    measures: false,
    run: Runner::Report(run),
};
fn value(ctx: &dyn Context, name: &str, default: &str) -> String {
    ctx.get(name).unwrap_or_else(|| default.into())
}
fn required(ctx: &dyn Context, name: &str) -> Result<String> {
    ctx.get(name)
        .ok_or_else(|| Error::Unsupported(format!("ERROR 000735: {name}: Value is required")))
}
fn run(ctx: &dyn Context) -> Result<Report> {
    // Reuse the established quoted ArcGIS multi-value parser, but reject type columns.
    let inputs = crate::delete_multiple::parse(&required(ctx, "in_data")?)?;
    if inputs.iter().any(|i| i.data_type.is_some()) {
        return Err(Error::Unsupported(
            "datasets require paths without type columns; quote paths containing spaces".into(),
        ));
    }
    if value(ctx, "reuse_schema", "DO_NOT_REUSE").eq_ignore_ascii_case("REUSE")
        || ctx.get("extract_using_geometry_features").is_some()
    {
        return Err(Error::Unsupported(
            "schema reuse and geometry filters are not supported for extraction".into(),
        ));
    }
    let out_type = value(ctx, "out_type", "GEODATABASE").to_ascii_uppercase();
    let create_new = out_type == "NEW_FILE_GEODATABASE";
    let destination = if create_new {
        let folder = required(ctx, "out_folder_path")?;
        let mut name = required(ctx, "out_name")?;
        if name.contains(['/', '\\']) || name == "." || name == ".." {
            return Err(Error::Unsupported(
                "out_name must be a geodatabase name, without a path".into(),
            ));
        }
        if !name.to_ascii_lowercase().ends_with(".gdb") {
            name.push_str(".gdb");
        }
        format!("{}/{name}", folder.trim_end_matches(['/', '\\']))
    } else if out_type == "GEODATABASE" {
        required(ctx, "out_geodatabase")?
    } else {
        return Err(Error::Unsupported(
            "extraction supports FileGDB destinations only".into(),
        ));
    };
    let request = Request {
        inputs: inputs.into_iter().map(|i| i.path).collect(),
        destination,
        create_new,
        schema_only: value(ctx, "extract_type", "DATA").eq_ignore_ascii_case("SCHEMA_ONLY"),
        expand: value(ctx, "expand_feature_classes_and_tables", "USE_DEFAULTS")
            .to_ascii_uppercase(),
        related: value(
            ctx,
            "get_related_data",
            if value(ctx, "expand_feature_classes_and_tables", "USE_DEFAULTS")
                .eq_ignore_ascii_case("ADD_WITH_SCHEMA_ONLY")
            {
                "DO_NOT_GET_RELATED"
            } else {
                "GET_RELATED"
            },
        )
        .eq_ignore_ascii_case("GET_RELATED"),
        all_table_rows: matches!(
            value(ctx, "all_records_for_tables", "SCHEMA_ONLY_FOR_TABLES")
                .to_ascii_uppercase()
                .as_str(),
            "ALL_RECORDS_FOR_TABLES" | "TRUE"
        ),
    };
    ctx.extract_data(&request)?;
    Ok(Report {
        rows: vec![],
        messages: vec![format!("Extracted data to {}", request.destination)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::FeatureClass;
    #[test]
    fn host_without_workspace_capability_refuses_extraction() {
        struct Host;
        impl Context for Host {
            fn get(&self, name: &str) -> Option<String> {
                match name {
                    "in_data" => Some("data.gdb/sites".into()),
                    "out_geodatabase" => Some("out.gdb".into()),
                    _ => None,
                }
            }
            fn features(&self, _: &str) -> Result<Box<dyn FeatureClass + '_>> {
                unreachable!()
            }
        }
        assert!(
            run(&Host)
                .unwrap_err()
                .to_string()
                .contains("geodatabase extraction is not supported by this host")
        );
    }
}
