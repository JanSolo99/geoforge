//! Graphic Buffer: ArcGIS reference-tested cap and join styles over the native geometry engine.

use unigis_core::{Error, Feature, Field, FieldType, GeometryKind, Result, Value};

use crate::ToolOutput;
use crate::registry::{Context, Kind, LINEAR_UNITS, Param, Runner, Tool, distance_in_units};
use unigis_geometry::buffer::{GraphicCap as Cap, GraphicJoin as Join, graphic_buffer_geometry};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

fn graphic_distance(text: &str, wkt: Option<&str>, from_field: bool) -> Result<f64> {
    let text = text.trim();
    let split = text
        .find(|c: char| c.is_alphabetic() && c != 'e' && c != 'E')
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number
        .trim()
        .parse()
        .map_err(|_| err(format!("Graphic Buffer cannot read distance '{text}'")))?;
    let unit = unit.to_ascii_uppercase().replace([' ', '_'], "");
    // Graphic Buffer's ArcGIS unit parser defaults to the US definitions. Keep
    // this local: other tools' unit parsers have not been probed for these aliases.
    let meters = match unit.as_str() {
        "FEET" | "FOOT" | "FEETUS" | "FOOTUS" => Some(1200.0 / 3937.0),
        "FEETINT" | "FOOTINT" => Some(0.3048),
        "INCH" | "INCHES" => Some(100.0 / 3937.0),
        "YARD" | "YARDS" => Some(3600.0 / 3937.0),
        "MILE" | "MILES" => Some(6_336_000.0 / 3937.0),
        "NAUTICALMILE" | "NAUTICALMILES" => Some(1853.248),
        _ => None,
    };
    let distance = if let Some(meters) = meters {
        distance_in_units(&format!("{} Meters", value * meters), wkt)?
    } else {
        match distance_in_units(text, wkt) {
            Ok(v) => v,
            // An unrecognized unit in a distance field falls back to map units.
            // Malformed numeric text is refused above, including empty strings.
            Err(_)
                if from_field
                    && !unit.is_empty()
                    && unit != "UNKNOWN"
                    && !LINEAR_UNITS.iter().any(|(name, _)| *name == unit) =>
            {
                value
            }
            Err(e) => return Err(e),
        }
    };
    if !distance.is_finite() {
        return Err(err("distance must be finite"));
    }
    Ok(distance)
}

fn validate_distance(distance: f64, kind: GeometryKind, from_field: bool) -> Result<()> {
    if !distance.is_finite() {
        return Err(err("distance must be finite"));
    }
    if kind != GeometryKind::Polygon {
        if distance < 0.0 {
            return Err(err(
                "ERROR 000109: The buffer distance cannot be negative for lines and points",
            ));
        }
        if distance == 0.0 && !from_field {
            return Err(err("ERROR 000026: Buffer distance is zero"));
        }
    }
    Ok(())
}

fn graphic_buffer(ctx: &dyn Context) -> Result<ToolOutput> {
    let input = ctx.features("input")?;
    let kind = input
        .geometry_kind()
        .ok_or_else(|| err("ERROR 000368: Invalid input data: features are required"))?;
    let wkt = input.spatial_reference_wkt();
    if wkt.is_some_and(|w| w.trim_start().to_ascii_uppercase().starts_with("GEOG")) {
        return Err(err(
            "Graphic Buffer geographic input is not supported yet; project the features first",
        ));
    }
    let distance = ctx
        .get("distance")
        .ok_or_else(|| err("ERROR 000735: Distance: value is required"))?;
    let distance_field = input
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(distance.trim()));
    let constant = if distance_field.is_none() {
        let d = graphic_distance(&distance, wkt, false).map_err(|_| {
            err(format!(
                "ERROR 000728: Field {distance} does not exist within table, or distance is unsupported"
            ))
        })?;
        validate_distance(d, kind, false)?;
        Some(d)
    } else {
        None
    };
    if let Some(index) = distance_field {
        match input.fields()[index].field_type {
            FieldType::SmallInteger
            | FieldType::Integer
            | FieldType::BigInteger
            | FieldType::Single
            | FieldType::Double
            | FieldType::String => {}
            _ => {
                return Err(err(
                    "Graphic Buffer distance fields must be numeric or text",
                ));
            }
        }
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
    let mut dist_field = Field::new("BUFF_DIST", FieldType::Double);
    dist_field.precision = 18;
    dist_field.scale = 11;
    let mut orig = Field::new("ORIG_FID", FieldType::Integer);
    orig.length = 10;
    orig.precision = 10;
    if matches!(kind, GeometryKind::Point | GeometryKind::Multipoint) && matches!(cap, Cap::Butt) {
        return Err(err(
            "ERROR 000800: The value is not a member of SQUARE | ROUND.",
        ));
    }
    let mut fields = input.fields().to_vec();
    let dist_index = fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case("BUFF_DIST"))
        .unwrap_or_else(|| {
            fields.push(dist_field);
            fields.len() - 1
        });
    let orig_index = fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case("ORIG_FID"))
        .unwrap_or_else(|| {
            fields.push(orig);
            fields.len() - 1
        });
    let mut features = Vec::new();
    let mut messages = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if g.has_curves() || g.z.is_some() {
            return Err(err("Graphic Buffer does not handle curves or Z yet"));
        }
        let d = if let Some(index) = distance_field {
            match f.attributes.get(index).unwrap_or(&Value::Null) {
                Value::SmallInteger(v) => f64::from(*v),
                Value::Integer(v) => f64::from(*v),
                Value::BigInteger(v) => *v as f64,
                Value::Single(v) => f64::from(*v),
                Value::Double(v) => *v,
                Value::String(v) => graphic_distance(v, wkt, true)?,
                Value::Null if input.fields()[index].field_type != FieldType::String => 0.0,
                _ => {
                    return Err(err(format!(
                        "Graphic Buffer cannot read the distance field for feature {}; null or malformed text distances are unsupported",
                        f.oid
                    )));
                }
            }
        } else {
            constant.expect("constant distance was validated")
        };
        validate_distance(d, kind, distance_field.is_some())?;
        let geometry = if d == 0.0 {
            None
        } else {
            graphic_buffer_geometry(g, d, cap, join, limit)?
        };
        let Some(geometry) = geometry else {
            messages.push(format!("WARNING 000636: Skipping feature {} because a negative or very small distance resulted in no geometry.", f.oid));
            continue;
        };
        let mut attributes = f.attributes.clone();
        attributes.resize(fields.len(), Value::Null);
        attributes[dist_index] = Value::Double(d);
        attributes[orig_index] = Value::Integer(
            i32::try_from(f.oid).map_err(|_| err("Graphic Buffer requires 32-bit feature IDs"))?,
        );
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(geometry),
            attributes,
        });
    }
    if features.is_empty() {
        messages.push("WARNING 000117: Warning empty output generated.".into());
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages,
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
            help: "A constant (e.g. 20 Meters) or a numeric/text distance field.",
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
