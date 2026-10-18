//! Spatial Statistics: Measuring Geographic Distributions (Mean Center, Median Center, Central
//! Feature, Standard Distance, Directional Distribution, Linear Directional Mean).
//!
//! Matched to ArcGIS Pro 3.7 through the bridge (`testdata/arcgis-reference/spatstat.json`):
//! - Features stand for their centroids (points as they are); weights come from a numeric field.
//! - With a case field there is one output feature per distinct value, in ascending order (Central
//!   Feature: in the order of the features it picks). Mean Center and Median Center copy the case
//!   field as a 254-wide text field and add a field named like the dimension field holding its
//!   (weighted) mean; Central Feature keeps the input's fields after `Id`.
//! - Standard Distance is `sqrt(sum(w * (dx^2 + dy^2)) / sum(w))`, population-style, times the
//!   number of deviations; its circle has 361 vertices from east, clockwise, one degree apart.
//! - Directional Distribution follows Esri's formulas (the ellipse is the rotated, weighted
//!   covariance); its polygon has 361 vertices, the first on the rotated minor axis' end. XStdDist
//!   is the deviation along whichever axis is nearer east-west.
//! - Linear Directional Mean: the mean direction of the lines' start-to-end bearings, `DirMean` is
//!   `90 - CompassA`, the Rayleigh p-value is `exp(-n R^2)` and the test statistic
//!   `2Z (1 + (Z - 2) / 4n)` with `Z = n R^2`; the orientation-only mean folds bearings onto
//!   `[0, 180)`. The result line is centered on the mean of the lines' midpoints and as long as
//!   their mean length (for the orientation-only mean, drawn from its front end to its back end).
//! - Median Center is Weiszfeld's iteration from the mean center. ArcGIS stops it by a rule not
//!   reproduced here (each step shows the same points as ArcGIS's, but ArcGIS stops after a
//!   different number of them), so results differ from ArcGIS's by up to a few metres.

use std::cmp::Ordering;

use geoforge_core::{
    Error, Feature, FeatureClass, Field, FieldType, Geometry, GeometryKind, Result, Value,
};

use crate::ToolOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// The value of a numeric attribute.
fn number(v: &Value) -> Option<f64> {
    Some(match v {
        Value::SmallInteger(v) => *v as f64,
        Value::Integer(v) => *v as f64,
        Value::BigInteger(v) => *v as f64,
        Value::Single(v) => *v as f64,
        Value::Double(v) => *v,
        _ => return None,
    })
}

fn is_numeric(t: FieldType) -> bool {
    matches!(
        t,
        FieldType::SmallInteger
            | FieldType::Integer
            | FieldType::BigInteger
            | FieldType::Single
            | FieldType::Double
    )
}

fn field_index(input: &dyn FeatureClass, name: &str, numeric: bool) -> Result<usize> {
    let i = input
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            err(format!(
                "ERROR 000728: Field {name} does not exist within table"
            ))
        })?;
    if numeric && !is_numeric(input.fields()[i].field_type) {
        return Err(err(format!("ERROR 000989: field {name} must be numeric")));
    }
    Ok(i)
}

/// A case value: numbers sort numerically, text alphabetically.
#[derive(Debug, Clone, PartialEq)]
enum Case {
    None,
    Num(f64),
    Text(String),
}

impl Case {
    fn of(v: &Value) -> Case {
        match number(v) {
            Some(n) => Case::Num(n),
            None => match v {
                Value::Null => Case::None,
                Value::String(s) => Case::Text(s.clone()),
                other => Case::Text(format!("{other:?}")),
            },
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Case::None => 0,
            Case::Num(_) => 1,
            Case::Text(_) => 2,
        }
    }

    fn cmp(&self, other: &Case) -> Ordering {
        match (self, other) {
            (Case::Num(a), Case::Num(b)) => a.total_cmp(b),
            (Case::Text(a), Case::Text(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

/// One input feature as the statistics see it.
struct Item {
    index: usize,
    xy: [f64; 2],
    weight: f64,
    case: Case,
    attrs: Vec<Value>,
    geometry: Geometry,
}

struct Options<'a> {
    weight: Option<&'a str>,
    case: Option<&'a str>,
}

fn items(input: &dyn FeatureClass, opt: &Options) -> Result<Vec<Item>> {
    input
        .geometry_kind()
        .ok_or_else(|| err("input has no geometry"))?;
    let w = opt
        .weight
        .filter(|n| !n.is_empty())
        .map(|n| field_index(input, n, true))
        .transpose()?;
    let c = opt
        .case
        .filter(|n| !n.is_empty())
        .map(|n| field_index(input, n, false))
        .transpose()?;
    let mut out = Vec::new();
    for (index, f) in input.features().enumerate() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let Some(xy) = (match g.kind {
            GeometryKind::Point => g.xy.first().copied(),
            _ => geoforge_geometry::centroid(g)?,
        }) else {
            continue;
        };
        let weight = match w {
            Some(i) => match f.attributes.get(i).and_then(number) {
                Some(v) if v < 0.0 => {
                    return Err(err("ERROR 001090: the weight field has negative values"));
                }
                Some(v) => v,
                None => continue,
            },
            None => 1.0,
        };
        let case = c.map_or(Case::None, |i| {
            f.attributes.get(i).map_or(Case::None, Case::of)
        });
        out.push(Item {
            index,
            xy,
            weight,
            case,
            attrs: f.attributes.clone(),
            geometry: g.clone(),
        });
    }
    if out.is_empty() {
        return Err(err(
            "ERROR 110018: the input has no features with geometry and weight",
        ));
    }
    Ok(out)
}

/// The items split by case value (ascending), or all of them.
fn groups(items: &[Item], by_case: bool) -> Vec<Vec<&Item>> {
    if !by_case {
        return vec![items.iter().collect()];
    }
    let mut keys: Vec<&Case> = Vec::new();
    for it in items {
        if !keys.iter().any(|k| **k == it.case) {
            keys.push(&it.case);
        }
    }
    keys.sort_by(|a, b| a.cmp(b));
    keys.into_iter()
        .map(|k| items.iter().filter(|it| it.case == *k).collect())
        .collect()
}

fn id_field() -> Field {
    let mut f = Field::new("Id", FieldType::Integer);
    f.precision = 6;
    f
}

fn double(name: &str) -> Field {
    Field::new(name, FieldType::Double)
}

/// The case field as the output copies it: text 254 wide, numbers as they are.
fn case_field(input: &dyn FeatureClass, name: &str) -> Result<Field> {
    let src = &input.fields()[field_index(input, name, false)?];
    let mut f = Field::new(src.name.clone(), src.field_type);
    if src.field_type == FieldType::String {
        f.length = 254;
    }
    Ok(f)
}

fn case_value(it: &Item, input: &dyn FeatureClass, name: &str) -> Value {
    field_index(input, name, false)
        .ok()
        .and_then(|i| it.attrs.get(i).cloned())
        .unwrap_or(Value::Null)
}

fn output(
    input: &dyn FeatureClass,
    kind: GeometryKind,
    fields: Vec<Field>,
    features: Vec<Feature>,
) -> ToolOutput {
    ToolOutput {
        kind,
        has_z: false,
        has_m: false,
        fields,
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages: Vec::new(),
    }
}

fn weighted_mean(g: &[&Item]) -> Result<[f64; 2]> {
    let total: f64 = g.iter().map(|i| i.weight).sum();
    if total <= 0.0 {
        return Err(err("ERROR 001091: the weights add up to zero"));
    }
    Ok([
        g.iter().map(|i| i.weight * i.xy[0]).sum::<f64>() / total,
        g.iter().map(|i| i.weight * i.xy[1]).sum::<f64>() / total,
    ])
}

/// Weiszfeld's iteration from the mean center (see the module notes on where ArcGIS stops).
fn weiszfeld(g: &[&Item]) -> Result<[f64; 2]> {
    // The start is the mean center, unweighted (observed: the weighted run starts there too).
    let n = g.len() as f64;
    let mut c = [
        g.iter().map(|i| i.xy[0]).sum::<f64>() / n,
        g.iter().map(|i| i.xy[1]).sum::<f64>() / n,
    ];
    if g.iter().map(|i| i.weight).sum::<f64>() <= 0.0 {
        return Err(err("ERROR 001091: the weights add up to zero"));
    }
    for _ in 0..100_000 {
        let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
        for i in g {
            let d = (i.xy[0] - c[0]).hypot(i.xy[1] - c[1]).max(1e-12);
            let w = i.weight / d;
            sx += w * i.xy[0];
            sy += w * i.xy[1];
            sw += w;
        }
        let next = [sx / sw, sy / sw];
        let step = (next[0] - c[0]).hypot(next[1] - c[1]);
        c = next;
        if step < 0.8 {
            break;
        }
    }
    Ok(c)
}

fn center(
    input: &dyn FeatureClass,
    weight: Option<&str>,
    case: Option<&str>,
    dimension: Option<&str>,
    median: bool,
) -> Result<ToolOutput> {
    let case = case.filter(|c| !c.is_empty());
    let dimension = dimension.filter(|c| !c.is_empty());
    let all = items(input, &Options { weight, case })?;
    let dim = dimension.map(|n| field_index(input, n, true)).transpose()?;
    let mut fields = vec![id_field(), double("XCoord"), double("YCoord")];
    if let Some(c) = case {
        fields.push(case_field(input, c)?);
    }
    if let Some(d) = dimension {
        fields.push(double(&input.fields()[field_index(input, d, true)?].name));
    }
    let mut features = Vec::new();
    for g in groups(&all, case.is_some()) {
        let c = if median {
            weiszfeld(&g)?
        } else {
            weighted_mean(&g)?
        };
        let mut attrs = vec![Value::Integer(0), Value::Double(c[0]), Value::Double(c[1])];
        if let Some(name) = case {
            attrs.push(case_value(g[0], input, name));
        }
        if let Some(d) = dim {
            let vals: Vec<(f64, f64)> = g
                .iter()
                .filter_map(|i| i.attrs.get(d).and_then(number).map(|v| (i.weight, v)))
                .collect();
            let total: f64 = vals.iter().map(|v| v.0).sum();
            attrs.push(if total > 0.0 {
                Value::Double(vals.iter().map(|v| v.0 * v.1).sum::<f64>() / total)
            } else {
                Value::Null
            });
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(Geometry::point(c[0], c[1])),
            attributes: attrs,
        });
    }
    Ok(output(input, GeometryKind::Point, fields, features))
}

/// Mean Center (`weight_field`, `case_field`, `dimension_field` optional).
pub fn mean_center(
    input: &dyn FeatureClass,
    weight: Option<&str>,
    case: Option<&str>,
    dimension: Option<&str>,
) -> Result<ToolOutput> {
    center(input, weight, case, dimension, false)
}

/// Median Center (approximate: see the module notes).
pub fn median_center(
    input: &dyn FeatureClass,
    weight: Option<&str>,
    case: Option<&str>,
    dimension: Option<&str>,
) -> Result<ToolOutput> {
    center(input, weight, case, dimension, true)
}

/// Central Feature: the feature with the smallest weighted sum of distances to the others.
/// `manhattan` selects MANHATTAN_DISTANCE; a self potential adds `weight * potential` to a
/// feature's total (observed).
pub fn central_feature(
    input: &dyn FeatureClass,
    manhattan: bool,
    weight: Option<&str>,
    self_potential: Option<&str>,
    case: Option<&str>,
) -> Result<ToolOutput> {
    let case = case.filter(|c| !c.is_empty());
    let all = items(input, &Options { weight, case })?;
    let sp = self_potential
        .filter(|n| !n.is_empty())
        .map(|n| field_index(input, n, true))
        .transpose()?;
    let mut picked: Vec<&Item> = Vec::new();
    for g in groups(&all, case.is_some()) {
        let mut best: Option<(f64, &Item)> = None;
        for i in &g {
            let mut cost = 0.0;
            for j in &g {
                let (dx, dy) = ((i.xy[0] - j.xy[0]).abs(), (i.xy[1] - j.xy[1]).abs());
                cost += j.weight * if manhattan { dx + dy } else { dx.hypot(dy) };
            }
            if let Some(k) = sp {
                cost += i.weight * i.attrs.get(k).and_then(number).unwrap_or(0.0);
            }
            if best.is_none_or(|(b, _)| cost < b) {
                best = Some((cost, i));
            }
        }
        picked.extend(best.map(|b| b.1));
    }
    // In the order of the features picked.
    picked.sort_by_key(|i| i.index);
    let mut fields = vec![id_field()];
    fields.extend(input.fields().iter().cloned());
    let features = picked
        .iter()
        .enumerate()
        .map(|(n, i)| Feature {
            oid: n as i64,
            geometry: Some(i.geometry.clone()),
            attributes: std::iter::once(Value::Integer(0))
                .chain(i.attrs.iter().cloned())
                .collect(),
        })
        .collect();
    Ok(output(
        input,
        input.geometry_kind().unwrap_or(GeometryKind::Point),
        fields,
        features,
    ))
}

fn polygon(ring: Vec<[f64; 2]>) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polygon;
    g.parts = vec![0];
    g.xy = ring;
    g
}

fn deviations(name: &str, text: &str) -> Result<f64> {
    match text.to_ascii_uppercase().as_str() {
        "1_STANDARD_DEVIATION" => Ok(1.0),
        "2_STANDARD_DEVIATIONS" => Ok(2.0),
        "3_STANDARD_DEVIATIONS" => Ok(3.0),
        other => Err(err(format!(
            "ERROR 000067: {name} {other}: 1_STANDARD_DEVIATION, 2_STANDARD_DEVIATIONS or 3_STANDARD_DEVIATIONS"
        ))),
    }
}

/// Standard Distance (`circle_size`: `1_STANDARD_DEVIATION`, `2_STANDARD_DEVIATIONS` or
/// `3_STANDARD_DEVIATIONS`).
pub fn standard_distance(
    input: &dyn FeatureClass,
    circle_size: &str,
    weight: Option<&str>,
    case: Option<&str>,
) -> Result<ToolOutput> {
    let k = deviations("circle size", circle_size)?;
    let case = case.filter(|c| !c.is_empty());
    let all = items(input, &Options { weight, case })?;
    let mut fields = vec![
        id_field(),
        double("CenterX"),
        double("CenterY"),
        double("StdDist"),
    ];
    if let Some(c) = case {
        fields.push(case_field(input, c)?);
    }
    let mut features = Vec::new();
    for g in groups(&all, case.is_some()) {
        let c = weighted_mean(&g)?;
        let total: f64 = g.iter().map(|i| i.weight).sum();
        let sd = (g
            .iter()
            .map(|i| i.weight * ((i.xy[0] - c[0]).powi(2) + (i.xy[1] - c[1]).powi(2)))
            .sum::<f64>()
            / total)
            .sqrt();
        let r = k * sd;
        // From east, clockwise, a degree at a time, closed.
        let ring = (0..=360)
            .map(|d| {
                let a = -(d as f64).to_radians();
                [c[0] + r * a.cos(), c[1] + r * a.sin()]
            })
            .collect();
        let mut attrs = vec![
            Value::Integer(0),
            Value::Double(c[0]),
            Value::Double(c[1]),
            Value::Double(r),
        ];
        if let Some(name) = case {
            attrs.push(case_value(g[0], input, name));
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(polygon(ring)),
            attributes: attrs,
        });
    }
    Ok(output(input, GeometryKind::Polygon, fields, features))
}

/// Directional Distribution, the standard deviational ellipse (`ellipse_size` as for Standard
/// Distance).
pub fn directional_distribution(
    input: &dyn FeatureClass,
    ellipse_size: &str,
    weight: Option<&str>,
    case: Option<&str>,
) -> Result<ToolOutput> {
    let k = deviations("ellipse size", ellipse_size)?;
    let case = case.filter(|c| !c.is_empty());
    let all = items(input, &Options { weight, case })?;
    let mut fields = vec![
        double("CenterX"),
        double("CenterY"),
        double("XStdDist"),
        double("YStdDist"),
        double("Rotation"),
    ];
    if let Some(c) = case {
        fields.push(case_field(input, c)?);
    }
    let mut features = Vec::new();
    for g in groups(&all, case.is_some()) {
        if g.len() < 3 {
            return Err(err(
                "ERROR 001092: a directional distribution needs at least three features",
            ));
        }
        let c = weighted_mean(&g)?;
        let total: f64 = g.iter().map(|i| i.weight).sum();
        let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
        for i in &g {
            let (x, y) = (i.xy[0] - c[0], i.xy[1] - c[1]);
            sxx += i.weight * x * x / total;
            syy += i.weight * y * y / total;
            sxy += i.weight * x * y / total;
        }
        let a = sxx - syy;
        let b = (a * a + 4.0 * sxy * sxy).sqrt();
        // tan(theta) = (A + B) / C; the ellipse is the same turned half a circle.
        let mut theta = if sxy == 0.0 {
            if a >= 0.0 {
                std::f64::consts::FRAC_PI_2
            } else {
                0.0
            }
        } else {
            ((a + b) / (2.0 * sxy)).atan()
        };
        if theta < 0.0 {
            theta += std::f64::consts::PI;
        }
        let (ct, st) = (theta.cos(), theta.sin());
        let along = |f: &dyn Fn(f64, f64) -> f64| {
            (2.0 * g
                .iter()
                .map(|i| i.weight * f(i.xy[0] - c[0], i.xy[1] - c[1]).powi(2))
                .sum::<f64>()
                / total)
                .sqrt()
        };
        let sx = along(&|x, y| x * ct - y * st);
        let sy = along(&|x, y| x * st + y * ct);
        // `sy` is along the compass bearing `theta`, `sx` across it. ArcGIS reports as XStdDist the
        // one of the two nearer east-west, which is `sy` when the east-west variance is the larger.
        let (along, across) = (k * sy, k * sx);
        let (xs, ys) = if a >= 0.0 {
            (along, across)
        } else {
            (across, along)
        };
        let ring = (0..=360)
            .map(|i| {
                // The first vertex is on the axis ArcGIS reports as X (observed on both branches).
                let t = (-(i as f64) - if a >= 0.0 { 90.0 } else { 0.0 }).to_radians();
                let (px, py) = (across * t.cos(), along * t.sin());
                let r = -theta;
                [
                    c[0] + px * r.cos() - py * r.sin(),
                    c[1] + px * r.sin() + py * r.cos(),
                ]
            })
            .collect();
        let mut attrs = vec![
            Value::Double(c[0]),
            Value::Double(c[1]),
            Value::Double(xs),
            Value::Double(ys),
            Value::Double(theta.to_degrees()),
        ];
        if let Some(name) = case {
            attrs.push(case_value(g[0], input, name));
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(polygon(ring)),
            attributes: attrs,
        });
    }
    Ok(output(input, GeometryKind::Polygon, fields, features))
}

/// The chi-square critical value (0.05, two degrees of freedom) ArcGIS compares the test with.
const CHI2_05_2: f64 = 5.991_464_547_107_979;

/// Linear Directional Mean (`orientation_only`: ORIENTATION_ONLY, else DIRECTION).
pub fn linear_directional_mean(
    input: &dyn FeatureClass,
    orientation_only: bool,
    case: Option<&str>,
) -> Result<ToolOutput> {
    if input.geometry_kind() != Some(GeometryKind::Polyline) {
        return Err(err("ERROR 000368: the input must be lines"));
    }
    let case = case.filter(|c| !c.is_empty());
    let all = items(input, &Options { weight: None, case })?;
    let mut fields = vec![
        id_field(),
        double("CompassA"),
        double("DirMean"),
        double("CirVar"),
        double("AveX"),
        double("AveY"),
        double("AveLen"),
        double("TestStat"),
        double("RefValue"),
        double("PValue"),
    ];
    let mut unif = Field::new("UnifTest", FieldType::String);
    unif.length = 254;
    fields.push(unif);
    if let Some(c) = case {
        fields.push(case_field(input, c)?);
    }
    let mut features = Vec::new();
    for g in groups(&all, case.is_some()) {
        let n = g.len() as f64;
        let (mut s, mut c_, mut mx, mut my, mut len) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for i in &g {
            let (Some(first), Some(last)) = (i.geometry.xy.first(), i.geometry.xy.last()) else {
                continue;
            };
            let (dx, dy) = (last[0] - first[0], last[1] - first[1]);
            let mut bearing = dx.atan2(dy);
            if orientation_only {
                bearing = bearing.rem_euclid(std::f64::consts::PI);
            }
            s += bearing.sin();
            c_ += bearing.cos();
            mx += (first[0] + last[0]) / 2.0;
            my += (first[1] + last[1]) / 2.0;
            len += dx.hypot(dy);
        }
        let r = s.hypot(c_) / n;
        let compass = s.atan2(c_).to_degrees().rem_euclid(360.0);
        let z = n * r * r;
        let test = 2.0 * z * (1.0 + (z - 2.0) / (4.0 * n));
        let p = (-z).exp();
        let (ax, ay, al) = (mx / n, my / n, len / n);
        let half = al / 2.0;
        let a = compass.to_radians();
        let mut line = Geometry::point(0.0, 0.0);
        line.kind = GeometryKind::Polyline;
        line.parts = vec![0];
        let (back, front) = (
            [ax - half * a.sin(), ay - half * a.cos()],
            [ax + half * a.sin(), ay + half * a.cos()],
        );
        // Orientation-only lines are drawn from the front to the back (observed).
        line.xy = if orientation_only {
            vec![front, back]
        } else {
            vec![back, front]
        };
        let mut attrs = vec![
            Value::Integer(0),
            Value::Double(compass),
            Value::Double((90.0 - compass).rem_euclid(360.0)),
            Value::Double(1.0 - r),
            Value::Double(ax),
            Value::Double(ay),
            Value::Double(al),
            Value::Double(test),
            Value::Double(CHI2_05_2),
            Value::Double(p),
            Value::String(
                if test < CHI2_05_2 {
                    "Uniform"
                } else {
                    "No uniform"
                }
                .into(),
            ),
        ];
        if let Some(name) = case {
            attrs.push(case_value(g[0], input, name));
        }
        features.push(Feature {
            oid: features.len() as i64,
            geometry: Some(line),
            attributes: attrs,
        });
    }
    Ok(output(input, GeometryKind::Polyline, fields, features))
}
