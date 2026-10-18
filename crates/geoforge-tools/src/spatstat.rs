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

// ---------------------------------------------------------------------------------------------
// Analyzing patterns and mapping clusters

use crate::registry::Report;

/// A two-sided p-value of a standard normal z-score (ArcGIS's own comes from an approximation that
/// is up to 2e-8 away from this exact one).
pub fn p_value(z: f64) -> f64 {
    erfc(z.abs() / std::f64::consts::SQRT_2)
}

/// The complementary error function: the series `erf(x) = 2/sqrt(pi) e^-x^2 sum 2^n x^(2n+1) /
/// (1 3 ... (2n+1))` (all terms positive) below 3, a continued fraction above.
fn erfc(x: f64) -> f64 {
    let ax = x.abs();
    let r = if ax < 2.0 {
        let x2 = ax * ax;
        let (mut term, mut sum) = (ax, ax);
        for n in 1..200 {
            term *= 2.0 * x2 / (2 * n + 1) as f64;
            sum += term;
            if term < 1e-18 * sum {
                break;
            }
        }
        1.0 - 2.0 / std::f64::consts::PI.sqrt() * (-x2).exp() * sum
    } else {
        let mut f = 0.0;
        for k in (1..=200).rev() {
            f = k as f64 / 2.0 / (ax + f);
        }
        (-ax * ax).exp() / (std::f64::consts::PI.sqrt() * (ax + f))
    };
    if x < 0.0 { 2.0 - r } else { r }
}

/// How distances are measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMethod {
    Euclidean,
    Manhattan,
}

impl DistanceMethod {
    pub fn parse(s: &str) -> Option<DistanceMethod> {
        match s.to_ascii_uppercase().as_str() {
            "EUCLIDEAN_DISTANCE" | "EUCLIDEAN" | "" => Some(DistanceMethod::Euclidean),
            "MANHATTAN_DISTANCE" | "MANHATTAN" => Some(DistanceMethod::Manhattan),
            _ => None,
        }
    }

    fn between(self, a: [f64; 2], b: [f64; 2]) -> f64 {
        match self {
            DistanceMethod::Euclidean => (a[0] - b[0]).hypot(a[1] - b[1]),
            DistanceMethod::Manhattan => (a[0] - b[0]).abs() + (a[1] - b[1]).abs(),
        }
    }
}

/// How neighbors weigh on each other (Conceptualization of Spatial Relationships).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Concept {
    InverseDistance,
    InverseDistanceSquared,
    FixedDistanceBand,
    ZoneOfIndifference,
    /// The `k` nearest neighbors (8 for the autocorrelation tools).
    KNearestNeighbors(usize),
}

impl Concept {
    pub fn parse(s: &str, neighbors: usize) -> Option<Concept> {
        Some(match s.to_ascii_uppercase().as_str() {
            "INVERSE_DISTANCE" => Concept::InverseDistance,
            "INVERSE_DISTANCE_SQUARED" => Concept::InverseDistanceSquared,
            "FIXED_DISTANCE_BAND" => Concept::FixedDistanceBand,
            "ZONE_OF_INDIFFERENCE" => Concept::ZoneOfIndifference,
            "K_NEAREST_NEIGHBORS" => Concept::KNearestNeighbors(neighbors),
            _ => return None,
        })
    }
}

/// Spatial weights as rows of `(neighbor, weight)`, sorted by neighbor; a feature is not its own
/// neighbor (Hot Spot Analysis adds itself at weight 1).
struct Weights {
    rows: Vec<Vec<(usize, f64)>>,
}

const MAX_FEATURES: usize = 8000;

impl Weights {
    /// `band` is the threshold distance (0 for none). Fixed bands and the zone of indifference need
    /// one; inverse distance with none relates every pair.
    fn build(
        points: &[[f64; 2]],
        concept: Concept,
        method: DistanceMethod,
        band: f64,
    ) -> Result<Weights> {
        let n = points.len();
        let needs_band = matches!(
            concept,
            Concept::FixedDistanceBand | Concept::ZoneOfIndifference
        );
        if needs_band && (band.is_nan() || band <= 0.0) {
            return Err(err(
                "ERROR 000928: A non-zero distance band is required for this conceptualization of spatial relationships.",
            ));
        }
        if matches!(
            concept,
            Concept::InverseDistance | Concept::InverseDistanceSquared
        ) && band <= 0.0
            && n > MAX_FEATURES
        {
            return Err(err(format!(
                "ERROR 000800: relating every pair of {n} features needs a distance band (at most {MAX_FEATURES} without)"
            )));
        }
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            let mut row: Vec<(usize, f64)> = Vec::new();
            match concept {
                Concept::KNearestNeighbors(k) => {
                    let mut d: Vec<(f64, usize)> = (0..n)
                        .filter(|j| *j != i)
                        .map(|j| (method.between(points[i], points[j]), j))
                        .collect();
                    d.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                    row = d.into_iter().take(k).map(|(_, j)| (j, 1.0)).collect();
                    row.sort_by_key(|r| r.0);
                }
                _ => {
                    for j in 0..n {
                        if j == i {
                            continue;
                        }
                        let d = method.between(points[i], points[j]);
                        let w = match concept {
                            Concept::InverseDistance | Concept::InverseDistanceSquared => {
                                if band > 0.0 && d > band {
                                    continue;
                                }
                                // Distances under one count as one.
                                let d = d.max(1.0);
                                if matches!(concept, Concept::InverseDistance) {
                                    1.0 / d
                                } else {
                                    1.0 / (d * d)
                                }
                            }
                            Concept::FixedDistanceBand => {
                                if d > band {
                                    continue;
                                }
                                1.0
                            }
                            Concept::ZoneOfIndifference => {
                                if d <= band {
                                    1.0
                                } else {
                                    1.0 / (1.0 + (d - band))
                                }
                            }
                            Concept::KNearestNeighbors(_) => continue,
                        };
                        row.push((j, w));
                    }
                }
            }
            rows.push(row);
        }
        Ok(Weights { rows })
    }

    fn row_standardize(&mut self) {
        for r in &mut self.rows {
            let s: f64 = r.iter().map(|x| x.1).sum();
            if s > 0.0 {
                for x in r.iter_mut() {
                    x.1 /= s;
                }
            }
        }
    }

    fn get(&self, i: usize, j: usize) -> f64 {
        self.rows[i]
            .binary_search_by_key(&j, |x| x.0)
            .map_or(0.0, |k| self.rows[i][k].1)
    }
}

fn numeric_values(
    input: &dyn FeatureClass,
    field: &str,
    case: Option<&str>,
) -> Result<(Vec<Item>, Vec<f64>)> {
    let idx = field_index(input, field, true)?;
    let all = items(input, &Options { weight: None, case })?;
    let mut kept = Vec::new();
    let mut values = Vec::new();
    for it in all {
        if let Some(v) = it.attrs.get(idx).and_then(number) {
            values.push(v);
            kept.push(it);
        }
    }
    Ok((kept, values))
}

/// Average Nearest Neighbor: NNRatio, z-score, p-value, expected and observed mean distances.
/// `area` is the study area (0: the minimum enclosing rectangle of the features).
pub fn average_nearest_neighbor(
    input: &dyn FeatureClass,
    method: DistanceMethod,
    area: f64,
) -> Result<Report> {
    let all = items(
        input,
        &Options {
            weight: None,
            case: None,
        },
    )?;
    let n = all.len();
    if n < 2 {
        return Err(err("ERROR 001095: at least two features are needed"));
    }
    let pts: Vec<[f64; 2]> = all.iter().map(|i| i.xy).collect();
    let area = if area > 0.0 {
        area
    } else {
        let mut g = Geometry::point(0.0, 0.0);
        g.kind = GeometryKind::Multipoint;
        g.xy = pts.clone();
        let b = geoforge_geometry::bounding(
            [&g],
            geoforge_geometry::BoundingKind::RectangleByArea,
            0.0,
        )?
        .ok_or_else(|| err("no bounding rectangle"))?;
        let a = b.width * b.length;
        if a <= 0.0 {
            return Err(err(
                "ERROR 001095: the features cover no area; give the study area",
            ));
        }
        a
    };
    let observed = (0..n)
        .map(|i| {
            (0..n)
                .filter(|j| *j != i)
                .map(|j| method.between(pts[i], pts[j]))
                .fold(f64::INFINITY, f64::min)
        })
        .sum::<f64>()
        / n as f64;
    let nf = n as f64;
    let expected = 0.5 / (nf / area).sqrt();
    let se = 0.26136 / (nf * nf / area).sqrt();
    let z = (observed - expected) / se;
    Ok(Report {
        rows: vec![
            ("NNRatio".into(), observed / expected),
            ("ZScore".into(), z),
            ("PValue".into(), p_value(z)),
            ("Expected Mean Distance".into(), expected),
            ("Observed Mean Distance".into(), observed),
        ],
        messages: Vec::new(),
    })
}

/// Spatial Autocorrelation (Global Moran's I) of a numeric field: index, expected index, variance,
/// z-score and p-value (variance under randomization).
pub fn spatial_autocorrelation(
    input: &dyn FeatureClass,
    field: &str,
    concept: Concept,
    method: DistanceMethod,
    row_standardized: bool,
    band: f64,
) -> Result<Report> {
    let (kept, x) = numeric_values(input, field, None)?;
    let n = x.len();
    if n < 4 {
        return Err(err(
            "ERROR 001095: at least four features with values are needed",
        ));
    }
    let pts: Vec<[f64; 2]> = kept.iter().map(|i| i.xy).collect();
    let mut w = Weights::build(&pts, concept, method, band)?;
    if row_standardized {
        w.row_standardize();
    }
    let nf = n as f64;
    let mean = x.iter().sum::<f64>() / nf;
    let z: Vec<f64> = x.iter().map(|v| v - mean).collect();
    let m2: f64 = z.iter().map(|v| v * v).sum();
    if m2 == 0.0 {
        return Err(err("ERROR 001096: every value is the same"));
    }
    let (mut s0, mut s1, mut s2, mut cross) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        let mut out_sum = 0.0;
        for &(j, wij) in &w.rows[i] {
            let wji = w.get(j, i);
            s0 += wij;
            // Over all ordered pairs, (wij + wji)^2 / 2: a pair listed one way counts twice.
            s1 += if wji != 0.0 {
                0.5 * (wij + wji).powi(2)
            } else {
                wij * wij
            };
            out_sum += wij;
            cross += wij * z[i] * z[j];
        }
        let in_sum: f64 = (0..n).map(|j| w.get(j, i)).sum();
        s2 += (out_sum + in_sum).powi(2);
    }
    if s0 == 0.0 {
        return Err(err(
            "ERROR 000928: no features have neighbors: widen the distance band",
        ));
    }
    let index = nf / s0 * cross / m2;
    let expected = -1.0 / (nf - 1.0);
    let kurtosis = nf * z.iter().map(|v| v.powi(4)).sum::<f64>() / (m2 * m2);
    let a = nf * ((nf * nf - 3.0 * nf + 3.0) * s1 - nf * s2 + 3.0 * s0 * s0);
    let b = kurtosis * ((nf * nf - nf) * s1 - 2.0 * nf * s2 + 6.0 * s0 * s0);
    let c = (nf - 1.0) * (nf - 2.0) * (nf - 3.0) * s0 * s0;
    let variance = (a - b) / c - expected * expected;
    let zscore = (index - expected) / variance.sqrt();
    Ok(Report {
        rows: vec![
            ("Moran's Index".into(), index),
            ("Expected Index".into(), expected),
            ("Variance".into(), variance),
            ("ZScore".into(), zscore),
            ("PValue".into(), p_value(zscore)),
        ],
        messages: Vec::new(),
    })
}

/// The critical p-value of the Benjamini-Hochberg false discovery rate at `alpha` (0 when none of
/// the p-values passes).
fn fdr_critical(p: &[f64], alpha: f64) -> f64 {
    let mut sorted: Vec<f64> = p.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let n = sorted.len() as f64;
    let mut crit = 0.0;
    for (i, v) in sorted.iter().enumerate() {
        if *v <= alpha * (i + 1) as f64 / n {
            crit = *v;
        }
    }
    crit
}

/// Hot Spot Analysis (Getis-Ord Gi*): the features with `SOURCE_ID`, the field, `GiZScore`,
/// `GiPValue`, `NNeighbors` (the feature counts itself) and `Gi_Bin` (+-3, 2, 1 at 99, 95 and 90 %
/// confidence, or by the false discovery rate). Row standardization changes nothing for Gi*.
pub fn hot_spot_analysis(
    input: &dyn FeatureClass,
    field: &str,
    concept: Concept,
    method: DistanceMethod,
    band: f64,
    fdr: bool,
) -> Result<ToolOutput> {
    let (kept, x) = numeric_values(input, field, None)?;
    let n = x.len();
    if n < 3 {
        return Err(err(
            "ERROR 001095: at least three features with values are needed",
        ));
    }
    let pts: Vec<[f64; 2]> = kept.iter().map(|i| i.xy).collect();
    let w = Weights::build(&pts, concept, method, band)?;
    let nf = n as f64;
    let mean = x.iter().sum::<f64>() / nf;
    let s = (x.iter().map(|v| v * v).sum::<f64>() / nf - mean * mean).sqrt();
    if s == 0.0 {
        return Err(err("ERROR 001096: every value is the same"));
    }
    let mut zs = Vec::with_capacity(n);
    let mut counts = Vec::with_capacity(n);
    for i in 0..n {
        // The feature itself counts at weight 1.
        let (mut sw, mut sw2, mut swx) = (1.0, 1.0, x[i]);
        for &(j, wij) in &w.rows[i] {
            sw += wij;
            sw2 += wij * wij;
            swx += wij * x[j];
        }
        counts.push(w.rows[i].len() + 1);
        let den = s * ((nf * sw2 - sw * sw) / (nf - 1.0)).sqrt();
        zs.push(if den > 0.0 {
            (swx - mean * sw) / den
        } else {
            0.0
        });
    }
    let ps: Vec<f64> = zs.iter().map(|z| p_value(*z)).collect();
    let crit: Option<[f64; 3]> = fdr.then(|| {
        [
            fdr_critical(&ps, 0.01),
            fdr_critical(&ps, 0.05),
            fdr_critical(&ps, 0.10),
        ]
    });
    let bin = |z: f64, p: f64| -> i32 {
        let level = match crit {
            Some(c) => {
                if p <= c[0] && c[0] > 0.0 {
                    3
                } else if p <= c[1] && c[1] > 0.0 {
                    2
                } else if p <= c[2] && c[2] > 0.0 {
                    1
                } else {
                    0
                }
            }
            // By p-value: 99, 95 and 90 % confidence.
            None => {
                if p <= 0.01 {
                    3
                } else if p <= 0.05 {
                    2
                } else if p <= 0.10 {
                    1
                } else {
                    0
                }
            }
        };
        if z < 0.0 { -level } else { level }
    };
    let src = &input.fields()[field_index(input, field, true)?];
    let mut fields = vec![
        Field::new("SOURCE_ID", FieldType::Integer),
        double(&src.name),
        double("GiZScore"),
        double("GiPValue"),
        Field::new("NNeighbors", FieldType::Integer),
        Field::new("Gi_Bin", FieldType::Integer),
    ];
    for f in &mut fields {
        if f.field_type == FieldType::Integer {
            f.precision = 10;
        }
    }
    let idx = field_index(input, field, true)?;
    let features = kept
        .iter()
        .enumerate()
        .map(|(k, it)| Feature {
            oid: k as i64,
            geometry: Some(it.geometry.clone()),
            attributes: vec![
                Value::Integer(it.index as i32),
                Value::Double(it.attrs.get(idx).and_then(number).unwrap_or(0.0)),
                Value::Double(zs[k]),
                Value::Double(ps[k]),
                Value::Integer(counts[k] as i32),
                Value::Integer(bin(zs[k], ps[k])),
            ],
        })
        .collect();
    Ok(output(
        input,
        input.geometry_kind().unwrap_or(GeometryKind::Point),
        fields,
        features,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erfc_matches_known_values() {
        for (x, want) in [
            (0.0, 1.0),
            (0.1, 0.887_537_083_981_715_2),
            (0.5, 0.479_500_122_186_953_5),
            (1.0, 0.157_299_207_050_285_13),
            (2.0, 0.004_677_734_981_047_266),
            (2.5, 4.069_520_174_449_589e-4),
            (4.0, 1.541_725_790_028_002e-8),
            (3.0, 2.209_049_699_858_544e-5),
            (5.0, 1.537_459_794_428_035e-12),
        ] {
            let got = erfc(x);
            assert!((got - want).abs() <= 1e-12 * want, "{x}: {got} vs {want}");
        }
        assert!((erfc(-1.0) - (2.0 - 0.157_299_207_050_285_13)).abs() < 1e-13);
        assert!((p_value(1.959_963_984_540_054) - 0.05).abs() < 1e-12);
    }
}
