//! Spatial weights matrix files (`.swm`) and the tools that make and read them: Generate Spatial
//! Weights Matrix and Convert Spatial Weights Matrix to Table.
//!
//! The file format, worked out by writing files with ArcGIS Pro 3.7 on small inputs and reading
//! the bytes (`docs/formats/swm.md`):
//! - one header line of `KEY@value` pairs joined by `;` (`VERSION@10.1;UNIQUEID@<field>;...`),
//!   ended by `\n`;
//! - the number of features and a row-standardization flag (0/1), then one record per feature in
//!   the input's order: its ID, its neighbor count `k`, the `k` neighbor IDs, the weights and the sum
//!   of the weights before standardization. When every weight of a row is the same
//!   (`FIXEDWEIGHTS@True`) a single weight is stored; a feature without neighbors stores its ID
//!   and 0 only.
//! - IDs and counts are little-endian `i32`, or `i64` when the unique ID field is a 64-bit integer
//!   (`HASID64@True`); weights are `f64`.
//!
//! How ArcGIS builds the weights (matched on 2,000 rows of 60 files):
//! - Distance bands (inverse distance, fixed distance, space-time window) list the neighbors in the
//!   order of a k-d tree of the features: a node of more than 16 features chooses the axis its
//!   features spread most along and moves its median there with MSVC's `std::nth_element`, then
//!   splits; the neighbors appear in the order the tree leaves the features in.
//! - The default threshold is the largest nearest-neighbor distance times 1.0001; a threshold must
//!   be shorter than the longer side of the extent. With a number of neighbors, a feature with
//!   fewer within the threshold gets its nearest ones instead, in order of distance.
//! - K nearest neighbors are in order of distance (equal distances by input order; ArcGIS breaks
//!   those ties in an order not reproduced here).
//! - Delaunay triangulation: neighbors whose Voronoi cells share an edge inside the points' convex
//!   hull (the Voronoi diagram clipped to the hull).
//! - Polygon contiguity: polygons sharing a boundary segment (edges only) or any boundary point
//!   (edges and corners), within the XY tolerance; higher orders add the neighbors' neighbors.
//! - Kernels (bisquare `15/16 (1-u^2)^2`, Gaussian `exp(-u^2/2)/sqrt(2 pi)`, triangular `1-u`,
//!   Epanechnikov `3/4 (1-u^2)`, `u = d / bandwidth`; zero weights drop the neighbor): the adaptive
//!   bandwidth is the distance to the `adaptive_neighbors`-th nearest feature (default: the number
//!   of neighbors plus one); the fixed one is the threshold for fixed distance bands, the given
//!   bandwidth, or `0.9 min(SD, sqrt(1/ln 2) Dm) n^-0.2` (Silverman's rule: standard distance and
//!   median distance to the mean center); Delaunay and contiguity always use the fixed one,
//!   inverse distance ignores kernels.
//! - Field weights are the neighbor's value and always row-standardized; shared border length
//!   weighs contiguity neighbors by the length of their common boundary.
//! - Neighbor order of Delaunay rows and higher-order contiguity rows follows a Python set (of
//!   feature indices and of IDs); the insertion order ArcGIS uses is not known, so rows of large
//!   inputs may list the same neighbors in another order.

use std::collections::HashMap;
use std::path::Path;

use unigis_core::{Error, FeatureClass, Field, FieldType, GeometryKind, Result, Value};

use crate::TableOutput;
use crate::spatstat::DistanceMethod;

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// The header keys in the order ArcGIS writes them.
const HEADER_KEYS: [&str; 30] = [
    "VERSION",
    "UNIQUEID",
    "SPATIALREFNAME",
    "INPUTFC",
    "WTYPE",
    "DISTANCEMETHOD",
    "EXPONENT",
    "THRESHOLD",
    "NUMNEIGHS",
    "INPUTTABLE",
    "TIMEFIELD",
    "TIMETYPE",
    "TIMEVALUE",
    "INPUTNET",
    "IMPEDANCEFIELD",
    "BARRIERFC",
    "UTURNPOLICY",
    "RESTRICTIONS",
    "USEHIERARCHY",
    "SEARCHTOLERANCE",
    "ADDCONCEPT",
    "FIXEDWEIGHTS",
    "HASZ",
    "HASID64",
    "MASTERISOID",
    "WEIGHTFIELD",
    "WEIGHTINGMETHOD",
    "ISSELECTION",
    "ISDEFINTIONQUERY",
    "INCLUDEFOCALFEATURE",
];

/// One feature's row of a weights matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct SwmRow {
    pub id: i64,
    pub neighbors: Vec<i64>,
    /// One weight per neighbor, as stored (row-standardized when the matrix is).
    pub weights: Vec<f64>,
    /// The sum of the weights before standardization.
    pub sum: f64,
}

/// A spatial weights matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct Swm {
    /// The header's `KEY@value` pairs, in order.
    pub header: Vec<(String, String)>,
    pub row_standardized: bool,
    pub rows: Vec<SwmRow>,
    /// The number of records the file announces (the features); a converted table can hold more
    /// records, which follow (see `table_records`).
    pub count: usize,
}

impl Swm {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.header
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The name of the unique ID field.
    pub fn unique_id(&self) -> &str {
        self.get("UNIQUEID").unwrap_or("")
    }

    pub fn fixed_weights(&self) -> bool {
        self.get("FIXEDWEIGHTS") == Some("True")
    }

    pub fn id64(&self) -> bool {
        self.get("HASID64") == Some("True")
    }

    pub fn from_bytes(b: &[u8]) -> Result<Swm> {
        let bad = || err("ERROR 000915: not a spatial weights matrix file");
        let nl = b.iter().position(|c| *c == b'\n').ok_or_else(bad)?;
        let text = std::str::from_utf8(&b[..nl]).map_err(|_| bad())?;
        let header: Vec<(String, String)> = text
            .split(';')
            .map(|kv| match kv.split_once('@') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (kv.to_string(), String::new()),
            })
            .collect();
        if header.first().map(|h| h.0.as_str()) != Some("VERSION") {
            return Err(bad());
        }
        let mut swm = Swm {
            header,
            row_standardized: false,
            rows: Vec::new(),
            count: 0,
        };
        let id64 = swm.id64();
        let fixed = swm.fixed_weights();
        let mut p = nl + 1;
        let int = |p: &mut usize| -> Result<i64> {
            let w = if id64 { 8 } else { 4 };
            let s = b.get(*p..*p + w).ok_or_else(bad)?;
            *p += w;
            Ok(if id64 {
                i64::from_le_bytes(s.try_into().unwrap())
            } else {
                i32::from_le_bytes(s.try_into().unwrap()) as i64
            })
        };
        let dbl = |p: &mut usize| -> Result<f64> {
            let s = b.get(*p..*p + 8).ok_or_else(bad)?;
            *p += 8;
            Ok(f64::from_le_bytes(s.try_into().unwrap()))
        };
        let n = int(&mut p)?;
        swm.row_standardized = int(&mut p)? != 0;
        if n < 0 || n as usize > b.len() {
            return Err(bad());
        }
        swm.count = n as usize;
        for _ in 0..n {
            let id = int(&mut p)?;
            let k = int(&mut p)?;
            if k < 0 || k as usize > b.len() {
                return Err(bad());
            }
            let k = k as usize;
            let mut neighbors = Vec::with_capacity(k);
            for _ in 0..k {
                neighbors.push(int(&mut p)?);
            }
            let (weights, sum) = if k == 0 {
                (Vec::new(), 0.0)
            } else if fixed {
                let w = dbl(&mut p)?;
                (vec![w; k], dbl(&mut p)?)
            } else {
                let mut w = Vec::with_capacity(k);
                for _ in 0..k {
                    w.push(dbl(&mut p)?);
                }
                (w, dbl(&mut p)?)
            };
            swm.rows.push(SwmRow {
                id,
                neighbors,
                weights,
                sum,
            });
        }
        Ok(swm)
    }

    pub fn read(path: &Path) -> Result<Swm> {
        let b = std::fs::read(path).map_err(|e| {
            err(format!(
                "ERROR 000732: Input Spatial Weights Matrix File: {} does not exist ({e})",
                path.display()
            ))
        })?;
        Swm::from_bytes(&b)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = self
            .header
            .iter()
            .map(|(k, v)| format!("{k}@{v}"))
            .collect::<Vec<_>>()
            .join(";")
            .into_bytes();
        out.push(b'\n');
        let id64 = self.id64();
        let fixed = self.fixed_weights();
        let int = |out: &mut Vec<u8>, v: i64| {
            if id64 {
                out.extend_from_slice(&v.to_le_bytes());
            } else {
                out.extend_from_slice(&(v as i32).to_le_bytes());
            }
        };
        int(&mut out, self.count as i64);
        int(&mut out, self.row_standardized as i64);
        for r in &self.rows {
            int(&mut out, r.id);
            int(&mut out, r.neighbors.len() as i64);
            if r.neighbors.is_empty() {
                continue;
            }
            for n in &r.neighbors {
                int(&mut out, *n);
            }
            if fixed {
                out.extend_from_slice(&r.weights[0].to_le_bytes());
            } else {
                for w in &r.weights {
                    out.extend_from_slice(&w.to_le_bytes());
                }
            }
            out.extend_from_slice(&r.sum.to_le_bytes());
        }
        out
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_bytes()).map_err(|e| err(format!("{}: {e}", path.display())))
    }
}

// ---------------------------------------------------------------------------------------------
// MSVC's std::nth_element, which orders ArcGIS's k-d tree.

fn med3(a: &mut [usize], f: usize, m: usize, l: usize, lt: &impl Fn(usize, usize) -> bool) {
    if lt(a[m], a[f]) {
        a.swap(m, f);
    }
    if lt(a[l], a[m]) {
        a.swap(l, m);
        if lt(a[m], a[f]) {
            a.swap(m, f);
        }
    }
}

fn guess_median(a: &mut [usize], f: usize, m: usize, l: usize, lt: &impl Fn(usize, usize) -> bool) {
    let count = l - f;
    if count > 40 {
        let step = (count + 1) >> 3;
        let two = step << 1;
        med3(a, f, f + step, f + two, lt);
        med3(a, m - step, m, m + step, lt);
        med3(a, l - two, l - step, l, lt);
        med3(a, f + step, m, l - step, lt);
    } else {
        med3(a, f, m, l, lt);
    }
}

/// `_Partition_by_median_guess_unchecked`: the fat pivot's range.
fn partition(
    a: &mut [usize],
    first: usize,
    last: usize,
    lt: &impl Fn(usize, usize) -> bool,
) -> (usize, usize) {
    let mid = first + ((last - first) >> 1);
    guess_median(a, first, mid, last - 1, lt);
    let mut pf = mid;
    let mut pl = pf + 1;
    while first < pf && !lt(a[pf - 1], a[pf]) && !lt(a[pf], a[pf - 1]) {
        pf -= 1;
    }
    while pl < last && !lt(a[pl], a[pf]) && !lt(a[pf], a[pl]) {
        pl += 1;
    }
    let mut gf = pl;
    let mut gl = pf;
    loop {
        while gf < last {
            if lt(a[pf], a[gf]) {
            } else if lt(a[gf], a[pf]) {
                break;
            } else if pl != gf {
                a.swap(pl, gf);
                pl += 1;
            } else {
                pl += 1;
            }
            gf += 1;
        }
        while first < gl {
            let p = gl - 1;
            if lt(a[p], a[pf]) {
            } else if lt(a[pf], a[p]) {
                break;
            } else {
                pf -= 1;
                if pf != p {
                    a.swap(pf, p);
                }
            }
            gl -= 1;
        }
        if gl == first && gf == last {
            return (pf, pl);
        }
        if gl == first {
            if pl != gf {
                a.swap(pf, pl);
            }
            pl += 1;
            a.swap(pf, gf);
            pf += 1;
            gf += 1;
        } else if gf == last {
            gl -= 1;
            pf -= 1;
            if gl != pf {
                a.swap(gl, pf);
            }
            pl -= 1;
            a.swap(pf, pl);
        } else {
            gl -= 1;
            a.swap(gf, gl);
            gf += 1;
        }
    }
}

fn insertion_sort(a: &mut [usize], first: usize, last: usize, lt: &impl Fn(usize, usize) -> bool) {
    for mid in first + 1..last {
        let val = a[mid];
        if lt(val, a[first]) {
            a.copy_within(first..mid, first + 1);
            a[first] = val;
        } else {
            let mut hole = mid;
            while lt(val, a[hole - 1]) {
                a[hole] = a[hole - 1];
                hole -= 1;
            }
            a[hole] = val;
        }
    }
}

fn nth_element(
    a: &mut [usize],
    mut first: usize,
    nth: usize,
    mut last: usize,
    lt: &impl Fn(usize, usize) -> bool,
) {
    if nth == last {
        return;
    }
    while last - first > 32 {
        let (pf, pl) = partition(a, first, last, lt);
        if pl <= nth {
            first = pl;
        } else if pf <= nth {
            return;
        } else {
            last = pf;
        }
    }
    insertion_sort(a, first, last, lt);
}

/// The order ArcGIS's k-d tree leaves the points in (see the module notes).
fn kd_order(pts: &[[f64; 2]]) -> Vec<usize> {
    fn rec(a: &mut [usize], f: usize, l: usize, pts: &[[f64; 2]]) {
        let n = l - f;
        if n <= 16 {
            return;
        }
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for &i in &a[f..l] {
            x0 = x0.min(pts[i][0]);
            x1 = x1.max(pts[i][0]);
            y0 = y0.min(pts[i][1]);
            y1 = y1.max(pts[i][1]);
        }
        let d = if x1 - x0 >= y1 - y0 { 0 } else { 1 };
        let m = f + n / 2;
        nth_element(a, f, m, l, &|p, q| pts[p][d] < pts[q][d]);
        rec(a, f, m, pts);
        rec(a, m, l, pts);
    }
    let mut a: Vec<usize> = (0..pts.len()).collect();
    let n = a.len();
    rec(&mut a, 0, n, pts);
    a
}

/// The iteration order of a CPython set of these integers, added in this order.
fn pyset_order(items: &[i64]) -> Vec<i64> {
    fn hash(v: i64) -> u64 {
        // CPython's hash of an int: the value modulo 2^61 - 1, with -1 as -2.
        const M: i128 = (1 << 61) - 1;
        let mut h = (v as i128).abs() % M;
        if v < 0 {
            h = -h;
        }
        let h = if h == -1 { -2 } else { h };
        h as i64 as u64
    }
    fn insert(table: &mut [Option<i64>], v: i64) -> bool {
        let mask = table.len() as u64 - 1;
        let h = hash(v);
        let mut i = h & mask;
        let mut perturb = h;
        loop {
            let probes = if i + 9 <= mask { 9 } else { 0 };
            for j in i..=i + probes {
                match table[j as usize] {
                    None => {
                        table[j as usize] = Some(v);
                        return true;
                    }
                    Some(x) if x == v => return false,
                    _ => {}
                }
            }
            perturb >>= 5;
            i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb)) & mask;
        }
    }
    let mut table: Vec<Option<i64>> = vec![None; 8];
    let mut used = 0usize;
    for &v in items {
        if !insert(&mut table, v) {
            continue;
        }
        used += 1;
        let mask = table.len() - 1;
        if used * 5 >= mask * 3 {
            let mut size = 8;
            while size <= used * 4 {
                size <<= 1;
            }
            let old: Vec<i64> = table.iter().flatten().copied().collect();
            table = vec![None; size];
            for x in old {
                insert(&mut table, x);
            }
        }
    }
    table.into_iter().flatten().collect()
}

// ---------------------------------------------------------------------------------------------
// Generate Spatial Weights Matrix

/// Conceptualization of Spatial Relationships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwmConcept {
    InverseDistance,
    FixedDistance,
    KNearestNeighbors,
    ContiguityEdgesOnly,
    ContiguityEdgesCorners,
    Delaunay,
    SpaceTimeWindow,
    ConvertTable,
}

impl SwmConcept {
    pub fn parse(s: &str) -> Option<SwmConcept> {
        Some(match s.to_ascii_uppercase().replace(' ', "_").as_str() {
            "INVERSE_DISTANCE" => SwmConcept::InverseDistance,
            "FIXED_DISTANCE" => SwmConcept::FixedDistance,
            "K_NEAREST_NEIGHBORS" => SwmConcept::KNearestNeighbors,
            "CONTIGUITY_EDGES_ONLY" => SwmConcept::ContiguityEdgesOnly,
            "CONTIGUITY_EDGES_CORNERS" => SwmConcept::ContiguityEdgesCorners,
            "DELAUNAY_TRIANGULATION" => SwmConcept::Delaunay,
            "SPACE_TIME_WINDOW" => SwmConcept::SpaceTimeWindow,
            "CONVERT_TABLE" => SwmConcept::ConvertTable,
            _ => return None,
        })
    }

    fn wtype(self) -> u8 {
        match self {
            SwmConcept::InverseDistance => 0,
            SwmConcept::FixedDistance => 1,
            SwmConcept::KNearestNeighbors => 2,
            SwmConcept::Delaunay => 3,
            SwmConcept::ContiguityEdgesOnly => 4,
            SwmConcept::ContiguityEdgesCorners => 5,
            SwmConcept::ConvertTable => 6,
            SwmConcept::SpaceTimeWindow => 9,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kernel {
    Bisquare,
    Gaussian,
    Triangular,
    Quadratic,
}

impl Kernel {
    /// The weight at distance `d` with bandwidth `b`, computed as ArcGIS does (`u^2` as
    /// `d^2 / b^2`, to the last bit).
    fn at(self, d: f64, b: f64) -> f64 {
        let u2 = (d * d) / (b * b);
        match self {
            Kernel::Gaussian => (-0.5 * u2).exp() / (2.0 * std::f64::consts::PI).sqrt(),
            _ if u2 >= 1.0 => 0.0,
            Kernel::Bisquare => 15.0 / 16.0 * (1.0 - u2).powi(2),
            Kernel::Triangular => 1.0 - d / b,
            Kernel::Quadratic => 0.75 * (1.0 - u2),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kernel::Bisquare => "BISQUARE",
            Kernel::Gaussian => "GAUSSIAN",
            Kernel::Triangular => "TRIANGULAR",
            Kernel::Quadratic => "QUADRATIC",
        }
    }
}

/// How neighbors are weighted (the `weighting_method` parameter).
#[derive(Debug, Clone, PartialEq)]
pub enum Weighting {
    Unweighted,
    Kernel(Kernel),
    /// The neighbor's value of this field.
    Field(String),
    /// Shared border length (contiguity).
    Border,
}

impl Weighting {
    pub fn parse(s: &str, field: Option<&str>) -> Option<Weighting> {
        Some(match s.to_ascii_uppercase().as_str() {
            "" | "UNWEIGHTED" => Weighting::Unweighted,
            "BISQUARE" => Weighting::Kernel(Kernel::Bisquare),
            "GAUSSIAN" => Weighting::Kernel(Kernel::Gaussian),
            "TRIANGULAR" => Weighting::Kernel(Kernel::Triangular),
            "QUADRATIC" | "EPANECHNIKOV" => Weighting::Kernel(Kernel::Quadratic),
            "FIELD" => Weighting::Field(field?.to_string()),
            "BORDER" => Weighting::Border,
            _ => return None,
        })
    }
}

/// The interval unit of a space-time window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
    Years,
}

impl TimeUnit {
    pub fn parse(s: &str) -> Option<TimeUnit> {
        Some(match s.to_ascii_uppercase().as_str() {
            "SECONDS" => TimeUnit::Seconds,
            "MINUTES" => TimeUnit::Minutes,
            "HOURS" => TimeUnit::Hours,
            "DAYS" => TimeUnit::Days,
            "WEEKS" => TimeUnit::Weeks,
            "MONTHS" => TimeUnit::Months,
            "YEARS" => TimeUnit::Years,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            TimeUnit::Seconds => "SECONDS",
            TimeUnit::Minutes => "MINUTES",
            TimeUnit::Hours => "HOURS",
            TimeUnit::Days => "DAYS",
            TimeUnit::Weeks => "WEEKS",
            TimeUnit::Months => "MONTHS",
            TimeUnit::Years => "YEARS",
        }
    }

    /// The length in seconds (months of 30 days and years of 365, which ArcGIS may count
    /// differently; unchecked).
    fn seconds(self) -> f64 {
        match self {
            TimeUnit::Seconds => 1.0,
            TimeUnit::Minutes => 60.0,
            TimeUnit::Hours => 3600.0,
            TimeUnit::Days => 86400.0,
            TimeUnit::Weeks => 7.0 * 86400.0,
            TimeUnit::Months => 30.0 * 86400.0,
            TimeUnit::Years => 365.0 * 86400.0,
        }
    }
}

/// The parameters of Generate Spatial Weights Matrix.
#[derive(Clone)]
pub struct SwmOptions<'a> {
    pub unique_id: &'a str,
    pub concept: SwmConcept,
    pub method: DistanceMethod,
    pub exponent: f64,
    pub threshold: Option<f64>,
    pub neighbors: Option<usize>,
    pub row_standardize: bool,
    /// Convert table: a table of `<unique id>`, `NID` and `WEIGHT`.
    pub table: Option<&'a dyn FeatureClass>,
    pub time_field: Option<&'a str>,
    pub time_unit: Option<TimeUnit>,
    pub time_value: f64,
    /// Contiguity order (1: direct neighbors).
    pub order: usize,
    pub include_focal: bool,
    pub weighting: Weighting,
    /// Adaptive kernel bandwidths (the default) or a fixed one.
    pub adaptive: bool,
    pub adaptive_neighbors: Option<usize>,
    pub bandwidth: Option<f64>,
    /// The input's path as the header names it.
    pub input_path: &'a str,
}

impl Default for SwmOptions<'_> {
    fn default() -> Self {
        SwmOptions {
            unique_id: "",
            concept: SwmConcept::InverseDistance,
            method: DistanceMethod::Euclidean,
            exponent: 1.0,
            threshold: None,
            neighbors: None,
            row_standardize: true,
            table: None,
            time_field: None,
            time_unit: None,
            time_value: 0.0,
            order: 1,
            include_focal: false,
            weighting: Weighting::Unweighted,
            adaptive: true,
            adaptive_neighbors: None,
            bandwidth: None,
            input_path: "",
        }
    }
}

/// A feature as the weights see it.
struct Site {
    id: i64,
    xy: [f64; 2],
    geometry: unigis_core::Geometry,
    attrs: Vec<Value>,
}

/// A feature's center as ArcGIS computes it: points as they are, polygons' true centroid (the
/// area-weighted shoelace sums taken relative to the first vertex, which keeps the last bits ArcGIS
/// gets), other shapes' centroid.
pub(crate) fn center(g: &unigis_core::Geometry) -> Result<Option<[f64; 2]>> {
    match g.kind {
        GeometryKind::Point => Ok(g.xy.first().copied()),
        GeometryKind::Polygon if g.curves.is_empty() && !g.xy.is_empty() => {
            let o = g.xy[0];
            let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
            let n = g.xy.len();
            let mut starts: Vec<usize> = g.parts.iter().map(|p| *p as usize).collect();
            if starts.is_empty() {
                starts.push(0);
            }
            for (k, &st) in starts.iter().enumerate() {
                let e = starts.get(k + 1).copied().unwrap_or(n);
                for i in st..e.saturating_sub(1) {
                    let (x0, y0) = (g.xy[i][0] - o[0], g.xy[i][1] - o[1]);
                    let (x1, y1) = (g.xy[i + 1][0] - o[0], g.xy[i + 1][1] - o[1]);
                    let c = x0 * y1 - x1 * y0;
                    a += c;
                    cx += (x0 + x1) * c;
                    cy += (y0 + y1) * c;
                }
            }
            if a == 0.0 {
                return unigis_geometry::centroid(g);
            }
            Ok(Some([cx / (3.0 * a) + o[0], cy / (3.0 * a) + o[1]]))
        }
        _ => unigis_geometry::centroid(g),
    }
}

fn id_value(v: &Value) -> Option<i64> {
    match v {
        Value::SmallInteger(v) => Some(*v as i64),
        Value::Integer(v) => Some(*v as i64),
        Value::BigInteger(v) => Some(*v),
        _ => None,
    }
}

/// The features, their IDs from the unique ID field (or the OID), whether the IDs are 64-bit and
/// whether the field is the OID.
fn sites(input: &dyn FeatureClass, uid: &str) -> Result<(Vec<Site>, bool, bool)> {
    let is_oid = uid.eq_ignore_ascii_case(input.oid_field_name());
    let idx = if is_oid {
        None
    } else {
        let i = input
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(uid))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {uid} does not exist within table"
                ))
            })?;
        if !matches!(
            input.fields()[i].field_type,
            FieldType::SmallInteger | FieldType::Integer | FieldType::BigInteger
        ) {
            return Err(err(format!(
                "ERROR 000338: The unique ID field {uid} must be an integer field"
            )));
        }
        Some(i)
    };
    let id64 = idx.is_some_and(|i| input.fields()[i].field_type == FieldType::BigInteger);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        let Some(xy) = center(g)? else {
            continue;
        };
        let id = match idx {
            None => f.oid,
            Some(i) => match f.attributes.get(i).and_then(id_value) {
                Some(v) => v,
                None => continue,
            },
        };
        if !seen.insert(id) {
            return Err(err(format!(
                "ERROR 001271: The unique ID field {uid} has duplicate values ({id})"
            )));
        }
        out.push(Site {
            id,
            xy,
            geometry: g.clone(),
            attrs: f.attributes,
        });
    }
    if out.len() < 2 {
        return Err(err("ERROR 000841: at least two features are needed"));
    }
    Ok((out, id64, is_oid))
}

/// The name of a coordinate system, and its linear unit's name as ArcGIS writes it in messages.
fn crs_names(wkt: Option<&str>) -> (String, String) {
    let Some(wkt) = wkt else {
        return ("Unknown".into(), "units".into());
    };
    let quoted = |after: &str| -> Option<String> {
        let i = wkt.find(after)? + after.len();
        let rest = &wkt[i..];
        let s = rest.find('"')? + 1;
        let e = rest[s..].find('"')? + s;
        Some(rest[s..e].to_string())
    };
    let name = quoted("PROJCS[")
        .or_else(|| quoted("GEOGCS["))
        .unwrap_or_else(|| "Unknown".into());
    let unit = wkt
        .rfind("UNIT[")
        .and_then(|i| {
            let rest = &wkt[i + 5..];
            let s = rest.find('"')? + 1;
            let e = rest[s..].find('"')? + s;
            Some(rest[s..e].to_ascii_lowercase())
        })
        .unwrap_or_default();
    let unit = match unit.as_str() {
        "meter" | "metre" => "meters".to_string(),
        u if u.contains("foot") => "feet".to_string(),
        "degree" => "degrees".to_string(),
        "" => "units".to_string(),
        u => u.to_string(),
    };
    (name, unit)
}

fn exponent_text(e: f64) -> String {
    if e.fract() == 0.0 && e.abs() < 1e15 {
        format!("{}", e as i64)
    } else {
        format!("{e:.4}")
    }
}

/// A row of (neighbor index, raw weight).
type Row = Vec<(usize, f64)>;

/// Raw (unstandardized) neighbor weights per feature index.
type Rows = Vec<Row>;

fn knn(pts: &[[f64; 2]], i: usize, k: usize, method: DistanceMethod) -> Vec<(usize, f64)> {
    let mut d: Vec<(f64, usize)> = (0..pts.len())
        .filter(|j| *j != i)
        .map(|j| (method.between(pts[i], pts[j]), j))
        .collect();
    let k = k.min(d.len());
    if k < d.len() {
        d.select_nth_unstable_by(k, |a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        d.truncate(k + 1);
    }
    d.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    d.into_iter().take(k).map(|(dd, j)| (j, dd)).collect()
}

/// The `k`-th smallest distance from `i` to another feature (1-based).
fn kth_distance(pts: &[[f64; 2]], i: usize, k: usize, method: DistanceMethod) -> f64 {
    let mut d: Vec<f64> = (0..pts.len())
        .filter(|j| *j != i)
        .map(|j| method.between(pts[i], pts[j]))
        .collect();
    if d.is_empty() {
        return 0.0;
    }
    let k = k.clamp(1, d.len()) - 1;
    d.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    d[k]
}

/// Silverman's rule: `0.9 min(SD, sqrt(1/ln 2) Dm) n^-0.2`.
fn default_bandwidth(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len() as f64;
    let mx = pts.iter().map(|p| p[0]).sum::<f64>() / n;
    let my = pts.iter().map(|p| p[1]).sum::<f64>() / n;
    let sxx: f64 = pts.iter().map(|p| (p[0] - mx) * (p[0] - mx)).sum();
    let syy: f64 = pts.iter().map(|p| (p[1] - my) * (p[1] - my)).sum();
    let sd = (sxx / n + syy / n).sqrt();
    let mut d: Vec<f64> = pts.iter().map(|p| (p[0] - mx).hypot(p[1] - my)).collect();
    d.sort_by(|a, b| a.total_cmp(b));
    let m = d.len();
    let dm = if m % 2 == 1 {
        d[m / 2]
    } else {
        (d[m / 2 - 1] + d[m / 2]) / 2.0
    };
    0.9 * sd.min((1.0 / std::f64::consts::LN_2).sqrt() * dm) * n.powf(-0.2)
}

/// Neighbors within `band` of each feature, in the k-d tree's order.
fn within(
    pts: &[[f64; 2]],
    band: f64,
    method: DistanceMethod,
    keep: impl Fn(usize, usize) -> bool,
) -> Rows {
    let order = kd_order(pts);
    // A grid of cells as wide as the band finds the candidates.
    let cell = if band > 0.0 { band } else { 1.0 };
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let key = |p: [f64; 2]| ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64);
    for &i in &order {
        grid.entry(key(pts[i])).or_default().push(i);
    }
    let mut rank = vec![0usize; pts.len()];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    let mut rows = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let (cx, cy) = key(pts[i]);
        let mut row: Vec<(usize, f64)> = Vec::new();
        for gx in cx - 1..=cx + 1 {
            for gy in cy - 1..=cy + 1 {
                if let Some(c) = grid.get(&(gx, gy)) {
                    for &j in c {
                        if j == i {
                            continue;
                        }
                        let d = method.between(pts[i], pts[j]);
                        if d <= band && keep(i, j) {
                            row.push((j, d));
                        }
                    }
                }
            }
        }
        row.sort_by_key(|x| rank[x.0]);
        rows.push(row);
    }
    rows
}

/// Delaunay neighbors: Voronoi cells sharing an edge inside the convex hull.
fn delaunay(pts: &[[f64; 2]]) -> Result<Vec<Vec<usize>>> {
    use spade::{DelaunayTriangulation, Point2, Triangulation};
    let mut tri: DelaunayTriangulation<Point2<f64>> = DelaunayTriangulation::new();
    let mut vertex_site: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, p) in pts.iter().enumerate() {
        let h = tri
            .insert(Point2::new(p[0], p[1]))
            .map_err(|e| err(format!("Delaunay triangulation: {e:?}")))?;
        vertex_site.entry(h.index()).or_default().push(i);
    }
    let mut nb: Vec<Vec<usize>> = vec![Vec::new(); pts.len()];
    // Coincident features neighbor each other.
    for sites in vertex_site.values() {
        for &a in sites {
            for &b in sites {
                if a != b {
                    nb[a].push(b);
                }
            }
        }
    }
    let vpos: Vec<[f64; 2]> = tri
        .vertices()
        .map(|v| [v.position().x, v.position().y])
        .collect();
    let hull = convex_hull(&vpos);
    if hull.len() < 3 {
        return Err(err(
            "ERROR 000841: the features are collinear: no Delaunay triangulation",
        ));
    }
    // Opposite vertices of each undirected edge.
    let mut opp: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for e in tri.directed_edges() {
        let a = e.from().fix().index();
        let b = e.to().fix().index();
        if let Some(o) = e.opposite_vertex() {
            opp.entry((a.min(b), a.max(b)))
                .or_default()
                .push(o.fix().index());
        } else {
            opp.entry((a.min(b), a.max(b))).or_default();
        }
    }
    for ((a, b), o) in opp {
        let pa = vpos[a];
        let pb = vpos[b];
        let centers: Vec<[f64; 2]> = o.iter().map(|&c| circumcenter(pa, pb, vpos[c])).collect();
        let (p0, p1) = match centers.as_slice() {
            [c1, c2] => (*c1, *c2),
            [c] => {
                // A hull edge: the ray from the circumcenter away from the triangle.
                let t = [pb[0] - pa[0], pb[1] - pa[1]];
                let mut nrm = [-t[1], t[0]];
                let third = vpos[o[0]];
                let mid = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0];
                if nrm[0] * (third[0] - mid[0]) + nrm[1] * (third[1] - mid[1]) > 0.0 {
                    nrm = [-nrm[0], -nrm[1]];
                }
                let len = nrm[0].hypot(nrm[1]);
                let far = 1e3 * hull_size(&hull) / len;
                (*c, [c[0] + nrm[0] * far, c[1] + nrm[1] * far])
            }
            _ => continue,
        };
        if clip_length(p0, p1, &hull) > 0.0 {
            for &sa in &vertex_site[&a] {
                for &sb in &vertex_site[&b] {
                    nb[sa].push(sb);
                    nb[sb].push(sa);
                }
            }
        }
    }
    for r in &mut nb {
        r.sort_unstable();
        r.dedup();
    }
    Ok(nb)
}

fn hull_size(hull: &[[f64; 2]]) -> f64 {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in hull {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
    }
    (x1 - x0).max(y1 - y0).max(1.0)
}

fn circumcenter(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> [f64; 2] {
    // Relative to `a` for precision.
    let (bx, by) = (b[0] - a[0], b[1] - a[1]);
    let (cx, cy) = (c[0] - a[0], c[1] - a[1]);
    let d = 2.0 * (bx * cy - by * cx);
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    [
        a[0] + (cy * b2 - by * c2) / d,
        a[1] + (bx * c2 - cx * b2) / d,
    ]
}

/// The convex hull, counter-clockwise (Andrew's monotone chain).
fn convex_hull(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut p: Vec<[f64; 2]> = pts.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut h: Vec<[f64; 2]> = Vec::new();
    for &q in &p {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
    h
}

/// The length of the segment inside a counter-clockwise convex polygon (Cyrus-Beck).
fn clip_length(p0: [f64; 2], p1: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    let d = [p1[0] - p0[0], p1[1] - p0[1]];
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for k in 0..poly.len() {
        let a = poly[k];
        let b = poly[(k + 1) % poly.len()];
        let e = [b[0] - a[0], b[1] - a[1]];
        let nrm = [e[1], -e[0]];
        let num = nrm[0] * (a[0] - p0[0]) + nrm[1] * (a[1] - p0[1]);
        let den = nrm[0] * d[0] + nrm[1] * d[1];
        if den == 0.0 {
            if num < 0.0 {
                return 0.0;
            }
        } else {
            let t = num / den;
            if den > 0.0 {
                t1 = t1.min(t);
            } else {
                t0 = t0.max(t);
            }
        }
    }
    (t1 - t0).max(0.0) * d[0].hypot(d[1])
}

/// The boundary segments of a polygon.
fn segments(g: &unigis_core::Geometry) -> Vec<[[f64; 2]; 2]> {
    let mut out = Vec::new();
    let n = g.xy.len();
    let mut starts: Vec<usize> = g.parts.iter().map(|p| *p as usize).collect();
    if starts.is_empty() {
        starts.push(0);
    }
    for (k, &s) in starts.iter().enumerate() {
        let e = starts.get(k + 1).copied().unwrap_or(n);
        for i in s..e.saturating_sub(1) {
            if g.xy[i] != g.xy[i + 1] {
                out.push([g.xy[i], g.xy[i + 1]]);
            }
        }
    }
    out
}

/// How two polygons touch: the length of boundary they share and whether they touch at all.
fn shared(a: &[[[f64; 2]; 2]], b: &[[[f64; 2]; 2]], tol: f64) -> (f64, bool) {
    let mut length = 0.0;
    let mut touch = false;
    for s in a {
        let d = [s[1][0] - s[0][0], s[1][1] - s[0][1]];
        let len = d[0].hypot(d[1]);
        let u = [d[0] / len, d[1] / len];
        for t in b {
            // Envelope test.
            if s[0][0].min(s[1][0]) > t[0][0].max(t[1][0]) + tol
                || t[0][0].min(t[1][0]) > s[0][0].max(s[1][0]) + tol
                || s[0][1].min(s[1][1]) > t[0][1].max(t[1][1]) + tol
                || t[0][1].min(t[1][1]) > s[0][1].max(s[1][1]) + tol
            {
                continue;
            }
            // Collinear overlap: both of t's ends within tol of s's line.
            let off = |p: [f64; 2]| (p[0] - s[0][0]) * u[1] - (p[1] - s[0][1]) * u[0];
            let along = |p: [f64; 2]| (p[0] - s[0][0]) * u[0] + (p[1] - s[0][1]) * u[1];
            if off(t[0]).abs() <= tol && off(t[1]).abs() <= tol {
                let (a0, a1) = (along(t[0]), along(t[1]));
                let lo = a0.min(a1).max(0.0);
                let hi = a0.max(a1).min(len);
                if hi - lo > tol {
                    // The overlap's ends are vertices of one segment or the other.
                    let (tlo, thi) = if a0 <= a1 { (t[0], t[1]) } else { (t[1], t[0]) };
                    let p = if a0.min(a1) > 0.0 { tlo } else { s[0] };
                    let q = if a0.max(a1) < len { thi } else { s[1] };
                    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
                    length += (dx * dx + dy * dy).sqrt();
                    touch = true;
                    continue;
                }
                if hi - lo >= -tol {
                    touch = true;
                    continue;
                }
            }
            if !touch && segments_touch(s, t, tol) {
                touch = true;
            }
        }
    }
    (length, touch)
}

fn point_segment_distance(p: [f64; 2], s: &[[f64; 2]; 2]) -> f64 {
    let d = [s[1][0] - s[0][0], s[1][1] - s[0][1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 > 0.0 {
        (((p[0] - s[0][0]) * d[0] + (p[1] - s[0][1]) * d[1]) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - s[0][0] - t * d[0]).hypot(p[1] - s[0][1] - t * d[1])
}

fn segments_touch(s: &[[f64; 2]; 2], t: &[[f64; 2]; 2], tol: f64) -> bool {
    if point_segment_distance(s[0], t) <= tol
        || point_segment_distance(s[1], t) <= tol
        || point_segment_distance(t[0], s) <= tol
        || point_segment_distance(t[1], s) <= tol
    {
        return true;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let d1 = cross(t[0], t[1], s[0]);
    let d2 = cross(t[0], t[1], s[1]);
    let d3 = cross(s[0], s[1], t[0]);
    let d4 = cross(s[0], s[1], t[1]);
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Contiguity neighbors with their shared border lengths.
pub(crate) fn contiguity(
    geoms: &[&unigis_core::Geometry],
    corners: bool,
    tol: f64,
) -> Vec<Vec<(usize, f64)>> {
    use rstar::{AABB, RTree, RTreeObject};
    struct Env(usize, AABB<[f64; 2]>);
    impl RTreeObject for Env {
        type Envelope = AABB<[f64; 2]>;
        fn envelope(&self) -> Self::Envelope {
            self.1
        }
    }
    let segs: Vec<Vec<[[f64; 2]; 2]>> = geoms.iter().map(|g| segments(g)).collect();
    let envs: Vec<Env> = geoms
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let e = g.envelope();
            Env(
                i,
                AABB::from_corners([e.xmin - tol, e.ymin - tol], [e.xmax + tol, e.ymax + tol]),
            )
        })
        .collect();
    let tree = RTree::bulk_load(envs.iter().map(|e| Env(e.0, e.1)).collect());
    let mut out = vec![Vec::new(); geoms.len()];
    for i in 0..geoms.len() {
        for c in tree.locate_in_envelope_intersecting(&envs[i].1) {
            let j = c.0;
            if j <= i {
                continue;
            }
            let (len, touch) = shared(&segs[i], &segs[j], tol);
            if len > 0.0 || (corners && touch) {
                out[i].push((j, len));
                out[j].push((i, len));
            }
        }
    }
    for r in &mut out {
        r.sort_by_key(|x| x.0);
    }
    out
}

fn time_values(input: &dyn FeatureClass, sites: &[Site], field: &str) -> Result<Vec<f64>> {
    let i = input
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(field))
        .ok_or_else(|| {
            err(format!(
                "ERROR 000728: Field {field} does not exist within table"
            ))
        })?;
    sites
        .iter()
        .map(|s| match s.attrs.get(i) {
            Some(Value::Date(d)) => {
                Ok(d.and_utc().timestamp() as f64
                    + d.and_utc().timestamp_subsec_nanos() as f64 * 1e-9)
            }
            Some(Value::DateOnly(d)) => Ok(d
                .and_hms_opt(0, 0, 0)
                .map(|d| d.and_utc().timestamp() as f64)
                .unwrap_or(0.0)),
            Some(Value::TimestampOffset(d)) => Ok(d.timestamp() as f64),
            _ => Err(err(format!(
                "ERROR 110125: {field} has empty or non-date values"
            ))),
        })
        .collect()
}

/// Generate Spatial Weights Matrix: the matrix and the tool's messages.
pub fn generate_spatial_weights_matrix(
    input: &dyn FeatureClass,
    opt: &SwmOptions,
) -> Result<(Swm, Vec<String>)> {
    use SwmConcept as C;
    let (sites, id64, is_oid) = sites(input, opt.unique_id)?;
    let n = sites.len();
    let pts: Vec<[f64; 2]> = sites.iter().map(|s| s.xy).collect();
    let mut messages = Vec::new();
    let method = opt.method;
    let polygons = input.geometry_kind() == Some(GeometryKind::Polygon);
    let (crs_name, unit) = crs_names(input.spatial_reference_wkt());
    let distance_band = matches!(
        opt.concept,
        C::InverseDistance | C::FixedDistance | C::SpaceTimeWindow
    );
    if matches!(
        opt.concept,
        C::ContiguityEdgesOnly | C::ContiguityEdgesCorners
    ) && !polygons
    {
        return Err(err(
            "ERROR 000842: polygon contiguity needs polygon features",
        ));
    }
    let mut threshold = opt.threshold.filter(|t| *t > 0.0);
    if distance_band {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for s in &sites {
            let e = s.geometry.envelope();
            x0 = x0.min(e.xmin);
            x1 = x1.max(e.xmax);
            y0 = y0.min(e.ymin);
            y1 = y1.max(e.ymax);
        }
        match threshold {
            Some(t) if t >= (x1 - x0).max(y1 - y0) => {
                return Err(err(
                    "ERROR 000929: The threshold distance must be smaller than the feature class extent.",
                ));
            }
            Some(_) => {}
            None => {
                let t = (0..n)
                    .map(|i| kth_distance(&pts, i, 1, method))
                    .fold(0.0f64, f64::max)
                    * 1.0001;
                messages.push(format!(
                    "WARNING 000853: The default neighborhood search threshold was {t:.4} {unit}."
                ));
                threshold = Some(t);
            }
        }
    }
    let k = opt.neighbors.filter(|k| *k > 0);
    if let Some(k) = k
        && k >= n
    {
        return Err(err(format!(
            "ERROR 000876: the number of neighbors ({k}) must be less than the number of features ({n})"
        )));
    }
    let kernel = match &opt.weighting {
        Weighting::Kernel(k) => Some(*k),
        _ => None,
    };
    let fixed_bandwidth = || {
        opt.bandwidth
            .filter(|b| *b > 0.0)
            .unwrap_or_else(|| default_bandwidth(&pts))
    };
    // Raw rows: (neighbor, distance or border length), one per record; records are the features in
    // order except for a converted table.
    let mut rows: Rows;
    let mut record_ids: Vec<usize> = (0..n).collect();
    let mut bandwidths: Vec<f64> = Vec::new();
    match opt.concept {
        C::InverseDistance | C::FixedDistance | C::SpaceTimeWindow => {
            let band = threshold.unwrap_or(0.0);
            if opt.concept == C::SpaceTimeWindow {
                let field = opt
                    .time_field
                    .ok_or_else(|| err("ERROR 000735: Date/Time Field: value is required"))?;
                let unit = opt.time_unit.ok_or_else(|| {
                    err("ERROR 000735: Date/Time Interval Type: value is required")
                })?;
                let t = time_values(input, &sites, field)?;
                let window = unit.seconds() * opt.time_value;
                rows = within(&pts, band, method, |i, j| (t[i] - t[j]).abs() <= window);
            } else {
                rows = within(&pts, band, method, |_, _| true);
                if let Some(k) = k {
                    for (i, r) in rows.iter_mut().enumerate() {
                        if r.len() < k {
                            *r = knn(&pts, i, k, method);
                        }
                    }
                }
            }
            if kernel.is_some() && opt.concept == C::FixedDistance {
                bandwidths = vec![band; n];
            }
        }
        C::KNearestNeighbors => {
            let k = k.unwrap_or(8);
            rows = (0..n).map(|i| knn(&pts, i, k, method)).collect();
            if kernel.is_some() {
                bandwidths = if opt.adaptive {
                    let a = opt.adaptive_neighbors.filter(|a| *a > 0).unwrap_or(k + 1);
                    (0..n).map(|i| kth_distance(&pts, i, a, method)).collect()
                } else {
                    vec![fixed_bandwidth(); n]
                };
            }
        }
        C::Delaunay => {
            let nb = delaunay(&pts)?;
            rows = nb
                .into_iter()
                .enumerate()
                .map(|(i, r)| {
                    let order: Vec<i64> = r.iter().map(|j| *j as i64).collect();
                    pyset_order(&order)
                        .into_iter()
                        .map(|j| (j as usize, method.between(pts[i], pts[j as usize])))
                        .collect()
                })
                .collect();
            if kernel.is_some() {
                bandwidths = vec![fixed_bandwidth(); n];
            }
        }
        C::ContiguityEdgesOnly | C::ContiguityEdgesCorners => {
            let tol = xy_tolerance(input.spatial_reference_wkt());
            let geoms: Vec<&unigis_core::Geometry> = sites.iter().map(|s| &s.geometry).collect();
            let first = contiguity(&geoms, opt.concept == C::ContiguityEdgesCorners, tol);
            if opt.order > 1 {
                // Neighbors up to `order` steps away, as a Python set of their IDs.
                rows = Vec::with_capacity(n);
                for i in 0..n {
                    let mut seen = vec![false; n];
                    seen[i] = true;
                    let mut frontier = vec![i];
                    let mut all: Vec<usize> = Vec::new();
                    for _ in 0..opt.order {
                        let mut next = Vec::new();
                        for &f in &frontier {
                            for &(j, _) in &first[f] {
                                if !seen[j] {
                                    seen[j] = true;
                                    next.push(j);
                                }
                            }
                        }
                        all.extend(&next);
                        frontier = next;
                    }
                    if opt.include_focal && !all.is_empty() {
                        all.push(i);
                    }
                    all.sort_unstable();
                    let ids: Vec<i64> = all.iter().map(|j| sites[*j].id).collect();
                    let by_id: HashMap<i64, usize> =
                        all.iter().map(|j| (sites[*j].id, *j)).collect();
                    rows.push(
                        pyset_order(&ids)
                            .into_iter()
                            .map(|id| {
                                let j = by_id[&id];
                                let len = first[i].iter().find(|x| x.0 == j).map_or(0.0, |x| x.1);
                                (j, len)
                            })
                            .collect(),
                    );
                }
            } else {
                rows = first;
                if opt.include_focal {
                    for (i, r) in rows.iter_mut().enumerate() {
                        if !r.is_empty() {
                            r.push((i, 0.0));
                        }
                    }
                }
            }
            if opt.weighting == Weighting::Border {
                rows = rows
                    .into_iter()
                    .enumerate()
                    .map(|(i, r)| r.into_iter().filter(|x| x.1 > 0.0 || x.0 == i).collect())
                    .collect();
            } else {
                // Distances between centroids, for kernels.
                for (i, r) in rows.iter_mut().enumerate() {
                    for x in r.iter_mut() {
                        x.1 = method.between(pts[i], pts[x.0]);
                    }
                }
            }
            if kernel.is_some() {
                bandwidths = vec![fixed_bandwidth(); n];
            }
        }
        C::ConvertTable => {
            let t = opt
                .table
                .ok_or_else(|| err("ERROR 000735: Input Table: value is required"))?;
            let records = table_records(t, opt.unique_id, &sites)?;
            record_ids = records.iter().map(|r| r.0).collect();
            rows = records.into_iter().map(|r| r.1).collect();
        }
    }
    // Focal features (not for contiguity, done above).
    if opt.include_focal
        && !matches!(
            opt.concept,
            C::ContiguityEdgesOnly | C::ContiguityEdgesCorners | C::ConvertTable
        )
    {
        let order = kd_order(&pts);
        let mut rank = vec![0usize; n];
        for (r, &i) in order.iter().enumerate() {
            rank[i] = r;
        }
        for (i, r) in rows.iter_mut().enumerate() {
            if r.is_empty() {
                continue;
            }
            match opt.concept {
                C::KNearestNeighbors => r.insert(0, (i, 0.0)),
                C::InverseDistance | C::FixedDistance | C::SpaceTimeWindow => {
                    r.push((i, 0.0));
                    r.sort_by_key(|x| rank[x.0]);
                }
                _ => r.push((i, 0.0)),
            }
        }
    }
    // Weights.
    let field_values: Option<Vec<f64>> = match &opt.weighting {
        Weighting::Field(f) => {
            let fi = input
                .fields()
                .iter()
                .position(|x| x.name.eq_ignore_ascii_case(f))
                .ok_or_else(|| {
                    err(format!(
                        "ERROR 000728: Field {f} does not exist within table"
                    ))
                })?;
            Some(
                sites
                    .iter()
                    .map(|s| match s.attrs.get(fi) {
                        Some(Value::SmallInteger(v)) => *v as f64,
                        Some(Value::Integer(v)) => *v as f64,
                        Some(Value::BigInteger(v)) => *v as f64,
                        Some(Value::Single(v)) => *v as f64,
                        Some(Value::Double(v)) => *v,
                        _ => 0.0,
                    })
                    .collect(),
            )
        }
        _ => None,
    };
    let applies_kernel = kernel.is_some()
        && !matches!(
            opt.concept,
            C::InverseDistance | C::SpaceTimeWindow | C::ConvertTable
        );
    let mut fixed = true;
    let mut weighted: Rows = Vec::with_capacity(n);
    for (i, r) in rows.into_iter().enumerate() {
        let mut out = Vec::with_capacity(r.len());
        for (j, d) in r {
            let w = if opt.concept == C::ConvertTable {
                d
            } else if let Some(fv) = &field_values {
                fv[j]
            } else if applies_kernel {
                kernel.unwrap().at(d, bandwidths[i])
            } else if opt.weighting == Weighting::Border
                && matches!(
                    opt.concept,
                    C::ContiguityEdgesOnly | C::ContiguityEdgesCorners
                )
            {
                d
            } else if opt.concept == C::InverseDistance {
                1.0 / d.max(1.0).powf(opt.exponent)
            } else {
                1.0
            };
            if applies_kernel && w <= 0.0 {
                continue;
            }
            out.push((j, w));
        }
        weighted.push(out);
    }
    if opt.concept == C::ConvertTable
        || opt.concept == C::InverseDistance
        || applies_kernel
        || field_values.is_some()
        || opt.weighting == Weighting::Border
    {
        fixed = false;
    }
    let row_std = opt.row_standardize || field_values.is_some();
    let swm_rows: Vec<SwmRow> = weighted
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let sum = numpy_sum(&r.iter().map(|x| x.1).collect::<Vec<_>>());
            SwmRow {
                id: sites[record_ids[i]].id,
                neighbors: r.iter().map(|x| sites[x.0].id).collect(),
                weights: r
                    .iter()
                    .map(|x| {
                        if row_std && sum != 0.0 {
                            x.1 / sum
                        } else {
                            x.1
                        }
                    })
                    .collect(),
                sum,
            }
        })
        .collect();
    // Header.
    let mut h: HashMap<&str, String> = HashMap::new();
    h.insert("VERSION", "10.1".into());
    h.insert("UNIQUEID", opt.unique_id.to_string());
    h.insert("SPATIALREFNAME", crs_name);
    h.insert("INPUTFC", opt.input_path.to_string());
    h.insert("WTYPE", opt.concept.wtype().to_string());
    let table = opt.concept == C::ConvertTable;
    h.insert(
        "DISTANCEMETHOD",
        if table {
            "#".into()
        } else {
            match method {
                DistanceMethod::Euclidean => "EUCLIDEAN".into(),
                DistanceMethod::Manhattan => "MANHATTAN".into(),
            }
        },
    );
    if matches!(opt.concept, C::InverseDistance | C::FixedDistance) {
        h.insert("EXPONENT", exponent_text(opt.exponent));
    }
    if distance_band {
        h.insert("THRESHOLD", format!("{:.6}", threshold.unwrap_or(0.0)));
    }
    match opt.concept {
        C::KNearestNeighbors => {
            h.insert("NUMNEIGHS", k.unwrap_or(8).to_string());
        }
        C::ContiguityEdgesOnly | C::ContiguityEdgesCorners => {
            h.insert("NUMNEIGHS", "0".into());
        }
        _ => {}
    }
    if opt.concept == C::SpaceTimeWindow {
        h.insert("TIMEFIELD", opt.time_field.unwrap_or("").to_string());
        h.insert(
            "TIMETYPE",
            opt.time_unit.map_or("", |u| u.name()).to_string(),
        );
        h.insert("TIMEVALUE", exponent_text(opt.time_value));
    }
    h.insert("FIXEDWEIGHTS", if fixed { "True" } else { "False" }.into());
    h.insert("HASZ", if table { "False" } else { "None" }.into());
    h.insert("HASID64", if id64 { "True" } else { "False" }.into());
    h.insert("MASTERISOID", if is_oid { "True" } else { "False" }.into());
    if !(table || opt.concept == C::SpaceTimeWindow) {
        let (wf, wm) = match &opt.weighting {
            Weighting::Unweighted => ("None".to_string(), "UNWEIGHTED"),
            Weighting::Kernel(k) => ("None".to_string(), k.name()),
            Weighting::Field(f) => (f.clone(), "FIELD"),
            Weighting::Border => ("None".to_string(), "BORDER"),
        };
        h.insert("WEIGHTFIELD", wf);
        h.insert("WEIGHTINGMETHOD", wm.into());
    }
    h.insert("ISSELECTION", "False".into());
    h.insert("ISDEFINTIONQUERY", "False".into());
    h.insert(
        "INCLUDEFOCALFEATURE",
        if table {
            "False"
        } else if opt.include_focal {
            "True"
        } else {
            "None"
        }
        .into(),
    );
    let header = HEADER_KEYS
        .iter()
        .map(|k| (k.to_string(), h.remove(k).unwrap_or_else(|| "#".into())))
        .collect();
    let swm = Swm {
        header,
        row_standardized: row_std,
        rows: swm_rows,
        count: n,
    };
    messages.splice(
        0..0,
        [format!(
            "Constructing spatial weights based on {}....",
            match opt.concept {
                C::InverseDistance | C::FixedDistance => "distance criteria",
                C::KNearestNeighbors => "k-nearest neighbors",
                C::ContiguityEdgesOnly => "polygon contiguity (Edges Only)",
                C::ContiguityEdgesCorners => "polygon contiguity (Edges and Corners)",
                C::Delaunay => "Delaunay triangulation",
                C::SpaceTimeWindow => "space-time window",
                C::ConvertTable => "input table",
            }
        )],
    );
    messages.extend(summary(&swm));
    Ok((swm, messages))
}

/// NumPy's pairwise summation (`np.sum`), which ArcGIS's sums of weights follow to the last bit.
fn numpy_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut r = 0.0;
        for v in a {
            r += v;
        }
        r
    } else if n <= 128 {
        let mut r = [0.0f64; 8];
        r.copy_from_slice(&a[..8]);
        let mut i = 8;
        while i < n - n % 8 {
            for (j, rj) in r.iter_mut().enumerate() {
                *rj += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for v in &a[n - n % 8..] {
            res += v;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        numpy_sum(&a[..n2]) + numpy_sum(&a[n2..])
    }
}

/// The warnings about features without neighbors and the summary ArcGIS prints.
pub fn summary(swm: &Swm) -> Vec<String> {
    let mut out = Vec::new();
    let rows = &swm.rows[..swm.count.min(swm.rows.len())];
    let lonely: Vec<String> = rows
        .iter()
        .filter(|r| r.neighbors.is_empty())
        .map(|r| r.id.to_string())
        .collect();
    if !lonely.is_empty() {
        out.push(format!(
            "WARNING 000846: {} feature(s) had no neighbors which generally invalidates the statistical properties of a test.",
            lonely.len()
        ));
        out.push(format!(
            "WARNING 000847: Features with no neighbors (only includes first 30): {} = {}.",
            swm.unique_id(),
            lonely
                .iter()
                .take(30)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let n = rows.len().max(1);
    let counts: Vec<usize> = rows.iter().map(|r| r.neighbors.len()).collect();
    let total: usize = counts.iter().sum();
    let pct = total as f64 / (n * n) as f64 * 100.0;
    if pct > 30.0 {
        out.push("WARNING 110609: The spatial connectivity of your neighborhoods exceeds 30% globally. Estimates will be biased. Consider configuring smaller neighborhoods.".into());
    }
    let values = [
        ("Number of Features", rows.len().to_string()),
        ("Percentage of Spatial Connectivity", format!("{pct:.2}")),
        (
            "Average Number of Neighbors",
            format!("{:.2}", total as f64 / n as f64),
        ),
        (
            "Minimum Number of Neighbors",
            counts.iter().min().copied().unwrap_or(0).to_string(),
        ),
        (
            "Maximum Number of Neighbors",
            counts.iter().max().copied().unwrap_or(0).to_string(),
        ),
    ];
    let w = values.iter().map(|v| v.1.len()).max().unwrap_or(0);
    out.push(String::new());
    out.push(py_center("Spatial Weights Matrix Summary", 37 + w));
    for (k, v) in values {
        out.push(format!("{k:<38}{v:<w$}"));
    }
    out
}

/// Python's `str.center`.
fn py_center(s: &str, width: usize) -> String {
    let pad = width.saturating_sub(s.len());
    let left = pad / 2 + (pad & width & 1);
    format!("{}{s}{}", " ".repeat(left), " ".repeat(pad - left))
}

/// The XY tolerance of a coordinate system: 0.001 m in its units (0.0000000089831528 degrees for
/// geographic ones).
pub(crate) fn xy_tolerance(wkt: Option<&str>) -> f64 {
    match wkt {
        Some(w) if w.trim_start().starts_with("GEOGCS") => 8.983_152_841_195_214e-9,
        Some(w) if w.to_ascii_lowercase().contains("foot") => 0.001 / 0.3048,
        _ => 0.001,
    }
}

/// Convert table: the table's rows of `<unique id>`, `NID`, `WEIGHT` (IDs that are not features
/// are skipped), grouped as ArcGIS does: each run of rows with the same ID becomes one record, in
/// table order, so an ID that comes back later gets a second record (which ArcGIS writes after the
/// counted ones); features the table does not name follow without neighbors.
fn table_records(t: &dyn FeatureClass, uid: &str, sites: &[Site]) -> Result<Vec<(usize, Row)>> {
    let find = |name: &str| {
        t.fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                err(format!(
                    "ERROR 000728: Field {name} does not exist within table"
                ))
            })
    };
    let (iu, inid, iw) = (find(uid)?, find("NID")?, find("WEIGHT")?);
    let index: HashMap<i64, usize> = sites.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
    let mut groups: Vec<(usize, Vec<(usize, f64)>)> = Vec::new();
    for f in t.features() {
        let f = f?;
        let a = f.attributes.get(iu).and_then(id_value);
        let b = f.attributes.get(inid).and_then(id_value);
        let w = match f.attributes.get(iw) {
            Some(Value::Double(v)) => Some(*v),
            Some(Value::Single(v)) => Some(*v as f64),
            Some(v) => id_value(v).map(|v| v as f64),
            None => None,
        };
        let (Some(a), Some(b), Some(w)) = (a, b, w) else {
            continue;
        };
        let (Some(&i), Some(&j)) = (index.get(&a), index.get(&b)) else {
            continue;
        };
        match groups.last_mut() {
            Some(g) if g.0 == i => g.1.push((j, w)),
            _ => groups.push((i, vec![(j, w)])),
        }
    }
    let mut named = vec![false; sites.len()];
    for g in &groups {
        named[g.0] = true;
    }
    for (i, n) in named.iter().enumerate() {
        if !n {
            groups.push((i, Vec::new()));
        }
    }
    Ok(groups)
}

/// Convert Spatial Weights Matrix to Table: `<unique id>`, `NID`, `WEIGHT` for each neighbor pair
/// in the file's order (dBASE tables also get ArcGIS's empty `Field1`).
pub fn convert_spatial_weights_matrix_to_table(swm: &Swm, dbase: bool) -> TableOutput {
    let int = |name: &str| {
        let mut f = Field::new(name, FieldType::Integer);
        f.precision = 10;
        f
    };
    let mut fields = Vec::new();
    if dbase {
        fields.push(int("Field1"));
    }
    let id_type = if swm.id64() {
        FieldType::BigInteger
    } else {
        FieldType::Integer
    };
    let mut uid = int(swm.unique_id());
    uid.field_type = id_type;
    let mut nid = int("NID");
    nid.field_type = id_type;
    fields.push(uid);
    fields.push(nid);
    fields.push(Field::new("WEIGHT", FieldType::Double));
    let idv = |v: i64| {
        if swm.id64() {
            Value::BigInteger(v)
        } else {
            Value::Integer(v as i32)
        }
    };
    let mut rows = Vec::new();
    for r in swm.rows.iter().take(swm.count) {
        for (n, w) in r.neighbors.iter().zip(&r.weights) {
            let mut row = Vec::new();
            if dbase {
                row.push(Value::Integer(0));
            }
            row.push(idv(r.id));
            row.push(idv(*n));
            row.push(Value::Double(*w));
            rows.push(row);
        }
    }
    TableOutput { fields, rows }
}

/// Weights by feature index for the statistics tools: the file's rows matched to the features
/// through `ids` (in feature order). Features missing from the file have no neighbors.
/// With `raw` the weights of a row-standardized file are scaled back by the row sums the file
/// keeps (Hot Spot Analysis does this).
pub fn rows_for(swm: &Swm, ids: &[i64], raw: bool) -> Vec<Vec<(usize, f64)>> {
    let index: HashMap<i64, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut rows = vec![Vec::new(); ids.len()];
    for r in swm.rows.iter().take(swm.count) {
        let Some(&i) = index.get(&r.id) else { continue };
        let mut row: Vec<(usize, f64)> = r
            .neighbors
            .iter()
            .zip(&r.weights)
            .filter_map(|(n, w)| {
                let w = if raw && swm.row_standardized {
                    w * r.sum
                } else {
                    *w
                };
                index.get(n).map(|j| (*j, w))
            })
            .collect();
        row.sort_by_key(|x| x.0);
        rows[i] = row;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_set_order() {
        // CPython: list({106, 118, 109, 121, 130}) and two collisions.
        assert_eq!(
            pyset_order(&[106, 109, 118, 121, 130]),
            vec![130, 106, 109, 118, 121]
        );
        assert_eq!(pyset_order(&[0, 7, 23]), vec![0, 23, 7]);
        assert_eq!(pyset_order(&[3, 7, 8, 19]), vec![8, 19, 3, 7]);
    }

    #[test]
    fn nth_element_matches_sorting_below_33() {
        let v: Vec<f64> = (0..30).map(|i| ((i * 7919) % 31) as f64).collect();
        let mut a: Vec<usize> = (0..30).collect();
        nth_element(&mut a, 0, 15, 30, &|p, q| v[p] < v[q]);
        let mut s: Vec<usize> = (0..30).collect();
        s.sort_by(|p, q| v[*p].total_cmp(&v[*q]));
        assert_eq!(a, s);
    }

    #[test]
    fn bytes_round_trip() {
        let swm = Swm {
            header: HEADER_KEYS
                .iter()
                .map(|k| {
                    (
                        k.to_string(),
                        match *k {
                            "VERSION" => "10.1".to_string(),
                            "FIXEDWEIGHTS" => "True".to_string(),
                            _ => "#".to_string(),
                        },
                    )
                })
                .collect(),
            row_standardized: true,
            count: 2,
            rows: vec![
                SwmRow {
                    id: 7,
                    neighbors: vec![3, 11],
                    weights: vec![0.5, 0.5],
                    sum: 2.0,
                },
                SwmRow {
                    id: 3,
                    neighbors: vec![],
                    weights: vec![],
                    sum: 0.0,
                },
            ],
        };
        let b = swm.to_bytes();
        assert_eq!(Swm::from_bytes(&b).unwrap(), swm);
    }
}
