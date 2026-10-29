//! Calculate Geometry Attributes (Data Management): geometry properties written into existing
//! fields.

use unigis_core::{Error, FeatureClass, FieldType, Result, Value};

use crate::{GeometryProperty as G, ToolOutput};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// A property keyword: the Add Geometry Attributes property and its column, or a count made
/// here.
enum Source {
    Attribute(G, &'static str),
    Holes,
    Curves,
}

fn source(keyword: &str) -> Result<Source> {
    use Source::Attribute as A;
    Ok(match keyword.trim().to_ascii_uppercase().as_str() {
        "AREA" => A(G::Area, "POLY_AREA"),
        "PERIMETER_LENGTH" => A(G::PerimeterLength, "PERIMETER"),
        "LENGTH" => A(G::Length, "LENGTH"),
        "CENTROID_X" => A(G::Centroid, "CENTROID_X"),
        "CENTROID_Y" => A(G::Centroid, "CENTROID_Y"),
        "INSIDE_X" => A(G::CentroidInside, "INSIDE_X"),
        "INSIDE_Y" => A(G::CentroidInside, "INSIDE_Y"),
        "EXTENT_MIN_X" => A(G::Extent, "EXT_MIN_X"),
        "EXTENT_MIN_Y" => A(G::Extent, "EXT_MIN_Y"),
        "EXTENT_MAX_X" => A(G::Extent, "EXT_MAX_X"),
        "EXTENT_MAX_Y" => A(G::Extent, "EXT_MAX_Y"),
        "LINE_START_X" => A(G::LineStartMidEnd, "START_X"),
        "LINE_START_Y" => A(G::LineStartMidEnd, "START_Y"),
        "LINE_END_X" => A(G::LineStartMidEnd, "END_X"),
        "LINE_END_Y" => A(G::LineStartMidEnd, "END_Y"),
        "LINE_BEARING" => A(G::LineBearing, "BEARING"),
        "PART_COUNT" => A(G::PartCount, "PART_COUNT"),
        "POINT_COUNT" => A(G::PointCount, "PNT_COUNT"),
        "POINT_X" => A(G::PointXy, "POINT_X"),
        "POINT_Y" => A(G::PointXy, "POINT_Y"),
        "HOLE_COUNT" => Source::Holes,
        "CURVE_COUNT" => Source::Curves,
        other => {
            return Err(err(format!(
                "geometry property {other} is not supported yet (Z, M, geodesic and notation properties)"
            )));
        }
    })
}

/// The number of holes of a polygon: rings wound counterclockwise (Esri outer rings are
/// clockwise).
fn holes(g: &unigis_core::Geometry) -> usize {
    if g.kind != unigis_core::GeometryKind::Polygon {
        return 0;
    }
    g.part_ranges()
        .filter(|r| {
            let p = &g.xy[r.clone()];
            let twice: f64 = p
                .windows(2)
                .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
                .sum();
            twice > 0.0
        })
        .count()
}

/// Calculate Geometry Attributes: each `(field, property)` writes the property into the
/// existing field, converted to its type (integers rounded); lengths in metres per
/// `length_unit`, areas per `area_unit`, as Add Geometry Attributes computes them.
pub fn calculate_geometry_attributes(
    input: &dyn FeatureClass,
    pairs: &[(String, String)],
    length_unit: f64,
    area_unit: f64,
) -> Result<ToolOutput> {
    let fields = input.fields().to_vec();
    let mut targets = Vec::new();
    let mut properties: Vec<G> = Vec::new();
    for (field, keyword) in pairs {
        let i = fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(field))
            .ok_or_else(|| err(format!("ERROR 000728: Field {field} does not exist")))?;
        let s = source(keyword)?;
        if let Source::Attribute(p, _) = &s
            && !properties.contains(p)
        {
            properties.push(*p);
        }
        targets.push((i, s));
    }
    let computed = crate::add_geometry_attributes(input, &properties, length_unit, area_unit)?;
    let column = |name: &str| computed.fields.iter().rposition(|f| f.name == name);
    let mut features = Vec::new();
    for (f, c) in input.features().zip(&computed.features) {
        let mut f = f?;
        for (i, s) in &targets {
            let value = match s {
                Source::Attribute(_, name) => {
                    column(name).map_or(Value::Null, |k| c.attributes[k].clone())
                }
                Source::Holes => f
                    .geometry
                    .as_ref()
                    .map_or(Value::Null, |g| Value::Double(holes(g) as f64)),
                Source::Curves => f
                    .geometry
                    .as_ref()
                    .map_or(Value::Null, |g| Value::Double(g.curves.len() as f64)),
            };
            let n = crate::number(&value);
            f.attributes[*i] = match (fields[*i].field_type, n) {
                (_, None) => Value::Null,
                (FieldType::SmallInteger, Some(v)) => Value::SmallInteger(v.round() as i16),
                (FieldType::Integer, Some(v)) => Value::Integer(v.round() as i32),
                (FieldType::BigInteger, Some(v)) => Value::BigInteger(v.round() as i64),
                (FieldType::Single, Some(v)) => Value::Single(v as f32),
                (FieldType::String, Some(v)) => Value::String(v.to_string()),
                (_, Some(v)) => Value::Double(v),
            };
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind: input
            .geometry_kind()
            .ok_or_else(|| err("ERROR 000368: Invalid input data: features are required"))?,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}
