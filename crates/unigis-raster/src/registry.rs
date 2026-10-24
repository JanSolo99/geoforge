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

pub static TOOLS: &[Tool] = &[
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
            option("ignore_nodata", "Ignore NoData", &["DATA", "NODATA"], "DATA"),
        ],
        run: filter,
    },
];
