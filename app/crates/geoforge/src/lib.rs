//! GeoForge's ArcGIS-verified geoprocessing tools (`geoforge-tools`) as RustGIS tools.
//!
//! Modified for GeoForge (not part of upstream RustGIS).
//!
//! The tools join the registry as the external suite [`SOURCE`] (ids `geoforge:<name>`), through
//! [`rustgis_analysis::register_external`]. A tool's layers are converted to GeoForge feature
//! classes, the engine runs the ArcGIS algorithm (parameters, output schema, row order as
//! ArcGIS's), and its output comes back as a layer.
//!
//! Layers in RustGIS are WGS 84 longitude/latitude, so the engine runs with that coordinate
//! system: overlay tools (topology, not distances) behave as ArcGIS does on geographic data,
//! with ArcGIS's geographic XY tolerance. Distance tools (Buffer, Near) wait for geodesic
//! support; ArcGIS's own buffer on geographic data is geodesic.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use geoforge_core as gf;
use geoforge_tools as tools;
use rustgis_analysis::{Inputs, Output, ParamKind as K, ParamSpec as P, ToolResult, ToolSpec};
use rustgis_data::source::filegdb::from_geoforge;
use rustgis_data::{Dataset, Field, FieldType, Geometry, GeometryType, Value};
use rustgis_geom::Coord;
use serde_json::Value as Json;

/// Tool ids get this prefix, so they never clash with the built-in tools.
pub const PREFIX: &str = "geoforge:";

/// The key the suite registers its tools under.
pub const SOURCE: &str = "geoforge";

/// Esri WKT of the coordinate system layers are held in.
const WGS84_WKT: &str = "GEOGCS[\"GCS_WGS_1984\",DATUM[\"D_WGS_1984\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]]";

/// Adds the suite to the geoprocessing tools.
pub fn register() {
    rustgis_analysis::register_external(SOURCE, specs, run);
}

// ---------------------------------------------------------------------------------------------
// conversions

/// A layer (or its selected rows) as a GeoForge feature class.
struct Fc {
    kind: Option<gf::GeometryKind>,
    fields: Vec<gf::Field>,
    features: Vec<gf::Feature>,
}

impl gf::FeatureClass for Fc {
    fn geometry_kind(&self) -> Option<gf::GeometryKind> {
        self.kind
    }
    fn has_z(&self) -> bool {
        false
    }
    fn has_m(&self) -> bool {
        false
    }
    fn fields(&self) -> &[gf::Field] {
        &self.fields
    }
    fn oid_field_name(&self) -> &str {
        "OBJECTID"
    }
    fn shape_field_name(&self) -> &str {
        "Shape"
    }
    fn spatial_reference_wkt(&self) -> Option<&str> {
        Some(WGS84_WKT)
    }
    fn stored_extent(&self) -> Option<gf::Envelope> {
        None
    }
    fn features(&self) -> Box<dyn Iterator<Item = gf::Result<gf::Feature>> + '_> {
        Box::new(self.features.iter().cloned().map(Ok))
    }
}

fn to_gf_field(f: &Field) -> gf::Field {
    let ty = match f.ty {
        FieldType::Integer => gf::FieldType::BigInteger,
        FieldType::Double => gf::FieldType::Double,
        FieldType::Text => gf::FieldType::String,
        FieldType::Boolean => gf::FieldType::SmallInteger,
    };
    let mut out = gf::Field::new(&f.name, ty);
    out.alias = f.alias.clone();
    if ty == gf::FieldType::String {
        out.length = 255;
    }
    out
}

fn to_gf_value(v: &Value, ty: FieldType) -> gf::Value {
    match (v, ty) {
        (Value::Null, _) => gf::Value::Null,
        (Value::Int(i), _) => gf::Value::BigInteger(*i),
        (Value::Float(f), _) => gf::Value::Double(*f),
        (Value::Text(s), _) => gf::Value::String(s.clone()),
        (Value::Bool(b), _) => gf::Value::SmallInteger(i16::from(*b)),
    }
}

/// A ring with Esri's orientation (outer clockwise, hole counter-clockwise), closed.
fn oriented(ring: &[Coord], outer: bool) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = ring.iter().map(|c| [c.x, c.y]).collect();
    if pts.first() != pts.last()
        && let Some(&first) = pts.first()
    {
        pts.push(first);
    }
    let area: f64 = pts.windows(2).map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1]).sum();
    // Positive area is counter-clockwise.
    if (area > 0.0) == outer {
        pts.reverse();
    }
    pts
}

fn to_gf_geometry(g: &Geometry) -> Option<gf::Geometry> {
    let mut out = gf::Geometry::point(0.0, 0.0);
    out.xy.clear();
    match g {
        Geometry::Point(c) => return Some(gf::Geometry::point(c.x, c.y)),
        Geometry::MultiPoint(v) => {
            out.kind = gf::GeometryKind::Multipoint;
            out.xy = v.iter().map(|c| [c.x, c.y]).collect();
        }
        Geometry::Line(parts) => {
            out.kind = gf::GeometryKind::Polyline;
            for p in parts.iter().filter(|p| p.len() >= 2) {
                out.parts.push(out.xy.len() as u32);
                out.xy.extend(p.iter().map(|c| [c.x, c.y]));
            }
        }
        Geometry::Polygon(polys) => {
            out.kind = gf::GeometryKind::Polygon;
            for poly in polys {
                for (i, ring) in poly.iter().enumerate().filter(|(_, r)| r.len() >= 3) {
                    out.parts.push(out.xy.len() as u32);
                    out.xy.extend(oriented(ring, i == 0));
                }
            }
        }
    }
    (!out.xy.is_empty()).then_some(out)
}

fn kind_of(t: GeometryType) -> Option<gf::GeometryKind> {
    match t {
        GeometryType::Point => Some(gf::GeometryKind::Point),
        GeometryType::Polyline => Some(gf::GeometryKind::Polyline),
        GeometryType::Polygon => Some(gf::GeometryKind::Polygon),
        GeometryType::None => None,
    }
}

/// Whether a layer holds points as multipoints (a layer's type says only "Point").
fn has_multipoints(ds: &Dataset) -> bool {
    ds.features.iter().any(|f| matches!(f.geometry, Some(Geometry::MultiPoint(_))))
}

fn to_fc(ds: &Dataset, rows: &[usize]) -> Fc {
    let mut kind = kind_of(ds.geometry_type);
    if kind == Some(gf::GeometryKind::Point) && has_multipoints(ds) {
        kind = Some(gf::GeometryKind::Multipoint);
    }
    let features = rows
        .iter()
        .filter_map(|&r| ds.features.get(r).map(|f| (r, f)))
        .map(|(_, f)| gf::Feature {
            oid: f.id as i64,
            geometry: f.geometry.as_ref().and_then(to_gf_geometry),
            attributes: f.attrs.iter().zip(&ds.fields).map(|(v, fl)| to_gf_value(v, fl.ty)).collect(),
        })
        .collect();
    Fc { kind, fields: ds.fields.iter().map(to_gf_field).collect(), features }
}

fn from_gf_value(v: gf::Value) -> Value {
    match v {
        gf::Value::Null => Value::Null,
        gf::Value::SmallInteger(i) => Value::Int(i64::from(i)),
        gf::Value::Integer(i) => Value::Int(i64::from(i)),
        gf::Value::BigInteger(i) => Value::Int(i),
        gf::Value::Single(f) => Value::Float(f64::from(f)),
        gf::Value::Double(f) => Value::Float(f),
        gf::Value::String(s) | gf::Value::Guid(s) | gf::Value::Xml(s) => Value::Text(s),
        gf::Value::Date(d) => Value::Text(d.to_string()),
        gf::Value::DateOnly(d) => Value::Text(d.to_string()),
        gf::Value::TimeOnly(t) => Value::Text(t.to_string()),
        gf::Value::TimestampOffset(t) => Value::Text(t.to_rfc3339()),
        gf::Value::Blob(b) => Value::Text(format!("<{} bytes>", b.len())),
    }
}

fn from_gf_field_type(t: gf::FieldType) -> FieldType {
    use gf::FieldType as G;
    match t {
        G::SmallInteger | G::Integer | G::BigInteger => FieldType::Integer,
        G::Single | G::Double => FieldType::Double,
        _ => FieldType::Text,
    }
}

/// A tool's output as a layer named `name`.
fn to_dataset(out: tools::ToolOutput, name: String) -> Dataset {
    let geometry_type = match out.kind {
        gf::GeometryKind::Point | gf::GeometryKind::Multipoint => GeometryType::Point,
        gf::GeometryKind::Polyline => GeometryType::Polyline,
        gf::GeometryKind::Polygon | gf::GeometryKind::MultiPatch => GeometryType::Polygon,
    };
    let fields = out
        .fields
        .iter()
        .filter(|f| !matches!(f.field_type, gf::FieldType::ObjectId | gf::FieldType::Geometry))
        .map(|f| {
            let mut fl = Field::new(&f.name, from_gf_field_type(f.field_type));
            fl.alias = f.alias.clone();
            fl
        })
        .collect();
    let mut ds = Dataset::new(name, geometry_type, fields);
    for f in out.features {
        let g = f.geometry.as_ref().and_then(from_geoforge);
        ds.push_next(g, f.attributes.into_iter().map(from_gf_value).collect());
    }
    ds
}

// ---------------------------------------------------------------------------------------------
// parameters

fn text<'a>(p: &'a Json, k: &str) -> &'a str {
    p.get(k).and_then(Json::as_str).unwrap_or("").trim()
}

fn flag(p: &Json, k: &str) -> bool {
    p.get(k).and_then(Json::as_bool).unwrap_or(false)
}

fn layer<'a>(inp: &'a dyn Inputs, p: &Json, k: &str) -> Result<(&'a Dataset, Fc), String> {
    let name = text(p, k);
    let ds = inp.dataset(name).ok_or_else(|| format!("no layer named '{name}'"))?;
    // As in ArcGIS, a layer with a selection is read as its selected features.
    let sel = inp.selection(name);
    let rows: Vec<usize> = if sel.is_empty() { (0..ds.features.len()).collect() } else { sel.to_vec() };
    Ok((ds, to_fc(ds, &rows)))
}

fn out_name(p: &Json, default: String) -> String {
    let n = text(p, "output");
    if n.is_empty() { default } else { n.to_string() }
}

fn finish(out: tools::ToolOutput, name: String) -> Result<ToolResult, String> {
    let messages = out.messages.clone();
    let ds = to_dataset(out, name);
    let mut m = vec![format!("{}: {} features", ds.name, ds.features.len())];
    m.extend(messages);
    Ok(ToolResult { output: Output::Layer(ds), messages: m })
}

fn err(e: gf::Error) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------------------------
// tools

fn clip(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, a) = layer(inp, p, "input")?;
    let (_, b) = layer(inp, p, "clip")?;
    finish(tools::clip(&a, &b).map_err(err)?, out_name(p, format!("{}_Clip", ds.name)))
}

fn erase(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, a) = layer(inp, p, "input")?;
    let (_, b) = layer(inp, p, "erase")?;
    finish(tools::erase(&a, &b).map_err(err)?, out_name(p, format!("{}_Erase", ds.name)))
}

fn two(inp: &dyn Inputs, p: &Json) -> Result<[(String, Fc); 2], String> {
    let (a, fa) = layer(inp, p, "input")?;
    let (b, fb) = layer(inp, p, "other")?;
    Ok([(a.name.clone(), fa), (b.name.clone(), fb)])
}

fn overlay(
    inp: &dyn Inputs,
    p: &Json,
    suffix: &str,
    run: impl FnOnce(&[tools::NamedInput]) -> gf::Result<tools::ToolOutput>,
) -> Result<ToolResult, String> {
    let pair = two(inp, p)?;
    let default = format!("{}_{suffix}", pair[0].0);
    let named: Vec<tools::NamedInput> = pair.iter().map(|(n, f)| tools::NamedInput { name: n.clone(), features: f }).collect();
    finish(run(&named).map_err(err)?, out_name(p, default))
}

fn intersect(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    overlay(inp, p, "Intersect", tools::intersect)
}

fn union(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    overlay(inp, p, "Union", tools::union)
}

fn symmetrical_difference(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    overlay(inp, p, "SymDiff", tools::symmetrical_difference)
}

fn dissolve(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let fields: Vec<String> = text(p, "fields").split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    let multi = !flag(p, "single_part");
    finish(tools::dissolve(&fc, &fields, &[], multi).map_err(err)?, out_name(p, format!("{}_Dissolve", ds.name)))
}

fn multipart_to_singlepart(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    finish(tools::multipart_to_singlepart(&fc).map_err(err)?, out_name(p, format!("{}_Single", ds.name)))
}

fn feature_to_point(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let inside = text(p, "point_location").eq_ignore_ascii_case("INSIDE");
    finish(tools::feature_to_point(&fc, inside).map_err(err)?, out_name(p, format!("{}_Point", ds.name)))
}

const INPUT: P = P { name: "input", label: "Input Features", kind: K::Layer, required: true, default: "", help: "" };
const OTHER: P = P { name: "other", label: "Overlay Features", kind: K::Layer, required: true, default: "", help: "" };
const OUTPUT: P = P {
    name: "output",
    label: "Output Name",
    kind: K::Text,
    required: false,
    default: "",
    help: "Name of the new layer (default: the input's name and the tool).",
};

struct Def {
    id: &'static str,
    name: &'static str,
    toolbox: &'static str,
    summary: &'static str,
    params: &'static [P],
    run: fn(&dyn Inputs, &Json) -> Result<ToolResult, String>,
}

const DEFS: &[Def] = &[
    Def {
        id: "clip",
        name: "Clip",
        toolbox: "GeoForge › Analysis › Extract",
        summary: "Cuts the input features out with polygons (ArcGIS Pro Clip).",
        params: &[INPUT, P { name: "clip", label: "Clip Features", kind: K::PolygonLayer, required: true, default: "", help: "" }, OUTPUT],
        run: clip,
    },
    Def {
        id: "erase",
        name: "Erase",
        toolbox: "GeoForge › Analysis › Overlay",
        summary: "Keeps the parts of the input outside the erase polygons (ArcGIS Pro Erase).",
        params: &[
            INPUT,
            P { name: "erase", label: "Erase Features", kind: K::PolygonLayer, required: true, default: "", help: "" },
            OUTPUT,
        ],
        run: erase,
    },
    Def {
        id: "intersect",
        name: "Intersect",
        toolbox: "GeoForge › Analysis › Overlay",
        summary: "The parts where both layers overlap, with both layers' attributes (ArcGIS Pro Intersect).",
        params: &[INPUT, OTHER, OUTPUT],
        run: intersect,
    },
    Def {
        id: "union",
        name: "Union",
        toolbox: "GeoForge › Analysis › Overlay",
        summary: "All of both polygon layers, split where they overlap (ArcGIS Pro Union).",
        params: &[INPUT, OTHER, OUTPUT],
        run: union,
    },
    Def {
        id: "symmetrical_difference",
        name: "Symmetrical Difference",
        toolbox: "GeoForge › Analysis › Overlay",
        summary: "The parts of two polygon layers that do not overlap (ArcGIS Pro Symmetrical Difference).",
        params: &[INPUT, OTHER, OUTPUT],
        run: symmetrical_difference,
    },
    Def {
        id: "dissolve",
        name: "Dissolve",
        toolbox: "GeoForge › Data Management › Generalization",
        summary: "Merges features that share values in the dissolve fields (ArcGIS Pro Dissolve).",
        params: &[
            INPUT,
            P {
                name: "fields",
                label: "Dissolve Fields",
                kind: K::Text,
                required: false,
                default: "",
                help: "Field names separated by semicolons; none dissolves everything into one feature.",
            },
            P { name: "single_part", label: "Create single-part features", kind: K::Bool, required: false, default: "false", help: "" },
            OUTPUT,
        ],
        run: dissolve,
    },
    Def {
        id: "multipart_to_singlepart",
        name: "Multipart To Singlepart",
        toolbox: "GeoForge › Data Management › Features",
        summary: "One feature per part (ArcGIS Pro Multipart To Singlepart).",
        params: &[INPUT, OUTPUT],
        run: multipart_to_singlepart,
    },
    Def {
        id: "feature_to_point",
        name: "Feature To Point",
        toolbox: "GeoForge › Data Management › Features",
        summary: "A point for each feature, at its centroid or inside it (ArcGIS Pro Feature To Point).",
        params: &[
            INPUT,
            P {
                name: "point_location",
                label: "Point Location",
                kind: K::Choice(&["CENTROID", "INSIDE"]),
                required: false,
                default: "\"CENTROID\"",
                help: "",
            },
            OUTPUT,
        ],
        run: feature_to_point,
    },
];

fn specs() -> Vec<ToolSpec> {
    DEFS.iter()
        .map(|d| ToolSpec {
            // Ids are `geoforge:<id>`; leaked once, like the other suites' specs.
            id: Box::leak(format!("{PREFIX}{}", d.id).into_boxed_str()),
            name: d.name,
            toolbox: d.toolbox,
            summary: d.summary,
            params: d.params,
            run: rustgis_analysis::external_placeholder,
        })
        .collect()
}

fn run(spec: &ToolSpec, inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let id = spec.id.strip_prefix(PREFIX).unwrap_or(spec.id);
    let def = DEFS.iter().find(|d| d.id == id).ok_or_else(|| format!("no tool '{}'", spec.id))?;
    (def.run)(inp, p)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Layers(Vec<Dataset>);

    impl Inputs for Layers {
        fn dataset(&self, layer: &str) -> Option<&Dataset> {
            self.0.iter().find(|d| d.name == layer)
        }
        fn selection(&self, _: &str) -> &[usize] {
            &[]
        }
    }

    fn square(name: &str, x: f64, y: f64, s: f64) -> Dataset {
        let mut d = Dataset::new(name, GeometryType::Polygon, vec![Field::new("NAME", FieldType::Text)]);
        let ring = vec![Coord::new(x, y), Coord::new(x, y + s), Coord::new(x + s, y + s), Coord::new(x + s, y), Coord::new(x, y)];
        d.push(Some(Geometry::Polygon(vec![vec![ring]])), vec![Value::Text(name.into())]);
        d
    }

    fn area(d: &Dataset) -> f64 {
        let mut a = 0.0;
        for f in &d.features {
            if let Some(Geometry::Polygon(ps)) = &f.geometry {
                for p in ps {
                    for (i, r) in p.iter().enumerate() {
                        let s: f64 = r.windows(2).map(|w| w[0].x * w[1].y - w[1].x * w[0].y).sum::<f64>().abs() / 2.0;
                        a += if i == 0 { s } else { -s };
                    }
                }
            }
        }
        a
    }

    fn dataset(r: Result<ToolResult, String>) -> Dataset {
        match r.unwrap().output {
            Output::Layer(d) => d,
            _ => panic!("expected a layer"),
        }
    }

    #[test]
    fn overlay_tools_run_through_the_registry() {
        register();
        let layers = Layers(vec![square("a", 0.0, 0.0, 2.0), square("b", 1.0, 1.0, 2.0)]);
        let spec = rustgis_analysis::tool("geoforge:clip").expect("registered");
        assert_eq!(spec.name, "Clip");
        let clip = dataset(rustgis_analysis::run("geoforge:clip", &layers, &json!({"input": "a", "clip": "b"})));
        assert!((area(&clip) - 1.0).abs() < 1e-9, "{}", area(&clip));
        let erase = dataset(rustgis_analysis::run("geoforge:erase", &layers, &json!({"input": "a", "erase": "b"})));
        assert!((area(&erase) - 3.0).abs() < 1e-9);
        let inter = dataset(rustgis_analysis::run("geoforge:intersect", &layers, &json!({"input": "a", "other": "b"})));
        assert!((area(&inter) - 1.0).abs() < 1e-9);
        // ArcGIS's schema: FID_<name> per input, then the fields.
        let names: Vec<&str> = inter.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["FID_a", "NAME", "FID_b", "NAME_1"]);
        let union = dataset(rustgis_analysis::run("geoforge:union", &layers, &json!({"input": "a", "other": "b"})));
        assert!((area(&union) - 7.0).abs() < 1e-9);
        assert_eq!(union.features.len(), 3);
    }

    #[test]
    fn bad_parameters_are_errors_not_panics() {
        register();
        let layers = Layers(vec![square("a", 0.0, 0.0, 1.0)]);
        for id in ["clip", "erase", "intersect", "union", "dissolve", "feature_to_point"] {
            let _ = rustgis_analysis::run(&format!("geoforge:{id}"), &layers, &json!({"input": "nope", "clip": 3, "other": null}));
            let _ = rustgis_analysis::run(
                &format!("geoforge:{id}"),
                &layers,
                &json!({"input": "a", "clip": "a", "erase": "a", "other": "a", "fields": "NOPE"}),
            );
        }
    }
}
