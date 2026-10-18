//! One description of each tool, shared by every front end.
//!
//! A [`Tool`] carries the ArcGIS name, toolbox, summary and typed parameters of a geoprocessing
//! tool together with the function that runs it. The command line (`gf run`), the application's Tools
//! pane and the MCP/Python bindings are built from this list, so a tool added here shows up in all
//! of them without its own wiring (older tools still have a command and an application wrapper of
//! their own).

use geoforge_core::{Error, FeatureClass, Result};

use crate::construct::{self, Fishnet, Placement};
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
    /// The coordinate system new data (a fishnet) gets when no template names one.
    fn default_spatial_reference_wkt(&self) -> Option<String> {
        None
    }
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
        run: densify,
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
        run: generate_points_along_lines,
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
        run: create_fishnet,
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
        run: polygon_to_line,
    },
];
