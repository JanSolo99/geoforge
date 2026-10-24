//! Data Management geometry tools: Feature Envelope To Polygon, Split Line At Vertices, Split Line
//! At Point, Eliminate Polygon Part and XY To Line.
//!
//! Matched to ArcGIS Pro 3.7 (`testdata/arcgis-reference/manifest.json`, cases `env_*`, `slv_*`,
//! `sap_*`, `epp_*`, `xy2l_*`):
//! - Feature Envelope To Polygon (a part without width or height gives no polygon) writes the envelope ring clockwise from the lower left corner
//!   (xmin,ymin; xmin,ymax; xmax,ymax; xmax,ymin) with `ORIG_FID`; `MULTIPART` gives one ring per
//!   part (the exterior ring's envelope for a polygon part), `SINGLEPART` one envelope per feature.
//! - Split Line At Vertices writes each segment as a feature with `ORIG_FID` (the input's FID) and
//!   `ORIG_SEQ` (1, 2, ... along the feature, continuing over its parts).
//! - Split Line At Point splits at the nearest location of every point within the search radius
//!   (planar distance); a location at a part's end is no split, one at a vertex is. Pieces are
//!   numbered in `ORIG_SEQ` like the vertex segments. Without a search radius ArcGIS's result is
//!   not understood: the radius is required here.
//! - Eliminate Polygon Part removes holes (`CONTAINED_ONLY`) or any part (`ANY`) smaller than the
//!   area and/or the percent. A part's or hole's percent is of the total of the feature's exterior
//!   rings; a polygon keeps its largest part.
//! - XY To Line draws a straight line from the start to the end coordinates of each row. Rows whose
//!   ends coincide are skipped with WARNING 000072. `ATTRIBUTES` keeps every field and adds
//!   `ORIG_FID`; otherwise the four coordinate fields and the ID field are kept.

use unigis_core::{
    Error, Feature, FeatureClass, Field, FieldType, Geometry, GeometryKind, Result, Value,
    geometry::ring_signed_area2,
};

use crate::{ToolOutput, with_orig_fid, with_value};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

fn kind_of(input: &dyn FeatureClass) -> Result<GeometryKind> {
    input
        .geometry_kind()
        .ok_or_else(|| err("ERROR 000735: Input Features: Value is required"))
}

fn new_output(
    input: &dyn FeatureClass,
    kind: GeometryKind,
    fields: Vec<Field>,
    features: Vec<Feature>,
) -> ToolOutput {
    ToolOutput {
        kind,
        has_z: input.has_z() && kind != GeometryKind::Polygon,
        has_m: input.has_m() && kind != GeometryKind::Polygon,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    }
}

/// The parts of a polygon as (exterior ring range, hole ranges): a clockwise ring starts a part,
/// a counter-clockwise ring is a hole of the part that contains it.
fn polygon_parts(g: &Geometry) -> Vec<(std::ops::Range<usize>, Vec<std::ops::Range<usize>>)> {
    let mut parts: Vec<(std::ops::Range<usize>, Vec<std::ops::Range<usize>>)> = Vec::new();
    for r in g.part_ranges() {
        if ring_signed_area2(&g.xy[r.clone()]) <= 0.0 || parts.is_empty() {
            parts.push((r, Vec::new()));
        } else {
            let probe = g.xy[r.start];
            let owner = parts
                .iter()
                .rposition(|(e, _)| unigis_core::geometry::point_in_ring(probe, &g.xy[e.clone()]))
                .unwrap_or(parts.len() - 1);
            parts[owner].1.push(r);
        }
    }
    parts
}

fn ring_area(g: &Geometry, r: &std::ops::Range<usize>) -> f64 {
    ring_signed_area2(&g.xy[r.clone()]).abs() / 2.0
}

fn rect(env: [f64; 4]) -> Geometry {
    let [x0, y0, x1, y1] = env;
    Geometry {
        kind: GeometryKind::Polygon,
        parts: vec![0],
        part_types: None,
        xy: vec![[x0, y0], [x0, y1], [x1, y1], [x1, y0], [x0, y0]],
        z: None,
        m: None,
        curves: Vec::new(),
    }
}

fn bounds(points: &[[f64; 2]]) -> Option<[f64; 4]> {
    let mut it = points.iter();
    let first = it.next()?;
    let mut b = [first[0], first[1], first[0], first[1]];
    for p in it {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    Some(b)
}

/// `arcpy.management.FeatureEnvelopeToPolygon(in_features, out_feature_class, single_envelope)`.
pub fn feature_envelope_to_polygon(
    input: &dyn FeatureClass,
    multipart: bool,
) -> Result<ToolOutput> {
    let kind = kind_of(input)?;
    if kind == GeometryKind::Point {
        return Err(err(
            "ERROR 003917: The feature class geometry is not of type multipoint | polyline | polygon",
        ));
    }
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let mut shape = Geometry {
            kind: GeometryKind::Polygon,
            parts: Vec::new(),
            part_types: None,
            xy: Vec::new(),
            z: None,
            m: None,
            curves: Vec::new(),
        };
        let mut envelopes: Vec<[f64; 4]> = Vec::new();
        if multipart && kind == GeometryKind::Polyline {
            envelopes.extend(g.part_ranges().filter_map(|r| bounds(&g.xy[r])));
        } else if multipart && kind == GeometryKind::Polygon {
            envelopes.extend(
                polygon_parts(g)
                    .into_iter()
                    .filter_map(|(e, _)| bounds(&g.xy[e])),
            );
        } else if let Some(b) = bounds(&g.xy) {
            envelopes.push(b);
        }
        // A part with no width or no height has no envelope polygon.
        envelopes.retain(|b| b[0] < b[2] && b[1] < b[3]);
        for b in envelopes {
            shape.parts.push(shape.xy.len() as u32);
            shape.xy.extend(rect(b).xy);
        }
        if shape.xy.is_empty() {
            continue;
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(shape),
            attributes: with_value(&f.attributes, [Value::Integer(f.oid as i32)]),
        });
    }
    Ok(new_output(
        input,
        GeometryKind::Polygon,
        with_orig_fid(input),
        features,
    ))
}

fn require_plain_lines(input: &dyn FeatureClass) -> Result<()> {
    if kind_of(input)? != GeometryKind::Polyline {
        return Err(err(
            "ERROR 000735: Input Features: the geometry is not of type polyline",
        ));
    }
    Ok(())
}

/// The pieces of a line part between consecutive `cuts` (vertex index, or a new point between
/// vertex `i` and `i + 1`).
struct Cut {
    segment: usize,
    /// Whether the cut is at vertex `segment` (otherwise inside the segment that starts there).
    at_vertex: bool,
    point: [f64; 2],
    z: Option<f64>,
    m: Option<f64>,
}

fn piece(g: &Geometry, range: &std::ops::Range<usize>, from: &Cut, to: &Cut) -> Geometry {
    // Vertices strictly between the two cuts.
    let first = from.segment + 1;
    let last = to.segment as i64 - i64::from(to.at_vertex);
    let mut xy = vec![from.point];
    let mut z = from.z.map(|v| vec![v]);
    let mut m = from.m.map(|v| vec![v]);
    for i in first..=(last.max(0) as usize) {
        if last < first as i64 {
            break;
        }
        let k = range.start + i;
        xy.push(g.xy[k]);
        if let (Some(zz), Some(src)) = (&mut z, &g.z) {
            zz.push(src[k]);
        }
        if let (Some(mm), Some(src)) = (&mut m, &g.m) {
            mm.push(src[k]);
        }
    }
    xy.push(to.point);
    if let Some(zz) = &mut z {
        zz.push(to.z.unwrap_or(f64::NAN));
    }
    if let Some(mm) = &mut m {
        mm.push(to.m.unwrap_or(f64::NAN));
    }
    Geometry {
        kind: GeometryKind::Polyline,
        parts: vec![0],
        part_types: None,
        xy,
        z,
        m,
        curves: Vec::new(),
    }
}

fn vertex_cut(g: &Geometry, range: &std::ops::Range<usize>, i: usize) -> Cut {
    let k = range.start + i;
    Cut {
        // A cut at vertex i starts after segment i - 1; `piece` adds vertices first..=last.
        segment: i,
        at_vertex: true,
        point: g.xy[k],
        z: g.z.as_ref().map(|z| z[k]),
        m: g.m.as_ref().map(|m| m[k]),
    }
}

/// Splits every part at the given cuts (sorted along the part), numbering the pieces of the
/// feature from 1.
fn split_feature(
    f: &Feature,
    g: &Geometry,
    cuts_per_part: &[Vec<Cut>],
    features: &mut Vec<Feature>,
) {
    let mut seq = 0;
    for (range, cuts) in g.part_ranges().zip(cuts_per_part) {
        let n = range.len();
        if n < 2 {
            continue;
        }
        let mut bounds: Vec<Cut> = Vec::with_capacity(cuts.len() + 2);
        bounds.push(vertex_cut(g, &range, 0));
        for c in cuts {
            bounds.push(Cut {
                segment: c.segment,
                at_vertex: c.at_vertex,
                point: c.point,
                z: c.z,
                m: c.m,
            });
        }
        bounds.push(vertex_cut(g, &range, n - 1));
        for w in bounds.windows(2) {
            seq += 1;
            let mut shape = piece(g, &range, &w[0], &w[1]);
            shape.z = shape.z.filter(|_| g.z.is_some());
            features.push(Feature {
                oid: features.len() as i64,
                geometry: Some(shape),
                attributes: with_value(
                    &f.attributes,
                    [Value::Integer(f.oid as i32), Value::Integer(seq)],
                ),
            });
        }
    }
}

fn with_split_fields(input: &dyn FeatureClass) -> Vec<Field> {
    let mut fields = with_orig_fid(input);
    fields.push(Field::new("ORIG_SEQ", FieldType::Integer));
    fields
}

/// `arcpy.management.SplitLine(in_features, out_feature_class)` (Split Line At Vertices).
pub fn split_line_at_vertices(input: &dyn FeatureClass) -> Result<ToolOutput> {
    require_plain_lines(input)?;
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let cuts: Vec<Vec<Cut>> = g
            .part_ranges()
            .map(|r| {
                (1..r.len().saturating_sub(1))
                    .map(|i| vertex_cut(g, &r, i))
                    .collect()
            })
            .collect();
        split_feature(&f, g, &cuts, &mut features);
    }
    Ok(new_output(
        input,
        GeometryKind::Polyline,
        with_split_fields(input),
        features,
    ))
}

/// The closest location of `p` on a part: (segment index, point, distance).
fn closest_on_part(
    g: &Geometry,
    r: &std::ops::Range<usize>,
    p: [f64; 2],
) -> Option<(usize, [f64; 2], f64, f64)> {
    let mut best: Option<(usize, [f64; 2], f64, f64)> = None;
    for i in 0..r.len().saturating_sub(1) {
        let (a, b) = (g.xy[r.start + i], g.xy[r.start + i + 1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 == 0.0 {
            0.0
        } else {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
        };
        let q = [a[0] + t * dx, a[1] + t * dy];
        let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
        if best.is_none_or(|b| d < b.2) {
            best = Some((i, q, d, t));
        }
    }
    best
}

/// `arcpy.management.SplitLineAtPoint(in_features, point_features, out_feature_class,
/// search_radius)`.
pub fn split_line_at_point(
    input: &dyn FeatureClass,
    points: &dyn FeatureClass,
    search_radius: f64,
    tolerance: f64,
) -> Result<ToolOutput> {
    require_plain_lines(input)?;
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for f in points.features() {
        if let Some(g) = f?.geometry {
            pts.extend(g.xy);
        }
    }
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let mut cuts_per_part: Vec<Vec<Cut>> = Vec::new();
        for r in g.part_ranges() {
            // (segment, t, point) of each split location.
            let mut found: Vec<(usize, f64, [f64; 2])> = Vec::new();
            for &p in &pts {
                let Some((seg, q, d, t)) = closest_on_part(g, &r, p) else {
                    continue;
                };
                if d > search_radius + 1e-9 {
                    continue;
                }
                // Normalise a location at a vertex to the vertex that starts the next segment.
                let n = r.len();
                let a = g.xy[r.start + seg];
                let b = g.xy[r.start + seg + 1];
                let near = |u: [f64; 2]| {
                    ((q[0] - u[0]).powi(2) + (q[1] - u[1]).powi(2)).sqrt() <= tolerance
                };
                let (seg, t, q) = if near(a) {
                    (seg, 0.0, a)
                } else if near(b) {
                    (seg + 1, 0.0, b)
                } else {
                    (seg, t, q)
                };
                // The part's own ends are no splits.
                if t == 0.0 && (seg == 0 || seg >= n - 1) {
                    continue;
                }
                found.push((seg, t, q));
            }
            found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
            found.dedup_by(|a, b| a.0 == b.0 && (a.1 - b.1).abs() * 1e9 < 1.0);
            let cuts = found
                .into_iter()
                .map(|(seg, t, q)| {
                    if t == 0.0 {
                        // A vertex: the piece before it ends there, the one after starts there.
                        vertex_cut(g, &r, seg)
                    } else {
                        let k = r.start + seg;
                        let lerp =
                            |v: &Option<Vec<f64>>| v.as_ref().map(|v| v[k] + t * (v[k + 1] - v[k]));
                        Cut {
                            segment: seg,
                            at_vertex: false,
                            point: q,
                            z: lerp(&g.z),
                            m: lerp(&g.m),
                        }
                    }
                })
                .collect();
            cuts_per_part.push(cuts);
        }
        split_feature(&f, g, &cuts_per_part, &mut features);
    }
    Ok(new_output(
        input,
        GeometryKind::Polyline,
        with_split_fields(input),
        features,
    ))
}

/// How Eliminate Polygon Part combines its area and percent tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    Area,
    Percent,
    AreaAndPercent,
    AreaOrPercent,
}

impl Condition {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "AREA" => Self::Area,
            "PERCENT" => Self::Percent,
            "AREA_AND_PERCENT" => Self::AreaAndPercent,
            "AREA_OR_PERCENT" => Self::AreaOrPercent,
            _ => return None,
        })
    }

    fn small(self, area: f64, percent: f64, max_area: f64, max_percent: f64) -> bool {
        let (a, p) = (area < max_area, percent < max_percent);
        match self {
            Self::Area => a,
            Self::Percent => p,
            Self::AreaAndPercent => a && p,
            Self::AreaOrPercent => a || p,
        }
    }
}

/// `arcpy.management.EliminatePolygonPart(in_features, out_feature_class, condition, part_area,
/// part_area_percent, part_option)`.
pub fn eliminate_polygon_part(
    input: &dyn FeatureClass,
    condition: Condition,
    part_area: f64,
    part_percent: f64,
    any_part: bool,
) -> Result<ToolOutput> {
    if kind_of(input)? != GeometryKind::Polygon {
        return Err(err(
            "ERROR 000735: Input Features: the geometry is not of type polygon",
        ));
    }
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let attributes = with_value(&f.attributes, [Value::Integer(f.oid as i32)]);
        let Some(g) = &f.geometry else {
            features.push(Feature {
                oid: features.len() as i64,
                geometry: None,
                attributes,
            });
            continue;
        };
        let parts = polygon_parts(g);
        let total: f64 = parts.iter().map(|(e, _)| ring_area(g, e)).sum();
        let largest = parts
            .iter()
            .enumerate()
            .max_by(|a, b| ring_area(g, &a.1.0).total_cmp(&ring_area(g, &b.1.0)))
            .map(|(i, _)| i);
        let mut out = Geometry {
            kind: GeometryKind::Polygon,
            parts: Vec::new(),
            part_types: None,
            xy: Vec::new(),
            z: g.z.as_ref().map(|_| Vec::new()),
            m: g.m.as_ref().map(|_| Vec::new()),
            curves: Vec::new(),
        };
        let mut push = |r: &std::ops::Range<usize>| {
            out.parts.push(out.xy.len() as u32);
            out.xy.extend_from_slice(&g.xy[r.clone()]);
            if let (Some(o), Some(s)) = (&mut out.z, &g.z) {
                o.extend_from_slice(&s[r.clone()]);
            }
            if let (Some(o), Some(s)) = (&mut out.m, &g.m) {
                o.extend_from_slice(&s[r.clone()]);
            }
        };
        for (i, (ext, holes)) in parts.iter().enumerate() {
            let area = ring_area(g, ext);
            let percent = if total > 0.0 {
                area / total * 100.0
            } else {
                0.0
            };
            if any_part
                && Some(i) != largest
                && condition.small(area, percent, part_area, part_percent)
            {
                continue;
            }
            push(ext);
            for h in holes {
                let ha = ring_area(g, h);
                let hp = if total > 0.0 { ha / total * 100.0 } else { 0.0 };
                if !condition.small(ha, hp, part_area, part_percent) {
                    push(h);
                }
            }
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(out),
            attributes,
        });
    }
    let mut out = new_output(input, GeometryKind::Polygon, with_orig_fid(input), features);
    out.has_z = input.has_z();
    out.has_m = input.has_m();
    Ok(out)
}

fn number(v: &Value) -> Option<f64> {
    Some(match v {
        Value::SmallInteger(n) => f64::from(*n),
        Value::Integer(n) => f64::from(*n),
        Value::BigInteger(n) => *n as f64,
        Value::Single(n) => f64::from(*n),
        Value::Double(n) => *n,
        _ => return None,
    })
}

/// `arcpy.management.XYToLine(in_table, out_featureclass, startx_field, starty_field,
/// endx_field, endy_field, line_type, id_field, spatial_reference, attributes)`. The line type
/// only matters for long lines, which ArcGIS densifies along the geodesic; here every line is
/// straight.
pub fn xy_to_line(
    input: &dyn FeatureClass,
    coordinate_fields: [&str; 4],
    id_field: Option<&str>,
    spatial_reference_wkt: Option<String>,
    attributes: bool,
) -> Result<ToolOutput> {
    let find = |name: &str| {
        input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {name} does not exist within table"
                ))
            })
    };
    let idx: Vec<usize> = coordinate_fields
        .iter()
        .map(|n| find(n))
        .collect::<Result<_>>()?;
    let id = id_field.map(find).transpose()?;
    let kept: Vec<usize> = if attributes {
        (0..input.fields().len())
            .filter(|&i| {
                !matches!(
                    input.fields()[i].field_type,
                    FieldType::ObjectId | FieldType::Geometry
                )
            })
            .collect()
    } else {
        let mut k = idx.clone();
        k.extend(id.filter(|i| !idx.contains(i)));
        k
    };
    let mut fields: Vec<Field> = kept.iter().map(|&i| input.fields()[i].clone()).collect();
    if attributes {
        fields.push(Field::new("ORIG_FID", FieldType::Integer));
    }
    let mut features = Vec::new();
    let mut messages = Vec::new();
    for (row, f) in input.features().enumerate() {
        let f = f?;
        let oid = if f.oid >= 0 { f.oid } else { row as i64 };
        let c: Option<Vec<f64>> = idx
            .iter()
            .map(|&i| f.attributes.get(i).and_then(number))
            .collect();
        let Some(c) = c else {
            messages.push(format!(
                "WARNING 000072: Cannot process feature with OID {oid}"
            ));
            continue;
        };
        if c[0] == c[2] && c[1] == c[3] {
            messages.push(format!(
                "WARNING 000072: Cannot process feature with OID {oid}"
            ));
            continue;
        }
        let mut values: Vec<Value> = kept.iter().map(|&i| f.attributes[i].clone()).collect();
        if attributes {
            values.push(Value::Integer(oid as i32));
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(Geometry {
                kind: GeometryKind::Polyline,
                parts: vec![0],
                part_types: None,
                xy: vec![[c[0], c[1]], [c[2], c[3]]],
                z: None,
                m: None,
                curves: Vec::new(),
            }),
            attributes: values,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt,
        features,
        messages,
    })
}

/// Metres in a distance unit of Bearing Distance To Line.
pub fn distance_unit_meters(unit: &str) -> Option<f64> {
    Some(match unit.to_ascii_uppercase().as_str() {
        "METERS" => 1.0,
        "KILOMETERS" => 1000.0,
        "MILES" => 5280.0 * 1200.0 / 3937.0,
        "NAUTICAL_MILES" => 1852.0,
        "FEET" => 0.3048,
        "US_SURVEY_FEET" => 1200.0 / 3937.0,
        "YARDS" => 0.9144,
        _ => return None,
    })
}

/// Degrees in one bearing unit of Bearing Distance To Line.
pub fn bearing_unit_degrees(unit: &str) -> Option<f64> {
    Some(match unit.to_ascii_uppercase().as_str() {
        "DEGREES" => 1.0,
        "MILS" => 360.0 / 6400.0,
        "RADS" => 180.0 / std::f64::consts::PI,
        "GRADS" => 0.9,
        _ => return None,
    })
}

/// The fields and values Bearing Distance To Line and XY To Line keep (`attributes`: all fields
/// and `ORIG_FID`; else the coordinate fields and the ID field).
struct Kept {
    fields: Vec<Field>,
    indexes: Vec<usize>,
}

fn kept_fields(
    input: &dyn FeatureClass,
    coordinates: &[usize],
    id: Option<usize>,
    attributes: bool,
) -> Kept {
    let indexes: Vec<usize> = if attributes {
        (0..input.fields().len())
            .filter(|&i| {
                !matches!(
                    input.fields()[i].field_type,
                    FieldType::ObjectId | FieldType::Geometry
                )
            })
            .collect()
    } else {
        let mut k = coordinates.to_vec();
        k.extend(id.filter(|i| !coordinates.contains(i)));
        k
    };
    let mut fields: Vec<Field> = indexes.iter().map(|&i| input.fields()[i].clone()).collect();
    if attributes {
        fields.push(Field::new("ORIG_FID", FieldType::Integer));
    }
    Kept { fields, indexes }
}

/// `arcpy.management.BearingDistanceToLine(in_table, out_featureclass, x_field, y_field,
/// distance_field, distance_units, bearing_field, bearing_units, line_type, id_field,
/// spatial_reference, attributes)`: the line from each (x, y) along the WGS 84 geodesic of the
/// bearing and distance (clockwise from north), whatever the line type; as XY To Line, only
/// straight lines (short ones) are drawn.
#[cfg(feature = "proj")]
#[allow(clippy::too_many_arguments)]
pub fn bearing_distance_to_line(
    input: &dyn FeatureClass,
    fields: [&str; 4],
    distance_unit: f64,
    bearing_unit: f64,
    id_field: Option<&str>,
    crs: &unigis_proj::Crs,
    attributes: bool,
) -> Result<ToolOutput> {
    let find = |name: &str| {
        input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {name} does not exist within table"
                ))
            })
    };
    let idx: Vec<usize> = fields.iter().map(|n| find(n)).collect::<Result<_>>()?;
    let id = id_field.map(find).transpose()?;
    let kept = kept_fields(input, &idx, id, attributes);
    let wgs84 = unigis_proj::Crs::parse("4326")?;
    let (to_geographic, from_geographic) = if crs.is_geographic() {
        (None, None)
    } else {
        (
            Some(unigis_proj::Transform::new(crs, &wgs84)?),
            Some(unigis_proj::Transform::new(&wgs84, crs)?),
        )
    };
    let mut features = Vec::new();
    let mut messages = Vec::new();
    for (row, f) in input.features().enumerate() {
        let f = f?;
        let oid = if f.oid >= 0 { f.oid } else { row as i64 };
        let c: Option<Vec<f64>> = idx
            .iter()
            .map(|&i| f.attributes.get(i).and_then(number))
            .collect();
        let Some(c) = c.filter(|c| c[2] > 0.0) else {
            messages.push(format!(
                "WARNING 000072: Cannot process feature with OID {oid}"
            ));
            continue;
        };
        let mut start = [[c[0], c[1]]];
        if let Some(t) = &to_geographic {
            t.apply(&mut start)?;
        }
        let mut end = [unigis_geometry::geodesic::direct(
            start[0],
            c[3] * bearing_unit,
            c[2] * distance_unit,
        )];
        if let Some(t) = &from_geographic {
            t.apply(&mut end)?;
        }
        let mut values: Vec<Value> = kept
            .indexes
            .iter()
            .map(|&i| f.attributes[i].clone())
            .collect();
        if attributes {
            values.push(Value::Integer(oid as i32));
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(Geometry {
                kind: GeometryKind::Polyline,
                parts: vec![0],
                part_types: None,
                xy: vec![[c[0], c[1]], end[0]],
                z: None,
                m: None,
                curves: Vec::new(),
            }),
            attributes: values,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: kept.fields,
        spatial_reference_wkt: Some(crs.esri_wkt().to_owned()),
        features,
        messages,
    })
}

/// Piece ends (piece, is the end) by (group, rounded node).
type NodeEnds = std::collections::HashMap<(usize, (i64, i64)), Vec<(usize, bool)>>;

/// `arcpy.management.UnsplitLine(in_features, out_feature_class, dissolve_field)`.
///
/// Within each group of equal dissolve-field values, lines are joined where exactly two line ends
/// meet at a point (three or more ends make a junction that stays split; a closed ring stays
/// whole). Only the dissolve fields are kept. ArcGIS lists the lines that were not joined to any
/// other first (in input order), then the joined ones in the order of their first piece.
pub fn unsplit_line(input: &dyn FeatureClass, dissolve_fields: &[String]) -> Result<ToolOutput> {
    require_plain_lines(input)?;
    if input.has_z() || input.has_m() {
        return Err(err("Unsplit Line with Z or M values is not supported"));
    }
    let keys: Vec<usize> = dissolve_fields
        .iter()
        .map(|name| {
            input
                .fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    err(format!(
                        "ERROR 000728: Field {name} does not exist within table"
                    ))
                })
        })
        .collect::<Result<_>>()?;

    // Every part of every feature is a piece (in input order) with its group key.
    let mut group_keys: Vec<Vec<Value>> = Vec::new();
    let mut pieces: Vec<(usize, Vec<[f64; 2]>)> = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let key: Vec<Value> = keys.iter().map(|&i| f.attributes[i].clone()).collect();
        let group = group_keys
            .iter()
            .position(|k| *k == key)
            .unwrap_or_else(|| {
                group_keys.push(key);
                group_keys.len() - 1
            });
        for r in g.part_ranges() {
            if r.len() >= 2 {
                pieces.push((group, g.xy[r].to_vec()));
            }
        }
    }

    let node = |p: [f64; 2]| ((p[0] / 1e-3).round() as i64, (p[1] / 1e-3).round() as i64);
    // The piece ends (piece, is the end) at each node, per group.
    let mut at_node: NodeEnds = std::collections::HashMap::new();
    for (i, (key, xy)) in pieces.iter().enumerate() {
        let (a, b) = (node(xy[0]), node(xy[xy.len() - 1]));
        at_node.entry((*key, a)).or_default().push((i, false));
        at_node.entry((*key, b)).or_default().push((i, true));
    }
    // The only other piece end at the node of piece `i`'s end, when exactly two ends meet there.
    let partner = |i: usize, end: bool| -> Option<(usize, bool)> {
        let xy = &pieces[i].1;
        let p = if end { xy[xy.len() - 1] } else { xy[0] };
        let ends = at_node.get(&(pieces[i].0, node(p)))?;
        match ends.as_slice() {
            [x, y] if *x == (i, end) && y.0 != i => Some(*y),
            [x, y] if *y == (i, end) && x.0 != i => Some(*x),
            _ => None,
        }
    };

    let mut used = vec![false; pieces.len()];
    let mut joined: Vec<(usize, Vec<[f64; 2]>)> = Vec::new();
    let mut single: Vec<(usize, Vec<[f64; 2]>)> = Vec::new();
    for first in 0..pieces.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut line = pieces[first].1.clone();
        let mut count = 1;
        // Forward from the end, then backward from the start.
        for forward in [true, false] {
            let (mut cur, mut end) = (first, forward);
            while let Some((next, next_end)) = partner(cur, end) {
                if used[next] {
                    break;
                }
                used[next] = true;
                count += 1;
                let mut xy = pieces[next].1.clone();
                // Leave the next piece through its other end.
                if forward == next_end {
                    xy.reverse();
                }
                if forward {
                    line.extend_from_slice(&xy[1..]);
                } else {
                    xy.pop();
                    xy.extend_from_slice(&line);
                    line = xy;
                }
                (cur, end) = (next, !next_end);
            }
        }
        if count == 1 {
            single.push((first, line));
        } else {
            joined.push((first, line));
        }
    }

    let fields: Vec<Field> = keys.iter().map(|&i| input.fields()[i].clone()).collect();
    let features = single
        .into_iter()
        .chain(joined)
        .enumerate()
        .map(|(n, (first, xy))| Feature {
            oid: n as i64,
            geometry: Some(Geometry {
                kind: GeometryKind::Polyline,
                parts: vec![0],
                part_types: None,
                xy,
                z: None,
                m: None,
                curves: Vec::new(),
            }),
            attributes: group_keys[pieces[first].0].clone(),
        })
        .collect();
    Ok(new_output(input, GeometryKind::Polyline, fields, features))
}
