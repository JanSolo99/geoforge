//! GeoForge's ArcGIS-verified geoprocessing tools (`geoforge-tools`) as GeoForge tools.
//!
//! Modified for GeoForge (not part of upstream GeoForge).
//!
//! The tools join the registry as the external suite [`SOURCE`] (ids `geoforge:<name>`), through
//! [`rustgis_analysis::register_external`]. A tool's layers are converted to GeoForge feature
//! classes, the engine runs the ArcGIS algorithm (parameters, output schema, row order as
//! ArcGIS's), and its output comes back as a layer.
//!
//! Layers in GeoForge are WGS 84 longitude/latitude. Tools that only relate shapes (Clip, Erase,
//! Intersect, ...) run on them as they are, with ArcGIS's geographic XY tolerance. Tools that
//! measure (Buffer, Near, search distances) run in the UTM zone of the data, as ArcGIS does on
//! projected data, in metres, and their results are projected back: see [`Work`].
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod extra;
mod raster;

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

/// Buffer: geodesic, as ArcGIS buffers geographic data; a UTM zone where that cannot be drawn.
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
    // The layers are longitude/latitude, so the buffer is ArcGIS's geodesic one: the distance on the
    // ellipsoid, vertex for vertex as ArcGIS places them. Where it would cross the antimeridian or
    // reach a pole, the buffer falls back to the UTM zone of the data (a zone at a time for wide,
    // undissolved layers).
    let degrees = Work::degrees();
    let (ds, fc) = layer_in(inp, p, "input", &degrees, false)?;
    match tools::buffer(&fc, tools::BufferDistance::Geodesic(d), end, side, dissolve.clone()) {
        Ok(out) => return finish_in(out, out_name(p, format!("{}_Buffer", ds.name)), &degrees),
        Err(e) if e.to_string().contains("antimeridian") => {}
        Err(e) => return Err(err(e)),
    }
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
    let work = Work::degrees();
    let (ds, fc) = layer_in(inp, p, "input", &work, true)?;
    let (nds, nfc) = layer_in(inp, p, "near", &work, false)?;
    let radius = metres(p, "search_radius");
    let (loc, ang) = (flag(p, "location"), flag(p, "angle"));
    let near = [tools::PathInput { source: nds.name.clone(), features: &nfc }];
    let out = tools::near_with(&fc, &near, radius, loc, ang, tools::NearMethod::Geodesic).map_err(err)?;
    let out_ds = to_dataset_checked(out.clone(), ds.name.clone(), &work)?;
    let m = messages(&out, &out_ds, &work);
    Ok(ToolResult { output: Output::Update { layer: ds.name.clone(), dataset: out_ds }, messages: m })
}

fn spatial_join(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let option = match tools::MatchOption::parse(text(p, "match_option")).unwrap_or(tools::MatchOption::Intersect) {
        // Distances are on the ground.
        tools::MatchOption::WithinADistance => tools::MatchOption::WithinADistanceGeodesic,
        tools::MatchOption::Closest => tools::MatchOption::ClosestGeodesic,
        other => other,
    };
    let work = Work::degrees();
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

/// A tool's output replacing the layer it was run on (the tools ArcGIS runs in place).
fn finish_update(out: tools::ToolOutput, ds: &Dataset) -> Result<ToolResult, String> {
    let work = Work::degrees();
    let new = to_dataset_checked(out.clone(), ds.name.clone(), &work)?;
    let m = messages(&out, &new, &work);
    Ok(ToolResult { output: Output::Update { layer: ds.name.clone(), dataset: new }, messages: m })
}

fn identity(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let [(an, a), (bn, b)] = two(inp, p)?;
    let opts = tools::IdentityOptions {
        join_attributes: tools::JoinAttributes::parse(text(p, "join_attributes")).unwrap_or_default(),
        keep_relationships: flag(p, "keep_relationships"),
    };
    let out =
        tools::identity_with(tools::NamedInput { name: an.clone(), features: &a }, tools::NamedInput { name: bn, features: &b }, &opts)
            .map_err(err)?;
    finish(out, out_name(p, format!("{an}_Identity")))
}

fn update(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, a) = layer(inp, p, "input")?;
    let (_, b) = layer(inp, p, "other")?;
    finish(tools::update(&a, &b).map_err(err)?, out_name(p, format!("{}_Update", ds.name)))
}

/// Merge: the layers named in `inputs` (semicolon-separated) into one.
fn merge(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let names = list(p, "inputs");
    if names.is_empty() {
        return Err("Merge: name the layers to merge".into());
    }
    let fcs: Vec<(String, Fc)> = names
        .iter()
        .map(|n| layer(inp, &serde_json::json!({ "x": n }), "x").map(|(d, f)| (d.name.clone(), f)))
        .collect::<Result<_, _>>()?;
    let inputs: Vec<tools::PathInput> = fcs.iter().map(|(n, f)| tools::PathInput { source: n.clone(), features: f }).collect();
    let out = tools::merge_with(&inputs, flag(p, "add_source"), None).map_err(err)?;
    finish(out, out_name(p, "Merge".to_string()))
}

/// Append: the layers in `inputs` added to the target layer, which is replaced by the result.
fn append(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let names = list(p, "inputs");
    let (target_ds, target) = layer_in(inp, p, "input", &Work::degrees(), true)?;
    let fcs: Vec<(String, Fc)> = names
        .iter()
        .map(|n| layer(inp, &serde_json::json!({ "x": n }), "x").map(|(d, f)| (d.name.clone(), f)))
        .collect::<Result<_, _>>()?;
    let inputs: Vec<tools::PathInput> = fcs.iter().map(|(n, f)| tools::PathInput { source: n.clone(), features: f }).collect();
    let test = text(p, "schema_type").eq_ignore_ascii_case("TEST");
    let out = tools::append_with(&inputs, &target, test, None).map_err(err)?;
    finish_update(out, target_ds)
}

fn minimum_bounding_geometry(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use geoforge_geometry::BoundingKind as K;
    let (ds, fc) = layer(inp, p, "input")?;
    let kind = match text(p, "geometry_type").to_ascii_uppercase().as_str() {
        "ENVELOPE" => K::Envelope,
        "CONVEX_HULL" => K::ConvexHull,
        "RECTANGLE_BY_WIDTH" => K::RectangleByWidth,
        "CIRCLE" => K::Circle,
        _ => K::RectangleByArea,
    };
    let group = match text(p, "group_option").to_ascii_uppercase().as_str() {
        "ALL" => tools::BoundingGroup::All,
        "LIST" => tools::BoundingGroup::List(list(p, "group_fields")),
        _ => tools::BoundingGroup::None,
    };
    let out = tools::minimum_bounding_geometry(&fc, kind, &group, flag(p, "mbg_fields")).map_err(err)?;
    finish(out, out_name(p, format!("{}_MBG", ds.name)))
}

fn feature_vertices_to_points(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let at = tools::VertexLocation::parse(text(p, "point_location")).unwrap_or(tools::VertexLocation::All);
    finish(tools::feature_vertices_to_points(&fc, at).map_err(err)?, out_name(p, format!("{}_Vertices", ds.name)))
}

fn points_to_line(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let opt = |k: &str| Some(text(p, k)).filter(|s| !s.is_empty());
    let out = tools::points_to_line(&fc, opt("line_field"), opt("sort_field"), flag(p, "close")).map_err(err)?;
    finish(out, out_name(p, format!("{}_Line", ds.name)))
}

fn sort(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    // "FIELD ASCENDING;FIELD2 DESCENDING"
    let keys: Vec<(String, bool)> = list(p, "sort_fields")
        .iter()
        .map(|k| match k.rsplit_once(char::is_whitespace) {
            Some((f, o)) if o.eq_ignore_ascii_case("DESCENDING") => (f.trim().to_string(), false),
            Some((f, o)) if o.eq_ignore_ascii_case("ASCENDING") => (f.trim().to_string(), true),
            _ => (k.clone(), true),
        })
        .collect();
    let spatial = tools::SpatialSort::parse(text(p, "spatial_sort_method")).unwrap_or(tools::SpatialSort::UpperRight);
    finish(tools::sort(&fc, &keys, spatial).map_err(err)?, out_name(p, format!("{}_Sort", ds.name)))
}

fn add_xy(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer_in(inp, p, "input", &Work::degrees(), true)?;
    finish_update(tools::add_xy(&fc).map_err(err)?, ds)
}

fn add_field(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer_in(inp, p, "input", &Work::degrees(), true)?;
    let n = |k: &str| number(p, k).map(|v| v as u32);
    let out =
        tools::add_field(&fc, text(p, "field_name"), text(p, "field_type"), n("field_precision"), n("field_scale"), n("field_length"))
            .map_err(err)?;
    finish_update(out, ds)
}

fn delete_field(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer_in(inp, p, "input", &Work::degrees(), true)?;
    let keep = text(p, "method").eq_ignore_ascii_case("KEEP_FIELDS");
    finish_update(tools::delete_field(&fc, &list(p, "fields"), keep).map_err(err)?, ds)
}

fn calculate_field(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer_in(inp, p, "input", &Work::degrees(), true)?;
    let ty = Some(text(p, "field_type")).filter(|s| !s.is_empty());
    let out = tools::calculate_field(&fc, text(p, "field"), text(p, "expression"), text(p, "code_block"), ty).map_err(err)?;
    finish_update(out, ds)
}

fn delete_identical(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer_in(inp, p, "input", &Work::degrees(), true)?;
    let tol = number(p, "xy_tolerance").unwrap_or(0.001);
    finish_update(tools::delete_identical(&fc, &list(p, "fields"), tol).map_err(err)?, ds)
}

fn find_identical(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let t = tools::find_identical(&fc, &list(p, "fields"), flag(p, "only_duplicates")).map_err(err)?;
    let t = table_dataset(t, out_name(p, format!("{}_Identical", ds.name)));
    let m = vec![format!("{}: {} rows", t.name, t.features.len())];
    Ok(ToolResult { output: Output::Table(t), messages: m })
}

/// Generate Near Table: distances in metres.
fn generate_near_table(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let work = Work::degrees();
    let (ds, fc) = layer_in(inp, p, "input", &work, false)?;
    let (_, nfc) = layer_in(inp, p, "near", &work, false)?;
    let count = number(p, "closest_count").map_or(1, |c| c.max(1.0) as usize);
    let all_matches = text(p, "method").eq_ignore_ascii_case("ALL");
    let t = tools::generate_near_table_with(
        &fc,
        &nfc,
        metres(p, "search_radius"),
        false,
        flag(p, "angle"),
        all_matches,
        count,
        tools::NearMethod::Geodesic,
    )
    .map_err(err)?;
    let t = table_dataset(t, out_name(p, format!("{}_NearTable", ds.name)));
    let mut m = vec![format!("{}: {} rows", t.name, t.features.len())];
    m.extend(work.note);
    Ok(ToolResult { output: Output::Table(t), messages: m })
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
        summary: "Adds the distance (m, on the ellipsoid) and ID of the nearest feature to the input layer (ArcGIS Pro Near with GEODESIC, in place).",
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
                help: "Azimuth in degrees clockwise from north.",
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
    Def {
        id: "identity",
        name: "Identity",
        toolbox: "GeoForge › Analysis › Overlay",
        summary: "Splits the input where identity polygons overlap it, keeping all input (ArcGIS Pro Identity).",
        params: &[
            INPUT,
            OTHER,
            P {
                name: "join_attributes",
                label: "Attributes to Join",
                kind: K::Choice(&["ALL", "NO_FID", "ONLY_FID"]),
                required: false,
                default: "\"ALL\"",
                help: "",
            },
            P {
                name: "keep_relationships",
                label: "Keep relationships",
                kind: K::Bool,
                required: false,
                default: "false",
                help: "Lines and points: LEFT_/RIGHT_ columns.",
            },
            OUTPUT,
        ],
        run: identity,
    },
    Def {
        id: "update",
        name: "Update",
        toolbox: "GeoForge › Analysis › Overlay",
        summary: "The input with the update polygons laid over it (ArcGIS Pro Update).",
        params: &[INPUT, OTHER, OUTPUT],
        run: update,
    },
    Def {
        id: "merge",
        name: "Merge",
        toolbox: "GeoForge › Data Management › General",
        summary: "Combines layers into one, matching fields by name (ArcGIS Pro Merge).",
        params: &[
            P {
                name: "inputs",
                label: "Input Datasets",
                kind: K::Text,
                required: true,
                default: "",
                help: "Layer names separated by semicolons.",
            },
            P {
                name: "add_source",
                label: "Add source information (MERGE_SRC)",
                kind: K::Bool,
                required: false,
                default: "false",
                help: "",
            },
            OUTPUT,
        ],
        run: merge,
    },
    Def {
        id: "append",
        name: "Append",
        toolbox: "GeoForge › Data Management › General",
        summary: "Adds features of other layers to a target layer, which is updated (ArcGIS Pro Append).",
        params: &[
            P {
                name: "inputs",
                label: "Input Datasets",
                kind: K::Text,
                required: true,
                default: "",
                help: "Layer names separated by semicolons.",
            },
            P { name: "input", label: "Target Dataset", kind: K::Layer, required: true, default: "", help: "" },
            P {
                name: "schema_type",
                label: "Field Matching Type",
                kind: K::Choice(&["NO_TEST", "TEST"]),
                required: false,
                default: "\"NO_TEST\"",
                help: "",
            },
        ],
        run: append,
    },
    Def {
        id: "minimum_bounding_geometry",
        name: "Minimum Bounding Geometry",
        toolbox: "GeoForge › Data Management › Features",
        summary: "A polygon around each feature or group (ArcGIS Pro Minimum Bounding Geometry).",
        params: &[
            INPUT,
            P {
                name: "geometry_type",
                label: "Geometry Type",
                kind: K::Choice(&["RECTANGLE_BY_AREA", "RECTANGLE_BY_WIDTH", "CONVEX_HULL", "CIRCLE", "ENVELOPE"]),
                required: false,
                default: "\"RECTANGLE_BY_AREA\"",
                help: "",
            },
            P {
                name: "group_option",
                label: "Group Option",
                kind: K::Choice(&["NONE", "ALL", "LIST"]),
                required: false,
                default: "\"NONE\"",
                help: "",
            },
            P {
                name: "group_fields",
                label: "Group Fields",
                kind: K::Text,
                required: false,
                default: "",
                help: "For LIST: field names separated by semicolons.",
            },
            P {
                name: "mbg_fields",
                label: "Add geometry characteristic fields",
                kind: K::Bool,
                required: false,
                default: "false",
                help: "",
            },
            OUTPUT,
        ],
        run: minimum_bounding_geometry,
    },
    Def {
        id: "feature_vertices_to_points",
        name: "Feature Vertices To Points",
        toolbox: "GeoForge › Data Management › Features",
        summary: "Points at the vertices, ends, middle or dangles of lines and polygons (ArcGIS Pro Feature Vertices To Points).",
        params: &[
            INPUT,
            P {
                name: "point_location",
                label: "Point Type",
                kind: K::Choice(&["ALL", "START", "END", "BOTH_ENDS", "MID", "DANGLE"]),
                required: false,
                default: "\"ALL\"",
                help: "",
            },
            OUTPUT,
        ],
        run: feature_vertices_to_points,
    },
    Def {
        id: "points_to_line",
        name: "Points To Line",
        toolbox: "GeoForge › Data Management › Features",
        summary: "Connects points into lines, by line field and sort field (ArcGIS Pro Points To Line).",
        params: &[
            INPUT,
            P { name: "line_field", label: "Line Field", kind: K::Field, required: false, default: "", help: "" },
            P { name: "sort_field", label: "Sort Field", kind: K::Field, required: false, default: "", help: "" },
            P { name: "close", label: "Close Line", kind: K::Bool, required: false, default: "false", help: "" },
            OUTPUT,
        ],
        run: points_to_line,
    },
    Def {
        id: "sort",
        name: "Sort",
        toolbox: "GeoForge › Data Management › General",
        summary: "Reorders features by fields or by position (ArcGIS Pro Sort).",
        params: &[
            INPUT,
            P {
                name: "sort_fields",
                label: "Sort Fields",
                kind: K::Text,
                required: true,
                default: "",
                help: "FIELD ASCENDING|DESCENDING pairs separated by semicolons; Shape sorts by position.",
            },
            P {
                name: "spatial_sort_method",
                label: "Spatial Sort Method",
                kind: K::Choice(&["UR", "UL", "LR", "LL"]),
                required: false,
                default: "\"UR\"",
                help: "",
            },
            OUTPUT,
        ],
        run: sort,
    },
    Def {
        id: "add_xy",
        name: "Add XY Coordinates",
        toolbox: "GeoForge › Data Management › Features",
        summary: "Adds POINT_X and POINT_Y to the layer, in place (ArcGIS Pro Add XY Coordinates).",
        params: &[INPUT],
        run: add_xy,
    },
    Def {
        id: "add_field",
        name: "Add Field",
        toolbox: "GeoForge › Data Management › Fields",
        summary: "Adds a field to the layer, in place (ArcGIS Pro Add Field).",
        params: &[
            INPUT,
            P { name: "field_name", label: "Field Name", kind: K::Text, required: true, default: "", help: "" },
            P {
                name: "field_type",
                label: "Field Type",
                kind: K::Choice(&["TEXT", "SHORT", "LONG", "FLOAT", "DOUBLE", "DATE"]),
                required: true,
                default: "\"TEXT\"",
                help: "",
            },
            P { name: "field_length", label: "Field Length", kind: K::Number, required: false, default: "", help: "Text fields." },
            P { name: "field_precision", label: "Field Precision", kind: K::Number, required: false, default: "", help: "" },
            P { name: "field_scale", label: "Field Scale", kind: K::Number, required: false, default: "", help: "" },
        ],
        run: add_field,
    },
    Def {
        id: "delete_field",
        name: "Delete Field",
        toolbox: "GeoForge › Data Management › Fields",
        summary: "Removes fields from the layer, in place (ArcGIS Pro Delete Field).",
        params: &[
            INPUT,
            P { name: "fields", label: "Fields", kind: K::Text, required: true, default: "", help: "Field names separated by semicolons." },
            P {
                name: "method",
                label: "Method",
                kind: K::Choice(&["DELETE_FIELDS", "KEEP_FIELDS"]),
                required: false,
                default: "\"DELETE_FIELDS\"",
                help: "",
            },
        ],
        run: delete_field,
    },
    Def {
        id: "calculate_field",
        name: "Calculate Field",
        toolbox: "GeoForge › Data Management › Fields",
        summary: "Fills a field from a Python expression using !FIELD! values, in place (ArcGIS Pro Calculate Field, PYTHON3).",
        params: &[
            INPUT,
            P { name: "field", label: "Field Name", kind: K::Text, required: true, default: "", help: "" },
            P { name: "expression", label: "Expression", kind: K::Text, required: true, default: "", help: "e.g. !POP! / !AREA! * 1000" },
            P {
                name: "code_block",
                label: "Code Block",
                kind: K::Text,
                required: false,
                default: "",
                help: "Python functions the expression uses.",
            },
            P {
                name: "field_type",
                label: "Field Type",
                kind: K::Choice(&["", "TEXT", "SHORT", "LONG", "FLOAT", "DOUBLE", "DATE"]),
                required: false,
                default: "",
                help: "Creates the field if it is missing.",
            },
        ],
        run: calculate_field,
    },
    Def {
        id: "delete_identical",
        name: "Delete Identical",
        toolbox: "GeoForge › Data Management › General",
        summary: "Removes features that repeat the values or shape of an earlier one, in place (ArcGIS Pro Delete Identical).",
        params: &[
            INPUT,
            P {
                name: "fields",
                label: "Fields",
                kind: K::Text,
                required: true,
                default: "",
                help: "Field names separated by semicolons; Shape compares geometries.",
            },
            P {
                name: "xy_tolerance",
                label: "XY Tolerance",
                kind: K::Number,
                required: false,
                default: "",
                help: "Degrees. Default 0.001.",
            },
        ],
        run: delete_identical,
    },
    Def {
        id: "find_identical",
        name: "Find Identical",
        toolbox: "GeoForge › Data Management › General",
        summary: "A table of features that share values or shape (ArcGIS Pro Find Identical).",
        params: &[
            INPUT,
            P {
                name: "fields",
                label: "Fields",
                kind: K::Text,
                required: true,
                default: "",
                help: "Field names separated by semicolons; Shape compares geometries.",
            },
            P {
                name: "only_duplicates",
                label: "Output only duplicate records",
                kind: K::Bool,
                required: false,
                default: "false",
                help: "",
            },
            OUTPUT,
        ],
        run: find_identical,
    },
    Def {
        id: "generate_near_table",
        name: "Generate Near Table",
        toolbox: "GeoForge › Analysis › Proximity",
        summary: "A table of the nearest features and their distances in metres (ArcGIS Pro Generate Near Table).",
        params: &[
            INPUT,
            P { name: "near", label: "Near Features", kind: K::Layer, required: true, default: "", help: "" },
            P { name: "search_radius", label: "Search Radius", kind: K::Number, required: false, default: "", help: "" },
            UNITS,
            P {
                name: "method",
                label: "Closest or all",
                kind: K::Choice(&["CLOSEST", "ALL"]),
                required: false,
                default: "\"CLOSEST\"",
                help: "",
            },
            P { name: "closest_count", label: "Maximum closest features", kind: K::Number, required: false, default: "1", help: "" },
            P { name: "angle", label: "Add NEAR_ANGLE", kind: K::Bool, required: false, default: "false", help: "" },
            OUTPUT,
        ],
        run: generate_near_table,
    },
];

fn specs() -> Vec<ToolSpec> {
    DEFS.iter()
        .chain(raster::DEFS)
        .chain(extra::DEFS)
        .map(|d| ToolSpec {
            // Ids are `geoforge:<id>`; leaked once, like the other suites' specs.
            id: Box::leak(format!("{PREFIX}{}", d.id).into_boxed_str()),
            name: d.name,
            toolbox: d.toolbox,
            summary: d.summary,
            params: raster::params_of(d),
            run: rustgis_analysis::external_placeholder,
        })
        .collect()
}

fn run(spec: &ToolSpec, inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let id = spec.id.strip_prefix(PREFIX).unwrap_or(spec.id);
    let def = DEFS.iter().chain(raster::DEFS).chain(extra::DEFS).find(|d| d.id == id).ok_or_else(|| format!("no tool '{}'", spec.id))?;
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

    #[test]
    fn overlay_and_fuzzy_tools_equal_the_engine() {
        use geoforge_raster::overlay::*;
        register();
        let (a, mut inp) = dem("R1");
        let (_, second) = dem("R2");
        inp.rasters.extend(second.rasters);
        // Two copies of the cone: the sum is twice the cone, the mean is the cone.
        let sum = raster_of(rustgis_analysis::run("geoforge:cell_statistics", &inp, &json!({"inputs": "R1;R2", "statistic": "SUM"})));
        let want = cell_statistics(&[&a, &a], CellStat::Sum, true).unwrap();
        assert_eq!((sum.width, sum.height), (want.width, want.height));
        assert_eq!(sum.bands[0], want.bands[0]);
        let ws = raster_of(rustgis_analysis::run("geoforge:weighted_sum", &inp, &json!({"inputs": "R1;R2", "weights": "0.5;0.25"})));
        assert!((ws.get(12, 12).unwrap() - 0.75 * a.get(12, 12).unwrap()).abs() < 1e-3);
        let hp = raster_of(rustgis_analysis::run("geoforge:highest_position", &inp, &json!({"inputs": "R1;R2"})));
        assert_eq!(hp.get(12, 12), Some(1.0), "ties go to the first raster");
        let fz = raster_of(rustgis_analysis::run(
            "geoforge:fuzzy_membership",
            &inp,
            &json!({"input": "R1", "function": "LINEAR", "minimum": 0, "maximum": 100}),
        ));
        assert!((fz.get(12, 12).unwrap() - a.get(12, 12).unwrap() / 100.0).abs() < 1e-3);
        let fo = raster_of(rustgis_analysis::run(
            "geoforge:fuzzy_overlay",
            &inp,
            &json!({"inputs": "R1;R2", "overlay_type": "GAMMA", "gamma": 0.5}),
        ));
        assert_eq!((fo.width, fo.height), (24, 24));
        let wo = raster_of(rustgis_analysis::run("geoforge:weighted_overlay", &inp, &json!({"table": "R1:60:0=1;R2:40:0=1"})));
        // No cell value is in the remap: all NoData.
        assert!(wo.bands[0].iter().all(|v| wo.is_nodata(*v)));
        // The extent environment applies.
        let small = raster_of(rustgis_analysis::run(
            "geoforge:cell_statistics",
            &inp,
            &json!({"inputs": "R1;R2", "statistic": "MAXIMUM", "extent": "500060 5000060 500300 5000300"}),
        ));
        assert_eq!((small.width, small.height), (8, 8));
        for bad in [
            json!({"inputs": "R1;nope"}),
            json!({"inputs": ""}),
            json!({"inputs": "R1;R2", "statistic": "AVERAGE"}),
            json!({"inputs": "R1;R2", "ignore_nodata": "MAYBE"}),
        ] {
            assert!(rustgis_analysis::run("geoforge:cell_statistics", &inp, &bad).is_err(), "{bad}");
        }
        assert!(rustgis_analysis::run("geoforge:weighted_sum", &inp, &json!({"inputs": "R1;R2", "weights": "1"})).is_err());
        assert!(rustgis_analysis::run("geoforge:weighted_overlay", &inp, &json!({"table": "R1:50:1=1"})).is_err());
        assert!(rustgis_analysis::run("geoforge:fuzzy_overlay", &inp, &json!({"inputs": "R1;R2", "overlay_type": "GAMMA"})).is_err());
        assert!(rustgis_analysis::run("geoforge:fuzzy_membership", &inp, &json!({"input": "R1", "function": "LINEAR"})).is_err());
    }

    #[test]
    fn idw_interpolates_points_to_a_raster() {
        register();
        // Four corners of a square, values 0..3: the middle cell is their mean.
        let layers = Layers(vec![points("p", &[(0.0, 0.0), (2.0, 0.0), (0.0, 2.0), (2.0, 2.0)])]);
        let out = raster_of(rustgis_analysis::run("geoforge:idw", &layers, &json!({"input": "p", "z_field": "N", "cell_size": 1})));
        // Cell centers 0, 1, 2 from the points' lower-left corner: 3 x 3 cells.
        assert_eq!((out.width, out.height), (3, 3));
        assert!((out.get(1, 1).unwrap() - 1.5).abs() < 1e-6);
        // A point on a cell center gives its value (row 2 is the southern row).
        assert_eq!(out.get(0, 2), Some(0.0));
        assert_eq!(out.get(2, 0), Some(3.0));
        // A fixed radius that reaches no point leaves NoData.
        let fixed = raster_of(rustgis_analysis::run(
            "geoforge:idw",
            &layers,
            &json!({"input": "p", "z_field": "N", "cell_size": 1, "search_type": "FIXED", "fixed_distance": 0.5}),
        ));
        assert_eq!(fixed.get(1, 1), None);
        assert!(rustgis_analysis::run("geoforge:idw", &layers, &json!({"input": "p", "z_field": "N", "power": -1})).is_err());
        assert!(rustgis_analysis::run("geoforge:idw", &layers, &json!({"input": "p", "z_field": "nope"})).is_err());
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
    fn wide_layers_are_buffered_geodesically_and_near_the_antimeridian_by_zone() {
        register();
        // Three continents: one UTM zone would put two of these far from its central meridian.
        let spots = [(151.2, -33.9), (18.9, 69.6), (-118.2, 34.0), (151.3, -33.8)];
        let layers = Layers(vec![points("p", &spots)]);
        let r = rustgis_analysis::run("geoforge:buffer", &layers, &json!({"input": "p", "distance": 1000})).unwrap();
        // Geodesic: exact on the ellipsoid wherever the layer is, whatever its width.
        assert!(!r.messages.iter().any(|m| m.contains("UTM")), "{:?}", r.messages);
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
    fn near_measures_metres_on_the_ellipsoid_and_updates_in_place() {
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
    fn near_and_spatial_join_are_geodesic_across_wide_areas() {
        register();
        // Far apart and far north: one UTM zone could not measure this.
        let layers = Layers(vec![points("a", &[(-120.0, 70.0)]), points("b", &[(-100.0, 71.0), (60.0, 80.0)])]);
        // ArcGIS Near (GEODESIC) on the same points.
        let want = 750_149.660_147_738_2;
        let r = rustgis_analysis::run("geoforge:near", &layers, &json!({"input": "a", "near": "b", "angle": true})).unwrap();
        let Output::Update { dataset, .. } = r.output else { panic!("update") };
        let at = |n: &str| dataset.features[0].attrs[dataset.field_index(n).unwrap()].clone();
        assert!(matches!(at("NEAR_DIST"), Value::Float(d) if (d - want).abs() < 0.01), "{:?} vs {want}", at("NEAR_DIST"));
        // ArcGIS: azimuth 72.1355 degrees clockwise from north.
        assert!(matches!(at("NEAR_ANGLE"), Value::Float(a) if (a - 72.13552930038288).abs() < 1e-6), "{:?}", at("NEAR_ANGLE"));
        let j = rustgis_analysis::run(
            "geoforge:spatial_join",
            &layers,
            &json!({"input": "a", "other": "b", "match_option": "CLOSEST", "distance_field_name": "D"}),
        )
        .unwrap();
        let Output::Layer(out) = j.output else { panic!("layer") };
        assert!(matches!(out.features[0].attrs[out.field_index("D").unwrap()], Value::Float(d) if (d - want).abs() < 0.01));
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
    fn field_tools_work_in_place_and_others_make_layers() {
        register();
        let layers = Layers(vec![points("p", &[(10.0, 50.0), (11.0, 51.0), (10.0, 50.0), (12.0, 52.0)]), square("a", 0.0, 0.0, 1.0)]);
        let update = |tool: &str, params: Json| match rustgis_analysis::run(tool, &layers, &params).unwrap().output {
            Output::Update { layer, dataset } => {
                assert_eq!(layer, "p");
                dataset
            }
            _ => panic!("expected an in-place update"),
        };
        let d = update("geoforge:add_field", json!({"input": "p", "field_name": "dbl", "field_type": "DOUBLE"}));
        assert_eq!(d.fields.last().unwrap().name, "dbl");
        let d = update("geoforge:calculate_field", json!({"input": "p", "field": "N", "expression": "!N! * 10 + 1"}));
        assert_eq!(d.features.iter().map(|f| f.attrs[0].clone()).collect::<Vec<_>>(), [1, 11, 21, 31].map(Value::Int));
        let d = update("geoforge:add_xy", json!({"input": "p"}));
        let names: Vec<&str> = d.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["N", "POINT_X", "POINT_Y"]);
        let d = update("geoforge:delete_identical", json!({"input": "p", "fields": "Shape"}));
        assert_eq!(d.features.len(), 3, "the repeated location is gone");
        let d = update("geoforge:delete_field", json!({"input": "p", "fields": "N", "method": "KEEP_FIELDS"}));
        assert_eq!(d.fields.len(), 1);
        // Sort puts the largest first; Merge adds the layers up; Find Identical lists the repeats.
        let Output::Layer(sorted) =
            rustgis_analysis::run("geoforge:sort", &layers, &json!({"input": "p", "sort_fields": "N DESCENDING"})).unwrap().output
        else {
            panic!("layer")
        };
        assert_eq!(sorted.features.iter().map(|f| f.attrs[0].clone()).collect::<Vec<_>>(), [3, 2, 1, 0].map(Value::Int));
        let Output::Layer(m) = rustgis_analysis::run("geoforge:merge", &layers, &json!({"inputs": "p;p"})).unwrap().output else {
            panic!("layer")
        };
        assert_eq!(m.features.len(), 8);
        let Output::Table(t) =
            rustgis_analysis::run("geoforge:find_identical", &layers, &json!({"input": "p", "fields": "Shape", "only_duplicates": true}))
                .unwrap()
                .output
        else {
            panic!("table")
        };
        assert_eq!(t.features.len(), 2);
        let Output::Layer(v) =
            rustgis_analysis::run("geoforge:feature_vertices_to_points", &layers, &json!({"input": "a", "point_location": "ALL"}))
                .unwrap()
                .output
        else {
            panic!("layer")
        };
        assert_eq!(v.geometry_type, GeometryType::Point);
        assert!(v.features.len() >= 4);
    }

    /// A raster layer on disk, as the app holds one.
    struct WithRasters {
        layers: Layers,
        rasters: Vec<(String, rustgis_data::raster::Raster, String)>,
    }

    impl Inputs for WithRasters {
        fn dataset(&self, layer: &str) -> Option<&Dataset> {
            self.layers.dataset(layer)
        }
        fn selection(&self, _: &str) -> &[usize] {
            &[]
        }
        fn raster(&self, layer: &str) -> Option<rustgis_analysis::RasterInput<'_>> {
            self.rasters.iter().find(|(n, ..)| n == layer).map(|(_, r, path)| rustgis_analysis::RasterInput {
                raster: r,
                path: Some(path.as_str()),
                embedded: None,
            })
        }
    }

    /// A cone-shaped DEM in UTM 33N, written as a GeoTIFF and read back by the app.
    fn dem(name: &str) -> (geoforge_raster::Raster, WithRasters) {
        let mut r = geoforge_raster::Raster::new(24, 24, geoforge_raster::DataType::F32, [500_000.0, 30.0, 0.0, 5_000_720.0, 0.0, -30.0]);
        r.epsg = Some(32633);
        r.nodata = Some(-9999.0);
        for row in 0..24 {
            for col in 0..24 {
                let d = (((col as f64 - 11.5).powi(2) + (row as f64 - 11.5).powi(2)).sqrt()) * 4.0;
                r.bands[0][row * 24 + col] = (100.0 - d) as f32 as f64;
            }
        }
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "gf-bridge-{name}-{}-{}.tif",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        geoforge_raster::write_geotiff(&path, &r, geoforge_raster::Compression::None).unwrap();
        let app = rustgis_data::raster::read_path(&path).unwrap();
        (r, WithRasters { layers: Layers(vec![]), rasters: vec![(name.to_string(), app, path.display().to_string())] })
    }

    fn raster_of(r: Result<ToolResult, String>) -> geoforge_raster::Raster {
        let Output::Layers(mut v) = r.unwrap().output else { panic!("layers") };
        let rustgis_analysis::OutputLayer::Raster { bytes, .. } = v.remove(0) else { panic!("raster") };
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "gf-bridge-out-{}-{}.tif",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        let out = geoforge_raster::read_geotiff(&path).unwrap();
        std::fs::remove_file(&path).ok();
        out
    }

    #[test]
    fn raster_tools_equal_the_engine() {
        register();
        let (dem_gf, inp) = dem("DEM");
        // Slope: the cells the engine computes, with the input's grid and coordinate system.
        let out = raster_of(rustgis_analysis::run(
            "geoforge:slope",
            &inp,
            &json!({"input": "DEM", "output_measurement": "PERCENT_RISE", "z_factor": 2}),
        ));
        let want = geoforge_raster::surface::slope(&dem_gf, geoforge_raster::surface::SlopeUnit::PercentRise, 2.0);
        assert_eq!((out.width, out.height, out.epsg), (24, 24, Some(32633)));
        assert_eq!(out.bands[0], want.bands[0]);
        assert_eq!(out.transform, want.transform);
        // Local math with a constant, and a Con on the result.
        let plus = raster_of(rustgis_analysis::run("geoforge:plus", &inp, &json!({"in1": "DEM", "in2": "10"})));
        assert!((plus.bands[0][12 * 24 + 12] - (dem_gf.bands[0][12 * 24 + 12] + 10.0)).abs() < 1e-4);
        // Focal statistics, reclassify and resample run and keep the grid or change it as asked.
        let focal = raster_of(rustgis_analysis::run(
            "geoforge:focal_statistics",
            &inp,
            &json!({"input": "DEM", "neighborhood": "Rectangle 3 3 CELL", "statistics_type": "MAXIMUM"}),
        ));
        assert_eq!((focal.width, focal.height), (24, 24));
        let rs = raster_of(rustgis_analysis::run(
            "geoforge:resample",
            &inp,
            &json!({"input": "DEM", "cell_size": 60, "resampling_type": "BILINEAR"}),
        ));
        assert_eq!((rs.width, rs.height), (12, 12));
        let reclass =
            raster_of(rustgis_analysis::run("geoforge:reclassify", &inp, &json!({"input": "DEM", "remap": "-100 40 1;40 101 2"})));
        assert!(reclass.bands[0].iter().all(|v| [1.0, 2.0].contains(v) || *v == reclass.nodata.unwrap_or(f64::NAN)));
        // Hillshade with shadows and the hydrology chain.
        let hs = raster_of(rustgis_analysis::run("geoforge:hillshade", &inp, &json!({"input": "DEM", "model_shadows": true})));
        assert_eq!(hs.width, 24);
        let filled = rustgis_analysis::run("geoforge:fill", &inp, &json!({"input": "DEM", "output": "Filled"})).unwrap();
        assert!(filled.messages[0].starts_with("Filled"));
    }

    #[test]
    fn raster_environments_equal_the_engine() {
        use geoforge_raster::env::{self, Environment, Reads, Sampling};
        register();
        let (dem_gf, inp) = dem("DEMENV");
        // A coarser cell size and a shifted extent: Slope reads one cell of context, bilinearly.
        let out = raster_of(rustgis_analysis::run(
            "geoforge:slope",
            &inp,
            &json!({"input": "DEMENV", "cell_size": 60, "extent": "500100 5000100 500500 5000500"}),
        ));
        let e = Environment { cell_size: Some(60.0), extent: Some([500_100.0, 5_000_100.0, 500_500.0, 5_000_500.0]), ..Default::default() };
        let want = env::run(&[&dem_gf], &e, Reads::Context, Sampling::Bilinear, &|r| {
            geoforge_raster::surface::slope(&r[0], geoforge_raster::surface::SlopeUnit::Degree, 1.0)
        })
        .unwrap();
        assert_eq!((out.width, out.height, out.transform), (want.width, want.height, want.transform));
        assert_eq!(out.bands[0], want.bands[0]);
        // Local math honours the extent too, and a raster mask turns cells to NoData.
        let plus = raster_of(rustgis_analysis::run(
            "geoforge:plus",
            &inp,
            &json!({"in1": "DEMENV", "in2": "1", "extent": "500060 5000060 500300 5000300"}),
        ));
        assert_eq!((plus.width, plus.height), (8, 8));
        let masked =
            raster_of(rustgis_analysis::run("geoforge:fill", &inp, &json!({"input": "DEMENV", "mask": "DEMENV", "cell_size": 30})));
        assert_eq!(masked.width, 24);
        // Bad environment values are errors, not panics.
        for bad in [
            json!({"cell_size": -1}),
            json!({"cell_size": 0}),
            json!({"extent": "1 2 3"}),
            json!({"extent": "a b c d"}),
            json!({"snap_raster": "nope"}),
            json!({"mask": "nope"}),
            json!({"extent": "5 5 1 1"}),
        ] {
            let mut p = json!({"input": "DEMENV"});
            p.as_object_mut().unwrap().extend(bad.as_object().unwrap().clone());
            assert!(rustgis_analysis::run("geoforge:slope", &inp, &p).is_err(), "{p}");
        }
    }

    #[test]
    fn raster_vector_results_are_in_longitude_latitude() {
        register();
        let (_, inp) = dem("DEM");
        let Output::Layer(c) =
            rustgis_analysis::run("geoforge:contour", &inp, &json!({"input": "DEM", "contour_interval": 20, "base_contour": 0}))
                .unwrap()
                .output
        else {
            panic!("layer")
        };
        assert!(c.features.len() >= 3, "{}", c.features.len());
        assert_eq!(c.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Id", "Contour"]);
        for f in &c.features {
            let Some(Geometry::Line(parts)) = &f.geometry else { panic!("line") };
            // UTM 33N, 500 000 E 5 000 000 N is 15 E, about 45.1 N.
            for pt in parts.iter().flatten() {
                assert!((14.0..16.0).contains(&pt.x) && (44.9..45.3).contains(&pt.y), "{pt:?}");
            }
        }
        let Output::Layer(pts) = rustgis_analysis::run("geoforge:raster_to_point", &inp, &json!({"input": "DEM"})).unwrap().output else {
            panic!("layer")
        };
        assert_eq!(pts.features.len(), 24 * 24);
        let fields: Vec<&str> = pts.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(fields, ["pointid", "grid_code"]);
    }

    #[test]
    fn a_buffer_over_the_antimeridian_falls_back_to_the_zone() {
        register();
        let layers = Layers(vec![points("p", &[(179.9999, 10.0)])]);
        let r = rustgis_analysis::run("geoforge:buffer", &layers, &json!({"input": "p", "distance": 5000})).unwrap();
        let Output::Layer(d) = r.output else { panic!("layer") };
        assert_eq!(d.features.len(), 1);
    }

    fn bowtie(name: &str) -> Dataset {
        let mut d = Dataset::new(name, GeometryType::Polygon, vec![Field::new("K", FieldType::Text)]);
        let ring = vec![
            Coord::new(10.0, 50.0),
            Coord::new(10.01, 50.01),
            Coord::new(10.01, 50.0),
            Coord::new(10.0, 50.01),
            Coord::new(10.0, 50.0),
        ];
        d.push(Some(Geometry::Polygon(vec![vec![ring]])), vec![Value::Text("a".into())]);
        d
    }

    #[test]
    fn geometry_tools_check_repair_split_and_measure() {
        register();
        let layers = Layers(vec![bowtie("bow"), square("sq", 10.0, 50.0, 0.01), square("cut", 10.0, 50.0, 0.005)]);
        // Check Geometry names the self-intersection; Repair Geometry (in place) rebuilds it.
        let Output::Table(t) = rustgis_analysis::run("geoforge:check_geometry", &layers, &json!({"input": "bow"})).unwrap().output else {
            panic!("table")
        };
        assert_eq!(t.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["CLASS", "FEATURE_ID", "PROBLEM"]);
        assert_eq!(t.features.len(), 1);
        let Output::Update { layer, dataset } =
            rustgis_analysis::run("geoforge:repair_geometry", &layers, &json!({"input": "bow"})).unwrap().output
        else {
            panic!("update")
        };
        assert_eq!((layer.as_str(), dataset.features.len()), ("bow", 1));
        // Add Geometry Attributes: the area is on the ground (about 0.01 deg x 0.01 deg at 50 N), the centroid is a coordinate.
        let Output::Update { dataset: g, .. } = rustgis_analysis::run(
            "geoforge:add_geometry_attributes",
            &layers,
            &json!({"input": "sq", "properties": "AREA;CENTROID", "area_unit": "SQUARE_KILOMETERS"}),
        )
        .unwrap()
        .output
        else {
            panic!("update")
        };
        let at = |n: &str| g.features[0].attrs[g.field_index(n).unwrap()].as_f64().unwrap();
        let w = ground(Coord::new(10.0, 50.0), Coord::new(10.01, 50.0));
        let h = ground(Coord::new(10.0, 50.0), Coord::new(10.0, 50.01));
        assert!((at("POLY_AREA") - w * h / 1e6).abs() / (w * h / 1e6) < 0.01, "{} vs {}", at("POLY_AREA"), w * h / 1e6);
        assert!((at("CENTROID_X") - 10.005).abs() < 1e-9 && (at("CENTROID_Y") - 50.005).abs() < 1e-9);
        // Split: the square cut by the smaller one, named by the text field's value.
        let Output::Layers(parts) =
            rustgis_analysis::run("geoforge:split", &layers, &json!({"input": "sq", "other": "cut", "split_field": "NAME"}))
                .unwrap()
                .output
        else {
            panic!("layers")
        };
        assert_eq!(parts.len(), 1);
        let rustgis_analysis::OutputLayer::Vector(d) = &parts[0] else { panic!("vector") };
        assert_eq!(d.name, "sq_cut");
        assert!((area(d) - 0.005 * 0.005).abs() < 1e-9);
    }

    #[test]
    fn vector_to_raster_and_zonal_table() {
        register();
        let layers = Layers(vec![square("sq", 10.0, 50.0, 0.01)]);
        let r = raster_of(rustgis_analysis::run("geoforge:polygon_to_raster", &layers, &json!({"input": "sq", "cell_size": 0.001})));
        assert_eq!((r.width, r.height, r.epsg), (10, 10, Some(4326)));
        assert!(r.bands[0].iter().all(|v| *v == 1.0));
        // A cell size that would need more cells than a raster may have is refused.
        let e = rustgis_analysis::run("geoforge:polygon_to_raster", &layers, &json!({"input": "sq", "cell_size": 1e-12})).unwrap_err();
        assert!(e.contains("cells"), "{e}");
        // Zonal Statistics as Table: two zones over the cone.
        let (_, inp) = dem("DEM");
        let zones_path = std::env::temp_dir().join(format!("gf-bridge-zones-{}.tif", std::process::id()));
        let mut z = geoforge_raster::Raster::new(24, 24, geoforge_raster::DataType::I16, [500_000.0, 30.0, 0.0, 5_000_720.0, 0.0, -30.0]);
        z.epsg = Some(32633);
        for row in 0..24 {
            for col in 0..24 {
                z.bands[0][row * 24 + col] = if col < 12 { 1.0 } else { 2.0 };
            }
        }
        geoforge_raster::write_geotiff(&zones_path, &z, geoforge_raster::Compression::None).unwrap();
        let mut inp = inp;
        inp.rasters.push(("Z".into(), rustgis_data::raster::read_path(&zones_path).unwrap(), zones_path.display().to_string()));
        let Output::Table(t) = rustgis_analysis::run(
            "geoforge:zonal_statistics_as_table",
            &inp,
            &json!({"zones": "Z", "input": "DEM", "statistics_type": "MEAN"}),
        )
        .unwrap()
        .output
        else {
            panic!("table")
        };
        assert_eq!(t.features.len(), 2);
        assert!(t.fields.iter().any(|f| f.name == "MEAN"));
        std::fs::remove_file(&zones_path).ok();
    }

    #[test]
    fn bad_parameters_are_errors_not_panics() {
        register();
        let layers = Layers(vec![square("a", 0.0, 0.0, 1.0), points("p", &[(10.0, 50.0), (10.0, 50.0)])]);
        let tools = rustgis_analysis::external_tools_of(SOURCE);
        assert!(tools.len() >= 25, "{} tools", tools.len());
        let hostile = [
            json!({}),
            json!({"input": "nope", "clip": 3, "other": null, "near": [], "inputs": {}}),
            json!({"input": "a", "clip": "a", "erase": "a", "other": "a", "near": "a", "fields": "NOPE", "inputs": "a;a", "field": "NOPE", "expression": "1/0 +"}),
            json!({"input": "p", "clip": "p", "other": "p", "near": "p", "inputs": "p;a;nope", "fields": "N;Shape;N", "field_name": "x y", "field_type": "BLOB",
                   "distance": -1e308, "search_radius": "NaN", "search_distance": 1e308, "units": "FURLONGS", "closest_count": -5, "sort_fields": "N SIDEWAYS;;",
                   "statistics": "N MODE", "where": "((", "expression": "!N! +", "code_block": "def f(", "point_location": "x", "match_option": "x"}),
            json!({"input": 5, "distance": "far", "field": {}, "fields": [1, 2], "method": null}),
        ];
        for t in tools {
            for p in &hostile {
                if let Err(e) = rustgis_analysis::run(t.id, &layers, p) {
                    // `run` turns a panic into this message: that is a bug in the tool.
                    assert!(!e.contains("failed unexpectedly"), "{} panicked on {p}", t.id);
                }
            }
        }
        // The raster tools against a real raster, with parameters that do not fit it.
        let (_, rinp) = dem("DEM");
        let raster_hostile = [
            json!({"input": "DEM"}),
            json!({"input": "DEM", "in1": "DEM", "in2": "DEM", "mask": "DEM", "zones": "DEM", "streams": "DEM", "weight": "DEM", "pour_points": "DEM", "true_value": "DEM", "false_value": "nan"}),
            json!({"input": "DEM", "cell_size": 0, "contour_interval": -1, "z_factor": "x", "remap": "1 2", "neighborhood": "Circle -3 CELL", "statistics_type": "MODE", "where_clause": "VALUE >>", "maximum_distance": -5}),
            json!({"input": "DEM", "cell_size": 1e-300, "contour_interval": 1e-9, "neighborhood": "Rectangle 99999 99999 CELL", "in1": "1e999", "in2": "0", "azimuth": "NaN", "altitude": 1e308}),
        ];
        for t in tools {
            for p in &raster_hostile {
                if let Err(e) = rustgis_analysis::run(t.id, &rinp, p) {
                    assert!(!e.contains("failed unexpectedly"), "{} panicked on {p}", t.id);
                }
            }
        }
    }
}
