//! Esri JSON and GeoJSON output, text for text as ArcGIS Pro's Features To JSON
//! writes it (checked against its output in `testdata/arcgis-reference`):
//!
//! - compact JSON; "formatted" output indents the header (`"key" : value`, two
//!   spaces) but keeps the features array on one line;
//! - coordinates and doubles of fields without a scale as C's `%.17g`; doubles
//!   of fields with a scale (dBASE `N` with decimals) as the shortest
//!   round-trip form, with `.0` for whole numbers;
//! - Esri JSON keeps rings as stored (exterior clockwise); GeoJSON writes them
//!   reversed (exterior counter-clockwise) and adds the OID as `id`;
//! - one-point line parts are left out; a missing shape is an empty geometry.

use unigis_core::{Feature, Field, FieldType, Geometry, GeometryKind, Value};

/// What is written about a feature class.
pub struct Dataset<'a> {
    pub kind: GeometryKind,
    pub oid_name: &'a str,
    pub fields: &'a [Field],
    pub features: &'a [Feature],
    /// Esri `wkid` and `latestWkid`, if the coordinate system has a code.
    pub wkid: Option<(u32, u32)>,
    /// Write Z / M (when the data has them).
    pub z: bool,
    pub m: bool,
}

/// C's `%.17g` without trailing zeros.
pub fn g17(x: f64) -> String {
    if x.is_nan() {
        return "\"NaN\"".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let sci = format!("{x:.16e}");
    let (mantissa, exp) = sci.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if (-4..17).contains(&exp) {
        if exp < 0 {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-exp - 1) as usize));
            out.push_str(&digits);
        } else {
            let int = (exp + 1) as usize;
            out.push_str(&digits[..int]);
            out.push('.');
            out.push_str(&digits[int..]);
        }
        let trimmed = out.trim_end_matches('0').trim_end_matches('.').to_owned();
        return trimmed;
    }
    let frac = digits[1..].trim_end_matches('0');
    out.push_str(&digits[..1]);
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    out.push_str(&format!(
        "e{}{:02}",
        if exp < 0 { '-' } else { '+' },
        exp.abs()
    ));
    out
}

/// The shortest round-trip form, `.0` for whole numbers.
fn shortest(x: f64) -> String {
    if x.is_finite() {
        format!("{x:?}")
    } else {
        "\"NaN\"".into()
    }
}

fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn value(v: &Value, field: &Field) -> String {
    match v {
        Value::Null => "null".into(),
        Value::SmallInteger(n) => n.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::BigInteger(n) => n.to_string(),
        Value::Single(x) if field.scale > 0 => shortest(f64::from(*x)),
        Value::Single(x) => g17(f64::from(*x)),
        Value::Double(x) if field.scale > 0 => shortest(*x),
        Value::Double(x) => g17(*x),
        Value::String(s) | Value::Guid(s) | Value::Xml(s) => string(s),
        Value::Date(d) => d.and_utc().timestamp_millis().to_string(),
        Value::DateOnly(d) => d.and_hms_opt(0, 0, 0).map_or("null".into(), |d| {
            d.and_utc().timestamp_millis().to_string()
        }),
        Value::TimeOnly(t) => string(&t.to_string()),
        Value::TimestampOffset(t) => t.timestamp_millis().to_string(),
        Value::Blob(_) => "null".into(),
    }
}

fn esri_type(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "esriFieldTypeSmallInteger",
        FieldType::Integer => "esriFieldTypeInteger",
        FieldType::BigInteger => "esriFieldTypeBigInteger",
        FieldType::Single => "esriFieldTypeSingle",
        FieldType::Double => "esriFieldTypeDouble",
        FieldType::String => "esriFieldTypeString",
        FieldType::Date => "esriFieldTypeDate",
        FieldType::DateOnly => "esriFieldTypeDateOnly",
        FieldType::TimeOnly => "esriFieldTypeTimeOnly",
        FieldType::TimestampOffset => "esriFieldTypeTimestampOffset",
        FieldType::ObjectId => "esriFieldTypeOID",
        FieldType::Geometry => "esriFieldTypeGeometry",
        FieldType::Blob => "esriFieldTypeBlob",
        FieldType::Raster => "esriFieldTypeRaster",
        FieldType::Guid => "esriFieldTypeGUID",
        FieldType::GlobalId => "esriFieldTypeGlobalID",
        FieldType::Xml => "esriFieldTypeXML",
    }
}

fn geometry_type(kind: GeometryKind) -> &'static str {
    match kind {
        GeometryKind::Point => "esriGeometryPoint",
        GeometryKind::Multipoint => "esriGeometryMultipoint",
        GeometryKind::Polyline => "esriGeometryPolyline",
        GeometryKind::Polygon => "esriGeometryPolygon",
        GeometryKind::MultiPatch => "esriGeometryMultiPatch",
    }
}

/// A coordinate tuple with the requested Z/M.
fn coord(g: &Geometry, i: usize, z: bool, m: bool) -> String {
    let mut parts = vec![g17(g.xy[i][0]), g17(g.xy[i][1])];
    if z {
        parts.push(g.z.as_ref().map_or("0".into(), |v| g17(v[i])));
    }
    if m {
        parts.push(g.m.as_ref().map_or("\"NaN\"".into(), |v| g17(v[i])));
    }
    format!("[{}]", parts.join(","))
}

/// A line's parts with two or more points (ArcGIS leaves out one-point parts).
fn paths(g: &Geometry, line: bool) -> Vec<std::ops::Range<usize>> {
    g.part_ranges()
        .filter(|r| !line || r.len() >= 2)
        .filter(|r| !r.is_empty())
        .collect()
}

fn esri_geometry(g: Option<&Geometry>, kind: GeometryKind, z: bool, m: bool) -> String {
    let flags = format!(
        "{}{}",
        if z { "\"hasZ\":true," } else { "" },
        if m { "\"hasM\":true," } else { "" }
    );
    let empty = Geometry {
        kind,
        parts: Vec::new(),
        part_types: None,
        xy: Vec::new(),
        z: None,
        m: None,
        curves: Vec::new(),
    };
    let g = g.unwrap_or(&empty);
    match kind {
        GeometryKind::Point => match g.xy.first() {
            Some(p) => {
                let mut s = format!("{{{flags}\"x\":{},\"y\":{}", g17(p[0]), g17(p[1]));
                if z {
                    s.push_str(&format!(
                        ",\"z\":{}",
                        g.z.as_ref().map_or("0".into(), |v| g17(v[0]))
                    ));
                }
                if m {
                    s.push_str(&format!(
                        ",\"m\":{}",
                        g.m.as_ref().map_or("\"NaN\"".into(), |v| g17(v[0]))
                    ));
                }
                s.push('}');
                s
            }
            None => format!("{{{flags}\"x\":\"NaN\",\"y\":\"NaN\"}}"),
        },
        GeometryKind::Multipoint => {
            let pts: Vec<String> = (0..g.xy.len()).map(|i| coord(g, i, z, m)).collect();
            format!("{{{flags}\"points\":[{}]}}", pts.join(","))
        }
        _ => {
            let key = if kind == GeometryKind::Polygon {
                "rings"
            } else {
                "paths"
            };
            let parts: Vec<String> = paths(g, kind == GeometryKind::Polyline)
                .into_iter()
                .map(|r| {
                    let c: Vec<String> = r.map(|i| coord(g, i, z, m)).collect();
                    format!("[{}]", c.join(","))
                })
                .collect();
            format!("{{{flags}\"{key}\":[{}]}}", parts.join(","))
        }
    }
}

/// A pretty-printed JSON fragment: "key" : value pairs, two-space indents.
enum Node {
    Raw(String),
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
}

fn render(n: &Node, pretty: bool, indent: usize) -> String {
    let pad = |k: usize| " ".repeat(k);
    match n {
        Node::Raw(s) => s.clone(),
        Node::Object(items) if pretty => {
            let inner: Vec<String> = items
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}{} : {}",
                        pad(indent + 2),
                        string(k),
                        render(v, true, indent + 2)
                    )
                })
                .collect();
            format!("{{\n{}\n{}}}", inner.join(",\n"), pad(indent))
        }
        Node::Object(items) => {
            let inner: Vec<String> = items
                .iter()
                .map(|(k, v)| format!("{}:{}", string(k), render(v, false, 0)))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Node::Array(items) if pretty => {
            let inner: Vec<String> = items
                .iter()
                .map(|v| format!("{}{}", pad(indent + 2), render(v, true, indent + 2)))
                .collect();
            format!("[\n{}\n{}]", inner.join(",\n"), pad(indent))
        }
        Node::Array(items) => {
            let inner: Vec<String> = items.iter().map(|v| render(v, false, 0)).collect();
            format!("[{}]", inner.join(","))
        }
    }
}

fn raw(s: impl Into<String>) -> Node {
    Node::Raw(s.into())
}

/// Esri JSON (ArcGIS `FeaturesToJSON` with `NO_GEOJSON`).
pub fn esri_json(d: &Dataset, formatted: bool) -> String {
    let z = d.z;
    let m = d.m;
    let mut header: Vec<(String, Node)> = vec![("displayFieldName".into(), raw("\"\""))];
    if z {
        header.push(("hasZ".into(), raw("true")));
    }
    if m {
        header.push(("hasM".into(), raw("true")));
    }
    let mut aliases = vec![(d.oid_name.to_owned(), raw(string(d.oid_name)))];
    aliases.extend(d.fields.iter().map(|f| {
        (
            f.name.clone(),
            raw(string(f.alias.as_deref().unwrap_or(&f.name))),
        )
    }));
    header.push(("fieldAliases".into(), Node::Object(aliases)));
    header.push(("geometryType".into(), raw(string(geometry_type(d.kind)))));
    if let Some((wkid, latest)) = d.wkid {
        header.push((
            "spatialReference".into(),
            Node::Object(vec![
                ("wkid".into(), raw(wkid.to_string())),
                ("latestWkid".into(), raw(latest.to_string())),
            ]),
        ));
    }
    let mut fields = vec![Node::Object(vec![
        ("name".into(), raw(string(d.oid_name))),
        ("type".into(), raw(string("esriFieldTypeOID"))),
        ("alias".into(), raw(string(d.oid_name))),
    ])];
    for f in d.fields {
        let mut items = vec![
            ("name".into(), raw(string(&f.name))),
            ("type".into(), raw(string(esri_type(f.field_type)))),
            (
                "alias".into(),
                raw(string(f.alias.as_deref().unwrap_or(&f.name))),
            ),
        ];
        if f.field_type == FieldType::String {
            items.push(("length".into(), raw(f.length.to_string())));
        }
        fields.push(Node::Object(items));
    }
    header.push(("fields".into(), Node::Array(fields)));
    let features: Vec<String> = d
        .features
        .iter()
        .map(|f| {
            let mut attrs = vec![format!("{}:{}", string(d.oid_name), f.oid)];
            attrs.extend(
                d.fields
                    .iter()
                    .zip(&f.attributes)
                    .map(|(fd, v)| format!("{}:{}", string(&fd.name), value(v, fd))),
            );
            format!(
                "{{\"attributes\":{{{}}},\"geometry\":{}}}",
                attrs.join(","),
                esri_geometry(f.geometry.as_ref(), d.kind, z, m)
            )
        })
        .collect();
    // The features array stays on one line even when formatted.
    header.push(("features".into(), raw(format!("[{}]", features.join(",")))));
    render(&Node::Object(header), formatted, 0)
}

fn geojson_geometry(g: Option<&Geometry>, kind: GeometryKind, z: bool, m: bool) -> String {
    let Some(g) = g.filter(|g| !g.xy.is_empty()) else {
        return "null".into();
    };
    let ring = |r: std::ops::Range<usize>, reverse: bool| {
        let mut c: Vec<String> = r.map(|i| coord(g, i, z, m)).collect();
        if reverse {
            c.reverse();
        }
        format!("[{}]", c.join(","))
    };
    match kind {
        GeometryKind::Point => format!(
            "{{\"type\":\"Point\",\"coordinates\":{}}}",
            coord(g, 0, z, m)
        ),
        GeometryKind::Multipoint => {
            let pts: Vec<String> = (0..g.xy.len()).map(|i| coord(g, i, z, m)).collect();
            format!(
                "{{\"type\":\"MultiPoint\",\"coordinates\":[{}]}}",
                pts.join(",")
            )
        }
        GeometryKind::Polyline => {
            let parts: Vec<String> = paths(g, true).into_iter().map(|r| ring(r, false)).collect();
            if parts.len() == 1 {
                format!("{{\"type\":\"LineString\",\"coordinates\":{}}}", parts[0])
            } else {
                format!(
                    "{{\"type\":\"MultiLineString\",\"coordinates\":[{}]}}",
                    parts.join(",")
                )
            }
        }
        _ => {
            // Exterior rings (clockwise in Esri order) start polygons; holes follow.
            let mut polygons: Vec<Vec<String>> = Vec::new();
            for r in paths(g, false) {
                let pts = &g.xy[r.clone()];
                let area: f64 = pts
                    .windows(2)
                    .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
                    .sum();
                if area <= 0.0 || polygons.is_empty() {
                    polygons.push(vec![ring(r, true)]);
                } else {
                    polygons.last_mut().unwrap().push(ring(r, true));
                }
            }
            let rendered: Vec<String> = polygons
                .iter()
                .map(|p| format!("[{}]", p.join(",")))
                .collect();
            if rendered.len() == 1 {
                format!("{{\"type\":\"Polygon\",\"coordinates\":{}}}", rendered[0])
            } else {
                format!(
                    "{{\"type\":\"MultiPolygon\",\"coordinates\":[{}]}}",
                    rendered.join(",")
                )
            }
        }
    }
}

/// GeoJSON (ArcGIS `FeaturesToJSON` with `GEOJSON`).
pub fn geojson(d: &Dataset, formatted: bool) -> String {
    let mut header: Vec<(String, Node)> = vec![("type".into(), raw("\"FeatureCollection\""))];
    if let Some((_, latest)) = d.wkid {
        header.push((
            "crs".into(),
            Node::Object(vec![
                ("type".into(), raw("\"name\"")),
                (
                    "properties".into(),
                    Node::Object(vec![(
                        "name".into(),
                        raw(string(&format!("EPSG:{latest}"))),
                    )]),
                ),
            ]),
        ));
    }
    let features: Vec<String> = d
        .features
        .iter()
        .map(|f| {
            let mut props = vec![format!("{}:{}", string(d.oid_name), f.oid)];
            props.extend(
                d.fields
                    .iter()
                    .zip(&f.attributes)
                    .map(|(fd, v)| format!("{}:{}", string(&fd.name), value(v, fd))),
            );
            format!(
                "{{\"type\":\"Feature\",\"id\":{},\"geometry\":{},\"properties\":{{{}}}}}",
                f.oid,
                geojson_geometry(f.geometry.as_ref(), d.kind, d.z, d.m),
                props.join(",")
            )
        })
        .collect();
    header.push(("features".into(), raw(format!("[{}]", features.join(",")))));
    render(&Node::Object(header), formatted, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formats() {
        assert_eq!(g17(499950.0), "499950");
        assert_eq!(g17(1754.62), "1754.6199999999999");
        assert_eq!(g17(68.4), "68.400000000000006");
        assert_eq!(g17(107.3), "107.3");
        assert_eq!(g17(15.003613215782456), "15.003613215782456");
        // Checked against C's printf("%.17g").
        assert_eq!(g17(0.000012), "1.2e-05");
        assert_eq!(g17(1e20), "1e+20");
        assert_eq!(g17(123456789012345678.0), "1.2345678901234568e+17");
        assert_eq!(g17(0.0001), "0.0001");
        assert_eq!(shortest(70.0), "70.0");
        assert_eq!(shortest(68.1), "68.1");
    }
}

/// Features read from Esri JSON or GeoJSON (ArcGIS JSON To Features).
#[derive(Debug, Clone)]
pub struct Imported {
    pub kind: GeometryKind,
    pub has_z: bool,
    pub has_m: bool,
    pub fields: Vec<Field>,
    pub features: Vec<Feature>,
    /// The coordinate system's code (latest wkid / EPSG), or its WKT.
    pub wkid: Option<u32>,
    pub wkt: Option<String>,
}

fn bad(msg: &str) -> unigis_core::Error {
    unigis_core::Error::format("JSON", msg)
}

type Json = serde_json::Value;

fn coords(v: &Json, z: bool, m: bool) -> Option<([f64; 2], f64, f64)> {
    let a = v.as_array()?;
    let n = |i: usize| a.get(i).and_then(Json::as_f64);
    let x = n(0)?;
    let y = n(1)?;
    let mut k = 2;
    let zv = if z {
        k += 1;
        n(2).unwrap_or(0.0)
    } else {
        0.0
    };
    let mv = if m {
        n(k).unwrap_or(f64::NAN)
    } else {
        f64::NAN
    };
    Some(([x, y], zv, mv))
}

fn build(
    kind: GeometryKind,
    parts: Vec<Vec<([f64; 2], f64, f64)>>,
    z: bool,
    m: bool,
) -> Option<Geometry> {
    let parts: Vec<_> = parts.into_iter().filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut g = Geometry {
        kind,
        parts: Vec::new(),
        part_types: None,
        xy: Vec::new(),
        z: z.then(Vec::new),
        m: m.then(Vec::new),
        curves: Vec::new(),
    };
    for p in parts {
        if !matches!(kind, GeometryKind::Point | GeometryKind::Multipoint) {
            g.parts.push(g.xy.len() as u32);
        }
        for (xy, zv, mv) in p {
            g.xy.push(xy);
            if let Some(zs) = &mut g.z {
                zs.push(zv);
            }
            if let Some(ms) = &mut g.m {
                ms.push(mv);
            }
        }
    }
    Some(g)
}

fn esri_field(f: &Json) -> Option<Field> {
    let name = f.get("name")?.as_str()?;
    let t = f.get("type")?.as_str()?;
    let field_type = match t {
        "esriFieldTypeOID" | "esriFieldTypeInteger" => FieldType::Integer,
        "esriFieldTypeSmallInteger" => FieldType::SmallInteger,
        "esriFieldTypeBigInteger" => FieldType::BigInteger,
        "esriFieldTypeDouble" => FieldType::Double,
        "esriFieldTypeSingle" => FieldType::Single,
        "esriFieldTypeString" => FieldType::String,
        "esriFieldTypeDate" => FieldType::Date,
        "esriFieldTypeGUID" | "esriFieldTypeGlobalID" => FieldType::Guid,
        _ => return None,
    };
    let mut field = Field::new(name, field_type);
    if field_type == FieldType::String {
        field.length = f.get("length").and_then(Json::as_u64).unwrap_or(255) as u32;
    }
    if field_type == FieldType::Integer {
        field.precision = 10;
    }
    if let Some(a) = f.get("alias").and_then(Json::as_str).filter(|a| *a != name) {
        field.alias = Some(a.to_owned());
    }
    Some(field)
}

fn attribute(v: Option<&Json>, f: &Field) -> Value {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return Value::Null;
    };
    match f.field_type {
        FieldType::String | FieldType::Guid => match v {
            Json::String(s) => Value::String(s.clone()),
            other => Value::String(other.to_string()),
        },
        FieldType::SmallInteger => v
            .as_f64()
            .map_or(Value::Null, |n| Value::SmallInteger(n as i16)),
        FieldType::Integer => v.as_f64().map_or(Value::Null, |n| Value::Integer(n as i32)),
        FieldType::BigInteger => v
            .as_f64()
            .map_or(Value::Null, |n| Value::BigInteger(n as i64)),
        FieldType::Single => v.as_f64().map_or(Value::Null, |n| Value::Single(n as f32)),
        FieldType::Date => v
            .as_i64()
            .and_then(chrono_ms)
            .map_or(Value::Null, Value::Date),
        _ => v.as_f64().map_or(Value::Null, Value::Double),
    }
}

fn chrono_ms(ms: i64) -> Option<chrono::NaiveDateTime> {
    chrono::DateTime::from_timestamp_millis(ms).map(|d| d.naive_utc())
}

/// Reads Esri JSON (a feature set) or GeoJSON (a FeatureCollection; only the
/// features of `geojson_kind` are taken, made 3D as ArcGIS does).
pub fn read(text: &str, geojson_kind: Option<GeometryKind>) -> unigis_core::Result<Imported> {
    let doc: Json = serde_json::from_str(text).map_err(|e| bad(&e.to_string()))?;
    if doc.get("type").and_then(Json::as_str) == Some("FeatureCollection") {
        return read_geojson(&doc, geojson_kind);
    }
    let kind = match doc.get("geometryType").and_then(Json::as_str) {
        Some("esriGeometryPoint") => GeometryKind::Point,
        Some("esriGeometryMultipoint") => GeometryKind::Multipoint,
        Some("esriGeometryPolyline") => GeometryKind::Polyline,
        Some("esriGeometryPolygon") => GeometryKind::Polygon,
        other => return Err(bad(&format!("geometryType {other:?}"))),
    };
    let flag = |k: &str| doc.get(k).and_then(Json::as_bool).unwrap_or(false);
    let (has_z, has_m) = (flag("hasZ"), flag("hasM"));
    let sr = doc.get("spatialReference");
    let wkid = sr
        .and_then(|s| s.get("latestWkid").or_else(|| s.get("wkid")))
        .and_then(Json::as_u64)
        .map(|c| c as u32);
    let wkt = sr
        .and_then(|s| s.get("wkt"))
        .and_then(Json::as_str)
        .map(str::to_owned);
    let declared: Vec<&Json> = doc
        .get("fields")
        .and_then(Json::as_array)
        .map(|fs| fs.iter().collect())
        .unwrap_or_default();
    let mut fields: Vec<Field> = declared.iter().filter_map(|f| esri_field(f)).collect();
    // The OID becomes an ordinary field whose values are not carried over.
    let oid_names: Vec<&str> = declared
        .iter()
        .filter(|f| f.get("type").and_then(Json::as_str) == Some("esriFieldTypeOID"))
        .filter_map(|f| f.get("name").and_then(Json::as_str))
        .collect();
    // Through a geodatabase: lines and polygons gain these (left at 0).
    let shape_fields: &[&str] = match kind {
        GeometryKind::Polyline => &["Shape_Length"],
        GeometryKind::Polygon => &["Shape_Length", "Shape_Area"],
        _ => &[],
    };
    let attr_count = fields.len();
    fields.extend(
        shape_fields
            .iter()
            .map(|n| Field::new(*n, FieldType::Double)),
    );
    let mut features = Vec::new();
    for (i, f) in doc
        .get("features")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let attrs = f.get("attributes");
        let mut values: Vec<Value> = fields[..attr_count]
            .iter()
            .map(|fd| {
                if oid_names.contains(&fd.name.as_str()) {
                    Value::Null
                } else {
                    attribute(attrs.and_then(|a| a.get(&fd.name)), fd)
                }
            })
            .collect();
        values.extend(shape_fields.iter().map(|_| Value::Double(0.0)));
        let g = f.get("geometry").filter(|g| !g.is_null());
        let geometry = g.and_then(|g| esri_geometry_in(g, kind, has_z, has_m));
        features.push(Feature {
            oid: i as i64,
            geometry,
            attributes: values,
        });
    }
    Ok(Imported {
        kind,
        has_z,
        has_m,
        fields,
        features,
        wkid,
        wkt,
    })
}

fn esri_geometry_in(g: &Json, kind: GeometryKind, z: bool, m: bool) -> Option<Geometry> {
    let list = |key: &str| -> Vec<Vec<([f64; 2], f64, f64)>> {
        g.get(key)
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .map(|part| {
                part.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| coords(c, z, m))
                    .collect()
            })
            .collect()
    };
    match kind {
        GeometryKind::Point => {
            let x = g.get("x")?.as_f64()?;
            let y = g.get("y")?.as_f64()?;
            let zv = g.get("z").and_then(Json::as_f64).unwrap_or(0.0);
            let mv = g.get("m").and_then(Json::as_f64).unwrap_or(f64::NAN);
            build(kind, vec![vec![([x, y], zv, mv)]], z, m)
        }
        GeometryKind::Multipoint => {
            let pts: Vec<_> = g
                .get("points")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .filter_map(|c| coords(c, z, m))
                .collect();
            build(kind, vec![pts], z, m)
        }
        GeometryKind::Polyline => build(kind, list("paths"), z, m),
        _ => build(kind, list("rings"), z, m),
    }
}

fn read_geojson(doc: &Json, kind: Option<GeometryKind>) -> unigis_core::Result<Imported> {
    let kind = kind.ok_or_else(|| bad("GeoJSON needs a geometry type"))?;
    let wanted = |t: &str| match kind {
        GeometryKind::Point => t == "Point",
        GeometryKind::Multipoint => t == "MultiPoint",
        GeometryKind::Polyline => t == "LineString" || t == "MultiLineString",
        GeometryKind::Polygon => t == "Polygon" || t == "MultiPolygon",
        GeometryKind::MultiPatch => false,
    };
    let feats: Vec<&Json> = doc
        .get("features")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter(|f| {
            f.get("geometry")
                .and_then(|g| g.get("type"))
                .and_then(Json::as_str)
                .is_some_and(wanted)
        })
        .collect();
    // Fields in order of first appearance: numbers are doubles, the rest text(254).
    let mut fields: Vec<Field> = Vec::new();
    for f in &feats {
        if let Some(props) = f.get("properties").and_then(Json::as_object) {
            for (k, v) in props {
                if fields.iter().any(|fd| fd.name == *k) || v.is_null() {
                    continue;
                }
                let mut fd = Field::new(
                    k,
                    if v.is_number() {
                        FieldType::Double
                    } else {
                        FieldType::String
                    },
                );
                if fd.field_type == FieldType::String {
                    fd.length = 254;
                }
                fields.push(fd);
            }
        }
    }
    let mut features = Vec::new();
    for (i, f) in feats.iter().enumerate() {
        let props = f.get("properties");
        let attributes = fields
            .iter()
            .map(|fd| attribute(props.and_then(|p| p.get(&fd.name)), fd))
            .collect();
        let g = &f["geometry"];
        let c = &g["coordinates"];
        let path = |v: &Json| -> Vec<([f64; 2], f64, f64)> {
            v.as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| coords(p, true, false))
                .collect()
        };
        let mut parts: Vec<Vec<([f64; 2], f64, f64)>> = Vec::new();
        match g["type"].as_str().unwrap_or("") {
            "Point" => parts.push(coords(c, true, false).into_iter().collect()),
            "MultiPoint" => parts.push(path(c)),
            "LineString" => parts.push(path(c)),
            "MultiLineString" | "Polygon" => {
                parts.extend(c.as_array().into_iter().flatten().map(path))
            }
            "MultiPolygon" => {
                for poly in c.as_array().into_iter().flatten() {
                    parts.extend(poly.as_array().into_iter().flatten().map(path));
                }
            }
            _ => {}
        }
        features.push(Feature {
            oid: i as i64,
            geometry: build(kind, parts, true, false),
            attributes,
        });
    }
    let wkid = doc
        .get("crs")
        .and_then(|c| c.get("properties"))
        .and_then(|p| p.get("name"))
        .and_then(Json::as_str)
        .and_then(|n| n.rsplit(':').next())
        .and_then(|c| c.parse::<u32>().ok())
        .or(Some(4326));
    Ok(Imported {
        kind,
        has_z: true,
        has_m: false,
        fields,
        features,
        wkid,
        wkt: None,
    })
}
