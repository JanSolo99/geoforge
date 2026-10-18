//! One description of each tool, shared by every front end.
//!
//! A [`Tool`] carries the ArcGIS name, toolbox, summary and typed parameters of a geoprocessing
//! tool together with the function that runs it. The command line (`gf run`), the application's Tools
//! pane and the MCP/Python bindings are built from this list, so a tool added here shows up in all
//! of them without its own wiring (older tools still have a command and an application wrapper of
//! their own).

use geoforge_core::{Error, FeatureClass, Result};

use crate::construct::{self, Fishnet, Placement};
use crate::{TableOutput, ToolOutput, spatstat, tables};

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
    /// The coordinate system new data (a fishnet) gets when no template names one.
    fn default_spatial_reference_wkt(&self) -> Option<String> {
        None
    }
}

/// What a tool without a feature output reports: named numbers, and messages.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub rows: Vec<(String, f64)>,
    pub messages: Vec<String>,
}

/// How a tool runs: to a new feature class, or to a report of values.
#[derive(Clone, Copy)]
pub enum Runner {
    Features(fn(&dyn Context) -> Result<ToolOutput>),
    Report(fn(&dyn Context) -> Result<Report>),
    /// Gives features or a table, as the input decides.
    Dynamic(fn(&dyn Context) -> Result<Outcome>),
}

/// What a run gave.
pub enum Outcome {
    Features(ToolOutput),
    Table(TableOutput),
    Report(Report),
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
    pub run: Runner,
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
pub fn run(tool: &Tool, ctx: &dyn Context) -> Result<Outcome> {
    validate(tool, ctx)?;
    match tool.run {
        Runner::Features(f) => f(ctx).map(Outcome::Features),
        Runner::Report(f) => f(ctx).map(Outcome::Report),
        Runner::Dynamic(f) => f(ctx),
    }
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

const LINEAR_UNITS: &[(&str, f64)] = &[
    ("METERS", 1.0),
    ("METER", 1.0),
    ("KILOMETERS", 1000.0),
    ("KILOMETER", 1000.0),
    ("CENTIMETERS", 0.01),
    ("MILLIMETERS", 0.001),
    ("DECIMETERS", 0.1),
    ("FEET", 0.3048),
    ("FOOT", 0.3048),
    ("INCHES", 0.0254),
    ("YARDS", 0.9144),
    ("MILES", 1609.344),
    ("NAUTICALMILES", 1852.0),
];

/// Meters per coordinate unit of a projected coordinate system (`None` for geographic ones).
fn meters_per_unit(wkt: &str) -> Option<f64> {
    if !wkt.trim_start().to_ascii_uppercase().starts_with("PROJCS") {
        return None;
    }
    let at = wkt.rfind("UNIT[")?;
    wkt[at..]
        .split(',')
        .nth(1)?
        .trim_end_matches([']', ')', ' '])
        .trim()
        .parse()
        .ok()
}

/// A distance as `25`, `25 Meters` or `0.5 Kilometers`, in coordinate units.
fn distance_in_units(text: &str, wkt: Option<&str>) -> Result<f64> {
    let t = text.trim();
    let split = t
        .find(|c: char| c.is_alphabetic() && c != 'e' && c != 'E')
        .unwrap_or(t.len());
    let (num, unit) = t.split_at(split);
    let value: f64 = num
        .trim()
        .parse()
        .map_err(|_| Error::Unsupported(format!("ERROR 000094: '{text}' is not a distance")))?;
    let unit = unit.trim().to_ascii_uppercase().replace([' ', '_'], "");
    if unit.is_empty() || unit == "UNKNOWN" {
        return Ok(value);
    }
    let meters = LINEAR_UNITS
        .iter()
        .find(|(n, _)| *n == unit)
        .map(|u| u.1)
        .ok_or_else(|| {
            Error::Unsupported(format!("ERROR 000094: unknown distance unit '{unit}'"))
        })?;
    let per_unit = wkt.and_then(meters_per_unit).ok_or_else(|| {
        Error::Unsupported(
            "ERROR 000800: a distance with a unit needs a projected coordinate system".into(),
        )
    })?;
    Ok(value * meters / per_unit)
}

fn pair(text: &str, what: &str) -> Result<[f64; 2]> {
    let v: Vec<f64> = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|w| !w.is_empty())
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| {
            Error::Unsupported(format!("ERROR 000094: {what} '{text}': expected 'x y'"))
        })?;
    match v.as_slice() {
        [x, y] if x.is_finite() && y.is_finite() => Ok([*x, *y]),
        _ => Err(Error::Unsupported(format!(
            "ERROR 000094: {what} '{text}': expected 'x y'"
        ))),
    }
}

fn densify(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("densify").ok_or_else(|| Error::Unsupported("densify".into()))?;
    let fc = ctx.features("input")?;
    let d = text(ctx, t, "distance")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    construct::densify(fc.as_ref(), d)
}

fn generate_points_along_lines(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("generate_points_along_lines")
        .ok_or_else(|| Error::Unsupported("generate_points_along_lines".into()))?;
    let fc = ctx.features("input")?;
    let placement =
        if text(ctx, t, "point_placement").is_some_and(|p| p.eq_ignore_ascii_case("PERCENTAGE")) {
            Placement::Percentage(
                text(ctx, t, "percentage")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0),
            )
        } else {
            let d = text(ctx, t, "distance").ok_or_else(|| {
                Error::Unsupported("ERROR 000735: Distance: value is required".into())
            })?;
            Placement::Distance(distance_in_units(&d, fc.spatial_reference_wkt())?)
        };
    let ends =
        text(ctx, t, "include_end_points").is_some_and(|p| p.eq_ignore_ascii_case("END_POINTS"));
    construct::generate_points_along_lines(fc.as_ref(), placement, ends)
}

fn create_fishnet(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("create_fishnet").ok_or_else(|| Error::Unsupported("create_fishnet".into()))?;
    let num = |k: &str| {
        text(ctx, t, k)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let f = Fishnet {
        origin: pair(
            &text(ctx, t, "origin_coord").unwrap_or_default(),
            "Fishnet Origin Coordinate",
        )?,
        y_axis: pair(
            &text(ctx, t, "y_axis_coord").unwrap_or_default(),
            "Y-Axis Coordinate",
        )?,
        cell: [num("cell_width"), num("cell_height")],
        rows: num("number_rows").max(0.0) as usize,
        columns: num("number_columns").max(0.0) as usize,
        corner: text(ctx, t, "corner_coord")
            .map(|c| pair(&c, "Opposite Corner of Fishnet"))
            .transpose()?,
        polylines: text(ctx, t, "geometry_type")
            .is_some_and(|g| g.eq_ignore_ascii_case("POLYLINE")),
    };
    let wkt = match text(ctx, t, "template") {
        Some(_) => ctx
            .features("template")?
            .spatial_reference_wkt()
            .map(str::to_owned),
        None => ctx.default_spatial_reference_wkt(),
    };
    construct::create_fishnet(&f, wkt)
}

fn polygon_to_line(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("polygon_to_line").ok_or_else(|| Error::Unsupported("polygon_to_line".into()))?;
    if text(ctx, t, "neighbor_option").is_some_and(|n| n.eq_ignore_ascii_case("IDENTIFY_NEIGHBORS"))
    {
        return Err(Error::Unsupported(
            "IDENTIFY_NEIGHBORS is not done yet: use IGNORE_NEIGHBORS".into(),
        ));
    }
    let fc = ctx.features("input")?;
    construct::polygon_to_line(fc.as_ref())
}

const CONCEPTS: &[&str] = &[
    "INVERSE_DISTANCE",
    "INVERSE_DISTANCE_SQUARED",
    "FIXED_DISTANCE_BAND",
    "ZONE_OF_INDIFFERENCE",
    "K_NEAREST_NEIGHBORS",
];
const DISTANCE_METHODS: &[&str] = &["EUCLIDEAN_DISTANCE", "MANHATTAN_DISTANCE"];

fn number_of(ctx: &dyn Context, t: &Tool, name: &str) -> f64 {
    text(ctx, t, name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0)
}

fn distance_method(ctx: &dyn Context, t: &Tool) -> Result<spatstat::DistanceMethod> {
    let m = text(ctx, t, "distance_method").unwrap_or_default();
    spatstat::DistanceMethod::parse(&m)
        .ok_or_else(|| Error::Unsupported(format!("distance method {m}")))
}

fn concept(ctx: &dyn Context, t: &Tool, neighbors: usize) -> Result<spatstat::Concept> {
    let c = text(ctx, t, "conceptualization").unwrap_or_default();
    spatstat::Concept::parse(&c, neighbors)
        .ok_or_else(|| Error::Unsupported(format!("conceptualization {c}")))
}

fn average_nearest_neighbor(ctx: &dyn Context) -> Result<Report> {
    let t = find("average_nearest_neighbor")
        .ok_or_else(|| Error::Unsupported("average_nearest_neighbor".into()))?;
    let fc = ctx.features("input")?;
    spatstat::average_nearest_neighbor(
        fc.as_ref(),
        distance_method(ctx, t)?,
        number_of(ctx, t, "area"),
    )
}

fn spatial_autocorrelation(ctx: &dyn Context) -> Result<Report> {
    let t = find("spatial_autocorrelation")
        .ok_or_else(|| Error::Unsupported("spatial_autocorrelation".into()))?;
    let fc = ctx.features("input")?;
    let row = text(ctx, t, "standardization").is_some_and(|s| s.eq_ignore_ascii_case("ROW"));
    spatstat::spatial_autocorrelation(
        fc.as_ref(),
        &text(ctx, t, "field").unwrap_or_default(),
        concept(ctx, t, 8)?,
        distance_method(ctx, t)?,
        row,
        number_of(ctx, t, "distance_band"),
    )
}

fn hot_spot_analysis(ctx: &dyn Context) -> Result<ToolOutput> {
    let t =
        find("hot_spot_analysis").ok_or_else(|| Error::Unsupported("hot_spot_analysis".into()))?;
    let fc = ctx.features("input")?;
    let k = number_of(ctx, t, "number_of_neighbors").max(1.0) as usize;
    let fdr = text(ctx, t, "apply_fdr").is_some_and(|s| s.eq_ignore_ascii_case("APPLY_FDR"));
    spatstat::hot_spot_analysis(
        fc.as_ref(),
        &text(ctx, t, "field").unwrap_or_default(),
        concept(ctx, t, k)?,
        distance_method(ctx, t)?,
        number_of(ctx, t, "distance_band"),
        fdr,
    )
}

fn where_of(ctx: &dyn Context, t: &Tool) -> Option<String> {
    text(ctx, t, "where_clause")
}

fn copy_rows(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("copy_rows").ok_or_else(|| Error::Unsupported("copy_rows".into()))?;
    let fc = ctx.features("input")?;
    tables::copy_rows(fc.as_ref(), where_of(ctx, t).as_deref()).map(Outcome::Table)
}

fn table_to_table(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("table_to_table").ok_or_else(|| Error::Unsupported("table_to_table".into()))?;
    let fc = ctx.features("input")?;
    tables::copy_rows(fc.as_ref(), where_of(ctx, t).as_deref()).map(Outcome::Table)
}

fn get_count(ctx: &dyn Context) -> Result<Report> {
    let t = find("get_count").ok_or_else(|| Error::Unsupported("get_count".into()))?;
    let fc = ctx.features("input")?;
    let n = tables::get_count(fc.as_ref(), where_of(ctx, t).as_deref())?;
    Ok(Report {
        rows: vec![("Row Count".into(), n as f64)],
        messages: Vec::new(),
    })
}

fn join_field(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("join_field").ok_or_else(|| Error::Unsupported("join_field".into()))?;
    let fc = ctx.features("input")?;
    let join = ctx.features("join_table")?;
    let fields: Vec<String> = text(ctx, t, "fields")
        .map(|f| {
            f.split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    match tables::join_field(
        fc.as_ref(),
        &text(ctx, t, "in_field").unwrap_or_default(),
        join.as_ref(),
        &text(ctx, t, "join_field").unwrap_or_default(),
        &fields,
    )? {
        tables::Joined::Features(o) => Ok(Outcome::Features(o)),
        tables::Joined::Table(o) => Ok(Outcome::Table(o)),
    }
}

fn add_fields(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("add_fields").ok_or_else(|| Error::Unsupported("add_fields".into()))?;
    let fc = ctx.features("input")?;
    tables::add_fields(fc.as_ref(), &text(ctx, t, "fields").unwrap_or_default())
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
        run: Runner::Features(mean_center),
    },
    Tool {
        id: "median_center",
        name: "Median Center",
        toolbox: "Spatial Statistics › Measuring Geographic Distributions",
        summary: "The point with the smallest total distance to the features (ArcGIS Pro Median Center; approximate).",
        params: &[INPUT, WEIGHT, CASE, DIMENSION],
        output_suffix: "MedianCenter",
        measures: true,
        run: Runner::Features(median_center),
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
        run: Runner::Features(central_feature),
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
        run: Runner::Features(standard_distance),
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
        run: Runner::Features(directional_distribution),
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
        run: Runner::Features(directional_mean),
    },
    Tool {
        id: "densify",
        name: "Densify",
        toolbox: "Editing",
        summary: "Adds vertices along the segments of lines and polygons, at most a distance apart (ArcGIS Pro Densify; the copy, not the input, is edited).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Lines or polygons.",
            },
            Param {
                name: "distance",
                label: "Distance",
                kind: Kind::Number,
                required: true,
                default: "",
                help: "In the coordinate system's units.",
            },
        ],
        output_suffix: "Densify",
        measures: false,
        run: Runner::Features(densify),
    },
    Tool {
        id: "generate_points_along_lines",
        name: "Generate Points Along Lines",
        toolbox: "Data Management › Features",
        summary: "Points at a distance or a percentage along lines or polygon boundaries (ArcGIS Pro Generate Points Along Lines).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Lines or polygons.",
            },
            Param {
                name: "point_placement",
                label: "Point Placement",
                kind: Kind::Choice(&["DISTANCE", "PERCENTAGE"]),
                required: false,
                default: "DISTANCE",
                help: "",
            },
            Param {
                name: "distance",
                label: "Distance",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "For example `25 Meters`.",
            },
            Param {
                name: "percentage",
                label: "Percentage",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "Between 0 and 100.",
            },
            Param {
                name: "include_end_points",
                label: "Include End Points",
                kind: Kind::Choice(&["NO_END_POINTS", "END_POINTS"]),
                required: false,
                default: "NO_END_POINTS",
                help: "",
            },
        ],
        output_suffix: "Points",
        measures: true,
        run: Runner::Features(generate_points_along_lines),
    },
    Tool {
        id: "create_fishnet",
        name: "Create Fishnet",
        toolbox: "Data Management › Features",
        summary: "A grid of rectangular cells or lines from an origin (ArcGIS Pro Create Fishnet, without labels).",
        params: &[
            Param {
                name: "origin_coord",
                label: "Fishnet Origin Coordinate",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "x y",
            },
            Param {
                name: "y_axis_coord",
                label: "Y-Axis Coordinate",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "x y: a point along the grid's y-axis.",
            },
            Param {
                name: "cell_width",
                label: "Cell Size Width",
                kind: Kind::Number,
                required: false,
                default: "0",
                help: "",
            },
            Param {
                name: "cell_height",
                label: "Cell Size Height",
                kind: Kind::Number,
                required: false,
                default: "0",
                help: "",
            },
            Param {
                name: "number_rows",
                label: "Number of Rows",
                kind: Kind::Number,
                required: false,
                default: "0",
                help: "",
            },
            Param {
                name: "number_columns",
                label: "Number of Columns",
                kind: Kind::Number,
                required: false,
                default: "0",
                help: "",
            },
            Param {
                name: "corner_coord",
                label: "Opposite Corner of Fishnet",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "x y: replaces the cell size.",
            },
            Param {
                name: "geometry_type",
                label: "Geometry Type",
                kind: Kind::Choice(&["POLYGON", "POLYLINE"]),
                required: false,
                default: "POLYGON",
                help: "",
            },
            Param {
                name: "template",
                label: "Template Extent",
                kind: Kind::Features,
                required: false,
                default: "",
                help: "Its coordinate system is the fishnet's.",
            },
        ],
        output_suffix: "Fishnet",
        measures: false,
        run: Runner::Features(create_fishnet),
    },
    Tool {
        id: "polygon_to_line",
        name: "Polygon To Line",
        toolbox: "Data Management › Features",
        summary: "The boundaries of polygons as lines (ArcGIS Pro Polygon To Line; IGNORE_NEIGHBORS).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Polygons.",
            },
            Param {
                name: "neighbor_option",
                label: "Identify and store polygon neighboring information",
                kind: Kind::Choice(&["IGNORE_NEIGHBORS", "IDENTIFY_NEIGHBORS"]),
                required: false,
                default: "IGNORE_NEIGHBORS",
                help: "IDENTIFY_NEIGHBORS is not done yet.",
            },
        ],
        output_suffix: "Line",
        measures: false,
        run: Runner::Features(polygon_to_line),
    },
    Tool {
        id: "average_nearest_neighbor",
        name: "Average Nearest Neighbor",
        toolbox: "Spatial Statistics › Analyzing Patterns",
        summary: "The ratio of observed to expected mean nearest-neighbor distance, with its z-score (ArcGIS Pro Average Nearest Neighbor).",
        params: &[
            INPUT,
            Param {
                name: "distance_method",
                label: "Distance Method",
                kind: Kind::Choice(DISTANCE_METHODS),
                required: false,
                default: "EUCLIDEAN_DISTANCE",
                help: "",
            },
            Param {
                name: "area",
                label: "Area",
                kind: Kind::Number,
                required: false,
                default: "0",
                help: "The study area; 0 is the minimum enclosing rectangle of the features.",
            },
        ],
        output_suffix: "AverageNearestNeighbor",
        measures: true,
        run: Runner::Report(average_nearest_neighbor),
    },
    Tool {
        id: "spatial_autocorrelation",
        name: "Spatial Autocorrelation (Global Moran's I)",
        toolbox: "Spatial Statistics › Analyzing Patterns",
        summary: "Whether the values of a field cluster, disperse or are random in space (ArcGIS Pro Spatial Autocorrelation; polygon contiguity is not done).",
        params: &[
            INPUT,
            field("field", "Input Field", true, "The numeric field to test."),
            Param {
                name: "conceptualization",
                label: "Conceptualization of Spatial Relationships",
                kind: Kind::Choice(CONCEPTS),
                required: false,
                default: "INVERSE_DISTANCE",
                help: "",
            },
            Param {
                name: "distance_method",
                label: "Distance Method",
                kind: Kind::Choice(DISTANCE_METHODS),
                required: false,
                default: "EUCLIDEAN_DISTANCE",
                help: "",
            },
            Param {
                name: "standardization",
                label: "Standardization",
                kind: Kind::Choice(&["NONE", "ROW"]),
                required: false,
                default: "NONE",
                help: "",
            },
            Param {
                name: "distance_band",
                label: "Distance Band or Threshold Distance",
                kind: Kind::Number,
                required: false,
                default: "0",
                help: "In the coordinate system's units; 0 relates every pair for inverse distance.",
            },
        ],
        output_suffix: "SpatialAutocorrelation",
        measures: true,
        run: Runner::Report(spatial_autocorrelation),
    },
    Tool {
        id: "hot_spot_analysis",
        name: "Hot Spot Analysis (Getis-Ord Gi*)",
        toolbox: "Spatial Statistics › Mapping Clusters",
        summary: "Where features with high or low values cluster, as z-scores and confidence bins (ArcGIS Pro Hot Spot Analysis).",
        params: &[
            INPUT,
            field("field", "Input Field", true, "The numeric field to test."),
            Param {
                name: "conceptualization",
                label: "Conceptualization of Spatial Relationships",
                kind: Kind::Choice(CONCEPTS),
                required: false,
                default: "INVERSE_DISTANCE",
                help: "",
            },
            Param {
                name: "distance_method",
                label: "Distance Method",
                kind: Kind::Choice(DISTANCE_METHODS),
                required: false,
                default: "EUCLIDEAN_DISTANCE",
                help: "",
            },
            Param {
                name: "standardization",
                label: "Standardization",
                kind: Kind::Choice(&["NONE", "ROW"]),
                required: false,
                default: "NONE",
                help: "",
            },
            Param {
                name: "distance_band",
                label: "Distance Band or Threshold Distance",
                kind: Kind::Number,
                required: false,
                default: "0",
                help: "In the coordinate system's units; 0 relates every pair for inverse distance.",
            },
            Param {
                name: "number_of_neighbors",
                label: "Number of Neighbors",
                kind: Kind::Number,
                required: false,
                default: "8",
                help: "For K_NEAREST_NEIGHBORS.",
            },
            Param {
                name: "apply_fdr",
                label: "Apply False Discovery Rate (FDR) Correction",
                kind: Kind::Choice(&["NO_FDR", "APPLY_FDR"]),
                required: false,
                default: "NO_FDR",
                help: "",
            },
        ],
        output_suffix: "HotSpots",
        measures: true,
        run: Runner::Features(hot_spot_analysis),
    },
    Tool {
        id: "copy_rows",
        name: "Copy Rows",
        toolbox: "Data Management › Tables",
        summary: "Copies the rows of a table or the attributes of a feature class into a new table (ArcGIS Pro Copy Rows).",
        params: &[Param {
            name: "input",
            label: "Input Rows",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "",
        }],
        output_suffix: "Rows",
        measures: false,
        run: Runner::Dynamic(copy_rows),
    },
    Tool {
        id: "table_to_table",
        name: "Table To Table",
        toolbox: "Conversion › To Geodatabase",
        summary: "Copies the rows a where clause selects into a new table (ArcGIS Pro Table To Table).",
        params: &[
            Param {
                name: "input",
                label: "Input Rows",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "where_clause",
                label: "Expression",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "An SQL where clause.",
            },
        ],
        output_suffix: "Table",
        measures: false,
        run: Runner::Dynamic(table_to_table),
    },
    Tool {
        id: "get_count",
        name: "Get Count",
        toolbox: "Data Management › Tables",
        summary: "The number of rows of a table or layer, or that an expression selects (ArcGIS Pro Get Count).",
        params: &[
            Param {
                name: "input",
                label: "Input Rows",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "where_clause",
                label: "Expression",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "An SQL where clause.",
            },
        ],
        output_suffix: "Count",
        measures: false,
        run: Runner::Report(get_count),
    },
    Tool {
        id: "join_field",
        name: "Join Field",
        toolbox: "Data Management › Joins",
        summary: "Adds the fields of a table to another by a common field; the copy, not the input, gets them (ArcGIS Pro Join Field).",
        params: &[
            Param {
                name: "input",
                label: "Input Table",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            field(
                "in_field",
                "Input Join Field",
                false,
                "The field that matches the join table.",
            ),
            Param {
                name: "join_table",
                label: "Join Table",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "join_field",
                label: "Join Table Field",
                kind: Kind::Field {
                    of: "join_table",
                    numeric: false,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "fields",
                label: "Transfer Fields",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Field names separated by semicolons; none: all of them.",
            },
        ],
        output_suffix: "Join",
        measures: false,
        run: Runner::Dynamic(join_field),
    },
    Tool {
        id: "add_fields",
        name: "Add Fields",
        toolbox: "Data Management › Fields",
        summary: "Adds several fields to a copy of a feature class (ArcGIS Pro Add Fields, shapefile rules).",
        params: &[
            Param {
                name: "input",
                label: "Input Table",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "fields",
                label: "Field Description",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "NAME TYPE [LENGTH] items separated by semicolons; TYPE is TEXT, SHORT, LONG, FLOAT, DOUBLE or DATE.",
            },
        ],
        output_suffix: "Fields",
        measures: false,
        run: Runner::Features(add_fields),
    },
];
