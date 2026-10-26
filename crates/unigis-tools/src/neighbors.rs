//! Polygon Neighbors and Count Overlapping Features (registry entries).

use unigis_core::{Error, Feature, Field, FieldType, GeometryKind, Result, Value};

use crate::registry::{Context, Kind, Outcome, Param, Runner, Tool};
use crate::{TableOutput, ToolOutput};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// A 10-digit integer field (what ArcGIS reports as 64-bit in a shapefile).
fn integer_field(name: &str) -> Field {
    let mut f = Field::new(name, FieldType::Integer);
    f.length = 10;
    f.precision = 10;
    f
}

/// Polygon Neighbors: for every pair of polygons that share a boundary stretch, a node, or (with
/// `AREA_OVERLAP`) area, one row per direction with the chosen fields of both and the length of
/// the shared boundary, the number of shared nodes and the overlap area.
fn polygon_neighbors(ctx: &dyn Context) -> Result<Outcome> {
    let fc = ctx.features("input")?;
    if fc.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(err(
            "ERROR 000368: Invalid input data: polygons are required",
        ));
    }
    let side = ctx.get("both_sides").unwrap_or_else(|| "BOTH_SIDES".into());
    if !side.eq_ignore_ascii_case("BOTH_SIDES") {
        return Err(err("only BOTH_SIDES is supported"));
    }
    let area_overlap = ctx
        .get("area_overlap")
        .is_some_and(|v| v.eq_ignore_ascii_case("AREA_OVERLAP"));
    let schema = fc.fields();
    let chosen: Vec<usize> = ctx
        .get("fields")
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|name| {
            schema
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| err(format!("ERROR 000728: Field {name} does not exist")))
        })
        .collect::<Result<_>>()?;
    let rows: Vec<Feature> = fc.features().collect::<Result<_>>()?;
    let geoms: Vec<_> = rows
        .iter()
        .map(|f| {
            f.geometry
                .as_ref()
                .ok_or_else(|| err("a feature has no geometry"))
        })
        .collect::<Result<_>>()?;

    let mut fields = Vec::new();
    for &i in &chosen {
        for prefix in ["src_", "nbr_"] {
            let mut f = schema[i].clone();
            f.name = format!("{prefix}{}", schema[i].name);
            f.alias = None;
            fields.push(f);
        }
    }
    if area_overlap {
        let mut f = Field::new("AREA", FieldType::Double);
        f.precision = 18;
        f.scale = 11;
        fields.push(f);
    }
    let mut length_field = Field::new("LENGTH", FieldType::Double);
    length_field.precision = 18;
    length_field.scale = 11;
    fields.push(length_field);
    fields.push(integer_field("NODE_COUNT"));

    let mut out = Vec::new();
    for (i, a) in geoms.iter().enumerate() {
        for (j, b) in geoms.iter().enumerate() {
            if i == j {
                continue;
            }
            let (ea, eb) = (a.envelope(), b.envelope());
            if ea.xmin > eb.xmax || eb.xmin > ea.xmax || ea.ymin > eb.ymax || eb.ymin > ea.ymax {
                continue;
            }
            let meet = unigis_geometry::meeting(a, b, unigis_geometry::XY_TOLERANCE)?;
            let overlap = if area_overlap {
                match unigis_geometry::intersection(a, b)? {
                    Some(g) if g.kind == GeometryKind::Polygon => unigis_geometry::area(&g)?,
                    _ => 0.0,
                }
            } else {
                0.0
            };
            let length: f64 = meet
                .shared
                .iter()
                .map(|p| {
                    p.windows(2)
                        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                        .sum::<f64>()
                })
                .sum();
            if meet.shared.is_empty() && meet.points.is_empty() && overlap == 0.0 {
                continue;
            }
            let mut row = Vec::new();
            for &c in &chosen {
                row.push(rows[i].attributes[c].clone());
                row.push(rows[j].attributes[c].clone());
            }
            if area_overlap {
                row.push(Value::Double(overlap));
            }
            row.push(Value::Double(length));
            row.push(Value::Integer(meet.points.len() as i32));
            out.push(row);
        }
    }
    Ok(Outcome::Table(TableOutput { fields, rows: out }))
}

pub const POLYGON_NEIGHBORS: Tool = Tool {
    id: "polygon_neighbors",
    name: "Polygon Neighbors",
    toolbox: "Analysis › Statistics",
    summary: "Table of the polygons that touch or overlap each other (ArcGIS Pro Polygon Neighbors).",
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
            name: "fields",
            label: "Input Fields",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated fields to report for both polygons.",
        },
        Param {
            name: "area_overlap",
            label: "Area Overlap",
            kind: Kind::Choice(&["NO_AREA_OVERLAP", "AREA_OVERLAP"]),
            required: false,
            default: "NO_AREA_OVERLAP",
            help: "",
        },
        Param {
            name: "both_sides",
            label: "Both Sides",
            kind: Kind::Choice(&["BOTH_SIDES", "LEFT_SIDE"]),
            required: false,
            default: "BOTH_SIDES",
            help: "",
        },
    ],
    output_suffix: "Neighbors",
    measures: true,
    run: Runner::Dynamic(polygon_neighbors),
};

/// Count Overlapping Features: the areas covered by the same set of features, with the number
/// of features and of input datasets covering them.
fn count_overlapping(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    if fc.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(err("ERROR 000468: Input shape types are not equal"));
    }
    let minimum: usize = ctx
        .get("minimum_overlap_count")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1);
    let input: Vec<(i64, unigis_core::Geometry)> = fc
        .features()
        .filter_map(|f| match f {
            Ok(f) => f.geometry.map(|g| Ok((f.oid, g))),
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<_>>()?;
    let mut features = Vec::new();
    for cell in unigis_geometry::polygon_overlay(&[input])? {
        let count = cell.members[0].len();
        let Some(geometry) = unigis_geometry::cell_geometry(&cell) else {
            continue;
        };
        if count < minimum.max(1) {
            continue;
        }
        features.push(Feature {
            oid: 0,
            geometry: Some(geometry),
            attributes: vec![Value::Integer(count as i32), Value::Integer(1)],
        });
    }
    // Least overlap first (observed).
    features.sort_by_key(|f| match f.attributes[0] {
        Value::Integer(n) => n,
        _ => 0,
    });
    for (i, f) in features.iter_mut().enumerate() {
        f.oid = i as i64;
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields: vec![integer_field("COUNT_"), integer_field("COUNT_FC")],
        spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

pub const COUNT_OVERLAPPING_FEATURES: Tool = Tool {
    id: "count_overlapping_features",
    name: "Count Overlapping Features",
    toolbox: "Analysis › Overlay",
    summary: "Areas covered by the same set of polygons, with the overlap count (ArcGIS Pro Count Overlapping Features).",
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
            name: "minimum_overlap_count",
            label: "Minimum Overlap Count",
            kind: Kind::Number,
            required: false,
            default: "1",
            help: "",
        },
    ],
    output_suffix: "Overlap",
    measures: true,
    run: Runner::Features(count_overlapping),
};

/// Subdivide Polygon: each polygon cut into strips of equal area by lines parallel to the X
/// axis, from the bottom up, whatever the shape (observed); the strips keep the polygon's
/// attributes.
fn subdivide_polygon(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    if fc.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(err(
            "ERROR 000368: Invalid input data: polygons are required",
        ));
    }
    let method = ctx
        .get("method")
        .unwrap_or_else(|| "NUMBER_OF_EQUAL_PARTS".into());
    let (parts, target) = if method.eq_ignore_ascii_case("NUMBER_OF_EQUAL_PARTS") {
        let n: usize = ctx
            .get("number_of_areas")
            .and_then(|v| v.trim().parse().ok())
            .ok_or_else(|| err("ERROR 000735: Number of Areas: Value is required"))?;
        (Some(n), None)
    } else if method.eq_ignore_ascii_case("EQUAL_AREAS") {
        let a: f64 = ctx
            .get("target_area")
            .and_then(|v| v.trim().parse().ok())
            .ok_or_else(|| err("ERROR 000735: Target Area: Value is required"))?;
        (None, Some(a))
    } else {
        return Err(err(format!(
            "subdivision method {method} is not supported yet"
        )));
    };
    let mut features = Vec::new();
    for f in fc.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        for piece in unigis_geometry::horizontal_strips(g, parts, target)? {
            features.push(Feature {
                oid: features.len() as i64,
                geometry: Some(piece),
                attributes: f.attributes.clone(),
            });
        }
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields: fc.fields().to_vec(),
        spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

pub const SUBDIVIDE_POLYGON: Tool = Tool {
    id: "subdivide_polygon",
    name: "Subdivide Polygon",
    toolbox: "Data Management › Features",
    summary: "Cuts polygons into strips of equal area (ArcGIS Pro Subdivide Polygon).",
    params: &[
        Param {
            name: "input",
            label: "Input Polygons",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "",
        },
        Param {
            name: "method",
            label: "Subdivision Method",
            kind: Kind::Choice(&["NUMBER_OF_EQUAL_PARTS", "EQUAL_AREAS"]),
            required: false,
            default: "NUMBER_OF_EQUAL_PARTS",
            help: "",
        },
        Param {
            name: "number_of_areas",
            label: "Number of Areas",
            kind: Kind::Number,
            required: false,
            default: "",
            help: "",
        },
        Param {
            name: "target_area",
            label: "Target Area",
            kind: Kind::Number,
            required: false,
            default: "",
            help: "",
        },
    ],
    output_suffix: "Subdivided",
    measures: true,
    run: Runner::Features(subdivide_polygon),
};

/// Orders attribute values for grouping: numbers by value, text alphabetically, nulls first.
fn value_order(a: &Value, b: &Value) -> std::cmp::Ordering {
    fn num(v: &Value) -> Option<f64> {
        Some(match v {
            Value::SmallInteger(n) => f64::from(*n),
            Value::Integer(n) => f64::from(*n),
            Value::BigInteger(n) => *n as f64,
            Value::Single(n) => f64::from(*n),
            Value::Double(n) => *n,
            _ => return None,
        })
    }
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        _ => match (a, b) {
            (Value::String(x), Value::String(y)) => x.cmp(y),
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        },
    }
}

/// The features of `fc` grouped by the values of `fields` (indices into its schema), dissolved to
/// one geometry per group, in the order of the values.
fn dissolved_groups(
    fc: &dyn unigis_core::FeatureClass,
    fields: &[usize],
) -> Result<Vec<(Vec<Value>, unigis_core::Geometry)>> {
    let mut groups: Vec<(Vec<Value>, Vec<unigis_core::Geometry>)> = Vec::new();
    for f in fc.features() {
        let f = f?;
        let Some(g) = f.geometry else { continue };
        let key: Vec<Value> = fields.iter().map(|&i| f.attributes[i].clone()).collect();
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, gs)) => gs.push(g),
            None => groups.push((key, vec![g])),
        }
    }
    groups.sort_by(|a, b| {
        a.0.iter()
            .zip(&b.0)
            .map(|(x, y)| value_order(x, y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    groups
        .into_iter()
        .filter_map(|(k, gs)| match unigis_geometry::dissolve(gs.iter()) {
            Ok(Some(g)) => Some(Ok((k, g))),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
        .collect()
}

fn field_indices(fc: &dyn unigis_core::FeatureClass, list: &str) -> Result<Vec<usize>> {
    list.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|name| {
            fc.fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| err(format!("ERROR 000728: Field {name} does not exist")))
        })
        .collect()
}

/// Units per metre (`area` for square units) of an output unit keyword.
fn unit_factor(unit: &str, area: bool) -> Result<f64> {
    let unit = unit.trim().to_ascii_uppercase();
    let unit = unit.strip_prefix("SQUARE_").unwrap_or(&unit);
    let linear = match unit {
        "" | "METERS" | "METRES" => 1.0,
        "KILOMETERS" => 0.001,
        "FEET" => 1.0 / 0.3048,
        "MILES" => 1.0 / 1609.344,
        "HECTARES" if area => 1e-4_f64.sqrt(),
        "ACRES" if area => (1.0 / 4046.8564224_f64).sqrt(),
        _ => return Err(err(format!("ERROR 000094: unknown output unit {unit}"))),
    };
    Ok(if area { linear * linear } else { linear })
}

/// Tabulate Intersection: for each group of zone features (by `zone_fields`) and class group (by
/// `class_fields`), the area (or length, for line classes) of the class inside the zone, and its
/// percentage of the zone's area (for lines, of the class's total length; observed).
fn tabulate_intersection(ctx: &dyn Context) -> Result<Outcome> {
    let zones = ctx.features("input")?;
    let classes = ctx.features("class_features")?;
    if zones.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(err(
            "ERROR 000368: Invalid input data: zones must be polygons",
        ));
    }
    let class_kind = classes.geometry_kind();
    let zone_idx = field_indices(zones.as_ref(), &ctx.get("zone_fields").unwrap_or_default())?;
    let class_idx = field_indices(
        classes.as_ref(),
        &ctx.get("class_fields").unwrap_or_default(),
    )?;
    let lines = class_kind == Some(GeometryKind::Polyline);
    if !lines && class_kind != Some(GeometryKind::Polygon) {
        return Err(err(
            "ERROR 000368: Invalid input data: classes must be lines or polygons",
        ));
    }
    let factor = unit_factor(&ctx.get("output_units").unwrap_or_default(), !lines)?;

    let measure = |g: &unigis_core::Geometry| -> Result<f64> {
        if lines {
            unigis_geometry::length(g)
        } else {
            unigis_geometry::area(g)
        }
    };
    // Class features are not dissolved: overlapping ones count twice (observed).
    let mut class_groups: Vec<(Vec<Value>, Vec<unigis_core::Geometry>)> = Vec::new();
    for f in classes.features() {
        let f = f?;
        let Some(g) = f.geometry else { continue };
        let key: Vec<Value> = class_idx.iter().map(|&i| f.attributes[i].clone()).collect();
        match class_groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, gs)) => gs.push(g),
            None => class_groups.push((key, vec![g])),
        }
    }
    class_groups.sort_by(|a, b| {
        a.0.iter()
            .zip(&b.0)
            .map(|(x, y)| value_order(x, y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let class_total: f64 = {
        let mut t = 0.0;
        for f in classes.features() {
            if let Some(g) = f?.geometry {
                t += measure(&g)?;
            }
        }
        t
    };
    let mut fields = Vec::new();
    for &i in &zone_idx {
        fields.push(zones.fields()[i].clone());
    }
    for &i in &class_idx {
        fields.push(classes.fields()[i].clone());
    }
    let mut value_field = Field::new(if lines { "LENGTH" } else { "AREA" }, FieldType::Double);
    value_field.precision = 18;
    value_field.scale = 11;
    let mut percent_field = Field::new("PERCENTAGE", FieldType::Double);
    percent_field.precision = 18;
    percent_field.scale = 11;
    fields.push(value_field);
    fields.push(percent_field);

    let mut rows = Vec::new();
    for (zone_key, zone) in dissolved_groups(zones.as_ref(), &zone_idx)? {
        let denominator = if lines {
            class_total
        } else {
            unigis_geometry::area(&zone)?
        };
        for (class_key, class) in &class_groups {
            let mut amount = 0.0;
            for g in class {
                if let Some(part) = unigis_geometry::intersection(g, &zone)? {
                    amount += measure(&part)?;
                }
            }
            if amount <= 0.0 {
                continue;
            }
            let mut row = zone_key.clone();
            row.extend(class_key.iter().cloned());
            row.push(Value::Double(amount * factor));
            row.push(Value::Double(amount / denominator * 100.0));
            rows.push(row);
        }
    }
    Ok(Outcome::Table(TableOutput { fields, rows }))
}

pub const TABULATE_INTERSECTION: Tool = Tool {
    id: "tabulate_intersection",
    name: "Tabulate Intersection",
    toolbox: "Analysis › Statistics",
    summary: "Area or length of class features inside each zone (ArcGIS Pro Tabulate Intersection).",
    params: &[
        Param {
            name: "input",
            label: "Input Zone Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "",
        },
        Param {
            name: "zone_fields",
            label: "Zone Fields",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Semicolon-separated.",
        },
        Param {
            name: "class_features",
            label: "Input Class Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "",
        },
        Param {
            name: "class_fields",
            label: "Class Fields",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Semicolon-separated.",
        },
        Param {
            name: "output_units",
            label: "Output Units",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "e.g. SQUARE_METERS or METERS.",
        },
    ],
    output_suffix: "Tabulate",
    measures: true,
    run: Runner::Dynamic(tabulate_intersection),
};
