//! One description of each tool, shared by every front end.
//!
//! A [`Tool`] carries the ArcGIS name, toolbox, summary and typed parameters of a geoprocessing
//! tool together with the function that runs it. The command line (`ug run`), the application's Tools
//! pane and the MCP/Python bindings are built from this list, so a tool added here shows up in all
//! of them without its own wiring (older tools still have a command and an application wrapper of
//! their own).

use unigis_core::{Error, FeatureClass, Field, FieldType, Result, Value};

use crate::construct::{self, Fishnet, Placement};
use crate::planar;
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

/// One named dataset to write into a host-supported workspace.
pub enum WorkspaceOutput {
    Features(ToolOutput),
    Table(TableOutput),
}

/// A domain needed by an additive schema edit. Existing compatible domains are retained.
pub struct SchemaDomain {
    pub name: &'static str,
    pub description: &'static str,
    pub field_type: FieldType,
    pub kind: SchemaDomainKind,
}
pub enum SchemaDomainKind {
    Coded(Vec<(Value, String)>),
    Range(Value, Value),
}

/// Tool parameters, input access and optional workspace output supplied by a host.
pub trait Context {
    /// The parameter's value as text, when it was given and is not empty.
    fn get(&self, name: &str) -> Option<String>;
    /// The feature layer named by the parameter.
    fn features(&self, name: &str) -> Result<Box<dyn FeatureClass + '_>>;
    /// Write precomputed partitions. Hosts without filesystem/workspace support
    /// must fail explicitly; a successful report must never imply unwritten data.
    fn write_workspace(
        &self,
        _workspace: &str,
        _outputs: Vec<(String, WorkspaceOutput)>,
    ) -> Result<()> {
        Err(Error::Unsupported(
            "workspace outputs are not supported by this host".into(),
        ))
    }
    /// Add fields/domains to an existing dataset. A read-only host must fail explicitly.
    fn add_schema_fields(
        &self,
        _input: &str,
        _fields: &[Field],
        _domains: &[SchemaDomain],
    ) -> Result<()> {
        Err(Error::Unsupported(
            "schema edits are not supported by this host".into(),
        ))
    }
    /// Delete explicitly named data items. Read-only hosts must refuse.
    fn delete_items(&self, _items: &[crate::delete_multiple::DeleteItem]) -> Result<()> {
        Err(Error::Unsupported(
            "dataset deletion is not supported by this host".into(),
        ))
    }
    /// Enable COGO in the named input; read-only hosts fail explicitly.
    fn enable_cogo(&self, _input: &str) -> Result<()> {
        Err(Error::Unsupported(
            "COGO edits are not supported by this host".into(),
        ))
    }
    /// Disable COGO in the named input; hosts without mutation support fail explicitly.
    fn disable_cogo(&self, _input: &str) -> Result<()> {
        Err(Error::Unsupported(
            "COGO edits are not supported by this host".into(),
        ))
    }
    /// Migrate Date fields; mutation must be supplied by the host.
    fn migrate_date_fields(&self, _input: &str, _fields: &[String]) -> Result<()> {
        Err(Error::Unsupported(
            "date precision edits are not supported by this host".into(),
        ))
    }
    /// Enable tracking roles; mutation must be supplied by the host.
    fn enable_editor_tracking(
        &self,
        _input: &str,
        _fields: [String; 4],
        _add_fields: bool,
        _utc: bool,
    ) -> Result<()> {
        Err(Error::Unsupported(
            "editor tracking edits are not supported by this host".into(),
        ))
    }
    /// Disable selected tracking roles; read-only hosts must explicitly refuse.
    fn disable_editor_tracking(&self, _input: &str, _disable: [bool; 4]) -> Result<()> {
        Err(Error::Unsupported(
            "editor tracking edits are not supported by this host".into(),
        ))
    }
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
    /// Features plus tables, each written where the named parameter says (when it is given).
    FeaturesAndTables(ToolOutput, Vec<(&'static str, TableOutput)>),
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

pub(crate) const LINEAR_UNITS: &[(&str, f64)] = &[
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
pub fn distance_in_units(text: &str, wkt: Option<&str>) -> Result<f64> {
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

/// An area in coordinate units squared; bare values already use those units.
pub(crate) fn area_in_units(text: &str, wkt: Option<&str>) -> Result<f64> {
    let text = text.trim();
    let split = text
        .find(|c: char| c.is_alphabetic() && c != 'e' && c != 'E')
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number
        .trim()
        .parse()
        .map_err(|_| Error::Unsupported(format!("ERROR 000094: '{text}' is not an area")))?;
    let unit = unit.trim().to_ascii_uppercase().replace([' ', '_'], "");
    if unit.is_empty() || unit == "UNKNOWN" {
        return Ok(value);
    }
    let square_foot_us = (1200.0 / 3937.0) * (1200.0 / 3937.0);
    let square_metres = match unit.as_str() {
        "SQUAREMETERS" => 1.0,
        "SQUAREKILOMETERS" => 1_000_000.0,
        "SQUARECENTIMETERS" => 0.0001,
        "SQUAREMILLIMETERS" => 0.000001,
        "SQUAREDECIMETERS" => 0.01,
        "HECTARES" => 10_000.0,
        "ARES" => 100.0,
        "ACRES" => 4046.8564224,
        "SQUAREFEET" => 0.09290304,
        "SQUAREYARDS" => 0.83612736,
        "SQUAREINCHES" => 0.00064516,
        // ArcGIS uses this rounded international square-mile conversion.
        "SQUAREMILES" => 2_589_988.11,
        "SQUARENAUTICALMILES" => 3_429_904.0,
        "SQUAREFEETUS" => square_foot_us,
        "SQUAREYARDSUS" => 9.0 * square_foot_us,
        "SQUAREINCHESUS" => square_foot_us / 144.0,
        "ACRESUS" => 43_560.0 * square_foot_us,
        "SQUAREMILESUS" => 27_878_400.0 * square_foot_us,
        _ => {
            return Err(Error::Unsupported(format!(
                "ERROR 000094: unknown area unit '{unit}'"
            )));
        }
    };
    // ArcGIS treats an unknown coordinate system as meters for explicit units.
    let per_unit = match wkt {
        None => 1.0,
        Some(wkt) => meters_per_unit(wkt).filter(|u| u.is_finite() && *u > 0.0).ok_or_else(|| {
            Error::Unsupported("ERROR 000800: an area with a unit needs a projected or unknown coordinate system".into())
        })?,
    };
    Ok(value * square_metres / (per_unit * per_unit))
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
    let chainage =
        text(ctx, t, "add_chainage_fields").is_some_and(|p| p.eq_ignore_ascii_case("ADD_CHAINAGE"));
    construct::generate_points_along_lines(fc.as_ref(), placement, ends, chainage)
}

fn generate_transects_along_lines(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("generate_transects_along_lines")
        .ok_or_else(|| Error::Unsupported("generate_transects_along_lines".into()))?;
    let fc = ctx.features("input")?;
    let wkt = fc.spatial_reference_wkt();
    let get = |name: &str, label: &str| -> Result<f64> {
        let v = text(ctx, t, name).ok_or_else(|| {
            Error::Unsupported(format!("ERROR 000735: {label}: value is required"))
        })?;
        distance_in_units(&v, wkt)
    };
    let ends =
        text(ctx, t, "include_end_points").is_some_and(|p| p.eq_ignore_ascii_case("END_POINTS"));
    construct::generate_transects_along_lines(
        fc.as_ref(),
        get("interval", "Interval")?,
        get("transect_length", "Transect Length")?,
        ends,
    )
}

fn calculate_geometry_attributes(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("calculate_geometry_attributes")
        .ok_or_else(|| Error::Unsupported("calculate_geometry_attributes".into()))?;
    let input = ctx.features("input")?;
    let pairs: Vec<(String, String)> = text(ctx, t, "geometry_property")
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Geometry Attributes: value is required".into())
        })?
        .split(';')
        .filter_map(|item| {
            let mut w = item.split_whitespace();
            Some((w.next()?.to_owned(), w.next()?.to_owned()))
        })
        .collect();
    let unit = |k: &str| {
        text(ctx, t, k)
            .map(|u| u.trim().to_ascii_uppercase())
            .unwrap_or_default()
    };
    let length = match unit("length_unit").as_str() {
        "" | "METERS" => 1.0,
        "KILOMETERS" => 1000.0,
        "FEET_INT" => 0.3048,
        "FEET_US" => 1200.0 / 3937.0,
        "YARDS_INT" => 0.9144,
        "YARDS_US" => 3600.0 / 3937.0,
        "MILES_INT" => 1609.344,
        "MILES_US" => 6_336_000.0 / 3937.0,
        "NAUTICAL_MILES_INT" => 1852.0,
        other => return Err(Error::Unsupported(format!("length unit {other}"))),
    };
    let area = match unit("area_unit").as_str() {
        "" | "SQUARE_METERS" => 1.0,
        "SQUARE_KILOMETERS" => 1e6,
        "HECTARES" => 1e4,
        "ACRES" => 4046.8564224,
        "SQUARE_FEET_INT" => 0.09290304,
        "SQUARE_YARDS_INT" => 0.83612736,
        "SQUARE_MILES_INT" => 2_589_988.110336,
        other => return Err(Error::Unsupported(format!("area unit {other}"))),
    };
    crate::calc_geometry::calculate_geometry_attributes(input.as_ref(), &pairs, length, area)
}

fn adjust_3d_z(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("adjust_3d_z").ok_or_else(|| Error::Unsupported("adjust_3d_z".into()))?;
    let input = ctx.features("input")?;
    let (adjust, from, to) = (
        text(ctx, t, "adjust_value"),
        text(ctx, t, "from_units"),
        text(ctx, t, "to_units"),
    );
    crate::adjust_z::adjust_3d_z(
        input.as_ref(),
        &crate::adjust_z::AdjustZ {
            reverse: text(ctx, t, "reverse_sign")
                .is_some_and(|r| r.eq_ignore_ascii_case("REVERSE")),
            adjust: adjust.as_deref(),
            from_units: from.as_deref(),
            to_units: to.as_deref(),
        },
    )
}

fn validate_join(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("validate_join").ok_or_else(|| Error::Unsupported("validate_join".into()))?;
    let need = |k: &str| text(ctx, t, k).ok_or_else(|| Error::Unsupported(format!("missing {k}")));
    let (in_path, join_path) = (need("input")?, need("join_table")?);
    let same = in_path == join_path
        || std::fs::canonicalize(&in_path)
            .ok()
            .zip(std::fs::canonicalize(&join_path).ok())
            .is_some_and(|(a, b)| a == b);
    let (input, join) = (ctx.features("input")?, ctx.features("join_table")?);
    let (in_field, join_field) = (need("in_field")?, need("join_field")?);
    let (_, table) = crate::validate_join::validate_join(
        &crate::validate_join::JoinSide {
            table: input.as_ref(),
            field: &in_field,
            name: crate::validate_join::table_name(&in_path),
        },
        &crate::validate_join::JoinSide {
            table: join.as_ref(),
            field: &join_field,
            name: crate::validate_join::table_name(&join_path),
        },
        same,
    )?;
    Ok(Outcome::Table(table))
}

fn convert_time_zone(ctx: &dyn Context) -> Result<Outcome> {
    let t =
        find("convert_time_zone").ok_or_else(|| Error::Unsupported("convert_time_zone".into()))?;
    let input = ctx.features("input")?;
    let need = |k: &str| text(ctx, t, k).ok_or_else(|| Error::Unsupported(format!("missing {k}")));
    let adjusted =
        |k: &str| !text(ctx, t, k).is_some_and(|v| v.to_ascii_uppercase().contains("NOT_ADJUSTED"));
    let out = crate::timezone::convert_time_zone(
        input.as_ref(),
        &need("input_time_field")?,
        &need("input_time_zone")?,
        &need("output_time_field")?,
        &need("output_time_zone")?,
        adjusted("input_dst"),
        adjusted("output_dst"),
    )?;
    Ok(if input.geometry_kind().is_some() {
        Outcome::Features(out)
    } else {
        Outcome::Table(TableOutput {
            fields: out.fields,
            rows: out.features.into_iter().map(|f| f.attributes).collect(),
        })
    })
}

fn convert_time_field(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("convert_time_field")
        .ok_or_else(|| Error::Unsupported("convert_time_field".into()))?;
    let input = ctx.features("input")?;
    let need = |k: &str| text(ctx, t, k).ok_or_else(|| Error::Unsupported(format!("missing {k}")));
    let out = crate::temporal::convert_time_field(
        input.as_ref(),
        &need("input_time_field")?,
        text(ctx, t, "input_time_format").as_deref(),
        &need("output_time_field")?,
        &text(ctx, t, "output_time_type").unwrap_or_else(|| "DATE".into()),
        text(ctx, t, "output_time_format").as_deref(),
    )?;
    Ok(if input.geometry_kind().is_some() {
        Outcome::Features(out)
    } else {
        Outcome::Table(TableOutput {
            fields: out.fields,
            rows: out.features.into_iter().map(|f| f.attributes).collect(),
        })
    })
}

fn batch_update_fields(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("batch_update_fields")
        .ok_or_else(|| Error::Unsupported("batch_update_fields".into()))?;
    let input = ctx.features("input")?;
    let table = ctx.features("field_definition_table")?;
    let column = |param: &str| -> Result<Option<usize>> {
        let Some(name) = text(ctx, t, param).filter(|n| !n.trim().is_empty()) else {
            return Ok(None);
        };
        table
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name.trim()))
            .map(Some)
            .ok_or_else(|| Error::Unsupported(format!("ERROR 000728: Field {name} does not exist")))
    };
    let name = column("output_field_name")?.ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Output Field Name: value is required".into())
    })?;
    let (source, kind, size, alias, script) = (
        column("source_field_name")?,
        column("output_field_type")?,
        column("output_field_decimals_or_length")?,
        column("output_field_alias")?,
        column("output_field_script")?,
    );
    let cell = |row: &[Value], i: Option<usize>| -> Option<String> {
        let v = &row[i?];
        let s = match v {
            Value::String(s) => s.trim().to_owned(),
            Value::Null => return None,
            other => crate::number(other).map(|n| n.to_string())?,
        };
        (!s.is_empty()).then_some(s)
    };
    let mut defs = Vec::new();
    for f in table.features() {
        let row = f?.attributes;
        let Some(n) = cell(&row, Some(name)) else {
            continue;
        };
        defs.push(crate::batch_fields::FieldDefinition {
            name: n,
            source: cell(&row, source),
            field_type: cell(&row, kind),
            size: cell(&row, size)
                .and_then(|s| s.parse::<f64>().ok())
                .map(|v| v as u32),
            alias: cell(&row, alias),
            script: cell(&row, script),
        });
    }
    let out = crate::batch_fields::batch_update_fields(input.as_ref(), &defs)?;
    Ok(if input.geometry_kind().is_some() {
        Outcome::Features(out)
    } else {
        Outcome::Table(TableOutput {
            fields: out.fields,
            rows: out.features.into_iter().map(|f| f.attributes).collect(),
        })
    })
}

fn add_spatial_join(ctx: &dyn Context) -> Result<ToolOutput> {
    let t =
        find("add_spatial_join").ok_or_else(|| Error::Unsupported("add_spatial_join".into()))?;
    let target = ctx.features("target_features")?;
    let join = ctx.features("join_features")?;
    let wkt = target.spatial_reference_wkt();
    let match_option = text(ctx, t, "match_option").unwrap_or_else(|| "INTERSECT".into());
    let opts = crate::SpatialJoinOptions {
        // The permanent fields are those of a one-to-one join of every target feature,
        // whatever the join operation and type (observed).
        one_to_many: false,
        keep_all: true,
        match_option: crate::MatchOption::parse(&match_option)
            .ok_or_else(|| Error::Unsupported(format!("match_option {match_option}")))?,
        search_radius: text(ctx, t, "search_radius")
            .filter(|r| !r.trim().is_empty())
            .map(|r| distance_in_units(&r, wkt))
            .transpose()?,
        distance_field_name: text(ctx, t, "distance_field_name").filter(|d| !d.trim().is_empty()),
        merge_rules: Vec::new(),
        field_map: None,
        target_name: String::new(),
        join_name: String::new(),
    };
    let mut out = crate::spatial_join(target.as_ref(), join.as_ref(), &opts)?;
    // Spatial Join writes Join_Count, the distance field, TARGET_FID, the target's fields and
    // the join's; the joined target has its own fields first, then Join_Count, the distance
    // field and the join's fields.
    let head = 2 + usize::from(opts.distance_field_name.is_some());
    let targets = target.fields().len();
    let order: Vec<usize> = (head..head + targets)
        .chain(0..head - 1)
        .chain(head + targets..out.fields.len())
        .collect();
    out.fields = order.iter().map(|&i| out.fields[i].clone()).collect();
    for f in &mut out.features {
        f.attributes = order.iter().map(|&i| f.attributes[i].clone()).collect();
    }
    Ok(out)
}

fn trim_line(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("trim_line").ok_or_else(|| Error::Unsupported("trim_line".into()))?;
    let fc = ctx.features("input")?;
    let wkt = fc.spatial_reference_wkt();
    let length = text(ctx, t, "dangle_length").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Dangle Length: value is required".into())
    })?;
    let length = distance_in_units(&length, wkt)?;
    let delete_short =
        !text(ctx, t, "delete_shorts").is_some_and(|m| m.eq_ignore_ascii_case("KEEP_SHORT"));
    crate::trim::trim_line(
        fc.as_ref(),
        length,
        delete_short,
        crate::default_xy_tolerance(wkt),
    )
}

fn extend_line(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("extend_line").ok_or_else(|| Error::Unsupported("extend_line".into()))?;
    let fc = ctx.features("input")?;
    let wkt = fc.spatial_reference_wkt();
    let length = text(ctx, t, "length")
        .ok_or_else(|| Error::Unsupported("ERROR 000735: Length: value is required".into()))?;
    let length = distance_in_units(&length, wkt)?;
    let to_extensions =
        text(ctx, t, "extend_to").is_some_and(|m| m.eq_ignore_ascii_case("EXTENSION"));
    crate::extend::extend_line(
        fc.as_ref(),
        length,
        to_extensions,
        crate::default_xy_tolerance(wkt),
    )
}

fn flip_line(ctx: &dyn Context) -> Result<ToolOutput> {
    construct::flip_line(ctx.features("input")?.as_ref())
}

#[cfg(not(feature = "proj"))]
fn convert_coordinate_notation(ctx: &dyn Context) -> Result<ToolOutput> {
    let _ = ctx;
    Err(Error::Unsupported(
        "Convert Coordinate Notation needs the proj feature".into(),
    ))
}

#[cfg(feature = "proj")]
fn convert_coordinate_notation(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("convert_coordinate_notation")
        .ok_or_else(|| Error::Unsupported("convert_coordinate_notation".into()))?;
    let need = |k: &str| text(ctx, t, k).ok_or_else(|| Error::Unsupported(format!("missing {k}")));
    let wgs84 = unigis_proj::Crs::parse("4326")?;
    construct::convert_coordinate_notation(
        ctx.features("input")?.as_ref(),
        &need("x_field")?,
        &text(ctx, t, "y_field").unwrap_or_default(),
        &need("input_format")?,
        &need("output_format")?,
        Some(wgs84.esri_wkt().to_owned()),
    )
}

#[cfg(not(feature = "proj"))]
fn table_to_ellipse(ctx: &dyn Context) -> Result<ToolOutput> {
    let _ = ctx;
    Err(Error::Unsupported(
        "Table To Ellipse needs the proj feature".into(),
    ))
}

#[cfg(feature = "proj")]
fn table_to_ellipse(ctx: &dyn Context) -> Result<ToolOutput> {
    let t =
        find("table_to_ellipse").ok_or_else(|| Error::Unsupported("table_to_ellipse".into()))?;
    let need = |k: &str| text(ctx, t, k).ok_or_else(|| Error::Unsupported(format!("missing {k}")));
    if text(ctx, t, "method").is_some_and(|m| !m.eq_ignore_ascii_case("planar")) {
        return Err(Error::Unsupported("geodesic ellipses".into()));
    }
    let crs = unigis_proj::Crs::parse(&need("spatial_reference")?)?;
    let wkt = crs.esri_wkt().to_owned();
    let unit = text(ctx, t, "distance_units").unwrap_or_else(|| "Meters".into());
    let scale = distance_in_units(&format!("1 {unit}"), Some(&wkt))?;
    let to_rad = match text(ctx, t, "azimuth_units")
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "MILS" => std::f64::consts::TAU / 6400.0,
        "RADIANS" => 1.0,
        "GRADIANS" => std::f64::consts::TAU / 400.0,
        _ => std::f64::consts::PI / 180.0,
    };
    let polygon = !text(ctx, t, "shape_type").is_some_and(|s| s.eq_ignore_ascii_case("line"));
    construct::table_to_ellipse(
        ctx.features("input")?.as_ref(),
        &need("x_field")?,
        &need("y_field")?,
        &need("major_field")?,
        &need("minor_field")?,
        scale,
        text(ctx, t, "azimuth_field").as_deref(),
        to_rad,
        polygon,
        Some(wkt),
    )
}

fn geodetic_densify(ctx: &dyn Context) -> Result<ToolOutput> {
    let t =
        find("geodetic_densify").ok_or_else(|| Error::Unsupported("geodetic_densify".into()))?;
    if text(ctx, t, "densify_method").is_some_and(|m| !m.eq_ignore_ascii_case("geodesic")) {
        return Err(Error::Unsupported(
            "densify methods other than Geodesic".into(),
        ));
    }
    let given = text(ctx, t, "max_segment_length")
        .ok_or_else(|| Error::Unsupported("a maximum segment length is required".into()))?;
    let given = given.trim();
    let split = given
        .find(|c: char| c.is_alphabetic() && c != 'e' && c != 'E')
        .unwrap_or(given.len());
    let (num, unit) = given.split_at(split);
    let value: f64 = num
        .trim()
        .parse()
        .map_err(|_| Error::Unsupported(format!("ERROR 000094: '{given}' is not a distance")))?;
    let unit = unit.trim().to_ascii_uppercase().replace([' ', '_'], "");
    let meters = if unit.is_empty() {
        1.0
    } else {
        LINEAR_UNITS
            .iter()
            .find(|(n, _)| *n == unit)
            .map(|u| u.1)
            .ok_or_else(|| Error::Unsupported(format!("unknown distance unit '{unit}'")))?
    };
    construct::geodetic_densify(ctx.features("input")?.as_ref(), value * meters)
}

fn encode_field(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("encode_field").ok_or_else(|| Error::Unsupported("encode_field".into()))?;
    let field = text(ctx, t, "field")
        .ok_or_else(|| Error::Unsupported("ERROR 000735: Field: value is required".into()))?;
    let method = text(ctx, t, "method").unwrap_or_else(|| "One-hot".into());
    tables::encode_field(ctx.features("input")?.as_ref(), &field, &method)
}

fn generalize(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("generalize").ok_or_else(|| Error::Unsupported("generalize".into()))?;
    let fc = ctx.features("input")?;
    let tolerance = match text(ctx, t, "tolerance") {
        Some(v) => distance_in_units(&v, fc.spatial_reference_wkt())?,
        None => 0.0,
    };
    construct::generalize(fc.as_ref(), tolerance)
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

fn generate_tessellation(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("generate_tessellation")
        .ok_or_else(|| Error::Unsupported("generate_tessellation".into()))?;
    let shape = text(ctx, t, "shape").unwrap_or_else(|| "HEXAGON".into());
    let shape = match shape.to_ascii_uppercase().as_str() {
        "SQUARE" => construct::Tessellation::Square,
        "DIAMOND" => construct::Tessellation::Diamond,
        "HEXAGON" => construct::Tessellation::Hexagon,
        "TRANSVERSE_HEXAGON" => construct::Tessellation::TransverseHexagon,
        "TRIANGLE" => construct::Tessellation::Triangle,
        other => {
            return Err(Error::Unsupported(format!(
                "tessellation shape {other} is not supported yet"
            )));
        }
    };
    let nums: Vec<f64> = text(ctx, t, "extent")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error::Unsupported("ERROR 000094: Extent: expected four numbers".into()))?;
    let [x0, y0, x1, y1] = nums[..] else {
        return Err(Error::Unsupported(
            "ERROR 000735: Extent: four numbers (xmin ymin xmax ymax) are required".into(),
        ));
    };
    let size = text(ctx, t, "size")
        .ok_or_else(|| Error::Unsupported("ERROR 000735: Size: value is required".into()))?;
    let wkt = match text(ctx, t, "template") {
        Some(_) => ctx
            .features("template")?
            .spatial_reference_wkt()
            .map(str::to_owned),
        None => ctx.default_spatial_reference_wkt(),
    };
    let area = area_in_units(&size, wkt.as_deref())?;
    construct::generate_tessellation(shape, [x0, y0, x1, y1], area, wkt)
}

fn polygon_to_line(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("polygon_to_line").ok_or_else(|| Error::Unsupported("polygon_to_line".into()))?;
    let fc = ctx.features("input")?;
    match text(ctx, t, "neighbor_option") {
        Some(n) if n.eq_ignore_ascii_case("IGNORE_NEIGHBORS") => {
            construct::polygon_to_line(fc.as_ref())
        }
        Some(n) if !n.eq_ignore_ascii_case("IDENTIFY_NEIGHBORS") => Err(Error::Unsupported(
            format!("ERROR 000800: neighbor_option {n}: IDENTIFY_NEIGHBORS or IGNORE_NEIGHBORS"),
        )),
        _ => planar::polygon_to_line_neighbors(fc.as_ref()),
    }
}

/// The dataset name ArcGIS uses in `FID_<name>`: the last path component without extension.
fn dataset_name(path: &str) -> String {
    let last = path
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path);
    match last.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => last.to_string(),
    }
}

/// The cluster tolerance (a distance, in the input's units unless a unit is given).
fn cluster_tolerance(ctx: &dyn Context, t: &Tool, wkt: Option<&str>) -> Result<Option<f64>> {
    text(ctx, t, "cluster_tolerance")
        .map(|c| distance_in_units(&c, wkt))
        .transpose()
}

fn attributes_flag(ctx: &dyn Context, t: &Tool) -> Result<bool> {
    match text(ctx, t, "attributes") {
        None => Ok(true),
        Some(a) if a.eq_ignore_ascii_case("ATTRIBUTES") => Ok(true),
        Some(a) if a.eq_ignore_ascii_case("NO_ATTRIBUTES") => Ok(false),
        Some(a) => Err(Error::Unsupported(format!(
            "ERROR 000800: attributes {a}: ATTRIBUTES or NO_ATTRIBUTES"
        ))),
    }
}

fn feature_to_polygon(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("feature_to_polygon")
        .ok_or_else(|| Error::Unsupported("feature_to_polygon".into()))?;
    let fc = ctx.features("input")?;
    let name = dataset_name(&ctx.get("input").unwrap_or_default());
    let tol = cluster_tolerance(ctx, t, fc.spatial_reference_wkt())?;
    let labels = match text(ctx, t, "label_features") {
        Some(_) => Some(ctx.features("label_features")?),
        None => None,
    };
    let inputs = [crate::NamedInput {
        name,
        features: fc.as_ref(),
    }];
    planar::feature_to_polygon(
        &inputs,
        tol,
        attributes_flag(ctx, t)?,
        labels.as_ref().map(|l| l.as_ref() as &dyn FeatureClass),
    )
}

fn feature_to_line(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("feature_to_line").ok_or_else(|| Error::Unsupported("feature_to_line".into()))?;
    let fc = ctx.features("input")?;
    let name = dataset_name(&ctx.get("input").unwrap_or_default());
    let tol = cluster_tolerance(ctx, t, fc.spatial_reference_wkt())?;
    let inputs = [crate::NamedInput {
        name,
        features: fc.as_ref(),
    }];
    planar::feature_to_line(&inputs, tol, attributes_flag(ctx, t)?)
}

const CONCEPTS: &[&str] = &[
    "INVERSE_DISTANCE",
    "INVERSE_DISTANCE_SQUARED",
    "FIXED_DISTANCE_BAND",
    "ZONE_OF_INDIFFERENCE",
    "K_NEAREST_NEIGHBORS",
    "CONTIGUITY_EDGES_ONLY",
    "CONTIGUITY_EDGES_CORNERS",
    "GET_SPATIAL_WEIGHTS_FROM_FILE",
];
const DISTANCE_METHODS: &[&str] = &["EUCLIDEAN_DISTANCE", "MANHATTAN_DISTANCE"];

const WEIGHTS_FILE: Param = Param {
    name: "weights_matrix_file",
    label: "Weights Matrix File",
    kind: Kind::Text,
    required: false,
    default: "",
    help: "A .swm file, for GET_SPATIAL_WEIGHTS_FROM_FILE.",
};

fn number_of(ctx: &dyn Context, t: &Tool, name: &str) -> f64 {
    text(ctx, t, name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0)
}

/// The distance band, or -1 when none is given (the tools' default threshold).
fn band_of(ctx: &dyn Context, t: &Tool) -> f64 {
    text(ctx, t, "distance_band")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(-1.0)
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

/// The weights file a tool names, when its conceptualization asks for one.
fn weights_file(ctx: &dyn Context, t: &Tool) -> Result<Option<crate::swm::Swm>> {
    let from_file = text(ctx, t, "conceptualization")
        .is_some_and(|c| c.eq_ignore_ascii_case("GET_SPATIAL_WEIGHTS_FROM_FILE"));
    if !from_file {
        return Ok(None);
    }
    let path = text(ctx, t, "weights_matrix_file").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Weights Matrix File: value is required".into())
    })?;
    crate::swm::Swm::read(std::path::Path::new(&path)).map(Some)
}

fn spatial_autocorrelation(ctx: &dyn Context) -> Result<Report> {
    let t = find("spatial_autocorrelation")
        .ok_or_else(|| Error::Unsupported("spatial_autocorrelation".into()))?;
    let fc = ctx.features("input")?;
    let row = text(ctx, t, "standardization").is_some_and(|s| s.eq_ignore_ascii_case("ROW"));
    let k = number_of(ctx, t, "number_of_neighbors").max(1.0) as usize;
    let swm = weights_file(ctx, t)?;
    spatstat::spatial_autocorrelation(
        fc.as_ref(),
        &text(ctx, t, "field").unwrap_or_default(),
        concept(ctx, t, k)?,
        distance_method(ctx, t)?,
        row,
        band_of(ctx, t),
        swm.as_ref(),
    )
}

fn hot_spot_analysis(ctx: &dyn Context) -> Result<ToolOutput> {
    let t =
        find("hot_spot_analysis").ok_or_else(|| Error::Unsupported("hot_spot_analysis".into()))?;
    let fc = ctx.features("input")?;
    let k = number_of(ctx, t, "number_of_neighbors").max(1.0) as usize;
    let fdr = text(ctx, t, "apply_fdr").is_some_and(|s| s.eq_ignore_ascii_case("APPLY_FDR"));
    let swm = weights_file(ctx, t)?;
    spatstat::hot_spot_analysis(
        fc.as_ref(),
        &text(ctx, t, "field").unwrap_or_default(),
        concept(ctx, t, k)?,
        distance_method(ctx, t)?,
        band_of(ctx, t),
        fdr,
        swm.as_ref(),
    )
}

fn cluster_and_outlier_analysis(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("cluster_and_outlier_analysis")
        .ok_or_else(|| Error::Unsupported("cluster_and_outlier_analysis".into()))?;
    let fc = ctx.features("input")?;
    let k = number_of(ctx, t, "number_of_neighbors").max(1.0) as usize;
    let fdr = text(ctx, t, "apply_fdr").is_some_and(|s| s.eq_ignore_ascii_case("APPLY_FDR"));
    let row = text(ctx, t, "standardization").is_some_and(|s| s.eq_ignore_ascii_case("ROW"));
    let perms = number_of(ctx, t, "number_of_permutations");
    if !(0.0..=9999.0).contains(&perms) {
        return Err(Error::Unsupported(
            "ERROR 000854: Number of Permutations must be 0 to 9999".into(),
        ));
    }
    let swm = weights_file(ctx, t)?;
    spatstat::cluster_and_outlier_analysis(
        fc.as_ref(),
        &text(ctx, t, "field").unwrap_or_default(),
        concept(ctx, t, k)?,
        distance_method(ctx, t)?,
        row,
        band_of(ctx, t),
        swm.as_ref(),
        fdr,
        perms as usize,
    )
}

fn multi_distance_spatial_clustering(ctx: &dyn Context) -> Result<Outcome> {
    use unigis_core::{Field, FieldType, Value};
    let t = find("multi_distance_spatial_clustering")
        .ok_or_else(|| Error::Unsupported("multi_distance_spatial_clustering".into()))?;
    let fc = ctx.features("input")?;
    let bands = number_of(ctx, t, "number_of_distance_bands");
    let perms: usize = match text(ctx, t, "compute_confidence_envelope")
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "9_PERMUTATIONS" => 9,
        "99_PERMUTATIONS" => 99,
        "999_PERMUTATIONS" => 999,
        _ => 0,
    };
    let boundary = match text(ctx, t, "boundary_correction_method")
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "RIPLEY_EDGE_CORRECTION_FORMULA" => spatstat::KBoundary::Ripley,
        "" | "NONE" => spatstat::KBoundary::None,
        other => {
            return Err(Error::Unsupported(format!(
                "boundary correction {other} is not done (NONE, RIPLEY_EDGE_CORRECTION_FORMULA)"
            )));
        }
    };
    let user_area = text(ctx, t, "study_area_method")
        .is_some_and(|m| m.eq_ignore_ascii_case("USER_PROVIDED_STUDY_AREA_FEATURE_CLASS"));
    let area = if user_area {
        let sa = ctx.features("study_area_feature_class")?;
        let mut g: Option<unigis_core::Geometry> = None;
        for f in sa.features() {
            if let Some(geom) = f?.geometry {
                g = Some(match g {
                    None => geom,
                    Some(mut acc) => {
                        let off = acc.xy.len() as u32;
                        let parts: Vec<u32> = if geom.parts.is_empty() {
                            vec![0]
                        } else {
                            geom.parts.clone()
                        };
                        if acc.parts.is_empty() {
                            acc.parts.push(0);
                        }
                        acc.parts.extend(parts.iter().map(|p| p + off));
                        acc.xy.extend(geom.xy);
                        acc
                    }
                });
            }
        }
        Some(g.ok_or_else(|| Error::Unsupported("the study area has no polygon".into()))?)
    } else {
        None
    };
    let opt = |name: &str| text(ctx, t, name).and_then(|v| v.trim().parse::<f64>().ok());
    let wf = text(ctx, t, "weight_field");
    let (rows, _messages) = spatstat::ripleys_k(
        fc.as_ref(),
        bands.max(0.0) as usize,
        perms,
        wf.as_deref(),
        opt("beginning_distance"),
        opt("distance_increment"),
        boundary,
        area.as_ref(),
    )?;
    let dbase = ctx
        .get("output")
        .is_some_and(|o| o.to_ascii_lowercase().ends_with(".dbf"));
    let mut fields = Vec::new();
    if dbase {
        let mut f = Field::new("Field1", FieldType::Integer);
        f.precision = 10;
        fields.push(f);
    }
    let mut names = vec!["ExpectedK", "ObservedK", "DiffK"];
    if perms > 0 {
        names.extend(["LwConfEnv", "HiConfEnv"]);
    }
    fields.extend(names.iter().map(|n| Field::new(*n, FieldType::Double)));
    let rows = rows
        .into_iter()
        .map(|r| {
            let mut v = Vec::new();
            if dbase {
                v.push(Value::Integer(0));
            }
            v.extend(r[..names.len()].iter().map(|x| Value::Double(*x)));
            v
        })
        .collect();
    Ok(Outcome::Table(TableOutput { fields, rows }))
}

fn ordinary_least_squares(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("ordinary_least_squares")
        .ok_or_else(|| Error::Unsupported("ordinary_least_squares".into()))?;
    let fc = ctx.features("input")?;
    let ex: Vec<String> = text(ctx, t, "explanatory_variables")
        .unwrap_or_default()
        .split([';', ','])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let dbf = |name: &str| {
        ctx.get(name)
            .is_some_and(|o| o.to_ascii_lowercase().ends_with(".dbf"))
    };
    let coef_dbf = dbf("coefficient_output_table");
    let diag_dbf = dbf("diagnostic_output_table");
    let mut r = crate::regression::ordinary_least_squares(
        fc.as_ref(),
        &text(ctx, t, "unique_id_field").unwrap_or_default(),
        &text(ctx, t, "dependent_variable").unwrap_or_default(),
        &ex,
        coef_dbf,
    )?;
    if diag_dbf != coef_dbf {
        // Field1 follows each table's own format.
        let other = crate::regression::ordinary_least_squares(
            fc.as_ref(),
            &text(ctx, t, "unique_id_field").unwrap_or_default(),
            &text(ctx, t, "dependent_variable").unwrap_or_default(),
            &ex,
            diag_dbf,
        )?;
        r.diagnostics = other.diagnostics;
    }
    let mut features = r.features;
    features.messages = r.messages;
    Ok(Outcome::FeaturesAndTables(
        features,
        vec![
            ("coefficient_output_table", r.coefficients),
            ("diagnostic_output_table", r.diagnostics),
        ],
    ))
}

fn collect_events(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    let shp = ctx
        .get("output")
        .is_none_or(|o| o.to_ascii_lowercase().ends_with(".shp"));
    let (mut out, max) = spatstat::collect_events(fc.as_ref(), shp)?;
    out.messages.push(format!("Max count: {max}"));
    Ok(out)
}

fn generate_spatial_weights_matrix(ctx: &dyn Context) -> Result<Report> {
    use crate::swm;
    let t = find("generate_spatial_weights_matrix")
        .ok_or_else(|| Error::Unsupported("generate_spatial_weights_matrix".into()))?;
    let output = ctx
        .get("output")
        .ok_or_else(|| Error::Unsupported("output=<file.swm> is required".into()))?;
    let output = if output.to_ascii_lowercase().ends_with(".swm") {
        output
    } else {
        format!("{output}.swm")
    };
    let fc = ctx.features("input")?;
    let table = match text(ctx, t, "input_table") {
        Some(_) => Some(ctx.features("input_table")?),
        None => None,
    };
    let c = text(ctx, t, "conceptualization").unwrap_or_default();
    let concept = swm::SwmConcept::parse(&c)
        .ok_or_else(|| Error::Unsupported(format!("conceptualization {c}")))?;
    let m = text(ctx, t, "distance_method").unwrap_or_default();
    let method = spatstat::DistanceMethod::parse(&m)
        .ok_or_else(|| Error::Unsupported(format!("distance method {m}")))?;
    let opt_num = |name: &str| text(ctx, t, name).and_then(|v| v.trim().parse::<f64>().ok());
    let count = |name: &str| -> Result<Option<usize>> {
        match opt_num(name) {
            Some(v) if !(0.0..=100_000.0).contains(&v) => Err(Error::Unsupported(format!(
                "ERROR 000854: {name} must be 0 to 100000"
            ))),
            Some(v) if v >= 1.0 => Ok(Some(v as usize)),
            _ => Ok(None),
        }
    };
    let wfield = text(ctx, t, "weight_field");
    let wm = text(ctx, t, "weighting_method").unwrap_or_default();
    let weighting = swm::Weighting::parse(&wm, wfield.as_deref()).ok_or_else(|| {
        Error::Unsupported(format!("weighting method {wm} (FIELD needs weight_field)"))
    })?;
    let unit = text(ctx, t, "date_time_interval_type");
    let input_path = ctx.get("input").unwrap_or_default();
    let uid = text(ctx, t, "unique_id_field").unwrap_or_default();
    let tfield = text(ctx, t, "date_time_field");
    let opt = swm::SwmOptions {
        unique_id: &uid,
        concept,
        method,
        exponent: opt_num("exponent").unwrap_or(1.0),
        threshold: opt_num("threshold_distance"),
        neighbors: count("number_of_neighbors")?,
        row_standardize: !text(ctx, t, "row_standardization")
            .is_some_and(|s| s.eq_ignore_ascii_case("NO_STANDARDIZATION")),
        table: table.as_deref(),
        time_field: tfield.as_deref(),
        time_unit: unit.as_deref().and_then(swm::TimeUnit::parse),
        time_value: opt_num("date_time_interval_value").unwrap_or(0.0),
        order: count("order")?.unwrap_or(1).min(100),
        include_focal: text(ctx, t, "include_focal_feature")
            .is_some_and(|s| s.eq_ignore_ascii_case("INCLUDE_FOCAL")),
        weighting,
        adaptive: !text(ctx, t, "kernel_type").is_some_and(|s| s.eq_ignore_ascii_case("FIXED")),
        adaptive_neighbors: count("adaptive_neighbors")?,
        bandwidth: opt_num("kernel_bandwidth"),
        input_path: &input_path,
    };
    let (m, messages) = swm::generate_spatial_weights_matrix(fc.as_ref(), &opt)?;
    m.write(std::path::Path::new(&output))?;
    Ok(Report {
        rows: Vec::new(),
        messages,
    })
}

fn convert_spatial_weights_matrix_to_table(ctx: &dyn Context) -> Result<Outcome> {
    let path = ctx
        .get("input")
        .ok_or_else(|| Error::Unsupported("ERROR 000735: input: value is required".into()))?;
    let m = crate::swm::Swm::read(std::path::Path::new(&path))?;
    let dbase = ctx
        .get("output")
        .is_some_and(|o| o.to_ascii_lowercase().ends_with(".dbf"));
    Ok(Outcome::Table(
        crate::swm::convert_spatial_weights_matrix_to_table(&m, dbase),
    ))
}

fn where_of(ctx: &dyn Context, t: &Tool) -> Option<String> {
    text(ctx, t, "where_clause")
}

fn copy_rows(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("copy_rows").ok_or_else(|| Error::Unsupported("copy_rows".into()))?;
    let fc = ctx.features("input")?;
    let mut table = tables::copy_rows(fc.as_ref(), where_of(ctx, t).as_deref())?;
    // Copy Rows writes decimal fields at most 19 characters wide (observed on a dBASE source).
    for f in &mut table.fields {
        if f.field_type == unigis_core::FieldType::Double && f.precision > 0 {
            f.precision = f.precision.min(19 - u32::from(f.scale > 0));
        }
    }
    Ok(Outcome::Table(table))
}

fn generate_origin_destination_links(ctx: &dyn Context) -> Result<ToolOutput> {
    let t = find("generate_origin_destination_links").unwrap();
    if text(ctx, t, "line_type").is_some_and(|v| !v.eq_ignore_ascii_case("PLANAR")) {
        return Err(Error::Unsupported(
            "GEODESIC links are not yet supported".into(),
        ));
    }
    if text(ctx, t, "aggregate_links").is_some_and(|v| !v.eq_ignore_ascii_case("NO_AGGREGATE"))
        || text(ctx, t, "sum_fields").is_some()
    {
        return Err(Error::Unsupported(
            "aggregated links and summary fields are not yet supported".into(),
        ));
    }
    let orig_group = text(ctx, t, "origin_group_field");
    let dest_group = text(ctx, t, "destination_group_field");
    if orig_group.is_some() != dest_group.is_some() {
        return Err(Error::Unsupported(
            "ERROR 000735: both group fields are required when grouping".into(),
        ));
    }
    let nearest = text(ctx, t, "num_nearest")
        .map(|v| -> Result<usize> {
            let n = v
                .parse::<f64>()
                .map_err(|_| Error::Unsupported("invalid destination count".into()))?;
            if !n.is_finite() || n < 0.0 || n.fract() != 0.0 || n > u32::MAX as f64 {
                return Err(Error::Unsupported(
                    "destination count must be a finite nonnegative integer".into(),
                ));
            }
            Ok(n as usize)
        })
        .transpose()?;
    let radius = text(ctx, t, "search_distance")
        .map(|v| {
            v.parse::<f64>()
                .map_err(|_| Error::Unsupported("invalid search distance".into()))
        })
        .transpose()?;
    let origin_path = ctx.get("origin_features");
    let dest_path = ctx.get("destination_features");
    let same_source = origin_path == dest_path
        || origin_path
            .as_ref()
            .zip(dest_path.as_ref())
            .is_some_and(|(a, b)| {
                std::fs::canonicalize(a)
                    .ok()
                    .zip(std::fs::canonicalize(b).ok())
                    .is_some_and(|(a, b)| a == b)
            });
    let origins = ctx.features("origin_features")?;
    let destinations = ctx.features("destination_features")?;
    let unit = text(ctx, t, "distance_unit");
    crate::origin_destination::generate(
        origins.as_ref(),
        destinations.as_ref(),
        &crate::origin_destination::Options {
            groups: orig_group.as_deref().zip(dest_group.as_deref()),
            num_nearest: nearest,
            search_distance: radius,
            distance_unit: unit.as_deref(),
            same_source,
        },
    )
}

fn assign_weights_by_pairwise_comparison(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("assign_weights_by_pairwise_comparison")
        .ok_or_else(|| Error::Unsupported("assign_weights_by_pairwise_comparison".into()))?;
    let variables: Vec<String> = text(ctx, t, "input_variables")
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Input Variables To Compare: value is required".into())
        })?
        .split(';')
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
        .collect();
    let add_matrix = !text(ctx, t, "add_comparison_matrix")
        .is_some_and(|m| m.eq_ignore_ascii_case("WEIGHTS_ONLY"));
    let matrix = text(ctx, t, "comparison_matrix");
    Ok(Outcome::Table(
        crate::pairwise::assign_weights_by_pairwise_comparison(
            &variables,
            matrix.as_deref(),
            add_matrix,
        )?,
    ))
}

fn field_statistics_to_table(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("field_statistics_to_table")
        .ok_or_else(|| Error::Unsupported("field_statistics_to_table".into()))?;
    let fields: Vec<String> = text(ctx, t, "fields")
        .ok_or_else(|| Error::Unsupported("ERROR 000735: Input Fields: value is required".into()))?
        .split(';')
        .map(|f| f.trim().to_owned())
        .filter(|f| !f.is_empty())
        .collect();
    let kind = match text(ctx, t, "field_type")
        .map(|f| f.to_ascii_uppercase())
        .as_deref()
    {
        Some("TEXT") => tables::FieldStatisticsType::Text,
        Some("DATE") => tables::FieldStatisticsType::Date,
        _ => tables::FieldStatisticsType::Numeric,
    };
    let group_by = text(ctx, t, "group_by").filter(|g| !g.trim().is_empty());
    // `KEYWORD [Output Name];...`
    let statistics: Vec<(String, String)> = text(ctx, t, "statistics")
        .unwrap_or_default()
        .split(';')
        .filter_map(|item| {
            let item = item.trim();
            let (key, name) = item.split_once(char::is_whitespace).unwrap_or((item, ""));
            let name = name.trim().trim_matches('\'');
            let name = if name == "#" { "" } else { name };
            (!key.is_empty()).then(|| (key.to_owned(), name.to_owned()))
        })
        .collect();
    Ok(Outcome::Table(tables::field_statistics_to_table(
        ctx.features("input")?.as_ref(),
        &fields,
        kind,
        group_by.as_deref().map(str::trim),
        &statistics,
    )?))
}

fn table_select(ctx: &dyn Context) -> Result<Outcome> {
    let t = find("table_select").ok_or_else(|| Error::Unsupported("table_select".into()))?;
    let fc = ctx.features("input")?;
    let mut table = tables::copy_rows(fc.as_ref(), where_of(ctx, t).as_deref())?;
    // Like Copy Rows, decimal fields are at most 19 characters wide (observed).
    for f in &mut table.fields {
        if f.field_type == unigis_core::FieldType::Double && f.precision > 0 {
            f.precision = f.precision.min(19 - u32::from(f.scale > 0));
        }
    }
    Ok(Outcome::Table(table))
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
            Param {
                name: "add_chainage_fields",
                label: "Add Chainage Fields",
                kind: Kind::Choice(&["NO_CHAINAGE", "ADD_CHAINAGE"]),
                required: false,
                default: "NO_CHAINAGE",
                help: "Adds ORIG_LEN (distance along the line) and ORIG_SEQ (number along the line).",
            },
        ],
        output_suffix: "Points",
        measures: true,
        run: Runner::Features(generate_points_along_lines),
    },
    Tool {
        id: "generate_transects_along_lines",
        name: "Generate Transects Along Lines",
        toolbox: "Data Management › Features",
        summary: "Lines across lines at regular intervals (ArcGIS Pro Generate Transects Along Lines).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Lines.",
            },
            Param {
                name: "interval",
                label: "Interval",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "For example `100 Meters`.",
            },
            Param {
                name: "transect_length",
                label: "Transect Length",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "For example `20 Meters`.",
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
        output_suffix: "Transects",
        measures: true,
        run: Runner::Features(generate_transects_along_lines),
    },
    Tool {
        id: "convert_coordinate_notation",
        name: "Convert Coordinate Notation",
        toolbox: "Data Management › Features",
        summary: "Converts angular, GARS and GEOREF fields to DD, DDM, DMS and grid notations.",
        params: &[
            Param {
                name: "input",
                label: "Input Table",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "A table with angular, GARS or GEOREF coordinates.",
            },
            Param {
                name: "x_field",
                label: "X Field (Longitude or Coordinate)",
                kind: Kind::Field {
                    of: "input",
                    numeric: false,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "y_field",
                label: "Y Field (Latitude)",
                kind: Kind::Field {
                    of: "input",
                    numeric: false,
                },
                required: false,
                default: "",
                help: "Required for two-field input formats; ignored for single-field formats.",
            },
            Param {
                name: "input_format",
                label: "Input Coordinate Format",
                kind: Kind::Choice(&[
                    "DD_1",
                    "DD_2",
                    "DDM_1",
                    "DDM_2",
                    "DMS_1",
                    "DMS_2",
                    "GARS",
                    "GEOREF",
                    "UTM_BANDS",
                    "UTM_ZONES",
                    "MGRS",
                    "USNG",
                ]),
                required: true,
                default: "DD_2",
                help: "Single-field angular formats use latitude then longitude unless hemispheres specify the axes. GARS/GEOREF inputs support DD_NUMERIC, angular text, GARS and GEOREF outputs; UTM_BANDS/UTM_ZONES/MGRS/USNG inputs support DD_NUMERIC, DD_1 and DD_2 outputs.",
            },
            Param {
                name: "output_format",
                label: "Output Coordinate Format",
                kind: Kind::Choice(&[
                    "DD_1",
                    "DD_2",
                    "DDM_1",
                    "DDM_2",
                    "DMS_1",
                    "DMS_2",
                    "DD_NUMERIC",
                    "UTM_BANDS",
                    "UTM_ZONES",
                    "MGRS",
                    "USNG",
                    "GARS",
                    "GEOREF",
                ]),
                required: true,
                default: "DD_2",
                help: "",
            },
        ],
        output_suffix: "Notation",
        measures: false,
        run: Runner::Features(convert_coordinate_notation),
    },
    Tool {
        id: "table_to_ellipse",
        name: "Table To Ellipse",
        toolbox: "Data Management › Features",
        summary: "Ellipses from center, axis and azimuth fields, as polygons or lines (ArcGIS Pro Table To Ellipse, planar).",
        params: &[
            Param {
                name: "input",
                label: "Input Table",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "A table with center, axis and azimuth fields.",
            },
            Param {
                name: "x_field",
                label: "X Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "y_field",
                label: "Y Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "major_field",
                label: "Major Axis Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "minor_field",
                label: "Minor Axis Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "distance_units",
                label: "Distance Units",
                kind: Kind::Choice(&[
                    "Meters",
                    "Kilometers",
                    "Miles",
                    "Nautical miles",
                    "Feet",
                    "U.S. survey feet",
                ]),
                required: false,
                default: "Meters",
                help: "",
            },
            Param {
                name: "azimuth_field",
                label: "Azimuth Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "azimuth_units",
                label: "Azimuth Units",
                kind: Kind::Choice(&["Decimal degrees", "Mils", "Radians", "Gradians"]),
                required: false,
                default: "Decimal degrees",
                help: "",
            },
            Param {
                name: "spatial_reference",
                label: "Coordinate System",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "A projected coordinate system, e.g. 32633.",
            },
            Param {
                name: "shape_type",
                label: "Output Type",
                kind: Kind::Choice(&["Polygon", "Line"]),
                required: false,
                default: "Polygon",
                help: "",
            },
            Param {
                name: "method",
                label: "Method",
                kind: Kind::Choice(&["Planar"]),
                required: false,
                default: "Planar",
                help: "",
            },
        ],
        output_suffix: "Ellipses",
        measures: false,
        run: Runner::Features(table_to_ellipse),
    },
    Tool {
        id: "geodetic_densify",
        name: "Geodetic Densify",
        toolbox: "Data Management › Features",
        summary: "Adds vertices along the geodesic between a segment's ends (ArcGIS Pro Geodetic Densify, GEODESIC).",
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
                name: "densify_method",
                label: "Densification Method",
                kind: Kind::Choice(&["Geodesic"]),
                required: false,
                default: "Geodesic",
                help: "",
            },
            Param {
                name: "max_segment_length",
                label: "Maximum Segment Length",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "A length, for example `500 Kilometers` (bare numbers are meters).",
            },
        ],
        output_suffix: "Densified",
        measures: false,
        run: Runner::Features(geodetic_densify),
    },
    Tool {
        id: "encode_field",
        name: "Encode Field",
        toolbox: "Data Management › Fields",
        summary: "One-hot or one-cold integer fields for the values of a text or integer field (ArcGIS Pro Encode Field).",
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
                name: "field",
                label: "Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: false,
                },
                required: true,
                default: "",
                help: "Text or integer field to encode.",
            },
            Param {
                name: "method",
                label: "Encoding Method",
                kind: Kind::Choice(&["One-hot", "One-cold"]),
                required: false,
                default: "One-hot",
                help: "",
            },
        ],
        output_suffix: "Encoded",
        measures: false,
        run: Runner::Features(encode_field),
    },
    Tool {
        id: "generate_origin_destination_links",
        name: "Generate Origin-Destination Links",
        toolbox: "Analysis › Proximity",
        summary: "Generate planar links between projected feature centroids, with groups, destination limits and search distances.",
        params: &[
            Param {
                name: "origin_features",
                label: "Origin Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "destination_features",
                label: "Destination Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "origin_group_field",
                label: "Origin Group Field",
                kind: Kind::Field {
                    of: "origin_features",
                    numeric: false,
                },
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "destination_group_field",
                label: "Destination Group Field",
                kind: Kind::Field {
                    of: "destination_features",
                    numeric: false,
                },
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "line_type",
                label: "Line Type",
                kind: Kind::Choice(&["PLANAR", "GEODESIC"]),
                required: false,
                default: "PLANAR",
                help: "Geodesic links are not yet supported.",
            },
            Param {
                name: "num_nearest",
                label: "Number of Nearest Destinations",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "Zero or omitted means all. ArcGIS 3.7 PLANAR controls take qualifying destinations in input order.",
            },
            Param {
                name: "search_distance",
                label: "Search Distance",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "Zero or omitted means unlimited; measured in Distance Unit.",
            },
            Param {
                name: "distance_unit",
                label: "Distance Unit",
                kind: Kind::Choice(&[
                    "METERS",
                    "KILOMETERS",
                    "MILES",
                    "NAUTICALMILES",
                    "YARDS",
                    "FEET",
                ]),
                required: false,
                default: "",
                help: "Default: origin coordinate units.",
            },
            Param {
                name: "aggregate_links",
                label: "Aggregate Overlapping Links",
                kind: Kind::Choice(&["NO_AGGREGATE", "AGGREGATE_OVERLAPPING"]),
                required: false,
                default: "NO_AGGREGATE",
                help: "Aggregation is not yet supported.",
            },
            Param {
                name: "sum_fields",
                label: "Summary Fields",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Summary fields are not yet supported.",
            },
        ],
        output_suffix: "ODLinks",
        measures: false,
        run: Runner::Features(generate_origin_destination_links),
    },
    Tool {
        id: "calculate_geometry_attributes",
        name: "Calculate Geometry Attributes",
        toolbox: "Data Management › Fields",
        summary: "Writes geometry properties (area, length, centroid, extent, counts, ...) into existing fields (ArcGIS Pro Calculate Geometry Attributes, planar).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "geometry_property",
                label: "Geometry Attributes",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "`FIELD PROPERTY` pairs separated by semicolons, e.g. `AREA_D AREA;CX CENTROID_X`.",
            },
            Param {
                name: "length_unit",
                label: "Length Unit",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "KILOMETERS, METERS, FEET_INT, FEET_US, YARDS_INT, YARDS_US, MILES_INT, MILES_US, NAUTICAL_MILES_INT.",
            },
            Param {
                name: "area_unit",
                label: "Area Unit",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "SQUARE_KILOMETERS, HECTARES, SQUARE_METERS, ACRES, SQUARE_FEET_INT, SQUARE_YARDS_INT, SQUARE_MILES_INT.",
            },
        ],
        output_suffix: "Geometry",
        measures: true,
        run: Runner::Features(calculate_geometry_attributes),
    },
    Tool {
        id: "adjust_3d_z",
        name: "Adjust 3D Z",
        toolbox: "Data Management › Features",
        summary: "Shifts, converts or negates the z values of 3D features (ArcGIS Pro Adjust 3D Z).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Z-enabled features.",
            },
            Param {
                name: "reverse_sign",
                label: "Reverse Sign",
                kind: Kind::Choice(&["NO_REVERSE", "REVERSE"]),
                required: false,
                default: "NO_REVERSE",
                help: "",
            },
            Param {
                name: "adjust_value",
                label: "Adjust Value",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "A number or a numeric field added to every z (null counts as 0).",
            },
            Param {
                name: "from_units",
                label: "Convert From Units",
                kind: Kind::Choice(&[
                    "MILLIMETERS",
                    "CENTIMETERS",
                    "METERS",
                    "INCHES",
                    "FEET",
                    "YARDS",
                    "FATHOMS",
                ]),
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "to_units",
                label: "Convert To Units",
                kind: Kind::Choice(&[
                    "MILLIMETERS",
                    "CENTIMETERS",
                    "METERS",
                    "INCHES",
                    "FEET",
                    "YARDS",
                    "FATHOMS",
                ]),
                required: false,
                default: "",
                help: "",
            },
        ],
        output_suffix: "AdjustedZ",
        measures: true,
        run: Runner::Features(adjust_3d_z),
    },
    Tool {
        id: "validate_join",
        name: "Validate Join",
        toolbox: "Data Management › Joins",
        summary: "Checks an attribute join and writes ArcGIS's message table: field indexes, no matches, row counts (ArcGIS Pro Validate Join).",
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
                name: "in_field",
                label: "Input Join Field",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "",
            },
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
                kind: Kind::Text,
                required: true,
                default: "",
                help: "",
            },
        ],
        output_suffix: "ValidateJoin",
        measures: false,
        run: Runner::Dynamic(validate_join),
    },
    Tool {
        id: "convert_time_zone",
        name: "Convert Time Zone",
        toolbox: "Data Management › Fields",
        summary: "Date values converted from one time zone to another into a new field (ArcGIS Pro Convert Time Zone).",
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
                name: "input_time_field",
                label: "Input Time Field",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "A date field.",
            },
            Param {
                name: "input_time_zone",
                label: "Input Time Zone",
                kind: Kind::Text,
                required: true,
                default: "Pacific Standard Time",
                help: "Windows time zone name, e.g. `Pacific Standard Time`.",
            },
            Param {
                name: "output_time_field",
                label: "Output Time Field",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "The new date field.",
            },
            Param {
                name: "output_time_zone",
                label: "Output Time Zone",
                kind: Kind::Text,
                required: true,
                default: "UTC",
                help: "",
            },
            Param {
                name: "input_dst",
                label: "Input Time Is Adjusted For Daylight Saving",
                kind: Kind::Choice(&["INPUT_ADJUSTED_FOR_DST", "INPUT_NOT_ADJUSTED_FOR_DST"]),
                required: false,
                default: "INPUT_ADJUSTED_FOR_DST",
                help: "",
            },
            Param {
                name: "output_dst",
                label: "Output Time Is Adjusted For Daylight Saving",
                kind: Kind::Choice(&["OUTPUT_ADJUSTED_FOR_DST", "OUTPUT_NOT_ADJUSTED_FOR_DST"]),
                required: false,
                default: "OUTPUT_ADJUSTED_FOR_DST",
                help: "",
            },
        ],
        output_suffix: "Converted",
        measures: false,
        run: Runner::Dynamic(convert_time_zone),
    },
    Tool {
        id: "convert_time_field",
        name: "Convert Temporal Field",
        toolbox: "Data Management › Fields",
        summary: "Writes a date field as text or numbers in a format, or reads text or numbers in a format into a date field (ArcGIS Pro Convert Temporal Field).",
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
                name: "input_time_field",
                label: "Input Time Field",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "The date, text or number field.",
            },
            Param {
                name: "input_time_format",
                label: "Input Time Format",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "For text or numeric input, e.g. `yyyyMMdd` or `MM/dd/yyyy HH:mm:ss`.",
            },
            Param {
                name: "output_time_field",
                label: "Output Time Field",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "The new field.",
            },
            Param {
                name: "output_time_type",
                label: "Output Time Type",
                kind: Kind::Choice(&[
                    "DATE",
                    "TEXT",
                    "LONG",
                    "SHORT",
                    "DOUBLE",
                    "FLOAT",
                    "BIGINTEGER",
                ]),
                required: false,
                default: "DATE",
                help: "",
            },
            Param {
                name: "output_time_format",
                label: "Output Time Format",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "For text or numeric output; text defaults to `yyyyMMddHHmmss`.",
            },
        ],
        output_suffix: "Converted",
        measures: false,
        run: Runner::Dynamic(convert_time_field),
    },
    Tool {
        id: "batch_update_fields",
        name: "Batch Update Fields",
        toolbox: "Data Management › Fields",
        summary: "A copy with the fields a definition table lists: renamed, retyped or scripted (ArcGIS Pro Batch Update Fields).",
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
                name: "field_definition_table",
                label: "Field Definition Table",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "One row per output field.",
            },
            Param {
                name: "output_field_name",
                label: "Output Field Name",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Column of the definition table with the output field names.",
            },
            Param {
                name: "source_field_name",
                label: "Source Field Name",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Column with the input field each output field copies.",
            },
            Param {
                name: "output_field_type",
                label: "Output Field Type",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Column with TEXT, SHORT, LONG, FLOAT, DOUBLE, DATE or BIGINTEGER (blank: the source's type).",
            },
            Param {
                name: "output_field_decimals_or_length",
                label: "Output Field Decimals Or Length",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Column with the text length or the decimals.",
            },
            Param {
                name: "output_field_alias",
                label: "Output Field Alias",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Column with the aliases.",
            },
            Param {
                name: "output_field_script",
                label: "Output Field Script",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Column with Python expressions over the output fields, e.g. `!PID! * 10`.",
            },
        ],
        output_suffix: "Updated",
        measures: false,
        run: Runner::Dynamic(batch_update_fields),
    },
    Tool {
        id: "add_spatial_join",
        name: "Add Spatial Join",
        toolbox: "Data Management › Joins",
        summary: "The target features with Join_Count and the attributes of the join features they match (ArcGIS Pro Add Spatial Join, permanent fields).",
        params: &[
            Param {
                name: "target_features",
                label: "Target Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "join_features",
                label: "Join Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "join_operation",
                label: "Join Operation",
                kind: Kind::Choice(&["JOIN_ONE_TO_ONE", "JOIN_ONE_TO_MANY"]),
                required: false,
                default: "JOIN_ONE_TO_ONE",
                help: "Permanent fields are one to one either way.",
            },
            Param {
                name: "join_type",
                label: "Keep All Target Features",
                kind: Kind::Choice(&["KEEP_ALL", "KEEP_COMMON"]),
                required: false,
                default: "KEEP_ALL",
                help: "Every target feature is kept either way.",
            },
            Param {
                name: "match_option",
                label: "Match Option",
                kind: Kind::Text,
                required: false,
                default: "INTERSECT",
                help: "As in Spatial Join, for example INTERSECT, WITHIN_A_DISTANCE or CLOSEST.",
            },
            Param {
                name: "search_radius",
                label: "Search Radius",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "For example `50 Meters`.",
            },
            Param {
                name: "distance_field_name",
                label: "Distance Field Name",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Filled for CLOSEST, -1 otherwise.",
            },
            Param {
                name: "permanent_join",
                label: "Permanent Join",
                kind: Kind::Choice(&["PERMANENT_FIELDS", "NO_PERMANENT_FIELDS"]),
                required: false,
                default: "PERMANENT_FIELDS",
                help: "The output always holds the joined fields.",
            },
        ],
        output_suffix: "SpatialJoin",
        measures: false,
        run: Runner::Features(add_spatial_join),
    },
    Tool {
        id: "assign_weights_by_pairwise_comparison",
        name: "Assign Weights By Pairwise Comparison",
        toolbox: "Analysis › Overlay",
        summary: "Relative weights of 2 to 9 variables from pairwise comparisons (ArcGIS Pro Assign Weights By Pairwise Comparison).",
        params: &[
            Param {
                name: "input_variables",
                label: "Input Variables To Compare",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "Variable names separated by semicolons.",
            },
            Param {
                name: "add_comparison_matrix",
                label: "Add comparison matrix to output",
                kind: Kind::Choice(&["ADD_MATRIX", "WEIGHTS_ONLY"]),
                required: false,
                default: "ADD_MATRIX",
                help: "",
            },
            Param {
                name: "comparison_matrix",
                label: "Pairwise Comparison Matrix",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "`Variables A B 3; A C 5; B C 2`: each upper-triangle pair once, value 1/9..9; default all 1.",
            },
        ],
        output_suffix: "Weights",
        measures: false,
        run: Runner::Dynamic(assign_weights_by_pairwise_comparison),
    },
    Tool {
        id: "trim_line",
        name: "Trim Line",
        toolbox: "Editing › Tools",
        summary: "Removes dangling line pieces shorter than a length (ArcGIS Pro Trim Line).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Lines.",
            },
            Param {
                name: "dangle_length",
                label: "Dangle Length",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "Dangles shorter than this are trimmed, for example `25 Meters`.",
            },
            Param {
                name: "delete_shorts",
                label: "Delete Short Features",
                kind: Kind::Choice(&["DELETE_SHORT", "KEEP_SHORT"]),
                required: false,
                default: "DELETE_SHORT",
                help: "Lines free at both ends made only of short dangles are deleted or kept unchanged.",
            },
        ],
        output_suffix: "Trimmed",
        measures: false,
        run: Runner::Features(trim_line),
    },
    Tool {
        id: "extend_line",
        name: "Extend Line",
        toolbox: "Editing › Tools",
        summary: "Extends dangling line ends to the first line they reach (ArcGIS Pro Extend Line).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Lines.",
            },
            Param {
                name: "length",
                label: "Extend Length",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "The longest extension, for example `15 Meters`.",
            },
            Param {
                name: "extend_to",
                label: "Extend To",
                kind: Kind::Choice(&["FEATURE", "EXTENSION"]),
                required: false,
                default: "FEATURE",
                help: "EXTENSION also stops ends where they meet another end's extension.",
            },
        ],
        output_suffix: "Extended",
        measures: false,
        run: Runner::Features(extend_line),
    },
    Tool {
        id: "field_statistics_to_table",
        name: "Field Statistics To Table",
        toolbox: "Data Management › Fields",
        summary: "Descriptive statistics per field (ArcGIS Pro Field Statistics To Table), one field type (numeric, text or date) per run, optionally per group.",
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
                label: "Input Fields",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "Field names separated by semicolons.",
            },
            Param {
                name: "field_type",
                label: "Field Type",
                kind: Kind::Choice(&["Numeric", "Text", "Date"]),
                required: false,
                default: "Numeric",
                help: "The kind of fields to summarize; others in the list are skipped.",
            },
            Param {
                name: "group_by",
                label: "Group By Field",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Short, Long, BigInteger, Text or date field; statistics repeat per value.",
            },
            Param {
                name: "statistics",
                label: "Output Statistics",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "KEYWORD [Output Name] items separated by semicolons (MEDIAN Med;SUM;...), in output order; default all.",
            },
        ],
        output_suffix: "FieldStatistics",
        measures: false,
        run: Runner::Dynamic(field_statistics_to_table),
    },
    Tool {
        id: "flip_line",
        name: "Flip Line",
        toolbox: "Editing › Tools",
        summary: "Reverses the direction of lines (ArcGIS Pro Flip Line).",
        params: &[Param {
            name: "input",
            label: "Input Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "Lines.",
        }],
        output_suffix: "Flipped",
        measures: false,
        run: Runner::Features(flip_line),
    },
    Tool {
        id: "generalize",
        name: "Generalize",
        toolbox: "Editing › Tools",
        summary: "Removes vertices within a tolerance of the line (ArcGIS Pro Generalize, lines).",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Features,
                required: true,
                default: "",
                help: "Lines.",
            },
            Param {
                name: "tolerance",
                label: "Maximum Offset Tolerance",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "For example `20 Meters`.",
            },
        ],
        output_suffix: "Generalized",
        measures: true,
        run: Runner::Features(generalize),
    },
    Tool {
        id: "generate_tessellation",
        name: "Generate Tessellation",
        toolbox: "Data Management › Sampling",
        summary: "Square, diamond, triangle or hexagon cells over an extent (ArcGIS Pro Generate Tessellation).",
        params: &[
            Param {
                name: "extent",
                label: "Extent",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "xmin ymin xmax ymax.",
            },
            Param {
                name: "shape",
                label: "Shape Type",
                kind: Kind::Choice(&[
                    "HEXAGON",
                    "TRANSVERSE_HEXAGON",
                    "SQUARE",
                    "TRIANGLE",
                    "DIAMOND",
                ]),
                required: false,
                default: "HEXAGON",
                help: "",
            },
            Param {
                name: "size",
                label: "Size",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "The cell area, e.g. `100 SquareMeters`.",
            },
            Param {
                name: "template",
                label: "Spatial Reference From",
                kind: Kind::Features,
                required: false,
                default: "",
                help: "A layer whose coordinate system the cells take.",
            },
        ],
        output_suffix: "Tessellation",
        measures: true,
        run: Runner::Features(generate_tessellation),
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
        summary: "The boundaries of polygons as lines, split where neighbors meet, with the polygons on their left and right (ArcGIS Pro Polygon To Line).",
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
                kind: Kind::Choice(&["IDENTIFY_NEIGHBORS", "IGNORE_NEIGHBORS"]),
                required: false,
                default: "IDENTIFY_NEIGHBORS",
                help: "IDENTIFY_NEIGHBORS: shared boundaries split, LEFT_FID and RIGHT_FID; IGNORE_NEIGHBORS: each polygon's rings as one line with ORIG_FID.",
            },
        ],
        output_suffix: "Line",
        measures: false,
        run: Runner::Features(polygon_to_line),
    },
    Tool {
        id: "feature_to_polygon",
        name: "Feature To Polygon",
        toolbox: "Data Management › Features",
        summary: "Polygons from the areas enclosed by lines or polygon outlines (ArcGIS Pro Feature To Polygon).",
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
                name: "cluster_tolerance",
                label: "XY Tolerance",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Coordinates this close are merged (default: the coordinate system's XY tolerance).",
            },
            Param {
                name: "attributes",
                label: "Preserve attributes",
                kind: Kind::Choice(&["ATTRIBUTES", "NO_ATTRIBUTES"]),
                required: false,
                default: "ATTRIBUTES",
                help: "Take the label points' attributes.",
            },
            Param {
                name: "label_features",
                label: "Label Features",
                kind: Kind::Features,
                required: false,
                default: "",
                help: "Points whose attributes the polygons containing them take (the first point in a polygon).",
            },
        ],
        output_suffix: "Polygon",
        measures: false,
        run: Runner::Features(feature_to_polygon),
    },
    Tool {
        id: "feature_to_line",
        name: "Feature To Line",
        toolbox: "Data Management › Features",
        summary: "Lines and polygon outlines split where they cross or meet (ArcGIS Pro Feature To Line).",
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
                name: "cluster_tolerance",
                label: "XY Tolerance",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "Coordinates this close are merged (default: the coordinate system's XY tolerance).",
            },
            Param {
                name: "attributes",
                label: "Preserve attributes",
                kind: Kind::Choice(&["ATTRIBUTES", "NO_ATTRIBUTES"]),
                required: false,
                default: "ATTRIBUTES",
                help: "Keep FID_<input> and the input's fields (one row per feature on a shared stretch).",
            },
        ],
        output_suffix: "Line",
        measures: false,
        run: Runner::Features(feature_to_line),
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
        summary: "Whether the values of a field cluster, disperse or are random in space (ArcGIS Pro Spatial Autocorrelation).",
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
                default: "ROW",
                help: "",
            },
            Param {
                name: "distance_band",
                label: "Distance Band or Threshold Distance",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "In the coordinate system's units; empty: the largest nearest-neighbor distance; 0 relates every pair for inverse distance.",
            },
            WEIGHTS_FILE,
            Param {
                name: "number_of_neighbors",
                label: "Number of Neighbors",
                kind: Kind::Number,
                required: false,
                default: "8",
                help: "For K_NEAREST_NEIGHBORS.",
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
                default: "ROW",
                help: "",
            },
            Param {
                name: "distance_band",
                label: "Distance Band or Threshold Distance",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "In the coordinate system's units; empty: the largest nearest-neighbor distance; 0 relates every pair for inverse distance.",
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
            WEIGHTS_FILE,
        ],
        output_suffix: "HotSpots",
        measures: true,
        run: Runner::Features(hot_spot_analysis),
    },
    Tool {
        id: "cluster_and_outlier_analysis",
        name: "Cluster and Outlier Analysis (Anselin Local Moran's I)",
        toolbox: "Spatial Statistics › Mapping Clusters",
        summary: "Clusters of high or low values and outliers among their neighbors, as local Moran's I with z-scores and HH/LL/HL/LH types (ArcGIS Pro Cluster and Outlier Analysis).",
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
                default: "ROW",
                help: "",
            },
            Param {
                name: "distance_band",
                label: "Distance Band or Threshold Distance",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "In the coordinate system's units; empty: the largest nearest-neighbor distance; 0 relates every pair for inverse distance.",
            },
            WEIGHTS_FILE,
            Param {
                name: "apply_fdr",
                label: "Apply False Discovery Rate (FDR) Correction",
                kind: Kind::Choice(&["NO_FDR", "APPLY_FDR"]),
                required: false,
                default: "NO_FDR",
                help: "",
            },
            Param {
                name: "number_of_permutations",
                label: "Number of Permutations",
                kind: Kind::Choice(&["0", "99", "199", "499", "999", "9999"]),
                required: false,
                default: "499",
                help: "0 gives analytic p-values; others pseudo p-values of random permutations.",
            },
            Param {
                name: "number_of_neighbors",
                label: "Number of Neighbors",
                kind: Kind::Number,
                required: false,
                default: "8",
                help: "For K_NEAREST_NEIGHBORS.",
            },
        ],
        output_suffix: "ClustersOutliers",
        measures: true,
        run: Runner::Features(cluster_and_outlier_analysis),
    },
    Tool {
        id: "multi_distance_spatial_clustering",
        name: "Multi-Distance Spatial Cluster Analysis (Ripley's k-function)",
        toolbox: "Spatial Statistics › Analyzing Patterns",
        summary: "Whether features cluster or disperse at a range of distances, as Ripley's L(d) per distance band (ArcGIS Pro Multi-Distance Spatial Cluster Analysis).",
        params: &[
            INPUT,
            Param {
                name: "number_of_distance_bands",
                label: "Number of Distance Bands",
                kind: Kind::Number,
                required: false,
                default: "10",
                help: "",
            },
            Param {
                name: "compute_confidence_envelope",
                label: "Compute Confidence Envelope",
                kind: Kind::Choice(&[
                    "0_PERMUTATIONS_-_NO_CONFIDENCE_ENVELOPE",
                    "9_PERMUTATIONS",
                    "99_PERMUTATIONS",
                    "999_PERMUTATIONS",
                ]),
                required: false,
                default: "0_PERMUTATIONS_-_NO_CONFIDENCE_ENVELOPE",
                help: "",
            },
            WEIGHT,
            Param {
                name: "beginning_distance",
                label: "Beginning Distance",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "Empty: one increment.",
            },
            Param {
                name: "distance_increment",
                label: "Distance Increment",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "Empty: a quarter of the study area's longer side over the number of bands.",
            },
            Param {
                name: "boundary_correction_method",
                label: "Boundary Correction Method",
                kind: Kind::Choice(&["NONE", "RIPLEY_EDGE_CORRECTION_FORMULA"]),
                required: false,
                default: "NONE",
                help: "",
            },
            Param {
                name: "study_area_method",
                label: "Study Area Method",
                kind: Kind::Choice(&[
                    "MINIMUM_ENCLOSING_RECTANGLE",
                    "USER_PROVIDED_STUDY_AREA_FEATURE_CLASS",
                ]),
                required: false,
                default: "MINIMUM_ENCLOSING_RECTANGLE",
                help: "",
            },
            Param {
                name: "study_area_feature_class",
                label: "Study Area Feature Class",
                kind: Kind::Features,
                required: false,
                default: "",
                help: "",
            },
        ],
        output_suffix: "KFunction",
        measures: true,
        run: Runner::Dynamic(multi_distance_spatial_clustering),
    },
    Tool {
        id: "ordinary_least_squares",
        name: "Ordinary Least Squares (OLS)",
        toolbox: "Spatial Statistics › Modeling Spatial Relationships",
        summary: "A global linear regression with coefficient and diagnostic tables and residuals per feature (ArcGIS Pro Ordinary Least Squares).",
        params: &[
            INPUT,
            Param {
                name: "unique_id_field",
                label: "Unique ID Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "dependent_variable",
                label: "Dependent Variable",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "explanatory_variables",
                label: "Explanatory Variables",
                kind: Kind::Text,
                required: true,
                default: "",
                help: "Numeric fields separated by ;.",
            },
            Param {
                name: "coefficient_output_table",
                label: "Coefficient Output Table",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "diagnostic_output_table",
                label: "Diagnostic Output Table",
                kind: Kind::Text,
                required: false,
                default: "",
                help: "",
            },
        ],
        output_suffix: "OLS",
        measures: false,
        run: Runner::Dynamic(ordinary_least_squares),
    },
    Tool {
        id: "collect_events",
        name: "Collect Events",
        toolbox: "Spatial Statistics › Utilities",
        summary: "One point per location with ICOUNT, the number of incidents there (ArcGIS Pro Collect Events).",
        params: &[INPUT],
        output_suffix: "CollectEvents",
        measures: false,
        run: Runner::Features(collect_events),
    },
    Tool {
        id: "generate_spatial_weights_matrix",
        name: "Generate Spatial Weights Matrix",
        toolbox: "Spatial Statistics › Modeling Spatial Relationships",
        summary: "Writes a spatial weights matrix file (.swm) of how the features relate (ArcGIS Pro Generate Spatial Weights Matrix; give output=<file.swm>).",
        params: &[
            INPUT,
            Param {
                name: "unique_id_field",
                label: "Unique ID Field",
                kind: Kind::Field {
                    of: "input",
                    numeric: true,
                },
                required: true,
                default: "",
                help: "An integer field with a different value for each feature.",
            },
            Param {
                name: "conceptualization",
                label: "Conceptualization of Spatial Relationships",
                kind: Kind::Choice(&[
                    "INVERSE_DISTANCE",
                    "FIXED_DISTANCE",
                    "K_NEAREST_NEIGHBORS",
                    "CONTIGUITY_EDGES_ONLY",
                    "CONTIGUITY_EDGES_CORNERS",
                    "DELAUNAY_TRIANGULATION",
                    "SPACE_TIME_WINDOW",
                    "CONVERT_TABLE",
                ]),
                required: true,
                default: "",
                help: "",
            },
            Param {
                name: "distance_method",
                label: "Distance Method",
                kind: Kind::Choice(&["EUCLIDEAN", "MANHATTAN"]),
                required: false,
                default: "EUCLIDEAN",
                help: "",
            },
            Param {
                name: "exponent",
                label: "Exponent",
                kind: Kind::Number,
                required: false,
                default: "1",
                help: "Inverse distance weights are 1 / distance^exponent.",
            },
            Param {
                name: "threshold_distance",
                label: "Threshold Distance",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "Empty: the largest nearest-neighbor distance.",
            },
            Param {
                name: "number_of_neighbors",
                label: "Number of Neighbors",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "K nearest neighbors (8 when empty); the least neighbors for distance bands.",
            },
            Param {
                name: "row_standardization",
                label: "Row Standardization",
                kind: Kind::Choice(&["ROW_STANDARDIZATION", "NO_STANDARDIZATION"]),
                required: false,
                default: "ROW_STANDARDIZATION",
                help: "",
            },
            Param {
                name: "input_table",
                label: "Input Table",
                kind: Kind::Features,
                required: false,
                default: "",
                help: "CONVERT_TABLE: a table of the unique ID field, NID and WEIGHT.",
            },
            field(
                "date_time_field",
                "Date/Time Field",
                false,
                "SPACE_TIME_WINDOW: the date field.",
            ),
            Param {
                name: "date_time_interval_type",
                label: "Date/Time Interval Type",
                kind: Kind::Choice(&[
                    "SECONDS", "MINUTES", "HOURS", "DAYS", "WEEKS", "MONTHS", "YEARS",
                ]),
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "date_time_interval_value",
                label: "Date/Time Interval Value",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "order",
                label: "Polygon Contiguity Order",
                kind: Kind::Number,
                required: false,
                default: "1",
                help: "",
            },
            Param {
                name: "include_focal_feature",
                label: "Include Focal Feature in Weights",
                kind: Kind::Choice(&["INCLUDE_FOCAL", "EXCLUDE_FOCAL"]),
                required: false,
                default: "EXCLUDE_FOCAL",
                help: "",
            },
            Param {
                name: "weighting_method",
                label: "Weighting Scheme",
                kind: Kind::Choice(&[
                    "UNWEIGHTED",
                    "BISQUARE",
                    "GAUSSIAN",
                    "TRIANGULAR",
                    "QUADRATIC",
                    "FIELD",
                    "BORDER",
                ]),
                required: false,
                default: "UNWEIGHTED",
                help: "",
            },
            Param {
                name: "kernel_type",
                label: "Kernel Bandwidth",
                kind: Kind::Choice(&["ADAPTIVE", "FIXED"]),
                required: false,
                default: "ADAPTIVE",
                help: "",
            },
            Param {
                name: "adaptive_neighbors",
                label: "Number of Neighbors for Adaptive Kernel",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "",
            },
            Param {
                name: "kernel_bandwidth",
                label: "Distance Band",
                kind: Kind::Number,
                required: false,
                default: "",
                help: "Fixed kernel bandwidth; empty: Silverman's rule.",
            },
            field(
                "weight_field",
                "Weight Field",
                true,
                "FIELD weighting: the weights.",
            ),
        ],
        output_suffix: "swm",
        measures: true,
        run: Runner::Report(generate_spatial_weights_matrix),
    },
    Tool {
        id: "convert_spatial_weights_matrix_to_table",
        name: "Convert Spatial Weights Matrix to Table",
        toolbox: "Spatial Statistics › Utilities",
        summary: "A table of the neighbor pairs and weights of a spatial weights matrix file (ArcGIS Pro Convert Spatial Weights Matrix to Table).",
        params: &[Param {
            name: "input",
            label: "Input Spatial Weights Matrix File",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "A .swm file.",
        }],
        output_suffix: "Table",
        measures: false,
        run: Runner::Dynamic(convert_spatial_weights_matrix_to_table),
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
        id: "table_select",
        name: "Table Select",
        toolbox: "Analysis › Extract",
        summary: "Copies the rows a where clause selects into a new table (ArcGIS Pro Table Select).",
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
        output_suffix: "Select",
        measures: false,
        run: Runner::Dynamic(table_select),
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
    crate::dm_tools::FEATURE_ENVELOPE_TO_POLYGON,
    crate::dm_tools::SPLIT_LINE_AT_VERTICES,
    crate::dm_tools::SPLIT_LINE_AT_POINT,
    crate::dm_tools::UNSPLIT_LINE,
    crate::dm_tools::DICE,
    crate::dm_tools::ELIMINATE_POLYGON_PART,
    crate::dm_tools::XY_TO_LINE,
    crate::dm_tools::CALCULATE_FIELDS,
    crate::dm_tools::STANDARDIZE_FIELD,
    crate::dm_tools::TRANSFORM_FIELD,
    crate::dm_tools::RECLASSIFY_FIELD,
    crate::dm_tools::TRANSPOSE_FIELDS,
    crate::dm_tools::PIVOT_TABLE,
    crate::dm_tools::BEARING_DISTANCE_TO_LINE,
    crate::dm_tools::CREATE_FEATURE_CLASS,
    crate::dm_tools::CREATE_TABLE,
    crate::dm_tools::DEFINE_PROJECTION,
    crate::dm_tools::COPY,
    crate::dm_tools::MULTIPLE_RING_BUFFER,
    crate::dm_tools::CALCULATE_END_TIME,
    crate::neighbors::SUBDIVIDE_POLYGON,
    crate::neighbors::TABULATE_INTERSECTION,
    crate::neighbors::CREATE_THIESSEN_POLYGONS,
    crate::neighbors::SIMPLIFY_LINE,
    crate::neighbors::GRID_INDEX_FEATURES,
    crate::neighbors::SUMMARIZE_WITHIN,
    crate::neighbors::SUMMARIZE_NEARBY,
    crate::graphic_buffer::GRAPHIC_BUFFER,
    crate::neighbors::APPORTION_POLYGON,
    crate::neighbors::POLYGON_NEIGHBORS,
    crate::pairwise_neighbors::TOOL,
    crate::split_attributes::TOOL,
    crate::gps_metadata::TOOL,
    crate::delete_multiple::TOOL,
    crate::cogo::DISABLE,
    crate::cogo::ENABLE,
    crate::editor_tracking::DISABLE,
    crate::editor_tracking::ENABLE,
    crate::date_precision::MIGRATE,
    crate::neighbors::COUNT_OVERLAPPING_FEATURES,
];
