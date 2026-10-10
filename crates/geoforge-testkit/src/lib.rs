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

/// Runs `ogrinfo` on `path` and returns `layer` (or the first layer).
pub fn ogr_layer(path: &Path, layer: Option<&str>) -> Result<OgrLayer, String> {
    let json = run(Command::new("ogrinfo")
        .args(["-json", "-features", "-nomd"])
        .arg(path)
        .args(layer))?;
    let json: Json = serde_json::from_str(&json).map_err(|e| format!("ogrinfo JSON: {e}"))?;
    let first = &json["layers"][0];
    let fields = first["fields"]
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
    for f in first["features"].as_array().into_iter().flatten() {
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

    let mut text_cmd = Command::new("ogrinfo");
    text_cmd
        .args(["-q", "-nomd", "-fields=NO", "-geom=ISO_WKT"])
        .arg(path);
    match layer {
        Some(l) => text_cmd.arg(l),
        None => text_cmd.arg("-al"),
    };
    let text = run(&mut text_cmd)?;
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

/// GDAL's straight-line approximation of every feature in `layer` (curves
/// linearized), keyed by FID. `None` for null/empty geometries.
pub fn ogr_linearized(path: &Path, layer: &str) -> Result<BTreeMap<i64, Option<String>>, String> {
    let csv = run(Command::new("ogr2ogr")
        .args(["-f", "CSV", "/vsistdout/"])
        .arg(path)
        .args(["-sql", &format!("SELECT FID AS gf_fid FROM \"{layer}\"")])
        .args(["-nlt", "CONVERT_TO_LINEAR", "-lco", "GEOMETRY=AS_WKT"]))?;
    let mut out = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let (wkt, fid) = match line.strip_prefix('"') {
            Some(rest) => {
                let end = rest.find('"').ok_or("unterminated WKT")?;
                (
                    Some(rest[..end].to_owned()),
                    rest[end + 1..].trim_start_matches(','),
                )
            }
            None => (None, line.trim_start_matches(',')),
        };
        let fid: i64 = fid
            .trim()
            .parse()
            .map_err(|_| format!("bad fid in `{line}`"))?;
        out.insert(fid, wkt.filter(|w| !w.is_empty()));
    }
    Ok(out)
}

/// Compares the shapes of two linear geometries loosely: every vertex of each must
/// lie within `tol` of the other's segments (a two-sided Hausdorff check on XY).
/// Used for curves, which GDAL and we densify with different step sizes.
pub fn compare_shapes(ours: &Geometry, gdal_wkt: &str, tol: f64) -> Option<String> {
    let a: Vec<Vec<[f64; 2]>> = ours.part_ranges().map(|r| ours.xy[r].to_vec()).collect();
    let b: Vec<Vec<[f64; 2]>> = wkt_sequences(gdal_wkt)
        .into_iter()
        .map(|s| s.into_iter().map(|c| [c[0], c[1]]).collect())
        .collect();
    // GDAL's vertices lie on the true curve, so they must be close to our (finer)
    // densification. Our vertices may sit off GDAL's coarser chords by up to their
    // sagitta, so that direction allows 5% of the GDAL segment length on top.
    let far = |from: &[Vec<[f64; 2]>], to: &[Vec<[f64; 2]>], chord_allowance: f64| {
        from.iter()
            .flatten()
            .copied()
            .map(|p| (p, dist_to_paths(p, to, chord_allowance)))
            .find(|&(_, d)| d > tol)
    };
    if let Some((p, d)) = far(&b, &a, 0.0) {
        return Some(format!(
            "GDAL vertex {p:?} is {d:.3e} from our shape (tolerance {tol:.1e})"
        ));
    }
    if let Some((p, d)) = far(&a, &b, 0.05) {
        return Some(format!(
            "our vertex {p:?} is {d:.3e} beyond GDAL's chords (tolerance {tol:.1e})"
        ));
    }
    None
}

/// Distance from `p` to the nearest segment, less `chord_allowance` × that
/// segment's length.
fn dist_to_paths(p: [f64; 2], paths: &[Vec<[f64; 2]>], chord_allowance: f64) -> f64 {
    let seg = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 == 0.0 {
            0.0
        } else {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
        };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy) - chord_allowance * len2.sqrt()
    };
    paths
        .iter()
        .flat_map(|path| {
            if path.len() == 1 {
                vec![seg(path[0], path[0])]
            } else {
                path.windows(2).map(|w| seg(w[0], w[1])).collect()
            }
        })
        .fold(f64::INFINITY, f64::min)
}

/// GDAL's version as (major, minor), e.g. (3, 13).
pub fn gdal_version() -> Option<(u32, u32)> {
    let out = Command::new("ogrinfo").arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v = text.split_whitespace().nth(1)?.trim_end_matches(',');
    let mut parts = v.split('.').map(|p| p.parse().ok());
    Some((parts.next()??, parts.next()??))
}

/// Names of all layers GDAL lists in a dataset.
pub fn ogr_layer_names(path: &Path) -> Result<Vec<String>, String> {
    let json = run(Command::new("ogrinfo")
        .args(["-json", "-so", "-nomd"])
        .arg(path))?;
    let json: Json = serde_json::from_str(&json).map_err(|e| format!("ogrinfo JSON: {e}"))?;
    Ok(json["layers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l["name"].as_str().map(str::to_owned))
        .collect())
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
                    PartType::Triangles => {
                        out.extend(p.chunks_exact(3).map(|t| vec![t[0], t[1], t[2], t[0]]));
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
    // GDAL renders true curves as CIRCULARSTRING etc.; callers compare those with
    // `compare_shapes` against GDAL's linearization.
    if ours.is_some_and(Geometry::has_curves) {
        return None;
    }
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
    // GDAL's shapefile reader splits self-touching rings at the repeated vertex;
    // ArcGIS (and we) keep them whole. Accept either form.
    if ours.is_some_and(|g| g.kind == GeometryKind::Polygon) {
        let split: Vec<Vec<Coord>> = our_seqs
            .iter()
            .flat_map(|r| split_self_touching(r))
            .collect();
        if split.len() != our_seqs.len()
            && sequences_equal(
                &canonical(split, false),
                &canonical(gdal_seqs.clone(), false),
            )
        {
            return None;
        }
    }
    let normalize = ours.is_some_and(|g| g.kind == GeometryKind::MultiPatch);
    let (a, b) = (
        canonical(our_seqs, normalize),
        canonical(gdal_seqs, normalize),
    );
    let equal = sequences_equal(&a, &b);
    (!equal).then(|| {
        format!(
            "geometry differs: ours {} sequences {:?}..., GDAL `{}`",
            a.len(),
            a.first().map(|s| &s[..s.len().min(3)]),
            gdal_wkt.map_or("<null>".into(), |w| w.chars().take(200).collect::<String>())
        )
    })
}

fn sequences_equal(a: &[Vec<Coord>], b: &[Vec<Coord>]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)))
}

/// Splits a closed ring wherever a vertex repeats, into simple closed rings.
fn split_self_touching(ring: &[Coord]) -> Vec<Vec<Coord>> {
    if ring.len() < 4 || !same(&ring[0], &ring[ring.len() - 1]) {
        return vec![ring.to_vec()];
    }
    let mut out = Vec::new();
    let mut stack: Vec<Coord> = Vec::new();
    for &p in &ring[..ring.len() - 1] {
        if let Some(i) = stack.iter().position(|q| same(q, &p)) {
            let mut loop_ring = stack.split_off(i);
            loop_ring.push(p);
            out.push(loop_ring);
        }
        stack.push(p);
    }
    stack.push(stack[0]);
    out.push(stack);
    out
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
        (Value::Single(v), _) => num(f64::from(*v)),
        (Value::Date(d), Json::String(g)) => {
            // Shapefile dates are date-only; FileGDB ones carry a time (and GDAL may
            // append a UTC offset such as "+00").
            *g == d.format("%Y/%m/%d").to_string()
                || parse_gdal_datetime(g)
                    .is_some_and(|(t, _)| (t - *d).num_milliseconds().abs() <= 1)
        }
        (Value::DateOnly(d), Json::String(g)) => *g == d.format("%Y/%m/%d").to_string(),
        // GDAL drops sub-second precision for time-only values.
        (Value::TimeOnly(t), Json::String(g)) => *g == t.format("%H:%M:%S").to_string(),
        (Value::TimestampOffset(t), Json::String(g)) => {
            parse_gdal_datetime(g).is_some_and(|(local, off)| {
                (local - t.naive_local()).num_milliseconds().abs() <= 1
                    && off.is_none_or(|o| o == t.offset().local_minus_utc() / 60)
            })
        }
        (Value::Guid(s) | Value::Xml(s), Json::String(g)) => s == g,
        // GDAL leaves binary fields out of its JSON.
        (Value::Blob(_), _) => true,
        _ => false,
    };
    (!ok).then(|| format!("ours {ours:?}, GDAL {gdal}"))
}

/// Parses GDAL's `YYYY/MM/DD HH:MM:SS[.sss][±HH[MM]]` into local time and the
/// offset in minutes, if any.
fn parse_gdal_datetime(s: &str) -> Option<(chrono::NaiveDateTime, Option<i32>)> {
    let (body, offset) = match s.rfind(['+', '-']) {
        Some(i) if i > 10 => {
            let (b, o) = s.split_at(i);
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let digits: String = o[1..].chars().filter(char::is_ascii_digit).collect();
            let h: i32 = digits.get(..2)?.parse().ok()?;
            let m: i32 = digits.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
            (b, Some(sign * (h * 60 + m)))
        }
        _ => (s, None),
    };
    let t = chrono::NaiveDateTime::parse_from_str(body, "%Y/%m/%d %H:%M:%S%.f").ok()?;
    Some((t, offset))
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
