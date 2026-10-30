//! GPX To Features (Conversion): GPX waypoints, tracks and routes as features.

use unigis_core::{Error, Feature, Field, FieldType, Geometry, GeometryKind, Result, Value};

use crate::ToolOutput;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

const WGS84: &str = "GEOGCS[\"GCS_WGS_1984\",DATUM[\"D_WGS_1984\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.017453292519943295]],VERTCS[\"WGS_1984\",DATUM[\"D_WGS_1984\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PARAMETER[\"Vertical_Shift\",0.0],PARAMETER[\"Direction\",1.0],UNIT[\"Meter\",1.0]]";

/// One output point before it becomes a feature.
struct Point {
    lon: f64,
    lat: f64,
    ele: f64,
    name: String,
    desc: String,
    kind: &'static str,
    cmt: String,
    sym: String,
    time: String,
}

fn child<'a>(n: roxmltree::Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == tag)
        .and_then(|c| c.text())
        .map(str::trim)
}

fn coord(n: roxmltree::Node, attr: &str) -> Option<f64> {
    n.attribute(attr)?.trim().parse().ok()
}

/// The points of a track or route: the container's name, and description, comment and symbol
/// that carry over from point to point once seen (the container's, then each point's), as
/// ArcGIS Pro 3.7 writes them.
fn container(c: roxmltree::Node, point_tag: &str, kind: &'static str, out: &mut Vec<Point>) {
    let name = child(c, "name").unwrap_or("").to_owned();
    let mut desc = child(c, "desc").unwrap_or("").to_owned();
    let mut cmt = child(c, "cmt").unwrap_or("").to_owned();
    let mut sym = child(c, "sym").unwrap_or("").to_owned();
    for p in c
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == point_tag)
    {
        let (Some(lon), Some(lat)) = (coord(p, "lon"), coord(p, "lat")) else {
            continue;
        };
        for (tag, value) in [("desc", &mut desc), ("cmt", &mut cmt), ("sym", &mut sym)] {
            if let Some(v) = child(p, tag) {
                *value = v.to_owned();
            }
        }
        out.push(Point {
            lon,
            lat,
            ele: child(p, "ele").and_then(|e| e.parse().ok()).unwrap_or(0.0),
            name: name.clone(),
            desc: desc.clone(),
            kind,
            cmt: cmt.clone(),
            sym: sym.clone(),
            time: child(p, "time").unwrap_or("").to_owned(),
        });
    }
}

fn parse_time(t: &str) -> Value {
    let t = t.trim().trim_end_matches('Z');
    chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S"))
        .map_or(Value::Null, Value::Date)
}

fn text_field(name: &str) -> Field {
    let mut f = Field::new(name, FieldType::String);
    f.length = 254;
    f
}

/// GPX To Features: with `lines` one single-part polyline per `<trk>` (with Name, Descript,
/// Type), else a point per track point, route point and waypoint, in that order,
/// with Name, Descript, Type (TRKPT/RTEPT/WPT), Comment, Symbol, DateTimeS (the `<time>` text)
/// and Elevation (missing: 0). Track and route points take the track's or route's name; their
/// description, comment and symbol carry over once seen. Geometry has Z (the elevation) and an
/// empty M. A shapefile output starts with ArcGIS's `Id` field (0) and has no DateTime field;
/// other outputs end with DateTime (observed against ArcGIS Pro 3.7).
pub fn gpx_to_features(text: &str, lines: bool, shapefile: bool) -> Result<ToolOutput> {
    let doc = roxmltree::Document::parse(text).map_err(|e| {
        err(format!(
            "ERROR 000732: Input GPX File is not valid GPX: {e}"
        ))
    })?;
    let root = doc.root_element();
    let elements = |tag: &'static str| {
        root.children()
            .filter(move |n| n.is_element() && n.tag_name().name() == tag)
    };
    let mut fields = Vec::new();
    if shapefile {
        let mut id = Field::new("Id", FieldType::Integer);
        id.length = 6;
        fields.push(id);
    }
    fields.extend([
        text_field("Name"),
        text_field("Descript"),
        text_field("Type"),
    ]);
    let mut features = Vec::new();
    if lines {
        for (i, trk) in elements("trk").enumerate() {
            let mut pts = Vec::new();
            container(trk, "trkpt", "TRKPT", &mut pts);
            if pts.len() < 2 {
                continue;
            }
            let first = &pts[0];
            let mut attributes = Vec::new();
            if shapefile {
                attributes.push(Value::Integer(0));
            }
            attributes.extend([
                Value::String(first.name.clone()),
                Value::String(first.desc.clone()),
                Value::String("TRKPT".into()),
            ]);
            features.push(Feature {
                oid: i as i64,
                geometry: Some(Geometry {
                    kind: GeometryKind::Polyline,
                    parts: vec![0],
                    part_types: None,
                    xy: pts.iter().map(|p| [p.lon, p.lat]).collect(),
                    z: Some(pts.iter().map(|p| p.ele).collect()),
                    m: Some(vec![f64::NAN; pts.len()]),
                    curves: Vec::new(),
                }),
                attributes,
            });
        }
        return Ok(ToolOutput {
            kind: GeometryKind::Polyline,
            has_z: true,
            has_m: true,
            fields,
            spatial_reference_wkt: Some(WGS84.to_owned()),
            features,
            messages: Vec::new(),
        });
    }
    fields.extend([
        text_field("Comment"),
        text_field("Symbol"),
        text_field("DateTimeS"),
        Field::new("Elevation", FieldType::Double),
    ]);
    if !shapefile {
        fields.push(Field::new("DateTime", FieldType::Date));
    }
    let mut pts = Vec::new();
    for trk in elements("trk") {
        container(trk, "trkpt", "TRKPT", &mut pts);
    }
    for rte in elements("rte") {
        container(rte, "rtept", "RTEPT", &mut pts);
    }
    for w in elements("wpt") {
        let (Some(lon), Some(lat)) = (coord(w, "lon"), coord(w, "lat")) else {
            continue;
        };
        let get = |tag: &str| child(w, tag).unwrap_or("").to_owned();
        pts.push(Point {
            lon,
            lat,
            ele: child(w, "ele").and_then(|e| e.parse().ok()).unwrap_or(0.0),
            name: get("name"),
            desc: get("desc"),
            kind: "WPT",
            cmt: get("cmt"),
            sym: get("sym"),
            time: get("time"),
        });
    }
    for (i, p) in pts.into_iter().enumerate() {
        let mut attributes = Vec::new();
        if shapefile {
            attributes.push(Value::Integer(0));
        }
        let time = parse_time(&p.time);
        attributes.extend([
            Value::String(p.name),
            Value::String(p.desc),
            Value::String(p.kind.into()),
            Value::String(p.cmt),
            Value::String(p.sym),
            Value::String(p.time),
            Value::Double(p.ele),
        ]);
        if !shapefile {
            attributes.push(time);
        }
        let mut g = Geometry::point(p.lon, p.lat);
        g.z = Some(vec![p.ele]);
        g.m = Some(vec![f64::NAN]);
        features.push(Feature {
            oid: i as i64,
            geometry: Some(g),
            attributes,
        });
    }
    Ok(ToolOutput {
        kind: GeometryKind::Point,
        has_z: true,
        has_m: true,
        fields,
        spatial_reference_wkt: Some(WGS84.to_owned()),
        features,
        messages: Vec::new(),
    })
}
