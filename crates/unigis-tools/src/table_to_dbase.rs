//! Table To dBASE: a batch of tables written as DBF files into a folder.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, Result};

pub const TOOL: Tool = Tool {
    id: "table_to_dbase",
    name: "Table To dBASE",
    toolbox: "Conversion › To dBASE",
    summary: "Writes tables as dBASE files into a folder, named after their inputs.",
    params: &[
        Param {
            name: "input_tables",
            label: "Input Tables",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated table, feature class or CSV paths; optional single quotes around each path.",
        },
        Param {
            name: "output_folder",
            label: "Output Folder",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "An existing folder for the output .dbf tables.",
        },
    ],
    output_suffix: "TableToDBASE",
    measures: false,
    run: Runner::Report(run),
};

fn run(ctx: &dyn Context) -> Result<Report> {
    let input = ctx.get("input_tables").unwrap_or_default();
    let inputs: Vec<String> = input
        .split(';')
        .map(|p| p.trim().trim_matches('\'').to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    if inputs.is_empty() {
        return Err(Error::Unsupported(
            "ERROR 000735: Input Tables: Value is required".into(),
        ));
    }
    let folder = ctx.get("output_folder").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Output Folder: Value is required".into())
    })?;
    let names = ctx.export_dbase(&inputs, &folder)?;
    Ok(Report {
        rows: vec![("dBASE tables written".into(), names.len() as f64)],
        messages: names,
    })
}
