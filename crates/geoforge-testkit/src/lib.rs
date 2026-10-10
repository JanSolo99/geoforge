//! Oracle tests: compare what a GeoForge reader returns with what GDAL's `ogrinfo`
//! reports for the same file.
//!
//! GDAL is invoked as a subprocess, so it is only needed when tests run. Attributes
//! come from `ogrinfo -json`; geometries from the text output in ISO WKT, which
//! (unlike the JSON) keeps M values and MultiPatch structure.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use geoforge_core::{Feature, Geometry, GeometryKind, PartType, Value};
use serde_json::Value as Json;

/// Whether `ogrinfo` is on `PATH`. Oracle tests skip themselves when it is not.
pub fn ogrinfo_available() -> bool {
    Command::new("ogrinfo")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[derive(Debug, Clone)]
pub struct OgrField {
    pub name: String,
    pub field_type: String,
    pub width: Option<u64>,
    pub precision: Option<u64>,
}

/// A coordinate as `[x, y, z, m]`, NaN where the ordinate is absent.
pub type Coord = [f64; 4];

#[derive(Debug, Clone, Default)]
pub struct OgrFeature {
    pub properties: serde_json::Map<String, Json>,
    /// The ISO WKT, or `None` for a null geometry.
    pub wkt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OgrLayer {
    pub fields: Vec<OgrField>,
    pub features: BTreeMap<i64, OgrFeature>,
}

/// Runs `ogrinfo` on `path` and returns its single layer.
pub fn ogr_layer(path: &Path) -> Result<OgrLayer, String> {
    let json = run(Command::new("ogrinfo")
        .args(["-json", "-features", "-nomd"])
        .arg(path))?;
    let json: Json = serde_json::from_str(&json).map_err(|e| format!("ogrinfo JSON: {e}"))?;
    let layer = &json["layers"][0];
    let fields = layer["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| OgrField {
            name: f["name"].as_str().unwrap_or_default().to_owned(),
            field_type: f["type"].as_str().unwrap_or_default().to_owned(),
            width: f["width"].as_u64(),
            precision: f["precision"].as_u64(),
        })
        .collect();
    let mut features: BTreeMap<i64, OgrFeature> = BTreeMap::new();
    for f in layer["features"].as_array().into_iter().flatten() {
        let fid = f["fid"].as_i64().ok_or("feature without fid")?;
        let properties = f["properties"].as_object().cloned().unwrap_or_default();
        features.insert(
            fid,
            OgrFeature {
                properties,
                wkt: None,
            },
        );
    }

    let text = run(Command::new("ogrinfo")
        .args(["-al", "-q", "-nomd", "-fields=NO", "-geom=ISO_WKT"])
        .arg(path))?;
    let mut current = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("OGRFeature(") {
            current = rest
                .split_once("):")
                .and_then(|(_, fid)| fid.trim().parse::<i64>().ok());
        } else if let (Some(fid), Some(wkt)) = (current, line.strip_prefix("  ")) {
            if let Some(f) = features.get_mut(&fid) {
                f.wkt = Some(wkt.trim().to_owned());
            }
        }
    }
    Ok(OgrLayer { fields, features })
}

fn run(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("running ogrinfo: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "ogrinfo failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Coordinate sequences of a WKT string: every innermost parenthesized list.
pub fn wkt_sequences(wkt: &str) -> Vec<Vec<Coord>> {
    let head = wkt.split('(').next().unwrap_or_default();
    let words: Vec<&str> = head.split_whitespace().collect();
    let has_zm = words.contains(&"ZM");
    let has_z = has_zm || words.contains(&"Z");
    let has_m = has_zm || words.contains(&"M");

    let mut seqs = Vec::new();
    let bytes = wkt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            let end = i + 1 + wkt[i + 1..].find(')').unwrap_or(wkt.len() - i - 1);
            // Innermost list: no nested '(' before its ')'.
            if !wkt[i + 1..end].contains('(') {
                let seq = wkt[i + 1..end]
                    .split(',')
                    .map(|c| {
                        let n: Vec<f64> = c
                            .split_whitespace()
                            .filter_map(|t| t.parse().ok())
                            .collect();
                        let get = |k: usize| n.get(k).copied().unwrap_or(f64::NAN);
                        match (has_z, has_m) {
                            (true, true) => [get(0), get(1), get(2), get(3)],
                            (true, false) => [get(0), get(1), get(2), f64::NAN],
                            (false, true) => [get(0), get(1), f64::NAN, get(2)],
                            (false, false) => [get(0), get(1), f64::NAN, f64::NAN],
                        }
                    })
                    .collect();
                seqs.push(seq);
                i = end;
            }
        }
        i += 1;
    }
    seqs
}

/// Coordinate sequences of our geometry, shaped the way GDAL renders it as WKT:
/// one sequence per point of a multipoint, per path/ring, and per triangle of a
/// MultiPatch strip or fan.
pub fn geometry_sequences(g: &Geometry) -> Vec<Vec<Coord>> {
    let coord = |i: usize| {
        [
            g.xy[i][0],
            g.xy[i][1],
            g.z.as_ref().map_or(f64::NAN, |z| z[i]),
            g.m.as_ref().map_or(f64::NAN, |m| m[i]),
        ]
    };
    match g.kind {
        GeometryKind::Point | GeometryKind::Multipoint => {
            (0..g.xy.len()).map(|i| vec![coord(i)]).collect()
        }
        GeometryKind::Polyline | GeometryKind::Polygon => {
            g.part_ranges().map(|r| r.map(coord).collect()).collect()
        }
        GeometryKind::MultiPatch => {
            let types = g.part_types.as_deref().unwrap_or_default();
            let mut out = Vec::new();
            for (r, t) in g.part_ranges().zip(types) {
                let p: Vec<Coord> = r.map(coord).collect();
                match t {
                    PartType::TriangleStrip if p.len() >= 3 => {
                        out.extend((0..p.len() - 2).map(|i| vec![p[i], p[i + 1], p[i + 2], p[i]]));
                    }
                    PartType::TriangleFan if p.len() >= 3 => {
                        out.extend((1..p.len() - 1).map(|i| vec![p[0], p[i], p[i + 1], p[0]]));
                    }
                    _ => out.push(p),
                }
            }
            out
        }
    }
}

/// Order-independent form for comparison: sort the sequences (GDAL regroups polygon
/// rings) and, with `normalize_rings`, drop each closed sequence's closing vertex and
/// sort its vertices (GDAL re-orders MultiPatch triangle vertices).
fn canonical(mut seqs: Vec<Vec<Coord>>, normalize_rings: bool) -> Vec<Vec<Coord>> {
    let key = |c: &Coord| c.map(|v| if v.is_nan() { f64::NEG_INFINITY } else { v });
    let cmp = |a: &Coord, b: &Coord| {
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    for s in &mut seqs {
        if normalize_rings && s.len() > 1 && same(&s[0], &s[s.len() - 1]) {
            s.pop();
            s.sort_by(cmp);
        }
    }
    seqs.sort_by(|a, b| {
        a.iter()
            .zip(b)
            .map(|(x, y)| cmp(x, y))
            .find(|o| o.is_ne())
            .unwrap_or(a.len().cmp(&b.len()))
    });
    seqs
}

fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// Equal within tolerance. A missing or no-data measure on either side matches any
/// missing/no-data measure on the other (GDAL drops all-no-data M).
fn same(a: &Coord, b: &Coord) -> bool {
    let m_missing = |m: f64| m.is_nan() || m <= -1e38;
    let z_ok = close(a[2], b[2])
        || (a[2].is_nan() && b[2] == 0.0)
        || (b[2].is_nan() && a[2] == 0.0)
        || (a[2].is_nan() && b[2].is_nan());
    let m_ok = (m_missing(a[3]) && m_missing(b[3])) || close(a[3], b[3]);
    close(a[0], b[0]) && close(a[1], b[1]) && z_ok && m_ok
}

/// Compares our feature's geometry to GDAL's WKT. Returns a description of the
/// first difference, if any.
pub fn compare_geometry(ours: Option<&Geometry>, gdal_wkt: Option<&str>) -> Option<String> {
    let gdal_seqs = gdal_wkt.map(wkt_sequences).unwrap_or_default();
    let mut our_seqs = ours.map(geometry_sequences).unwrap_or_default();
    // GDAL decides per layer whether M exists (e.g. drops it when the header M range
    // is no-data), so only compare M when GDAL reports it.
    let gdal_has_m = gdal_wkt.is_some_and(|w| {
        let head = w.split('(').next().unwrap_or_default();
        head.split_whitespace().any(|t| t == "M" || t == "ZM")
    });
    if !gdal_has_m {
        our_seqs.iter_mut().flatten().for_each(|c| c[3] = f64::NAN);
    }
    let normalize = ours.is_some_and(|g| g.kind == GeometryKind::MultiPatch);
    let (a, b) = (
        canonical(our_seqs, normalize),
        canonical(gdal_seqs, normalize),
    );
    let equal = a.len() == b.len()
        && a.iter()
            .zip(&b)
            .all(|(x, y)| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)));
    (!equal).then(|| {
        format!(
            "geometry differs: ours {} sequences {:?}..., GDAL `{}`",
            a.len(),
            a.first().map(|s| &s[..s.len().min(3)]),
            gdal_wkt.map_or("<null>".into(), |w| w.chars().take(200).collect::<String>())
        )
    })
}

/// Compares two of our geometries (vertex order matters except within MultiPatch
/// triangles).
pub fn compare_geometries(a: Option<&Geometry>, b: Option<&Geometry>) -> Option<String> {
    let normalize = a.is_some_and(|g| g.kind == GeometryKind::MultiPatch);
    let seqs =
        |g: Option<&Geometry>| canonical(g.map(geometry_sequences).unwrap_or_default(), normalize);
    let (x, y) = (seqs(a), seqs(b));
    let equal = x.len() == y.len()
        && x.iter()
            .zip(&y)
            .all(|(p, q)| p.len() == q.len() && p.iter().zip(q).all(|(c, d)| same(c, d)));
    (!equal).then(|| {
        format!(
            "geometry differs: {} vs {} sequences; first {:?} vs {:?}",
            x.len(),
            y.len(),
            x.first(),
            y.first()
        )
    })
}

/// Compares one attribute value with GDAL's JSON property.
pub fn compare_value(ours: &Value, gdal: &Json) -> Option<String> {
    let num = |v: f64| match gdal {
        Json::Number(n) => n.as_f64().is_some_and(|g| close(v, g)),
        Json::Null => false,
        _ => false,
    };
    let ok = match (ours, gdal) {
        (Value::Null, Json::Null) => true,
        // dBASE "unset" logical; GDAL < 3.9 reports it raw instead of null.
        (Value::Null, Json::String(g)) => g == "?",
        (Value::String(s), Json::Null) => s.is_empty(),
        (Value::String(s), Json::String(g)) => s == g,
        (Value::String(s), Json::Bool(b)) => {
            matches!((s.as_str(), b), ("T" | "Y", true) | ("F" | "N", false))
        }
        (Value::String(s), Json::Number(_)) => s.trim().parse::<f64>().is_ok_and(num),
        (Value::SmallInteger(v), _) => num(f64::from(*v)),
        (Value::Integer(v), _) => num(f64::from(*v)),
        (Value::BigInteger(v), _) => num(*v as f64),
        (Value::Double(v), _) => num(*v),
        (Value::Date(d), Json::String(g)) => d.format("%Y/%m/%d").to_string() == *g,
        _ => false,
    };
    (!ok).then(|| format!("ours {ours:?}, GDAL {gdal}"))
}

/// Compares a whole feature; returns all differences found.
pub fn compare_feature(ours: &Feature, field_names: &[&str], gdal: &OgrFeature) -> Vec<String> {
    let mut diffs = Vec::new();
    if let Some(d) = compare_geometry(ours.geometry.as_ref(), gdal.wkt.as_deref()) {
        diffs.push(format!("FID {}: {d}", ours.oid));
    }
    for (name, value) in field_names.iter().zip(&ours.attributes) {
        let g = gdal.properties.get(*name).unwrap_or(&Json::Null);
        if let Some(d) = compare_value(value, g) {
            diffs.push(format!("FID {} field {name}: {d}", ours.oid));
        }
    }
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_wkt_sequences() {
        let s = wkt_sequences(
            "MULTIPOLYGON ZM (((0 0 1 2,1 0 1 2,0 0 1 2)),((5 5 0 0,6 5 0 0,5 5 0 0)))",
        );
        assert_eq!(s.len(), 2);
        assert_eq!(s[0][1], [1.0, 0.0, 1.0, 2.0]);
        let m = wkt_sequences("LINESTRING M (0 0 10,1 1 20)");
        assert!(m[0][0][2].is_nan());
        assert_eq!(m[0][1][3], 20.0);
        assert!(wkt_sequences("POINT EMPTY").is_empty());
        let gc = wkt_sequences("GEOMETRYCOLLECTION Z (TIN Z (((0 0 1,1 0 1,0 0 1))))");
        assert_eq!(gc.len(), 1);
    }
}
