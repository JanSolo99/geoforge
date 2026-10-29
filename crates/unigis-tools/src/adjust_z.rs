//! Adjust 3D Z (Data Management): z values shifted, converted between units and/or negated.

use unigis_core::{Error, FeatureClass, Result};

use crate::ToolOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// Metres per unit; feet, yards, fathoms and inches are US survey ones (1 m = 39.37 in), as
/// ArcGIS uses here.
fn metres_per(unit: &str) -> Result<f64> {
    Ok(match unit.to_ascii_uppercase().as_str() {
        "MILLIMETERS" => 0.001,
        "CENTIMETERS" => 0.01,
        "METERS" => 1.0,
        "INCHES" => 1.0 / 39.37,
        "FEET" => 1200.0 / 3937.0,
        "YARDS" => 3600.0 / 3937.0,
        "FATHOMS" => 7200.0 / 3937.0,
        other => return Err(err(format!("ERROR 000800: unit {other} is not supported"))),
    })
}

/// How to adjust: `adjust` is a number or the name of a numeric field (null counts as 0).
pub struct AdjustZ<'a> {
    pub reverse: bool,
    pub adjust: Option<&'a str>,
    pub from_units: Option<&'a str>,
    pub to_units: Option<&'a str>,
}

/// Adjust 3D Z: every z becomes `(z + adjust) * factor`, negated with `reverse` (observed
/// against ArcGIS Pro 3.7). The factor converts `from_units` to `to_units`. ArcGIS leaves a
/// feature unchanged (WARNING 002631) when a new z falls outside the class's z domain; that
/// check is not made here.
pub fn adjust_3d_z(input: &dyn FeatureClass, opts: &AdjustZ) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| err("ERROR 000368: Invalid input data: features are required"))?;
    if !input.has_z() {
        return Err(err(
            "ERROR 001244: The input feature class must be Z-enabled",
        ));
    }
    let factor = match (opts.from_units, opts.to_units) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => metres_per(a)? / metres_per(b)?,
        _ => 1.0,
    };
    let fields = input.fields().to_vec();
    let adjust = opts.adjust.map(str::trim).filter(|a| !a.is_empty());
    let (constant, field) = match adjust {
        None => (0.0, None),
        Some(a) => match a.parse::<f64>() {
            Ok(v) => (v, None),
            Err(_) => (
                0.0,
                Some(
                    fields
                        .iter()
                        .position(|f| f.name.eq_ignore_ascii_case(a))
                        .ok_or_else(|| err(format!("ERROR 000728: Field {a} does not exist")))?,
                ),
            ),
        },
    };
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        let shift = field.map_or(constant, |i| crate::number(&f.attributes[i]).unwrap_or(0.0));
        if let Some(g) = &mut f.geometry
            && let Some(z) = &mut g.z
        {
            for v in z.iter_mut() {
                let a = (*v + shift) * factor;
                *v = if opts.reverse { -a } else { a };
            }
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind,
        has_z: true,
        has_m: input.has_m(),
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}
