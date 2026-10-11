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
}
