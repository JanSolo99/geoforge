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
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    if clip_features.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(Error::Unsupported(
            "ERROR 000365: clip features must be polygons".into(),
        ));
    }
    let clip_geoms: Vec<_> = clip_features
        .features()
        .filter_map(|f| f.map(|f| f.geometry).transpose())
        .collect::<Result<_>>()?;
    let area = ClipArea::new(clip_geoms.iter())?;
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if let Some(mut out) = geoforge_geometry::clip(g, &area)? {
            if kind != GeometryKind::Point && kind != GeometryKind::Multipoint {
                zm::restore(&mut out, g, input.has_z(), input.has_m());
            }
            features.push(Feature {
                oid: f.oid,
                geometry: Some(out),
                attributes: f.attributes,
            });
        }
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
    let mut add = |mut f: Field| {
        let base = f.name.clone();
        let mut n = 1;
        while fields.iter().any(|g| g.name.eq_ignore_ascii_case(&f.name)) {
            f.name = format!("{base}_{n}");
            n += 1;
        }
        fields.push(f);
    };
    for input in inputs {
        add(Field::new(
            format!("FID_{}", input.name),
            FieldType::Integer,
        ));
        input.features.fields().iter().cloned().for_each(&mut add);
    }
    fields
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
        let mut next = Vec::new();
        for (fids, g) in &partial {
            for f in &data[i] {
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
                for (cell, members) in &cells {
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
    let partial: Vec<(geoforge_core::Geometry, Vec<Value>)> = rows
        .into_iter()
        .map(|(_, fids, g)| {
            (
                g,
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
        has_z: false,
        has_m: false,
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
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if let Some(out) = geoforge_geometry::erase(g, &area)? {
            features.push(Feature {
                oid: f.oid,
                geometry: Some(out),
                attributes: f.attributes,
            });
        }
    }
    Ok(ToolOutput {
        kind,
        has_z: false,
        has_m: false,
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
        has_z: false,
        has_m: false,
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
        let mut out = union(&[input, identity_features])?;
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
    let polys = load(identity_features.features)?;
    let area = ClipArea::new(polys.iter().filter_map(|f| f.geometry.as_ref()))?;
    let missing: Vec<Value> = std::iter::once(Value::Integer(-1))
        .chain(std::iter::repeat_n(
            Value::Null,
            identity_features.features.fields().len(),
        ))
        .collect();
    // (covered?, input FID, identity FID, geometry, attributes)
    let mut rows = Vec::new();
    for f in load(input.features)? {
        let g = f.geometry.as_ref().unwrap();
        let mut head = vec![Value::Integer(f.oid as i32)];
        head.extend(f.attributes.iter().cloned());
        if let Some(rest) = geoforge_geometry::erase(g, &area)? {
            let mut attrs = head.clone();
            attrs.extend(missing.iter().cloned());
            rows.push((false, f.oid, -1, rest, attrs));
        }
        for p in &polys {
            if let Some(common) = geoforge_geometry::intersection(g, p.geometry.as_ref().unwrap())?
            {
                let mut attrs = head.clone();
                attrs.push(Value::Integer(p.oid as i32));
                attrs.extend(p.attributes.iter().cloned());
                rows.push((true, f.oid, p.oid, common, attrs));
            }
        }
    }
    if dimension(kind) == 0 {
        rows.sort_by(|a, b| {
            let (pa, pb) = (
                a.3.xy.first().copied().unwrap_or_default(),
                b.3.xy.first().copied().unwrap_or_default(),
            );
            pa[1].total_cmp(&pb[1]).then(pa[0].total_cmp(&pb[0]))
        });
    } else {
        rows.sort_by_key(|a| (a.0, a.1, a.2));
    }
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
        kind,
        has_z: false,
        has_m: false,
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

/// A Merge input: its features and the path ArcGIS records in `MERGE_SRC`.
pub struct MergeInput<'a> {
    pub source: String,
    pub features: &'a dyn FeatureClass,
}

/// `arcpy.management.Merge(inputs, out, "", add_source)` with the default field
/// mapping: fields of all inputs matched by name (first definition wins, in input
/// order), features of each input in turn, null where an input lacks a field.
/// `add_source` appends `MERGE_SRC` (text, 254) holding each feature's input path.
pub fn merge(inputs: &[MergeInput], add_source: bool) -> Result<ToolOutput> {
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
