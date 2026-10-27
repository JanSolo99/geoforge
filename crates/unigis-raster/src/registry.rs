//! One description of each raster tool, so the command line and the reference tests need no code
//! of their own for it: `ug run nibble input=a.tif mask=m.tif output=out.tif`.
//!
//! A new Spatial Analyst tool is a function from a [`Context`] to a [`Raster`] plus one entry in
//! [`TOOLS`]. The ArcGIS reference script names the tool's parameters in the same way (see
//! `RUN` in `scripts/make-arcgis-reference.py`), and the reference test runs the registry
//! command from that.

use unigis_core::{Error, Result};

use crate::Raster;

/// What a parameter holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A raster path.
    Raster,
    Choice(&'static [&'static str]),
    Number,
    Text,
}

#[derive(Debug, Clone, Copy)]
pub struct Param {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub required: bool,
    /// Used when none is given (empty for none).
    pub default: &'static str,
}

/// How a tool reads its parameters.
pub trait Context {
    /// The parameter's value as text, when it was given and is not empty.
    fn get(&self, name: &str) -> Option<String>;
    /// The raster the parameter names.
    fn raster(&self, name: &str) -> Result<Raster>;
    /// The raster at `path`.
    fn raster_at(&self, path: &str) -> Result<Raster>;
    /// The numeric columns (name, values) of the attribute table (`.vat.dbf`) of the raster the
    /// parameter names.
    fn attribute_table(&self, name: &str) -> Result<Vec<(String, Vec<f64>)>>;
}

/// A tool's result: the raster, and whether ArcGIS writes statistics (`.aux.xml`) beside it.
pub struct Output {
    pub raster: Raster,
    pub statistics: bool,
    /// ArcGIS writes a STATISTICS_COUNT of 0 (Basin, Sink).
    pub zero_count: bool,
}

pub struct Tool {
    pub id: &'static str,
    /// The ArcGIS name, e.g. `Nibble`.
    pub name: &'static str,
    pub summary: &'static str,
    pub params: &'static [Param],
    pub run: fn(&dyn Context) -> Result<Output>,
}

pub fn find(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id.eq_ignore_ascii_case(id))
}

/// The parameter's value, or its default.
pub fn value(ctx: &dyn Context, p: &Param) -> Option<String> {
    ctx.get(p.name)
        .or_else(|| (!p.default.is_empty()).then(|| p.default.to_string()))
}

/// Runs `tool` after checking the parameters (required ones present, choices valid, numbers parse).
pub fn run(tool: &Tool, ctx: &dyn Context) -> Result<Output> {
    for p in tool.params {
        let Some(v) = value(ctx, p) else {
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
    (tool.run)(ctx)
}

fn choice(ctx: &dyn Context, tool: &str, name: &str) -> String {
    find(tool)
        .and_then(|t| t.params.iter().find(|p| p.name == name))
        .and_then(|p| value(ctx, p))
        .unwrap_or_default()
}

const fn raster(name: &'static str, label: &'static str, required: bool) -> Param {
    Param {
        name,
        label,
        kind: Kind::Raster,
        required,
        default: "",
    }
}

const fn option(
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
    }
}

fn nibble(ctx: &dyn Context) -> Result<Output> {
    let input = ctx.raster("input")?;
    let mask = ctx.raster("mask")?;
    let raster = crate::resample::nibble(
        &input,
        &mask,
        choice(ctx, "nibble", "values").eq_ignore_ascii_case("DATA_ONLY"),
        !choice(ctx, "nibble", "nodata").eq_ignore_ascii_case("PROCESS_NODATA"),
    )
    .ok_or_else(|| Error::Unsupported("the rasters must share a grid".into()))?;
    Ok(Output {
        raster,
        statistics: false,
        zero_count: false,
    })
}

fn filter(ctx: &dyn Context) -> Result<Output> {
    let low = choice(ctx, "filter", "filter_type").eq_ignore_ascii_case("LOW");
    let data = choice(ctx, "filter", "ignore_nodata").eq_ignore_ascii_case("DATA");
    with_statistics(crate::focal::filter(&ctx.raster("input")?, low, data), true)
}

fn parse_numbers(text: &str, count: usize, label: &str) -> Result<Vec<f64>> {
    let v: Vec<f64> = text
        .split([' ', ',', ';'])
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect();
    if v.len() == count {
        Ok(v)
    } else {
        Err(Error::Unsupported(format!(
            "ERROR 000094: {label}: expected {count} numbers"
        )))
    }
}

fn inside(ctx: &dyn Context, tool: &str) -> bool {
    !choice(ctx, tool, "extraction_area").eq_ignore_ascii_case("OUTSIDE")
}

fn with_statistics(raster: Raster, statistics: bool) -> Result<Output> {
    Ok(Output {
        raster,
        statistics,
        zero_count: false,
    })
}

fn extract_by_rectangle(ctx: &dyn Context) -> Result<Output> {
    let text = ctx.get("rectangle").unwrap_or_default();
    let r = parse_numbers(&text, 4, "Extent")?;
    with_statistics(
        crate::extract::extract_by_rectangle(
            &ctx.raster("input")?,
            (r[0], r[1], r[2], r[3]),
            inside(ctx, "extract_by_rectangle"),
        ),
        false,
    )
}

fn extract_by_circle(ctx: &dyn Context) -> Result<Output> {
    let c = parse_numbers(
        &ctx.get("center_point").unwrap_or_default(),
        2,
        "Center Point",
    )?;
    let radius: f64 = ctx
        .get("radius")
        .and_then(|r| r.parse().ok())
        .ok_or_else(|| Error::Unsupported("ERROR 000735: Radius: value is required".into()))?;
    with_statistics(
        crate::extract::extract_by_circle(
            &ctx.raster("input")?,
            (c[0], c[1]),
            radius,
            inside(ctx, "extract_by_circle"),
        ),
        false,
    )
}

fn extract_by_attributes(ctx: &dyn Context) -> Result<Output> {
    let text = ctx.get("where_clause").unwrap_or_default();
    let clause = crate::local::ValueClause::parse(&text).ok_or_else(|| {
        Error::Unsupported(format!(
            "ERROR 000358: Invalid expression: {text} (only comparisons of VALUE with numbers are supported)"
        ))
    })?;
    with_statistics(
        crate::extract::extract_by_attributes(&ctx.raster("input")?, &clause),
        true,
    )
}

fn slice(ctx: &dyn Context) -> Result<Output> {
    let zones: usize = ctx
        .get("number_zones")
        .and_then(|n| n.parse().ok())
        .filter(|&n| n >= 1)
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Number of zones: value is required".into())
        })?;
    let kind = match choice(ctx, "slice", "slice_type")
        .to_ascii_uppercase()
        .as_str()
    {
        "EQUAL_INTERVAL" => crate::extract::SliceType::EqualInterval,
        "EQUAL_AREA" => crate::extract::SliceType::EqualArea,
        other => return Err(Error::Unsupported(format!("slice type {other}"))),
    };
    let base: i64 = choice(ctx, "slice", "base_output_zone")
        .parse()
        .unwrap_or(1);
    with_statistics(
        crate::extract::slice(&ctx.raster("input")?, zones, kind, base),
        false,
    )
}

fn lookup(ctx: &dyn Context) -> Result<Output> {
    let field = ctx.get("lookup_field").unwrap_or_default();
    let columns = ctx.attribute_table("input")?;
    let column = |name: &str| columns.iter().find(|c| c.0.eq_ignore_ascii_case(name));
    let values = column("VALUE")
        .ok_or_else(|| Error::Unsupported("the raster has no VALUE field".into()))?;
    let picked = column(&field).ok_or_else(|| {
        Error::Unsupported(format!(
            "ERROR 001000: Lookup field: Field {field} does not exist"
        ))
    })?;
    let table: Vec<(f64, f64)> = values
        .1
        .iter()
        .copied()
        .zip(picked.1.iter().copied())
        .collect();
    with_statistics(
        crate::extract::lookup(
            &ctx.raster("input")?,
            &table,
            field.eq_ignore_ascii_case("VALUE"),
        ),
        true,
    )
}

fn pick(ctx: &dyn Context) -> Result<Output> {
    let position = ctx.raster("position")?;
    let mut items = Vec::new();
    for item in ctx.get("items").unwrap_or_default().split(';') {
        let item = item.trim();
        items.push(match crate::local::Operand::constant(item) {
            Some(c) => c,
            None => crate::local::Operand::Raster(ctx.raster_at(item)?),
        });
    }
    let raster = crate::local::pick(&position, &items)
        .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
    with_statistics(raster, true)
}

fn sink(ctx: &dyn Context) -> Result<Output> {
    Ok(Output {
        zero_count: true,
        ..with_statistics(crate::hydro::sink(&ctx.raster("input")?), true)?
    })
}

/// One-raster math tools (`unary_tool!(fn name, id, ArcGIS name, op)`).
macro_rules! unary_fn {
    ($f:ident, $op:ident) => {
        fn $f(ctx: &dyn Context) -> Result<Output> {
            with_statistics(
                crate::local::unary(crate::local::UnaryOp::$op, &ctx.raster("input")?),
                true,
            )
        }
    };
}
macro_rules! unary_tool {
    ($f:ident, $id:literal, $name:literal) => {
        Tool {
            id: $id,
            name: $name,
            summary: "Spatial Analyst Math tool on one raster.",
            params: &[raster("input", "Input Raster", true)],
            run: $f,
        }
    };
}
unary_fn!(abs_tool, Abs);
unary_fn!(negate_tool, Negate);
unary_fn!(square_tool, Square);
unary_fn!(square_root_tool, Sqrt);
unary_fn!(exp_tool, Exp);
unary_fn!(exp2_tool, Exp2);
unary_fn!(exp10_tool, Exp10);
unary_fn!(ln_tool, Ln);
unary_fn!(log2_tool, Log2);
unary_fn!(log10_tool, Log10);
unary_fn!(sin_tool, Sin);
unary_fn!(cos_tool, Cos);
unary_fn!(tan_tool, Tan);
unary_fn!(asin_tool, ASin);
unary_fn!(acos_tool, ACos);
unary_fn!(atan_tool, ATan);
unary_fn!(sinh_tool, SinH);
unary_fn!(cosh_tool, CosH);
unary_fn!(tanh_tool, TanH);
unary_fn!(asinh_tool, ASinH);
unary_fn!(acosh_tool, ACosH);
unary_fn!(atanh_tool, ATanH);
unary_fn!(round_down_tool, RoundDown);
unary_fn!(round_up_tool, RoundUp);
unary_fn!(boolean_not_tool, BooleanNot);
unary_fn!(bitwise_not_tool, BitwiseNot);

fn operand(ctx: &dyn Context, name: &str) -> Result<crate::local::Operand> {
    let text = ctx.get(name).unwrap_or_default();
    Ok(match crate::local::Operand::constant(&text) {
        Some(c) => c,
        None => crate::local::Operand::Raster(ctx.raster_at(text.trim())?),
    })
}

macro_rules! binary_fn {
    ($f:ident, $op:ident) => {
        fn $f(ctx: &dyn Context) -> Result<Output> {
            let raster = crate::local::binary(
                crate::local::BinaryOp::$op,
                &operand(ctx, "in1")?,
                &operand(ctx, "in2")?,
            )
            .ok_or_else(|| Error::Unsupported("at least one input must be a raster".into()))?;
            with_statistics(raster, true)
        }
    };
}
macro_rules! binary_tool {
    ($f:ident, $id:literal, $name:literal) => {
        Tool {
            id: $id,
            name: $name,
            summary: "Spatial Analyst Math tool on two rasters or constants.",
            params: &[
                Param {
                    name: "in1",
                    label: "Input Raster Or Constant Value 1",
                    kind: Kind::Text,
                    required: true,
                    default: "",
                },
                Param {
                    name: "in2",
                    label: "Input Raster Or Constant Value 2",
                    kind: Kind::Text,
                    required: true,
                    default: "",
                },
            ],
            run: $f,
        }
    };
}
binary_fn!(atan2_tool, ATan2);
binary_fn!(power_tool, Power);
binary_fn!(mod_tool, Mod);
binary_fn!(boolean_and_tool, BooleanAnd);
binary_fn!(boolean_or_tool, BooleanOr);
binary_fn!(boolean_xor_tool, BooleanXor);
binary_fn!(equal_to_tool, EqualTo);
binary_fn!(greater_than_tool, GreaterThan);
binary_fn!(greater_than_equal_tool, GreaterThanEqual);
binary_fn!(less_than_tool, LessThan);
binary_fn!(less_than_equal_tool, LessThanEqual);
binary_fn!(not_equal_tool, NotEqual);
binary_fn!(bitwise_and_tool, BitwiseAnd);
binary_fn!(bitwise_or_tool, BitwiseOr);
binary_fn!(bitwise_xor_tool, BitwiseXor);
binary_fn!(bitwise_left_shift_tool, BitwiseLeftShift);
binary_fn!(bitwise_right_shift_tool, BitwiseRightShift);

/// The raster is passed through; `ug run` writes it as text (see `run_raster_tool`).
fn raster_to_ascii(ctx: &dyn Context) -> Result<Output> {
    with_statistics(ctx.raster("input")?, false)
}

macro_rules! combinatorial_fn {
    ($f:ident, $op:ident) => {
        fn $f(ctx: &dyn Context) -> Result<Output> {
            let raster = crate::local::combinatorial(
                crate::local::CombinatorialOp::$op,
                &ctx.raster("in1")?,
                &ctx.raster("in2")?,
            )
            .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
            with_statistics(raster, true)
        }
    };
}
combinatorial_fn!(combinatorial_and_tool, And);
combinatorial_fn!(combinatorial_or_tool, Or);
combinatorial_fn!(combinatorial_xor_tool, Xor);

/// Reclass by ASCII File: the remap file has one `value : new` or `from to : new` per line. A
/// `NODATA` new value is read as 0 (observed).
fn reclass_by_ascii_file(ctx: &dyn Context) -> Result<Output> {
    let path = ctx.get("remap_file").unwrap_or_default();
    let text = std::fs::read_to_string(&path)
        .map_err(|e| Error::Unsupported(format!("ERROR 000732: {path}: {e}")))?;
    let mut remap = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let bad = || Error::Unsupported(format!("ERROR 000094: bad remap line '{line}'"));
        let (lhs, rhs) = line.split_once(':').ok_or_else(bad)?;
        let nums: Vec<f64> = lhs
            .split_whitespace()
            .map(|t| t.parse().map_err(|_| bad()))
            .collect::<Result<_>>()?;
        let new = rhs.trim().parse().unwrap_or(0.0);
        let (from, to) = match nums[..] {
            [v] => (v, v),
            [a, b] => (a, b),
            _ => return Err(bad()),
        };
        remap.push(crate::zonal::Remap {
            from,
            to,
            new: Some(new),
        });
    }
    let keep =
        !choice(ctx, "reclass_by_ascii_file", "missing_values").eq_ignore_ascii_case("NODATA");
    with_statistics(
        crate::zonal::reclassify_ascii(&ctx.raster("input")?, &remap, keep),
        false,
    )
}

fn copy_raster(ctx: &dyn Context) -> Result<Output> {
    use crate::DataType;
    let pixel_type = match ctx
        .get("pixel_type")
        .map(|v| v.to_ascii_uppercase())
        .as_deref()
    {
        None => None,
        Some("8_BIT_UNSIGNED") => Some(DataType::U8),
        Some("8_BIT_SIGNED") => Some(DataType::I8),
        Some("16_BIT_UNSIGNED") => Some(DataType::U16),
        Some("16_BIT_SIGNED") => Some(DataType::I16),
        Some("32_BIT_UNSIGNED") => Some(DataType::U32),
        Some("32_BIT_SIGNED") => Some(DataType::I32),
        Some("32_BIT_FLOAT") => Some(DataType::F32),
        Some("64_BIT") => Some(DataType::F64),
        Some(other) => {
            return Err(Error::Unsupported(format!(
                "ERROR 000067: Pixel Type: {other} is not supported yet"
            )));
        }
    };
    let nodata = ctx.get("nodata_value").and_then(|v| v.trim().parse().ok());
    let input = ctx.raster("input")?;
    // No statistics beside a copy of a raster that has NoData cells (observed).
    let has_nodata = input.bands[0].iter().any(|&v| input.is_nodata(v));
    with_statistics(
        crate::extract::copy_raster(&input, pixel_type, nodata),
        !has_nodata,
    )
}

fn flip(ctx: &dyn Context) -> Result<Output> {
    with_statistics(crate::extract::flip(&ctx.raster("input")?), true)
}

fn mirror(ctx: &dyn Context) -> Result<Output> {
    with_statistics(crate::extract::mirror(&ctx.raster("input")?), true)
}

fn number(ctx: &dyn Context, name: &str) -> f64 {
    ctx.get(name).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn shift(ctx: &dyn Context) -> Result<Output> {
    let raster = crate::extract::shift(
        &ctx.raster("input")?,
        number(ctx, "x_value"),
        number(ctx, "y_value"),
    );
    with_statistics(raster, true)
}

fn clip_raster(ctx: &dyn Context) -> Result<Output> {
    let r = parse_numbers(&ctx.get("rectangle").unwrap_or_default(), 4, "Rectangle")?;
    if !choice(ctx, "clip_raster", "clipping_geometry").eq_ignore_ascii_case("NONE") {
        return Err(Error::Unsupported(
            "clipping to geometry is not supported yet".into(),
        ));
    }
    let nodata = ctx.get("nodata_value").and_then(|v| v.parse().ok());
    let maintain =
        choice(ctx, "clip_raster", "maintain_extent").eq_ignore_ascii_case("MAINTAIN_EXTENT");
    let raster = crate::extract::clip_rectangle(
        &ctx.raster("input")?,
        (r[0], r[1], r[2], r[3]),
        nodata,
        maintain,
    )
    .ok_or_else(|| {
        Error::Unsupported("ERROR 000008: the rectangle does not overlap the raster".into())
    })?;
    with_statistics(raster, true)
}

const EXTRACTION_AREA: &[&str] = &["INSIDE", "OUTSIDE"];

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "pick",
        name: "Pick",
        summary: "Each cell takes the value of the item its position raster value selects.",
        params: &[
            raster("position", "Input Position Raster", true),
            Param {
                name: "items",
                label: "Input Rasters or Constant Values",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: pick,
    },
    Tool {
        id: "sink",
        name: "Sink",
        summary: "The cells of a flow direction raster that drain only into each other.",
        params: &[raster("input", "Input Flow Direction Raster", true)],
        run: sink,
    },
    unary_tool!(abs_tool, "abs", "Abs"),
    unary_tool!(negate_tool, "negate", "Negate"),
    unary_tool!(square_tool, "square", "Square"),
    unary_tool!(square_root_tool, "square_root", "Square Root"),
    unary_tool!(exp_tool, "exp", "Exp"),
    unary_tool!(exp2_tool, "exp2", "Exp2"),
    unary_tool!(exp10_tool, "exp10", "Exp10"),
    unary_tool!(ln_tool, "ln", "Ln"),
    unary_tool!(log2_tool, "log2", "Log2"),
    unary_tool!(log10_tool, "log10", "Log10"),
    unary_tool!(sin_tool, "sin", "Sin"),
    unary_tool!(cos_tool, "cos", "Cos"),
    unary_tool!(tan_tool, "tan", "Tan"),
    unary_tool!(asin_tool, "asin", "ASin"),
    unary_tool!(acos_tool, "acos", "ACos"),
    unary_tool!(atan_tool, "atan", "ATan"),
    unary_tool!(sinh_tool, "sinh", "SinH"),
    unary_tool!(cosh_tool, "cosh", "CosH"),
    unary_tool!(tanh_tool, "tanh", "TanH"),
    unary_tool!(asinh_tool, "asinh", "ASinH"),
    unary_tool!(acosh_tool, "acosh", "ACosH"),
    unary_tool!(atanh_tool, "atanh", "ATanH"),
    unary_tool!(round_down_tool, "round_down", "Round Down"),
    unary_tool!(round_up_tool, "round_up", "Round Up"),
    unary_tool!(boolean_not_tool, "boolean_not", "Boolean Not"),
    unary_tool!(bitwise_not_tool, "bitwise_not", "Bitwise Not"),
    binary_tool!(atan2_tool, "atan2", "ATan2"),
    binary_tool!(power_tool, "power", "Power"),
    binary_tool!(mod_tool, "mod", "Mod"),
    binary_tool!(boolean_and_tool, "boolean_and", "Boolean And"),
    binary_tool!(boolean_or_tool, "boolean_or", "Boolean Or"),
    binary_tool!(boolean_xor_tool, "boolean_xor", "Boolean XOr"),
    binary_tool!(equal_to_tool, "equal_to", "Equal To"),
    binary_tool!(greater_than_tool, "greater_than", "Greater Than"),
    binary_tool!(
        greater_than_equal_tool,
        "greater_than_equal",
        "Greater Than Equal"
    ),
    binary_tool!(less_than_tool, "less_than", "Less Than"),
    binary_tool!(less_than_equal_tool, "less_than_equal", "Less Than Equal"),
    binary_tool!(not_equal_tool, "not_equal", "Not Equal"),
    binary_tool!(bitwise_and_tool, "bitwise_and", "Bitwise And"),
    binary_tool!(bitwise_or_tool, "bitwise_or", "Bitwise Or"),
    binary_tool!(bitwise_xor_tool, "bitwise_xor", "Bitwise XOr"),
    binary_tool!(
        bitwise_left_shift_tool,
        "bitwise_left_shift",
        "Bitwise Left Shift"
    ),
    binary_tool!(
        bitwise_right_shift_tool,
        "bitwise_right_shift",
        "Bitwise Right Shift"
    ),
    Tool {
        id: "raster_to_float",
        name: "Raster to Float",
        summary: "Writes a raster as a binary float file (the output path is the .flt file) with its .hdr.",
        params: &[raster("input", "Input Raster", true)],
        run: raster_to_ascii,
    },
    Tool {
        id: "raster_to_ascii",
        name: "Raster to ASCII",
        summary: "Writes a raster as an Esri ASCII grid (the output path is the .asc file).",
        params: &[raster("input", "Input Raster", true)],
        run: raster_to_ascii,
    },
    Tool {
        id: "combinatorial_and",
        name: "Combinatorial And",
        summary: "Numbers the pairs of values that satisfy the operation (Spatial Analyst).",
        params: &[
            raster("in1", "Input Raster 1", true),
            raster("in2", "Input Raster 2", true),
        ],
        run: combinatorial_and_tool,
    },
    Tool {
        id: "combinatorial_or",
        name: "Combinatorial Or",
        summary: "Numbers the pairs of values that satisfy the operation (Spatial Analyst).",
        params: &[
            raster("in1", "Input Raster 1", true),
            raster("in2", "Input Raster 2", true),
        ],
        run: combinatorial_or_tool,
    },
    Tool {
        id: "combinatorial_xor",
        name: "Combinatorial XOr",
        summary: "Numbers the pairs of values that satisfy the operation (Spatial Analyst).",
        params: &[
            raster("in1", "Input Raster 1", true),
            raster("in2", "Input Raster 2", true),
        ],
        run: combinatorial_xor_tool,
    },
    Tool {
        id: "reclass_by_ascii_file",
        name: "Reclass by ASCII File",
        summary: "Reclassifies values from a remap text file.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "remap_file",
                label: "Input ASCII Remap File",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "missing_values",
                "Missing Values",
                &["DATA", "NODATA"],
                "DATA",
            ),
        ],
        run: reclass_by_ascii_file,
    },
    Tool {
        id: "copy_raster",
        name: "Copy Raster",
        summary: "Copies a raster to a new GeoTIFF, optionally changing the pixel type or NoData value.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "nodata_value",
                label: "NoData Value",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            Param {
                name: "pixel_type",
                label: "Pixel Type",
                kind: Kind::Text,
                required: false,
                default: "",
            },
        ],
        run: copy_raster,
    },
    Tool {
        id: "flip",
        name: "Flip",
        summary: "Reverses the rows of a raster.",
        params: &[raster("input", "Input Raster", true)],
        run: flip,
    },
    Tool {
        id: "mirror",
        name: "Mirror",
        summary: "Reverses the columns of a raster.",
        params: &[raster("input", "Input Raster", true)],
        run: mirror,
    },
    Tool {
        id: "shift",
        name: "Shift",
        summary: "Moves a raster by an offset.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "x_value",
                label: "X Value",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            Param {
                name: "y_value",
                label: "Y Value",
                kind: Kind::Number,
                required: true,
                default: "",
            },
        ],
        run: shift,
    },
    Tool {
        id: "clip_raster",
        name: "Clip",
        summary: "Cuts a raster to a rectangle.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "rectangle",
                label: "Rectangle",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "nodata_value",
                label: "NoData Value",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            option(
                "clipping_geometry",
                "Use Input Features for Clipping Geometry",
                &["NONE", "ClippingGeometry"],
                "NONE",
            ),
            option(
                "maintain_extent",
                "Maintain Clipping Extent",
                &["NO_MAINTAIN_EXTENT", "MAINTAIN_EXTENT"],
                "NO_MAINTAIN_EXTENT",
            ),
        ],
        run: clip_raster,
    },
    Tool {
        id: "extract_by_rectangle",
        name: "Extract by Rectangle",
        summary: "Keeps the cells whose centers are inside (or outside) a rectangle.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "rectangle",
                label: "Extent",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "extraction_area",
                "Extraction Area",
                EXTRACTION_AREA,
                "INSIDE",
            ),
        ],
        run: extract_by_rectangle,
    },
    Tool {
        id: "extract_by_circle",
        name: "Extract by Circle",
        summary: "Keeps the cells whose centers are inside (or outside) a circle.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "center_point",
                label: "Center Point",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "radius",
                label: "Radius",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            option(
                "extraction_area",
                "Extraction Area",
                EXTRACTION_AREA,
                "INSIDE",
            ),
        ],
        run: extract_by_circle,
    },
    Tool {
        id: "extract_by_attributes",
        name: "Extract by Attributes",
        summary: "Keeps the cells whose value matches a where clause.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "where_clause",
                label: "Where Clause",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: extract_by_attributes,
    },
    Tool {
        id: "slice",
        name: "Slice",
        summary: "Cuts the value range into zones of equal interval or equal area.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "number_zones",
                label: "Number of Output Zones",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            option(
                "slice_type",
                "Slice Type",
                &["EQUAL_INTERVAL", "EQUAL_AREA"],
                "EQUAL_INTERVAL",
            ),
            Param {
                name: "base_output_zone",
                label: "Base Output Zone",
                kind: Kind::Number,
                required: false,
                default: "1",
            },
        ],
        run: slice,
    },
    Tool {
        id: "lookup",
        name: "Lookup",
        summary: "Gives each cell a field value from the raster's attribute table.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "lookup_field",
                label: "Lookup Field",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: lookup,
    },
    Tool {
        id: "nibble",
        name: "Nibble",
        summary: "Fills the cells the mask has no value for from the nearest others.",
        params: &[
            raster("input", "Input Raster", true),
            raster("mask", "Input Raster Mask", true),
            option(
                "values",
                "Nibble Values",
                &["ALL_VALUES", "DATA_ONLY"],
                "ALL_VALUES",
            ),
            option(
                "nodata",
                "Nibble NoData",
                &["PRESERVE_NODATA", "PROCESS_NODATA"],
                "PRESERVE_NODATA",
            ),
        ],
        run: nibble,
    },
    Tool {
        id: "filter",
        name: "Filter",
        summary: "A 3 x 3 low-pass or high-pass filter.",
        params: &[
            raster("input", "Input Raster", true),
            option("filter_type", "Filter Type", &["LOW", "HIGH"], "LOW"),
            option(
                "ignore_nodata",
                "Ignore NoData",
                &["DATA", "NODATA"],
                "DATA",
            ),
        ],
        run: filter,
    },
];
