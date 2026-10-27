//! Graphic Buffer: ArcGIS reference-tested cap and join styles over the native geometry engine.

use unigis_core::{Error, Feature, Field, FieldType, GeometryKind, Result, Value};

use crate::ToolOutput;
use crate::registry::{Context, Kind, Param, Runner, Tool, distance_in_units};
use unigis_geometry::buffer::{GraphicCap as Cap, GraphicJoin as Join, graphic_buffer_geometry};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

fn graphic_buffer(ctx: &dyn Context) -> Result<ToolOutput> {
    let input = ctx.features("input")?;
    input
        .geometry_kind()
        .ok_or_else(|| err("ERROR 000368: Invalid input data: features are required"))?;
    let d = distance_in_units(
        &ctx.get("distance")
            .ok_or_else(|| err("ERROR 000735: Distance: value is required"))?,
        input.spatial_reference_wkt(),
    )?;
    if !d.is_finite() || d <= 0.0 {
        return Err(err("ERROR 000800: the distance must be greater than zero"));
    }
    let cap = match ctx
        .get("line_caps")
        .unwrap_or_else(|| "SQUARE".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "SQUARE" => Cap::Square,
        "BUTT" => Cap::Butt,
        "ROUND" => Cap::Round,
        other => return Err(err(format!("ERROR 000067: Line Caps: {other}"))),
    };
    let join = match ctx
        .get("line_joins")
        .unwrap_or_else(|| "MITER".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "MITER" => Join::Miter,
        "BEVEL" => Join::Bevel,
        "ROUND" => Join::Round,
        other => return Err(err(format!("ERROR 000067: Line Joins: {other}"))),
    };
    let limit: f64 = ctx
        .get("miter_limit")
        .map(|v| v.trim().parse::<f64>())
        .transpose()
        .map_err(|_| err("miter_limit must be a finite number >= 1"))?
        .unwrap_or(10.0);
    if !limit.is_finite() || limit < 1.0 {
        return Err(err("miter_limit must be a finite number >= 1"));
    }
    if input.fields().iter().any(|f| {
        f.name.eq_ignore_ascii_case("BUFF_DIST") || f.name.eq_ignore_ascii_case("ORIG_FID")
    }) {
        return Err(err(
            "Graphic Buffer does not yet overwrite existing BUFF_DIST or ORIG_FID fields",
        ));
    }
    let mut dist_field = Field::new("BUFF_DIST", FieldType::Double);
    dist_field.precision = 18;
    dist_field.scale = 11;
    let mut orig = Field::new("ORIG_FID", FieldType::Integer);
    orig.length = 10;
    orig.precision = 10;
    let mut fields = input.fields().to_vec();
    fields.push(dist_field);
    fields.push(orig);
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if g.has_curves() || g.z.is_some() {
            return Err(err("Graphic Buffer does not handle curves or Z yet"));
        }
        let geometry = graphic_buffer_geometry(g, d, cap, join, limit)?;
        let mut attributes = f.attributes.clone();
        attributes.push(Value::Double(d));
        attributes.push(Value::Integer(f.oid as i32));
        features.push(Feature {
            oid: features.len() as i64,
            geometry,
            attributes,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

pub const GRAPHIC_BUFFER: Tool = Tool {
    id: "graphic_buffer",
    name: "Graphic Buffer",
    toolbox: "Analysis › Proximity",
    summary: "Buffers with chosen line caps and corner joins (ArcGIS Pro Graphic Buffer).",
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
            name: "distance",
            label: "Distance",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "e.g. 20 Meters.",
        },
        Param {
            name: "line_caps",
            label: "Line Caps",
            kind: Kind::Choice(&["SQUARE", "BUTT", "ROUND"]),
            required: false,
            default: "SQUARE",
            help: "",
        },
        Param {
            name: "line_joins",
            label: "Line Joins",
            kind: Kind::Choice(&["MITER", "BEVEL", "ROUND"]),
            required: false,
            default: "MITER",
            help: "",
        },
        Param {
            name: "miter_limit",
            label: "Miter Limit",
            kind: Kind::Number,
            required: false,
            default: "10",
            help: "",
        },
    ],
    output_suffix: "GraphicBuffer",
    measures: true,
    run: Runner::Features(graphic_buffer),
};
