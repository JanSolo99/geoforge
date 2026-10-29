//! Pairwise Polygon Neighbors: ranked contact statistics aggregated by report keys.
use std::collections::BTreeMap;

use unigis_core::{Error, FeatureClass, Field, FieldType, Geometry, GeometryKind, Result, Value};

use crate::TableOutput;
use crate::registry::{Context, Kind, Outcome, Param, Runner, Tool};

fn err(message: &str) -> Error {
    Error::Unsupported(message.into())
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Integer(i32),
    Text(String),
}
impl Key {
    fn from_value(value: &Value) -> Result<Self> {
        match value {
            Value::SmallInteger(v) => Ok(Self::Integer(i32::from(*v))),
            Value::Integer(v) => Ok(Self::Integer(*v)),
            Value::String(v) => Ok(Self::Text(v.clone())),
            _ => Err(err(
                "report fields currently require non-null text or 32-bit integer values",
            )),
        }
    }
    fn value(&self) -> Value {
        match self {
            Self::Integer(v) => Value::Integer(*v),
            Self::Text(v) => Value::String(v.clone()),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Contact {
    area: f64,
    length: f64,
    nodes: i32,
}

fn contact(a: &Geometry, b: &Geometry, area_overlap: bool, tolerance: f64) -> Result<Contact> {
    let overlap = match unigis_geometry::intersection(a, b)? {
        Some(g) if g.kind == GeometryKind::Polygon => unigis_geometry::area(&g)?,
        _ => 0.0,
    };
    // Pairwise excludes crossing/overlapping boundaries from lower-order contacts,
    // even for aligned coincident edges. Independent ArcGIS holdouts confirm this.
    if overlap > 0.0 {
        return Ok(Contact {
            area: if area_overlap { overlap } else { 0.0 },
            ..Contact::default()
        });
    }
    let meet = unigis_geometry::meeting(a, b, tolerance)?;
    let length = meet
        .shared
        .iter()
        .flat_map(|p| p.windows(2))
        .map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]))
        .sum();
    Ok(Contact {
        area: 0.0,
        length,
        nodes: if length > 0.0 {
            0
        } else {
            meet.points.len() as i32
        },
    })
}

fn integer(name: &str) -> Field {
    let mut f = Field::new(name, FieldType::Integer);
    f.length = 10;
    f.precision = 10;
    f
}

fn linear_scale(unit: &str, wkt: Option<&str>) -> Result<f64> {
    let meters = match unit.to_ascii_uppercase().as_str() {
        "UNKNOWN" | "" => return Ok(1.0),
        "METERS" => 1.0,
        "KILOMETERS" => 1000.0,
        "FEET_INTERNATIONAL" => 0.3048,
        "FEET" => 1200.0 / 3937.0,
        _ => {
            return Err(err(
                "unsupported output linear unit; use UNKNOWN, METERS, KILOMETERS, FEET_INTERNATIONAL or FEET",
            ));
        }
    };
    Ok(meters * crate::registry::distance_in_units("1 Meters", wkt)?)
}

fn run(ctx: &dyn Context) -> Result<Outcome> {
    if ctx
        .get("cluster_tolerance")
        .is_some_and(|v| !v.trim().is_empty())
    {
        return Err(err("custom cluster_tolerance is not yet supported"));
    }
    if ctx
        .get("precision")
        .is_some_and(|v| !v.eq_ignore_ascii_case("DEFAULT_PRECISION"))
    {
        return Err(err("MAX_PRECISION is not yet supported"));
    }
    let input = ctx.features("input")?;
    generate(
        input.as_ref(),
        ctx.get("fields").as_deref(),
        ctx.get("area_overlap")
            .is_some_and(|v| v.eq_ignore_ascii_case("AREA_OVERLAP")),
        !ctx.get("both_sides")
            .is_some_and(|v| v.eq_ignore_ascii_case("NO_BOTH_SIDES")),
        &ctx.get("out_linear_units")
            .unwrap_or_else(|| "UNKNOWN".into()),
        &ctx.get("out_area_units")
            .unwrap_or_else(|| "UNKNOWN".into()),
    )
    .map(Outcome::Table)
}

pub fn generate(
    input: &dyn FeatureClass,
    report: Option<&str>,
    area_overlap: bool,
    both_sides: bool,
    linear_unit: &str,
    area_unit: &str,
) -> Result<TableOutput> {
    if input.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(err(
            "ERROR 003917: The feature class geometry is not of type polygon",
        ));
    }
    let wkt = input.spatial_reference_wkt();
    if !wkt.is_some_and(|w| w.trim_start().starts_with("PROJCS[")) {
        return Err(err(
            "Pairwise Polygon Neighbors currently requires a projected coordinate system",
        ));
    }
    let linear_scale = linear_scale(linear_unit, wkt)?;
    let area_scale = if area_overlap {
        crate::registry::area_in_units(&format!("1 {area_unit}"), wkt)?
    } else {
        1.0
    };
    let report = report
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(input.oid_field_name());
    let mut selected = Vec::new();
    let mut fields = Vec::new();
    for name in report.split(';').map(str::trim) {
        let (position, field) = if name.eq_ignore_ascii_case(input.oid_field_name()) {
            (None, integer(input.oid_field_name()))
        } else {
            let index = crate::field_positions(input, &[name.to_owned()])?[0];
            let f = input.fields()[index].clone();
            if !matches!(
                f.field_type,
                FieldType::String | FieldType::SmallInteger | FieldType::Integer
            ) {
                return Err(err(
                    "report fields currently require text or 32-bit integer types",
                ));
            }
            (Some(index), f)
        };
        for prefix in ["src_", "nbr_"] {
            let mut f = field.clone();
            f.name = format!("{prefix}{}", field.name);
            f.alias = None;
            fields.push(f);
        }
        selected.push(position);
    }
    if area_overlap {
        fields.push(Field::new("AREA", FieldType::Double));
    }
    fields.push(Field::new("LENGTH", FieldType::Double));
    fields.push(integer("NODE_COUNT"));
    let mut keys = Vec::new();
    let mut geometries = Vec::new();
    for f in input.features() {
        let f = f?;
        let geometry = f
            .geometry
            .ok_or_else(|| err("null polygon geometry is not yet supported"))?;
        if geometry.has_curves() {
            return Err(err("curved polygon inputs are not yet supported"));
        }
        let mut key = Vec::new();
        for position in &selected {
            key.push(match position {
                Some(i) => Key::from_value(&f.attributes[*i])?,
                None => Key::Integer(
                    i32::try_from(f.oid)
                        .map_err(|_| err("64-bit ObjectIDs are not yet supported"))?,
                ),
            });
        }
        keys.push(key);
        geometries.push(geometry);
    }
    let mut grouped: BTreeMap<(Vec<Key>, Vec<Key>), Contact> = BTreeMap::new();
    let tolerance = crate::default_xy_tolerance(wkt);
    for (i, a) in geometries.iter().enumerate() {
        for (j, b) in geometries.iter().enumerate().skip(i + 1) {
            let (ea, eb) = (a.envelope(), b.envelope());
            if ea.xmin > eb.xmax + tolerance
                || eb.xmin > ea.xmax + tolerance
                || ea.ymin > eb.ymax + tolerance
                || eb.ymin > ea.ymax + tolerance
            {
                continue;
            }
            let c = contact(a, b, area_overlap, tolerance)?;
            if c.area == 0.0 && c.length == 0.0 && c.nodes == 0 {
                continue;
            }
            let (a, b) = (&keys[i], &keys[j]);
            let pairs = if both_sides {
                vec![(a, b), (b, a)]
            } else if a <= b {
                vec![(a, b)]
            } else {
                vec![(b, a)]
            };
            for (a, b) in pairs {
                let total = grouped.entry((a.clone(), b.clone())).or_default();
                total.area += c.area;
                total.length += c.length;
                total.nodes += c.nodes;
            }
        }
    }
    let rows = grouped
        .into_iter()
        .map(|((a, b), total)| {
            let mut row = Vec::new();
            for (a, b) in a.iter().zip(b.iter()) {
                row.push(a.value());
                row.push(b.value());
            }
            if area_overlap {
                row.push(Value::Double(total.area / area_scale));
            }
            let length = if total.area > 0.0 { 0.0 } else { total.length };
            row.push(Value::Double(length / linear_scale));
            row.push(Value::Integer(if total.area > 0.0 || total.length > 0.0 {
                0
            } else {
                total.nodes
            }));
            row
        })
        .collect();
    Ok(TableOutput { fields, rows })
}

pub const TOOL: Tool = Tool {
    id: "pairwise_polygon_neighbors",
    name: "Pairwise Polygon Neighbors",
    toolbox: "Analysis › Proximity",
    summary: "Polygon contiguity statistics by report fields, including optional area overlaps, one-sided relationships and units.",
    params: &[
        Param {
            name: "input",
            label: "Input Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "Projected polygon features.",
        },
        Param {
            name: "fields",
            label: "Report By Fields",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Semicolon-separated text/integer fields; default ObjectID.",
        },
        Param {
            name: "area_overlap",
            label: "Include area overlaps",
            kind: Kind::Choice(&["NO_AREA_OVERLAP", "AREA_OVERLAP"]),
            required: false,
            default: "NO_AREA_OVERLAP",
            help: "",
        },
        Param {
            name: "both_sides",
            label: "Include both sides",
            kind: Kind::Choice(&["BOTH_SIDES", "NO_BOTH_SIDES"]),
            required: false,
            default: "BOTH_SIDES",
            help: "",
        },
        Param {
            name: "cluster_tolerance",
            label: "XY Tolerance",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Custom tolerance is not yet supported.",
        },
        Param {
            name: "out_linear_units",
            label: "Output Linear Units",
            kind: Kind::Choice(&[
                "UNKNOWN",
                "METERS",
                "KILOMETERS",
                "FEET_INTERNATIONAL",
                "FEET",
            ]),
            required: false,
            default: "UNKNOWN",
            help: "",
        },
        Param {
            name: "out_area_units",
            label: "Output Area Units",
            kind: Kind::Text,
            required: false,
            default: "UNKNOWN",
            help: "Common projected area units such as SQUARE_METERS, HECTARES and SQUARE_FEET.",
        },
        Param {
            name: "precision",
            label: "Use maximum precision",
            kind: Kind::Choice(&["DEFAULT_PRECISION", "MAX_PRECISION"]),
            required: false,
            default: "DEFAULT_PRECISION",
            help: "Maximum precision is not yet supported.",
        },
    ],
    output_suffix: "Neighbors",
    measures: false,
    run: Runner::Dynamic(run),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolOutput, tables::Mem};
    use unigis_core::Feature;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Geometry {
        Geometry {
            kind: GeometryKind::Polygon,
            parts: vec![0],
            part_types: None,
            xy: vec![[x, y], [x, y + h], [x + w, y + h], [x + w, y], [x, y]],
            z: None,
            m: None,
            curves: vec![],
        }
    }

    #[test]
    fn aligned_overlaps_match_independent_arcgis_controls() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        for (b, area) in [
            (rect(5.0, 0.0, 10.0, 10.0), 50.0),
            (rect(0.0, 0.0, 5.0, 5.0), 25.0),
            (rect(2.0, 2.0, 2.0, 2.0), 4.0),
        ] {
            let no_area = contact(&a, &b, false, 0.001).unwrap();
            assert_eq!((no_area.area, no_area.length, no_area.nodes), (0.0, 0.0, 0));
            let yes_area = contact(&a, &b, true, 0.001).unwrap();
            assert_eq!(
                (yes_area.area, yes_area.length, yes_area.nodes),
                (area, 0.0, 0)
            );
        }
    }

    #[test]
    fn one_side_uses_report_value_order_not_feature_order() {
        // probe_order.json reverses report keys, independently of feature OIDs.
        let input = Mem(ToolOutput {
            kind: GeometryKind::Polygon,
            has_z: false,
            has_m: false,
            fields: vec![Field::new("NAME", FieldType::String)],
            spatial_reference_wkt: Some("PROJCS[\"probe\",UNIT[\"Meter\",1]]".into()),
            features: vec![
                Feature {
                    oid: 0,
                    geometry: Some(rect(0.0, 0.0, 10.0, 10.0)),
                    attributes: vec![Value::String("Z".into())],
                },
                Feature {
                    oid: 1,
                    geometry: Some(rect(10.0, 0.0, 10.0, 10.0)),
                    attributes: vec![Value::String("A".into())],
                },
            ],
            messages: vec![],
        });
        let output = generate(&input, Some("NAME"), true, false, "UNKNOWN", "UNKNOWN").unwrap();
        assert_eq!(
            output.rows,
            vec![vec![
                Value::String("A".into()),
                Value::String("Z".into()),
                Value::Double(0.0),
                Value::Double(10.0),
                Value::Integer(0)
            ]]
        );
    }
}
