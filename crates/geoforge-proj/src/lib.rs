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

    pub fn is_geographic(&self) -> bool {
        self.wkt.trim_start().starts_with("GEOGCS")
    }
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
        let utm = Crs::parse("32633").unwrap();
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
