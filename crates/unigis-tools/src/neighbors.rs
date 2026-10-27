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

/// Create Thiessen Polygons: the area closer to each point than to any other, inside the extent
/// of the points grown by 4.9945% of its width and height on each side (observed). The polygons
/// are listed in input order (ArcGIS's own order is a spatial one, not reproduced).
fn create_thiessen_polygons(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    if fc.geometry_kind() != Some(GeometryKind::Point) {
        return Err(err("ERROR 000368: Invalid input data: points are required"));
    }
    let all = ctx
        .get("fields_to_copy")
        .is_some_and(|v| v.eq_ignore_ascii_case("ALL"));
    let rows: Vec<Feature> = fc.features().collect::<Result<_>>()?;
    let points: Vec<[f64; 2]> = rows
        .iter()
        .filter_map(|f| f.geometry.as_ref().map(|g| [g.xy[0][0], g.xy[0][1]]))
        .collect();
    if points.len() != rows.len() || points.is_empty() {
        return Err(err(
            "ERROR 000735: Input Features: a feature has no geometry",
        ));
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &points {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
    }
    let grow = 0.049945;
    let (dx, dy) = ((x1 - x0) * grow, (y1 - y0) * grow);
    let frame = vec![
        [x0 - dx, y0 - dy],
        [x0 - dx, y1 + dy],
        [x1 + dx, y1 + dy],
        [x1 + dx, y0 - dy],
    ];
    // A default "Id" field comes first (shapefile output; a copied "Id" becomes "Id_1").
    let mut id = Field::new("Id", FieldType::Integer);
    id.length = 6;
    id.precision = 6;
    let mut fields = vec![id, integer_field("Input_FID")];
    if all {
        for f in fc.fields() {
            let mut f = f.clone();
            if f.name.eq_ignore_ascii_case("Id") {
                f.name = "Id_1".into();
            }
            fields.push(f);
        }
    }
    let mut features = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let mut cell = frame.clone();
        for (j, q) in points.iter().enumerate() {
            if i == j || q == p {
                continue;
            }
            // Keep the side of the bisector that holds `p`: n . x <= c.
            let n = [q[0] - p[0], q[1] - p[1]];
            let c = (n[0] * (p[0] + q[0]) + n[1] * (p[1] + q[1])) / 2.0;
            cell = clip_half_plane(&cell, n, c);
            if cell.len() < 3 {
                break;
            }
        }
        if cell.len() < 3 {
            continue;
        }
        let mut xy = cell.clone();
        xy.push(cell[0]);
        let mut attributes = vec![Value::Integer(0), Value::Integer(rows[i].oid as i32)];
        if all {
            attributes.extend(rows[i].attributes.iter().cloned());
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: unigis_geometry::paths_geometry(vec![xy]).map(|mut g| {
                g.kind = GeometryKind::Polygon;
                g
            }),
            attributes,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// The part of the convex polygon `poly` where `n . x <= c`.
fn clip_half_plane(poly: &[[f64; 2]], n: [f64; 2], c: f64) -> Vec<[f64; 2]> {
    let side = |p: [f64; 2]| n[0] * p[0] + n[1] * p[1] - c;
    let mut out = Vec::with_capacity(poly.len() + 1);
    for k in 0..poly.len() {
        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa <= 0.0 {
            out.push(a);
        }
        if (sa < 0.0 && sb > 0.0) || (sa > 0.0 && sb < 0.0) {
            let t = sa / (sa - sb);
            out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
        }
    }
    out
}

pub const CREATE_THIESSEN_POLYGONS: Tool = Tool {
    id: "create_thiessen_polygons",
    name: "Create Thiessen Polygons",
    toolbox: "Analysis › Proximity",
    summary: "Proximity polygons around points (ArcGIS Pro Create Thiessen Polygons).",
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
            name: "fields_to_copy",
            label: "Output Fields",
            kind: Kind::Choice(&["ONLY_FID", "ALL"]),
            required: false,
            default: "ONLY_FID",
            help: "",
        },
    ],
    output_suffix: "Thiessen",
    measures: true,
    run: Runner::Features(create_thiessen_polygons),
};

/// Douglas-Peucker on one path: keeps the vertex farthest from the chord while it is more than
/// `tol` away (ArcGIS Simplify Line, POINT_REMOVE: the same vertices, observed).
pub(crate) fn douglas_peucker(
    path: &[[f64; 2]],
    tol: f64,
    keep: &mut Vec<bool>,
    lo: usize,
    hi: usize,
) {
    if hi <= lo + 1 {
        return;
    }
    let (a, b) = (path[lo], path[hi]);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy);
    let (mut best, mut at) = (-1.0, lo);
    for (i, p) in path.iter().enumerate().take(hi).skip(lo + 1) {
        let d = if len == 0.0 {
            (p[0] - a[0]).hypot(p[1] - a[1])
        } else {
            (dy * (p[0] - a[0]) - dx * (p[1] - a[1])).abs() / len
        };
        if d > best {
            best = d;
            at = i;
        }
    }
    if best > tol {
        keep[at] = true;
        douglas_peucker(path, tol, keep, lo, at);
        douglas_peucker(path, tol, keep, at, hi);
    }
}

/// Simplify Line with POINT_REMOVE: each part loses the vertices within the tolerance of the
/// line through the kept ones. The output adds InLine_FID, MaxSimpTol and MinSimpTol.
fn simplify_line(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    if fc.geometry_kind() != Some(GeometryKind::Polyline) {
        return Err(err("ERROR 000368: Invalid input data: lines are required"));
    }
    let algorithm = ctx
        .get("algorithm")
        .unwrap_or_else(|| "POINT_REMOVE".into());
    if !algorithm.eq_ignore_ascii_case("POINT_REMOVE") {
        return Err(err(format!("algorithm {algorithm} is not supported yet")));
    }
    let tolerance = crate::registry::distance_in_units(
        &ctx.get("tolerance")
            .ok_or_else(|| err("ERROR 000735: Simplification Tolerance: Value is required"))?,
        fc.spatial_reference_wkt(),
    )?;
    let mut fields = fc.fields().to_vec();
    fields.push(integer_field("InLine_FID"));
    for name in ["MaxSimpTol", "MinSimpTol"] {
        let mut f = Field::new(name, FieldType::Double);
        f.precision = 18;
        f.scale = 11;
        fields.push(f);
    }
    let mut features = Vec::new();
    for f in fc.features() {
        let f = f?;
        let Some(mut g) = f.geometry else { continue };
        let mut xy = Vec::new();
        let mut parts = Vec::new();
        for r in g.part_ranges().collect::<Vec<_>>() {
            let path = &g.xy[r];
            let mut keep = vec![false; path.len()];
            if let Some(first) = keep.first_mut() {
                *first = true;
            }
            if let Some(last) = keep.last_mut() {
                *last = true;
            }
            douglas_peucker(path, tolerance, &mut keep, 0, path.len() - 1);
            parts.push(xy.len() as u32);
            xy.extend(path.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p));
        }
        g.xy = xy;
        g.parts = parts;
        g.z = None;
        g.m = None;
        g.curves = Vec::new();
        let mut attributes = f.attributes.clone();
        attributes.push(Value::Integer(f.oid as i32));
        attributes.push(Value::Double(tolerance));
        attributes.push(Value::Double(tolerance));
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(g),
            attributes,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

pub const SIMPLIFY_LINE: Tool = Tool {
    id: "simplify_line",
    name: "Simplify Line",
    toolbox: "Cartography › Generalization",
    summary: "Removes vertices from lines within a tolerance (ArcGIS Pro Simplify Line, POINT_REMOVE).",
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
            name: "algorithm",
            label: "Simplification Algorithm",
            kind: Kind::Choice(&["POINT_REMOVE", "BEND_SIMPLIFY"]),
            required: false,
            default: "POINT_REMOVE",
            help: "",
        },
        Param {
            name: "tolerance",
            label: "Simplification Tolerance",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "e.g. 5 Meters.",
        },
    ],
    output_suffix: "Simplified",
    measures: true,
    run: Runner::Features(simplify_line),
};

/// Whether the segment a-b has a point in the closed box (Liang-Barsky).
fn segment_meets_box(a: [f64; 2], b: [f64; 2], x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let d = [b[0] - a[0], b[1] - a[1]];
    for (p, q) in [
        (-d[0], a[0] - x0),
        (d[0], x1 - a[0]),
        (-d[1], a[1] - y0),
        (d[1], y1 - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Row letters: A..Z, then AA, AB, ...
fn page_letters(mut n: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (n % 26) as u8);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Grid Index Features: a grid of `width` by `height` cells anchored at the lower-left corner of
/// the input's extent, covering it; cells are named by row (A from the top) and column (1 from
/// the left) and numbered in that order. With INTERSECTFEATURE only the cells touching a
/// feature are kept (and numbered).
fn grid_index_features(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = ctx.features("input")?;
    let wkt = fc.spatial_reference_wkt();
    let size = |name: &str| -> Result<f64> {
        let v = ctx
            .get(name)
            .ok_or_else(|| err(format!("ERROR 000735: {name}: value is required")))?;
        crate::registry::distance_in_units(&v, wkt)
    };
    let (width, height) = (size("polygon_width")?, size("polygon_height")?);
    if !(width > 0.0 && height > 0.0) {
        return Err(err("ERROR 000800: the cell size must be greater than zero"));
    }
    let intersect = ctx
        .get("intersect_feature")
        .is_some_and(|v| v.eq_ignore_ascii_case("INTERSECTFEATURE"));
    let geoms: Vec<unigis_core::Geometry> = fc
        .features()
        .filter_map(|f| f.map(|f| f.geometry).transpose())
        .collect::<Result<_>>()?;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for g in &geoms {
        let e = g.envelope();
        x0 = x0.min(e.xmin);
        y0 = y0.min(e.ymin);
        x1 = x1.max(e.xmax);
        y1 = y1.max(e.ymax);
    }
    if geoms.is_empty() {
        return Err(err("ERROR 000735: Input Features: no features"));
    }
    let cols = (((x1 - x0) / width) - 1e-9).ceil().max(1.0) as usize;
    let rows = (((y1 - y0) / height) - 1e-9).ceil().max(1.0) as usize;
    let mut name_field = Field::new("PageName", FieldType::String);
    name_field.length = 254;
    let mut features = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let (cx0, cx1) = (x0 + c as f64 * width, x0 + (c + 1) as f64 * width);
            let cy1 = y0 + (rows - r) as f64 * height;
            let cy0 = cy1 - height;
            let cell = unigis_geometry::paths_geometry(vec![vec![
                [cx0, cy0],
                [cx0, cy1],
                [cx1, cy1],
                [cx1, cy0],
                [cx0, cy0],
            ]])
            .map(|mut g| {
                g.kind = GeometryKind::Polygon;
                g
            })
            .ok_or_else(|| err("empty cell"))?;
            if intersect {
                let mut touched = false;
                for g in &geoms {
                    let e = g.envelope();
                    if e.xmax < cx0 || e.xmin > cx1 || e.ymax < cy0 || e.ymin > cy1 {
                        continue;
                    }
                    if g.kind == GeometryKind::Point || g.kind == GeometryKind::Multipoint {
                        touched =
                            g.xy.iter()
                                .any(|p| p[0] >= cx0 && p[0] <= cx1 && p[1] >= cy0 && p[1] <= cy1);
                    } else {
                        // Touching the cell's edge or corner counts (observed).
                        touched = g.part_ranges().any(|r| {
                            g.xy[r]
                                .windows(2)
                                .any(|w| segment_meets_box(w[0], w[1], cx0, cy0, cx1, cy1))
                        }) || (g.kind == GeometryKind::Polygon
                            && unigis_geometry::intersection(&cell, g)?.is_some());
                    }
                    if touched {
                        break;
                    }
                }
                if !touched {
                    continue;
                }
            }
            features.push(Feature {
                oid: features.len() as i64,
                geometry: Some(cell),
                attributes: vec![
                    Value::String(format!("{}{}", page_letters(r), c + 1)),
                    Value::Integer(features.len() as i32 + 1),
                ],
            });
        }
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields: vec![name_field, integer_field("PageNumber")],
        spatial_reference_wkt: wkt.map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

pub const GRID_INDEX_FEATURES: Tool = Tool {
    id: "grid_index_features",
    name: "Grid Index Features",
    toolbox: "Cartography › Data Driven Pages",
    summary: "A grid of polygons over the input (ArcGIS Pro Grid Index Features, planar sizes).",
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
            name: "intersect_feature",
            label: "Intersect Feature",
            kind: Kind::Choice(&["NO_INTERSECTFEATURE", "INTERSECTFEATURE"]),
            required: false,
            default: "NO_INTERSECTFEATURE",
            help: "",
        },
        Param {
            name: "polygon_width",
            label: "Polygon Width",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "e.g. 150 Meters.",
        },
        Param {
            name: "polygon_height",
            label: "Polygon Height",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "e.g. 150 Meters.",
        },
    ],
    output_suffix: "Grid",
    measures: true,
    run: Runner::Features(grid_index_features),
};

/// Summarize Within: for each polygon, the number of the summary features (points, lines or
/// polygons) that meet it and statistics of their fields; lines and polygons can add the length
/// or area inside. KEEP_ALL lists the polygons that have features first, then the others, each in
/// input order (observed); ONLY_INTERSECTING drops the latter.
fn summarize_within(ctx: &dyn Context) -> Result<ToolOutput> {
    let polygons = ctx.features("input")?;
    let sums = ctx.features("sum_features")?;
    if polygons.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(err(
            "ERROR 000368: Invalid input data: polygons are required",
        ));
    }
    let kind = sums.geometry_kind();
    let (label, shape_name) = match kind {
        Some(GeometryKind::Point | GeometryKind::Multipoint) => ("Point", None),
        Some(GeometryKind::Polyline) => ("Polyline", Some("Length")),
        Some(GeometryKind::Polygon) => ("Polygon", Some("Area")),
        _ => return Err(err("ERROR 000368: Invalid input data")),
    };
    let keep_all = !ctx
        .get("keep_all")
        .is_some_and(|v| v.eq_ignore_ascii_case("ONLY_INTERSECTING"));
    // (field index, statistic)
    let mut stats: Vec<(usize, String)> = Vec::new();
    for part in ctx.get("summary_fields").unwrap_or_default().split(';') {
        let mut it = part.split_whitespace();
        let (Some(name), Some(stat)) = (it.next(), it.next()) else {
            continue;
        };
        let idx = sums
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| err(format!("ERROR 000728: Field {name} does not exist")))?;
        stats.push((idx, stat.to_ascii_uppercase()));
    }
    let add_shape = ctx
        .get("add_shape_sum")
        .is_some_and(|v| v.eq_ignore_ascii_case("ADD_SHAPE_SUM"))
        && shape_name.is_some();
    if add_shape {
        // ArcGIS measures these sums on the ellipsoid (a UTM line comes out 0.04 % longer than its
        // planar length), which needs the projection engine.
        return Err(err(
            "ADD_SHAPE_SUM needs geodesic lengths and areas, which are not implemented yet",
        ));
    }
    let (unit, factor) = (String::new(), 1.0);

    let sum_rows: Vec<Feature> = sums.features().collect::<Result<_>>()?;
    let value_of = |v: &Value| match v {
        Value::SmallInteger(n) => Some(f64::from(*n)),
        Value::Integer(n) => Some(f64::from(*n)),
        Value::BigInteger(n) => Some(*n as f64),
        Value::Single(n) => Some(f64::from(*n)),
        Value::Double(n) => Some(*n),
        _ => None,
    };

    let double = |name: String| {
        let mut f = Field::new(name, FieldType::Double);
        f.precision = 18;
        f.scale = 11;
        f
    };
    let mut fields = polygons.fields().to_vec();
    for (idx, stat) in &stats {
        fields.push(double(format!(
            "{}_{}",
            stat.to_ascii_lowercase(),
            sums.fields()[*idx].name
        )));
    }
    if add_shape {
        fields.push(double(format!(
            "sum_{}_{}",
            shape_name.unwrap_or_default(),
            unit.to_ascii_uppercase()
        )));
    }
    fields.push(integer_field(&format!("{label}_Count")));

    let mut hit: Vec<Feature> = Vec::new();
    let mut miss: Vec<Feature> = Vec::new();
    for poly in polygons.features() {
        let poly = poly?;
        let Some(pg) = &poly.geometry else { continue };
        let mut members = Vec::new();
        let mut shape_total = 0.0;
        for s in &sum_rows {
            let Some(sg) = &s.geometry else { continue };
            let Some(part) = unigis_geometry::intersection(sg, pg)? else {
                continue;
            };
            members.push(s);
            if add_shape {
                shape_total += if kind == Some(GeometryKind::Polygon) {
                    unigis_geometry::area(&part)?
                } else {
                    unigis_geometry::length(&part)?
                };
            }
        }
        let mut attributes = poly.attributes.clone();
        for (idx, stat) in &stats {
            let vals: Vec<f64> = members
                .iter()
                .filter_map(|m| value_of(&m.attributes[*idx]))
                .collect();
            let n = vals.len() as f64;
            let sum: f64 = vals.iter().sum();
            let v = match stat.as_str() {
                "SUM" => sum,
                "MEAN" if n > 0.0 => sum / n,
                "MIN" => vals.iter().copied().fold(f64::INFINITY, f64::min),
                "MAX" => vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                "COUNT" => n,
                "RANGE" => {
                    vals.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                        - vals.iter().copied().fold(f64::INFINITY, f64::min)
                }
                "STD" if n > 1.0 => {
                    let mean = sum / n;
                    (vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
                }
                _ => 0.0,
            };
            attributes.push(Value::Double(if v.is_finite() { v } else { 0.0 }));
        }
        if add_shape {
            attributes.push(Value::Double(shape_total * factor));
        }
        attributes.push(Value::Integer(members.len() as i32));
        let row = Feature {
            oid: 0,
            geometry: poly.geometry.clone(),
            attributes,
        };
        if members.is_empty() {
            miss.push(row);
        } else {
            hit.push(row);
        }
    }
    if keep_all {
        hit.extend(miss);
    }
    for (i, f) in hit.iter_mut().enumerate() {
        f.oid = i as i64;
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: polygons.spatial_reference_wkt().map(str::to_owned),
        features: hit,
        messages: Vec::new(),
    })
}

pub const SUMMARIZE_WITHIN: Tool = Tool {
    id: "summarize_within",
    name: "Summarize Within",
    toolbox: "Analysis › Summary",
    summary: "Counts and summarizes the features inside each polygon (ArcGIS Pro Summarize Within, planar).",
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
            name: "sum_features",
            label: "Input Summary Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "",
        },
        Param {
            name: "keep_all",
            label: "Keep All Polygons",
            kind: Kind::Choice(&["KEEP_ALL", "ONLY_INTERSECTING"]),
            required: false,
            default: "KEEP_ALL",
            help: "",
        },
        Param {
            name: "summary_fields",
            label: "Summary Fields",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "`Field Statistic;Field Statistic`.",
        },
        Param {
            name: "add_shape_sum",
            label: "Add Shape Summary Attributes",
            kind: Kind::Choice(&["NO_SHAPE_SUM", "ADD_SHAPE_SUM"]),
            required: false,
            default: "NO_SHAPE_SUM",
            help: "",
        },
        Param {
            name: "shape_unit",
            label: "Shape Unit",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "e.g. Meters, Square meters.",
        },
    ],
    output_suffix: "Summary",
    measures: true,
    run: Runner::Features(summarize_within),
};

/// Summarize Nearby (STRAIGHT_LINE, point summary features): each input feature buffered by each
/// distance, the buffer being the output geometry in feature-major order, with the number of
/// summary features that meet it. With several distances each output is the ring between a
/// buffer and the previous one and `BUFF_DIST` leads the fields; with one it follows the input's
/// (observed). Features without any summary feature are left out unless KEEP_ALL.
fn summarize_nearby(ctx: &dyn Context) -> Result<ToolOutput> {
    let input = ctx.features("input")?;
    let sums = ctx.features("sum_features")?;
    let near = ctx
        .get("near_type")
        .unwrap_or_else(|| "STRAIGHT_LINE".into());
    if !near.eq_ignore_ascii_case("STRAIGHT_LINE") {
        return Err(err(
            "only STRAIGHT_LINE is supported (network analysis is not)",
        ));
    }
    if ctx
        .get("add_shape_sum")
        .is_some_and(|v| v.eq_ignore_ascii_case("ADD_SHAPE_SUM"))
    {
        return Err(err(
            "ADD_SHAPE_SUM needs geodesic lengths and areas, which are not implemented yet",
        ));
    }
    if !matches!(
        sums.geometry_kind(),
        Some(GeometryKind::Point | GeometryKind::Multipoint)
    ) {
        // ArcGIS adds a geodesic length / area sum for lines and polygons.
        return Err(err(
            "line and polygon summary features need geodesic shape sums, which are not implemented yet",
        ));
    }
    let keep_all = !ctx
        .get("keep_all")
        .is_some_and(|v| v.eq_ignore_ascii_case("ONLY_INTERSECTING"));
    let unit = ctx.get("distance_unit").unwrap_or_else(|| "Meters".into());
    let wkt = input.spatial_reference_wkt();
    let mut distances = Vec::new();
    for d in ctx.get("distances").unwrap_or_default().split(';') {
        let d = d.trim();
        if d.is_empty() {
            continue;
        }
        let shown: f64 = d
            .parse()
            .map_err(|_| err(format!("ERROR 000094: '{d}' is not a distance")))?;
        distances.push((
            shown,
            crate::registry::distance_in_units(&format!("{d} {unit}"), wkt)?,
        ));
    }
    if distances.is_empty() {
        return Err(err("ERROR 000735: Distances: Value is required"));
    }
    let sum_geoms: Vec<unigis_core::Geometry> = sums
        .features()
        .filter_map(|f| f.map(|f| f.geometry).transpose())
        .collect::<Result<_>>()?;

    let mut dist_field = Field::new("BUFF_DIST", FieldType::Double);
    dist_field.precision = 18;
    dist_field.scale = 11;
    let multi = distances.len() > 1;
    let mut fields = Vec::new();
    if multi {
        fields.push(dist_field.clone());
    }
    fields.extend(input.fields().iter().cloned());
    if !multi {
        fields.push(dist_field);
    }
    fields.push(integer_field("Point_Count"));

    let rows: Vec<Feature> = input.features().collect::<Result<_>>()?;
    let mut per_distance = Vec::new();
    for (_, d) in &distances {
        per_distance.push(crate::buffer(
            input.as_ref(),
            crate::BufferDistance::Linear(*d),
            unigis_geometry::LineEnd::Round,
            unigis_geometry::LineSide::Full,
            crate::Dissolve::None,
        )?);
    }
    // Each distance after the first gives the ring between its buffer and the previous one.
    for k in (1..per_distance.len()).rev() {
        for i in 0..per_distance[k].features.len() {
            let inner = per_distance[k - 1].features[i].geometry.clone();
            let outer = per_distance[k].features[i].geometry.clone();
            if let (Some(outer), Some(inner)) = (outer, inner) {
                let area = unigis_geometry::ClipArea::new([&inner])?;
                per_distance[k].features[i].geometry = unigis_geometry::erase(&outer, &area)?;
            }
        }
    }
    let mut hit: Vec<Feature> = Vec::new();
    let mut miss: Vec<Feature> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if row.geometry.is_none() {
            continue;
        }
        for (k, (shown, _)) in distances.iter().enumerate() {
            let Some(b) = per_distance[k].features.get(i) else {
                continue;
            };
            // Counted against the drawn buffer (ArcGIS does the same; exact distances give a
            // different count for a point near the arcs).
            let Some(bg) = &b.geometry else { continue };
            let mut count = 0;
            for g in &sum_geoms {
                if unigis_geometry::intersection(g, bg)?.is_some() {
                    count += 1;
                }
            }
            let mut attributes = Vec::new();
            if multi {
                attributes.push(Value::Double(*shown));
            }
            attributes.extend(row.attributes.iter().cloned());
            if !multi {
                attributes.push(Value::Double(*shown));
            }
            attributes.push(Value::Integer(count));
            let feature = Feature {
                oid: 0,
                geometry: b.geometry.clone(),
                attributes,
            };
            if count > 0 {
                hit.push(feature);
            } else {
                miss.push(feature);
            }
        }
    }
    // Features with summary features come first, then the others (observed, as in Summarize
    // Within); ONLY_INTERSECTING drops the others.
    let mut features = hit;
    if keep_all {
        features.extend(miss);
    }
    for (i, f) in features.iter_mut().enumerate() {
        f.oid = i as i64;
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: wkt.map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

pub const SUMMARIZE_NEARBY: Tool = Tool {
    id: "summarize_nearby",
    name: "Summarize Nearby",
    toolbox: "Analysis › Summary",
    summary: "Buffers features and counts the point features that meet each buffer (ArcGIS Pro Summarize Nearby, straight line).",
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
            name: "sum_features",
            label: "Input Summary Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "",
        },
        Param {
            name: "near_type",
            label: "Nearby Type",
            kind: Kind::Choice(&["STRAIGHT_LINE"]),
            required: false,
            default: "STRAIGHT_LINE",
            help: "",
        },
        Param {
            name: "distances",
            label: "Distances",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated, e.g. `50;150`.",
        },
        Param {
            name: "distance_unit",
            label: "Distance Units",
            kind: Kind::Text,
            required: false,
            default: "Meters",
            help: "",
        },
        Param {
            name: "keep_all",
            label: "Keep All Input Features",
            kind: Kind::Choice(&["KEEP_ALL", "ONLY_INTERSECTING"]),
            required: false,
            default: "KEEP_ALL",
            help: "",
        },
        Param {
            name: "add_shape_sum",
            label: "Add Shape Summary Attributes",
            kind: Kind::Choice(&["NO_SHAPE_SUM", "ADD_SHAPE_SUM"]),
            required: false,
            default: "NO_SHAPE_SUM",
            help: "",
        },
    ],
    output_suffix: "Nearby",
    measures: true,
    run: Runner::Features(summarize_nearby),
};
