//! Coordinate reference systems and transformations, through the system PROJ
//! library. CRSs are carried as Esri WKT (what `.prj` files and geodatabases hold);
//! transformations take coordinates in x/y order (longitude, latitude for
//! geographic systems), as ArcGIS stores them.

use geoforge_core::{Error, Result};
use proj::Proj;
use proj::wkt::WktVersion;

fn proj_error(what: &str, e: impl std::fmt::Display) -> Error {
    Error::Unsupported(format!("{what}: {e}"))
}

/// A coordinate reference system.
#[derive(Debug, Clone, PartialEq)]
pub struct Crs {
    wkt: String,
}

impl Crs {
    /// A CRS from what ArcGIS accepts as a coordinate system parameter: a factory
    /// code (EPSG, else Esri), a `.prj` file, or WKT.
    pub fn parse(spec: &str) -> Result<Self> {
        let spec = spec.trim();
        if let Ok(code) = spec.parse::<u32>() {
            for authority in ["EPSG", "ESRI"] {
                if let Ok(crs) = Self::from_definition(&format!("{authority}:{code}")) {
                    return Ok(crs);
                }
            }
            return Err(Error::Unsupported(format!(
                "ERROR 000622: unknown coordinate system factory code {code}"
            )));
        }
        let path = std::path::Path::new(spec);
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("prj"))
        {
            let wkt = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
            return Self::from_wkt(&wkt);
        }
        Self::from_definition(spec)
    }

    /// A CRS from Esri (or any) WKT, kept as given.
    pub fn from_wkt(wkt: &str) -> Result<Self> {
        Proj::new(wkt.trim()).map_err(|e| proj_error("coordinate system", e))?;
        Ok(Self {
            wkt: wkt.trim().to_owned(),
        })
    }

    /// Any definition PROJ understands (`EPSG:4326`, PROJJSON, ...), as Esri WKT.
    fn from_definition(def: &str) -> Result<Self> {
        let p = Proj::new(def).map_err(|e| proj_error(def, e))?;
        let wkt = p
            .as_wkt(Some(WktVersion::Wkt1_Esri), None)
            .map_err(|e| proj_error(def, e))?;
        Ok(Self {
            wkt: arcgis_style(&wkt),
        })
    }

    /// The CRS as ArcGIS writes it to a new dataset: PROJ's Esri WKT of it (ArcGIS
    /// regenerates the WKT rather than copying the input's text).
    pub fn arcgis_wkt(&self) -> String {
        Proj::new(&self.wkt)
            .ok()
            .and_then(|p| p.as_wkt(Some(WktVersion::Wkt1_Esri), None).ok())
            .map_or_else(|| self.wkt.clone(), |w| arcgis_style(&w))
    }

    /// The Esri WKT of this CRS.
    pub fn esri_wkt(&self) -> &str {
        &self.wkt
    }

    /// The Esri `wkid` and `latestWkid` of this CRS, as PROJ identifies it: an
    /// Esri code (e.g. 102100) with the EPSG one as the latest, or the EPSG code
    /// for both. `None` when PROJ finds no confident match.
    pub fn wkid(&self) -> Option<(u32, u32)> {
        let matches = identify(&self.wkt, None);
        let best = |authority: &str| {
            matches
                .iter()
                .filter(|(a, _, c)| a == authority && *c >= 70)
                .max_by_key(|(_, _, c)| *c)
                .map(|(_, code, c)| (*code, *c))
        };
        match (best("ESRI"), best("EPSG")) {
            (Some((esri, 100)), Some((epsg, c))) if c < 100 => Some((esri, epsg)),
            (Some((esri, 100)), None) => Some((esri, esri)),
            // Web Mercator keeps its older Esri code as the wkid in ArcGIS.
            (_, Some((3857, _))) => Some((102100, 3857)),
            (_, Some((epsg, _))) => Some((epsg, epsg)),
            (Some((esri, _)), None) => Some((esri, esri)),
            (None, None) => None,
        }
    }

    pub fn is_geographic(&self) -> bool {
        self.wkt.trim_start().starts_with("GEOGCS")
    }

    /// The Esri WKT ArcGIS stores in databases (GeoPackage spatial reference
    /// rows): PROJ's, with the degree unit to 15 digits.
    pub fn database_wkt(&self) -> String {
        Proj::new(&self.wkt)
            .ok()
            .and_then(|p| p.as_wkt(Some(WktVersion::Wkt1_Esri), None).ok())
            .unwrap_or_else(|| self.wkt.clone())
    }

    /// ArcGIS's default XY domain for this CRS, as (x origin, y origin,
    /// resolution). Geographic systems: origin -400, resolution 1e-9 degrees.
    /// Projected systems: resolution 0.0001 m in the linear unit; the origin is
    /// the minimum of the projection's horizon rounded down to a multiple of 100,
    /// less 100 (observed in ArcGIS Pro 3.7: exact for Mercator variants, Plate
    /// Carrée and the y origin of Transverse Mercator; Transverse Mercator's x
    /// origin is approximate). `None` for other projections.
    pub fn xy_domain(&self) -> Option<(f64, f64, f64)> {
        if self.is_geographic() {
            return Some((-400.0, -400.0, 1e-9));
        }
        let wkt = &self.wkt;
        let projection = quoted_after(wkt, "PROJECTION[")?;
        let param = |name: &str| -> f64 {
            wkt.find(&format!("PARAMETER[\"{name}\","))
                .and_then(|at| {
                    let rest = &wkt[at..];
                    let v = &rest[rest.find(',')? + 1..rest.find(']')?];
                    v.trim().parse().ok()
                })
                .unwrap_or(0.0)
        };
        let unit: f64 = {
            let at = wkt.rfind("UNIT[")?;
            let rest = &wkt[at..];
            rest[rest.find(',')? + 1..rest.find(']')?]
                .trim()
                .parse()
                .ok()?
        };
        let cm = param("Central_Meridian");
        let origin = |m: f64| (m / 100.0).floor() * 100.0 - 100.0;
        if projection == "Plate_Carree" {
            // ArcGIS projects Plate Carrée on a sphere of the semimajor axis.
            let at = wkt.find("SPHEROID[")?;
            let rest = &wkt[at..];
            let a: f64 = rest[rest.find(',')? + 1..]
                .split(',')
                .next()?
                .trim()
                .parse()
                .ok()?;
            let fe = param("False_Easting");
            let fn_ = param("False_Northing");
            let pi = std::f64::consts::PI;
            return Some((
                origin(fe - pi * a / unit),
                origin(fn_ - pi / 2.0 * a / unit),
                0.0001 / unit,
            ));
        }
        let (dlon, lat) = match projection.as_str() {
            "Transverse_Mercator" | "Gauss_Kruger" => (45.0, 90.0),
            "Mercator" | "Mercator_Auxiliary_Sphere" => (180.0, 89.0),
            "Equidistant_Cylindrical" => (180.0, 90.0),
            _ => return None,
        };
        let geogcs = bracketed(wkt, "GEOGCS[")?;
        let from = Crs::from_wkt(&geogcs).ok()?;
        let t = Transform::new(&from, self).ok()?;
        // The horizon's boundary, the equator and the central meridian.
        let mut pts = Vec::new();
        let steps = 720;
        for k in 0..=steps {
            let f = k as f64 / steps as f64;
            let lon = cm - dlon + 2.0 * dlon * f;
            let la = -lat + 2.0 * lat * f;
            pts.extend([
                [lon, -lat],
                [lon, lat],
                [lon, 0.0],
                [cm - dlon, la],
                [cm + dlon, la],
                [cm, la],
            ]);
        }
        t.apply(&mut pts).ok()?;
        let min = |i: usize| {
            pts.iter()
                .map(|p| p[i])
                .filter(|v| v.is_finite())
                .fold(f64::INFINITY, f64::min)
        };
        Some((origin(min(0)), origin(min(1)), 0.0001 / unit))
    }
}

/// A parsed WKT node: `KEYWORD[arg, ...]`, arguments kept as written.
#[derive(Debug, Clone)]
enum WktArg {
    Text(String),
    Raw(String),
    Node(WktNode),
}

#[derive(Debug, Clone)]
struct WktNode {
    keyword: String,
    args: Vec<WktArg>,
}

impl WktNode {
    fn parse(s: &str) -> Option<WktNode> {
        let mut chars = s.trim().char_indices().peekable();
        let node = Self::parse_at(s.trim(), &mut chars)?;
        Some(node)
    }

    fn parse_at(s: &str, it: &mut std::iter::Peekable<std::str::CharIndices>) -> Option<WktNode> {
        let start = it.peek()?.0;
        while it.peek().is_some_and(|&(_, c)| c != '[') {
            it.next();
        }
        let keyword = s[start..it.peek()?.0].trim().to_string();
        it.next(); // [
        let mut args = Vec::new();
        loop {
            while it
                .peek()
                .is_some_and(|&(_, c)| c == ',' || c.is_whitespace())
            {
                it.next();
            }
            match it.peek()?.1 {
                ']' => {
                    it.next();
                    return Some(WktNode { keyword, args });
                }
                '"' => {
                    it.next();
                    let mut text = String::new();
                    for (_, c) in it.by_ref() {
                        if c == '"' {
                            break;
                        }
                        text.push(c);
                    }
                    args.push(WktArg::Text(text));
                }
                _ => {
                    // A number / identifier, or a nested node.
                    let from = it.peek()?.0;
                    let mut end = from;
                    let mut nested = false;
                    while let Some(&(i, c)) = it.peek() {
                        if c == '[' {
                            nested = true;
                            break;
                        }
                        if c == ',' || c == ']' {
                            break;
                        }
                        end = i + c.len_utf8();
                        it.next();
                    }
                    if nested {
                        let mut sub = s[from..].char_indices().peekable();
                        let node = Self::parse_at(&s[from..], &mut sub)?;
                        // Advance the outer iterator past the nested node.
                        let consumed = sub.peek().map_or(s.len() - from, |&(i, _)| i);
                        while it.peek().is_some_and(|&(i, _)| i < from + consumed) {
                            it.next();
                        }
                        args.push(WktArg::Node(node));
                    } else {
                        args.push(WktArg::Raw(s[from..end].trim().to_string()));
                    }
                }
            }
        }
    }

    fn child(&self, keyword: &str) -> Option<&WktNode> {
        self.args.iter().find_map(|a| match a {
            WktArg::Node(n) if n.keyword == keyword => Some(n),
            _ => None,
        })
    }

    fn children<'a>(&'a self, keyword: &'a str) -> impl Iterator<Item = &'a WktNode> + 'a {
        self.args.iter().filter_map(move |a| match a {
            WktArg::Node(n) if n.keyword == keyword => Some(n),
            _ => None,
        })
    }

    fn name(&self) -> &str {
        match self.args.first() {
            Some(WktArg::Text(t)) => t,
            _ => "",
        }
    }

    fn raw(&self, k: usize) -> &str {
        match self.args.get(k) {
            Some(WktArg::Raw(r)) => r,
            _ => "",
        }
    }
}

impl Crs {
    /// The WKT2 ArcGIS stores beside the Esri WKT in GeoPackages
    /// (`definition_12_063`), for projected and geographic systems without
    /// vertical parts; `authority` and `code` give its ID.
    pub fn esri_wkt2(&self, authority: &str, code: u32) -> Option<String> {
        let wkt = self.database_wkt();
        let root = WktNode::parse(&wkt)?;
        let id = format!("ID[\"{authority}\",\"{code}\"]");
        let geog = |g: &WktNode, keyword: &str, with_id: bool| -> Option<String> {
            let datum = g.child("DATUM")?;
            let sph = datum.child("SPHEROID")?;
            let pm = g.child("PRIMEM")?;
            let unit = g.child("UNIT")?;
            let angle = format!("ANGLEUNIT[\"{}\",{}]", unit.name(), unit.raw(1));
            let mut out = format!(
                "{keyword}[\"{}\",DATUM[\"{}\",ELLIPSOID[\"{}\",{},{}]],PRIMEM[\"{}\",{}],\
                 CS[ellipsoidal,2],AXIS[\"Latitude (lat)\",north,ORDER[1],{angle}],\
                 AXIS[\"Longitude (lon)\",east,ORDER[2],{angle}]",
                g.name(),
                datum.name(),
                sph.name(),
                sph.raw(1),
                sph.raw(2),
                pm.name(),
                pm.raw(1)
            );
            if with_id {
                out += &format!(",{id}");
            }
            out.push(']');
            Some(out)
        };
        match root.keyword.as_str() {
            "GEOGCS" => geog(&root, "GEOGCRS", true),
            "PROJCS" => {
                let base = geog(root.child("GEOGCS")?, "BASEGEOGCRS", false)?;
                let projection = root.child("PROJECTION")?.name().to_string();
                let unit = root.child("UNIT")?;
                let length = format!("LENGTHUNIT[\"{}\",{}]", unit.name(), unit.raw(1));
                let metres = unit.name() == "Meter";
                let params: Vec<String> = root
                    .children("PARAMETER")
                    .map(|p| {
                        let linear = matches!(p.name(), "False_Easting" | "False_Northing");
                        if linear && !metres {
                            format!("PARAMETER[\"{}\",{},{length}]", p.name(), p.raw(1))
                        } else {
                            format!("PARAMETER[\"{}\",{}]", p.name(), p.raw(1))
                        }
                    })
                    .collect();
                Some(format!(
                    "PROJCRS[\"{}\",{base},CONVERSION[\"{projection}\",METHOD[\"{projection}\"],{}],\
                     CS[Cartesian,2],AXIS[\"Easting (X)\",east,ORDER[1],{length}],\
                     AXIS[\"Northing (Y)\",north,ORDER[2],{length}],{id}]",
                    root.name(),
                    params.join(",")
                ))
            }
            _ => None,
        }
    }
}

/// The first quoted string after `key`.
fn quoted_after(wkt: &str, key: &str) -> Option<String> {
    let at = wkt.find(key)? + key.len();
    let rest = &wkt[at..];
    let open = rest.find('"')? + 1;
    let close = open + rest[open..].find('"')?;
    Some(rest[open..close].to_string())
}

/// The bracketed WKT node starting with `key` (e.g. `GEOGCS[`).
fn bracketed(wkt: &str, key: &str) -> Option<String> {
    let start = wkt.find(key)?;
    let mut depth = 0;
    for (i, c) in wkt[start..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(wkt[start..start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// PROJ's matches for a CRS definition: (authority, code, confidence 0–100).
fn identify(def: &str, authority: Option<&str>) -> Vec<(String, u32, i32)> {
    use proj_sys::*;
    use std::ffi::{CStr, CString};
    let Ok(text) = CString::new(def) else {
        return Vec::new();
    };
    let auth = authority.and_then(|a| CString::new(a).ok());
    let auth_ptr = auth.as_ref().map_or(std::ptr::null(), |a| a.as_ptr());
    let mut out = Vec::new();
    // SAFETY: every PROJ object created here is destroyed before returning, and
    // the strings read from PROJ are copied while their objects are alive.
    unsafe {
        let ctx = proj_context_create();
        let pj = proj_create(ctx, text.as_ptr());
        if !pj.is_null() {
            let mut confidence: *mut std::os::raw::c_int = std::ptr::null_mut();
            let list = proj_identify(ctx, pj, auth_ptr, std::ptr::null(), &mut confidence);
            if !list.is_null() {
                for i in 0..proj_list_get_count(list) {
                    let obj = proj_list_get(ctx, list, i);
                    if obj.is_null() {
                        continue;
                    }
                    let (auth, code) = (proj_get_id_auth_name(obj, 0), proj_get_id_code(obj, 0));
                    if !auth.is_null() && !code.is_null() {
                        let auth = CStr::from_ptr(auth).to_string_lossy().into_owned();
                        let code = CStr::from_ptr(code).to_string_lossy().parse::<u32>();
                        let c = if confidence.is_null() {
                            0
                        } else {
                            *confidence.add(i as usize)
                        };
                        if let Ok(code) = code {
                            out.push((auth, code, c));
                        }
                    }
                    proj_destroy(obj);
                }
                proj_list_destroy(list);
            }
            if !confidence.is_null() {
                proj_int_list_destroy(confidence);
            }
            proj_destroy(pj);
        }
        proj_context_destroy(ctx);
    }
    out
}

/// PROJ's Esri WKT as ArcGIS Pro writes it to a new `.prj`: the degree unit to full
/// double precision.
fn arcgis_style(wkt: &str) -> String {
    wkt.replace(
        "UNIT[\"Degree\",0.0174532925199433]",
        "UNIT[\"Degree\",0.017453292519943295]",
    )
}

/// A transformation between two CRSs (PROJ picks the operation; no datum grid
/// shifts are configured).
pub struct Transform {
    pj: Proj,
}

impl Transform {
    pub fn new(from: &Crs, to: &Crs) -> Result<Self> {
        // new_known_crs normalizes axis order to x/y (longitude first).
        let pj = Proj::new_known_crs(&from.wkt, &to.wkt, None)
            .map_err(|e| proj_error("transformation", e))?;
        Ok(Self { pj })
    }

    /// Transforms coordinates in place.
    pub fn apply(&self, xy: &mut [[f64; 2]]) -> Result<()> {
        for p in xy.iter_mut() {
            let (x, y) = self
                .pj
                .convert((p[0], p[1]))
                .map_err(|e| proj_error("transforming coordinates", e))?;
            *p = [x, y];
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utm_round_trip() {
        let wgs84 = Crs::parse("4326").unwrap();
        assert!(wgs84.is_geographic());
        assert!(wgs84.esri_wkt().contains("0.017453292519943295"));
        assert_eq!(wgs84.wkid(), Some((4326, 4326)));
        let utm = Crs::parse("32633").unwrap();
        assert_eq!(utm.wkid(), Some((32633, 32633)));
        let prj = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../testdata/geoproc/roads.prj"
        ))
        .unwrap();
        assert_eq!(Crs::from_wkt(&prj).unwrap().wkid(), Some((32633, 32633)));
        assert_eq!(Crs::parse("3857").unwrap().wkid(), Some((102100, 3857)));
        assert!(
            utm.esri_wkt()
                .starts_with("PROJCS[\"WGS_1984_UTM_Zone_33N\"")
        );
        let mut xy = [[15.0, 45.0]];
        Transform::new(&wgs84, &utm)
            .unwrap()
            .apply(&mut xy)
            .unwrap();
        assert!((xy[0][0] - 500000.0).abs() < 1e-6, "{xy:?}");
        assert!((xy[0][1] - 4982950.400226552).abs() < 1e-6, "{xy:?}");
        Transform::new(&utm, &wgs84)
            .unwrap()
            .apply(&mut xy)
            .unwrap();
        assert!((xy[0][0] - 15.0).abs() < 1e-12 && (xy[0][1] - 45.0).abs() < 1e-12);
    }

    #[test]
    fn arcgis_xy_domains() {
        // (code, x origin, y origin) as ArcGIS Pro 3.7 reports them; Transverse
        // Mercator x origins are not exact yet.
        for (code, x, y) in [
            (4326, Some(-400.0), -400.0),
            (32633, None, -9998100.0),
            (32733, None, 1900.0),
            (27700, None, -15524400.0),
            (31467, None, -10001000.0),
            (3857, Some(-20037700.0), -30241100.0),
            (3395, Some(-20037700.0), -30198300.0),
            (32662, Some(-20037700.0), -10018900.0),
        ] {
            let d = Crs::parse(&code.to_string()).unwrap().xy_domain().unwrap();
            if let Some(x) = x {
                assert_eq!(d.0, x, "{code}");
            }
            assert_eq!(d.1, y, "{code}");
        }
        // ArcGIS's WKT2 (GeoPackage definition_12_063).
        let utm = Crs::parse("32633").unwrap();
        assert_eq!(
            utm.esri_wkt2("EPSG", 32633).unwrap(),
            "PROJCRS[\"WGS_1984_UTM_Zone_33N\",BASEGEOGCRS[\"GCS_WGS_1984\",DATUM[\"D_WGS_1984\",ELLIPSOID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],CS[ellipsoidal,2],AXIS[\"Latitude (lat)\",north,ORDER[1],ANGLEUNIT[\"Degree\",0.0174532925199433]],AXIS[\"Longitude (lon)\",east,ORDER[2],ANGLEUNIT[\"Degree\",0.0174532925199433]]],CONVERSION[\"Transverse_Mercator\",METHOD[\"Transverse_Mercator\"],PARAMETER[\"False_Easting\",500000.0],PARAMETER[\"False_Northing\",0.0],PARAMETER[\"Central_Meridian\",15.0],PARAMETER[\"Scale_Factor\",0.9996],PARAMETER[\"Latitude_Of_Origin\",0.0]],CS[Cartesian,2],AXIS[\"Easting (X)\",east,ORDER[1],LENGTHUNIT[\"Meter\",1.0]],AXIS[\"Northing (Y)\",north,ORDER[2],LENGTHUNIT[\"Meter\",1.0]],ID[\"EPSG\",\"32633\"]]"
        );
        assert_eq!(
            Crs::parse("4269").unwrap().esri_wkt2("EPSG", 4269).unwrap(),
            "GEOGCRS[\"GCS_North_American_1983\",DATUM[\"D_North_American_1983\",ELLIPSOID[\"GRS_1980\",6378137.0,298.257222101]],PRIMEM[\"Greenwich\",0.0],CS[ellipsoidal,2],AXIS[\"Latitude (lat)\",north,ORDER[1],ANGLEUNIT[\"Degree\",0.0174532925199433]],AXIS[\"Longitude (lon)\",east,ORDER[2],ANGLEUNIT[\"Degree\",0.0174532925199433]],ID[\"EPSG\",\"4269\"]]"
        );
        // (PROJ's Esri WKT of feet-based systems differs from ArcGIS's: "US survey
        // foot" for Foot_US, 15 digits, standard parallels swapped.)
        let ft = Crs::parse("2263").unwrap().xy_domain();
        assert!(ft.is_none() || (ft.unwrap().2 - 0.0003280833333333333).abs() < 1e-15);
    }
}
