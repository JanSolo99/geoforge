//! Registry entries of the Data Management breadth tools (see `dm_geometry` and `dm_fields`).

use geoforge_core::{Error, Result};

use crate::registry::{Context, Kind, Outcome, Param, Runner, Tool, distance_in_units};
use crate::{TableOutput, ToolOutput, dm_fields, dm_geometry};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

const fn features(name: &'static str, label: &'static str) -> Param {
    Param {
        name,
        label,
        kind: Kind::Features,
        required: true,
        default: "",
        help: "",
    }
}

const fn text(
    name: &'static str,
    label: &'static str,
    required: bool,
    default: &'static str,
    help: &'static str,
) -> Param {
    Param {
        name,
        label,
        kind: Kind::Text,
        required,
        default,
        help,
    }
}

const fn number(
    name: &'static str,
    label: &'static str,
    default: &'static str,
    help: &'static str,
) -> Param {
    Param {
        name,
        label,
        kind: Kind::Number,
        required: false,
        default,
        help,
    }
}

const fn choice(
    name: &'static str,
    label: &'static str,
    options: &'static [&'static str],
    default: &'static str,
) -> Param {
    Param {
        name,
        label,
        kind: Kind::Choice(options),
        required: false,
        default,
        help: "",
    }
}

const fn field(name: &'static str, label: &'static str, required: bool) -> Param {
    Param {
        name,
        label,
        kind: Kind::Field {
            of: "input",
            numeric: false,
        },
        required,
        default: "",
        help: "",
    }
}

fn get(ctx: &dyn Context, name: &str, default: &str) -> String {
    ctx.get(name).unwrap_or_else(|| default.to_owned())
}

fn num(ctx: &dyn Context, name: &str) -> Option<f64> {
    ctx.get(name).and_then(|v| v.trim().parse().ok())
}

fn list(text: &str) -> Vec<String> {
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn feature_envelope_to_polygon(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    let multipart = get(ctx, "single_envelope", "SINGLEPART").eq_ignore_ascii_case("MULTIPART");
    dm_geometry::feature_envelope_to_polygon(fc.as_ref(), multipart)
}

fn split_line_at_vertices(ctx: &dyn Context) -> Result<ToolOutput> {
    dm_geometry::split_line_at_vertices(ctx.features("input")?.as_ref())
}

fn split_line_at_point(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    let points = ctx.features("points")?;
    let radius = ctx
        .get("search_radius")
        .ok_or_else(|| err("ERROR 000735: Search Radius: Value is required"))?;
    let wkt = fc.spatial_reference_wkt().map(str::to_owned);
    let radius = distance_in_units(&radius, wkt.as_deref())?;
    let tolerance = crate::default_xy_tolerance(wkt.as_deref());
    dm_geometry::split_line_at_point(fc.as_ref(), points.as_ref(), radius, tolerance)
}

fn eliminate_polygon_part(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    let condition = dm_geometry::Condition::parse(&get(ctx, "condition", "AREA"))
        .ok_or_else(|| err("ERROR 000067: unknown condition"))?;
    let area_text = get(ctx, "part_area", "0");
    let leading: String = area_text
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        .collect();
    let area = leading
        .parse::<f64>()
        .map_err(|_| err(format!("ERROR 000094: '{area_text}' is not an area")))?;
    let percent = num(ctx, "part_area_percent").unwrap_or(0.0);
    let any = get(ctx, "part_option", "CONTAINED_ONLY").eq_ignore_ascii_case("ANY");
    dm_geometry::eliminate_polygon_part(fc.as_ref(), condition, area, percent, any)
}

fn xy_to_line(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    let need = |n: &str| {
        ctx.get(n)
            .ok_or_else(|| err(format!("ERROR 000735: {n}: Value is required")))
    };
    let coordinates = [
        need("startx_field")?,
        need("starty_field")?,
        need("endx_field")?,
        need("endy_field")?,
    ];
    let wkt = match ctx.get("spatial_reference") {
        #[cfg(feature = "proj")]
        Some(spec) => Some(geoforge_proj::Crs::parse(&spec)?.esri_wkt().to_owned()),
        #[cfg(not(feature = "proj"))]
        Some(_) => return Err(err("spatial_reference needs the proj feature")),
        None => fc.spatial_reference_wkt().map(str::to_owned),
    };
    let attributes = get(ctx, "attributes", "NO_ATTRIBUTES").eq_ignore_ascii_case("ATTRIBUTES");
    dm_geometry::xy_to_line(
        fc.as_ref(),
        [
            &coordinates[0],
            &coordinates[1],
            &coordinates[2],
            &coordinates[3],
        ],
        ctx.get("id_field").as_deref(),
        wkt,
        attributes,
    )
}

fn calculate_fields(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    let spec = ctx
        .get("fields")
        .ok_or_else(|| err("ERROR 000735: Fields: Value is required"))?;
    let calculations: Vec<dm_fields::FieldCalculation> = list(&spec)
        .iter()
        .map(|i| dm_fields::FieldCalculation::parse(i))
        .collect::<Result<_>>()?;
    let code = ctx.get("code_block").unwrap_or_default();
    dm_fields::calculate_fields(fc.as_ref(), &calculations, &code)
}

fn field_list(ctx: &dyn Context) -> Result<Vec<(String, Option<String>)>> {
    let spec = ctx
        .get("fields")
        .ok_or_else(|| err("ERROR 000735: Fields: Value is required"))?;
    Ok(dm_fields::parse_field_list(&spec))
}

fn standardize_field(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    dm_fields::standardize_field(
        fc.as_ref(),
        &field_list(ctx)?,
        &get(ctx, "method", "Z-SCORE"),
        num(ctx, "min_value").unwrap_or(0.0),
        num(ctx, "max_value").unwrap_or(1.0),
    )
}

fn transform_field(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    dm_fields::transform_field(
        fc.as_ref(),
        &field_list(ctx)?,
        &get(ctx, "method", "BOX-COX"),
        num(ctx, "power"),
        num(ctx, "shift"),
    )
}

fn reclassify_field(ctx: &dyn Context) -> Result<ToolOutput> {
    use dm_fields::Reclass;
    let fc = ctx.features("input")?;
    let field = ctx
        .get("field")
        .ok_or_else(|| err("ERROR 000735: Field: Value is required"))?;
    let classes = num(ctx, "classes").map(|c| c as usize);
    let method = match get(ctx, "method", "EQUAL_INTERVAL")
        .to_ascii_uppercase()
        .as_str()
    {
        "EQUAL_INTERVAL" => Reclass::EqualInterval(classes.unwrap_or(5)),
        "QUANTILE" => Reclass::Quantile(classes.unwrap_or(5)),
        "DEFINED_INTERVAL" => Reclass::DefinedInterval(
            num(ctx, "interval").ok_or_else(|| err("ERROR 000735: Interval: Value is required"))?,
        ),
        "UNIQUE_VALUES" => Reclass::UniqueValues,
        "MANUAL" => {
            let table = ctx
                .get("reclass_table")
                .ok_or_else(|| err("ERROR 000735: Reclass Table: Value is required"))?;
            let rows = list(&table)
                .iter()
                .map(|r| {
                    let mut w = r.split_whitespace();
                    let bound = w.next().and_then(|v| v.parse::<f64>().ok());
                    let value = w.next().and_then(|v| v.parse::<i32>().ok());
                    bound
                        .zip(value)
                        .ok_or_else(|| err(format!("'{r}': expected UPPER_BOUND VALUE")))
                })
                .collect::<Result<Vec<_>>>()?;
            Reclass::Manual(rows)
        }
        other => {
            return Err(err(format!(
                "ERROR 000067: method {other} is not supported"
            )));
        }
    };
    let descending = get(ctx, "reverse_values", "ASC").eq_ignore_ascii_case("DESC");
    dm_fields::reclassify_field(
        fc.as_ref(),
        &field,
        &method,
        descending,
        ctx.get("output_field_name").as_deref(),
    )
}

fn transpose_fields(ctx: &dyn Context) -> Result<Outcome> {
    let fc = ctx.features("input")?;
    let table: TableOutput = dm_fields::transpose_fields(
        fc.as_ref(),
        &field_list(ctx)?,
        &get(ctx, "transposed_field_name", "FIELD"),
        &get(ctx, "value_field_name", "VALUE"),
        &list(&get(ctx, "attribute_fields", "")),
    )?;
    Ok(Outcome::Table(table))
}

fn pivot_table(ctx: &dyn Context) -> Result<Outcome> {
    let fc = ctx.features("input")?;
    let pivot = ctx
        .get("pivot_field")
        .ok_or_else(|| err("ERROR 000735: Pivot Field: Value is required"))?;
    let value = ctx
        .get("value_field")
        .ok_or_else(|| err("ERROR 000735: Value Field: Value is required"))?;
    let keys = list(&get(ctx, "fields", ""));
    Ok(Outcome::Table(dm_fields::pivot_table(
        fc.as_ref(),
        &keys,
        &pivot,
        &value,
    )?))
}

pub const FEATURE_ENVELOPE_TO_POLYGON: Tool = Tool {
    id: "feature_envelope_to_polygon",
    name: "Feature Envelope To Polygon",
    toolbox: "Data Management › Features",
    summary: "Polygons of the envelopes of features or of their parts (ArcGIS Pro Feature Envelope To Polygon).",
    params: &[
        features("input", "Input Features"),
        choice(
            "single_envelope",
            "Create Envelopes For",
            &["SINGLEPART", "MULTIPART"],
            "SINGLEPART",
        ),
    ],
    output_suffix: "Envelope",
    measures: false,
    run: Runner::Features(feature_envelope_to_polygon),
};

pub const SPLIT_LINE_AT_VERTICES: Tool = Tool {
    id: "split_line_at_vertices",
    name: "Split Line At Vertices",
    toolbox: "Data Management › Features",
    summary: "One line per segment of the input lines, with ORIG_FID and ORIG_SEQ (ArcGIS Pro Split Line At Vertices).",
    params: &[features("input", "Input Features")],
    output_suffix: "Split",
    measures: false,
    run: Runner::Features(split_line_at_vertices),
};

pub const SPLIT_LINE_AT_POINT: Tool = Tool {
    id: "split_line_at_point",
    name: "Split Line At Point",
    toolbox: "Data Management › Features",
    summary: "Splits lines where points within a search radius are nearest to them (ArcGIS Pro Split Line At Point).",
    params: &[
        features("input", "Input Features"),
        features("points", "Point Features"),
        text(
            "search_radius",
            "Search Radius",
            true,
            "",
            "A distance, such as `10 Meters`.",
        ),
    ],
    output_suffix: "SplitAtPoint",
    measures: false,
    run: Runner::Features(split_line_at_point),
};

pub const ELIMINATE_POLYGON_PART: Tool = Tool {
    id: "eliminate_polygon_part",
    name: "Eliminate Polygon Part",
    toolbox: "Data Management › Generalization",
    summary: "Removes small holes or parts of polygons by area and/or percent (ArcGIS Pro Eliminate Polygon Part).",
    params: &[
        features("input", "Input Features"),
        choice(
            "condition",
            "Condition",
            &["AREA", "PERCENT", "AREA_AND_PERCENT", "AREA_OR_PERCENT"],
            "AREA",
        ),
        text(
            "part_area",
            "Area",
            false,
            "0",
            "Parts smaller than this area go (coordinate units squared).",
        ),
        number(
            "part_area_percent",
            "Percentage",
            "0",
            "Parts smaller than this percent go.",
        ),
        choice(
            "part_option",
            "Eliminate contained part only",
            &["CONTAINED_ONLY", "ANY"],
            "CONTAINED_ONLY",
        ),
    ],
    output_suffix: "Eliminate",
    measures: true,
    run: Runner::Features(eliminate_polygon_part),
};

pub const XY_TO_LINE: Tool = Tool {
    id: "xy_to_line",
    name: "XY To Line",
    toolbox: "Data Management › Features",
    summary: "Straight lines from start and end coordinates in a table (ArcGIS Pro XY To Line).",
    params: &[
        features("input", "Input Table"),
        field("startx_field", "Start X Field", true),
        field("starty_field", "Start Y Field", true),
        field("endx_field", "End X Field", true),
        field("endy_field", "End Y Field", true),
        choice(
            "line_type",
            "Line Type",
            &["GEODESIC", "GREAT_CIRCLE", "RHUMB_LINE", "NORMAL_SECTION"],
            "GEODESIC",
        ),
        field("id_field", "ID Field", false),
        text(
            "spatial_reference",
            "Spatial Reference",
            false,
            "",
            "A factory code, a .prj file or WKT.",
        ),
        choice(
            "attributes",
            "Preserve attributes",
            &["NO_ATTRIBUTES", "ATTRIBUTES"],
            "NO_ATTRIBUTES",
        ),
    ],
    output_suffix: "Lines",
    measures: false,
    run: Runner::Features(xy_to_line),
};

pub const CALCULATE_FIELDS: Tool = Tool {
    id: "calculate_fields",
    name: "Calculate Fields",
    toolbox: "Data Management › Fields",
    summary: "Calculates several fields in turn with Python expressions (ArcGIS Pro Calculate Fields, PYTHON3).",
    params: &[
        features("input", "Input Table"),
        text(
            "fields",
            "Fields",
            true,
            "",
            "NAME=EXPRESSION[|WHERE[|TYPE]] items separated by semicolons.",
        ),
        text(
            "code_block",
            "Code Block",
            false,
            "",
            "Python definitions the expressions use.",
        ),
    ],
    output_suffix: "Calc",
    measures: false,
    run: Runner::Features(calculate_fields),
};

pub const STANDARDIZE_FIELD: Tool = Tool {
    id: "standardize_field",
    name: "Standardize Field",
    toolbox: "Data Management › Fields › Data Engineering",
    summary: "Adds standardized copies of numeric fields (ArcGIS Pro Standardize Field).",
    params: &[
        features("input", "Input Table"),
        text(
            "fields",
            "Fields",
            true,
            "",
            "FIELD [OUTPUT_NAME] items separated by semicolons.",
        ),
        choice(
            "method",
            "Method",
            &["Z-SCORE", "MIN-MAX", "MAXABS", "ROBUST"],
            "Z-SCORE",
        ),
        number("min_value", "Minimum", "0", "For MIN-MAX."),
        number("max_value", "Maximum", "1", "For MIN-MAX."),
    ],
    output_suffix: "Standardized",
    measures: false,
    run: Runner::Features(standardize_field),
};

pub const TRANSFORM_FIELD: Tool = Tool {
    id: "transform_field",
    name: "Transform Field",
    toolbox: "Data Management › Fields › Data Engineering",
    summary: "Adds transformed copies of numeric fields (ArcGIS Pro Transform Field).",
    params: &[
        features("input", "Input Table"),
        text(
            "fields",
            "Fields",
            true,
            "",
            "FIELD [OUTPUT_NAME] items separated by semicolons.",
        ),
        choice(
            "method",
            "Method",
            &[
                "INVX",
                "SQRT",
                "LOG",
                "BOX-COX",
                "INV_BOX-COX",
                "INV_SQRT",
                "INV_LOG",
            ],
            "BOX-COX",
        ),
        number("power", "Power", "", "The Box-Cox exponent."),
        number("shift", "Shift", "", "A constant added to the data."),
    ],
    output_suffix: "Transformed",
    measures: false,
    run: Runner::Features(transform_field),
};

pub const RECLASSIFY_FIELD: Tool = Tool {
    id: "reclassify_field",
    name: "Reclassify Field",
    toolbox: "Data Management › Fields › Data Engineering",
    summary: "Adds a class field (and its range as text) for a field (ArcGIS Pro Reclassify Field).",
    params: &[
        features("input", "Input Table"),
        field("field", "Field", true),
        choice(
            "method",
            "Method",
            &[
                "EQUAL_INTERVAL",
                "DEFINED_INTERVAL",
                "QUANTILE",
                "MANUAL",
                "UNIQUE_VALUES",
            ],
            "EQUAL_INTERVAL",
        ),
        number("classes", "Classes", "", "The number of classes."),
        number("interval", "Interval", "", "For DEFINED_INTERVAL."),
        text(
            "reclass_table",
            "Reclass Table",
            false,
            "",
            "UPPER_BOUND VALUE items separated by semicolons (MANUAL).",
        ),
        choice("reverse_values", "Reverse Values", &["ASC", "DESC"], "ASC"),
        text("output_field_name", "Output Field Name", false, "", ""),
    ],
    output_suffix: "Reclassified",
    measures: false,
    run: Runner::Features(reclassify_field),
};

pub const TRANSPOSE_FIELDS: Tool = Tool {
    id: "transpose_fields",
    name: "Transpose Fields",
    toolbox: "Data Management › Fields",
    summary: "A row per field and input row (ArcGIS Pro Transpose Fields).",
    params: &[
        features("input", "Input Table"),
        text(
            "fields",
            "Fields",
            true,
            "",
            "FIELD [ALIAS] items separated by semicolons.",
        ),
        text(
            "transposed_field_name",
            "Transposed Field Name",
            false,
            "FIELD",
            "",
        ),
        text("value_field_name", "Value Field Name", false, "VALUE", ""),
        text(
            "attribute_fields",
            "Attribute Fields",
            false,
            "",
            "Fields kept, separated by semicolons.",
        ),
    ],
    output_suffix: "Transposed",
    measures: false,
    run: Runner::Dynamic(transpose_fields),
};

pub const PIVOT_TABLE: Tool = Tool {
    id: "pivot_table",
    name: "Pivot Table",
    toolbox: "Data Management › Table",
    summary: "Turns the values of a field into columns (ArcGIS Pro Pivot Table).",
    params: &[
        features("input", "Input Table"),
        text(
            "fields",
            "Input Fields",
            true,
            "",
            "Key fields separated by semicolons.",
        ),
        field("pivot_field", "Pivot Field", true),
        field("value_field", "Value Field", true),
    ],
    output_suffix: "Pivot",
    measures: false,
    run: Runner::Dynamic(pivot_table),
};
