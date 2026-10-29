//! Polygon study-area sampling designs and explicit sample allocation.
use crate::{
    ToolOutput, construct,
    registry::{Context, Kind, Param, Runner, Tool},
};
use std::collections::BTreeMap;
use unigis_core::{Error, Feature, Field, FieldType, Geometry, GeometryKind, Result, Value};

pub const TOOL: Tool = Tool {
    id: "create_spatial_sampling_locations",
    name: "Create Spatial Sampling Locations",
    toolbox: "Data Management › Sampling",
    summary: "Creates random, stratified or systematic sampling locations within a polygon study area.",
    params: &[
        Param {
            name: "input",
            label: "Input Study Area",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "Polygon study area. Raster study areas are unsupported.",
        },
        Param {
            name: "sampling_method",
            label: "Sampling Method",
            kind: Kind::Choice(&[
                "RANDOM",
                "STRAT_POLY",
                "STRAT_ID",
                "STRAT_RAST",
                "SYSTEMATIC",
                "CLUSTER",
            ]),
            required: false,
            default: "RANDOM",
            help: "Random, polygon/field strata or systematic. Raster strata and cluster designs unsupported.",
        },
        Param {
            name: "strata_id_field",
            label: "Strata ID Field",
            kind: Kind::Field {
                of: "input",
                numeric: false,
            },
            required: false,
            default: "",
            help: "Integer or text field defining strata.",
        },
        Param {
            name: "strata_count_method",
            label: "Strata Sample Count Allocation Method",
            kind: Kind::Choice(&["EQUAL", "PROP_AREA", "FIELD", "PROP_FIELD"]),
            required: false,
            default: "EQUAL",
            help: "Equal per stratum, proportional to area/population, or exact population count.",
        },
        Param {
            name: "bin_shape",
            label: "Bin Shape",
            kind: Kind::Choice(&[
                "SQUARE",
                "DIAMOND",
                "HEXAGON",
                "TRANSVERSE_HEXAGON",
                "TRIANGLE",
                "H3_HEXAGON",
            ]),
            required: false,
            default: "HEXAGON",
            help: "Systematic tessellation. H3 is unsupported.",
        },
        Param {
            name: "bin_size",
            label: "Bin Size",
            kind: Kind::Text,
            required: false,
            default: "100",
            help: "Systematic bins require an explicit area unit; count optimization is unsupported.",
        },
        Param {
            name: "h3_resolution",
            label: "H3 Resolution",
            kind: Kind::Number,
            required: false,
            default: "7",
            help: "H3 bins are unsupported.",
        },
        Param {
            name: "num_samples",
            label: "Number of Samples",
            kind: Kind::Number,
            required: false,
            default: "100",
            help: "Total random or proportionally allocated samples.",
        },
        Param {
            name: "num_samples_per_strata",
            label: "Number of Samples Per Stratum",
            kind: Kind::Number,
            required: false,
            default: "100",
            help: "Samples in each stratum for EQUAL allocation.",
        },
        Param {
            name: "population_field",
            label: "Population Field",
            kind: Kind::Field {
                of: "input",
                numeric: true,
            },
            required: false,
            default: "",
            help: "Nonnegative integer field; summed within each ID stratum.",
        },
        Param {
            name: "geometry_type",
            label: "Output Geometry Type",
            kind: Kind::Choice(&["POINT", "POLYGON"]),
            required: false,
            default: "POINT",
            help: "Systematic centroid points or complete tessellation cells.",
        },
        Param {
            name: "min_distance",
            label: "Minimum Distance Between Sample Points",
            kind: Kind::Text,
            required: false,
            default: "0",
            help: "Planar spacing per stratum, with optional linear unit.",
        },
        Param {
            name: "spatial_relationship",
            label: "Spatial Relationship",
            kind: Kind::Choice(&["HAVE_THEIR_CENTER_IN", "COMPLETELY_WITHIN", "INTERSECT"]),
            required: false,
            default: "HAVE_THEIR_CENTER_IN",
            help: "Inclusion of systematic polygon bins; points use centroid inclusion.",
        },
        Param {
            name: "seed",
            label: "Random Seed",
            kind: Kind::Text,
            required: false,
            default: "0",
            help: "Reproducible native seed; differs from ArcGIS random streams.",
        },
    ],
    output_suffix: "samples",
    measures: false,
    run: Runner::Features(run),
};
fn bad(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}
fn value(ctx: &dyn Context, k: &str, d: &str) -> String {
    ctx.get(k).unwrap_or_else(|| d.into())
}
fn positive(ctx: &dyn Context, k: &str) -> Result<usize> {
    let n = value(ctx, k, "100")
        .parse::<usize>()
        .map_err(|_| bad("ERROR 000531: Value must be greater than zero"))?;
    if n == 0 {
        return Err(bad("ERROR 000531: Value must be greater than zero"));
    }
    if n > 1_000_000 {
        return Err(bad("at most 1000000 sampling locations are supported"));
    }
    Ok(n)
}
/// Largest-remainder allocation, with stable stratum order breaking ties.
fn allocate(total: usize, weights: &[f64]) -> Result<Vec<usize>> {
    let sum: f64 = weights.iter().sum();
    if !sum.is_finite() || sum <= 0.0 || weights.iter().any(|w| !w.is_finite() || *w < 0.0) {
        return Err(bad(
            "allocation requires nonnegative finite weights with positive total",
        ));
    }
    let exact: Vec<_> = weights.iter().map(|w| *w / sum * total as f64).collect();
    let mut counts: Vec<_> = exact.iter().map(|n| n.floor() as usize).collect();
    let assigned: usize = counts.iter().sum();
    let mut order: Vec<_> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| {
        (exact[b] - counts[b] as f64)
            .total_cmp(&(exact[a] - counts[a] as f64))
            .then_with(|| a.cmp(&b))
    });
    if assigned > total || total - assigned > order.len() {
        return Err(bad("sample allocation rounding overflow"));
    }
    for &i in order.iter().take(total - assigned) {
        counts[i] += 1;
    }
    Ok(counts)
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Null,
    Integer(i64),
    Text(String),
}
fn key(v: &Value) -> Result<Key> {
    Ok(match v {
        Value::Null => Key::Null,
        Value::SmallInteger(n) => Key::Integer(i64::from(*n)),
        Value::Integer(n) => Key::Integer(i64::from(*n)),
        Value::BigInteger(n) => Key::Integer(*n),
        Value::String(s) => Key::Text(s.clone()),
        _ => return Err(bad("strata ID field must contain integers or text")),
    })
}
fn integer(v: &Value) -> Result<u64> {
    let n = match v {
        Value::SmallInteger(n) => i64::from(*n),
        Value::Integer(n) => i64::from(*n),
        Value::BigInteger(n) => *n,
        Value::Null => 0,
        _ => return Err(bad("population field must contain nonnegative integers")),
    };
    u64::try_from(n).map_err(|_| bad("population field cannot contain negative values"))
}
fn dissolved(geometries: &[Geometry]) -> Result<Geometry> {
    unigis_geometry::dissolve(geometries)?.ok_or_else(|| bad("study area has no polygon area"))
}
fn field_index(fields: &[Field], name: Option<String>) -> Result<usize> {
    let name = name.ok_or_else(|| bad("ERROR 000530: Value is required"))?;
    fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| {
            bad(format!(
                "ERROR 000728: Field {name} does not exist within table"
            ))
        })
}
fn run(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    if fc.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(bad(
            "sampling study area must be polygons; raster inputs are unsupported",
        ));
    }
    let method = value(ctx, "sampling_method", "RANDOM").to_ascii_uppercase();
    if matches!(method.as_str(), "STRAT_RAST" | "CLUSTER") {
        return Err(bad("raster strata and cluster sampling are not supported"));
    }
    let allocation = value(ctx, "strata_count_method", "EQUAL").to_ascii_uppercase();
    let proportional = matches!(allocation.as_str(), "PROP_AREA" | "PROP_FIELD");
    let id_field = if method == "STRAT_ID" {
        Some(field_index(fc.fields(), ctx.get("strata_id_field"))?)
    } else {
        None
    };
    if let Some(i) = id_field
        && !matches!(
            fc.fields()[i].field_type,
            FieldType::String
                | FieldType::SmallInteger
                | FieldType::Integer
                | FieldType::BigInteger
        )
    {
        return Err(bad("strata ID field must be integer or text"));
    }
    let population =
        if method.starts_with("STRAT_") && matches!(allocation.as_str(), "FIELD" | "PROP_FIELD") {
            let i = field_index(fc.fields(), ctx.get("population_field"))?;
            if !matches!(
                fc.fields()[i].field_type,
                FieldType::SmallInteger | FieldType::Integer | FieldType::BigInteger
            ) {
                return Err(bad("population field must be an integer field"));
            }
            Some(i)
        } else {
            None
        };
    let mut groups: BTreeMap<Key, (Vec<Geometry>, u64)> = BTreeMap::new();
    for f in fc.features() {
        let f = f?;
        let Some(g) = f.geometry else { continue };
        if g.has_curves() || g.xy.iter().flatten().any(|v| !v.is_finite()) {
            return Err(bad("curved or nonfinite study areas are unsupported"));
        }
        let k = if method == "STRAT_POLY" {
            Key::Integer(f.oid)
        } else if let Some(i) = id_field {
            key(&f.attributes[i])?
        } else {
            Key::Null
        };
        let entry = groups.entry(k).or_default();
        if let Some(i) = population {
            entry.1 = entry
                .1
                .checked_add(integer(&f.attributes[i])?)
                .ok_or_else(|| bad("population total overflow"))?;
        }
        entry.0.push(g);
    }
    let shapes: Vec<_> = groups
        .values()
        .map(|(g, _)| dissolved(g))
        .collect::<Result<_>>()?;
    if shapes.is_empty() {
        return Err(bad("study area has no polygon area"));
    }
    let wkt = fc.spatial_reference_wkt().map(str::to_owned);
    if method == "SYSTEMATIC" {
        return systematic(ctx, &shapes[0], wkt);
    }
    let counts = if method == "RANDOM" {
        vec![positive(ctx, "num_samples")?]
    } else {
        match allocation.as_str() {
            "EQUAL" => vec![positive(ctx, "num_samples_per_strata")?; shapes.len()],
            "FIELD" => groups
                .values()
                .map(|(_, n)| usize::try_from(*n).map_err(|_| bad("population count overflow")))
                .collect::<Result<_>>()?,
            "PROP_AREA" => allocate(
                positive(ctx, "num_samples")?,
                &shapes
                    .iter()
                    .map(unigis_geometry::area)
                    .collect::<Result<Vec<_>>>()?,
            )?,
            "PROP_FIELD" => allocate(
                positive(ctx, "num_samples")?,
                &groups.values().map(|(_, p)| *p as f64).collect::<Vec<_>>(),
            )?,
            _ => return Err(bad("unsupported allocation method")),
        }
    };
    if counts
        .iter()
        .try_fold(0usize, |sum, n| sum.checked_add(*n))
        .is_none_or(|n| n > 1_000_000)
    {
        return Err(bad("at most 1000000 sampling locations are supported"));
    }
    let gap = crate::registry::distance_in_units(&value(ctx, "min_distance", "0"), wkt.as_deref())?;
    if !gap.is_finite() || gap < 0.0 {
        return Err(bad("minimum spacing must be finite and nonnegative"));
    }
    let seed = value(ctx, "seed", "0")
        .parse::<u64>()
        .map_err(|_| bad("seed must be an unsigned integer"))?;
    let mut cid = Field::new("CID", FieldType::Integer);
    cid.precision = 10;
    let mut out = ToolOutput {
        kind: GeometryKind::Point,
        has_z: false,
        has_m: false,
        fields: vec![cid],
        spatial_reference_wkt: wkt,
        features: vec![],
        messages: vec![],
    };
    for (i, (g, n)) in shapes.iter().zip(counts).enumerate() {
        let points = crate::random_points::polygon_points(g, n, gap, seed.wrapping_add(i as u64))?;
        if points.len() < n && !out.messages.iter().any(|m| m.contains("110547")) {
            out.messages.push("WARNING 110547: The specified number of locations could not be created in all cases.".into());
        }
        let cid = if method == "RANDOM" {
            1
        } else {
            i32::try_from(i + usize::from(!proportional)).map_err(|_| bad("too many strata"))?
        };
        for p in points {
            out.features.push(Feature {
                oid: out.features.len() as i64,
                geometry: Some(Geometry::point(p[0], p[1])),
                attributes: vec![Value::Integer(cid)],
            });
        }
    }
    if out.features.is_empty() {
        return Err(bad("ERROR 110544: No sample locations were created."));
    }
    Ok(out)
}
fn systematic(ctx: &dyn Context, study: &Geometry, wkt: Option<String>) -> Result<ToolOutput> {
    let shape = match value(ctx, "bin_shape", "HEXAGON")
        .to_ascii_uppercase()
        .as_str()
    {
        "SQUARE" => construct::Tessellation::Square,
        "DIAMOND" => construct::Tessellation::Diamond,
        "HEXAGON" => construct::Tessellation::Hexagon,
        "TRANSVERSE_HEXAGON" => construct::Tessellation::TransverseHexagon,
        "TRIANGLE" => construct::Tessellation::Triangle,
        _ => return Err(bad("H3 sampling bins are unsupported")),
    };
    let size = value(ctx, "bin_size", "100");
    if size.trim().parse::<f64>().is_ok() {
        return Err(bad(
            "systematic sampling needs an explicit bin area; bin-count optimization is unsupported",
        ));
    }
    let area = crate::registry::area_in_units(&size, wkt.as_deref())?;
    let e = study.envelope();
    let mut out =
        construct::generate_tessellation(shape, [e.xmin, e.ymin, e.xmax, e.ymax], area, wkt)?;
    let polygon = value(ctx, "geometry_type", "POINT").eq_ignore_ascii_case("POLYGON");
    let relation = value(ctx, "spatial_relationship", "HAVE_THEIR_CENTER_IN").to_ascii_uppercase();
    let grid = out.fields[1].clone();
    out.fields = vec![
        Field::new("Shape_Length", FieldType::Double),
        Field::new("Shape_Area", FieldType::Double),
        grid,
    ];
    if !polygon {
        let mut id = Field::new("ORIG_FID", FieldType::Integer);
        id.precision = 10;
        out.fields.push(id);
    }
    out.kind = if polygon {
        GeometryKind::Polygon
    } else {
        GeometryKind::Point
    };
    let tolerance = crate::default_xy_tolerance(out.spatial_reference_wkt.as_deref());
    let resolution = tolerance / 10.0;
    let mut features = Vec::new();
    for f in out.features {
        let Some(mut g) = f.geometry else { continue };
        // ArcGIS builds a temporary geodatabase tessellation: measurements and
        // centroids use stored vertices at its standard XY resolution.
        for point in &mut g.xy {
            for coordinate in point {
                *coordinate = (*coordinate / resolution).round() * resolution;
            }
        }
        let Some(p) = unigis_geometry::centroid(&g)? else {
            continue;
        };
        let center = Geometry::point(p[0], p[1]);
        let include = if !polygon || relation == "HAVE_THEIR_CENTER_IN" {
            unigis_geometry::within(&center, study)?
                || unigis_geometry::nearest(&center, study)?
                    .is_some_and(|n| n.distance <= tolerance)
        } else if relation == "COMPLETELY_WITHIN" {
            unigis_geometry::within(&g, study)?
        } else {
            !unigis_geometry::relate_matches(&g, study, "FF*FF****")?
        };
        if include {
            let mut attrs = vec![
                Value::Double(unigis_geometry::length(&g)?),
                Value::Double(unigis_geometry::area(&g)?),
                f.attributes[1].clone(),
            ];
            if !polygon {
                attrs.push(Value::Integer(
                    i32::try_from(f.oid + 1).map_err(|_| bad("grid OID overflow"))?,
                ));
            }
            features.push(Feature {
                oid: features.len() as i64,
                geometry: Some(if polygon { g } else { center }),
                attributes: attrs,
            });
        }
    }
    if features.is_empty() {
        return Err(bad("ERROR 110544: No sample locations were created."));
    }
    out.features = features;
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn largest_remainder_keeps_total_and_stable_ties() {
        assert_eq!(allocate(7, &[1.0, 3.0, 2.0]).unwrap(), [1, 4, 2]);
        assert_eq!(allocate(9, &[5.0, 5.0]).unwrap(), [5, 4]);
        assert_eq!(allocate(9, &[2.0, 3.0, 5.0]).unwrap(), [2, 3, 4]);
        assert!(allocate(5, &[0.0, 0.0]).is_err());
        assert!(allocate(5, &[f64::NAN, 1.0]).is_err());
    }
}
