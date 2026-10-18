//! One description of each tool, shared by every front end.
//!
//! A [`Tool`] carries the ArcGIS name, toolbox, summary and typed parameters of a geoprocessing
//! tool together with the function that runs it. The command line (`gf run`), the application's Tools
//! pane and the MCP/Python bindings are built from this list, so a tool added here shows up in all
//! of them without its own wiring (older tools still have a command and an application wrapper of
//! their own).

use geoforge_core::{Error, FeatureClass, Result};

use crate::{ToolOutput, spatstat};

/// What a parameter holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A feature layer (a path on the command line, a layer in the application).
    Features,
    /// A field of the layer parameter `of`; `numeric` restricts the choice.
    Field {
        of: &'static str,
        numeric: bool,
    },
    Choice(&'static [&'static str]),
    Number,
    Text,
    Bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Param {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub required: bool,
    /// The value used when none is given (empty for none).
    pub default: &'static str,
    pub help: &'static str,
}

/// How a tool reads its parameters; each front end implements it over its own storage.
pub trait Context {
    /// The parameter's value as text, when it was given and is not empty.
    fn get(&self, name: &str) -> Option<String>;
    /// The feature layer named by the parameter.
    fn features(&self, name: &str) -> Result<Box<dyn FeatureClass + '_>>;
}

pub struct Tool {
    pub id: &'static str,
    pub name: &'static str,
    /// `Toolbox › Toolset`.
    pub toolbox: &'static str,
    pub summary: &'static str,
    pub params: &'static [Param],
    /// The name of the result when none is given: `<input>_<suffix>`.
    pub output_suffix: &'static str,
    /// Whether the tool measures distances or areas: an application holding longitude/latitude
    /// data then runs it in the data's UTM zone, as ArcGIS does on projected data.
    pub measures: bool,
    pub run: fn(&dyn Context) -> Result<ToolOutput>,
}

pub fn find(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id.eq_ignore_ascii_case(id))
}

/// A parameter's value, or its default.
pub fn value(ctx: &dyn Context, p: &Param) -> Option<String> {
    ctx.get(p.name)
        .or_else(|| (!p.default.is_empty()).then(|| p.default.to_string()))
}

/// Checks `ctx` against the tool's parameters: required ones are there, choices are among the
/// options (case-insensitively), numbers parse.
pub fn validate(tool: &Tool, ctx: &dyn Context) -> Result<()> {
    for p in tool.params {
        let v = value(ctx, p);
        let Some(v) = v else {
            if p.required {
                return Err(Error::Unsupported(format!(
                    "ERROR 000735: {}: value is required",
                    p.label
                )));
            }
            continue;
        };
        match p.kind {
            Kind::Choice(options) if !options.iter().any(|o| o.eq_ignore_ascii_case(&v)) => {
                return Err(Error::Unsupported(format!(
                    "ERROR 000067: {}: {v} is not one of {}",
                    p.label,
                    options.join(", ")
                )));
            }
            Kind::Number if v.trim().parse::<f64>().is_err() => {
                return Err(Error::Unsupported(format!(
                    "ERROR 000094: {}: {v} is not a number",
                    p.label
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Runs `tool` after checking the parameters.
pub fn run(tool: &Tool, ctx: &dyn Context) -> Result<ToolOutput> {
    validate(tool, ctx)?;
    (tool.run)(ctx)
}

fn text(ctx: &dyn Context, tool: &Tool, name: &str) -> Option<String> {
    tool.params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| value(ctx, p))
}

const INPUT: Param = Param {
    name: "input",
    label: "Input Feature Class",
    kind: Kind::Features,
    required: true,
    default: "",
    help: "",
};

const fn field(
    name: &'static str,
    label: &'static str,
    numeric: bool,
    help: &'static str,
) -> Param {
    Param {
        name,
        label,
        kind: Kind::Field {
            of: "input",
            numeric,
        },
        required: false,
        default: "",
        help,
    }
}

const WEIGHT: Param = field(
    "weight_field",
    "Weight Field",
    true,
    "A numeric field that weighs the features.",
);
const CASE: Param = field(
    "case_field",
    "Case Field",
    false,
    "One result per value of this field.",
);
const DIMENSION: Param = field(
    "dimension_field",
    "Dimension Field",
    true,
    "Its (weighted) mean is added to the result.",
);

const SIZES: &[&str] = &[
    "1_STANDARD_DEVIATION",
    "2_STANDARD_DEVIATIONS",
    "3_STANDARD_DEVIATIONS",
];

fn mean_center(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("mean_center").ok_or_else(|| Error::Unsupported("mean_center".into()))?;
    let fc = ctx.features("input")?;
    spatstat::mean_center(
        fc.as_ref(),
        text(ctx, t, "weight_field").as_deref(),
        text(ctx, t, "case_field").as_deref(),
        text(ctx, t, "dimension_field").as_deref(),
    )
}

fn median_center(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("median_center").ok_or_else(|| Error::Unsupported("median_center".into()))?;
    let fc = ctx.features("input")?;
    spatstat::median_center(
        fc.as_ref(),
        text(ctx, t, "weight_field").as_deref(),
        text(ctx, t, "case_field").as_deref(),
        text(ctx, t, "dimension_field").as_deref(),
    )
}

fn central_feature(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("central_feature").ok_or_else(|| Error::Unsupported("central_feature".into()))?;
    let fc = ctx.features("input")?;
    let manhattan = text(ctx, t, "distance_method")
        .is_some_and(|m| m.eq_ignore_ascii_case("MANHATTAN_DISTANCE"));
    spatstat::central_feature(
        fc.as_ref(),
        manhattan,
        text(ctx, t, "weight_field").as_deref(),
        text(ctx, t, "self_potential_weight_field").as_deref(),
        text(ctx, t, "case_field").as_deref(),
    )
}

fn standard_distance(ctx: &dyn Context) -> Result<ToolOutput> {
    let t =
        find("standard_distance").ok_or_else(|| Error::Unsupported("standard_distance".into()))?;
    let fc = ctx.features("input")?;
    spatstat::standard_distance(
        fc.as_ref(),
        &text(ctx, t, "circle_size").unwrap_or_default(),
        text(ctx, t, "weight_field").as_deref(),
        text(ctx, t, "case_field").as_deref(),
    )
}

fn directional_distribution(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("directional_distribution")
        .ok_or_else(|| Error::Unsupported("directional_distribution".into()))?;
    let fc = ctx.features("input")?;
    spatstat::directional_distribution(
        fc.as_ref(),
        &text(ctx, t, "ellipse_size").unwrap_or_default(),
        text(ctx, t, "weight_field").as_deref(),
        text(ctx, t, "case_field").as_deref(),
    )
}

fn directional_mean(ctx: &dyn Context) -> Result<ToolOutput> {
    let t =
        find("directional_mean").ok_or_else(|| Error::Unsupported("directional_mean".into()))?;
    let fc = ctx.features("input")?;
    let orientation = text(ctx, t, "orientation_only")
        .is_some_and(|m| m.eq_ignore_ascii_case("ORIENTATION_ONLY"));
    spatstat::linear_directional_mean(
        fc.as_ref(),
        orientation,
        text(ctx, t, "case_field").as_deref(),
    )
}

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "mean_center",
        name: "Mean Center",
        toolbox: "Spatial Statistics › Measuring Geographic Distributions",
        summary: "The geographic center of features, optionally weighted (ArcGIS Pro Mean Center).",
        params: &[INPUT, WEIGHT, CASE, DIMENSION],
        output_suffix: "MeanCenter",
        measures: true,
        run: mean_center,
    },
    Tool {
        id: "median_center",
        name: "Median Center",
        toolbox: "Spatial Statistics › Measuring Geographic Distributions",
        summary: "The point with the smallest total distance to the features (ArcGIS Pro Median Center; approximate).",
        params: &[INPUT, WEIGHT, CASE, DIMENSION],
        output_suffix: "MedianCenter",
        measures: true,
        run: median_center,
    },
    Tool {
        id: "central_feature",
        name: "Central Feature",
        toolbox: "Spatial Statistics › Measuring Geographic Distributions",
        summary: "The feature closest to all the others (ArcGIS Pro Central Feature).",
        params: &[
            INPUT,
            Param {
                name: "distance_method",
                label: "Distance Method",
                kind: Kind::Choice(&["EUCLIDEAN_DISTANCE", "MANHATTAN_DISTANCE"]),
                required: false,
                default: "EUCLIDEAN_DISTANCE",
                help: "",
            },
            WEIGHT,
            field(
                "self_potential_weight_field",
                "Self Potential Weight Field",
                true,
                "Added to a feature's own total, times its weight.",
            ),
            CASE,
        ],
        output_suffix: "CentralFeature",
        measures: true,
        run: central_feature,
    },
    Tool {
        id: "standard_distance",
        name: "Standard Distance",
        toolbox: "Spatial Statistics › Measuring Geographic Distributions",
        summary: "A circle around the mean center whose radius is the standard distance (ArcGIS Pro Standard Distance).",
        params: &[
            INPUT,
            Param {
                name: "circle_size",
                label: "Circle Size",
                kind: Kind::Choice(SIZES),
                required: false,
                default: "1_STANDARD_DEVIATION",
                help: "",
            },
            WEIGHT,
            CASE,
        ],
        output_suffix: "StandardDistance",
        measures: true,
        run: standard_distance,
    },
    Tool {
        id: "directional_distribution",
        name: "Directional Distribution",
        toolbox: "Spatial Statistics › Measuring Geographic Distributions",
        summary: "The standard deviational ellipse of features (ArcGIS Pro Directional Distribution).",
        params: &[
            INPUT,
            Param {
                name: "ellipse_size",
                label: "Ellipse Size",
                kind: Kind::Choice(SIZES),
                required: false,
                default: "1_STANDARD_DEVIATION",
                help: "",
            },
            WEIGHT,
            CASE,
        ],
        output_suffix: "DirectionalDistribution",
        measures: true,
        run: directional_distribution,
    },
    Tool {
        id: "directional_mean",
        name: "Linear Directional Mean",
        toolbox: "Spatial Statistics › Measuring Geographic Distributions",
        summary: "The mean direction of lines, with a test of uniformity (ArcGIS Pro Linear Directional Mean).",
        params: &[
            Param {
                name: "input",
                label: "Input Feature Class",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Lines.",
            },
            Param {
                name: "orientation_only",
                label: "Orientation Only",
                kind: Kind::Choice(&["DIRECTION", "ORIENTATION_ONLY"]),
                required: false,
                default: "DIRECTION",
                help: "ORIENTATION_ONLY ignores which way each line points.",
            },
            CASE,
        ],
        output_suffix: "DirectionalMean",
        measures: true,
        run: directional_mean,
    },
];
