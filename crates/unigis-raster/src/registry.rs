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
    /// The numeric columns (name, values) of the attribute table (`.vat.dbf`) of the raster the
    /// parameter names.
    fn attribute_table(&self, name: &str) -> Result<Vec<(String, Vec<f64>)>>;
}

/// A tool's result: the raster, and whether ArcGIS writes statistics (`.aux.xml`) beside it.
pub struct Output {
    pub raster: Raster,
    pub statistics: bool,
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
    })
}

fn filter(ctx: &dyn Context) -> Result<Output> {
    let low = choice(ctx, "filter", "filter_type").eq_ignore_ascii_case("LOW");
    let data = choice(ctx, "filter", "ignore_nodata").eq_ignore_ascii_case("DATA");
    Ok(Output {
        raster: crate::focal::filter(&ctx.raster("input")?, low, data),
        statistics: true,
    })
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
    Ok(Output { raster, statistics })
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

const EXTRACTION_AREA: &[&str] = &["INSIDE", "OUTSIDE"];

pub static TOOLS: &[Tool] = &[
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
