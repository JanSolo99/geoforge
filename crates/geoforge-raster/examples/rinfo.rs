//! Prints a raster's grid, NoData, EPSG and value range: `rinfo file.tif`.
fn main() {
    for p in std::env::args().skip(1) {
        match geoforge_raster::read_geotiff(std::path::Path::new(&p)) {
            Ok(r) => {
                let vals: Vec<f64> = r.bands[0]
                    .iter()
                    .copied()
                    .filter(|v| !r.is_nodata(*v))
                    .collect();
                let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
                let max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                println!(
                    "{p}: {}x{} {:?} bands={} transform={:?} nodata={:?} epsg={:?} min={min} max={max}",
                    r.width,
                    r.height,
                    r.data_type,
                    r.bands.len(),
                    r.transform,
                    r.nodata,
                    r.epsg
                );
            }
            Err(e) => println!("{p}: ERROR {e}"),
        }
    }
}
