//! GeoForge's ArcGIS-verified geoprocessing tools (`geoforge-tools`) as RustGIS tools.
//!
//! Modified for GeoForge (not part of upstream RustGIS).
//!
//! The tools join the registry as the external suite [`SOURCE`] (ids `geoforge:<name>`), through
//! [`rustgis_analysis::register_external`]. A tool's layers are converted to GeoForge feature
//! classes, the engine runs the ArcGIS algorithm (parameters, output schema, row order as
//! ArcGIS's), and its output comes back as a layer.
//!
//! Layers in RustGIS are WGS 84 longitude/latitude. Tools that only relate shapes (Clip, Erase,
//! Intersect, ...) run on them as they are, with ArcGIS's geographic XY tolerance. Tools that
//! measure (Buffer, Near, search distances) run in the UTM zone of the data, as ArcGIS does on
//! projected data, in metres, and their results are projected back: see [`Work`].
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use geoforge_core as gf;
use geoforge_tools as tools;
use rustgis_analysis::{Inputs, Output, ParamKind as K, ParamSpec as P, ToolResult, ToolSpec};
use rustgis_data::raster::{Crs, Transformer};
use rustgis_data::source::filegdb::{from_geoforge, to_geoforge as to_gf_geometry};
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
// working coordinate system

/// The coordinate system a tool runs in. Layers are longitude/latitude; a tool that measures
/// distances runs in the UTM zone of its inputs (metres, planar, like ArcGIS on projected data)
/// and its output is projected back.
struct Work {
    wkt: String,
    to: Option<(Transformer, Transformer)>,
    /// Said once to the user when the data is too wide for one zone to be accurate.
    note: Option<String>,
}

impl Work {
    /// Longitude/latitude as they are.
    fn degrees() -> Work {
        Work { wkt: WGS84_WKT.into(), to: None, note: None }
    }

    /// The UTM zone at the centre of `layers`' extent.
    fn metres(layers: &[&Dataset]) -> Result<Work, String> {
        let mut e = rustgis_geom::Extent::EMPTY;
        for g in layers.iter().flat_map(|d| d.features.iter()).filter_map(|f| f.geometry.as_ref()) {
            g.for_each_coord(|c| e.expand(c));
        }
        if e.is_empty() {
            return Ok(Work::degrees());
        }
        let wide = e.width() > 12.0 || (e.min_y.abs() > 84.0 || e.max_y.abs() > 84.0);
        Work::zone((e.min_x + e.max_x) / 2.0, (e.min_y + e.max_y) / 2.0, wide)
    }

    /// The UTM zone around a longitude/latitude. `wide`: the data reaches well beyond it.
    fn zone(lon: f64, lat: f64, wide: bool) -> Result<Work, String> {
        let lat = lat.clamp(-80.0, 84.0);
        let zone = (((lon + 180.0) / 6.0).floor() as i32).rem_euclid(60) + 1;
        let north = lat >= 0.0;
        let code = (if north { 32600 } else { 32700 }) + zone as u32;
        let crs = Crs::from_epsg(code).ok_or_else(|| format!("no coordinate system EPSG:{code}"))?;
        let t = crs.transformer()?;
        let back = crs.transformer()?;
        let central = f64::from(zone) * 6.0 - 183.0;
        let name = format!("WGS_1984_UTM_Zone_{zone}{}", if north { 'N' } else { 'S' });
        let wkt = format!(
            "PROJCS[\"{name}\",{WGS84_WKT},PROJECTION[\"Transverse_Mercator\"],PARAMETER[\"False_Easting\",500000.0],PARAMETER[\"False_Northing\",{}],PARAMETER[\"Central_Meridian\",{central}],PARAMETER[\"Scale_Factor\",0.9996],PARAMETER[\"Latitude_Of_Origin\",0.0],UNIT[\"Meter\",1.0]]",
            if north { "0.0" } else { "10000000.0" }
        );
        let note = wide.then(|| format!("The data spans more than one UTM zone; distances are measured in {name} and are approximate away from its central meridian."));
        Ok(Work { wkt, to: Some((t, back)), note })
    }

    /// A longitude/latitude coordinate in the working system.
    fn fwd(&self, c: Coord) -> Option<Coord> {
        match &self.to {
            None => Some(c),
            Some((t, _)) => t.from_lonlat(c),
        }
    }

    /// A working coordinate in longitude/latitude.
    fn back(&self, c: Coord) -> Option<Coord> {
        match &self.to {
            None => Some(c),
            Some((_, t)) => t.to_lonlat(c),
        }
    }

    /// A geometry with every coordinate moved by `f`; `None` when one can't be.
    fn geometry(&self, g: &Geometry, f: impl Fn(&Work, Coord) -> Option<Coord>) -> Option<Geometry> {
        let ok = std::cell::Cell::new(true);
        let out = g.map_coords(&|c| {
            f(self, c).filter(|p| p.x.is_finite() && p.y.is_finite()).unwrap_or_else(|| {
                ok.set(false);
                c
            })
        });
        ok.get().then_some(out)
    }
}

// ---------------------------------------------------------------------------------------------
// conversions

/// A layer (or its selected rows) as a GeoForge feature class.
struct Fc {
    wkt: String,
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
        Some(&self.wkt)
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

fn to_fc(ds: &Dataset, rows: &[usize], work: &Work) -> Fc {
    let mut kind = kind_of(ds.geometry_type);
    if kind == Some(gf::GeometryKind::Point) && has_multipoints(ds) {
        kind = Some(gf::GeometryKind::Multipoint);
    }
    let features = rows
        .iter()
        .filter_map(|&r| ds.features.get(r).map(|f| (r, f)))
        .map(|(_, f)| gf::Feature {
            oid: f.id as i64,
            geometry: f.geometry.as_ref().and_then(|g| work.geometry(g, Work::fwd)).as_ref().and_then(to_gf_geometry),
            attributes: f.attrs.iter().zip(&ds.fields).map(|(v, fl)| to_gf_value(v, fl.ty)).collect(),
        })
        .collect();
    Fc { wkt: work.wkt.clone(), kind, fields: ds.fields.iter().map(to_gf_field).collect(), features }
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
fn to_dataset(out: tools::ToolOutput, name: String, work: &Work) -> Dataset {
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
        let g = f.geometry.as_ref().and_then(from_geoforge).and_then(|g| work.geometry(&g, Work::back));
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

/// A layer as a feature class in `work`'s coordinate system. As in ArcGIS, a layer with a
/// selection is read as its selected features unless `whole`.
fn layer_in<'a>(inp: &'a dyn Inputs, p: &Json, k: &str, work: &Work, whole: bool) -> Result<(&'a Dataset, Fc), String> {
    let name = text(p, k);
    let ds = inp.dataset(name).ok_or_else(|| format!("no layer named '{name}'"))?;
    let sel = inp.selection(name);
    let rows: Vec<usize> = if sel.is_empty() || whole { (0..ds.features.len()).collect() } else { sel.to_vec() };
    Ok((ds, to_fc(ds, &rows, work)))
}

fn layer<'a>(inp: &'a dyn Inputs, p: &Json, k: &str) -> Result<(&'a Dataset, Fc), String> {
    layer_in(inp, p, k, &Work::degrees(), false)
}

/// The datasets named by parameters `keys`.
fn datasets<'a>(inp: &'a dyn Inputs, p: &Json, keys: &[&str]) -> Result<Vec<&'a Dataset>, String> {
    keys.iter()
        .map(|k| {
            let name = text(p, k);
            inp.dataset(name).ok_or_else(|| format!("no layer named '{name}'"))
        })
        .collect()
}

fn out_name(p: &Json, default: String) -> String {
    let n = text(p, "output");
    if n.is_empty() { default } else { n.to_string() }
}

fn messages(out: &tools::ToolOutput, ds: &Dataset, work: &Work) -> Vec<String> {
    let mut m = vec![format!("{}: {} features", ds.name, ds.features.len())];
    m.extend(work.note.clone());
    m.extend(out.messages.clone());
    m
}

fn finish_in(out: tools::ToolOutput, name: String, work: &Work) -> Result<ToolResult, String> {
    let ds = to_dataset_checked(out.clone(), name, work)?;
    let m = messages(&out, &ds, work);
    Ok(ToolResult { output: Output::Layer(ds), messages: m })
}

fn finish(out: tools::ToolOutput, name: String) -> Result<ToolResult, String> {
    finish_in(out, name, &Work::degrees())
}

/// [`to_dataset`], failing when a result coordinate can't be projected back.
fn to_dataset_checked(out: tools::ToolOutput, name: String, work: &Work) -> Result<Dataset, String> {
    let expected = out.features.iter().filter(|f| f.geometry.is_some()).count();
    let ds = to_dataset(out, name, work);
    let got = ds.features.iter().filter(|f| f.geometry.is_some()).count();
    if got < expected {
        return Err(format!("{} result shapes could not be projected back to longitude/latitude", expected - got));
    }
    Ok(ds)
}

fn table_dataset(t: tools::TableOutput, name: String) -> Dataset {
    let fields = t.fields.iter().map(|f| Field::new(&f.name, from_gf_field_type(f.field_type))).collect();
    let mut ds = Dataset::new(name, GeometryType::None, fields);
    for row in t.rows {
        ds.push_next(None, row.into_iter().map(from_gf_value).collect());
    }
    ds
}

fn err(e: gf::Error) -> String {
    e.to_string()
}

/// A number parameter.
fn number(p: &Json, k: &str) -> Option<f64> {
    p.get(k).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).filter(|f| f.is_finite())
}

/// A distance parameter `k` in the unit parameter `units`, in metres.
fn metres(p: &Json, k: &str) -> Option<f64> {
    let d = number(p, k)?;
    let f = match text(p, "units").to_ascii_uppercase().as_str() {
        "KILOMETERS" => 1000.0,
        "MILES" => 1609.344,
        "FEET" => 0.3048,
        "YARDS" => 0.9144,
        "NAUTICAL_MILES" => 1852.0,
        _ => 1.0,
    };
    Some(d * f)
}

fn list(p: &Json, k: &str) -> Vec<String> {
    text(p, k).split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
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
    let fields = list(p, "fields");
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

fn select(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let clause = text(p, "where");
    let clause = (!clause.is_empty()).then_some(clause);
    finish(tools::select(&fc, clause).map_err(err)?, out_name(p, format!("{}_Select", ds.name)))
}

/// Buffer: in the UTM zone of the input, so the distance is on the ground.
fn buffer(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use geoforge_geometry::{LineEnd, LineSide};
    let ds0 = datasets(inp, p, &["input"])?;
    let work = Work::metres(&ds0)?;
    let d = metres(p, "distance").ok_or("Buffer: 'Distance' must be a number")?;
    let end = match text(p, "line_end").to_ascii_uppercase().as_str() {
        "FLAT" => LineEnd::Flat,
        _ => LineEnd::Round,
    };
    let side = match text(p, "line_side").to_ascii_uppercase().as_str() {
        "LEFT" => LineSide::Left,
        "RIGHT" => LineSide::Right,
        "OUTSIDE_ONLY" => LineSide::OutsideOnly,
        _ => LineSide::Full,
    };
    let dissolve = match text(p, "dissolve").to_ascii_uppercase().as_str() {
        "ALL" => tools::Dissolve::All,
        "LIST" => tools::Dissolve::List(list(p, "dissolve_fields")),
        _ => tools::Dissolve::None,
    };
    // Features are buffered independently unless dissolved: data wider than one UTM zone is
    // buffered a zone at a time, each feature in the zone it lies in.
    if work.note.is_some() && matches!(dissolve, tools::Dissolve::None) {
        return buffer_by_zone(inp, p, d, end, side);
    }
    let (ds, fc) = layer_in(inp, p, "input", &work, false)?;
    let out = tools::buffer(&fc, tools::BufferDistance::Linear(d), end, side, dissolve).map_err(err)?;
    finish_in(out, out_name(p, format!("{}_Buffer", ds.name)), &work)
}

/// Undissolved Buffer of a wide layer: the features grouped by UTM zone, each group buffered in
/// its zone, and the results put back in the input's order (`ORIG_FID`).
fn buffer_by_zone(
    inp: &dyn Inputs,
    p: &Json,
    d: f64,
    end: geoforge_geometry::LineEnd,
    side: geoforge_geometry::LineSide,
) -> Result<ToolResult, String> {
    let name = text(p, "input");
    let ds = inp.dataset(name).ok_or_else(|| format!("no layer named '{name}'"))?;
    let sel = inp.selection(name);
    let rows: Vec<usize> = if sel.is_empty() { (0..ds.features.len()).collect() } else { sel.to_vec() };
    // Rows by zone (and hemisphere), in order of first appearance.
    let mut groups: Vec<((i32, bool), Vec<usize>)> = Vec::new();
    for r in rows {
        let Some(f) = ds.features.get(r) else { continue };
        let c = f.geometry.as_ref().map_or(Coord::new(0.0, 0.0), |g| {
            let e = g.extent();
            Coord::new((e.min_x + e.max_x) / 2.0, (e.min_y + e.max_y) / 2.0)
        });
        let key = ((((c.x + 180.0) / 6.0).floor() as i32).rem_euclid(60) + 1, c.y >= 0.0);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(r),
            None => groups.push((key, vec![r])),
        }
    }
    let mut out: Option<Dataset> = None;
    let mut rows_out: Vec<(i64, Option<Geometry>, Vec<Value>)> = Vec::new();
    let mut messages = Vec::new();
    for ((zone, north), members) in &groups {
        let lon = f64::from(*zone) * 6.0 - 183.0;
        let work = Work::zone(lon, if *north { 1.0 } else { -1.0 }, false)?;
        let fc = to_fc(ds, &members.clone(), &work);
        let res = tools::buffer(&fc, tools::BufferDistance::Linear(d), end, side, tools::Dissolve::None).map_err(err)?;
        messages.extend(res.messages.clone());
        let part = to_dataset_checked(res, ds.name.clone(), &work)?;
        let fid = part.field_index("ORIG_FID");
        for f in part.features {
            let key = fid.and_then(|i| f.attrs.get(i)).and_then(Value::as_f64).unwrap_or(0.0) as i64;
            rows_out.push((key, f.geometry, f.attrs));
        }
        out.get_or_insert_with(|| Dataset::new(ds.name.clone(), part.geometry_type, part.fields));
    }
    let mut out = out.ok_or("Buffer: nothing to buffer")?;
    out.name = out_name(p, format!("{}_Buffer", ds.name));
    rows_out.sort_by_key(|r| r.0);
    for (_, g, a) in rows_out {
        out.push_next(g, a);
    }
    let mut m = vec![format!("{}: {} features, buffered in {} UTM zone(s)", out.name, out.features.len(), groups.len())];
    m.extend(messages);
    Ok(ToolResult { output: Output::Layer(out), messages: m })
}

/// Near: adds `NEAR_FID`/`NEAR_DIST` (metres) to the input layer, as ArcGIS does in place.
fn near(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let all = datasets(inp, p, &["input", "near"])?;
    let work = Work::metres(&all)?;
    let (ds, fc) = layer_in(inp, p, "input", &work, true)?;
    let (nds, nfc) = layer_in(inp, p, "near", &work, false)?;
    let radius = metres(p, "search_radius");
    let (loc, ang) = (flag(p, "location"), flag(p, "angle"));
    let near = [tools::PathInput { source: nds.name.clone(), features: &nfc }];
    let out = tools::near(&fc, &near, radius, loc, ang).map_err(err)?;
    let mut out_ds = to_dataset_checked(out.clone(), ds.name.clone(), &work)?;
    // NEAR_X / NEAR_Y are working-system coordinates: back to longitude/latitude.
    if loc && let (Some(xi), Some(yi)) = (out_ds.field_index("NEAR_X"), out_ds.field_index("NEAR_Y")) {
        for f in &mut out_ds.features {
            let (x, y) = (f.attrs.get(xi).and_then(Value::as_f64), f.attrs.get(yi).and_then(Value::as_f64));
            if let (Some(x), Some(y)) = (x, y)
                && !(x == -1.0 && y == -1.0)
                && let Some(c) = work.back(Coord::new(x, y))
            {
                f.attrs[xi] = Value::Float(c.x);
                f.attrs[yi] = Value::Float(c.y);
            }
        }
    }
    let m = messages(&out, &out_ds, &work);
    Ok(ToolResult { output: Output::Update { layer: ds.name.clone(), dataset: out_ds }, messages: m })
}

fn spatial_join(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let option = tools::MatchOption::parse(text(p, "match_option")).unwrap_or(tools::MatchOption::Intersect);
    let measures = matches!(option, tools::MatchOption::WithinADistance | tools::MatchOption::Closest);
    let all = datasets(inp, p, &["input", "other"])?;
    let work = if measures { Work::metres(&all)? } else { Work::degrees() };
    let (target, tfc) = layer_in(inp, p, "input", &work, false)?;
    let (join, jfc) = layer_in(inp, p, "other", &work, false)?;
    let opts = tools::SpatialJoinOptions {
        merge_rules: Vec::new(),
        one_to_many: text(p, "join_operation").eq_ignore_ascii_case("JOIN_ONE_TO_MANY"),
        keep_all: !flag(p, "keep_common"),
        match_option: option,
        search_radius: metres(p, "search_radius"),
        distance_field_name: Some(text(p, "distance_field_name").to_string()).filter(|s| !s.is_empty()),
        field_map: None,
        target_name: target.name.clone(),
        join_name: join.name.clone(),
    };
    let out = tools::spatial_join(&tfc, &jfc, &opts).map_err(err)?;
    finish_in(out, out_name(p, format!("{}_SpatialJoin", target.name)), &work)
}

fn select_by_location(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let overlap = tools::Overlap::parse(text(p, "overlap_type")).unwrap_or(tools::Overlap::Intersect);
    let distance = metres(p, "search_distance");
    let measures = matches!(overlap, tools::Overlap::WithinADistance)
        || (matches!(overlap, tools::Overlap::Intersect) && distance.is_some_and(|d| d > 0.0));
    let all = datasets(inp, p, &["input", "other"])?;
    let work = if measures { Work::metres(&all)? } else { Work::degrees() };
    let (ds, mut fc) = layer_in(inp, p, "input", &work, false)?;
    let (_, sfc) = layer_in(inp, p, "other", &work, false)?;
    // The tool renumbers its output's OIDs, so each feature carries its own through a hidden field.
    fc.fields.push(gf::Field::new("__ID", gf::FieldType::BigInteger));
    for f in &mut fc.features {
        f.attributes.push(gf::Value::BigInteger(f.oid));
    }
    let out = tools::select_by_location(&fc, overlap, &sfc, distance, flag(p, "invert")).map_err(err)?;
    let ids: std::collections::HashSet<i64> = out
        .features
        .iter()
        .filter_map(|f| if let Some(gf::Value::BigInteger(i)) = f.attributes.last() { Some(*i) } else { None })
        .collect();
    let rows: Vec<usize> = ds.features.iter().enumerate().filter(|(_, f)| ids.contains(&(f.id as i64))).map(|(i, _)| i).collect();
    let mut m = vec![format!("{} of {} features selected", rows.len(), ds.features.len())];
    m.extend(work.note.clone());
    Ok(ToolResult { output: Output::Selection { layer: ds.name.clone(), rows }, messages: m })
}

fn frequency(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let t = tools::frequency(&fc, &list(p, "fields"), &list(p, "summary_fields")).map_err(err)?;
    let t = table_dataset(t, out_name(p, format!("{}_Frequency", ds.name)));
    let m = vec![format!("{}: {} rows", t.name, t.features.len())];
    Ok(ToolResult { output: Output::Table(t), messages: m })
}

fn statistics(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    // "FIELD STAT;FIELD STAT"
    let stats: Vec<(String, tools::Statistic)> = list(p, "statistics")
        .iter()
        .map(|s| {
            let (f, st) = s.rsplit_once(char::is_whitespace).ok_or_else(|| format!("'{s}': write FIELD STATISTIC"))?;
            let st = tools::Statistic::parse(st.trim()).ok_or_else(|| format!("unknown statistic '{st}'"))?;
            Ok((f.trim().to_string(), st))
        })
        .collect::<Result<_, String>>()?;
    let t = tools::statistics(&fc, &stats, &list(p, "case_fields")).map_err(err)?;
    let t = table_dataset(t, out_name(p, format!("{}_Statistics", ds.name)));
    let m = vec![format!("{}: {} rows", t.name, t.features.len())];
    Ok(ToolResult { output: Output::Table(t), messages: m })
}

const INPUT: P = P { name: "input", label: "Input Features", kind: K::Layer, required: true, default: "", help: "" };
const OTHER: P = P { name: "other", label: "Overlay Features", kind: K::Layer, required: true, default: "", help: "" };
const UNITS: P = P {
    name: "units",
    label: "Units",
    kind: K::Choice(&["METERS", "KILOMETERS", "MILES", "FEET", "YARDS", "NAUTICAL_MILES"]),
    required: false,
    default: "\"METERS\"",
    help: "Unit of the distances.",
};
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
    Def {
        id: "select",
        name: "Select",
        toolbox: "GeoForge › Analysis › Extract",
        summary: "Features that match an SQL expression (ArcGIS Pro Select).",
        params: &[
            INPUT,
            P {
                name: "where",
                label: "Expression",
                kind: K::Text,
                required: false,
                default: "",
                help: "SQL, e.g. POP > 100 AND NAME LIKE 'A%'",
            },
            OUTPUT,
        ],
        run: select,
    },
    Def {
        id: "buffer",
        name: "Buffer",
        toolbox: "GeoForge › Analysis › Proximity",
        summary: "Zones around features, measured on the ground in the data's UTM zone (ArcGIS Pro Buffer, planar).",
        params: &[
            INPUT,
            P { name: "distance", label: "Distance", kind: K::Number, required: true, default: "", help: "" },
            UNITS,
            P {
                name: "line_side",
                label: "Side Type",
                kind: K::Choice(&["FULL", "LEFT", "RIGHT", "OUTSIDE_ONLY"]),
                required: false,
                default: "\"FULL\"",
                help: "",
            },
            P { name: "line_end", label: "End Type", kind: K::Choice(&["ROUND", "FLAT"]), required: false, default: "\"ROUND\"", help: "" },
            P {
                name: "dissolve",
                label: "Dissolve Type",
                kind: K::Choice(&["NONE", "ALL", "LIST"]),
                required: false,
                default: "\"NONE\"",
                help: "",
            },
            P {
                name: "dissolve_fields",
                label: "Dissolve Fields",
                kind: K::Text,
                required: false,
                default: "",
                help: "For LIST: field names separated by semicolons.",
            },
            OUTPUT,
        ],
        run: buffer,
    },
    Def {
        id: "near",
        name: "Near",
        toolbox: "GeoForge › Analysis › Proximity",
        summary: "Adds the distance (m) and ID of the nearest feature to the input layer (ArcGIS Pro Near, in place).",
        params: &[
            INPUT,
            P { name: "near", label: "Near Features", kind: K::Layer, required: true, default: "", help: "" },
            P {
                name: "search_radius",
                label: "Search Radius",
                kind: K::Number,
                required: false,
                default: "",
                help: "No radius: search everywhere.",
            },
            UNITS,
            P {
                name: "location",
                label: "Add NEAR_X/NEAR_Y",
                kind: K::Bool,
                required: false,
                default: "false",
                help: "Written as longitude and latitude.",
            },
            P {
                name: "angle",
                label: "Add NEAR_ANGLE",
                kind: K::Bool,
                required: false,
                default: "false",
                help: "Degrees counter-clockwise from the UTM grid's east.",
            },
        ],
        run: near,
    },
    Def {
        id: "spatial_join",
        name: "Spatial Join",
        toolbox: "GeoForge › Analysis › Overlay",
        summary: "Joins attributes of one layer to another by location (ArcGIS Pro Spatial Join).",
        params: &[
            INPUT,
            OTHER,
            P {
                name: "join_operation",
                label: "Join Operation",
                kind: K::Choice(&["JOIN_ONE_TO_ONE", "JOIN_ONE_TO_MANY"]),
                required: false,
                default: "\"JOIN_ONE_TO_ONE\"",
                help: "",
            },
            P { name: "keep_common", label: "Keep only matching features", kind: K::Bool, required: false, default: "false", help: "" },
            P {
                name: "match_option",
                label: "Match Option",
                kind: K::Choice(&[
                    "INTERSECT",
                    "WITHIN_A_DISTANCE",
                    "CONTAINS",
                    "COMPLETELY_CONTAINS",
                    "WITHIN",
                    "COMPLETELY_WITHIN",
                    "CLOSEST",
                ]),
                required: false,
                default: "\"INTERSECT\"",
                help: "",
            },
            P { name: "search_radius", label: "Search Radius", kind: K::Number, required: false, default: "", help: "" },
            UNITS,
            P {
                name: "distance_field_name",
                label: "Distance Field Name",
                kind: K::Text,
                required: false,
                default: "",
                help: "For CLOSEST: metres.",
            },
            OUTPUT,
        ],
        run: spatial_join,
    },
    Def {
        id: "select_by_location",
        name: "Select Layer By Location",
        toolbox: "GeoForge › Data Management › Layers and Table Views",
        summary: "Selects the features of a layer related to features of another (ArcGIS Pro Select Layer By Location).",
        params: &[
            INPUT,
            P {
                name: "overlap_type",
                label: "Relationship",
                kind: K::Choice(&[
                    "INTERSECT",
                    "WITHIN_A_DISTANCE",
                    "CONTAINS",
                    "COMPLETELY_CONTAINS",
                    "CONTAINS_CLEMENTINI",
                    "WITHIN",
                    "COMPLETELY_WITHIN",
                    "WITHIN_CLEMENTINI",
                    "ARE_IDENTICAL_TO",
                    "BOUNDARY_TOUCHES",
                    "SHARE_A_LINE_SEGMENT_WITH",
                    "CROSSED_BY_THE_OUTLINE_OF",
                    "HAVE_THEIR_CENTER_IN",
                ]),
                required: false,
                default: "\"INTERSECT\"",
                help: "",
            },
            OTHER,
            P { name: "search_distance", label: "Search Distance", kind: K::Number, required: false, default: "", help: "" },
            UNITS,
            P { name: "invert", label: "Invert spatial relationship", kind: K::Bool, required: false, default: "false", help: "" },
        ],
        run: select_by_location,
    },
    Def {
        id: "frequency",
        name: "Frequency",
        toolbox: "GeoForge › Analysis › Statistics",
        summary: "A table of how often each combination of field values occurs (ArcGIS Pro Frequency).",
        params: &[
            INPUT,
            P {
                name: "fields",
                label: "Frequency Fields",
                kind: K::Text,
                required: true,
                default: "",
                help: "Field names separated by semicolons.",
            },
            P {
                name: "summary_fields",
                label: "Summary Fields",
                kind: K::Text,
                required: false,
                default: "",
                help: "Numeric fields to sum, separated by semicolons.",
            },
            OUTPUT,
        ],
        run: frequency,
    },
    Def {
        id: "statistics",
        name: "Summary Statistics",
        toolbox: "GeoForge › Analysis › Statistics",
        summary: "Statistics of fields, per case field (ArcGIS Pro Summary Statistics).",
        params: &[
            INPUT,
            P {
                name: "statistics",
                label: "Statistics Fields",
                kind: K::Text,
                required: true,
                default: "",
                help: "FIELD STATISTIC pairs separated by semicolons: POP SUM;AREA MEAN. Statistics: SUM MEAN MIN MAX RANGE STD COUNT FIRST LAST MEDIAN VARIANCE UNIQUE.",
            },
            P {
                name: "case_fields",
                label: "Case Fields",
                kind: K::Text,
                required: false,
                default: "",
                help: "Field names separated by semicolons.",
            },
            OUTPUT,
        ],
        run: statistics,
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

    fn points(name: &str, pts: &[(f64, f64)]) -> Dataset {
        let mut d = Dataset::new(name, GeometryType::Point, vec![Field::new("N", FieldType::Integer)]);
        for (i, (x, y)) in pts.iter().enumerate() {
            d.push(Some(Geometry::Point(Coord::new(*x, *y))), vec![Value::Int(i as i64)]);
        }
        d
    }

    /// Ground distance in metres between two lon/lat points (haversine).
    fn ground(a: Coord, b: Coord) -> f64 {
        let (p, q) = (a.y.to_radians(), b.y.to_radians());
        let h = ((q - p) / 2.0).sin().powi(2) + p.cos() * q.cos() * ((b.x - a.x).to_radians() / 2.0).sin().powi(2);
        2.0 * 6_371_008.8 * h.sqrt().asin()
    }

    #[test]
    fn buffer_distance_is_on_the_ground() {
        register();
        // Sydney and Tromsø: a degree is a very different length in each.
        for (lon, lat) in [(151.2, -33.9), (18.9, 69.6)] {
            let layers = Layers(vec![points("p", &[(lon, lat)])]);
            let d =
                dataset(rustgis_analysis::run("geoforge:buffer", &layers, &json!({"input": "p", "distance": 1, "units": "KILOMETERS"})));
            let Some(Geometry::Polygon(ps)) = &d.features[0].geometry else { panic!("polygon") };
            let c = Coord::new(lon, lat);
            let ring = &ps[0][0];
            assert!(ring.len() > 16);
            for v in ring {
                let r = ground(c, *v);
                assert!((r - 1000.0).abs() < 5.0, "{lat}: {r}");
            }
            // ArcGIS's schema for a buffer: the input fields, BUFF_DIST, ORIG_FID.
            let names: Vec<&str> = d.fields.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(names, ["N", "BUFF_DIST", "ORIG_FID"]);
        }
    }

    #[test]
    fn wide_layers_are_buffered_a_zone_at_a_time() {
        register();
        // Three continents: one UTM zone would put two of these far from its central meridian.
        let spots = [(151.2, -33.9), (18.9, 69.6), (-118.2, 34.0), (151.3, -33.8)];
        let layers = Layers(vec![points("p", &spots)]);
        let r = rustgis_analysis::run("geoforge:buffer", &layers, &json!({"input": "p", "distance": 1000})).unwrap();
        assert!(r.messages.iter().any(|m| m.contains("UTM zone")), "{:?}", r.messages);
        let Output::Layer(d) = r.output else { panic!("layer") };
        assert_eq!(d.features.len(), 4);
        for (f, (lon, lat)) in d.features.iter().zip(spots) {
            // Input order kept: feature i is the buffer of point i.
            let Some(Geometry::Polygon(ps)) = &f.geometry else { panic!("polygon") };
            let c = Coord::new(lon, lat);
            for v in &ps[0][0] {
                let r = ground(c, *v);
                assert!((r - 1000.0).abs() < 5.0, "{lat}: {r}");
            }
        }
        let ids: Vec<&Value> = d.features.iter().map(|f| &f.attrs[d.field_index("ORIG_FID").unwrap()]).collect();
        assert_eq!(ids, [&Value::Int(1), &Value::Int(2), &Value::Int(3), &Value::Int(4)]);
    }

    #[test]
    fn near_measures_metres_and_updates_in_place() {
        register();
        let layers = Layers(vec![points("a", &[(10.0, 50.0)]), points("b", &[(10.0, 50.01), (11.0, 50.0)])]);
        let r = rustgis_analysis::run("geoforge:near", &layers, &json!({"input": "a", "near": "b", "location": true})).unwrap();
        let Output::Update { layer, dataset } = r.output else { panic!("update") };
        assert_eq!(layer, "a");
        let at = |n: &str| dataset.features[0].attrs[dataset.field_index(n).unwrap()].clone();
        let want = ground(Coord::new(10.0, 50.0), Coord::new(10.0, 50.01));
        assert!(matches!(at("NEAR_DIST"), Value::Float(d) if (d - want).abs() < 1.0), "{:?} vs {want}", at("NEAR_DIST"));
        assert_eq!(at("NEAR_FID"), Value::Int(1));
        // NEAR_X/NEAR_Y come back as longitude/latitude.
        assert!(matches!(at("NEAR_Y"), Value::Float(y) if (y - 50.01).abs() < 1e-6));
    }

    #[test]
    fn select_by_distance_and_tables() {
        register();
        let layers = Layers(vec![points("a", &[(10.0, 50.0), (10.0, 50.5)]), points("b", &[(10.0, 50.001)])]);
        let r = rustgis_analysis::run(
            "geoforge:select_by_location",
            &layers,
            &json!({"input": "a", "other": "b", "overlap_type": "WITHIN_A_DISTANCE", "search_distance": 500}),
        )
        .unwrap();
        let Output::Selection { rows, .. } = r.output else { panic!("selection") };
        assert_eq!(rows, vec![0]);
        let f = rustgis_analysis::run("geoforge:frequency", &layers, &json!({"input": "a", "fields": "N"})).unwrap();
        let Output::Table(t) = f.output else { panic!("table") };
        assert_eq!(t.features.len(), 2);
        let s = rustgis_analysis::run("geoforge:statistics", &layers, &json!({"input": "a", "statistics": "N SUM"})).unwrap();
        let Output::Table(t) = s.output else { panic!("table") };
        assert_eq!(t.features[0].attrs, vec![Value::Int(2), Value::Float(1.0)]);
    }

    #[test]
    fn bad_parameters_are_errors_not_panics() {
        register();
        let layers = Layers(vec![square("a", 0.0, 0.0, 1.0)]);
        for id in [
            "clip",
            "erase",
            "intersect",
            "union",
            "dissolve",
            "feature_to_point",
            "buffer",
            "near",
            "spatial_join",
            "select_by_location",
            "select",
            "frequency",
            "statistics",
        ] {
            let _ = rustgis_analysis::run(&format!("geoforge:{id}"), &layers, &json!({"input": "nope", "clip": 3, "other": null}));
            let _ = rustgis_analysis::run(
                &format!("geoforge:{id}"),
                &layers,
                &json!({"input": "a", "clip": "a", "erase": "a", "other": "a", "fields": "NOPE"}),
            );
        }
    }
}
