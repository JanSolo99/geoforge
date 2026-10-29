//! Planar Generate Origin-Destination Links, observed against ArcGIS Pro 3.7.
use unigis_core::{
    Error, Feature, FeatureClass, Field, FieldType, Geometry, GeometryKind, Result, Value,
};

use crate::ToolOutput;

fn err(message: &str) -> Error {
    Error::Unsupported(message.into())
}

#[derive(Default)]
pub struct Options<'a> {
    pub groups: Option<(&'a str, &'a str)>,
    pub num_nearest: Option<usize>,
    pub search_distance: Option<f64>,
    pub distance_unit: Option<&'a str>,
    pub same_source: bool,
}

struct Location {
    oid: i32,
    xy: [f64; 2],
    group: Value,
}

fn locations(input: &dyn FeatureClass, group: Option<usize>) -> Result<Vec<Location>> {
    let mut points = Vec::new();
    for feature in input.features() {
        let feature = feature?;
        let Some(geometry) = feature.geometry else {
            continue;
        };
        let Some(xy) = unigis_geometry::centroid(&geometry)? else {
            continue;
        };
        if !xy.iter().all(|v| v.is_finite()) {
            return Err(err("nonfinite link endpoint"));
        }
        points.push(Location {
            oid: i32::try_from(feature.oid)
                .map_err(|_| err("64-bit link ObjectIDs are not supported"))?,
            xy,
            group: group.map_or(Value::Null, |i| feature.attributes[i].clone()),
        });
    }
    Ok(points)
}

fn group_field(input: &dyn FeatureClass, name: &str) -> Result<usize> {
    let index = crate::field_positions(input, &[name.to_owned()])?[0];
    if !matches!(
        input.fields()[index].field_type,
        FieldType::String | FieldType::Integer | FieldType::SmallInteger
    ) {
        return Err(err("group fields currently require text or integer values"));
    }
    Ok(index)
}

/// Generates origin-major links. ArcGIS 3.7's tested PLANAR path limits the
/// destination input sequence, rather than sorting by distance, for num_nearest.
/// Apply group, self and radius filters before the limit. Distinct co-located
/// features retain attributes with a null line, as in the saved oracle outputs.
pub fn generate(
    origins: &dyn FeatureClass,
    destinations: &dyn FeatureClass,
    options: &Options<'_>,
) -> Result<ToolOutput> {
    let wkt = origins.spatial_reference_wkt();
    if !wkt.is_some_and(|s| s.trim_start().starts_with("PROJCS["))
        || wkt != destinations.spatial_reference_wkt()
    {
        return Err(err(
            "planar links currently require the same projected coordinate system on both inputs",
        ));
    }
    if origins.geometry_kind().is_none() || destinations.geometry_kind().is_none() {
        return Err(err("link inputs must contain geometry"));
    }
    if options
        .search_distance
        .is_some_and(|d| !d.is_finite() || d < 0.0)
    {
        return Err(err("search distance must be finite and nonnegative"));
    }
    let unit_scale = options.distance_unit.map_or(Ok(1.0), |unit| {
        crate::registry::distance_in_units(&format!("1 {unit}"), wkt)
    })?;
    let (og, dg) = options
        .groups
        .map(|(a, b)| -> Result<(usize, usize)> {
            Ok((group_field(origins, a)?, group_field(destinations, b)?))
        })
        .transpose()?
        .unzip();
    if let (Some(a), Some(b)) = (og, dg)
        && origins.fields()[a].field_type != destinations.fields()[b].field_type
    {
        return Err(err(
            "origin and destination group fields must have the same type",
        ));
    }
    let orig = locations(origins, og)?;
    let dest = locations(destinations, dg)?;
    let integer = |name: &str, width| {
        let mut f = Field::new(name, FieldType::Integer);
        f.precision = width;
        f
    };
    let mut fields = vec![integer("Id", 6), integer("ORIG_FID", 10)];
    fields.extend(["ORIG_X", "ORIG_Y"].map(|n| Field::new(n, FieldType::Double)));
    fields.push(integer("DEST_FID", 10));
    fields.extend(["DEST_X", "DEST_Y", "LINK_DIST"].map(|n| Field::new(n, FieldType::Double)));
    if let Some(index) = og {
        let kind = origins.fields()[index].field_type;
        let mut f = if kind == FieldType::String {
            Field::new("GROUP_ID", kind)
        } else {
            integer("GROUP_ID", 10)
        };
        if kind == FieldType::String {
            f.length = 254;
        }
        fields.push(f);
    }
    fields.push(integer("COLOR_ID", 10));
    let mut colors: Vec<Value> = Vec::new();
    let mut features = Vec::new();
    for origin in &orig {
        let mut count = 0;
        for destination in &dest {
            if options.same_source && origin.oid == destination.oid {
                continue;
            }
            if og.is_some() && (origin.group == Value::Null || origin.group != destination.group) {
                continue;
            }
            let distance = (destination.xy[0] - origin.xy[0])
                .hypot(destination.xy[1] - origin.xy[1])
                / unit_scale;
            if options
                .search_distance
                .is_some_and(|radius| radius > 0.0 && distance > radius)
            {
                continue;
            }
            if options
                .num_nearest
                .is_some_and(|limit| limit > 0 && count >= limit)
            {
                break;
            }
            count += 1;
            let key = if og.is_some() {
                origin.group.clone()
            } else {
                Value::Integer(origin.oid)
            };
            let color = match colors.iter().position(|g| *g == key) {
                Some(i) => i,
                None => {
                    colors.push(key);
                    colors.len() - 1
                }
            };
            let mut attributes = vec![
                Value::Integer(0),
                Value::Integer(origin.oid),
                Value::Double(origin.xy[0]),
                Value::Double(origin.xy[1]),
                Value::Integer(destination.oid),
                Value::Double(destination.xy[0]),
                Value::Double(destination.xy[1]),
                Value::Double(distance),
            ];
            if og.is_some() {
                attributes.push(origin.group.clone());
            }
            attributes.push(Value::Integer((color % 8 + 1) as i32));
            let geometry = (origin.xy != destination.xy).then(|| Geometry {
                kind: GeometryKind::Polyline,
                parts: vec![0],
                part_types: None,
                xy: vec![origin.xy, destination.xy],
                z: None,
                m: None,
                curves: Vec::new(),
            });
            features.push(Feature {
                oid: features.len() as i64,
                geometry,
                attributes,
            });
        }
    }
    let messages = if features.is_empty() {
        vec!["WARNING 000117: Warning empty output generated.".into()]
    } else {
        Vec::new()
    };
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: wkt.map(str::to_owned),
        features,
        messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Mem;

    fn input(points: &[[f64; 2]], first_oid: i64) -> Mem {
        Mem(ToolOutput {
            kind: GeometryKind::Point,
            has_z: false,
            has_m: false,
            fields: vec![],
            spatial_reference_wkt: Some("PROJCS[\"probe\",UNIT[\"Meter\",1]]".into()),
            features: points
                .iter()
                .enumerate()
                .map(|(i, p)| Feature {
                    oid: first_oid + i as i64,
                    geometry: Some(Geometry::point(p[0], p[1])),
                    attributes: vec![],
                })
                .collect(),
            messages: vec![],
        })
    }

    #[test]
    fn planar_limits_and_colors_match_arcgis_controls() {
        // probe_limits_results.json: twelve origins (OIDs 1..12), four destinations.
        let origins = input(
            &(0..12)
                .map(|i| [500000.0 + i as f64, 5000000.0])
                .collect::<Vec<_>>(),
            1,
        );
        let destinations = input(
            &[
                [500030.0, 5000040.0],
                [500110.0, 5000000.0],
                [500000.0, 5000000.0],
                [500300.0, 5000300.0],
            ],
            0,
        );
        let output = generate(&origins, &destinations, &Options::default()).unwrap();
        assert_eq!(output.features.len(), 48);
        let colors: Vec<_> = output
            .features
            .iter()
            .step_by(4)
            .map(|f| f.attributes.last().unwrap().clone())
            .collect();
        assert_eq!(
            colors,
            [1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4].map(Value::Integer)
        );
        assert!(output.features[2].geometry.is_none());
        assert_eq!(output.features[2].attributes[7], Value::Double(0.0));
        let limited = generate(
            &origins,
            &destinations,
            &Options {
                num_nearest: Some(1),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(limited.features.len(), 12);
        assert!(
            limited
                .features
                .iter()
                .all(|f| f.attributes[4] == Value::Integer(0))
        );
        assert_eq!(limited.features[0].attributes[7], Value::Double(50.0));
        let zero = generate(
            &origins,
            &destinations,
            &Options {
                num_nearest: Some(0),
                search_distance: Some(0.0),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(zero.features.len(), 48);
    }
}
