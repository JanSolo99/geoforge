//! The XY domain ArcGIS Pro gives a coordinate system by default.
//!
//! ArcGIS derives each system's origin from its own projection engine (where the projection is
//! defined, with a margin), which no formula reproduces; `xy_domains.tsv.gz` holds what ArcGIS
//! answers for every projected and geographic system PROJ knows, by Esri name
//! (`scripts/make-xy-domains.py`). The domain's extent is always 2^53 / scale, so the origin, the
//! resolution and the tolerance are all there is.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

const DATA: &[u8] = include_bytes!("xy_domains.tsv.gz");

/// A coordinate system's default XY domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XyDomain {
    pub x_origin: f64,
    pub y_origin: f64,
    /// Units per unit of resolution: `1 / resolution`.
    pub scale: f64,
    pub tolerance: f64,
}

fn table() -> &'static HashMap<String, XyDomain> {
    static TABLE: OnceLock<HashMap<String, XyDomain>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut text = String::new();
        if flate2::read::GzDecoder::new(DATA)
            .read_to_string(&mut text)
            .is_err()
        {
            return HashMap::new();
        }
        let mut map = HashMap::new();
        for line in text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let f: Vec<&str> = line.split('\t').collect();
            let num = |i: usize, default: f64| {
                f.get(i)
                    .filter(|s| !s.is_empty())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(default)
            };
            let (Some(name), Some(x), Some(y)) = (
                f.first(),
                f.get(1).and_then(|s| s.parse().ok()),
                f.get(2).and_then(|s| s.parse().ok()),
            ) else {
                continue;
            };
            map.insert(
                (*name).to_owned(),
                XyDomain {
                    x_origin: x,
                    y_origin: y,
                    scale: scale_of(num(3, 0.0001)),
                    tolerance: num(4, 0.001),
                },
            );
        }
        map
    })
}

/// `1 / resolution`, whole when the resolution is a power of ten written in floating point.
fn scale_of(resolution: f64) -> f64 {
    let s = 1.0 / resolution;
    if (s - s.round()).abs() < 1e-6 {
        s.round()
    } else {
        s
    }
}

/// The default XY domain of the system named `name` (the name in its `PROJCS` / `GEOGCS`).
pub fn by_name(name: &str) -> Option<XyDomain> {
    table().get(name).copied()
}

/// The default XY domain of a system given as Esri WKT: found by the name of its outermost
/// `PROJCS` or `GEOGCS`.
pub fn for_wkt(wkt: &str) -> Option<XyDomain> {
    let wkt = wkt.trim_start();
    let rest = wkt
        .strip_prefix("PROJCS[\"")
        .or_else(|| wkt.strip_prefix("GEOGCS[\""))?;
    by_name(&rest[..rest.find('"')?])
}
