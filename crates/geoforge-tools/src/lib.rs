//! Geoprocessing tools. Each follows its ArcGIS Pro counterpart (parameters,
//! output schema, feature order, messages) as observed through the ArcGIS bridge
//! and recorded in `testdata/arcgis-reference/manifest.json`.

mod zm;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dissolve {
    None,
    All,
}

/// `arcpy.analysis.Buffer(in, out, distance, "FULL", line_end, dissolve)`, planar.
///
/// `distance` is in the input's linear unit. Without dissolving, the output has the
/// input fields plus `BUFF_DIST` and `ORIG_FID`, one feature per input feature
/// (in input order); with `Dissolve::All` it is a single feature with no fields.
pub fn buffer(
    input: &dyn FeatureClass,
    distance: f64,
    line_end: LineEnd,
    dissolve: Dissolve,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let sr = input.spatial_reference_wkt();
    if sr.is_some_and(|w| w.trim_start().starts_with("GEOGCS")) {
        return Err(Error::Unsupported(
            "buffering geographic data needs geodesic buffers (coordinate systems are Phase 4)"
                .into(),
        ));
    }
    let opts = BufferOptions {
        distance,
        line_end,
        max_deviation: geoforge_core::curves::arcgis_densify_deviation(false),
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
            let geoms: Vec<_> = input
                .features()
                .filter_map(|f| f.map(|f| f.geometry).transpose())
                .collect::<Result<_>>()?;
            if let Some(g) = geoforge_geometry::buffer_union(geoms.iter(), &opts)? {
                out.features.push(Feature {
                    oid: 1,
                    geometry: Some(g),
                    attributes: Vec::new(),
                });
            }
        }
        Dissolve::None => {
            out.fields = input.fields().to_vec();
            out.fields.push(Field::new("BUFF_DIST", FieldType::Double));
            out.fields.push(Field::new("ORIG_FID", FieldType::Integer));
            for f in input.features() {
                let f = f?;
                let Some(g) = &f.geometry else { continue };
                let Some(b) = geoforge_geometry::buffer(g, &opts)? else {
                    continue;
                };
                let mut attributes = f.attributes;
                attributes.push(Value::Double(distance));
                attributes.push(Value::Integer(f.oid as i32));
                out.features.push(Feature {
                    oid: f.oid,
                    geometry: Some(b),
                    attributes,
                });
            }
        }
    }
    let _ = kind;
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
    use geoforge_geometry::{XY_TOLERANCE as TOL, on_path};
    let [a_in, b_in] = inputs else {
        return Err(Error::Unsupported(
            "Intersect LINE/POINT output of more than two inputs".into(),
        ));
    };
    for i in inputs {
        if dimension(i.features.geometry_kind().unwrap_or(GeometryKind::Point)) == 0 {
            return Err(Error::Unsupported(
                "Intersect LINE/POINT output with point inputs".into(),
            ));
        }
    }
    let (a_data, b_data) = (load(a_in.features)?, load(b_in.features)?);
    let b_index = geoforge_geometry::EnvelopeIndex::new(b_data.iter().map(|f| f.geometry.as_ref()));
    let mut meetings = Vec::new();
    for fa in &a_data {
        let candidates = b_index.candidates(&fa.geometry.as_ref().unwrap().curve_envelope(), TOL);
        for fb in candidates.into_iter().map(|k| &b_data[k]) {
            let m = geoforge_geometry::meeting(
                fa.geometry.as_ref().unwrap(),
                fb.geometry.as_ref().unwrap(),
                TOL,
            )?;
            if !m.points.is_empty() || !m.shared.is_empty() {
                meetings.push((fa, fb, m));
            }
        }
    }
    // Sort key: FIDs with polygon inputs first.
    let polygon_first = b_in.features.geometry_kind() == Some(GeometryKind::Polygon)
        && a_in.features.geometry_kind() != Some(GeometryKind::Polygon);
    let key = |a: i64, b: i64| if polygon_first { (b, a) } else { (a, b) };
    // (pass, key, geometry, attributes)
    type Row = (u8, (i64, i64), geoforge_core::Geometry, Vec<Value>);
    let mut rows: Vec<Row> = Vec::new();
    let attrs = |fa: &Feature, fb: &Feature| {
        let mut v = vec![Value::Integer(fa.oid as i32)];
        v.extend(fa.attributes.iter().cloned());
        v.push(Value::Integer(fb.oid as i32));
        v.extend(fb.attributes.iter().cloned());
        v
    };
    let multipoint = |pts: Vec<[f64; 2]>| {
        let mut g = geoforge_core::Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Multipoint;
        g.xy = pts;
        g
    };
    for (fa, fb, m) in &meetings {
        if !points {
            if let Some(g) = geoforge_geometry::paths_geometry(m.shared.clone()) {
                rows.push((0, key(fa.oid, fb.oid), g, attrs(fa, fb)));
            }
            continue;
        }
        let in_shared = |p: [f64; 2]| {
            meetings
                .iter()
                .any(|(_, _, o)| o.shared.iter().any(|s| on_path(p, s, TOL, true)))
        };
        let (late, first): (Vec<[f64; 2]>, Vec<[f64; 2]>) =
            m.points.iter().partition(|&&p| in_shared(p));
        let mut late = late;
        // Nodes made inside this pair's shared stretches by other pairs' points.
        for (_, _, o) in &meetings {
            for &p in &o.points {
                if m.shared.iter().any(|s| on_path(p, s, TOL, true))
                    && !late.iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) <= TOL)
                {
                    late.push(p);
                }
            }
        }
        if !first.is_empty() {
            rows.push((0, key(fa.oid, fb.oid), multipoint(first), attrs(fa, fb)));
        }
        if !late.is_empty() {
            rows.push((1, key(fa.oid, fb.oid), multipoint(late), attrs(fa, fb)));
        }
    }
    // Z and M from the two features, in input order.
    let (has_z, has_m) = zm_of(inputs);
    if has_z || has_m {
        let geom = |data: &[Feature], oid: i64| {
            data.iter()
                .find(|f| f.oid == oid)
                .and_then(|f| f.geometry.clone())
        };
        for r in &mut rows {
            let (oa, ob) = if polygon_first { (r.1.1, r.1.0) } else { r.1 };
            let (ga, gb) = (geom(&a_data, oa), geom(&b_data, ob));
            // POINT output takes Z from the polygon input first (its row order too).
            let sources: Vec<&geoforge_core::Geometry> = if points && polygon_first {
                gb.iter().chain(ga.iter()).collect()
            } else {
                ga.iter().chain(gb.iter()).collect()
            };
            zm::restore_from(&mut r.2, &sources, has_z, has_m, true);
        }
    }
    rows.sort_by_key(|r| (r.0, r.1));
    let features: Vec<Feature> = rows
        .into_iter()
        .enumerate()
        .map(|(i, r)| Feature {
            oid: i as i64,
            geometry: Some(r.2),
            attributes: r.3,
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
        spatial_reference_wkt: a_in.features.spatial_reference_wkt().map(str::to_owned),
        messages: if features.is_empty() {
            vec!["WARNING 000117: Warning empty output generated.".into()]
        } else {
            Vec::new()
        },
        features,
    })
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
                    rows.push((false, fids, piece.clone()));
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
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
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
        let src = &input.fields()[i];
        let field_type = match s {
            Statistic::Count => FieldType::Integer,
            Statistic::Min | Statistic::Max | Statistic::First | Statistic::Last => src.field_type,
            _ => FieldType::Double,
        };
        let mut f = Field::new(format!("{}_{}", s.prefix(), src.name), field_type);
        f.length = src.length;
        fields.push(f);
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
            let values: Vec<&Value> = members
                .iter()
                .map(|f| &f.attributes[i])
                .filter(|v| !v.is_null())
                .collect();
            let nums: Vec<f64> = values.iter().filter_map(|v| number(v)).collect();
            let n = nums.len() as f64;
            let mean = nums.iter().sum::<f64>() / n;
            attrs.push(match s {
                Statistic::Count => Value::Integer(values.len() as i32),
                Statistic::Sum => Value::Double(nums.iter().sum()),
                Statistic::Mean => {
                    if nums.is_empty() {
                        Value::Null
                    } else {
                        Value::Double(mean)
                    }
                }
                Statistic::Range => nums
                    .iter()
                    .copied()
                    .reduce(f64::max)
                    .zip(nums.iter().copied().reduce(f64::min))
                    .map_or(Value::Null, |(a, b)| Value::Double(a - b)),
                // ArcGIS reports the population standard deviation.
                Statistic::Std => {
                    if nums.is_empty() {
                        Value::Null
                    } else {
                        Value::Double(
                            (nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt(),
                        )
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
                Statistic::First => members
                    .first()
                    .map_or(Value::Null, |f| f.attributes[i].clone()),
                Statistic::Last => members
                    .last()
                    .map_or(Value::Null, |f| f.attributes[i].clone()),
            });
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
    for i in inputs {
        for f in i.features.fields() {
            if !fields.iter().any(|g| g.name.eq_ignore_ascii_case(&f.name)) {
                fields.push(f.clone());
            }
        }
    }
    let mut features = Vec::new();
    for i in inputs {
        let positions: Vec<Option<usize>> = fields
            .iter()
            .map(|f| {
                i.features
                    .fields()
                    .iter()
                    .position(|g| g.name.eq_ignore_ascii_case(&f.name))
            })
            .collect();
        for f in i.features.features() {
            let f = f?;
            let mut attributes: Vec<Value> = positions
                .iter()
                .map(|p| p.map_or(Value::Null, |p| f.attributes[p].clone()))
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
fn push_unique(fields: &mut Vec<Field>, mut f: Field) {
    let base = f.name.clone();
    let mut n = 1;
    while fields.iter().any(|g| g.name.eq_ignore_ascii_case(&f.name)) {
        f.name = format!("{base}_{n}");
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
    ) -> Result<Option<(usize, geoforge_geometry::Nearest)>> {
        self.index
            .closest(g, |i| self.features[i].geometry.as_ref(), radius)
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
                if let Some((i, n)) = candidates.closest(g, search_radius)?
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
}

impl MatchOption {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "INTERSECT" => Self::Intersect,
            "WITHIN_A_DISTANCE" => Self::WithinADistance,
            "CONTAINS" => Self::Contains,
            "COMPLETELY_CONTAINS" => Self::CompletelyContains,
            "WITHIN" => Self::Within,
            "COMPLETELY_WITHIN" => Self::CompletelyWithin,
            "CLOSEST" => Self::Closest,
            _ => return None,
        })
    }
}

/// Spatial Join options beyond the inputs.
#[derive(Debug, Clone)]
pub struct SpatialJoinOptions {
    /// `JOIN_ONE_TO_MANY` (else one to one).
    pub one_to_many: bool,
    /// `KEEP_ALL` (else `KEEP_COMMON`).
    pub keep_all: bool,
    pub match_option: MatchOption,
    pub search_radius: Option<f64>,
    pub distance_field_name: Option<String>,
}

/// `arcpy.analysis.SpatialJoin(target, join, out, ...)` with the default field
/// mapping (merge rule First): `Join_Count`, the optional distance field,
/// `TARGET_FID` (and `JOIN_FID` one to many), then target and join fields.
/// The distance field is only filled for `CLOSEST` (-1 otherwise, as in ArcGIS).
/// Matches are taken in join-FID order; one to one keeps the first's attributes,
/// one to many writes a row per match (`Join_Count` 1, or 0 with `JOIN_FID` -1
/// for an unmatched target kept by `KEEP_ALL`).
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
    for f in target.fields().iter().chain(join.fields()) {
        push_unique(&mut fields, f.clone());
    }
    let radius = opts.search_radius.unwrap_or(0.0);
    let mut features = Vec::new();
    for t in target.features() {
        let t = t?;
        // (join index, distance)
        let mut matches: Vec<(usize, f64)> = Vec::new();
        if let Some(tg) = &t.geometry {
            if opts.match_option == MatchOption::Closest {
                if let Some((i, n)) = joins.closest(tg, opts.search_radius)? {
                    matches.push((i, n.distance));
                }
            } else {
                for i in joins.candidates(tg, radius) {
                    let jg = joins.features[i].geometry.as_ref().unwrap();
                    let Some(n) = geoforge_geometry::nearest(tg, jg)? else {
                        continue;
                    };
                    let hit = match opts.match_option {
                        MatchOption::Intersect | MatchOption::WithinADistance => {
                            n.distance <= radius
                        }
                        MatchOption::Within => geoforge_geometry::within(tg, jg)?,
                        MatchOption::CompletelyWithin => {
                            geoforge_geometry::completely_within(tg, jg)?
                        }
                        MatchOption::Contains => geoforge_geometry::within(jg, tg)?,
                        MatchOption::CompletelyContains => {
                            geoforge_geometry::completely_within(jg, tg)?
                        }
                        MatchOption::Closest => unreachable!(),
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
                    Some((_, d)) if opts.match_option == MatchOption::Closest => d,
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
        let mut rows = Vec::new();
        if matches.is_empty() {
            if opts.keep_all {
                rows.push(row(0, None));
            }
        } else if opts.one_to_many {
            rows.extend(matches.iter().map(|&m| row(1, Some(m))));
        } else {
            rows.push(row(matches.len(), Some(matches[0])));
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
        messages: Vec::new(),
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
    let kind = target.geometry_kind();
    let fields = target.fields().to_vec();
    let mut features: Vec<Feature> = target.features().collect::<Result<_>>()?;
    let mut messages = Vec::new();
    for input in inputs {
        let same_kind = input.geometry_kind() == kind;
        let positions: Vec<Option<usize>> = fields
            .iter()
            .map(|f| {
                input
                    .fields()
                    .iter()
                    .position(|g| g.name.eq_ignore_ascii_case(&f.name))
            })
            .collect();
        if test {
            let same = input.fields().len() == fields.len()
                && positions
                    .iter()
                    .zip(&fields)
                    .all(|(p, f)| p.is_some_and(|p| input.fields()[p].field_type == f.field_type));
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
                    .map(|p| p.map_or(Value::Null, |p| f.attributes[p].clone()))
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
