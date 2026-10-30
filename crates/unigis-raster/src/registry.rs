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
    /// The line features of the layer the parameter names: their parts and the numeric `field`'s
    /// value, whether that field is integral, and the layer's EPSG code.
    #[allow(clippy::type_complexity)]
    fn line_values(
        &self,
        name: &str,
        field: &str,
    ) -> Result<(Vec<(Vec<Vec<[f64; 2]>>, f64)>, bool, Option<u32>)> {
        let _ = (name, field);
        Err(Error::Unsupported(
            "this tool needs line features, which this context cannot read".into(),
        ))
    }
    /// The points of the feature layer with numeric field values and its EPSG code.
    fn point_samples(
        &self,
        name: &str,
        field: &str,
    ) -> Result<(Vec<crate::interpolate::Sample>, Option<u32>)> {
        let _ = (name, field);
        Err(Error::Unsupported(
            "this tool needs point features, which this context cannot read".into(),
        ))
    }
    /// The multipatch geometries of the feature layer and its EPSG code.
    fn multipatches(&self, name: &str) -> Result<(Vec<unigis_core::Geometry>, Option<u32>)> {
        let _ = name;
        Err(Error::Unsupported(
            "this tool needs multipatch features, which this context cannot read".into(),
        ))
    }
}

/// A tool's result: the raster, and whether ArcGIS writes statistics (`.aux.xml`) beside it.
pub struct Output {
    pub raster: Raster,
    pub statistics: bool,
    /// Physical TIFF marker when it differs from the logical statistics marker.
    pub nodata_tag: Option<f64>,
    /// ArcGIS writes a STATISTICS_COUNT of 0 (Basin, Sink).
    pub zero_count: bool,
    /// Write ArcGIS empty-band statistics for entirely NoData bands.
    pub empty_band_statistics: bool,
    /// Include GP-recomputed covariance statistics after stored-cell normalization.
    pub covariance_statistics: bool,
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
        empty_band_statistics: false,
        covariance_statistics: false,
        nodata_tag: None,
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
        empty_band_statistics: false,
        covariance_statistics: false,
        nodata_tag: None,
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

fn zonal_geometry_table_unsupported(_: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Zonal Geometry as Table writes a table; run it through `ug run`".into(),
    ))
}

fn zonal_geometry(ctx: &dyn Context) -> Result<Output> {
    let zones = ctx.raster("in_zone_data")?;
    if let Some(field) = ctx.get("zone_field")
        && !field.eq_ignore_ascii_case("VALUE")
    {
        return Err(Error::Unsupported(
            "Zonal Geometry currently supports only the VALUE zone field".into(),
        ));
    }
    if let Some(text) = ctx.get("cell_size") {
        let cell: f64 = text.parse().map_err(|_| {
            Error::Unsupported("cell_size must be the input raster's cell size".into())
        })?;
        if cell <= 0.0
            || (cell - zones.cell_width()).abs() > 1e-9 * cell.max(1.0)
            || (cell - zones.cell_height()).abs() > 1e-9 * cell.max(1.0)
        {
            return Err(Error::Unsupported(
                "Zonal Geometry currently requires the input raster's cell size".into(),
            ));
        }
    }
    let geometry = crate::zonal::ZonalGeometryType::parse(
        &ctx.get("geometry_type").unwrap_or_else(|| "AREA".into()),
    )
    .ok_or_else(|| Error::Unsupported("unknown Zonal Geometry type".into()))?;
    with_statistics(crate::zonal::zonal_geometry(&zones, geometry)?, true)
}

fn zonal_fill(ctx: &dyn Context) -> Result<Output> {
    let out = crate::zonal::zonal_fill(
        &ctx.raster("in_zone_raster")?,
        &ctx.raster("in_weight_raster")?,
    )
    .ok_or_else(|| Error::Unsupported("the zone and weight rasters must share a grid".into()))?;
    with_statistics(out, true)
}

fn rescale_by_function(ctx: &dyn Context) -> Result<Output> {
    let text = ctx
        .get("transformation_function")
        .unwrap_or_else(|| "MSSMALL # # # # # #".into());
    let t = crate::rescale::Transformation::parse(&text).ok_or_else(|| {
        Error::Unsupported(format!("ERROR 000800: transformation function {text}"))
    })?;
    let scale = |name: &str, default: f64| -> Result<f64> {
        match ctx.get(name) {
            Some(v) => v
                .trim()
                .parse()
                .map_err(|_| Error::Unsupported(format!("ERROR 000094: {name} {v}"))),
            None => Ok(default),
        }
    };
    let (from, to) = (scale("from_scale", 1.0)?, scale("to_scale", 10.0)?);
    with_statistics(
        crate::rescale::rescale_by_function(&ctx.raster("input")?, &t, from, to),
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
            let input = ctx.raster("input")?;
            let op = crate::local::UnaryOp::$op;
            if input.bands.len() > 1 && !crate::local::supports_multiband_unary(op, &input) {
                return Err(Error::Unsupported(format!(
                    "{op:?} is not yet supported for this multiband input"
                )));
            }
            let mut output = with_statistics(crate::local::unary(op, &input), true)?;
            // GP postprocessing recomputes statistics from stored pixels when
            // an empty band has an unrepresentable integer NoData marker.
            let (lo, hi) = crate::local::type_range_of(output.raster.data_type);
            let outside = output.raster.nodata.is_some_and(|nd| nd < lo || nd > hi);
            let empty_band = output
                .raster
                .bands
                .iter()
                .any(|band| band.iter().all(|&value| output.raster.is_nodata(value)));
            if matches!(
                op,
                crate::local::UnaryOp::Abs
                    | crate::local::UnaryOp::Negate
                    | crate::local::UnaryOp::Square
            ) && !input.data_type.is_float()
                && !output.raster.data_type.is_float()
                && input.statistics.is_some()
                && outside
                && empty_band
            {
                let nodata = output.raster.nodata;
                for value in output.raster.bands.iter_mut().flatten() {
                    if Some(*value) == nodata {
                        *value = 0.0;
                    }
                }
                output.covariance_statistics = true;
            }
            Ok(output)
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

fn raster_to_other_format(_ctx: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Raster To Other Format requires a writable workspace host".into(),
    ))
}

fn raster_to_polyline(_ctx: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Raster To Polyline requires a feature output host".into(),
    ))
}

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
    let raster = crate::extract::copy_raster(&input, pixel_type, nodata);
    // Cached tagged sources reuse embedded statistics without a PAM sidecar
    // when the effective type and marker are unchanged. Actual missing cells,
    // raster size and band count do not select this path.
    // The default signed32 system marker retains omission for cached U32 input
    // tagged 2^31. Adjacent/high markers recompute, verified by the U32 controls.
    let default_u32_system_marker =
        input.data_type == DataType::U32 && input.nodata == Some(2147483648.0) && nodata.is_none();
    let statistics = input.statistics.is_none()
        || input.nodata.is_none()
        || raster.data_type != input.data_type
        || (raster.nodata != input.nodata && !default_u32_system_marker);
    let mut output = with_statistics(raster, statistics)?;
    if matches!(
        output.raster.data_type,
        DataType::U8 | DataType::U16 | DataType::U32
    ) {
        // Negative fractional unsigned markers remain raw in TIFF, while the
        // logical rounded marker controls ArcGIS statistics. Old payloads stay.
        output.nodata_tag = nodata
            .or(input.nodata)
            .filter(|v| v.is_finite() && *v < 0.0 && v.fract() != 0.0);
    }
    Ok(output)
}

fn mosaic_to_new_raster(ctx: &dyn Context) -> Result<Output> {
    use crate::DataType;
    use crate::extract::MosaicMethod;
    let mut rasters = Vec::new();
    for path in ctx.get("inputs").unwrap_or_default().split(';') {
        if !path.trim().is_empty() {
            rasters.push(ctx.raster_at(path.trim())?);
        }
    }
    let pixel_type = match ctx
        .get("pixel_type")
        .unwrap_or_else(|| "8_BIT_UNSIGNED".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "8_BIT_UNSIGNED" => DataType::U8,
        "8_BIT_SIGNED" => DataType::I8,
        "16_BIT_UNSIGNED" => DataType::U16,
        "16_BIT_SIGNED" => DataType::I16,
        "32_BIT_UNSIGNED" => DataType::U32,
        "32_BIT_SIGNED" => DataType::I32,
        "32_BIT_FLOAT" => DataType::F32,
        "64_BIT" => DataType::F64,
        other => {
            return Err(Error::Unsupported(format!(
                "ERROR 000067: Pixel Type: {other} is not supported yet"
            )));
        }
    };
    let method = match ctx
        .get("mosaic_method")
        .unwrap_or_else(|| "LAST".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "FIRST" => MosaicMethod::First,
        "LAST" => MosaicMethod::Last,
        "MEAN" => MosaicMethod::Mean,
        "MINIMUM" => MosaicMethod::Minimum,
        "MAXIMUM" => MosaicMethod::Maximum,
        other => {
            return Err(Error::Unsupported(format!(
                "ERROR 000067: Mosaic Method: {other} is not supported yet"
            )));
        }
    };
    let cell = ctx.get("cellsize").and_then(|v| v.trim().parse().ok());
    let raster = crate::extract::mosaic(&rasters, pixel_type, cell, method).ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Input Rasters: value is required".into())
    })?;
    with_statistics(raster, true)
}

/// "x y;x y;..." as coordinate pairs.
fn coordinate_list(text: &str) -> Result<Vec<(f64, f64)>> {
    text.split(';')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let v = parse_numbers(p, 2, "Polygon")?;
            Ok((v[0], v[1]))
        })
        .collect()
}

fn extract_by_polygon(ctx: &dyn Context) -> Result<Output> {
    let ring = coordinate_list(&ctx.get("polygon").unwrap_or_default())?;
    with_statistics(
        crate::extract::extract_by_polygon(
            &ctx.raster("input")?,
            &ring,
            inside(ctx, "extract_by_polygon"),
        ),
        true,
    )
}

fn extract_by_points(ctx: &dyn Context) -> Result<Output> {
    let points = coordinate_list(&ctx.get("points").unwrap_or_default())?;
    with_statistics(
        crate::extract::extract_by_points(
            &ctx.raster("input")?,
            &points,
            inside(ctx, "extract_by_points"),
        ),
        true,
    )
}

fn topographic_position_index(ctx: &dyn Context) -> Result<Output> {
    let neighborhood = match ctx.get("neighborhood") {
        None => None,
        Some(text) => Some(crate::focal::Neighborhood::parse(&text).ok_or_else(|| {
            Error::Unsupported(format!(
                "ERROR 000094: Neighborhood: '{text}' is not understood"
            ))
        })?),
    };
    with_statistics(
        crate::focal::topographic_position_index(&ctx.raster("input")?, neighborhood),
        true,
    )
}

fn top_hat_transform(ctx: &dyn Context) -> Result<Output> {
    let neighborhood = match ctx.get("neighborhood") {
        None => None,
        Some(text) => Some(crate::focal::Neighborhood::parse(&text).ok_or_else(|| {
            Error::Unsupported(format!(
                "ERROR 000094: Neighborhood: '{text}' is not understood"
            ))
        })?),
    };
    if matches!(
        neighborhood,
        Some(crate::focal::Neighborhood::Annulus { .. })
    ) {
        return Err(Error::Unsupported(
            "ERROR 000864: Neighborhood: The input is not within the defined domain.".into(),
        ));
    }
    let valley = choice(ctx, "top_hat_transform", "analysis_type").eq_ignore_ascii_case("VALLEY");
    with_statistics(
        crate::focal::top_hat_transform(&ctx.raster("input")?, neighborhood, valley),
        true,
    )
}

fn test_tool(ctx: &dyn Context) -> Result<Output> {
    let clause = crate::local::ValueClause::parse(&ctx.get("where_clause").unwrap_or_default())
        .ok_or_else(|| Error::Unsupported("ERROR 000358: Invalid expression".into()))?;
    with_statistics(crate::extract::test(&ctx.raster("input")?, &clause), true)
}

fn trend(ctx: &dyn Context) -> Result<Output> {
    let field = ctx.get("z_field").unwrap_or_default();
    let (samples, epsg) = ctx.point_samples("input", &field)?;
    let order: usize = ctx
        .get("order")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1);
    if !(1..=12).contains(&order) {
        return Err(Error::Unsupported(
            "ERROR 000094: Polynomial order must be between 1 and 12".into(),
        ));
    }
    let env = crate::env::Environment {
        cell_size: ctx.get("cell_size").and_then(|v| v.trim().parse().ok()),
        ..Default::default()
    };
    let raster = crate::interpolate::trend(&samples, order, &env, epsg).ok_or_else(|| {
        Error::Unsupported("ERROR 010003: too few points for this polynomial order".into())
    })?;
    with_statistics(raster, true)
}

fn rank_or_popularity_tool(ctx: &dyn Context, popularity: bool) -> Result<Output> {
    let k: usize = ctx
        .get("rank")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000094: the rank must be a whole number".into())
        })?;
    let mut rasters = Vec::new();
    for path in ctx.get("rasters").unwrap_or_default().split(';') {
        if !path.trim().is_empty() {
            rasters.push(ctx.raster_at(path.trim())?);
        }
    }
    if rasters.is_empty() {
        return Err(Error::Unsupported(
            "ERROR 000735: Input Rasters: value is required".into(),
        ));
    }
    for r in &rasters[1..] {
        if (r.width, r.height) != (rasters[0].width, rasters[0].height) {
            return Err(Error::Unsupported("the rasters must share a grid".into()));
        }
    }
    let raster = crate::local::rank_or_popularity(k, &rasters, popularity)
        .ok_or_else(|| Error::Unsupported("ERROR 000094: the rank is out of range".into()))?;
    with_statistics(raster, true)
}

fn rank_tool(ctx: &dyn Context) -> Result<Output> {
    rank_or_popularity_tool(ctx, false)
}

fn popularity_tool(ctx: &dyn Context) -> Result<Output> {
    rank_or_popularity_tool(ctx, true)
}

fn frequency_tool(ctx: &dyn Context, comparison: crate::frequency::Comparison) -> Result<Output> {
    let input = ctx.raster("input")?;
    let paths = ctx.get("rasters").unwrap_or_default();
    let paths: Vec<_> = paths
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if paths.len() > 256 {
        return Err(Error::Unsupported(
            "Frequency supports at most 256 comparison rasters".into(),
        ));
    }
    let rasters = paths
        .iter()
        .map(|path| ctx.raster_at(path))
        .collect::<Result<Vec<_>>>()?;
    let multiband = ctx
        .get("process_as_multiband")
        .is_some_and(|v| v.eq_ignore_ascii_case("MULTI_BAND"));
    let mut output = with_statistics(
        crate::frequency::frequency(&input, &rasters, comparison, multiband)?,
        true,
    )?;
    output.empty_band_statistics = true;
    Ok(output)
}

fn equal_to_frequency(ctx: &dyn Context) -> Result<Output> {
    frequency_tool(ctx, crate::frequency::Comparison::Equal)
}
fn greater_than_frequency(ctx: &dyn Context) -> Result<Output> {
    frequency_tool(ctx, crate::frequency::Comparison::Greater)
}
fn less_than_frequency(ctx: &dyn Context) -> Result<Output> {
    frequency_tool(ctx, crate::frequency::Comparison::Less)
}

fn in_list_tool(ctx: &dyn Context) -> Result<Output> {
    let input = ctx.raster("input")?;
    let mut list = Vec::new();
    for item in ctx.get("list").unwrap_or_default().split(';') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        list.push(match crate::local::Operand::constant(item) {
            Some(c) => c,
            None => {
                let r = ctx.raster_at(item)?;
                if (r.width, r.height) != (input.width, input.height) {
                    return Err(Error::Unsupported("the rasters must share a grid".into()));
                }
                crate::local::Operand::Raster(r)
            }
        });
    }
    with_statistics(crate::local::in_list(&input, &list), true)
}

fn create_constant_raster(ctx: &dyn Context) -> Result<Output> {
    let value: f64 = ctx
        .get("constant")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Constant value: value is required".into())
        })?;
    let integer = !choice(ctx, "create_constant_raster", "data_type").eq_ignore_ascii_case("FLOAT");
    let cell: f64 = ctx
        .get("cell_size")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| Error::Unsupported("ERROR 000735: Cell size: value is required".into()))?;
    let e = parse_numbers(&ctx.get("extent").unwrap_or_default(), 4, "Extent")?;
    let raster = crate::extract::constant_raster(value, integer, cell, (e[0], e[1], e[2], e[3]))
        .ok_or_else(|| Error::Unsupported("ERROR 000094: bad cell size or extent".into()))?;
    with_statistics(raster, true)
}

/// Create Normal Raster: without settings, cell size 1 and 250 x 250 cells from 0, 0 (ArcGIS's
/// defaults without environment settings).
fn create_normal_raster(ctx: &dyn Context) -> Result<Output> {
    let cell: f64 = ctx
        .get("cell_size")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1.0);
    let e = match ctx.get("extent") {
        Some(text) => parse_numbers(&text, 4, "Extent")?,
        None => vec![0.0, 0.0, 250.0 * cell, 250.0 * cell],
    };
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let raster = crate::extract::normal_raster(cell, (e[0], e[1], e[2], e[3]), seed)
        .ok_or_else(|| Error::Unsupported("ERROR 000094: bad cell size or extent".into()))?;
    with_statistics(raster, true)
}

fn euclidean_back_direction(ctx: &dyn Context) -> Result<Output> {
    let max = ctx
        .get("maximum_distance")
        .and_then(|v| v.trim().parse().ok());
    with_statistics(
        crate::resample::euclidean_back_direction(&ctx.raster("input")?, max),
        false,
    )
}

/// Distance Accumulation / Allocation without barriers, surface or cost rasters: the Euclidean
/// distance / allocation (identical output, observed).
fn distance_accumulation(ctx: &dyn Context) -> Result<Output> {
    with_statistics(
        crate::resample::euclidean_distance(&ctx.raster("input")?, None),
        false,
    )
}

/// Allocation values are stored as bytes (16 bit beyond 255): NoData 0 when no cell holds 0,
/// none otherwise (observed), no statistics.
fn distance_allocation(ctx: &dyn Context) -> Result<Output> {
    use crate::DataType;
    let alloc = crate::resample::euclidean_allocation(&ctx.raster("input")?, None);
    let valid = |v: f64| !alloc.is_nodata(v);
    let max = alloc.bands[0]
        .iter()
        .copied()
        .filter(|&v| valid(v))
        .fold(0.0_f64, f64::max);
    let uses_zero = alloc.bands[0].iter().any(|&v| valid(v) && v == 0.0);
    let t = if max <= 255.0 {
        DataType::U8
    } else {
        DataType::U16
    };
    let nodata = if uses_zero { None } else { Some(0.0) };
    let mut out = alloc.like(t);
    out.nodata = nodata;
    out.bands[0] = alloc.bands[0]
        .iter()
        .map(|&v| if valid(v) { v } else { nodata.unwrap_or(0.0) })
        .collect();
    with_statistics(out, false)
}

fn focal_flow(ctx: &dyn Context) -> Result<Output> {
    let threshold = ctx
        .get("threshold_value")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0);
    with_statistics(
        crate::hydro::focal_flow(&ctx.raster("input")?, threshold),
        true,
    )
}

fn snap_pour_point(ctx: &dyn Context) -> Result<Output> {
    let field = ctx.get("pour_point_field").unwrap_or_else(|| "FID".into());
    let (samples, _) = ctx.point_samples("input", &field)?;
    let snap: f64 = ctx
        .get("snap_distance")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Snap distance: value is required".into())
        })?;
    let points: Vec<(f64, f64, f64)> = samples.iter().map(|s| (s.x, s.y, s.z)).collect();
    with_statistics(
        crate::hydro::snap_pour_point(&points, &ctx.raster("accumulation")?, snap),
        true,
    )
}

/// Sample writes a table, which the command line produces (see `run_sample` in the CLI).
fn sample_unsupported(_: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Sample writes a table; run it through `ug run sample`".into(),
    ))
}

/// Extract Multi Values To Points writes the points with new fields; the command line builds it
/// (`run_extract_multi_values` in the CLI).
fn add_surface_unsupported(_: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Add Surface Information edits features; run it through `ug run`".into(),
    ))
}

fn extract_multi_unsupported(_: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Extract Multi Values To Points writes features; run it through `ug run`".into(),
    ))
}

/// Interpolate Shape writes features, which the command line produces (`run_interpolate_shape`).
fn interpolate_shape_unsupported(_: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Interpolate Shape writes features; run it through `ug run`".into(),
    ))
}

fn zonal_characterization_unsupported(_: &dyn Context) -> Result<Output> {
    Err(Error::Unsupported(
        "Zonal Characterization writes a table; run it through `ug run`".into(),
    ))
}

fn point_statistics(ctx: &dyn Context) -> Result<Output> {
    use crate::interpolate::{PointNeighborhood, PointStatistic};
    let field = ctx.get("field").unwrap_or_default();
    let (samples, epsg) = ctx.point_samples("input", &field)?;
    let cell: f64 = ctx
        .get("cell_size")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Output cell size: value is required".into())
        })?;
    let text = ctx.get("neighborhood").unwrap_or_default();
    let parts: Vec<&str> = text.split_whitespace().collect();
    let unit = parts.last().copied().unwrap_or("").to_ascii_uppercase();
    let scale = match unit.as_str() {
        "MAP" => 1.0,
        "CELL" => cell,
        _ => {
            return Err(Error::Unsupported(format!(
                "ERROR 000094: Neighborhood: '{text}' is not understood"
            )));
        }
    };
    let num = |i: usize| parts.get(i).and_then(|v| v.parse::<f64>().ok());
    let neighborhood = match parts.first().map(|p| p.to_ascii_uppercase()).as_deref() {
        Some("CIRCLE") => PointNeighborhood::Circle(num(1).unwrap_or(0.0) * scale),
        Some("RECTANGLE") => PointNeighborhood::Rectangle(
            num(1).unwrap_or(0.0) * scale,
            num(2).unwrap_or(0.0) * scale,
        ),
        Some("ANNULUS") => {
            PointNeighborhood::Annulus(num(1).unwrap_or(0.0) * scale, num(2).unwrap_or(0.0) * scale)
        }
        _ => {
            return Err(Error::Unsupported(format!(
                "ERROR 000094: Neighborhood: '{text}' is not understood"
            )));
        }
    };
    let stat = match ctx
        .get("statistic")
        .unwrap_or_else(|| "MEAN".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "MEAN" => PointStatistic::Mean,
        "MAXIMUM" => PointStatistic::Maximum,
        "MINIMUM" => PointStatistic::Minimum,
        "RANGE" => PointStatistic::Range,
        "STD" => PointStatistic::Std,
        "SUM" => PointStatistic::Sum,
        other => {
            return Err(Error::Unsupported(format!(
                "ERROR 000067: Statistic: {other} is not supported"
            )));
        }
    };
    let raster = crate::interpolate::point_statistics(&samples, cell, neighborhood, stat, epsg)
        .ok_or_else(|| Error::Unsupported("ERROR 000735: no points".into()))?;
    with_statistics(raster, true)
}

fn line_statistics(ctx: &dyn Context) -> Result<Output> {
    let field = ctx.get("field").unwrap_or_default();
    let (lines, integer, epsg) = ctx.line_values("input", &field)?;
    let cell: f64 = ctx
        .get("cell_size")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Output cell size: value is required".into())
        })?;
    let radius: f64 = ctx
        .get("search_radius")
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000735: Search radius: value is required".into())
        })?;
    let stat = ctx.get("statistic").unwrap_or_else(|| "MEAN".into());
    let raster = crate::interpolate::line_statistics(&lines, integer, cell, radius, &stat, epsg)
        .ok_or_else(|| {
            Error::Unsupported("ERROR 000094: bad statistic, cell size or lines".into())
        })?;
    with_statistics(raster, true)
}

fn diff_tool(ctx: &dyn Context) -> Result<Output> {
    let r = crate::local::diff_or_over(false, &operand(ctx, "in1")?, &operand(ctx, "in2")?)
        .ok_or_else(|| Error::Unsupported("at least one input must be a raster".into()))?;
    with_statistics(r, true)
}

fn over_tool(ctx: &dyn Context) -> Result<Output> {
    let r = crate::local::diff_or_over(true, &operand(ctx, "in1")?, &operand(ctx, "in2")?)
        .ok_or_else(|| Error::Unsupported("at least one input must be a raster".into()))?;
    with_statistics(r, true)
}

fn compute_change_raster(ctx: &dyn Context) -> Result<Output> {
    let relative = match ctx
        .get("change_type")
        .unwrap_or_else(|| "DIFFERENCE".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "DIFFERENCE" => false,
        "RELATIVE_DIFFERENCE" => true,
        other => {
            return Err(Error::Unsupported(format!(
                "ERROR 000067: Change Type: {other} is not supported yet"
            )));
        }
    };
    let r = crate::local::compute_change(
        &ctx.raster("from_raster")?,
        &ctx.raster("to_raster")?,
        relative,
    )
    .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
    with_statistics(r, false)
}

fn create_binary_mask(ctx: &dyn Context) -> Result<Output> {
    let background = ctx
        .get("background_value")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0.0);
    with_statistics(
        crate::local::binary_mask(&ctx.raster("input")?, background),
        true,
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

fn composite_bands(ctx: &dyn Context) -> Result<Output> {
    let inputs = ctx
        .get("inputs")
        .unwrap_or_default()
        .split(';')
        .filter(|p| !p.trim().is_empty())
        .map(|p| ctx.raster_at(p.trim()))
        .collect::<Result<Vec<_>>>()?;
    with_statistics(crate::composite::composite_bands(&inputs)?, true)
}

fn lowest_position(ctx: &dyn Context) -> Result<Output> {
    let inputs = ctx
        .get("inputs")
        .unwrap_or_default()
        .split(';')
        .filter(|p| !p.trim().is_empty())
        .map(|p| ctx.raster_at(p.trim()))
        .collect::<Result<Vec<_>>>()?;
    let rasters: Vec<&Raster> = inputs.iter().collect();
    let raster = crate::overlay::position(&rasters, false)
        .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
    with_statistics(raster, true)
}

fn ascii_to_raster(ctx: &dyn Context) -> Result<Output> {
    let input = ctx.get("input").unwrap_or_default();
    let integer = choice(ctx, "ascii_to_raster", "data_type").eq_ignore_ascii_case("INTEGER");
    with_statistics(
        crate::grid_import::ascii_to_raster(std::path::Path::new(&input), integer)?,
        true,
    )
}

fn float_to_raster(ctx: &dyn Context) -> Result<Output> {
    let input = ctx.get("input").unwrap_or_default();
    with_statistics(
        crate::grid_import::float_to_raster(std::path::Path::new(&input))?,
        true,
    )
}

fn multipatch_to_raster(ctx: &dyn Context) -> Result<Output> {
    let (features, epsg) = ctx.multipatches("input")?;
    let cell = ctx.get("cell_size").and_then(|v| v.trim().parse().ok());
    let maximum = choice(ctx, "multipatch_to_raster", "cell_assignment_method")
        .eq_ignore_ascii_case("MAXIMUM_HEIGHT");
    let raster = crate::rasterize::multipatches_to_raster(&features, cell, maximum, epsg)
        .ok_or_else(|| Error::Unsupported("no multipatch surface or invalid cell size".into()))?;
    with_statistics(raster, false)
}

pub static TOOLS: &[Tool] = &[
    Tool {
        id: "ascii_to_raster",
        name: "ASCII To Raster",
        summary: "Imports an Esri ASCII grid as a signed integer or floating-point raster.",
        params: &[
            Param {
                name: "input",
                label: "Input ASCII Raster File",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "data_type",
                "Output Data Type",
                &["INTEGER", "FLOAT"],
                "INTEGER",
            ),
        ],
        run: ascii_to_raster,
    },
    Tool {
        id: "float_to_raster",
        name: "Float To Raster",
        summary: "Imports an IEEE float32 .flt file with its .hdr header (little or big endian).",
        params: &[Param {
            name: "input",
            label: "Input Floating Point Raster File",
            kind: Kind::Text,
            required: true,
            default: "",
        }],
        run: float_to_raster,
    },
    Tool {
        id: "multipatch_to_raster",
        name: "Multipatch To Raster",
        summary: "Converts multipatch surfaces to a floating-point elevation raster.",
        params: &[
            Param {
                name: "input",
                label: "Input Multipatch Features",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "cell_size",
                label: "Output Cell Size",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            option(
                "cell_assignment_method",
                "Cell Assignment Method",
                &["MAXIMUM_HEIGHT", "MINIMUM_HEIGHT"],
                "MAXIMUM_HEIGHT",
            ),
        ],
        run: multipatch_to_raster,
    },
    Tool {
        id: "composite_bands",
        name: "Composite Bands",
        summary: "Combines all input bands in order on the first raster's grid (nearest neighbour).",
        params: &[Param {
            name: "inputs",
            label: "Input Rasters",
            kind: Kind::Text,
            required: true,
            default: "",
        }],
        run: composite_bands,
    },
    Tool {
        id: "lowest_position",
        name: "Lowest Position",
        summary: "Returns the 1-based position of the first raster with the lowest cell value.",
        params: &[Param {
            name: "inputs",
            label: "Input Rasters or Constant Values",
            kind: Kind::Text,
            required: true,
            default: "",
        }],
        run: lowest_position,
    },
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
        id: "raster_to_other_format",
        name: "Raster To Other Format",
        summary: "Converts one or more rasters to uniquely named GeoTIFFs in an existing folder.",
        params: &[
            Param {
                name: "input_rasters",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "output_workspace",
                label: "Output Workspace",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option("raster_format", "Raster Format", &["TIFF"], "TIFF"),
        ],
        run: raster_to_other_format,
    },
    Tool {
        id: "raster_to_polyline",
        name: "Raster to Polyline",
        summary: "Converts an integer raster to a node/arc polyline feature class.",
        params: &[
            raster("input", "Input raster", true),
            option(
                "background_value",
                "Background value",
                &["ZERO", "NODATA"],
                "ZERO",
            ),
            Param {
                name: "minimum_dangle_length",
                label: "Minimum dangle length",
                kind: Kind::Text,
                required: false,
                default: "0",
            },
            option(
                "simplify",
                "Simplify polylines",
                &["SIMPLIFY", "NO_SIMPLIFY"],
                "SIMPLIFY",
            ),
            option("raster_field", "Field", &["Value"], "Value"),
        ],
        run: raster_to_polyline,
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
        id: "mosaic_to_new_raster",
        name: "Mosaic To New Raster",
        summary: "Mosaics rasters into a new GeoTIFF (first-raster grid, nearest values).",
        params: &[
            Param {
                name: "inputs",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "pixel_type",
                label: "Pixel Type",
                kind: Kind::Text,
                required: false,
                default: "8_BIT_UNSIGNED",
            },
            Param {
                name: "cellsize",
                label: "Cellsize",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            option(
                "mosaic_method",
                "Mosaic Operator",
                &["FIRST", "LAST", "MEAN", "MINIMUM", "MAXIMUM"],
                "LAST",
            ),
        ],
        run: mosaic_to_new_raster,
    },
    Tool {
        id: "topographic_position_index",
        name: "Topographic Position Index",
        summary: "A cell's value minus the mean of its neighborhood.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "neighborhood",
                label: "Neighborhood",
                kind: Kind::Text,
                required: false,
                default: "",
            },
        ],
        run: topographic_position_index,
    },
    Tool {
        id: "top_hat_transform",
        name: "Top Hat Transform",
        summary: "Ridge or valley structures of a surface (morphological top hat).",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "neighborhood",
                label: "Neighborhood",
                kind: Kind::Text,
                required: false,
                default: "",
            },
            option(
                "analysis_type",
                "Analysis Type",
                &["RIDGE", "VALLEY"],
                "RIDGE",
            ),
        ],
        run: top_hat_transform,
    },
    Tool {
        id: "test",
        name: "Test",
        summary: "1 where the cell value matches an expression, else 0.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "where_clause",
                label: "Expression",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: test_tool,
    },
    Tool {
        id: "trend",
        name: "Trend",
        summary: "A least-squares polynomial surface through points (LINEAR).",
        params: &[
            Param {
                name: "input",
                label: "Input Point Features",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "z_field",
                label: "Z Value Field",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "cell_size",
                label: "Output Cell Size",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            Param {
                name: "order",
                label: "Polynomial Order",
                kind: Kind::Number,
                required: false,
                default: "1",
            },
        ],
        run: trend,
    },
    Tool {
        id: "rank",
        name: "Rank",
        summary: "The k-th value of the rasters in each cell.",
        params: &[
            Param {
                name: "rank",
                label: "Rank",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            Param {
                name: "rasters",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: rank_tool,
    },
    Tool {
        id: "popularity",
        name: "Popularity",
        summary: "The k-th value of the rasters in each cell.",
        params: &[
            Param {
                name: "rank",
                label: "Rank",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            Param {
                name: "rasters",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: popularity_tool,
    },
    Tool {
        id: "equal_to_frequency",
        name: "Equal To Frequency",
        summary: "Counts matching comparison values on the same raster grid; supports SINGLE_BAND and MULTI_BAND.",
        params: &[
            raster("input", "Input Value Raster", true),
            Param {
                name: "rasters",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "process_as_multiband",
                "Process as Multiband",
                &["SINGLE_BAND", "MULTI_BAND"],
                "SINGLE_BAND",
            ),
        ],
        run: equal_to_frequency,
    },
    Tool {
        id: "greater_than_frequency",
        name: "Greater Than Frequency",
        summary: "Counts matching comparison values on the same raster grid; supports SINGLE_BAND and MULTI_BAND.",
        params: &[
            raster("input", "Input Value Raster", true),
            Param {
                name: "rasters",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "process_as_multiband",
                "Process as Multiband",
                &["SINGLE_BAND", "MULTI_BAND"],
                "SINGLE_BAND",
            ),
        ],
        run: greater_than_frequency,
    },
    Tool {
        id: "less_than_frequency",
        name: "Less Than Frequency",
        summary: "Counts matching comparison values on the same raster grid; supports SINGLE_BAND and MULTI_BAND.",
        params: &[
            raster("input", "Input Value Raster", true),
            Param {
                name: "rasters",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "process_as_multiband",
                "Process as Multiband",
                &["SINGLE_BAND", "MULTI_BAND"],
                "SINGLE_BAND",
            ),
        ],
        run: less_than_frequency,
    },
    Tool {
        id: "in_list",
        name: "In List",
        summary: "Keeps the cell values that occur in a list of constants and rasters.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "list",
                label: "Input Rasters Or Constants",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: in_list_tool,
    },
    Tool {
        id: "create_constant_raster",
        name: "Create Constant Raster",
        summary: "A raster whose cells all hold one value.",
        params: &[
            Param {
                name: "constant",
                label: "Constant Value",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            option("data_type", "Data Type", &["INTEGER", "FLOAT"], "INTEGER"),
            Param {
                name: "cell_size",
                label: "Cell Size",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            Param {
                name: "extent",
                label: "Extent",
                kind: Kind::Text,
                required: true,
                default: "",
            },
        ],
        run: create_constant_raster,
    },
    Tool {
        id: "create_normal_raster",
        name: "Create Normal Raster",
        summary: "A raster of random values from the standard normal distribution (mean 0, standard deviation 1).",
        params: &[
            Param {
                name: "cell_size",
                label: "Output cell size",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            Param {
                name: "extent",
                label: "Output extent",
                kind: Kind::Text,
                required: false,
                default: "",
            },
        ],
        run: create_normal_raster,
    },
    Tool {
        id: "euclidean_back_direction",
        name: "Euclidean Back Direction",
        summary: "Unrounded azimuth from each cell to its nearest source.",
        params: &[
            raster("input", "Input Source Data", true),
            Param {
                name: "maximum_distance",
                label: "Maximum Distance",
                kind: Kind::Number,
                required: false,
                default: "",
            },
        ],
        run: euclidean_back_direction,
    },
    Tool {
        id: "distance_accumulation",
        name: "Distance Accumulation",
        summary: "Euclidean distance / allocation from sources (no barriers, surface or cost rasters yet).",
        params: &[raster("input", "Input Source Data", true)],
        run: distance_accumulation,
    },
    Tool {
        id: "distance_allocation",
        name: "Distance Allocation",
        summary: "Euclidean distance / allocation from sources (no barriers, surface or cost rasters yet).",
        params: &[raster("input", "Input Source Data", true)],
        run: distance_allocation,
    },
    Tool {
        id: "focal_flow",
        name: "Focal Flow",
        summary: "Which neighbors of each cell are higher than it (flow directions into the cell).",
        params: &[
            raster("input", "Input Surface Raster", true),
            Param {
                name: "threshold_value",
                label: "Threshold Value",
                kind: Kind::Number,
                required: false,
                default: "",
            },
        ],
        run: focal_flow,
    },
    Tool {
        id: "snap_pour_point",
        name: "Snap Pour Point",
        summary: "Moves pour points to the highest flow accumulation within a distance.",
        params: &[
            Param {
                name: "input",
                label: "Input Pour Point Data",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            raster("accumulation", "Input Accumulation Raster", true),
            Param {
                name: "snap_distance",
                label: "Snap Distance",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            Param {
                name: "pour_point_field",
                label: "Pour Point Field",
                kind: Kind::Text,
                required: false,
                default: "",
            },
        ],
        run: snap_pour_point,
    },
    Tool {
        id: "sample",
        name: "Sample",
        summary: "Raster values at point locations, as a table.",
        params: &[
            Param {
                name: "input",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "locations",
                label: "Input Location Data",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "resampling",
                "Resampling Technique",
                &["NEAREST", "BILINEAR"],
                "NEAREST",
            ),
            Param {
                name: "unique_id_field",
                label: "Unique ID Field",
                kind: Kind::Text,
                required: false,
                default: "",
            },
        ],
        run: sample_unsupported,
    },
    Tool {
        id: "add_surface_information",
        name: "Add Surface Information",
        summary: "Adds surface elevation (points) or profile statistics (lines) from a raster surface to features.",
        params: &[
            Param {
                name: "input",
                label: "Input Features",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            raster("in_surface", "Input Surface", true),
            Param {
                name: "out_property",
                label: "Output Property",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option("method", "Method", &["LINEAR", "BILINEAR"], "LINEAR"),
            Param {
                name: "sample_distance",
                label: "Sampling Distance",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            Param {
                name: "z_factor",
                label: "Z Factor",
                kind: Kind::Number,
                required: false,
                default: "1",
            },
        ],
        run: add_surface_unsupported,
    },
    Tool {
        id: "extract_multi_values_to_points",
        name: "Extract Multi Values To Points",
        summary: "Adds a field per raster with the raster's value at each point.",
        params: &[
            Param {
                name: "input",
                label: "Input Point Features",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "rasters",
                label: "Input Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "interpolate",
                "Interpolate Values At The Point Locations",
                &["NONE", "BILINEAR"],
                "NONE",
            ),
        ],
        run: extract_multi_unsupported,
    },
    Tool {
        id: "interpolate_shape",
        name: "Interpolate Shape",
        summary: "Creates 3D features by interpolating z-values from a raster surface.",
        params: &[
            Param {
                name: "input",
                label: "Input Surface",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "features",
                label: "Input Feature Class",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "sample_distance",
                label: "Sample Distance",
                kind: Kind::Number,
                required: false,
                default: "",
            },
            Param {
                name: "z_factor",
                label: "Z Factor",
                kind: Kind::Number,
                required: false,
                default: "1",
            },
            option(
                "method",
                "Interpolation Method",
                &["BILINEAR", "NEAREST"],
                "BILINEAR",
            ),
            option(
                "vertices_only",
                "Interpolate Only At Vertices",
                &["DENSIFY", "VERTICES_ONLY"],
                "DENSIFY",
            ),
            Param {
                name: "pyramid_level_resolution",
                label: "Pyramid Level Resolution",
                kind: Kind::Number,
                required: false,
                default: "0",
            },
            option(
                "preserve_features",
                "Preserve Features Outside The Surface",
                &["EXCLUDE", "PRESERVE"],
                "EXCLUDE",
            ),
            option(
                "interpolate_m_value",
                "Interpolate M Values For Densified Vertices",
                &["NO_INTERPOLATE_M_VALUE", "INTERPOLATE_M_VALUE"],
                "NO_INTERPOLATE_M_VALUE",
            ),
        ],
        run: interpolate_shape_unsupported,
    },
    Tool {
        id: "zonal_characterization",
        name: "Zonal Characterization",
        summary: "Summarizes multiple value rasters within each zone as a table.",
        params: &[
            Param {
                name: "in_zone_data",
                label: "Input Raster or Feature Zones",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "in_value_rasters_statistics",
                label: "Input Value Rasters",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "out_statistics_features",
                label: "Output Statistics Feature Class",
                kind: Kind::Text,
                required: false,
                default: "",
            },
            Param {
                name: "zone_field",
                label: "Zone Field",
                kind: Kind::Text,
                required: false,
                default: "Value",
            },
            option(
                "ignore_nodata",
                "Ignore NoData in Calculations",
                &["DATA", "NODATA"],
                "DATA",
            ),
            Param {
                name: "percentile_values",
                label: "Percentile Values",
                kind: Kind::Text,
                required: false,
                default: "90",
            },
            option(
                "percentile_interpolation_type",
                "Percentile Interpolation Type",
                &["AUTO_DETECT", "NEAREST", "LINEAR"],
                "AUTO_DETECT",
            ),
            option(
                "circular_calculation",
                "Calculate Circular Statistics",
                &["ARITHMETIC", "CIRCULAR"],
                "ARITHMETIC",
            ),
            Param {
                name: "circular_wrap_value",
                label: "Circular Wrap Value",
                kind: Kind::Number,
                required: false,
                default: "360",
            },
            option(
                "process_as_multidimensional",
                "Process as Multidimensional",
                &["CURRENT_SLICE", "ALL_SLICES"],
                "CURRENT_SLICE",
            ),
            option(
                "add_zone_attributes",
                "Append Zone Attributes to Output Features",
                &["ZONE_FIELD_ONLY", "ALL"],
                "ZONE_FIELD_ONLY",
            ),
        ],
        run: zonal_characterization_unsupported,
    },
    Tool {
        id: "point_statistics",
        name: "Point Statistics",
        summary: "Statistics of point values within a neighborhood of each cell.",
        params: &[
            Param {
                name: "input",
                label: "Input Point Features",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "field",
                label: "Field",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "cell_size",
                label: "Output Cell Size",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            Param {
                name: "neighborhood",
                label: "Neighborhood",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "statistic",
                "Statistics Type",
                &["MEAN", "MAXIMUM", "MINIMUM", "RANGE", "STD", "SUM"],
                "MEAN",
            ),
        ],
        run: point_statistics,
    },
    Tool {
        id: "line_statistics",
        name: "Line Statistics",
        summary: "Length-weighted statistics of line field values around each cell.",
        params: &[
            Param {
                name: "input",
                label: "Input Polyline Features",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "field",
                label: "Field",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            Param {
                name: "cell_size",
                label: "Output Cell Size",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            Param {
                name: "search_radius",
                label: "Search Radius",
                kind: Kind::Number,
                required: true,
                default: "",
            },
            option(
                "statistic",
                "Statistics Type",
                &[
                    "MEAN", "MAJORITY", "MAXIMUM", "MINIMUM", "MINORITY", "RANGE", "VARIETY",
                    "LENGTH",
                ],
                "MEAN",
            ),
        ],
        run: line_statistics,
    },
    binary_tool!(diff_tool, "diff", "Diff"),
    binary_tool!(over_tool, "over", "Over"),
    Tool {
        id: "compute_change_raster",
        name: "Compute Change Raster",
        summary: "Difference or relative difference between two single-band rasters.",
        params: &[
            raster("from_raster", "From Raster", true),
            raster("to_raster", "To Raster", true),
            option(
                "change_type",
                "Method",
                &["DIFFERENCE", "RELATIVE_DIFFERENCE"],
                "DIFFERENCE",
            ),
        ],
        run: compute_change_raster,
    },
    Tool {
        id: "create_binary_mask",
        name: "Create Binary Mask",
        summary: "1 where the cell differs from the background value, else 0.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "background_value",
                label: "Background Value",
                kind: Kind::Number,
                required: false,
                default: "",
            },
        ],
        run: create_binary_mask,
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
        id: "extract_by_polygon",
        name: "Extract by Polygon",
        summary: "The cells inside (or outside) a polygon, on the input's grid.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "polygon",
                label: "Polygon",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "extraction_area",
                "Extraction Area",
                &["INSIDE", "OUTSIDE"],
                "INSIDE",
            ),
        ],
        run: extract_by_polygon,
    },
    Tool {
        id: "extract_by_points",
        name: "Extract by Points",
        summary: "The cells that contain (or do not contain) points, on the input's grid.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "points",
                label: "Points",
                kind: Kind::Text,
                required: true,
                default: "",
            },
            option(
                "extraction_area",
                "Extraction Area",
                &["INSIDE", "OUTSIDE"],
                "INSIDE",
            ),
        ],
        run: extract_by_points,
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
        id: "zonal_geometry_as_table",
        name: "Zonal Geometry as Table",
        summary: "A table of each zone's area, perimeter, thickness, centroid and equivalent ellipse.",
        params: &[
            raster("input", "Input raster or feature zone data", true),
            Param {
                name: "zone_field",
                label: "Zone field",
                kind: Kind::Text,
                required: false,
                default: "Value",
            },
        ],
        run: zonal_geometry_table_unsupported,
    },
    Tool {
        id: "zonal_geometry",
        name: "Zonal Geometry",
        summary: "Calculates area, perimeter, thickness or centroids for each raster zone.",
        params: &[
            raster("in_zone_data", "Input Raster or Feature Zone Data", true),
            Param {
                name: "zone_field",
                label: "Zone Field",
                kind: Kind::Text,
                required: true,
                default: "Value",
            },
            option(
                "geometry_type",
                "Geometry Type",
                &["AREA", "PERIMETER", "THICKNESS", "CENTROID"],
                "AREA",
            ),
            Param {
                name: "cell_size",
                label: "Output Cell Size",
                kind: Kind::Number,
                required: false,
                default: "",
            },
        ],
        run: zonal_geometry,
    },
    Tool {
        id: "zonal_fill",
        name: "Zonal Fill",
        summary: "Fills each zone with the lowest weight along its boundary at which it spills (the higher weight of each cell pair across the boundary).",
        params: &[
            raster("in_zone_raster", "Input zone raster", true),
            raster("in_weight_raster", "Input weight raster", true),
        ],
        run: zonal_fill,
    },
    Tool {
        id: "rescale_by_function",
        name: "Rescale by Function",
        summary: "Cell values through a transformation function (Linear, Gaussian, Logistic, MSSmall, ...) onto an evaluation scale.",
        params: &[
            raster("input", "Input Raster", true),
            Param {
                name: "transformation_function",
                label: "Transformation function",
                kind: Kind::Text,
                required: false,
                default: "MSSMALL # # # # # #",
            },
            Param {
                name: "from_scale",
                label: "From scale",
                kind: Kind::Number,
                required: false,
                default: "1",
            },
            Param {
                name: "to_scale",
                label: "To scale",
                kind: Kind::Number,
                required: false,
                default: "10",
            },
        ],
        run: rescale_by_function,
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
