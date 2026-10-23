//! `ug buffer` on geographic data: a distance in a linear unit is measured on the ellipsoid (ArcGIS's
//! geodesic buffer); a plain number or degrees is planar in degrees.

use std::path::{Path, PathBuf};
use std::process::Command;

use geographiclib_rs::{Geodesic, InverseGeodesic};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The vertices of the first polygon ogrinfo prints for a shapefile.
fn first_ring(shp: &Path) -> Vec<(f64, f64)> {
    let out = Command::new("ogrinfo")
        .args(["-al", "-geom=WKT"])
        .arg(shp)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = text
        .lines()
        .find(|l| {
            l.trim_start().starts_with("POLYGON") || l.trim_start().starts_with("MULTIPOLYGON")
        })
        .unwrap();
    let inner = line.split("((").nth(1).unwrap().split("))").next().unwrap();
    inner
        .split(',')
        .map(|p| {
            let mut v = p
                .trim()
                .trim_matches('(')
                .split_whitespace()
                .map(|x| x.parse::<f64>().unwrap());
            (v.next().unwrap(), v.next().unwrap())
        })
        .collect()
}

#[test]
fn linear_distances_on_geographic_data_are_geodesic() {
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let d = std::env::temp_dir().join(format!("gf-geobuf-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let shp = root().join("testdata/shp/point.shp");
    let run = |dist: &str, name: &str| {
        let out = d.join(name);
        let r = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["buffer"])
            .arg(&shp)
            .arg(&out)
            .arg(dist)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        out
    };
    // The first point of point.shp is (1, 2): every vertex is 1 km from it on the WGS 84 ellipsoid.
    let ring = first_ring(&run("1 Kilometers", "geodesic.shp"));
    let g = Geodesic::wgs84();
    assert_eq!(ring.len(), 224, "ArcGIS's 223 steps and the closing vertex");
    for (x, y) in &ring {
        let (s, _, _, _): (f64, f64, f64, f64) = g.inverse(2.0, 1.0, *y, *x);
        assert!((s - 1000.0).abs() < 1e-3, "{s}");
    }
    // Degrees: planar, a circle of radius 0.01 in the coordinates (ArcGIS gives 236 vertices).
    let ring = first_ring(&run("0.01 DecimalDegrees", "degrees.shp"));
    assert_eq!(ring.len(), 236);
    for (x, y) in &ring {
        assert!(((x - 1.0).hypot(y - 2.0) - 0.01).abs() < 1e-9);
    }
    let plain = first_ring(&run("0.01", "plain.shp"));
    assert_eq!(plain.len(), 236);
    std::fs::remove_dir_all(&d).ok();
}
