//! Create Random Points: uniform planar sampling subject to feature and spacing constraints.
use crate::{
    ToolOutput,
    registry::{Context, Kind, Param, Runner, Tool},
};
use std::collections::HashMap;
use unigis_core::{
    Envelope, Error, Feature, Field, FieldType, Geometry, GeometryKind, Result, Value,
};

pub const TOOL: Tool = Tool {
    id: "create_random_points",
    name: "Create Random Points",
    toolbox: "Data Management › Sampling",
    summary: "Creates random points in an extent, inside polygons, along lines or as a subset of input points.",
    params: &[
        Param {
            name: "constraining_feature_class",
            label: "Constraining Feature Class",
            kind: Kind::Features,
            required: false,
            default: "",
            help: "Polygon, line or point constraints; takes precedence over extent.",
        },
        Param {
            name: "constraining_extent",
            label: "Constraining Extent",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "xmin ymin xmax ymax, or the path to a dataset whose extent is used.",
        },
        Param {
            name: "number_of_points_or_field",
            label: "Number of Points",
            kind: Kind::Text,
            required: false,
            default: "100",
            help: "Count per polygon/line, total for extent/point subset, or a numeric field.",
        },
        Param {
            name: "minimum_allowed_distance",
            label: "Minimum Allowed Distance",
            kind: Kind::Text,
            required: false,
            default: "0",
            help: "Coordinate units, a linear unit, or a numeric field of polygon/line constraints.",
        },
        Param {
            name: "create_multipoint_output",
            label: "Create Multipoint Output",
            kind: Kind::Choice(&["POINT", "MULTIPOINT"]),
            required: false,
            default: "POINT",
            help: "Group points into multipoint geometries.",
        },
        Param {
            name: "multipoint_size",
            label: "Maximum Number of Points per Multipoint",
            kind: Kind::Number,
            required: false,
            default: "10",
            help: "Positive integral chunk size.",
        },
        Param {
            name: "seed",
            label: "Random Seed",
            kind: Kind::Text,
            required: false,
            default: "0",
            help: "Native reproducible unsigned seed. Does not reproduce ArcGIS random streams.",
        },
    ],
    output_suffix: "random_points",
    measures: false,
    run: Runner::Features(run),
};
fn bad(message: impl Into<String>) -> Error {
    Error::Unsupported(message.into())
}
fn value(ctx: &dyn Context, name: &str, default: &str) -> String {
    ctx.get(name).unwrap_or_else(|| default.into())
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
        x ^ (x >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}
fn numeric(v: &Value) -> f64 {
    match v {
        Value::SmallInteger(v) => f64::from(*v),
        Value::Integer(v) => f64::from(*v),
        Value::BigInteger(v) => *v as f64,
        Value::Single(v) => f64::from(*v),
        Value::Double(v) => *v,
        _ => 0.0,
    }
}
fn count(n: f64) -> Result<usize> {
    if !n.is_finite() || n > 1_000_000.0 {
        return Err(bad("point count must be finite and at most 1000000"));
    }
    Ok(n.round().max(0.0) as usize)
}
struct Spacing {
    distance: f64,
    cells: HashMap<(i64, i64), Vec<[f64; 2]>>,
}
impl Spacing {
    fn new(distance: f64) -> Self {
        Self {
            distance,
            cells: HashMap::new(),
        }
    }
    fn key(&self, p: [f64; 2]) -> (i64, i64) {
        (
            (p[0] / self.distance).floor() as i64,
            (p[1] / self.distance).floor() as i64,
        )
    }
    fn accept(&mut self, p: [f64; 2]) -> bool {
        if self.distance <= 0.0 {
            return true;
        }
        let (x, y) = self.key(p);
        for a in -1i64..=1 {
            for b in -1i64..=1 {
                if self
                    .cells
                    .get(&(x.saturating_add(a), y.saturating_add(b)))
                    .is_some_and(|ps| {
                        ps.iter()
                            .any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < self.distance)
                    })
                {
                    return false;
                }
            }
        }
        self.cells.entry((x, y)).or_default().push(p);
        true
    }
}
fn envelope(text: &str) -> Option<Envelope> {
    let n: Vec<_> = text
        .split_whitespace()
        .map(str::parse::<f64>)
        .collect::<std::result::Result<_, _>>()
        .ok()?;
    if n.len() != 4 {
        return None;
    }
    Some(Envelope {
        xmin: n[0],
        ymin: n[1],
        xmax: n[2],
        ymax: n[3],
        z: None,
        m: None,
    })
}
fn valid_extent(e: Envelope) -> Result<Envelope> {
    if [e.xmin, e.ymin, e.xmax, e.ymax]
        .iter()
        .any(|v| !v.is_finite())
        || e.xmin >= e.xmax
        || e.ymin >= e.ymax
    {
        return Err(bad(
            "constraining extent must have finite positive width and height",
        ));
    }
    Ok(e)
}
fn sample(g: &Geometry, rng: &mut Rng) -> Result<Option<[f64; 2]>> {
    match g.kind {
        GeometryKind::Polygon => {
            let e = g.envelope();
            let p = [
                e.xmin + rng.unit() * (e.xmax - e.xmin),
                e.ymin + rng.unit() * (e.ymax - e.ymin),
            ];
            Ok(unigis_geometry::within(&Geometry::point(p[0], p[1]), g)?.then_some(p))
        }
        GeometryKind::Polyline => {
            let mut segments = Vec::new();
            let mut total = 0.0;
            for part in g.part_ranges() {
                for pair in g.xy[part].windows(2) {
                    let length = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
                    if length > 0.0 {
                        total += length;
                        segments.push((pair[0], pair[1], length));
                    }
                }
            }
            if total == 0.0 {
                return Ok(None);
            }
            let mut distance = rng.unit() * total;
            for (a, b, length) in segments {
                if distance <= length {
                    let t = distance / length;
                    return Ok(Some([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]));
                }
                distance -= length;
            }
            Ok(None)
        }
        _ => Err(bad("unsupported random point constraint geometry")),
    }
}
fn append(
    out: &mut ToolOutput,
    points: Vec<[f64; 2]>,
    cid: i32,
    multi: bool,
    size: usize,
) -> Result<()> {
    if out.features.len().saturating_add(points.len()) > 1_000_000 {
        return Err(bad("too many output points"));
    }
    for chunk in points.chunks(if multi { size } else { 1 }) {
        let geometry = if multi {
            Geometry {
                kind: GeometryKind::Multipoint,
                xy: chunk.to_vec(),
                parts: vec![],
                part_types: None,
                z: None,
                m: None,
                curves: vec![],
            }
        } else {
            Geometry::point(chunk[0][0], chunk[0][1])
        };
        out.features.push(Feature {
            oid: out.features.len() as i64,
            geometry: Some(geometry),
            attributes: vec![Value::Integer(if multi { 0 } else { cid })],
        });
    }
    Ok(())
}
fn run(ctx: &dyn Context) -> Result<ToolOutput> {
    let fc = if ctx.get("constraining_feature_class").is_some() {
        Some(ctx.features("constraining_feature_class")?)
    } else {
        None
    };
    let mut wkt = fc
        .as_ref()
        .and_then(|f| f.spatial_reference_wkt())
        .map(str::to_owned);
    let extent = if fc.is_none() {
        let text = value(ctx, "constraining_extent", "0 0 250 250");
        let text = if text.eq_ignore_ascii_case("DEFAULT") {
            "0 0 250 250".to_string()
        } else {
            text
        };
        Some(if let Some(e) = envelope(&text) {
            valid_extent(e)?
        } else {
            let src = ctx.features("constraining_extent")?;
            wkt = src.spatial_reference_wkt().map(str::to_owned);
            let mut e = Envelope::EMPTY;
            for f in src.features() {
                if let Some(g) = f?.geometry {
                    e.union(&g.envelope());
                }
            }
            valid_extent(e)?
        })
    } else {
        None
    };
    let n_text = value(ctx, "number_of_points_or_field", "100");
    let gap_text = value(ctx, "minimum_allowed_distance", "0");
    let fields = fc.as_ref().map_or(&[][..], |f| f.fields());
    let n_field = fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(&n_text));
    let gap_field = fields
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(&gap_text));
    let constant_n = if n_field.is_none() {
        let n = n_text.parse::<f64>().map_err(|_| {
            bad(format!(
                "ERROR 000728: Field {n_text} does not exist within table"
            ))
        })?;
        if n < 0.0 {
            return Err(bad("ERROR 000963: The Value can not be negative."));
        }
        count(n)?
    } else {
        0
    };
    let gap = if gap_field.is_none() {
        crate::registry::distance_in_units(&gap_text, wkt.as_deref())?
    } else {
        0.0
    };
    if !gap.is_finite() || gap < 0.0 {
        return Err(bad(
            "ERROR 000673: The inhibition distance cannot be negative",
        ));
    }
    for i in [n_field, gap_field].into_iter().flatten() {
        if !matches!(
            fields[i].field_type,
            FieldType::SmallInteger
                | FieldType::Integer
                | FieldType::BigInteger
                | FieldType::Single
                | FieldType::Double
        ) {
            return Err(bad("count and distance fields must be numeric"));
        }
    }
    let multi = value(ctx, "create_multipoint_output", "POINT").eq_ignore_ascii_case("MULTIPOINT");
    let size = if multi {
        value(ctx, "multipoint_size", "10")
            .parse::<usize>()
            .ok()
            .filter(|&n| n > 0)
            .ok_or_else(|| bad("multipoint size must be a positive integer"))?
    } else {
        1
    };
    let mut rng = Rng(value(ctx, "seed", "0")
        .parse()
        .map_err(|_| bad("seed must be an unsigned integer"))?);
    let mut field = Field::new(if fc.is_some() { "CID" } else { "Id" }, FieldType::Integer);
    field.precision = if fc.is_some() { 10 } else { 6 };
    let mut out = ToolOutput {
        kind: if multi {
            GeometryKind::Multipoint
        } else {
            GeometryKind::Point
        },
        has_z: false,
        has_m: false,
        fields: vec![field],
        spatial_reference_wkt: wkt,
        features: vec![],
        messages: vec![],
    };
    let mut groups = Vec::new();
    if let Some(fc) = &fc {
        if !matches!(
            fc.geometry_kind(),
            Some(
                GeometryKind::Point
                    | GeometryKind::Multipoint
                    | GeometryKind::Polyline
                    | GeometryKind::Polygon
            )
        ) {
            return Err(bad(
                "random point constraints must be points, lines or polygons",
            ));
        }
        let rows = fc.features().collect::<Result<Vec<_>>>()?;
        if matches!(
            fc.geometry_kind(),
            Some(GeometryKind::Point | GeometryKind::Multipoint)
        ) {
            if n_field.is_some() || gap_field.is_some() {
                return Err(bad(
                    "field counts/distances require polygon or line constraints",
                ));
            }
            if multi {
                return Err(bad(
                    "multipoint output from point constraints is not supported",
                ));
            }
            let mut candidates = Vec::new();
            for f in rows {
                if let Some(g) = f.geometry {
                    for p in g.xy {
                        candidates.push((f.oid, p));
                    }
                }
            }
            for i in (1..candidates.len()).rev() {
                let j = (rng.unit() * (i + 1) as f64) as usize;
                candidates.swap(i, j);
            }
            let mut spacing = Spacing::new(gap);
            let mut chosen = Vec::new();
            for c in candidates {
                if chosen.len() >= constant_n {
                    break;
                }
                if spacing.accept(c.1) {
                    chosen.push(c);
                }
            }
            chosen.sort_by_key(|c| c.0);
            for (cid, p) in chosen {
                append(
                    &mut out,
                    vec![p],
                    i32::try_from(cid).map_err(|_| bad("CID exceeds 32-bit integer"))?,
                    false,
                    size,
                )?;
            }
            if out.features.is_empty() {
                out.messages
                    .push("WARNING 000117: Warning empty output generated.".into());
            }
            return Ok(out);
        }
        for f in rows {
            let n = if let Some(i) = n_field {
                count(numeric(&f.attributes[i]))?
            } else {
                constant_n
            };
            let distance = gap_field.map_or(gap, |i| numeric(&f.attributes[i]).max(0.0));
            if !distance.is_finite() {
                return Err(bad("minimum distance must be finite"));
            }
            if let Some(g) = f.geometry {
                if g.has_curves() {
                    return Err(bad("curved random point constraints are not supported"));
                }
                if g.xy.iter().flatten().any(|x| !x.is_finite()) {
                    return Err(bad("constraint coordinates must be finite"));
                }
                groups.push((
                    g,
                    n,
                    distance,
                    i32::try_from(f.oid).map_err(|_| bad("CID exceeds 32-bit integer"))?,
                ));
            }
        }
    } else if let Some(e) = extent {
        let mut g = unigis_geometry::paths_geometry(vec![vec![
            [e.xmin, e.ymin],
            [e.xmin, e.ymax],
            [e.xmax, e.ymax],
            [e.xmax, e.ymin],
            [e.xmin, e.ymin],
        ]])
        .ok_or_else(|| bad("invalid extent"))?;
        g.kind = GeometryKind::Polygon;
        groups.push((g, constant_n, gap, 0));
    }
    if groups
        .iter()
        .fold(0usize, |sum, (_, n, _, _)| sum.saturating_add(*n))
        > 1_000_000
    {
        return Err(bad("too many requested points"));
    }
    for (g, n, distance, cid) in groups {
        let mut points = Vec::new();
        let mut spacing = Spacing::new(distance);
        let mut misses = 0;
        while points.len() < n && misses < 10_000 {
            if let Some(p) = sample(&g, &mut rng)?
                && spacing.accept(p)
            {
                points.push(p);
                misses = 0;
            } else {
                misses += 1;
            }
        }
        if points.len() < n && !out.messages.iter().any(|m| m.contains("000983")) {
            out.messages.push("WARNING 000983: The specified number of points could not be created in all cases due to restrictions from the minimum allowed distance.".into());
        }
        append(&mut out, points, cid, multi, size)?;
    }
    if out.features.is_empty() {
        out.messages
            .push("WARNING 000117: Warning empty output generated.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Host(Vec<(&'static str, String)>);
    impl Context for Host {
        fn get(&self, name: &str) -> Option<String> {
            self.0.iter().find(|p| p.0 == name).map(|p| p.1.clone())
        }
        fn features(&self, _: &str) -> Result<Box<dyn unigis_core::FeatureClass + '_>> {
            unreachable!()
        }
    }
    fn host(n: usize, seed: u64, gap: f64) -> Host {
        Host(vec![
            ("constraining_extent", "0 0 10 10".into()),
            ("number_of_points_or_field", n.to_string()),
            ("seed", seed.to_string()),
            ("minimum_allowed_distance", gap.to_string()),
        ])
    }
    #[test]
    fn uniform_extent_reproducibility_and_independent_seeds() {
        let out = run(&host(2000, 0, 0.0)).unwrap();
        assert_eq!(out.features, run(&host(2000, 0, 0.0)).unwrap().features);
        assert_ne!(out.features, run(&host(2000, 1, 0.0)).unwrap().features);
        let mut bins = [0; 4];
        for f in out.features {
            let p = f.geometry.unwrap().xy[0];
            assert!((0.0..10.0).contains(&p[0]) && (0.0..10.0).contains(&p[1]));
            bins[usize::from(p[0] >= 5.0) + 2 * usize::from(p[1] >= 5.0)] += 1;
        }
        assert!(bins.iter().all(|&n| (420..580).contains(&n)), "{bins:?}");
    }
    #[test]
    fn spacing_and_saturation_contract() {
        let out = run(&host(80, 5, 0.5)).unwrap();
        assert_eq!(out.features.len(), 80);
        for (i, a) in out.features.iter().enumerate() {
            let a = a.geometry.as_ref().unwrap().xy[0];
            for b in &out.features[..i] {
                let b = b.geometry.as_ref().unwrap().xy[0];
                assert!((a[0] - b[0]).hypot(a[1] - b[1]) >= 0.5);
            }
        }
        let sparse = run(&host(3, 5, 100.0)).unwrap();
        assert_eq!(sparse.features.len(), 1);
        assert!(sparse.messages.iter().any(|m| m.contains("000983")));
    }
    #[test]
    fn polygon_hole_and_line_length_weighting() {
        let mut g = unigis_geometry::paths_geometry(vec![
            vec![
                [0.0, 0.0],
                [0.0, 10.0],
                [10.0, 10.0],
                [10.0, 0.0],
                [0.0, 0.0],
            ],
            vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0], [2.0, 2.0]],
        ])
        .unwrap();
        g.kind = GeometryKind::Polygon;
        let mut rng = Rng(20);
        let mut got = 0;
        for _ in 0..2000 {
            if let Some(p) = sample(&g, &mut rng).unwrap() {
                assert!(!(p[0] > 2.0 && p[0] < 8.0 && p[1] > 2.0 && p[1] < 8.0));
                got += 1;
            }
        }
        assert!((1160..1400).contains(&got), "{got}");
        let line = unigis_geometry::paths_geometry(vec![
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[10.0, 0.0], [19.0, 0.0]],
        ])
        .unwrap();
        let mut short = 0;
        for _ in 0..2000 {
            let p = sample(&line, &mut rng).unwrap().unwrap();
            if p[0] < 2.0 {
                short += 1;
            } else {
                assert!(p[0] >= 10.0 && p[0] <= 19.0);
            }
        }
        assert!((140..260).contains(&short), "{short}");
    }
    #[test]
    fn invalid_bounds_and_resource_limit_refused() {
        let mut h = host(4, 0, 0.0);
        h.0[0].1 = "0 0 0 10".into();
        assert!(run(&h).is_err());
        assert!(run(&host(1_000_001, 0, 0.0)).is_err());
        assert!(run(&host(1, 0, -1.0)).is_err());
    }
}
