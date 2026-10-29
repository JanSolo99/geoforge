//! Feature Class To Shapefile: a batch of feature classes written as shapefiles into a folder.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, Result};

pub const TOOL: Tool = Tool {
    id: "feature_class_to_shapefile",
    name: "Feature Class To Shapefile",
    toolbox: "Conversion › To Shapefile",
    summary: "Writes feature classes as shapefiles into a folder, named after their inputs (_1, _2 on collisions).",
    params: &[
        Param {
            name: "input_features",
            label: "Input Features",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated feature class paths; optional single quotes around each path.",
        },
        Param {
            name: "output_folder",
            label: "Output Folder",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "An existing folder. Existing shapefiles are kept; new names get _1, _2, etc.",
        },
    ],
    output_suffix: "FeatureClassToShapefile",
    measures: false,
    run: Runner::Report(run),
};

/// Feature Class To Shapefile: each input is converted as Feature Class To Feature Class does it
/// and written as `<input name>.shp` in the folder, or `<name>_1.shp`, `_2`, ... when the name
/// is taken (observed against ArcGIS Pro 3.7). Every input is opened before anything is written.
fn run(ctx: &dyn Context) -> Result<Report> {
    let input = ctx.get("input_features").unwrap_or_default();
    let inputs: Vec<String> = input
        .split(';')
        .map(|p| p.trim().trim_matches('\'').to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    if inputs.is_empty() {
        return Err(Error::Unsupported(
            "ERROR 000735: Input Features: Value is required".into(),
        ));
    }
    let folder = ctx.get("output_folder").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Output Folder: Value is required".into())
    })?;
    let names = ctx.export_shapefiles(&inputs, &folder)?;
    Ok(Report {
        rows: vec![("Shapefiles written".into(), names.len() as f64)],
        messages: names,
    })
}
