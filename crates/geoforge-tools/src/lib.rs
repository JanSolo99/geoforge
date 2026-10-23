//! Geoprocessing tools. Each follows its ArcGIS Pro counterpart (parameters,
//! output schema, feature order, messages) as observed through the ArcGIS bridge
//! and recorded in `testdata/arcgis-reference/manifest.json`.

pub mod construct;
mod dm_data;
mod dm_fields;
mod dm_geometry;
pub mod dm_tools;
mod fieldmap;
pub mod planar;
pub mod pycalc;
pub mod registry;
pub mod regression;
pub mod spatstat;
mod sql;
pub mod swm;
pub mod tables;
mod zm;

pub use fieldmap::{FieldMapEntry, MapSource, parse_field_mapping, table_stem};
pub use sql::WhereClause;

use geoforge_core::{Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value};
use geoforge_geometry::{BufferOptions, ClipArea, LineEnd};

/// A tool's result, ready to be written to any output format.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub kind: GeometryKind,
    pub has_z: bool,
    pub has_m: bool,
    pub fields: Vec<Field>,
    pub spatial_reference_wkt: Option<String>,
    pub features: Vec<Feature>,
    /// Warnings in ArcGIS style (`WARNING nnnnnn: ...`).
    pub messages: Vec<String>,
}

/// `arcpy.analysis.Clip(in_features, clip_features, out_feature_class)`.
///
/// Output keeps the input's schema, order and Z/M awareness; features entirely
/// outside the (dissolved) clip polygons are dropped; split features stay
/// multipart.
pub fn clip(input: &dyn FeatureClass, clip_features: &dyn FeatureClass) -> Result<ToolOutput> {
    if clip_features.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(Error::Unsupported(
            "ERROR 000365: clip features must be polygons".into(),
        ));
    }
    let clip_geoms: Vec<_> = clip_features
        .features()
        .filter_map(|f| f.map(|f| f.geometry).transpose())
        .collect::<Result<_>>()?;
    clip_to_area(input, &ClipArea::new(clip_geoms.iter())?, None)
}

/// The input's parts inside `area`. Lines are cracked at `nodes` (Split), or
/// only rejoined where the overlay split them at a mere touch (Clip).
fn clip_to_area(
    input: &dyn FeatureClass,
    area: &ClipArea,
    nodes: Option<&geoforge_geometry::LineNodes>,
) -> Result<ToolOutput> {
    let tol = default_xy_tolerance(input.spatial_reference_wkt());
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    // Clip (not Split) of M-aware lines goes through another ArcGIS code path.
    let measured_lines = nodes.is_none() && kind == GeometryKind::Polyline && input.has_m();
    let measured = if measured_lines {
        let lines = load(input)?;
        Some(geoforge_geometry::LineNodes::new(
            lines.iter().filter_map(|f| f.geometry.as_ref()),
            tol,
        )?)
    } else {
        None
    };
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if let Some(mut out) = geoforge_geometry::clip(g, area)? {
            if kind == GeometryKind::Polyline {
                out = match (nodes, &measured) {
                    (Some(n), _) => n.crack(&out),
                    // M-aware lines get a vertex where other input lines cross.
                    (None, Some(all)) => {
                        all.densify(&geoforge_geometry::LineNodes::new([g], tol)?.crack(&out))
                    }
                    (None, None) => geoforge_geometry::LineNodes::new([g], tol)?.crack(&out),
                };
            }
            if kind != GeometryKind::Point && kind != GeometryKind::Multipoint {
                zm::restore_from(
                    &mut out,
                    &[g],
                    input.has_z(),
                    input.has_m(),
                    nodes.is_none(),
                );
            }
            features.push(Feature {
                oid: f.oid,
                geometry: Some(out),
                attributes: f.attributes,
            });
        }
    }
    // That path writes the last feature first.
    if measured_lines {
        features.rotate_right(1);
    }
    let mut messages = Vec::new();
    if features.is_empty() {
        messages.push("WARNING 000117: Warning empty output generated.".into());
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages,
    })
}

/// ArcGIS `dissolve_option`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dissolve {
    None,
    All,
    /// One (multipart) buffer per combination of these fields' values.
    List(Vec<String>),
}

/// `arcpy.analysis.Buffer(in, out, distance, "FULL", line_end, dissolve)`, planar.
///
/// `distance` is in the input's linear unit. Without dissolving, the output has the
/// input fields plus `BUFF_DIST` and `ORIG_FID`, one feature per input feature
/// (in input order); with `Dissolve::All` it is a single feature with no fields.
/// `buffer_distance_or_field`: a linear distance or a numeric field holding one
/// per feature (in the coordinate system's unit).
#[derive(Debug, Clone)]
pub enum BufferDistance {
    Linear(f64),
    /// Metres measured on the ellipsoid (geographic input with a distance in a linear unit): the
    /// geodesic buffer. A plain number on geographic data is `Linear`, in degrees, and planar.
    Geodesic(f64),
    Field(String),
}

pub fn buffer(
    input: &dyn FeatureClass,
    distance: BufferDistance,
    line_end: LineEnd,
    side: geoforge_geometry::LineSide,
    dissolve: Dissolve,
) -> Result<ToolOutput> {
    buffer_inner(input, distance, line_end, side, dissolve, false)
}

/// [`buffer`] with the round parts as true circular arcs where ArcGIS keeps them
/// (geodatabase output): point buffers, and polygon buffers that are not dissolved.
/// Line buffers and dissolved polygon buffers stay densified, as in ArcGIS.
pub fn buffer_arcs(
    input: &dyn FeatureClass,
    distance: BufferDistance,
    line_end: LineEnd,
    side: geoforge_geometry::LineSide,
    dissolve: Dissolve,
) -> Result<ToolOutput> {
    buffer_inner(input, distance, line_end, side, dissolve, true)
}

fn buffer_inner(
    input: &dyn FeatureClass,
    distance: BufferDistance,
    line_end: LineEnd,
    side: geoforge_geometry::LineSide,
    dissolve: Dissolve,
    arcs: bool,
) -> Result<ToolOutput> {
    use geoforge_geometry::LineSide;
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let sr = input.spatial_reference_wkt();
    let geographic = sr.is_some_and(|w| w.trim_start().starts_with("GEOGCS"));
    if matches!(distance, BufferDistance::Geodesic(_)) && !geographic {
        return Err(Error::Unsupported(
            "a geodesic distance needs geographic input".into(),
        ));
    }
    match (side, kind) {
        (LineSide::Left | LineSide::Right, GeometryKind::Polyline)
        | (LineSide::OutsideOnly, GeometryKind::Polygon)
        | (LineSide::Full, _) => {}
        _ => {
            return Err(Error::Unsupported(format!(
                "line_side {side:?} does not apply to {kind:?} input"
            )));
        }
    }
    let field_index = |name: &str| {
        input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "ERROR 000728: Field {name} does not exist within table"
                ))
            })
    };
    let distance_field = match &distance {
        BufferDistance::Field(name) => Some(field_index(name)?),
        BufferDistance::Linear(_) | BufferDistance::Geodesic(_) => None,
    };
    let options = |d: f64| BufferOptions {
        distance: d,
        line_end,
        max_deviation: geoforge_core::curves::arcgis_densify_deviation(geographic),
        side,
    };
    // Each feature with its distance (features without one are left out).
    let mut buffered: Vec<(Feature, f64, geoforge_core::Geometry)> = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let d = match (&distance, distance_field) {
            (BufferDistance::Linear(d) | BufferDistance::Geodesic(d), _) => *d,
            (_, Some(i)) => match number(&f.attributes[i]) {
                Some(d) => d,
                None => continue,
            },
            _ => unreachable!(),
        };
        let b = if matches!(distance, BufferDistance::Geodesic(_)) {
            let g = if g.has_curves() {
                g.densify_curves_arcgis(0.0001)?
            } else {
                g.clone()
            };
            geoforge_geometry::buffer_geodesic(
                &g,
                &geoforge_geometry::GeodesicOptions {
                    distance: d,
                    line_end,
                    side,
                },
            )?
        } else {
            geoforge_geometry::buffer(g, &options(d))?
        };
        if let Some(b) = b {
            buffered.push((f, d, b));
        }
    }
    // The circles the buffers were built around, per buffered feature.
    let max_deviation = geoforge_core::curves::arcgis_densify_deviation(geographic);
    let is_point = matches!(kind, GeometryKind::Point | GeometryKind::Multipoint);
    // True arcs are for projected data; a geodesic buffer is vertices all the way.
    let keep_arcs = arcs
        && !matches!(distance, BufferDistance::Geodesic(_))
        && match kind {
            GeometryKind::Point | GeometryKind::Multipoint => true,
            GeometryKind::Polygon => matches!(dissolve, Dissolve::None),
            _ => false,
        };
    let circles_of =
        |g: &geoforge_core::Geometry, d: f64| -> Vec<geoforge_geometry::BufferCircle> {
            if d == 0.0 {
                return Vec::new();
            }
            g.xy.iter().map(|&p| (p, d.abs())).collect()
        };
    let with_arcs = |b: geoforge_core::Geometry, circles: &[geoforge_geometry::BufferCircle]| {
        if keep_arcs {
            geoforge_geometry::recover_arcs(&b, circles, max_deviation, is_point)
        } else {
            b
        }
    };
    let mut out = ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields: Vec::new(),
        spatial_reference_wkt: sr.map(str::to_owned),
        features: Vec::new(),
        messages: Vec::new(),
    };
    match dissolve {
        Dissolve::All => {
            let union = geoforge_geometry::dissolve(buffered.iter().map(|b| &b.2))?;
            if let Some(g) = union {
                let circles: Vec<_> = buffered
                    .iter()
                    .filter_map(|(f, d, _)| Some(circles_of(f.geometry.as_ref()?, *d)))
                    .flatten()
                    .collect();
                out.features.push(Feature {
                    oid: 1,
                    geometry: Some(with_arcs(g, &circles)),
                    attributes: Vec::new(),
                });
            }
        }
        Dissolve::List(names) => {
            let keys: Vec<usize> = names
                .iter()
                .map(|n| field_index(n))
                .collect::<Result<_>>()?;
            out.fields = keys.iter().map(|&i| input.fields()[i].clone()).collect();
            type Group = (
                Vec<Value>,
                Vec<geoforge_core::Geometry>,
                Vec<geoforge_geometry::BufferCircle>,
            );
            let mut groups: Vec<Group> = Vec::new();
            for (f, d, b) in buffered {
                let key: Vec<Value> = keys.iter().map(|&i| f.attributes[i].clone()).collect();
                let circles = f
                    .geometry
                    .as_ref()
                    .map_or_else(Vec::new, |g| circles_of(g, d));
                match groups.iter_mut().find(|(k, ..)| k == &key) {
                    Some((_, members, all)) => {
                        members.push(b);
                        all.extend(circles);
                    }
                    None => groups.push((key, vec![b], circles)),
                }
            }
            groups.sort_by(|a, b| {
                a.0.iter()
                    .zip(&b.0)
                    .map(|(x, y)| compare(x, y))
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for (key, members, circles) in groups {
                if let Some(g) = geoforge_geometry::dissolve(members.iter())? {
                    out.features.push(Feature {
                        oid: out.features.len() as i64,
                        geometry: Some(with_arcs(g, &circles)),
                        attributes: key,
                    });
                }
            }
        }
        Dissolve::None => {
            out.fields = input.fields().to_vec();
            out.fields.push(Field::new("BUFF_DIST", FieldType::Double));
            out.fields.push(Field::new("ORIG_FID", FieldType::Integer));
            for (f, d, b) in buffered {
                let circles = f
                    .geometry
                    .as_ref()
                    .map_or_else(Vec::new, |g| circles_of(g, d));
                let mut attributes = f.attributes;
                attributes.push(Value::Double(d));
                attributes.push(Value::Integer(f.oid as i32));
                out.features.push(Feature {
                    oid: f.oid,
                    geometry: Some(with_arcs(b, &circles)),
                    attributes,
                });
            }
        }
    }
    Ok(out)
}

/// A tool input with the name ArcGIS uses for it in output field names
/// (`FID_<name>`): the shapefile's base name or the feature class name.
pub struct NamedInput<'a> {
    pub name: String,
    pub features: &'a dyn FeatureClass,
}

fn dimension(k: GeometryKind) -> u8 {
    match k {
        GeometryKind::Point | GeometryKind::Multipoint => 0,
        GeometryKind::Polyline => 1,
        GeometryKind::Polygon | GeometryKind::MultiPatch => 2,
    }
}

/// Output fields for Intersect/Union: per input `FID_<name>` then its fields, with
/// names that clash with earlier ones suffixed `_1`, `_2`, ... (as ArcGIS does).
fn overlay_fields(inputs: &[NamedInput]) -> Vec<Field> {
    let mut fields: Vec<Field> = Vec::new();
    for input in inputs {
        push_unique(
            &mut fields,
            Field::new(format!("FID_{}", input.name), FieldType::Integer),
        );
        for f in input.features.fields() {
            push_unique(&mut fields, f.clone());
        }
    }
    fields
}

/// An overlay's output is Z- and M-aware if any input is.
fn zm_of(inputs: &[NamedInput]) -> (bool, bool) {
    (
        inputs.iter().any(|i| i.features.has_z()),
        inputs.iter().any(|i| i.features.has_m()),
    )
}

/// Every input's features, loaded once.
fn load(input: &dyn FeatureClass) -> Result<Vec<Feature>> {
    input
        .features()
        .filter(|f| f.as_ref().map_or(true, |f| f.geometry.is_some()))
        .collect()
}

/// `arcpy.analysis.Intersect([in1, in2, ...], out)` with `join_attributes="ALL"` and
/// `output_type="INPUT"`: one output per combination of one feature from each input
/// whose geometries share something, in the lowest input dimension. Rows are in
/// input FID order; point outputs are ordered by Y (as ArcGIS emits them).
pub fn intersect(inputs: &[NamedInput]) -> Result<ToolOutput> {
    intersect_with(inputs, &IntersectOptions::default())
}

/// Intersect / Union `join_attributes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JoinAttributes {
    #[default]
    All,
    NoFid,
    OnlyFid,
}

impl JoinAttributes {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "ALL" => Self::All,
            "NO_FID" => Self::NoFid,
            "ONLY_FID" => Self::OnlyFid,
            _ => return None,
        })
    }
}

/// Intersect `output_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputType {
    /// The lowest dimension of the inputs.
    #[default]
    Input,
    Line,
    Point,
}

impl OutputType {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "INPUT" => Self::Input,
            "LINE" => Self::Line,
            "POINT" => Self::Point,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct IntersectOptions {
    pub join_attributes: JoinAttributes,
    pub output_type: OutputType,
}

/// Keeps the columns `join` asks for from an output laid out as `FID_<name>` plus
/// the fields of each input in turn (renaming clashes afresh).
fn apply_join_attributes(out: &mut ToolOutput, inputs: &[NamedInput], join: JoinAttributes) {
    if join == JoinAttributes::All {
        return;
    }
    let mut keep = Vec::new();
    let mut fields = Vec::new();
    let mut col = 0;
    for input in inputs {
        if join == JoinAttributes::OnlyFid {
            keep.push(col);
            push_unique(
                &mut fields,
                Field::new(format!("FID_{}", input.name), FieldType::Integer),
            );
        } else {
            for (k, f) in input.features.fields().iter().enumerate() {
                keep.push(col + 1 + k);
                push_unique(&mut fields, f.clone());
            }
        }
        col += 1 + input.features.fields().len();
    }
    out.fields = fields;
    for f in &mut out.features {
        f.attributes = keep.iter().map(|&i| f.attributes[i].clone()).collect();
    }
}

/// `arcpy.analysis.Intersect(inputs, out, join_attributes, "", output_type)`.
/// LINE output of polygons is their shared boundaries; POINT output is where the
/// inputs' outlines (polygon boundaries, lines) cross or touch, one multipoint
/// per pair of features (two inputs).
pub fn intersect_with(inputs: &[NamedInput], opts: &IntersectOptions) -> Result<ToolOutput> {
    let all_polygons = inputs
        .iter()
        .all(|i| i.features.geometry_kind() == Some(GeometryKind::Polygon));
    let mut out = match opts.output_type {
        OutputType::Point => intersect_outlines(inputs, true)?,
        OutputType::Line if all_polygons => intersect_outlines(inputs, false)?,
        _ => intersect_input(inputs)?,
    };
    apply_join_attributes(&mut out, inputs, opts.join_attributes);
    Ok(out)
}

/// Intersect's POINT output, or LINE output of polygons, for two inputs.
///
/// Points come per pair of features. As in ArcGIS, a point lying inside a stretch
/// shared by some pair (e.g. where a third line crosses two coincident ones) is
/// written in a second pass after all others, both for the pair that shares the
/// stretch and for the pair that crosses there. Rows are ordered by FID, polygon
/// inputs' FIDs first.
fn intersect_outlines(inputs: &[NamedInput], points: bool) -> Result<ToolOutput> {
    use geoforge_geometry::{Meeting, XY_TOLERANCE as TOL, on_path};
    if inputs.len() < 2 {
        return Err(Error::Unsupported(
            "Intersect needs at least two inputs".into(),
        ));
    }
    for i in inputs {
        if dimension(i.features.geometry_kind().unwrap_or(GeometryKind::Point)) == 0 {
            return Err(Error::Unsupported(
                "Intersect LINE/POINT output with point inputs".into(),
            ));
        }
    }
    let data: Vec<Vec<Feature>> = inputs
        .iter()
        .map(|i| load(i.features))
        .collect::<Result<_>>()?;
    let indexes: Vec<geoforge_geometry::EnvelopeIndex> = data
        .iter()
        .map(|fs| geoforge_geometry::EnvelopeIndex::new(fs.iter().map(|f| f.geometry.as_ref())))
        .collect();
    let geom = |i: usize, k: usize| data[i][k].geometry.as_ref().unwrap();
    // Where the outlines of one feature per input all meet: pairs of the first
    // two inputs, narrowed by each further input.
    let mut combos: Vec<(Vec<usize>, Meeting)> = Vec::new();
    for ka in 0..data[0].len() {
        for kb in indexes[1].candidates(&geom(0, ka).curve_envelope(), TOL) {
            let m = geoforge_geometry::meeting(geom(0, ka), geom(1, kb), TOL)?;
            if !m.points.is_empty() || !m.shared.is_empty() {
                combos.push((vec![ka, kb], m));
            }
        }
    }
    for (i, index) in indexes.iter().enumerate().skip(2) {
        let mut next = Vec::new();
        for (ks, m) in &combos {
            let mut env = geoforge_core::Envelope::EMPTY;
            for p in m.points.iter().chain(m.shared.iter().flatten()) {
                env.expand_xy(p[0], p[1]);
            }
            for k in index.candidates(&env, TOL) {
                let narrowed = narrow_meeting(m, geom(i, k), TOL)?;
                if !narrowed.points.is_empty() || !narrowed.shared.is_empty() {
                    let mut ks = ks.clone();
                    ks.push(k);
                    next.push((ks, narrowed));
                }
            }
        }
        combos = next;
    }
    // Sort key: FIDs with polygon inputs first.
    let mut order: Vec<usize> = (0..inputs.len()).collect();
    order.sort_by_key(|&i| inputs[i].features.geometry_kind() != Some(GeometryKind::Polygon));
    let key = |ks: &[usize]| -> Vec<i64> { order.iter().map(|&i| data[i][ks[i]].oid).collect() };
    let attrs = |ks: &[usize]| -> Vec<Value> {
        let mut v = Vec::new();
        for (i, &k) in ks.iter().enumerate() {
            v.push(Value::Integer(data[i][k].oid as i32));
            v.extend(data[i][k].attributes.iter().cloned());
        }
        v
    };
    let multipoint = |pts: Vec<[f64; 2]>| {
        let mut g = geoforge_core::Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Multipoint;
        g.xy = pts;
        g
    };
    // Lines are cracked where any two input outlines meet.
    let nodes = if points {
        None
    } else {
        let outlines: Vec<geoforge_core::Geometry> = data
            .iter()
            .flatten()
            .filter_map(|f| f.geometry.as_ref().map(geoforge_geometry::outline))
            .collect();
        Some(geoforge_geometry::LineNodes::new(outlines.iter(), TOL)?)
    };
    // (pass, key, input feature indices, geometry, attributes)
    type Row = (
        u8,
        Vec<i64>,
        Vec<usize>,
        geoforge_core::Geometry,
        Vec<Value>,
    );
    let mut rows: Vec<Row> = Vec::new();
    for (ks, m) in &combos {
        if let Some(nodes) = &nodes {
            if let Some(g) = geoforge_geometry::paths_geometry(m.shared.clone()) {
                rows.push((0, key(ks), ks.clone(), nodes.crack(&g), attrs(ks)));
            }
            continue;
        }
        let in_shared = |p: [f64; 2]| {
            combos
                .iter()
                .any(|(_, o)| o.shared.iter().any(|s| on_path(p, s, TOL, true)))
        };
        let (late, first): (Vec<[f64; 2]>, Vec<[f64; 2]>) =
            m.points.iter().partition(|&&p| in_shared(p));
        let mut late = late;
        // Nodes made inside this combination's shared stretches by other ones' points.
        for (_, o) in &combos {
            for &p in &o.points {
                if m.shared.iter().any(|s| on_path(p, s, TOL, true))
                    && !late.iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) <= TOL)
                {
                    late.push(p);
                }
            }
        }
        if !first.is_empty() {
            rows.push((0, key(ks), ks.clone(), multipoint(first), attrs(ks)));
        }
        if !late.is_empty() {
            rows.push((1, key(ks), ks.clone(), multipoint(late), attrs(ks)));
        }
    }
    // Z and M from the features combined: polygon inputs first for POINT output
    // (as its row order), else in input order.
    let (has_z, has_m) = zm_of(inputs);
    if has_z || has_m {
        for r in &mut rows {
            let sources: Vec<&geoforge_core::Geometry> = if points {
                order.iter().map(|&i| geom(i, r.2[i])).collect()
            } else {
                (0..inputs.len()).map(|i| geom(i, r.2[i])).collect()
            };
            zm::restore_from(&mut r.3, &sources, has_z, has_m, true);
        }
    }
    rows.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let features: Vec<Feature> = rows
        .into_iter()
        .enumerate()
        .map(|(i, r)| Feature {
            oid: i as i64,
            geometry: Some(r.3),
            attributes: r.4,
        })
        .collect();
    Ok(ToolOutput {
        kind: if points {
            GeometryKind::Multipoint
        } else {
            GeometryKind::Polyline
        },
        has_z,
        has_m,
        fields: overlay_fields(inputs),
        spatial_reference_wkt: inputs[0]
            .features
            .spatial_reference_wkt()
            .map(str::to_owned),
        messages: if features.is_empty() {
            vec!["WARNING 000117: Warning empty output generated.".into()]
        } else {
            Vec::new()
        },
        features,
    })
}

/// What of a meeting of outlines also lies on `g`'s outline: its points on it, and
/// where its shared stretches meet it.
fn narrow_meeting(
    m: &geoforge_geometry::Meeting,
    g: &geoforge_core::Geometry,
    tol: f64,
) -> Result<geoforge_geometry::Meeting> {
    let outline = geoforge_geometry::outline(g);
    let paths: Vec<Vec<[f64; 2]>> = outline
        .part_ranges()
        .map(|r| outline.xy[r].to_vec())
        .filter(|p| p.len() >= 2)
        .collect();
    let mut out = geoforge_geometry::Meeting::default();
    let push = |out: &mut geoforge_geometry::Meeting, p: [f64; 2]| {
        if !out
            .points
            .iter()
            .any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) <= tol)
        {
            out.points.push(p);
        }
    };
    for &p in &m.points {
        if paths
            .iter()
            .any(|path| geoforge_geometry::on_path(p, path, tol, false))
        {
            push(&mut out, p);
        }
    }
    for s in &m.shared {
        let Some(stretch) = geoforge_geometry::paths_geometry(vec![s.clone()]) else {
            continue;
        };
        let sub = geoforge_geometry::meeting(&stretch, g, tol)?;
        for p in sub.points {
            push(&mut out, p);
        }
        out.shared.extend(sub.shared);
    }
    let shared = out.shared.clone();
    out.points.retain(|&p| {
        !shared
            .iter()
            .any(|s| geoforge_geometry::on_path(p, s, tol, false))
    });
    Ok(out)
}

fn intersect_input(inputs: &[NamedInput]) -> Result<ToolOutput> {
    if inputs.len() < 2 {
        return Err(Error::Unsupported(
            "Intersect needs at least two inputs".into(),
        ));
    }
    let mut kind = GeometryKind::Polygon;
    for i in inputs {
        let k = i
            .features
            .geometry_kind()
            .ok_or_else(|| Error::Unsupported(format!("{} has no geometry", i.name)))?;
        if dimension(k) < dimension(kind) {
            kind = k;
        }
    }
    let data: Vec<Vec<Feature>> = inputs
        .iter()
        .map(|i| load(i.features))
        .collect::<Result<_>>()?;
    let attrs_of = |input: usize, fid: i64| -> Vec<Value> {
        let mut a = vec![Value::Integer(fid as i32)];
        a.extend(
            data[input]
                .iter()
                .find(|f| f.oid == fid)
                .map(|f| f.attributes.clone())
                .unwrap_or_default(),
        );
        a
    };
    // Polygon inputs are overlaid into cells first (ArcGIS splits overlapping
    // features of one input into separate regions, output once per feature).
    let poly_inputs: Vec<usize> = (0..inputs.len())
        .filter(|&i| inputs[i].features.geometry_kind() == Some(GeometryKind::Polygon))
        .collect();
    let other_inputs: Vec<usize> = (0..inputs.len())
        .filter(|i| !poly_inputs.contains(i))
        .collect();
    let overlay_input: Vec<Vec<(i64, geoforge_core::Geometry)>> = poly_inputs
        .iter()
        .map(|&i| {
            data[i]
                .iter()
                .map(|f| (f.oid, f.geometry.clone().unwrap()))
                .collect()
        })
        .collect();
    let cells: Vec<(geoforge_core::Geometry, Vec<Vec<i64>>)> =
        geoforge_geometry::polygon_overlay(&overlay_input)?
            .into_iter()
            .filter(|c| c.members.iter().all(|m| !m.is_empty()))
            .filter_map(|c| geoforge_geometry::cell_geometry(&c).map(|g| (g, c.members)))
            .collect();
    // Combine with the lower-dimensional inputs feature by feature.
    // (pass: overlapping cell?, fids in input order, geometry)
    let mut rows: Vec<(bool, Vec<i64>, geoforge_core::Geometry)> = Vec::new();
    let mut partial: Vec<(Vec<(usize, i64)>, geoforge_core::Geometry)> = match other_inputs.first()
    {
        None => Vec::new(),
        Some(&first) => data[first]
            .iter()
            .map(|f| (vec![(first, f.oid)], f.geometry.clone().unwrap()))
            .collect(),
    };
    for &i in other_inputs.iter().skip(1) {
        let index =
            geoforge_geometry::EnvelopeIndex::new(data[i].iter().map(|f| f.geometry.as_ref()));
        let mut next = Vec::new();
        for (fids, g) in &partial {
            for k in index.candidates(&g.curve_envelope(), 0.0) {
                let f = &data[i][k];
                if let Some(common) =
                    geoforge_geometry::intersection(g, f.geometry.as_ref().unwrap())?
                {
                    let mut ids = fids.clone();
                    ids.push((i, f.oid));
                    next.push((ids, common));
                }
            }
        }
        partial = next;
    }
    let expand = |cell_members: &[Vec<i64>]| -> Vec<Vec<i64>> {
        let mut combos: Vec<Vec<i64>> = vec![Vec::new()];
        for m in cell_members {
            combos = combos
                .into_iter()
                .flat_map(|c| {
                    m.iter().map(move |&x| {
                        let mut c = c.clone();
                        c.push(x);
                        c
                    })
                })
                .collect();
        }
        combos
    };
    let cell_index = geoforge_geometry::EnvelopeIndex::new(cells.iter().map(|(g, _)| Some(g)));
    if other_inputs.is_empty() {
        for (g, members) in &cells {
            let overlapping = members.iter().any(|m| m.len() > 1);
            for fids in expand(members) {
                rows.push((overlapping, fids, g.clone()));
            }
        }
    } else {
        for (ids, g) in &partial {
            let pieces: Vec<(geoforge_core::Geometry, Vec<Vec<i64>>)> = if poly_inputs.is_empty() {
                vec![(g.clone(), Vec::new())]
            } else {
                let mut v = Vec::new();
                for k in cell_index.candidates(&g.curve_envelope(), 0.0) {
                    let (cell, members) = &cells[k];
                    if let Some(common) = geoforge_geometry::intersection(g, cell)? {
                        v.push((common, members.clone()));
                    }
                }
                v
            };
            for (piece, members) in pieces {
                for cell_fids in expand(&members) {
                    // FIDs in input order.
                    let mut fids = vec![0i64; inputs.len()];
                    for &(i, oid) in ids {
                        fids[i] = oid;
                    }
                    for (k, &i) in poly_inputs.iter().enumerate() {
                        fids[i] = cell_fids[k];
                    }
                    // Pieces in an overlap of one input's polygons come later.
                    let overlapping = members.iter().any(|m| m.len() > 1);
                    rows.push((overlapping, fids, piece.clone()));
                }
            }
        }
    }
    if dimension(kind) == 0 {
        rows.sort_by(|a, b| {
            let (pa, pb) = (
                a.2.xy.first().copied().unwrap_or_default(),
                b.2.xy.first().copied().unwrap_or_default(),
            );
            pa[1].total_cmp(&pb[1]).then(pa[0].total_cmp(&pb[0]))
        });
    } else {
        // FIDs of lower-dimension inputs come first in the order.
        let mut order: Vec<usize> = (0..inputs.len()).collect();
        order.sort_by_key(|&i| {
            dimension(
                inputs[i]
                    .features
                    .geometry_kind()
                    .unwrap_or(GeometryKind::Point),
            )
        });
        let key = |fids: &[i64]| -> Vec<i64> { order.iter().map(|&i| fids[i]).collect() };
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(key(&a.1).cmp(&key(&b.1))));
    }
    // ArcGIS cracks lines wherever two input lines meet.
    let nodes = if kind == GeometryKind::Polyline {
        Some(geoforge_geometry::LineNodes::new(
            data.iter().flatten().filter_map(|f| f.geometry.as_ref()),
            default_xy_tolerance(inputs[0].features.spatial_reference_wkt()),
        )?)
    } else {
        None
    };
    let (has_z, has_m) = zm_of(inputs);
    let by_oid: Vec<std::collections::HashMap<i64, usize>> = data
        .iter()
        .map(|fs| fs.iter().enumerate().map(|(k, f)| (f.oid, k)).collect())
        .collect();
    let partial: Vec<(geoforge_core::Geometry, Vec<Value>)> = rows
        .into_iter()
        .map(|(_, fids, g)| {
            (
                {
                    let mut g = nodes.as_ref().map_or(g.clone(), |n| n.crack(&g));
                    if has_z || has_m {
                        // Z and M from the features combined, in input order.
                        let sources: Vec<&geoforge_core::Geometry> = fids
                            .iter()
                            .enumerate()
                            .filter_map(|(i, &fid)| {
                                by_oid[i]
                                    .get(&fid)
                                    .and_then(|&k| data[i][k].geometry.as_ref())
                            })
                            .collect();
                        zm::restore_from(&mut g, &sources, has_z, has_m, true);
                    }
                    g
                },
                fids.iter()
                    .enumerate()
                    .flat_map(|(i, &fid)| attrs_of(i, fid))
                    .collect(),
            )
        })
        .collect();
    let features: Vec<Feature> = partial
        .into_iter()
        .enumerate()
        .map(|(i, (g, attributes))| Feature {
            oid: i as i64,
            geometry: Some(g),
            attributes,
        })
        .collect();
    Ok(ToolOutput {
        kind,
        has_z,
        has_m,
        fields: overlay_fields(inputs),
        spatial_reference_wkt: inputs[0]
            .features
            .spatial_reference_wkt()
            .map(str::to_owned),
        messages: if features.is_empty() {
            vec!["WARNING 000117: Warning empty output generated.".into()]
        } else {
            Vec::new()
        },
        features,
    })
}

/// `arcpy.analysis.Erase(in_features, erase_features, out)`: the parts of input
/// features outside the (dissolved) erase polygons; input schema and order.
pub fn erase(input: &dyn FeatureClass, erase_features: &dyn FeatureClass) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    if erase_features.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(Error::Unsupported("erase features must be polygons".into()));
    }
    let erase_geoms: Vec<_> = erase_features
        .features()
        .filter_map(|f| f.map(|f| f.geometry).transpose())
        .collect::<Result<_>>()?;
    let area = ClipArea::new(erase_geoms.iter())?;
    let tol = default_xy_tolerance(input.spatial_reference_wkt());
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if let Some(mut out) = geoforge_geometry::erase(g, &area)? {
            if kind == GeometryKind::Polyline {
                out = geoforge_geometry::LineNodes::new([g], tol)?.crack(&out);
            }
            zm::restore_from(&mut out, &[g], input.has_z(), input.has_m(), true);
            features.push(Feature {
                oid: f.oid,
                geometry: Some(out),
                attributes: f.attributes,
            });
        }
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        messages: if features.is_empty() {
            vec!["WARNING 000117: Warning empty output generated.".into()]
        } else {
            Vec::new()
        },
        features,
    })
}

/// `arcpy.analysis.Union([in1, in2, ...], out)` with `join_attributes="ALL"`: the
/// planar overlay of polygon inputs. A region covered by several features of one
/// input is output once per feature. `FID_<name>` is -1 where an input does not
/// cover a region. Order (as ArcGIS): regions covered by at most one feature per
/// input first, then the rest; within each, by which inputs cover them (first input
/// only, second only, both, ...), then by FIDs.
pub fn union(inputs: &[NamedInput]) -> Result<ToolOutput> {
    union_with(inputs, JoinAttributes::All, true)
}

/// Removes the rings of a polygon whose average width (area over half the perimeter)
/// is below `tolerance`.
fn drop_slivers(g: &mut geoforge_core::Geometry, tolerance: f64) {
    if g.z.is_some() || g.m.is_some() || g.has_curves() {
        return;
    }
    let rings: Vec<std::ops::Range<usize>> = g.part_ranges().collect();
    let mut xy = Vec::new();
    let mut parts = Vec::new();
    for r in rings {
        let ring = &g.xy[r];
        let area = geoforge_core::geometry::ring_signed_area2(ring).abs() / 2.0;
        let perimeter: f64 = ring
            .windows(2)
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum();
        if perimeter > 0.0 && area / (perimeter / 2.0) < tolerance {
            continue;
        }
        let ring = tidy_ring(ring, tolerance);
        if ring.len() < 4 {
            continue;
        }
        parts.push(xy.len() as u32);
        xy.extend_from_slice(&ring);
    }
    g.parts = parts;
    g.xy = xy;
}

/// A closed ring without vertices within `tolerance` of the previous one or of the
/// line through their neighbours (what overlay noise leaves on straight edges).
fn tidy_ring(ring: &[[f64; 2]], tolerance: f64) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = ring[..ring.len() - 1].to_vec();
    let dist = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    loop {
        let n = pts.len();
        if n < 3 {
            break;
        }
        let mut removed = false;
        for i in 0..n {
            let (prev, p, next) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let len = dist(prev, next);
            let off_line = if len == 0.0 {
                dist(prev, p)
            } else {
                ((next[0] - prev[0]) * (prev[1] - p[1]) - (prev[0] - p[0]) * (next[1] - prev[1]))
                    .abs()
                    / len
            };
            // Within noise of the line through its neighbours: collinear, or a spike.
            if dist(prev, p) < tolerance || off_line < tolerance {
                pts.remove(i);
                removed = true;
                break;
            }
        }
        if !removed {
            break;
        }
    }
    let first = pts[0];
    pts.push(first);
    pts
}

/// `arcpy.analysis.Update(in_features, update_features, out)` (`BORDERS`): the input
/// polygons without the area the update polygons cover, in input order, followed by the
/// update polygons. The output has the input's fields; an update polygon carries the
/// values of the fields it shares by name (the others are null).
pub fn update(input: &dyn FeatureClass, update_features: &dyn FeatureClass) -> Result<ToolOutput> {
    let mut out = erase(input, update_features)?;
    let positions: Vec<Option<usize>> = out
        .fields
        .iter()
        .map(|f| {
            update_features
                .fields()
                .iter()
                .position(|g| g.name.eq_ignore_ascii_case(&f.name))
        })
        .collect();
    for f in update_features.features() {
        let f = f?;
        out.features.push(Feature {
            oid: out.features.len() as i64,
            attributes: positions
                .iter()
                .map(|p| p.map_or(Value::Null, |p| f.attributes[p].clone()))
                .collect(),
            geometry: f.geometry,
        });
    }
    Ok(out)
}

/// `arcpy.analysis.SymDiff(in_features, update_features, out)`: the parts of either
/// input the other does not cover, with the attributes of both inputs (FID -1 and
/// empty values for the one that is absent). These are the rows of Union covered by
/// one input only, in Union's order.
pub fn symmetrical_difference(inputs: &[NamedInput]) -> Result<ToolOutput> {
    let [a, _] = inputs else {
        return Err(Error::Unsupported(
            "Symmetrical Difference takes exactly two inputs".into(),
        ));
    };
    let mut out = union(inputs)?;
    let b_fid = 1 + a.features.fields().len();
    let mut kept = Vec::new();
    for mut f in std::mem::take(&mut out.features) {
        let absent_a = matches!(f.attributes[0], Value::Integer(-1));
        let absent_b = matches!(f.attributes[b_fid], Value::Integer(-1));
        if absent_a == absent_b {
            continue;
        }
        // Slivers narrower than the XY tolerance (edges that coincide within it)
        // are not parts in ArcGIS's result.
        if let Some(g) = &mut f.geometry {
            drop_slivers(g, geoforge_geometry::XY_TOLERANCE);
            if g.xy.is_empty() {
                continue;
            }
        }
        kept.push(Feature {
            oid: kept.len() as i64,
            ..f
        });
    }
    out.features = kept;
    Ok(out)
}

/// `arcpy.analysis.Union(inputs, out, join_attributes, "", gaps)`. With `NO_GAPS`
/// (`gaps` false) each area enclosed by the inputs but covered by none is added
/// last, with FID -1 for every input.
pub fn union_with(inputs: &[NamedInput], join: JoinAttributes, gaps: bool) -> Result<ToolOutput> {
    let mut out = union_all(inputs)?;
    if !gaps {
        let geoms: Vec<geoforge_core::Geometry> = inputs
            .iter()
            .map(|i| load(i.features))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .filter_map(|f| f.geometry)
            .collect();
        let row: Vec<Value> = inputs
            .iter()
            .flat_map(|i| {
                std::iter::once(Value::Integer(-1))
                    .chain(std::iter::repeat_n(Value::Null, i.features.fields().len()))
            })
            .collect();
        for gap in geoforge_geometry::enclosed_gaps(geoms.iter())? {
            out.features.push(Feature {
                oid: out.features.len() as i64,
                geometry: Some(gap),
                attributes: row.clone(),
            });
        }
    }
    apply_join_attributes(&mut out, inputs, join);
    Ok(out)
}

fn union_all(inputs: &[NamedInput]) -> Result<ToolOutput> {
    let data: Vec<Vec<(i64, geoforge_core::Geometry, Vec<Value>)>> = inputs
        .iter()
        .map(|i| {
            Ok(load(i.features)?
                .into_iter()
                .map(|f| (f.oid, f.geometry.unwrap(), f.attributes))
                .collect())
        })
        .collect::<Result<_>>()?;
    let geometry_inputs: Vec<Vec<(i64, geoforge_core::Geometry)>> = data
        .iter()
        .map(|fs| fs.iter().map(|(o, g, _)| (*o, g.clone())).collect())
        .collect();
    let cells = geoforge_geometry::polygon_overlay(&geometry_inputs)?;

    // (pass, coverage mask, FIDs, geometry, attributes)
    let mut rows = Vec::new();
    for cell in &cells {
        let Some(geom) = geoforge_geometry::cell_geometry(cell) else {
            continue;
        };
        let overlapping = cell.members.iter().any(|m| m.len() > 1);
        let mask: u64 = cell
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.is_empty())
            .map(|(i, _)| 1u64 << i)
            .sum();
        // Cartesian product over inputs of their covering features (or none).
        let mut combos: Vec<Vec<i64>> = vec![Vec::new()];
        for m in &cell.members {
            let choices: Vec<i64> = if m.is_empty() { vec![-1] } else { m.clone() };
            combos = combos
                .into_iter()
                .flat_map(|c| {
                    choices.iter().map(move |&x| {
                        let mut c = c.clone();
                        c.push(x);
                        c
                    })
                })
                .collect();
        }
        for fids in combos {
            let mut attrs = Vec::new();
            for (i, &fid) in fids.iter().enumerate() {
                attrs.push(Value::Integer(fid as i32));
                match data[i].iter().find(|(o, _, _)| *o == fid) {
                    Some((_, _, a)) => attrs.extend(a.iter().cloned()),
                    None => attrs.extend(std::iter::repeat_n(
                        Value::Null,
                        inputs[i].features.fields().len(),
                    )),
                }
            }
            rows.push((overlapping, mask, fids, geom.clone(), attrs));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (has_z, has_m) = zm_of(inputs);
    if has_z || has_m {
        // Z and M from the covering features in input order, then from any
        // other feature whose outline passes there (a cell's edge can be an
        // uncovering neighbour's).
        let indexes: Vec<geoforge_geometry::EnvelopeIndex> = data
            .iter()
            .map(|fs| geoforge_geometry::EnvelopeIndex::new(fs.iter().map(|f| Some(&f.1))))
            .collect();
        for r in &mut rows {
            let env = r.3.envelope();
            let mut sources: Vec<&geoforge_core::Geometry> = r
                .2
                .iter()
                .enumerate()
                .filter_map(|(i, &fid)| data[i].iter().find(|(o, _, _)| *o == fid).map(|f| &f.1))
                .collect();
            for (i, index) in indexes.iter().enumerate() {
                for k in index.candidates(&env, 0.0) {
                    if data[i][k].0 != r.2[i] {
                        sources.push(&data[i][k].1);
                    }
                }
            }
            zm::restore_from(&mut r.3, &sources, has_z, has_m, true);
        }
    }
    let features = rows
        .into_iter()
        .enumerate()
        .map(|(i, r)| Feature {
            oid: i as i64,
            geometry: Some(r.3),
            attributes: r.4,
        })
        .collect();
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z,
        has_m,
        fields: overlay_fields(inputs),
        spatial_reference_wkt: inputs[0]
            .features
            .spatial_reference_wkt()
            .map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Dissolve statistics (`statistics_fields`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statistic {
    Sum,
    Mean,
    Min,
    Max,
    Range,
    Std,
    Count,
    First,
    Last,
    Median,
    Variance,
    Unique,
}

impl Statistic {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "SUM" => Self::Sum,
            "MEAN" => Self::Mean,
            "MIN" => Self::Min,
            "MAX" => Self::Max,
            "RANGE" => Self::Range,
            "STD" => Self::Std,
            "COUNT" => Self::Count,
            "FIRST" => Self::First,
            "LAST" => Self::Last,
            "MEDIAN" => Self::Median,
            "VARIANCE" => Self::Variance,
            "UNIQUE" => Self::Unique,
            _ => return None,
        })
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Sum => "SUM",
            Self::Mean => "MEAN",
            Self::Min => "MIN",
            Self::Max => "MAX",
            Self::Range => "RANGE",
            Self::Std => "STD",
            Self::Count => "COUNT",
            Self::First => "FIRST",
            Self::Last => "LAST",
            Self::Median => "MEDIAN",
            Self::Variance => "VARIANCE",
            Self::Unique => "UNIQUE",
        }
    }

    /// The output field `PREFIX_FIELD`: counts are integers, first/last/min/max keep
    /// the field's type, the rest are doubles.
    fn field(self, src: &Field) -> Field {
        let field_type = match self {
            Statistic::Count | Statistic::Unique => FieldType::Integer,
            Statistic::Min | Statistic::Max | Statistic::First | Statistic::Last => src.field_type,
            _ => FieldType::Double,
        };
        let mut f = Field::new(format!("{}_{}", self.prefix(), src.name), field_type);
        f.length = src.length;
        f
    }

    /// The statistic of a group's values (in feature order, nulls included).
    /// `sample`: standard deviation and variance over n - 1 (Summary Statistics)
    /// rather than n (Dissolve).
    fn compute(self, all: &[&Value], sample: bool) -> Value {
        let values: Vec<&Value> = all.iter().copied().filter(|v| !v.is_null()).collect();
        let mut nums: Vec<f64> = values.iter().filter_map(|v| number(v)).collect();
        let n = nums.len() as f64;
        let mean = nums.iter().sum::<f64>() / n;
        let or_null = |ok: bool, v: f64| if ok { Value::Double(v) } else { Value::Null };
        match self {
            Statistic::Count => Value::Integer(values.len() as i32),
            Statistic::Unique => {
                let mut distinct: Vec<&Value> = Vec::new();
                for v in &values {
                    if !distinct.contains(v) {
                        distinct.push(v);
                    }
                }
                Value::Integer(distinct.len() as i32)
            }
            Statistic::Sum => Value::Double(nums.iter().sum()),
            Statistic::Mean => or_null(!nums.is_empty(), mean),
            Statistic::Range => nums
                .iter()
                .copied()
                .reduce(f64::max)
                .zip(nums.iter().copied().reduce(f64::min))
                .map_or(Value::Null, |(a, b)| Value::Double(a - b)),
            Statistic::Std | Statistic::Variance => {
                let denominator = if sample { n - 1.0 } else { n };
                let var = if sample && n < 2.0 {
                    0.0
                } else {
                    nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / denominator
                };
                or_null(
                    !nums.is_empty(),
                    if self == Statistic::Std {
                        var.sqrt()
                    } else {
                        var
                    },
                )
            }
            Statistic::Median => {
                nums.sort_by(f64::total_cmp);
                let k = nums.len();
                if k == 0 {
                    Value::Null
                } else if k % 2 == 1 {
                    Value::Double(nums[k / 2])
                } else {
                    Value::Double((nums[k / 2 - 1] + nums[k / 2]) / 2.0)
                }
            }
            Statistic::Min => values
                .iter()
                .copied()
                .min_by(|a, b| compare(a, b))
                .cloned()
                .unwrap_or(Value::Null),
            Statistic::Max => values
                .iter()
                .copied()
                .max_by(|a, b| compare(a, b))
                .cloned()
                .unwrap_or(Value::Null),
            Statistic::First => all.first().map_or(Value::Null, |v| (*v).clone()),
            Statistic::Last => all.last().map_or(Value::Null, |v| (*v).clone()),
        }
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::SmallInteger(n) => Some(f64::from(*n)),
        Value::Integer(n) => Some(f64::from(*n)),
        Value::BigInteger(n) => Some(*n as f64),
        Value::Single(n) => Some(f64::from(*n)),
        Value::Double(n) => Some(*n),
        _ => None,
    }
}

/// Orders values for grouping/sorting: nulls first, then numbers, then text/dates.
fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        _ => match (number(a), number(b)) {
            (Some(x), Some(y)) => x.total_cmp(&y),
            _ => format!("{a:?}").cmp(&format!("{b:?}")),
        },
    }
}

/// `arcpy.management.Dissolve(in, out, dissolve_field, statistics_fields,
/// multi_part)`: features grouped by the dissolve fields' values (groups in
/// ascending order), geometries merged, statistics computed.
pub fn dissolve(
    input: &dyn FeatureClass,
    dissolve_fields: &[String],
    statistics: &[(String, Statistic)],
    multi_part: bool,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let index = |name: &str| {
        input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "ERROR 000728: Field {name} does not exist within table"
                ))
            })
    };
    let keys: Vec<usize> = dissolve_fields
        .iter()
        .map(|f| index(f))
        .collect::<Result<_>>()?;
    let stats: Vec<(usize, Statistic)> = statistics
        .iter()
        .map(|(f, s)| Ok((index(f)?, *s)))
        .collect::<Result<_>>()?;

    let mut groups: Vec<(Vec<Value>, Vec<Feature>)> = Vec::new();
    for f in input.features() {
        let f = f?;
        if f.geometry.is_none() {
            continue;
        }
        let key: Vec<Value> = keys.iter().map(|&i| f.attributes[i].clone()).collect();
        match groups.iter_mut().find(|(k, _)| k == &key) {
            Some((_, members)) => members.push(f),
            None => groups.push((key, vec![f])),
        }
    }
    groups.sort_by(|a, b| {
        a.0.iter()
            .zip(&b.0)
            .map(|(x, y)| compare(x, y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut fields: Vec<Field> = keys.iter().map(|&i| input.fields()[i].clone()).collect();
    for &(i, s) in &stats {
        fields.push(s.field(&input.fields()[i]));
    }
    let out_kind = if dimension(kind) == 0 && multi_part {
        GeometryKind::Multipoint
    } else {
        kind
    };

    let mut features = Vec::new();
    for (key, members) in &groups {
        let Some(g) =
            geoforge_geometry::dissolve(members.iter().filter_map(|f| f.geometry.as_ref()))?
        else {
            continue;
        };
        let mut attrs = key.clone();
        for &(i, s) in &stats {
            let values: Vec<&Value> = members.iter().map(|f| &f.attributes[i]).collect();
            attrs.push(s.compute(&values, false));
        }
        if multi_part {
            features.push(Feature {
                oid: features.len() as i64,
                geometry: Some(g),
                attributes: attrs,
            });
        } else {
            // ArcGIS emits the parts in the order of the source features (and their
            // parts) they came from.
            let mut parts: Vec<(usize, usize, geoforge_core::Geometry)> =
                geoforge_geometry::explode(&g)
                    .into_iter()
                    .map(|part| {
                        let pe = part.envelope();
                        let within = |e: &geoforge_core::Envelope| {
                            let tol = 1e-6 * (pe.xmax - pe.xmin).max(pe.ymax - pe.ymin).max(1.0);
                            e.xmin >= pe.xmin - tol
                                && e.xmax <= pe.xmax + tol
                                && e.ymin >= pe.ymin - tol
                                && e.ymax <= pe.ymax + tol
                        };
                        let origin = members
                            .iter()
                            .enumerate()
                            .find_map(|(mi, m)| {
                                let mg = m.geometry.as_ref()?;
                                geoforge_geometry::explode(mg)
                                    .iter()
                                    .position(|p| within(&p.envelope()))
                                    .map(|pi| (mi, pi))
                            })
                            .unwrap_or((usize::MAX, 0));
                        (origin.0, origin.1, part)
                    })
                    .collect();
            parts.sort_by_key(|p| (p.0, p.1));
            for (_, _, part) in parts {
                features.push(Feature {
                    oid: features.len() as i64,
                    geometry: Some(part),
                    attributes: attrs.clone(),
                });
            }
        }
    }
    Ok(ToolOutput {
        kind: out_kind,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// `arcpy.analysis.Identity(in_features, identity_features, out)` with
/// `join_attributes="ALL"`: the input features split by the identity polygons,
/// each piece carrying the attributes of the identity feature it falls in (or
/// `FID_<name>` = -1 outside them all). Polygon inputs give Union's cells covered
/// by the input, in Union's order; line pieces outside the identity features come
/// first, then one (multipart) piece per input/identity pair, by FIDs; points are
/// ordered by Y.
pub fn identity(input: NamedInput, identity_features: NamedInput) -> Result<ToolOutput> {
    identity_with(input, identity_features, &IdentityOptions::default())
}

/// Identity's `join_attributes` and `relationship` parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityOptions {
    /// Applies to the identity features' columns only, except that NO_FID also
    /// drops the input's FID.
    pub join_attributes: JoinAttributes,
    /// KEEP_RELATIONSHIPS (line and point input): each identity column becomes a
    /// `LEFT_`/`RIGHT_` pair; uncovered pieces get 0 or "".
    pub keep_relationships: bool,
}

/// `arcpy.analysis.Identity(in, identity, out, join_attributes, "", relationship)`.
pub fn identity_with(
    input: NamedInput,
    identity_features: NamedInput,
    opts: &IdentityOptions,
) -> Result<ToolOutput> {
    let input_cols = 1 + input.features.fields().len();
    let id_name = identity_features.name.clone();
    let id_fields = identity_features.features.fields().to_vec();
    // Polygon inputs: the option is ignored. Points get the columns, all 0 (a
    // point has no sides).
    let kind = input.features.geometry_kind();
    let relationships = opts.keep_relationships && kind != Some(GeometryKind::Polygon);
    let sideless = kind != Some(GeometryKind::Polyline);
    let id_cols_len = 1 + id_fields.len();
    let mut out = identity_all(input, identity_features, relationships && !sideless)?;
    let join = opts.join_attributes;
    // (output field, source column, a LEFT/RIGHT column). With relationships the
    // rows carry the left polygon's columns, then the right one's.
    let mut cols: Vec<(Field, usize, bool)> = Vec::new();
    for (k, f) in out.fields[..input_cols].iter().enumerate() {
        if k > 0 || join != JoinAttributes::NoFid {
            cols.push((f.clone(), k, false));
        }
    }
    let mut id_cols: Vec<(Field, usize)> = Vec::new();
    if join != JoinAttributes::NoFid {
        id_cols.push((out.fields[input_cols].clone(), input_cols));
    }
    if join != JoinAttributes::OnlyFid {
        id_cols.extend(
            out.fields[input_cols + 1..]
                .iter()
                .cloned()
                .zip(input_cols + 1..),
        );
    }
    for (f, k) in id_cols {
        if relationships {
            let base = if k == input_cols {
                id_name.clone()
            } else {
                id_fields[k - input_cols - 1].name.clone()
            };
            let right = if sideless { 0 } else { id_cols_len };
            for (side, offset) in [("LEFT_", 0), ("RIGHT_", right)] {
                let mut g = f.clone();
                g.name = format!("{side}{base}");
                cols.push((g, k + offset, true));
            }
        } else {
            cols.push((f, k, false));
        }
    }
    out.fields = cols.iter().map(|c| c.0.clone()).collect();
    // A side without a polygon: 0 and "" (ONLY_FID: FID -1); points: all 0.
    for feat in &mut out.features {
        let missing = |k: usize| {
            let fid = if k >= input_cols + id_cols_len && !sideless {
                input_cols + id_cols_len
            } else {
                input_cols
            };
            feat.attributes[fid] == Value::Integer(-1)
        };
        feat.attributes = cols
            .iter()
            .map(|(field, k, side)| {
                let only_fid = join == JoinAttributes::OnlyFid && !sideless;
                if *side && (sideless || (missing(*k) && !only_fid)) {
                    zero_value(field.field_type)
                } else {
                    feat.attributes[*k].clone()
                }
            })
            .collect();
    }
    Ok(out)
}

/// The identity polygons split where they overlap: (cell, covering FIDs). Only
/// groups of polygons whose interiors overlap are overlaid; any other polygon
/// is its own cell.
fn identity_cells(polys: &Indexed) -> Result<Vec<(geoforge_core::Geometry, Vec<i64>)>> {
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let n = polys.features.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        let a = polys.features[i].geometry.as_ref().unwrap();
        for j in polys.candidates(a, 0.0) {
            if j <= i {
                continue;
            }
            let b = polys.features[j].geometry.as_ref().unwrap();
            if geoforge_geometry::relate_matches(a, b, "T********")? {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for i in 0..n {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut cells = Vec::new();
    for members in groups.into_values() {
        if let [i] = members[..] {
            let f = &polys.features[i];
            cells.push((f.geometry.clone().unwrap(), vec![f.oid]));
            continue;
        }
        let input: Vec<(i64, geoforge_core::Geometry)> = members
            .iter()
            .map(|&i| {
                let f = &polys.features[i];
                (f.oid, f.geometry.clone().unwrap())
            })
            .collect();
        for c in geoforge_geometry::polygon_overlay(&[input])? {
            if let Some(g) = geoforge_geometry::cell_geometry(&c) {
                cells.push((g, c.members[0].clone()));
            }
        }
    }
    Ok(cells)
}

/// Line Identity rows of one input feature. The line is cut where it meets
/// other input lines or an identity cell's outline; each piece gets the
/// polygons left and right of it (L, R) and pieces are grouped by (L, R).
/// Pass: 0 uncovered, 1 a polygon on one side only (an outer edge), 2 inside a
/// polygon or along an edge two share, 3 inside an overlap. Without
/// relationships a group gives a row per polygon in L then R; with them a row
/// per (left, right) pair.
#[allow(clippy::type_complexity)]
fn identity_line_rows(
    f: &Feature,
    head: &[Value],
    splitter: &geoforge_geometry::LineNodes,
    nodes: &geoforge_geometry::LineNodes,
    cells_at: &dyn Fn([f64; 2]) -> Vec<i64>,
    relationships: bool,
    id_attrs: &dyn Fn(i64) -> Vec<Value>,
) -> Vec<(u8, i64, i64, i64, geoforge_core::Geometry, Vec<Value>)> {
    let g = f.geometry.as_ref().unwrap();
    let pieces = splitter.split(&nodes.split(g));
    let mut groups: Vec<((Vec<i64>, Vec<i64>), geoforge_core::Geometry)> = Vec::new();
    for r in pieces.part_ranges() {
        let path = &pieces.xy[r];
        let Some((mid, dir)) = halfway(path) else {
            continue;
        };
        let off = 10.0 * splitter.tolerance();
        let left = cells_at([mid[0] - dir[1] * off, mid[1] + dir[0] * off]);
        let right = cells_at([mid[0] + dir[1] * off, mid[1] - dir[0] * off]);
        let key = (left, right);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, geom)) => {
                geom.parts.push(geom.xy.len() as u32);
                geom.xy.extend_from_slice(path);
            }
            None => {
                let mut geom = pieces.clone();
                geom.parts = vec![0];
                geom.xy = path.to_vec();
                groups.push((key, geom));
            }
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    let mut rows = Vec::new();
    for ((left, right), geom) in groups {
        let pass = match (left.len(), right.len()) {
            (0, 0) => 0,
            (0, _) | (_, 0) => 1,
            (1, 1) => 2,
            _ => 3,
        };
        let geom = nodes.crack(&geom);
        let or_none = |v: &[i64]| if v.is_empty() { vec![-1] } else { v.to_vec() };
        let pairs: Vec<(i64, i64)> = if relationships {
            let (l, r) = (or_none(&left), or_none(&right));
            l.iter()
                .flat_map(|&a| r.iter().map(move |&b| (a, b)))
                .collect()
        } else {
            let mut polys: Vec<i64> = Vec::new();
            for &p in left.iter().chain(&right) {
                if !polys.contains(&p) {
                    polys.push(p);
                }
            }
            or_none(&polys).into_iter().map(|p| (p, p)).collect()
        };
        for (l, r) in pairs {
            let mut attrs = head.to_vec();
            attrs.extend(id_attrs(l));
            if relationships {
                attrs.extend(id_attrs(r));
            }
            rows.push((pass, f.oid, l, r, geom.clone(), attrs));
        }
    }
    rows
}

/// A splitter for one line: its own outline plus those of the cells near it.
fn splitter_inputs(
    g: &geoforge_core::Geometry,
    cell_index: &geoforge_geometry::EnvelopeIndex,
    cells: &[(geoforge_core::Geometry, Vec<i64>)],
    tol: f64,
) -> Result<geoforge_geometry::LineNodes> {
    let near: Vec<geoforge_core::Geometry> = cell_index
        .candidates(&g.curve_envelope(), tol)
        .into_iter()
        .map(|k| geoforge_geometry::outline(&cells[k].0))
        .collect();
    geoforge_geometry::LineNodes::new(std::iter::once(g).chain(near.iter()), tol)
}

/// The point halfway along a path and the unit direction of the line there.
fn halfway(path: &[[f64; 2]]) -> Option<([f64; 2], [f64; 2])> {
    let len = |w: &[[f64; 2]]| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
    let half = path.windows(2).map(len).sum::<f64>() / 2.0;
    let mut walked = 0.0;
    for w in path.windows(2) {
        let l = len(w);
        if l > 0.0 && walked + l >= half {
            let t = (half - walked) / l;
            let d = [(w[1][0] - w[0][0]) / l, (w[1][1] - w[0][1]) / l];
            return Some((
                [
                    w[0][0] + t * (w[1][0] - w[0][0]),
                    w[0][1] + t * (w[1][1] - w[0][1]),
                ],
                d,
            ));
        }
        walked += l;
    }
    None
}

/// What ArcGIS writes for a missing related value: 0 or an empty string.
fn zero_value(t: FieldType) -> Value {
    match t {
        FieldType::SmallInteger => Value::SmallInteger(0),
        FieldType::Integer => Value::Integer(0),
        FieldType::BigInteger => Value::BigInteger(0),
        FieldType::Single => Value::Single(0.0),
        FieldType::Double => Value::Double(0.0),
        FieldType::String => Value::String(String::new()),
        _ => Value::Null,
    }
}

fn identity_all(
    input: NamedInput,
    identity_features: NamedInput,
    relationships: bool,
) -> Result<ToolOutput> {
    let kind = input
        .features
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    if identity_features.features.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(Error::Unsupported(
            "identity features must be polygons".into(),
        ));
    }
    if kind == GeometryKind::Polygon {
        let mut out = union_all(&[input, identity_features])?;
        out.features
            .retain(|f| f.attributes.first() != Some(&Value::Integer(-1)));
        for (i, f) in out.features.iter_mut().enumerate() {
            f.oid = i as i64;
        }
        return Ok(out);
    }
    let fields = overlay_fields(&[
        NamedInput {
            name: input.name.clone(),
            features: input.features,
        },
        NamedInput {
            name: identity_features.name.clone(),
            features: identity_features.features,
        },
    ]);
    let polys = Indexed::load(identity_features.features)?;
    // ArcGIS splits the identity polygons into cells where they overlap: a piece
    // in an overlap is written once per covering polygon, after all others.
    let cells = identity_cells(&polys)?;
    let cell_index = geoforge_geometry::EnvelopeIndex::new(cells.iter().map(|(g, _)| Some(g)));
    let by_oid: std::collections::HashMap<i64, usize> = polys
        .features
        .iter()
        .enumerate()
        .map(|(i, f)| (f.oid, i))
        .collect();
    let id_attrs = |fid: i64| -> Vec<Value> {
        let mut a = vec![Value::Integer(fid as i32)];
        match by_oid.get(&fid).map(|&i| &polys.features[i]) {
            Some(p) => a.extend(p.attributes.iter().cloned()),
            None => a.extend(std::iter::repeat_n(
                Value::Null,
                identity_features.features.fields().len(),
            )),
        }
        a
    };
    let lines = load(input.features)?;
    let input_geoms: std::collections::HashMap<i64, geoforge_core::Geometry> = lines
        .iter()
        .filter_map(|f| Some((f.oid, f.geometry.clone()?)))
        .collect();
    // ArcGIS cracks lines wherever two input lines meet.
    let nodes = geoforge_geometry::LineNodes::new(
        lines.iter().filter_map(|f| f.geometry.as_ref()),
        default_xy_tolerance(input.features.spatial_reference_wkt()),
    )?;
    let crack = |g: geoforge_core::Geometry| {
        if kind == GeometryKind::Polyline {
            nodes.crack(&g)
        } else {
            g
        }
    };
    // (pass, input FID, left FID, right FID, geometry, attributes); pass 0 is
    // uncovered, 1 inside one polygon, 2 inside an overlap.
    let mut rows = Vec::new();
    let tol = default_xy_tolerance(input.features.spatial_reference_wkt());
    // The identity polygons on each side of a line piece, probed just off its middle.
    let cells_at = |p: [f64; 2]| -> Vec<i64> {
        let pt = geoforge_core::Geometry::point(p[0], p[1]);
        cell_index
            .candidates(&pt.envelope(), 0.0)
            .into_iter()
            .find(|&k| geoforge_geometry::within(&pt, &cells[k].0).unwrap_or(false))
            .map_or_else(Vec::new, |k| cells[k].1.clone())
    };
    for f in lines {
        let g = f.geometry.as_ref().unwrap();
        let mut head = vec![Value::Integer(f.oid as i32)];
        head.extend(f.attributes.iter().cloned());
        if kind == GeometryKind::Polyline {
            rows.extend(identity_line_rows(
                &f,
                &head,
                &splitter_inputs(g, &cell_index, &cells, tol)?,
                &nodes,
                &cells_at,
                relationships,
                &id_attrs,
            ));
            continue;
        }
        // Only the identity features near this one can cover part of it.
        let near: Vec<&Feature> = polys
            .candidates(g, 0.0)
            .into_iter()
            .map(|i| &polys.features[i])
            .collect();
        let area = ClipArea::new(near.iter().filter_map(|p| p.geometry.as_ref()))?;
        if let Some(rest) = geoforge_geometry::erase(g, &area)? {
            let mut attrs = head.clone();
            attrs.extend(id_attrs(-1));
            if relationships {
                attrs.extend(id_attrs(-1));
            }
            rows.push((0, f.oid, -1, -1, crack(rest), attrs));
        }
        for k in cell_index.candidates(&g.curve_envelope(), 0.0) {
            let (cell, members) = &cells[k];
            let Some(common) = geoforge_geometry::intersection(g, cell)? else {
                continue;
            };
            let common = crack(common);
            let pass = if members.len() > 1 { 2 } else { 1 };
            let pairs: Vec<(i64, i64)> = if relationships {
                members
                    .iter()
                    .flat_map(|&l| members.iter().map(move |&r| (l, r)))
                    .collect()
            } else {
                members.iter().map(|&m| (m, m)).collect()
            };
            for (l, r) in pairs {
                let mut attrs = head.clone();
                attrs.extend(id_attrs(l));
                if relationships {
                    attrs.extend(id_attrs(r));
                }
                rows.push((pass, f.oid, l, r, common.clone(), attrs));
            }
        }
    }
    if dimension(kind) == 0 {
        rows.sort_by(|a, b| {
            let (pa, pb) = (
                a.4.xy.first().copied().unwrap_or_default(),
                b.4.xy.first().copied().unwrap_or_default(),
            );
            pa[1].total_cmp(&pb[1]).then(pa[0].total_cmp(&pb[0]))
        });
    } else {
        // Stable: a feature's rows keep their (L, R) order within a pass.
        rows.sort_by_key(|a| (a.0, a.1));
    }
    let (has_z, has_m) = (
        input.features.has_z() || identity_features.features.has_z(),
        input.features.has_m() || identity_features.features.has_m(),
    );
    if has_z || has_m {
        // Z and M from the input feature, then the identity polygon.
        for r in &mut rows {
            let sources: Vec<&geoforge_core::Geometry> = [
                input_geoms.get(&r.1),
                by_oid
                    .get(&r.2)
                    .and_then(|&k| polys.features[k].geometry.as_ref()),
            ]
            .into_iter()
            .flatten()
            .collect();
            zm::restore_from(&mut r.4, &sources, has_z, has_m, true);
        }
    }
    let features: Vec<Feature> = rows
        .into_iter()
        .enumerate()
        .map(|(i, r)| Feature {
            oid: i as i64,
            geometry: Some(r.4),
            attributes: r.5,
        })
        .collect();
    Ok(ToolOutput {
        kind,
        has_z,
        has_m,
        fields,
        spatial_reference_wkt: input.features.spatial_reference_wkt().map(str::to_owned),
        messages: if features.is_empty() {
            vec!["WARNING 000117: Warning empty output generated.".into()]
        } else {
            Vec::new()
        },
        features,
    })
}

/// An input with the path ArcGIS records for it (`MERGE_SRC`, `NEAR_FC`).
pub struct PathInput<'a> {
    pub source: String,
    pub features: &'a dyn FeatureClass,
}

/// `arcpy.management.Merge(inputs, out, "", add_source)` with the default field
/// mapping: fields of all inputs matched by name (first definition wins, in input
/// order), features of each input in turn, null where an input lacks a field.
/// `add_source` appends `MERGE_SRC` (text, 254) holding each feature's input path.
pub fn merge(inputs: &[PathInput], add_source: bool) -> Result<ToolOutput> {
    merge_with(inputs, add_source, None)
}

/// [`merge`] with a field mapping: the output fields are the mapping's, and each
/// input's rows take their values from the mapped source fields of that input
/// (found by table name; an input none of whose fields is mapped leaves the field
/// null), converted to the output field's type.
pub fn merge_with(
    inputs: &[PathInput],
    add_source: bool,
    field_map: Option<&[FieldMapEntry]>,
) -> Result<ToolOutput> {
    let first = inputs
        .first()
        .ok_or_else(|| Error::Unsupported("Merge needs at least one input".into()))?;
    let kind = first.features.geometry_kind();
    if inputs.iter().any(|i| i.features.geometry_kind() != kind) {
        return Err(Error::Unsupported(
            "ERROR 000898: all inputs must have the same geometry type".into(),
        ));
    }
    let mut fields: Vec<Field> = Vec::new();
    if let Some(map) = field_map {
        fields.extend(map.iter().map(|e| e.field.clone()));
    } else {
        for i in inputs {
            for f in i.features.fields() {
                if !fields.iter().any(|g| g.name.eq_ignore_ascii_case(&f.name)) {
                    fields.push(f.clone());
                }
            }
        }
    }
    let mut features = Vec::new();
    for i in inputs {
        // Per output field: the source position in this input, with the text span.
        let positions: Vec<Option<(usize, i64, i64)>> = match field_map {
            None => fields
                .iter()
                .map(|f| {
                    i.features
                        .fields()
                        .iter()
                        .position(|g| g.name.eq_ignore_ascii_case(&f.name))
                        .map(|p| (p, -1, -1))
                })
                .collect(),
            Some(map) => {
                let stem = table_stem(&i.source);
                map.iter()
                    .map(|e| {
                        e.sources
                            .iter()
                            .filter(|src| table_stem(&src.table) == stem)
                            .find_map(|src| {
                                i.features
                                    .fields()
                                    .iter()
                                    .position(|g| g.name.eq_ignore_ascii_case(&src.field))
                                    .map(|p| (p, src.start, src.end))
                            })
                    })
                    .collect()
            }
        };
        for f in i.features.features() {
            let f = f?;
            let mut attributes: Vec<Value> = positions
                .iter()
                .zip(&fields)
                .map(|(p, field)| match (p, field_map) {
                    (None, _) => Value::Null,
                    (Some((p, ..)), None) => f.attributes[*p].clone(),
                    (Some((p, a, b)), Some(_)) => {
                        fieldmap::coerce(&fieldmap::substring(&f.attributes[*p], *a, *b), field)
                    }
                })
                .collect();
            if add_source {
                attributes.push(Value::String(i.source.clone()));
            }
            features.push(Feature {
                oid: features.len() as i64,
                geometry: f.geometry,
                attributes,
            });
        }
    }
    if add_source {
        let mut f = Field::new("MERGE_SRC", FieldType::String);
        f.length = 254;
        fields.push(f);
    }
    Ok(ToolOutput {
        kind: kind.unwrap_or(GeometryKind::Point),
        has_z: inputs.iter().any(|i| i.features.has_z()),
        has_m: inputs.iter().any(|i| i.features.has_m()),
        fields,
        spatial_reference_wkt: first.features.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Appends `f` to `fields`, suffixing `_1`, `_2`, ... while its name is taken.
/// Adds a field, renamed as ArcGIS's overlays rename repeated names: NAME, NAME_1,
/// NAME_12, NAME_12_13, NAME_12_13_14, ...
fn push_unique(fields: &mut Vec<Field>, mut f: Field) {
    let base = f.name.clone();
    let mut n = 1;
    while fields.iter().any(|g| g.name.eq_ignore_ascii_case(&f.name)) {
        f.name = match n {
            1 => format!("{base}_1"),
            _ => {
                let mut name = format!("{base}_12");
                for k in 3..=n {
                    name.push_str(&format!("_1{k}"));
                }
                name
            }
        };
        n += 1;
    }
    fields.push(f);
}

/// Features loaded with an envelope index over their shapes.
struct Indexed {
    features: Vec<Feature>,
    index: geoforge_geometry::EnvelopeIndex,
}

impl Indexed {
    fn load(fc: &dyn FeatureClass) -> Result<Self> {
        Ok(Self::new(load(fc)?))
    }

    fn new(features: Vec<Feature>) -> Self {
        let index =
            geoforge_geometry::EnvelopeIndex::new(features.iter().map(|f| f.geometry.as_ref()));
        Self { features, index }
    }

    /// The closest feature to `g` (lowest FID on ties), within `radius` if given.
    fn closest(
        &self,
        g: &geoforge_core::Geometry,
        radius: Option<f64>,
        method: NearMethod,
    ) -> Result<Option<(usize, geoforge_geometry::Nearest)>> {
        match method {
            NearMethod::Planar => {
                self.index
                    .closest(g, |i| self.features[i].geometry.as_ref(), radius)
            }
            NearMethod::Geodesic => {
                self.index
                    .closest_geodesic(g, |i| self.features[i].geometry.as_ref(), radius)
            }
        }
    }

    /// Positions of the features whose envelopes come within `pad` of `g`'s.
    fn candidates(&self, g: &geoforge_core::Geometry, pad: f64) -> Vec<usize> {
        self.index.candidates(&g.curve_envelope(), pad)
    }
}

/// `arcpy.analysis.Near(in_features, near_features, search_radius, location,
/// angle)`, planar: adds `NEAR_FID` and `NEAR_DIST` (and `NEAR_X`, `NEAR_Y`,
/// `NEAR_ANGLE`) to a copy of the input. The location is the closest point on the
/// near feature; the angle runs from the input's closest point to it, in degrees
/// counter-clockwise from east (-180..180], 0 when they touch. Features with
/// nothing within the radius get -1 in every field. With several near feature
/// classes, `NEAR_FC` (text, 254) names the one the closest feature is in (the
/// earlier class wins a tie).
pub fn near(
    input: &dyn FeatureClass,
    near_features: &[PathInput],
    search_radius: Option<f64>,
    location: bool,
    angle: bool,
) -> Result<ToolOutput> {
    near_with(
        input,
        near_features,
        search_radius,
        location,
        angle,
        NearMethod::Planar,
    )
}

/// How Near measures: on a flat plane in the data's units, or on the WGS 84 ellipsoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearMethod {
    Planar,
    /// Data in longitude/latitude: distances and the search radius in metres, angles as azimuths
    /// (clockwise from north, -180..180]. Segments are straight in longitude/latitude.
    Geodesic,
}

impl NearMethod {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "" | "PLANAR" => Some(Self::Planar),
            "GEODESIC" => Some(Self::Geodesic),
            _ => None,
        }
    }
}

/// [`near`] with a measuring method.
pub fn near_with(
    input: &dyn FeatureClass,
    near_features: &[PathInput],
    search_radius: Option<f64>,
    location: bool,
    angle: bool,
    method: NearMethod,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let classes: Vec<Indexed> = near_features
        .iter()
        .map(|n| Indexed::load(n.features))
        .collect::<Result<_>>()?;
    let mut fields = input.fields().to_vec();
    let mut added = vec!["NEAR_FID", "NEAR_DIST"];
    if location {
        added.extend(["NEAR_X", "NEAR_Y"]);
    }
    if angle {
        added.push("NEAR_ANGLE");
    }
    if near_features.len() > 1 {
        added.push("NEAR_FC");
    }
    // Existing NEAR_* fields are overwritten, as ArcGIS does.
    let slots: Vec<usize> = added
        .iter()
        .map(|&name| {
            fields
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| {
                    let mut f = match name {
                        "NEAR_FID" => Field::new(name, FieldType::Integer),
                        "NEAR_FC" => Field::new(name, FieldType::String),
                        _ => Field::new(name, FieldType::Double),
                    };
                    if name == "NEAR_FC" {
                        f.length = 254;
                    }
                    fields.push(f);
                    fields.len() - 1
                })
        })
        .collect();
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        // (class, feature, nearest)
        let mut found: Option<(usize, usize, geoforge_geometry::Nearest)> = None;
        if let Some(g) = &f.geometry {
            for (c, candidates) in classes.iter().enumerate() {
                if let Some((i, n)) = candidates.closest(g, search_radius, method)?
                    && found.is_none_or(|(_, _, b)| n.distance < b.distance)
                {
                    found = Some((c, i, n));
                }
            }
        }
        let values: Vec<Value> = added
            .iter()
            .map(|&name| match &found {
                None if name == "NEAR_FID" => Value::Integer(-1),
                None if name == "NEAR_FC" => Value::Null,
                None => Value::Double(-1.0),
                Some((c, i, n)) => match name {
                    "NEAR_FID" => Value::Integer(classes[*c].features[*i].oid as i32),
                    "NEAR_FC" => Value::String(near_features[*c].source.clone()),
                    "NEAR_DIST" => Value::Double(n.distance),
                    "NEAR_X" => Value::Double(n.to[0]),
                    "NEAR_Y" => Value::Double(n.to[1]),
                    _ if n.distance == 0.0 => Value::Double(0.0),
                    _ if method == NearMethod::Geodesic => {
                        Value::Double(geoforge_geometry::azimuth(n.from, n.to))
                    }
                    _ => Value::Double(
                        (n.to[1] - n.from[1])
                            .atan2(n.to[0] - n.from[0])
                            .to_degrees(),
                    ),
                },
            })
            .collect();
        f.attributes.resize(fields.len(), Value::Null);
        for (&slot, v) in slots.iter().zip(values) {
            f.attributes[slot] = v;
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Spatial Join `match_option`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOption {
    Intersect,
    WithinADistance,
    Contains,
    CompletelyContains,
    Within,
    CompletelyWithin,
    Closest,
    /// `WITHIN_A_DISTANCE_GEODESIC`: longitude/latitude data, the radius and distances in metres.
    WithinADistanceGeodesic,
    /// `CLOSEST_GEODESIC`.
    ClosestGeodesic,
}

impl MatchOption {
    fn method(self) -> NearMethod {
        match self {
            Self::WithinADistanceGeodesic | Self::ClosestGeodesic => NearMethod::Geodesic,
            _ => NearMethod::Planar,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "INTERSECT" => Self::Intersect,
            "WITHIN_A_DISTANCE" => Self::WithinADistance,
            "CONTAINS" => Self::Contains,
            "COMPLETELY_CONTAINS" => Self::CompletelyContains,
            "WITHIN" => Self::Within,
            "COMPLETELY_WITHIN" => Self::CompletelyWithin,
            "CLOSEST" => Self::Closest,
            "WITHIN_A_DISTANCE_GEODESIC" => Self::WithinADistanceGeodesic,
            "CLOSEST_GEODESIC" => Self::ClosestGeodesic,
            _ => return None,
        })
    }
}

/// Spatial Join options beyond the inputs.
#[derive(Debug, Clone)]
pub struct SpatialJoinOptions {
    /// Merge rules of join fields (one to one), by field name; others take First.
    pub merge_rules: Vec<(String, MergeRule)>,
    /// `JOIN_ONE_TO_MANY` (else one to one).
    pub one_to_many: bool,
    /// `KEEP_ALL` (else `KEEP_COMMON`).
    pub keep_all: bool,
    pub match_option: MatchOption,
    pub search_radius: Option<f64>,
    pub distance_field_name: Option<String>,
    /// A full field mapping (`field_mapping` parameter): output fields, types, merge
    /// rules and their sources. `None` keeps every input field as it is.
    pub field_map: Option<Vec<FieldMapEntry>>,
    /// The file stems ([`table_stem`]) of the target and join tables, which tell the
    /// tables named in `field_map` apart.
    pub target_name: String,
    pub join_name: String,
}

/// `arcpy.analysis.SpatialJoin(target, join, out, ...)` with the default field
/// mapping (merge rule First): `Join_Count`, the optional distance field,
/// `TARGET_FID` (and `JOIN_FID` one to many), then target and join fields.
/// The distance field is only filled for `CLOSEST` (-1 otherwise, as in ArcGIS).
/// Matches are taken in join-FID order; one to one keeps the first's attributes,
/// one to many writes a row per match (`Join_Count` 1, or 0 with `JOIN_FID` -1
/// for an unmatched target kept by `KEEP_ALL`).
/// A field map's merge rule: how the values of several matching join features
/// become one (ArcGIS `FieldMap.mergeRule`).
#[derive(Debug, Clone, PartialEq)]
pub enum MergeRule {
    First,
    Last,
    Join(String),
    Sum,
    Mean,
    Median,
    Mode,
    Min,
    Max,
    StdDev,
    Count,
    Range,
}

impl MergeRule {
    /// `Sum`, `Join` (delimiter `,` unless given), ... as arcpy names them.
    pub fn parse(rule: &str, delimiter: Option<&str>) -> Option<Self> {
        Some(match rule.to_ascii_lowercase().as_str() {
            "first" => Self::First,
            "last" => Self::Last,
            "join" => Self::Join(delimiter.unwrap_or(",").to_owned()),
            "sum" => Self::Sum,
            "mean" => Self::Mean,
            "median" => Self::Median,
            "mode" => Self::Mode,
            "min" => Self::Min,
            "max" => Self::Max,
            "stddev" | "standard deviation" => Self::StdDev,
            "count" => Self::Count,
            "range" => Self::Range,
            _ => return None,
        })
    }
}

/// The merged value of `values` (matches in join FID order) for a field of `field`'s
/// type. `Err` holds text that does not fit a text field (ArcGIS WARNING 001156).
fn merge_values(
    rule: &MergeRule,
    values: &[&Value],
    field: &Field,
) -> std::result::Result<Value, String> {
    let text_field = field.field_type == FieldType::String;
    let numbers = || -> Vec<f64> { values.iter().filter_map(|v| number(v)).collect() };
    // Numbers stored in the output field's type (integers rounded).
    let typed = |x: f64| -> Value {
        match field.field_type {
            FieldType::SmallInteger => Value::SmallInteger(x.round() as i16),
            FieldType::Integer => Value::Integer(x.round() as i32),
            FieldType::BigInteger => Value::BigInteger(x.round() as i64),
            FieldType::Single => Value::Single(x as f32),
            FieldType::String => Value::String(format!("{x}")),
            _ => Value::Double(x),
        }
    };
    let stat = |f: &dyn Fn(&mut Vec<f64>) -> Option<f64>| -> Value {
        if text_field {
            return Value::Null;
        }
        let mut v = numbers();
        if v.is_empty() {
            return Value::Null;
        }
        v.sort_by(f64::total_cmp);
        f(&mut v).map_or(Value::Null, typed)
    };
    Ok(match rule {
        MergeRule::First => values.first().map_or(Value::Null, |v| (*v).clone()),
        MergeRule::Last => values.last().map_or(Value::Null, |v| (*v).clone()),
        MergeRule::Sum => stat(&|v| Some(v.iter().sum())),
        MergeRule::Mean => stat(&|v| Some(v.iter().sum::<f64>() / v.len() as f64)),
        MergeRule::Median => stat(&|v| {
            let n = v.len();
            Some(if n % 2 == 1 {
                v[n / 2]
            } else {
                (v[n / 2 - 1] + v[n / 2]) / 2.0
            })
        }),
        // Most frequent; ties go to the smallest value.
        MergeRule::Mode => stat(&|v| {
            let mut best = (v[0], 0usize);
            let mut i = 0;
            while i < v.len() {
                let j = v[i..].iter().take_while(|&&x| x == v[i]).count();
                if j > best.1 {
                    best = (v[i], j);
                }
                i += j;
            }
            Some(best.0)
        }),
        MergeRule::Min => stat(&|v| v.first().copied()),
        MergeRule::Max => stat(&|v| v.last().copied()),
        MergeRule::Range => stat(&|v| Some(v[v.len() - 1] - v[0])),
        MergeRule::StdDev => stat(&|v| {
            let n = v.len() as f64;
            if v.len() < 2 {
                return Some(0.0);
            }
            let mean = v.iter().sum::<f64>() / n;
            Some((v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt())
        }),
        MergeRule::Count => {
            let n = values.iter().filter(|v| !v.is_null()).count();
            if text_field {
                Value::String(n.to_string())
            } else {
                typed(n as f64)
            }
        }
        MergeRule::Join(delim) => {
            let parts: Vec<String> = values
                .iter()
                .filter(|v| !v.is_null())
                .map(|v| match v {
                    Value::String(s) => s.trim_end().to_owned(),
                    other => number(other).map_or_else(String::new, |x| format!("{x}")),
                })
                .collect();
            let joined = parts.join(delim);
            if text_field {
                if field.length > 0 && joined.chars().count() > field.length as usize {
                    return Err(joined);
                }
                Value::String(joined)
            } else {
                joined.trim().parse::<f64>().map_or(Value::Null, typed)
            }
        }
    })
}

pub fn spatial_join(
    target: &dyn FeatureClass,
    join: &dyn FeatureClass,
    opts: &SpatialJoinOptions,
) -> Result<ToolOutput> {
    let kind = target
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("target has no geometry".into()))?;
    let joins = Indexed::load(join)?;
    let mut fields = vec![Field::new("Join_Count", FieldType::Integer)];
    if let Some(name) = &opts.distance_field_name {
        fields.push(Field::new(name.clone(), FieldType::Double));
    }
    fields.push(Field::new("TARGET_FID", FieldType::Integer));
    if opts.one_to_many {
        fields.push(Field::new("JOIN_FID", FieldType::Integer));
    }
    let head_len = fields.len();
    // With a field mapping: for each output field, the (is join table, field index)
    // of each source.
    let plan: Option<Vec<Vec<(bool, usize)>>> = match &opts.field_map {
        None => None,
        Some(map) => {
            let find = |fc: &dyn FeatureClass, name: &str| {
                fc.fields()
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| {
                        Error::Unsupported(format!(
                            "ERROR 000728: Field {name} does not exist within table"
                        ))
                    })
            };
            Some(
                map.iter()
                    .map(|e| {
                        e.sources
                            .iter()
                            .map(|src| {
                                let stem = table_stem(&src.table);
                                if stem == opts.join_name && stem != opts.target_name {
                                    Ok((true, find(join, &src.field)?))
                                } else if stem == opts.target_name {
                                    Ok((false, find(target, &src.field)?))
                                } else {
                                    Err(Error::Unsupported(format!(
                                        "field mapping names the table {}, which is neither input",
                                        src.table
                                    )))
                                }
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .collect::<Result<Vec<_>>>()?,
            )
        }
    };
    match &opts.field_map {
        Some(map) => fields.extend(map.iter().map(|e| e.field.clone())),
        None => {
            for f in target.fields().iter().chain(join.fields()) {
                push_unique(&mut fields, f.clone());
            }
        }
    }
    let radius = opts.search_radius.unwrap_or(0.0);
    let rules = &opts.merge_rules;
    let mut messages = Vec::new();
    let mut features = Vec::new();
    for t in target.features() {
        let t = t?;
        // (join index, distance)
        let mut matches: Vec<(usize, f64)> = Vec::new();
        if let Some(tg) = &t.geometry {
            if matches!(
                opts.match_option,
                MatchOption::Closest | MatchOption::ClosestGeodesic
            ) {
                if let Some((i, n)) =
                    joins.closest(tg, opts.search_radius, opts.match_option.method())?
                {
                    matches.push((i, n.distance));
                }
            } else {
                let geodesic = opts.match_option.method() == NearMethod::Geodesic;
                let pad = if geodesic {
                    degrees_for(tg, radius)
                } else {
                    radius
                };
                for i in joins.candidates(tg, pad) {
                    let jg = joins.features[i].geometry.as_ref().unwrap();
                    let n = if geodesic {
                        geoforge_geometry::nearest_geodesic(tg, jg)?
                    } else {
                        geoforge_geometry::nearest(tg, jg)?
                    };
                    let Some(n) = n else {
                        continue;
                    };
                    let hit = match opts.match_option {
                        MatchOption::Intersect
                        | MatchOption::WithinADistance
                        | MatchOption::WithinADistanceGeodesic => n.distance <= radius,
                        MatchOption::Within => geoforge_geometry::within(tg, jg)?,
                        MatchOption::CompletelyWithin => {
                            geoforge_geometry::completely_within(tg, jg)?
                        }
                        MatchOption::Contains => geoforge_geometry::within(jg, tg)?,
                        MatchOption::CompletelyContains => {
                            geoforge_geometry::completely_within(jg, tg)?
                        }
                        MatchOption::Closest | MatchOption::ClosestGeodesic => unreachable!(),
                    };
                    if hit {
                        matches.push((i, n.distance));
                    }
                }
            }
        }
        let row = |count: usize, m: Option<(usize, f64)>| {
            let mut a = vec![Value::Integer(count as i32)];
            if opts.distance_field_name.is_some() {
                a.push(Value::Double(match m {
                    Some((_, d))
                        if matches!(
                            opts.match_option,
                            MatchOption::Closest | MatchOption::ClosestGeodesic
                        ) =>
                    {
                        d
                    }
                    _ => -1.0,
                }));
            }
            a.push(Value::Integer(t.oid as i32));
            if opts.one_to_many {
                a.push(Value::Integer(
                    m.map_or(-1, |m| joins.features[m.0].oid as i32),
                ));
            }
            a.extend(t.attributes.iter().cloned());
            match m {
                Some((i, _)) => a.extend(joins.features[i].attributes.iter().cloned()),
                None => a.extend(std::iter::repeat_n(Value::Null, join.fields().len())),
            }
            a
        };
        // One to one with merge rules: each join field merged over all matches.
        // Text that does not fit stops the row there: that field and the ones after
        // it stay empty (ArcGIS WARNING 001156).
        let merged = |mut a: Vec<Value>, messages: &mut Vec<String>| {
            let first = a.len() - join.fields().len();
            for (k, f) in join.fields().iter().enumerate() {
                let rule = rules
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&f.name))
                    .map_or(&MergeRule::First, |(_, r)| r);
                let values: Vec<&Value> = matches
                    .iter()
                    .map(|&(i, _)| &joins.features[i].attributes[k])
                    .collect();
                match merge_values(rule, &values, f) {
                    Ok(v) => a[first + k] = v,
                    Err(text) => {
                        messages.push(format!(
                            "WARNING 001156: Failed on input OID {}, could not write value '{text}' to output field {}",
                            t.oid, f.name
                        ));
                        a[first + k..].iter_mut().for_each(|v| *v = Value::Null);
                        break;
                    }
                }
            }
            a
        };
        // The mapped output fields for the join rows `subset`.
        let mapped = |mut head: Vec<Value>, subset: &[usize], messages: &mut Vec<String>| {
            head.truncate(head_len);
            let (map, plan) = (opts.field_map.as_ref().unwrap(), plan.as_ref().unwrap());
            let mut failed = false;
            for (entry, sources) in map.iter().zip(plan) {
                if failed {
                    head.push(Value::Null);
                    continue;
                }
                let mut values: Vec<Value> = Vec::new();
                for (src, &(from_join, k)) in entry.sources.iter().zip(sources) {
                    if from_join {
                        values.extend(subset.iter().map(|&i| {
                            fieldmap::substring(
                                &joins.features[i].attributes[k],
                                src.start,
                                src.end,
                            )
                        }));
                    } else {
                        values.push(fieldmap::substring(&t.attributes[k], src.start, src.end));
                    }
                }
                let refs: Vec<&Value> = values.iter().collect();
                match merge_values(&entry.rule, &refs, &entry.field) {
                    Ok(v) => head.push(fieldmap::coerce(&v, &entry.field)),
                    Err(text) => {
                        messages.push(format!(
                            "WARNING 001156: Failed on input OID {}, could not write value '{text}' to output field {}",
                            t.oid, entry.field.name
                        ));
                        failed = true;
                        head.push(Value::Null);
                    }
                }
            }
            head
        };
        let mut rows = Vec::new();
        if plan.is_some() {
            if matches.is_empty() {
                if opts.keep_all {
                    rows.push(mapped(row(0, None), &[], &mut messages));
                }
            } else if opts.one_to_many {
                for &m in &matches {
                    rows.push(mapped(row(1, Some(m)), &[m.0], &mut messages));
                }
            } else {
                let all: Vec<usize> = matches.iter().map(|m| m.0).collect();
                rows.push(mapped(
                    row(matches.len(), Some(matches[0])),
                    &all,
                    &mut messages,
                ));
            }
        } else if matches.is_empty() {
            if opts.keep_all {
                rows.push(row(0, None));
            }
        } else if opts.one_to_many {
            rows.extend(matches.iter().map(|&m| row(1, Some(m))));
        } else if rules.is_empty() {
            rows.push(row(matches.len(), Some(matches[0])));
        } else {
            rows.push(merged(row(matches.len(), Some(matches[0])), &mut messages));
        }
        for attributes in rows {
            features.push(Feature {
                oid: features.len() as i64,
                geometry: t.geometry.clone(),
                attributes,
            });
        }
    }
    Ok(ToolOutput {
        kind,
        has_z: target.has_z(),
        has_m: target.has_m(),
        fields,
        spatial_reference_wkt: target.spatial_reference_wkt().map(str::to_owned),
        features,
        messages,
    })
}

/// Select Layer By Location `overlap_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlap {
    Intersect,
    WithinADistance,
    Contains,
    CompletelyContains,
    ContainsClementini,
    Within,
    CompletelyWithin,
    WithinClementini,
    AreIdenticalTo,
    BoundaryTouches,
    ShareALineSegmentWith,
    CrossedByTheOutlineOf,
    HaveTheirCenterIn,
}

impl Overlap {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "INTERSECT" => Self::Intersect,
            "WITHIN_A_DISTANCE" => Self::WithinADistance,
            "CONTAINS" => Self::Contains,
            "COMPLETELY_CONTAINS" => Self::CompletelyContains,
            "CONTAINS_CLEMENTINI" => Self::ContainsClementini,
            "WITHIN" => Self::Within,
            "COMPLETELY_WITHIN" => Self::CompletelyWithin,
            "WITHIN_CLEMENTINI" => Self::WithinClementini,
            "ARE_IDENTICAL_TO" => Self::AreIdenticalTo,
            "BOUNDARY_TOUCHES" => Self::BoundaryTouches,
            "SHARE_A_LINE_SEGMENT_WITH" => Self::ShareALineSegmentWith,
            "CROSSED_BY_THE_OUTLINE_OF" => Self::CrossedByTheOutlineOf,
            "HAVE_THEIR_CENTER_IN" => Self::HaveTheirCenterIn,
            _ => return None,
        })
    }
}

/// Whether input geometry `a` stands in relation `overlap` to selecting geometry
/// `b` (Esri semantics: CONTAINS/WITHIN count boundary contact, the CLEMENTINI
/// variants do not, COMPLETELY_* exclude any boundary contact).
fn relates(
    a: &geoforge_core::Geometry,
    b: &geoforge_core::Geometry,
    overlap: Overlap,
    distance: f64,
) -> Result<bool> {
    use geoforge_geometry as g;
    Ok(match overlap {
        Overlap::Intersect | Overlap::WithinADistance => {
            g::nearest(a, b)?.is_some_and(|n| n.distance <= distance)
        }
        Overlap::Contains => g::within(b, a)?,
        Overlap::CompletelyContains => g::completely_within(b, a)?,
        Overlap::ContainsClementini => g::relate_matches(a, b, "T*****FF*")?,
        Overlap::Within => g::within(a, b)?,
        Overlap::CompletelyWithin => g::completely_within(a, b)?,
        Overlap::WithinClementini => g::relate_matches(a, b, "T*F**F***")?,
        Overlap::AreIdenticalTo => g::relate_matches(a, b, "T*F**FFF*")?,
        Overlap::BoundaryTouches => {
            g::relate_matches(a, b, "FT*******")?
                || g::relate_matches(a, b, "F**T*****")?
                || g::relate_matches(a, b, "F***T****")?
        }
        Overlap::ShareALineSegmentWith => {
            g::relate_matches(&g::outline(a), &g::outline(b), "1********")?
        }
        Overlap::CrossedByTheOutlineOf => {
            g::relate_matches(&g::outline(a), &g::outline(b), "0********")?
        }
        Overlap::HaveTheirCenterIn => match g::center(a)? {
            Some([x, y]) => g::nearest(&geoforge_core::Geometry::point(x, y), b)?
                .is_some_and(|n| n.distance == 0.0),
            None => false,
        },
    })
}

/// `arcpy.management.SelectLayerByLocation(in_layer, overlap_type,
/// select_features, search_distance, "NEW_SELECTION", invert)` followed by Copy
/// Features: the input features (in FID order) related to any selecting feature.
/// `search_distance` applies to INTERSECT and WITHIN_A_DISTANCE.
pub fn select_by_location(
    input: &dyn FeatureClass,
    overlap: Overlap,
    select_features: &dyn FeatureClass,
    search_distance: Option<f64>,
    invert: bool,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let selecting = Indexed::load(select_features)?;
    let distance = search_distance.unwrap_or(0.0);
    let pad = match overlap {
        Overlap::Intersect | Overlap::WithinADistance => distance,
        _ => 0.0,
    };
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let mut hit = false;
        if let Some(a) = &f.geometry {
            for i in selecting.candidates(a, pad) {
                let b = selecting.features[i].geometry.as_ref().unwrap();
                if relates(a, b, overlap, distance)? {
                    hit = true;
                    break;
                }
            }
        }
        if hit != invert {
            features.push(Feature {
                oid: features.len() as i64,
                ..f
            });
        }
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        messages: if features.is_empty() {
            vec!["WARNING 000117: Warning empty output generated.".into()]
        } else {
            Vec::new()
        },
        features,
    })
}

/// `arcpy.management.Append(inputs, target, schema_type)`: the target with the
/// inputs' features added after its own. The target keeps its schema; input
/// values go to target fields of the same name (others are dropped, missing ones
/// null); features of another geometry type are skipped with a warning. `TEST`
/// requires every input to match the target's fields and geometry type.
pub fn append(
    inputs: &[&dyn FeatureClass],
    target: &dyn FeatureClass,
    test: bool,
) -> Result<ToolOutput> {
    let inputs: Vec<PathInput> = inputs
        .iter()
        .map(|&features| PathInput {
            source: String::new(),
            features,
        })
        .collect();
    append_with(&inputs, target, test, None)
}

/// [`append`] with a field mapping (`NO_TEST`): each target field named in the mapping
/// takes the first of its sources that belongs to the input being appended (found by
/// table name), converted to the target field's type; unmapped target fields are null.
pub fn append_with(
    inputs: &[PathInput],
    target: &dyn FeatureClass,
    test: bool,
    field_map: Option<&[FieldMapEntry]>,
) -> Result<ToolOutput> {
    let kind = target.geometry_kind();
    let fields = target.fields().to_vec();
    let mut features: Vec<Feature> = target.features().collect::<Result<_>>()?;
    let mut messages = Vec::new();
    for pi in inputs {
        let input = pi.features;
        let same_kind = input.geometry_kind() == kind;
        let positions: Vec<Option<(usize, i64, i64)>> = match field_map {
            None => fields
                .iter()
                .map(|f| {
                    input
                        .fields()
                        .iter()
                        .position(|g| g.name.eq_ignore_ascii_case(&f.name))
                        .map(|p| (p, -1, -1))
                })
                .collect(),
            Some(map) => {
                let stem = table_stem(&pi.source);
                fields
                    .iter()
                    .map(|f| {
                        map.iter()
                            .find(|e| e.field.name.eq_ignore_ascii_case(&f.name))?
                            .sources
                            .iter()
                            .filter(|src| table_stem(&src.table) == stem)
                            .find_map(|src| {
                                input
                                    .fields()
                                    .iter()
                                    .position(|g| g.name.eq_ignore_ascii_case(&src.field))
                                    .map(|p| (p, src.start, src.end))
                            })
                    })
                    .collect()
            }
        };
        if test {
            let same = input.fields().len() == fields.len()
                && positions.iter().zip(&fields).all(|(p, f)| {
                    p.is_some_and(|(p, ..)| input.fields()[p].field_type == f.field_type)
                });
            if !same || !same_kind {
                return Err(Error::Unsupported(
                    "ERROR 000466: input does not match the schema of the target; use NO_TEST"
                        .into(),
                ));
            }
        }
        for (n, f) in input.features().enumerate() {
            let f = f?;
            // ArcGIS skips features of another geometry type, one warning each.
            if !same_kind {
                messages.push(format!(
                    "WARNING 000594: Input feature {n}: No support for this geometry type.."
                ));
                continue;
            }
            features.push(Feature {
                oid: features.len() as i64,
                attributes: positions
                    .iter()
                    .zip(&fields)
                    .map(|(p, field)| match (p, field_map) {
                        (None, _) => Value::Null,
                        (Some((p, ..)), None) => f.attributes[*p].clone(),
                        (Some((p, a, b)), Some(_)) => {
                            fieldmap::coerce(&fieldmap::substring(&f.attributes[*p], *a, *b), field)
                        }
                    })
                    .collect(),
                geometry: f.geometry,
            });
        }
    }
    Ok(ToolOutput {
        kind: kind.ok_or_else(|| Error::Unsupported("appending to tables".into()))?,
        has_z: target.has_z(),
        has_m: target.has_m(),
        fields,
        spatial_reference_wkt: target.spatial_reference_wkt().map(str::to_owned),
        features,
        messages,
    })
}

/// `arcpy.analysis.Split(in_features, split_features, split_field, out_workspace)`:
/// for each value of the text `split_field` (ascending), the input clipped by the
/// split features with that value, named after the value. Values whose clip is
/// empty give no output.
pub fn split(
    input: &dyn FeatureClass,
    split_features: &dyn FeatureClass,
    split_field: &str,
) -> Result<Vec<(String, ToolOutput)>> {
    let index = split_features
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(split_field))
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "ERROR 000728: Field {split_field} does not exist within table"
            ))
        })?;
    if split_features.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(Error::Unsupported("split features must be polygons".into()));
    }
    if split_features.fields()[index].field_type != FieldType::String {
        return Err(Error::Unsupported(
            "ERROR 003911: The field is not of type TEXT".into(),
        ));
    }
    let mut groups: std::collections::BTreeMap<String, Vec<geoforge_core::Geometry>> =
        Default::default();
    for f in load(split_features)? {
        let Value::String(v) = &f.attributes[index] else {
            continue;
        };
        groups
            .entry(v.trim_end().to_owned())
            .or_default()
            .push(f.geometry.unwrap());
    }
    // Unlike Clip, Split cracks lines wherever two input lines meet.
    let nodes = if input.geometry_kind() == Some(GeometryKind::Polyline) {
        let lines = load(input)?;
        Some(geoforge_geometry::LineNodes::new(
            lines.iter().filter_map(|f| f.geometry.as_ref()),
            default_xy_tolerance(input.spatial_reference_wkt()),
        )?)
    } else {
        None
    };
    let mut outputs = Vec::new();
    for (value, geoms) in groups {
        if value.is_empty() {
            continue;
        }
        let out = clip_to_area(input, &ClipArea::new(geoms.iter())?, nodes.as_ref())?;
        if !out.features.is_empty() {
            outputs.push((value, out));
        }
    }
    Ok(outputs)
}

/// ArcGIS's default XY tolerance for a coordinate system: 0.001 m in the
/// projection's linear unit, 8.983e-9 degrees for geographic coordinates.
pub fn default_xy_tolerance(spatial_reference_wkt: Option<&str>) -> f64 {
    let Some(wkt) = spatial_reference_wkt.map(str::trim_start) else {
        return geoforge_geometry::XY_TOLERANCE;
    };
    if wkt.starts_with("GEOGCS") {
        return 8.983152841195215e-9;
    }
    // The last UNIT is the projection's linear unit: UNIT["Foot_US",0.3048006096012192]
    wkt.rfind("UNIT[")
        .and_then(|i| wkt[i..].split(',').nth(1))
        .and_then(|f| f.trim_end_matches([']', ' ']).trim().parse::<f64>().ok())
        .filter(|&f| f > 0.0)
        .map_or(geoforge_geometry::XY_TOLERANCE, |f| {
            geoforge_geometry::XY_TOLERANCE / f
        })
}

/// One input of Check Geometry.
pub struct CheckInput<'a> {
    /// The path ArcGIS writes to the CLASS field.
    pub source: String,
    pub features: &'a dyn FeatureClass,
    /// False for a shapefile without a `.sbn` spatial index: ArcGIS reports
    /// that first, with FEATURE_ID -1.
    pub has_spatial_index: bool,
}

/// One row of Check Geometry's output table.
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryProblem {
    pub class: String,
    pub feature_id: i64,
    pub problem: String,
}

/// `arcpy.management.CheckGeometry(in_features, out_table, validation_method)`:
/// one row per problem feature (CLASS, FEATURE_ID, PROBLEM), each also reported
/// as WARNING 000442. The rows and the messages are returned.
pub fn check_geometry(
    inputs: &[CheckInput],
    method: geoforge_geometry::ValidationMethod,
) -> Result<(Vec<GeometryProblem>, Vec<String>)> {
    let mut rows = Vec::new();
    for input in inputs {
        let mut push = |feature_id: i64, problem: &str| {
            rows.push(GeometryProblem {
                class: input.source.clone(),
                feature_id,
                problem: problem.to_owned(),
            });
        };
        if !input.has_spatial_index {
            push(-1, "could not find spatial index");
        }
        if input.features.geometry_kind().is_none() {
            continue;
        }
        let tol = default_xy_tolerance(input.features.spatial_reference_wkt());
        for f in input.features.features() {
            let f = f?;
            if let Some(p) = geoforge_geometry::check(f.geometry.as_ref(), method, tol) {
                push(f.oid, p.description());
            }
        }
    }
    let messages = rows
        .iter()
        .map(|r| {
            format!(
                "WARNING 000442: {} at {} in {}",
                r.problem, r.feature_id, r.class
            )
        })
        .collect();
    Ok((rows, messages))
}

/// The fields of Check Geometry's output table.
pub fn check_geometry_fields() -> Vec<Field> {
    let mut class = Field::new("CLASS", FieldType::String);
    class.length = 254;
    let mut id = Field::new("FEATURE_ID", FieldType::Integer);
    id.precision = 10;
    let mut problem = Field::new("PROBLEM", FieldType::String);
    problem.length = 254;
    vec![class, id, problem]
}

/// `arcpy.management.RepairGeometry(in_features, delete_null, validation_method)`:
/// the input with each problem feature repaired (WARNING 000461). Null shapes
/// are deleted with `delete_null` (WARNING 000107), as are features nothing is
/// left of after repair; otherwise they stay with a null shape.
pub fn repair_geometry(
    input: &dyn FeatureClass,
    delete_null: bool,
    method: geoforge_geometry::ValidationMethod,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let tol = default_xy_tolerance(input.spatial_reference_wkt());
    let mut features = Vec::new();
    let mut messages = Vec::new();
    for f in input.features() {
        let mut f = f?;
        let problem = geoforge_geometry::check(f.geometry.as_ref(), method, tol);
        match problem {
            None => {}
            // Also a line with only one-point parts: it becomes null, unreported.
            Some(geoforge_geometry::Problem::NullGeometry) => {
                f.geometry = None;
                if delete_null {
                    messages.push(format!(
                        "WARNING 000107: Deleted feature {} because of empty geometry",
                        f.oid
                    ));
                    continue;
                }
            }
            Some(p) => {
                messages.push(format!(
                    "WARNING 000461: Repaired feature {} because of {}",
                    f.oid,
                    p.repair_reason()
                ));
                f.geometry = match &f.geometry {
                    Some(g) => geoforge_geometry::repair(g, method, tol)?,
                    None => None,
                };
                if f.geometry.is_none() && delete_null {
                    continue;
                }
            }
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages,
    })
}

/// `arcpy.management.Project(in_dataset, out_dataset, out_coor_system,
/// {transform_method})`: every vertex transformed to `target`. Without a
/// `transform_method` PROJ chooses the operation (not always ArcGIS's default for the
/// datum pair); with one, the named geographic transformations are applied. Z and M
/// are kept; true curves are densified first.
#[cfg(feature = "proj")]
pub fn project(
    input: &dyn FeatureClass,
    target: &geoforge_proj::Crs,
    transform_method: Option<&str>,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let source = input.spatial_reference_wkt().ok_or_else(|| {
        Error::Unsupported(
            "ERROR 000622: the input has an unknown coordinate system; define it first".into(),
        )
    })?;
    let source = geoforge_proj::Crs::from_wkt(source)?;
    let transform = geoforge_proj::Transform::with_method(&source, target, transform_method)?;
    let deviation = geoforge_core::curves::arcgis_densify_deviation(source.is_geographic());
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        if let Some(g) = &mut f.geometry {
            if g.has_curves() {
                *g = g.densify_curves_arcgis(deviation)?;
            }
            transform.apply(&mut g.xy)?;
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: Some(target.esri_wkt().to_owned()),
        features,
        messages: Vec::new(),
    })
}

/// The input's fields plus `ORIG_FID`.
fn with_orig_fid(input: &dyn FeatureClass) -> Vec<Field> {
    let mut fields = input.fields().to_vec();
    fields.push(Field::new("ORIG_FID", FieldType::Integer));
    fields
}

fn with_value(attrs: &[Value], extra: impl IntoIterator<Item = Value>) -> Vec<Value> {
    attrs.iter().cloned().chain(extra).collect()
}

fn point_geometry(p: [f64; 2], z: Option<f64>, m: Option<f64>) -> geoforge_core::Geometry {
    let mut g = geoforge_core::Geometry::point(p[0], p[1]);
    g.z = z.map(|z| vec![z]);
    g.m = m.map(|m| vec![m]);
    g
}

/// `arcpy.management.MultipartToSinglepart(in, out)`: one feature per part (a
/// polygon part is an exterior ring with its holes), with `ORIG_FID`.
pub fn multipart_to_singlepart(input: &dyn FeatureClass) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let out_kind = if kind == GeometryKind::Multipoint {
        GeometryKind::Point
    } else {
        kind
    };
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let parts: Vec<geoforge_core::Geometry> = match kind {
            GeometryKind::Multipoint => (0..g.xy.len())
                .map(|i| {
                    point_geometry(
                        g.xy[i],
                        g.z.as_ref().map(|z| z[i]),
                        g.m.as_ref().map(|m| m[i]),
                    )
                })
                .collect(),
            GeometryKind::Point => vec![g.clone()],
            _ => geoforge_geometry::explode(g),
        };
        for part in parts {
            features.push(Feature {
                oid: features.len() as i64,
                geometry: Some(part),
                attributes: with_value(&f.attributes, [Value::Integer(f.oid as i32)]),
            });
        }
    }
    Ok(ToolOutput {
        kind: out_kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: with_orig_fid(input),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// `arcpy.management.FeatureToPoint(in, out, point_location)`: the centroid, or
/// with `inside` a point inside the polygon / on the line.
pub fn feature_to_point(input: &dyn FeatureClass, inside: bool) -> Result<ToolOutput> {
    input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let p = if inside {
            geoforge_geometry::inside_point(g)?
        } else {
            geoforge_geometry::centroid(g)?
        };
        let Some(p) = p else { continue };
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(geoforge_core::Geometry::point(p[0], p[1])),
            attributes: with_value(&f.attributes, [Value::Integer(f.oid as i32)]),
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Point,
        has_z: false,
        has_m: false,
        fields: with_orig_fid(input),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Feature Vertices To Points `point_location`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexLocation {
    All,
    Start,
    End,
    BothEnds,
    Mid,
    Dangle,
}

impl VertexLocation {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "ALL" => Self::All,
            "START" => Self::Start,
            "END" => Self::End,
            "BOTH_ENDS" => Self::BothEnds,
            "MID" => Self::Mid,
            "DANGLE" => Self::Dangle,
            _ => return None,
        })
    }
}

/// `arcpy.management.FeatureVerticesToPoints(in, out, point_location)`: points
/// at every vertex, at each part's start/end, halfway along each feature, or at
/// dangling part ends (not touching any other line) with `DANGLE_LEN`, the
/// distance along the part to the nearest place another line meets it.
pub fn feature_vertices_to_points(
    input: &dyn FeatureClass,
    location: VertexLocation,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let lines = load(input)?;
    let tol = default_xy_tolerance(input.spatial_reference_wkt());
    let mut fields = with_orig_fid(input);
    let dangle = location == VertexLocation::Dangle;
    if dangle {
        if kind != GeometryKind::Polyline {
            return Err(Error::Unsupported("DANGLE needs line input".into()));
        }
        fields.push(Field::new("DANGLE_LEN", FieldType::Double));
    }
    // Every part as its own line, to find where other lines meet each one.
    let parts: Vec<(usize, geoforge_core::Geometry)> = lines
        .iter()
        .enumerate()
        .flat_map(|(i, f)| {
            f.geometry
                .iter()
                .flat_map(geoforge_geometry::explode)
                .map(move |p| (i, p))
                .collect::<Vec<_>>()
        })
        .collect();
    let index = geoforge_geometry::EnvelopeIndex::new(parts.iter().map(|(_, p)| Some(p)));
    let mut features = Vec::new();
    for f in &lines {
        let g = geoforge_geometry::explode(f.geometry.as_ref().unwrap())
            .into_iter()
            .collect::<Vec<_>>();
        let whole = f.geometry.as_ref().unwrap();
        let mut push =
            |p: [f64; 2], src: Option<(&geoforge_core::Geometry, usize)>, extra: Option<f64>| {
                let (z, m) = src.map_or((None, None), |(s, i)| {
                    (s.z.as_ref().map(|z| z[i]), s.m.as_ref().map(|m| m[i]))
                });
                let mut attrs = with_value(&f.attributes, [Value::Integer(f.oid as i32)]);
                if let Some(d) = extra {
                    attrs.push(Value::Double(d));
                }
                features.push(Feature {
                    oid: features.len() as i64,
                    geometry: Some(point_geometry(p, z, m)),
                    attributes: attrs,
                });
            };
        match location {
            VertexLocation::All => {
                for (i, &p) in whole.xy.iter().enumerate() {
                    push(p, Some((whole, i)), None);
                }
            }
            VertexLocation::Mid => {
                for part in &g {
                    if let Some(p) = geoforge_geometry::center(&geoforge_geometry::outline(part))? {
                        push(p, None, None);
                    }
                }
            }
            VertexLocation::Start | VertexLocation::End | VertexLocation::BothEnds => {
                for r in whole.part_ranges().filter(|r| !r.is_empty()) {
                    if location != VertexLocation::End {
                        push(whole.xy[r.start], Some((whole, r.start)), None);
                    }
                    if location != VertexLocation::Start {
                        push(whole.xy[r.end - 1], Some((whole, r.end - 1)), None);
                    }
                }
            }
            VertexLocation::Dangle => {
                // A feature's dangles come sorted by x, then y.
                let mut dangles: Vec<([f64; 2], usize, usize, f64)> = Vec::new();
                for (pi, part) in g.iter().enumerate() {
                    let path = &part.xy;
                    if path.len() < 2 {
                        continue;
                    }
                    // Where any other part meets this one: (distance along, point).
                    let mut meets: Vec<f64> = Vec::new();
                    let others: Vec<&geoforge_core::Geometry> = index
                        .candidates(&part.envelope(), tol)
                        .into_iter()
                        .map(|k| &parts[k].1)
                        .filter(|o| o.xy != part.xy)
                        .collect();
                    for o in &others {
                        let m = geoforge_geometry::meeting(part, o, tol)?;
                        for p in m.points.iter().chain(m.shared.iter().flatten()) {
                            meets.push(along(path, *p));
                        }
                    }
                    let total = along(path, path[path.len() - 1]);
                    let ends = [(path[0], 0usize), (path[path.len() - 1], path.len() - 1)];
                    for (end, at) in ends {
                        let s = along(path, end);
                        let touched = others.iter().any(|o| {
                            geoforge_geometry::nearest(
                                o,
                                &geoforge_core::Geometry::point(end[0], end[1]),
                            )
                            .ok()
                            .flatten()
                            .is_some_and(|n| n.distance <= tol)
                        });
                        if touched {
                            continue;
                        }
                        let len = meets
                            .iter()
                            .map(|&d| (d - s).abs())
                            .fold(f64::INFINITY, f64::min);
                        let len = if len.is_finite() { len } else { total };
                        dangles.push((end, pi, at, len));
                    }
                }
                dangles.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]).then(a.0[1].total_cmp(&b.0[1])));
                for (end, pi, at, len) in dangles {
                    push(end, Some((&g[pi], at)), Some(len));
                }
            }
        }
    }
    Ok(ToolOutput {
        kind: GeometryKind::Point,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Distance along a path to the point on it nearest `p`.
fn along(path: &[[f64; 2]], p: [f64; 2]) -> f64 {
    let mut walked = 0.0;
    let mut best = (f64::INFINITY, 0.0);
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        let t = if len == 0.0 {
            0.0
        } else {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (len * len)).clamp(0.0, 1.0)
        };
        let d = (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy);
        if d < best.0 {
            best = (d, walked + t * len);
        }
        walked += len;
    }
    best.1
}

/// Minimum Bounding Geometry `group_option`.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundingGroup {
    None,
    All,
    List(Vec<String>),
}

/// `arcpy.management.MinimumBoundingGeometry(in, out, geometry_type, group_option,
/// group_field, mbg_fields_option)`.
pub fn minimum_bounding_geometry(
    input: &dyn FeatureClass,
    kind: geoforge_geometry::BoundingKind,
    group: &BoundingGroup,
    mbg_fields: bool,
) -> Result<ToolOutput> {
    use geoforge_geometry::BoundingKind as K;
    input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let deviation = geoforge_core::curves::arcgis_densify_deviation(
        input
            .spatial_reference_wkt()
            .is_some_and(|w| w.trim_start().starts_with("GEOGCS")),
    );
    let features: Vec<Feature> = load(input)?;
    // (attributes, members)
    let mut groups: Vec<(Vec<Value>, Vec<&Feature>)> = Vec::new();
    let mut fields: Vec<Field> = Vec::new();
    match group {
        BoundingGroup::None => {
            fields = with_orig_fid(input);
            for f in &features {
                groups.push((
                    with_value(&f.attributes, [Value::Integer(f.oid as i32)]),
                    vec![f],
                ));
            }
        }
        BoundingGroup::All => groups.push((Vec::new(), features.iter().collect())),
        BoundingGroup::List(names) => {
            let keys: Vec<usize> = names
                .iter()
                .map(|n| {
                    input
                        .fields()
                        .iter()
                        .position(|f| f.name.eq_ignore_ascii_case(n))
                        .ok_or_else(|| {
                            Error::Unsupported(format!(
                                "ERROR 000728: Field {n} does not exist within table"
                            ))
                        })
                })
                .collect::<Result<_>>()?;
            fields = keys.iter().map(|&i| input.fields()[i].clone()).collect();
            for f in &features {
                let key: Vec<Value> = keys.iter().map(|&i| f.attributes[i].clone()).collect();
                match groups.iter_mut().find(|(k, _)| k == &key) {
                    Some((_, m)) => m.push(f),
                    None => groups.push((key, vec![f])),
                }
            }
            groups.sort_by(|a, b| {
                a.0.iter()
                    .zip(&b.0)
                    .map(|(x, y)| compare(x, y))
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
    let measure = |name: &str| {
        let mut f = Field::new(name, FieldType::Double);
        f.precision = 9;
        f.scale = 3;
        f
    };
    if mbg_fields {
        match kind {
            K::Circle => fields.push(measure("MBG_Diameter")),
            K::Envelope => {
                fields.push(measure("MBG_Width"));
                fields.push(measure("MBG_Length"));
            }
            _ => {
                fields.push(measure("MBG_Width"));
                fields.push(measure("MBG_Length"));
                fields.push(measure("MBG_Orientation"));
            }
        }
    }
    let mut out = Vec::new();
    for (attrs, members) in groups {
        let Some(b) = geoforge_geometry::bounding(
            members.iter().filter_map(|f| f.geometry.as_ref()),
            kind,
            deviation,
        )?
        else {
            continue;
        };
        let mut attrs = attrs;
        if mbg_fields {
            match kind {
                K::Circle => attrs.push(Value::Double(b.diameter)),
                K::Envelope => {
                    attrs.push(Value::Double(b.width));
                    attrs.push(Value::Double(b.length));
                }
                _ => {
                    attrs.push(Value::Double(b.width));
                    attrs.push(Value::Double(b.length));
                    attrs.push(Value::Double(b.orientation));
                }
            }
        }
        out.push(Feature {
            oid: out.len() as i64,
            geometry: Some(b.geometry),
            attributes: attrs,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features: out,
        messages: Vec::new(),
    })
}

/// `arcpy.management.AddXY(in_features)`: `POINT_X`, `POINT_Y` (and `POINT_Z`,
/// `POINT_M` for Z/M-aware data) added to point features (updated in place).
pub fn add_xy(input: &dyn FeatureClass) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    if kind != GeometryKind::Point {
        return Err(Error::Unsupported(
            "ERROR 000187: Only supports Geometry type Point.".into(),
        ));
    }
    let mut fields = input.fields().to_vec();
    let mut names = vec!["POINT_X", "POINT_Y"];
    if input.has_z() {
        names.push("POINT_Z");
    }
    if input.has_m() {
        names.push("POINT_M");
    }
    for n in &names {
        if !fields.iter().any(|f| f.name.eq_ignore_ascii_case(n)) {
            fields.push(Field::new(*n, FieldType::Double));
        }
    }
    let pos: Vec<usize> = names
        .iter()
        .map(|n| {
            fields
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(n))
                .unwrap()
        })
        .collect();
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        f.attributes.resize(fields.len(), Value::Null);
        if let Some(g) = f.geometry.as_ref().filter(|g| !g.xy.is_empty()) {
            let values = [
                Some(g.xy[0][0]),
                Some(g.xy[0][1]),
                g.z.as_ref().map(|z| z[0]),
                g.m.as_ref().map(|m| m[0]),
            ];
            let values: Vec<Option<f64>> = values
                .into_iter()
                .enumerate()
                .filter(|(i, _)| *i < 2 || (*i == 2 && input.has_z()) || (*i == 3 && input.has_m()))
                .map(|(_, v)| v)
                .collect();
            for (k, v) in pos.iter().zip(values) {
                f.attributes[*k] = v.map_or(Value::Null, Value::Double);
            }
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// A table result (no geometry): fields and rows.
#[derive(Debug, Clone)]
pub struct TableOutput {
    pub fields: Vec<Field>,
    pub rows: Vec<Vec<Value>>,
}

/// `arcpy.conversion.FeatureClassToFeatureClass(in, out_path, out_name, where_clause,
/// field_mapping)`: the features the clause selects, with the output fields the mapping
/// describes (its sources are the input's fields; `source` is the input's path, which
/// tells them from other tables) or all fields as they are.
pub fn feature_class_to_feature_class(
    input: &dyn FeatureClass,
    source: &str,
    where_clause: Option<&str>,
    field_map: Option<&[FieldMapEntry]>,
) -> Result<ToolOutput> {
    let mut out = select(input, where_clause)?;
    let Some(map) = field_map else {
        return Ok(out);
    };
    let stem = table_stem(source);
    // Per output field: the input position and text span of its first source.
    let positions: Vec<Option<(usize, i64, i64)>> = map
        .iter()
        .map(|e| {
            e.sources
                .iter()
                .filter(|src| table_stem(&src.table) == stem)
                .find_map(|src| {
                    input
                        .fields()
                        .iter()
                        .position(|g| g.name.eq_ignore_ascii_case(&src.field))
                        .map(|p| (p, src.start, src.end))
                })
        })
        .collect();
    for f in &mut out.features {
        f.attributes = positions
            .iter()
            .zip(map)
            .map(|(p, e)| match p {
                None => Value::Null,
                Some((p, a, b)) => {
                    fieldmap::coerce(&fieldmap::substring(&f.attributes[*p], *a, *b), &e.field)
                }
            })
            .collect();
    }
    out.fields = map.iter().map(|e| e.field.clone()).collect();
    Ok(out)
}

/// Sort's `spatial_sort_method`: the corner of the extent a `Shape` key starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialSort {
    /// Upper right: top to bottom, right to left.
    UpperRight,
    UpperLeft,
    LowerRight,
    LowerLeft,
}

impl SpatialSort {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "UR" => Self::UpperRight,
            "UL" => Self::UpperLeft,
            "LR" => Self::LowerRight,
            "LL" => Self::LowerLeft,
            _ => return None,
        })
    }
}

/// `arcpy.management.Sort(in, out, sort_field, spatial_sort_method)`: the features
/// ordered by the keys in turn (`ascending` per key; the field `Shape` sorts by the
/// envelope corner `spatial` names — its y, then its x — and is ignored if absent), ties
/// in input order. Text compares by bytes. FIDs are renumbered, the old one kept in
/// `ORIG_FID`.
pub fn sort(
    input: &dyn FeatureClass,
    keys: &[(String, bool)],
    spatial: SpatialSort,
) -> Result<ToolOutput> {
    use std::cmp::Ordering;
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    // Each key is a field position, or None for Shape.
    let mut resolved: Vec<(Option<usize>, bool)> = Vec::new();
    for (name, ascending) in keys {
        if name.eq_ignore_ascii_case("shape") {
            resolved.push((None, *ascending));
        } else {
            let i = input
                .fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    Error::Unsupported(format!(
                        "ERROR 000728: Field {name} does not exist within table"
                    ))
                })?;
            resolved.push((Some(i), *ascending));
        }
    }
    let mut features: Vec<Feature> = input.features().collect::<Result<_>>()?;
    let corner = |f: &Feature| -> (f64, f64) {
        let e = f
            .geometry
            .as_ref()
            .map_or(geoforge_core::Envelope::EMPTY, |g| g.envelope());
        let x = match spatial {
            SpatialSort::UpperRight | SpatialSort::LowerRight => e.xmax,
            _ => e.xmin,
        };
        let y = match spatial {
            SpatialSort::UpperRight | SpatialSort::UpperLeft => e.ymax,
            _ => e.ymin,
        };
        // Order: upper corners first top to bottom, right corners first right to left.
        let sy = if matches!(spatial, SpatialSort::UpperRight | SpatialSort::UpperLeft) {
            -1.0
        } else {
            1.0
        };
        let sx = if matches!(spatial, SpatialSort::UpperRight | SpatialSort::LowerRight) {
            -1.0
        } else {
            1.0
        };
        (sy * y, sx * x)
    };
    features.sort_by(|a, b| {
        for &(key, ascending) in &resolved {
            let ord = match key {
                Some(i) => compare_values(&a.attributes[i], &b.attributes[i]),
                None => {
                    let (ca, cb) = (corner(a), corner(b));
                    ca.0.total_cmp(&cb.0).then(ca.1.total_cmp(&cb.1))
                }
            };
            let ord = if ascending { ord } else { ord.reverse() };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });
    for (i, f) in features.iter_mut().enumerate() {
        f.attributes.push(Value::Integer(f.oid as i32));
        f.oid = i as i64;
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: with_orig_fid(input),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// Nulls first, numbers by value, text by bytes.
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        (Value::String(x), Value::String(y)) => x.as_bytes().cmp(y.as_bytes()),
        _ => compare(a, b),
    }
}

/// `arcpy.analysis.Select(in, out, where_clause)`: the features the clause
/// selects (all without one).
pub fn select(input: &dyn FeatureClass, where_clause: Option<&str>) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let clause = where_clause
        .filter(|w| !w.trim().is_empty())
        .map(|w| WhereClause::parse(w, input.fields()))
        .transpose()?;
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        if clause.as_ref().is_none_or(|c| c.matches(&f.attributes)) {
            features.push(f);
        }
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        messages: if features.is_empty() {
            vec!["WARNING 000117: Warning empty output generated.".into()]
        } else {
            Vec::new()
        },
        features,
    })
}

fn field_positions(input: &dyn FeatureClass, names: &[String]) -> Result<Vec<usize>> {
    names
        .iter()
        .map(|n| {
            input
                .fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(n))
                .ok_or_else(|| {
                    Error::Unsupported(format!(
                        "ERROR 000728: Field {n} does not exist within table"
                    ))
                })
        })
        .collect()
}

/// Rows grouped by the values at `keys`, groups sorted by those values.
fn group_rows(rows: Vec<Vec<Value>>, keys: &[usize]) -> Vec<(Vec<Value>, Vec<Vec<Value>>)> {
    let mut groups: Vec<(Vec<Value>, Vec<Vec<Value>>)> = Vec::new();
    for r in rows {
        let key: Vec<Value> = keys.iter().map(|&i| r[i].clone()).collect();
        match groups.iter_mut().find(|(k, _)| k == &key) {
            Some((_, m)) => m.push(r),
            None => groups.push((key, vec![r])),
        }
    }
    groups.sort_by(|a, b| {
        a.0.iter()
            .zip(&b.0)
            .map(|(x, y)| compare(x, y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    groups
}

/// `arcpy.analysis.Statistics(in, out_table, statistics_fields, case_field)`: per
/// combination of the case fields (sorted), FREQUENCY and each `PREFIX_FIELD`.
pub fn statistics(
    input: &dyn FeatureClass,
    stats: &[(String, Statistic)],
    case_fields: &[String],
) -> Result<TableOutput> {
    let keys = field_positions(input, case_fields)?;
    let stat_fields = field_positions(
        input,
        &stats.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
    )?;
    let mut fields: Vec<Field> = keys.iter().map(|&i| input.fields()[i].clone()).collect();
    fields.push(Field::new("FREQUENCY", FieldType::Integer));
    for (&i, (_, s)) in stat_fields.iter().zip(stats) {
        fields.push(s.field(&input.fields()[i]));
    }
    let rows: Vec<Vec<Value>> = input
        .features()
        .map(|f| f.map(|f| f.attributes))
        .collect::<Result<_>>()?;
    let mut out = Vec::new();
    for (key, members) in group_rows(rows, &keys) {
        let mut row = key;
        row.push(Value::Integer(members.len() as i32));
        for (&i, (_, s)) in stat_fields.iter().zip(stats) {
            let values: Vec<&Value> = members.iter().map(|m| &m[i]).collect();
            row.push(s.compute(&values, true));
        }
        out.push(row);
    }
    Ok(TableOutput { fields, rows: out })
}

/// `arcpy.analysis.Frequency(in, out_table, frequency_fields, summary_fields)`:
/// per combination of the frequency fields (sorted), FREQUENCY, the values, and
/// the sums of the summary fields.
pub fn frequency(
    input: &dyn FeatureClass,
    frequency_fields: &[String],
    summary_fields: &[String],
) -> Result<TableOutput> {
    let keys = field_positions(input, frequency_fields)?;
    let sums = field_positions(input, summary_fields)?;
    let mut fields = vec![Field::new("FREQUENCY", FieldType::Integer)];
    fields.extend(keys.iter().map(|&i| input.fields()[i].clone()));
    fields.extend(sums.iter().map(|&i| {
        let mut f = input.fields()[i].clone();
        f.field_type = FieldType::Double;
        f
    }));
    let rows: Vec<Vec<Value>> = input
        .features()
        .map(|f| f.map(|f| f.attributes))
        .collect::<Result<_>>()?;
    let mut out = Vec::new();
    for (key, members) in group_rows(rows, &keys) {
        let mut row = vec![Value::Integer(members.len() as i32)];
        row.extend(key);
        for &i in &sums {
            let values: Vec<&Value> = members.iter().map(|m| &m[i]).collect();
            row.push(Statistic::Sum.compute(&values, false));
        }
        out.push(row);
    }
    Ok(TableOutput { fields, rows: out })
}

/// One `Geometry_Properties` choice of Add Geometry Attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryProperty {
    Area,
    PerimeterLength,
    Length,
    Centroid,
    CentroidInside,
    Extent,
    LineStartMidEnd,
    LineBearing,
    PartCount,
    PointCount,
    PointXy,
}

impl GeometryProperty {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_uppercase().as_str() {
            "AREA" => Self::Area,
            "PERIMETER_LENGTH" => Self::PerimeterLength,
            "LENGTH" => Self::Length,
            "CENTROID" => Self::Centroid,
            "CENTROID_INSIDE" => Self::CentroidInside,
            "EXTENT" => Self::Extent,
            "LINE_START_MID_END" => Self::LineStartMidEnd,
            "LINE_BEARING" => Self::LineBearing,
            "PART_COUNT" => Self::PartCount,
            "POINT_COUNT" => Self::PointCount,
            "POINT_X_Y_Z_M" => Self::PointXy,
            _ => return None,
        })
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Area => &["POLY_AREA"],
            Self::PerimeterLength => &["PERIMETER"],
            Self::Length => &["LENGTH"],
            Self::Centroid => &["CENTROID_X", "CENTROID_Y"],
            Self::CentroidInside => &["INSIDE_X", "INSIDE_Y"],
            Self::Extent => &["EXT_MIN_X", "EXT_MIN_Y", "EXT_MAX_X", "EXT_MAX_Y"],
            Self::LineStartMidEnd => &["START_X", "START_Y", "MID_X", "MID_Y", "END_X", "END_Y"],
            Self::LineBearing => &["BEARING"],
            Self::PartCount => &["PART_COUNT"],
            Self::PointCount => &["PNT_COUNT"],
            Self::PointXy => &["POINT_X", "POINT_Y"],
        }
    }
}

/// `arcpy.management.AddGeometryAttributes(in, properties, length_unit,
/// area_unit)`: the properties' fields (doubles) added or overwritten; lengths
/// divided by `length_unit` and areas by `area_unit` (meters / square meters per
/// unit; 1 keeps the coordinate system's units).
pub fn add_geometry_attributes(
    input: &dyn FeatureClass,
    properties: &[GeometryProperty],
    length_unit: f64,
    area_unit: f64,
) -> Result<ToolOutput> {
    use GeometryProperty as P;
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let mut fields = input.fields().to_vec();
    let mut slots = Vec::new();
    for p in properties {
        for name in p.fields() {
            let i = fields
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| {
                    fields.push(Field::new(*name, FieldType::Double));
                    fields.len() - 1
                });
            slots.push(i);
        }
    }
    let mut features = Vec::new();
    for f in input.features() {
        let mut f = f?;
        f.attributes.resize(fields.len(), Value::Null);
        let mut values: Vec<Option<f64>> = Vec::new();
        let g = f.geometry.as_ref().filter(|g| !g.xy.is_empty());
        for p in properties {
            let Some(g) = g else {
                values.extend(std::iter::repeat_n(None, p.fields().len()));
                continue;
            };
            match p {
                P::Area => values.push(Some(geoforge_geometry::area(g)? / area_unit)),
                P::PerimeterLength | P::Length => {
                    values.push(Some(geoforge_geometry::length(g)? / length_unit));
                }
                P::Centroid => {
                    let c = geoforge_geometry::centroid(g)?;
                    values.extend([c.map(|c| c[0]), c.map(|c| c[1])]);
                }
                P::CentroidInside => {
                    let c = geoforge_geometry::inside_point(g)?;
                    values.extend([c.map(|c| c[0]), c.map(|c| c[1])]);
                }
                P::Extent => {
                    let e = g.envelope();
                    values.extend([Some(e.xmin), Some(e.ymin), Some(e.xmax), Some(e.ymax)]);
                }
                P::LineStartMidEnd => {
                    let mid = geoforge_geometry::center(g)?;
                    let (s, e) = (g.xy[0], g.xy[g.xy.len() - 1]);
                    values.extend([
                        Some(s[0]),
                        Some(s[1]),
                        mid.map(|m| m[0]),
                        mid.map(|m| m[1]),
                        Some(e[0]),
                        Some(e[1]),
                    ]);
                }
                P::LineBearing => {
                    let (s, e) = (g.xy[0], g.xy[g.xy.len() - 1]);
                    let b = (e[0] - s[0])
                        .atan2(e[1] - s[1])
                        .to_degrees()
                        .rem_euclid(360.0);
                    values.push(Some(b));
                }
                // Polygons count their exterior rings (holes belong to them).
                P::PartCount => values.push(Some(if g.kind == GeometryKind::Polygon {
                    geoforge_geometry::explode(g).len() as f64
                } else {
                    g.num_parts() as f64
                })),
                P::PointCount => values.push(Some(g.num_points() as f64)),
                P::PointXy => values.extend([Some(g.xy[0][0]), Some(g.xy[0][1])]),
            }
        }
        for (&i, v) in slots.iter().zip(values) {
            f.attributes[i] = v.map_or(Value::Null, Value::Double);
        }
        features.push(f);
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// `arcpy.management.PointsToLine(in, out, line_field, sort_field, close_line)`:
/// one line per value of `line_field` (sorted; one line without it) through its
/// points in `sort_field` order (else FID order).
pub fn points_to_line(
    input: &dyn FeatureClass,
    line_field: Option<&str>,
    sort_field: Option<&str>,
    close: bool,
) -> Result<ToolOutput> {
    let line_key = line_field
        .map(|n| field_positions(input, &[n.to_owned()]).map(|v| v[0]))
        .transpose()?;
    let sort_key = sort_field
        .map(|n| field_positions(input, &[n.to_owned()]).map(|v| v[0]))
        .transpose()?;
    let pts: Vec<Feature> = load(input)?;
    let keys: Vec<usize> = line_key.into_iter().collect();
    let rows: Vec<Vec<Value>> = pts
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut r = f.attributes.clone();
            r.push(Value::Integer(i as i32));
            r
        })
        .collect();
    let idx = input.fields().len();
    let mut features = Vec::new();
    for (key, mut members) in group_rows(rows, &keys) {
        if let Some(s) = sort_key {
            members.sort_by(|a, b| compare(&a[s], &b[s]));
        }
        let mut xy: Vec<[f64; 2]> = members
            .iter()
            .filter_map(|m| match &m[idx] {
                Value::Integer(i) => pts[*i as usize]
                    .geometry
                    .as_ref()
                    .and_then(|g| g.xy.first().copied()),
                _ => None,
            })
            .collect();
        if close && xy.len() > 2 {
            xy.push(xy[0]);
        }
        if xy.len() < 2 {
            continue;
        }
        let mut g = geoforge_core::Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Polyline;
        g.parts = vec![0];
        g.xy = xy;
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(g),
            attributes: key,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: line_key
            .map(|i| vec![input.fields()[i].clone()])
            .unwrap_or_default(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// `arcpy.analysis.GenerateNearTable(in, near, out_table, search_radius, location,
/// angle, closest, closest_count)`: per input feature (FID order) its nearest
/// features, nearest first (ties by FID): only the closest, or all within the
/// radius up to `closest_count` (0 = no limit) with NEAR_RANK.
pub fn generate_near_table(
    input: &dyn FeatureClass,
    near: &dyn FeatureClass,
    search_radius: Option<f64>,
    location: bool,
    angle: bool,
    all: bool,
    closest_count: usize,
) -> Result<TableOutput> {
    generate_near_table_with(
        input,
        near,
        search_radius,
        location,
        angle,
        all,
        closest_count,
        NearMethod::Planar,
    )
}

/// The conservative search radius in degrees of longitude/latitude for `radius` metres around `g`.
fn degrees_for(g: &geoforge_core::Geometry, radius: f64) -> f64 {
    let env = g.curve_envelope();
    let lat = env.ymin.abs().max(env.ymax.abs()).min(89.0);
    radius / (110_574.0 * lat.to_radians().cos().max(0.017))
}

/// [`generate_near_table`] with a measuring method.
#[allow(clippy::too_many_arguments)]
pub fn generate_near_table_with(
    input: &dyn FeatureClass,
    near: &dyn FeatureClass,
    search_radius: Option<f64>,
    location: bool,
    angle: bool,
    all: bool,
    closest_count: usize,
    method: NearMethod,
) -> Result<TableOutput> {
    let near_features = Indexed::load(near)?;
    let mut fields = vec![
        Field::new("IN_FID", FieldType::Integer),
        Field::new("NEAR_FID", FieldType::Integer),
        Field::new("NEAR_DIST", FieldType::Double),
    ];
    if all {
        fields.push(Field::new("NEAR_RANK", FieldType::Integer));
    }
    if location {
        for n in ["FROM_X", "FROM_Y", "NEAR_X", "NEAR_Y"] {
            fields.push(Field::new(n, FieldType::Double));
        }
    }
    if angle {
        fields.push(Field::new("NEAR_ANGLE", FieldType::Double));
    }
    let mut rows = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let mut found: Vec<(usize, geoforge_geometry::Nearest)> = Vec::new();
        if all {
            let pad = match (method, search_radius) {
                (NearMethod::Geodesic, Some(r)) => degrees_for(g, r),
                (_, r) => r.unwrap_or(f64::INFINITY),
            };
            let candidates: Vec<usize> = if pad.is_finite() {
                near_features.candidates(g, pad)
            } else {
                (0..near_features.features.len()).collect()
            };
            for i in candidates {
                let Some(ng) = &near_features.features[i].geometry else {
                    continue;
                };
                let n = match method {
                    NearMethod::Planar => geoforge_geometry::nearest(g, ng)?,
                    NearMethod::Geodesic => geoforge_geometry::nearest_geodesic(g, ng)?,
                };
                if let Some(n) = n
                    && search_radius.is_none_or(|r| n.distance <= r)
                {
                    found.push((i, n));
                }
            }
            found.sort_by(|a, b| a.1.distance.total_cmp(&b.1.distance).then(a.0.cmp(&b.0)));
            if closest_count > 0 {
                found.truncate(closest_count);
            }
        } else if let Some(hit) = near_features.closest(g, search_radius, method)? {
            found.push(hit);
        }
        for (rank, (i, n)) in found.into_iter().enumerate() {
            let mut row = vec![
                Value::Integer(f.oid as i32),
                Value::Integer(near_features.features[i].oid as i32),
                Value::Double(n.distance),
            ];
            if all {
                row.push(Value::Integer(rank as i32 + 1));
            }
            if location {
                row.extend([n.from[0], n.from[1], n.to[0], n.to[1]].map(Value::Double));
            }
            if angle {
                let a = if n.distance == 0.0 {
                    0.0
                } else if method == NearMethod::Geodesic {
                    geoforge_geometry::azimuth(n.from, n.to)
                } else {
                    (n.to[1] - n.from[1])
                        .atan2(n.to[0] - n.from[0])
                        .to_degrees()
                };
                row.push(Value::Double(a));
            }
            rows.push(row);
        }
    }
    Ok(TableOutput { fields, rows })
}

/// `arcpy.management.FindIdentical(in, out_table, fields, xy_tolerance, z_tolerance,
/// output_record_option)`: IN_FID and FEAT_SEQ, the same sequence number for
/// records whose values (or shapes, field `Shape`) are identical; with
/// `only_duplicates`, only groups of two or more.
pub fn find_identical(
    input: &dyn FeatureClass,
    fields: &[String],
    only_duplicates: bool,
) -> Result<TableOutput> {
    let shape = fields.iter().any(|f| f.eq_ignore_ascii_case("Shape"));
    let names: Vec<String> = fields
        .iter()
        .filter(|f| !f.eq_ignore_ascii_case("Shape"))
        .cloned()
        .collect();
    let keys = field_positions(input, &names)?;
    let feats: Vec<Feature> = input.features().collect::<Result<_>>()?;
    // Groups in order of first appearance: (values, shape, FIDs).
    type Group = (Vec<Value>, Option<Vec<[f64; 2]>>, Vec<i64>);
    let mut groups: Vec<Group> = Vec::new();
    for f in &feats {
        let key: Vec<Value> = keys.iter().map(|&i| f.attributes[i].clone()).collect();
        let geom = shape.then(|| {
            f.geometry
                .as_ref()
                .map(|g| g.xy.clone())
                .unwrap_or_default()
        });
        match groups.iter_mut().find(|(k, g, _)| *k == key && *g == geom) {
            Some((_, _, ids)) => ids.push(f.oid),
            None => groups.push((key, geom, vec![f.oid])),
        }
    }
    let mut rows = Vec::new();
    let mut seq = 0;
    for (_, _, ids) in groups {
        if only_duplicates && ids.len() < 2 {
            continue;
        }
        seq += 1;
        for id in ids {
            rows.push(vec![Value::Integer(id as i32), Value::Integer(seq)]);
        }
    }
    Ok(TableOutput {
        fields: vec![
            Field::new("IN_FID", FieldType::Integer),
            Field::new("FEAT_SEQ", FieldType::Integer),
        ],
        rows,
    })
}

/// `arcpy.management.AddField(in_table, name, type, precision, scale, length)` for a
/// shapefile: the field is appended and filled with its default (0, empty text, no date).
/// A name that already exists (any case) of the same type only warns (WARNING 000012).
pub fn add_field(
    input: &dyn FeatureClass,
    name: &str,
    field_type: &str,
    precision: Option<u32>,
    scale: Option<u32>,
    length: Option<u32>,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    if name.chars().count() > 10 {
        return Err(Error::Unsupported(
            "ERROR 000313: The length of Field Name must not be larger than 10".into(),
        ));
    }
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(Error::Unsupported(
            "ERROR 000310: The Field name must not start with a number".into(),
        ));
    }
    let (ft, default) = match field_type.to_ascii_uppercase().as_str() {
        "TEXT" => (FieldType::String, Value::String(String::new())),
        "SHORT" => (FieldType::SmallInteger, Value::SmallInteger(0)),
        "LONG" => (FieldType::Integer, Value::Integer(0)),
        "FLOAT" => (FieldType::Single, Value::Single(0.0)),
        "DOUBLE" => (FieldType::Double, Value::Double(0.0)),
        "DATE" => (FieldType::Date, Value::Null),
        "BIGINTEGER" => {
            return Err(Error::Unsupported(
                "ERROR 003738: The data type is not supported by the workspace.".into(),
            ));
        }
        "BLOB" => {
            return Err(Error::Unsupported(
                "ERROR 001140: Data format does not support BLOB fields.".into(),
            ));
        }
        other => return Err(Error::Unsupported(format!("field type {other}"))),
    };
    let mut field = Field::new(name, ft);
    field.precision = precision.unwrap_or(0);
    field.scale = scale.unwrap_or(0);
    if ft == FieldType::String {
        field.length = length.unwrap_or(254);
    }
    let mut out = ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features: input.features().collect::<Result<_>>()?,
        messages: Vec::new(),
    };
    // The name as the shapefile will keep it (spaces and symbols become `_`).
    let stored: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if let Some(existing) = out
        .fields
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(&stored))
    {
        if existing.field_type == ft {
            out.messages
                .push(format!("WARNING 000012: {name} already exists"));
            return Ok(out);
        }
        return Err(Error::Unsupported(format!(
            "ERROR 000012: {name} already exists"
        )));
    }
    if stored != name {
        out.messages
            .push(format!("WARNING 000304: Renamed field {name} to {stored}"));
    }
    out.fields.push(field);
    for f in &mut out.features {
        f.attributes.push(default.clone());
    }
    Ok(out)
}

/// `arcpy.management.DeleteField(in_table, drop_field, method)`: the listed fields are
/// removed (`DELETE_FIELDS`), or every field but the listed ones (`KEEP_FIELDS`). A
/// shapefile keeps at least one attribute field (ERROR 002557); names that do not exist
/// are ignored.
pub fn delete_field(
    input: &dyn FeatureClass,
    names: &[String],
    keep_listed: bool,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let listed = |f: &Field| names.iter().any(|n| n.eq_ignore_ascii_case(&f.name));
    let keep: Vec<usize> = input
        .fields()
        .iter()
        .enumerate()
        .filter(|(_, f)| listed(f) == keep_listed)
        .map(|(i, _)| i)
        .collect();
    if keep.is_empty() && !input.fields().is_empty() {
        let last = input.fields().last().map_or("", |f| f.name.as_str());
        return Err(Error::Unsupported(format!(
            "ERROR 002557: Cannot delete field {last}"
        )));
    }
    let features = input
        .features()
        .map(|f| {
            f.map(|mut f| {
                f.attributes = keep.iter().map(|&i| f.attributes[i].clone()).collect();
                f
            })
        })
        .collect::<Result<_>>()?;
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: keep.iter().map(|&i| input.fields()[i].clone()).collect(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    })
}

/// A row value as the Python of a Calculate Field expression sees it.
fn to_py(v: &Value) -> pycalc::Py {
    use pycalc::Py;
    match v {
        Value::Null => Py::None,
        Value::SmallInteger(n) => Py::Int(i64::from(*n)),
        Value::Integer(n) => Py::Int(i64::from(*n)),
        Value::BigInteger(n) => Py::Int(*n),
        Value::Single(n) => Py::Float(f64::from(*n)),
        Value::Double(n) => Py::Float(*n),
        Value::String(s) | Value::Guid(s) | Value::Xml(s) => Py::Str(s.clone()),
        Value::Date(d) => Py::Str(d.format("%Y-%m-%d %H:%M:%S").to_string()),
        Value::DateOnly(d) => Py::Str(d.format("%Y-%m-%d").to_string()),
        _ => Py::None,
    }
}

/// A calculated value stored in `field`; `None` for a result the field cannot hold.
fn from_py(v: &pycalc::Py, field: &Field) -> Option<Value> {
    use pycalc::Py;
    if matches!(v, Py::None) {
        return Some(Value::Null);
    }
    let number = match v {
        Py::Bool(b) => Some(f64::from(u8::from(*b))),
        Py::Int(n) => Some(*n as f64),
        Py::Float(f) => Some(*f),
        _ => None,
    };
    match field.field_type {
        FieldType::SmallInteger => {
            let n = number?.round_ties_even();
            (n.abs() <= f64::from(i16::MAX)).then_some(Value::SmallInteger(n as i16))
        }
        FieldType::Integer => {
            let n = number?.round_ties_even();
            (n.abs() <= f64::from(i32::MAX)).then_some(Value::Integer(n as i32))
        }
        FieldType::BigInteger => Some(Value::BigInteger(number?.round_ties_even() as i64)),
        FieldType::Single => Some(Value::Single(number? as f32)),
        FieldType::Double => Some(Value::Double(number?)),
        FieldType::String => {
            let text = pycalc::to_text(v);
            Some(Value::String(if field.length > 0 {
                text.chars().take(field.length as usize).collect()
            } else {
                text
            }))
        }
        FieldType::Date => match v {
            Py::Str(s) => {
                let s = s.trim();
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
                    .or_else(|_| {
                        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                            .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                    })
                    .ok()
                    .map(Value::Date)
            }
            _ => None,
        },
        _ => None,
    }
}

/// `arcpy.management.CalculateField(in_table, field, expression, "PYTHON3", code_block,
/// field_type)` for a shapefile: the Python expression evaluated per row with `!FIELD!`
/// values, stored in `field` (created from `field_type` if it does not exist). A row whose
/// expression fails (or gives None) has no value to store, and shapefile fields cannot be
/// null: ERROR 160145.
pub fn calculate_field(
    input: &dyn FeatureClass,
    field: &str,
    expression: &str,
    code_block: &str,
    field_type: Option<&str>,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let mut out = match input
        .fields()
        .iter()
        .any(|f| f.name.eq_ignore_ascii_case(field))
    {
        true => ToolOutput {
            kind,
            has_z: input.has_z(),
            has_m: input.has_m(),
            fields: input.fields().to_vec(),
            spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
            features: input.features().collect::<Result<_>>()?,
            messages: Vec::new(),
        },
        false => match field_type {
            Some(t) if !t.trim().is_empty() => add_field(input, field, t, None, None, None)?,
            _ => {
                return Err(Error::Unsupported(format!(
                    "ERROR 000728: Field {field} does not exist within table"
                )));
            }
        },
    };
    let target = out
        .fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(field))
        .ok_or_else(|| Error::Unsupported(format!("ERROR 000728: Field {field} does not exist")))?;
    let interp = pycalc::Interp::new(code_block)
        .map_err(|e| Error::Unsupported(format!("ERROR 000539: Error running expression: {e}")))?;
    // Fields the expression names (the OID field counts as FID / OBJECTID).
    let tokens = pycalc::field_tokens(expression);
    let mut positions: Vec<(String, Option<usize>)> = Vec::new();
    for t in &tokens {
        let pos = out
            .fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(t));
        if pos.is_none() && !["FID", "OID", "OBJECTID"].contains(&t.as_str()) {
            return Err(Error::Unsupported(format!(
                "ERROR 000539: Invalid field {t}"
            )));
        }
        positions.push((t.clone(), pos));
    }
    let target_field = out.fields[target].clone();
    for f in &mut out.features {
        let mut row = std::collections::HashMap::new();
        for (name, pos) in &positions {
            row.insert(
                name.clone(),
                match pos {
                    Some(p) => to_py(&f.attributes[*p]),
                    None => pycalc::Py::Int(f.oid),
                },
            );
        }
        let value = interp
            .eval(expression, &row)
            .ok()
            .and_then(|v| from_py(&v, &target_field))
            .unwrap_or(Value::Null);
        if value.is_null() {
            return Err(Error::Unsupported(
                "ERROR 160145: The field is not nullable.".into(),
            ));
        }
        f.attributes[target] = value;
    }
    Ok(out)
}

/// `arcpy.management.DeleteIdentical(in_dataset, fields, xy_tolerance)`: the first of each
/// group of records whose `fields` agree (the field `Shape` compares geometries, vertex by
/// vertex within `xy_tolerance`), in input order; the others are deleted and FIDs closed up.
pub fn delete_identical(
    input: &dyn FeatureClass,
    fields: &[String],
    xy_tolerance: f64,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let shape = fields.iter().any(|f| f.eq_ignore_ascii_case("Shape"));
    let names: Vec<String> = fields
        .iter()
        .filter(|f| !f.eq_ignore_ascii_case("Shape"))
        .cloned()
        .collect();
    let keys = field_positions(input, &names)?;
    let same_shape =
        |a: &Option<geoforge_core::Geometry>, b: &Option<geoforge_core::Geometry>| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.parts == b.parts
                    && a.xy.len() == b.xy.len()
                    && a.xy.iter().zip(&b.xy).all(|(p, q)| {
                        (p[0] - q[0]).abs() <= xy_tolerance && (p[1] - q[1]).abs() <= xy_tolerance
                    })
            }
            _ => false,
        };
    // Kept features and, per kept feature, how many records duplicated it.
    let mut kept: Vec<(Feature, Vec<Value>, usize)> = Vec::new();
    for f in input.features() {
        let f = f?;
        let key: Vec<Value> = keys.iter().map(|&i| f.attributes[i].clone()).collect();
        match kept
            .iter_mut()
            .find(|(g, k, _)| *k == key && (!shape || same_shape(&g.geometry, &f.geometry)))
        {
            Some((_, _, dups)) => *dups += 1,
            None => kept.push((f, key, 0)),
        }
    }
    let deleted: usize = kept.iter().map(|k| k.2).sum();
    let groups = kept.iter().filter(|k| k.2 > 0).count();
    let features = kept
        .into_iter()
        .enumerate()
        .map(|(i, (f, ..))| Feature { oid: i as i64, ..f })
        .collect();
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: vec![format!(
            "{deleted} duplicate(s) from {groups} group(s) deleted."
        )],
    })
}

/// `arcpy.management.XYTableToPoint(in_table, out, x_field, y_field, z_field,
/// coordinate_system)`: a point per row at its X/Y (and Z) values, all fields
/// kept; rows without coordinates get no shape.
pub fn xy_table_to_point(
    table: &dyn FeatureClass,
    x_field: &str,
    y_field: &str,
    z_field: Option<&str>,
    spatial_reference_wkt: Option<String>,
) -> Result<ToolOutput> {
    let pos = |n: &str| field_positions(table, &[n.to_owned()]).map(|v| v[0]);
    let (xi, yi) = (pos(x_field)?, pos(y_field)?);
    let zi = z_field.map(pos).transpose()?;
    let mut features = Vec::new();
    for f in table.features() {
        let f = f?;
        let (x, y) = (number(&f.attributes[xi]), number(&f.attributes[yi]));
        let geometry = x.zip(y).map(|(x, y)| {
            let mut g = geoforge_core::Geometry::point(x, y);
            if let Some(zi) = zi {
                g.z = Some(vec![number(&f.attributes[zi]).unwrap_or(0.0)]);
            }
            g
        });
        features.push(Feature {
            oid: features.len() as i64,
            geometry,
            attributes: f.attributes,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Point,
        has_z: zi.is_some(),
        has_m: false,
        fields: table.fields().to_vec(),
        spatial_reference_wkt,
        features,
        messages: if zi.is_some() {
            vec!["WARNING 090111: No vertical coordinate system defined".into()]
        } else {
            Vec::new()
        },
    })
}
