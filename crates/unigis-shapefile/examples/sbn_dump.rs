//! Writes our .sbn/.sbx for a shapefile to `<out_base>.sbn/.sbx` (for comparing with ArcGIS).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let shp = std::fs::read(&a[1]).unwrap();
    let (records, extent, entries) = unigis_shapefile::sbn::entries_from_shp(&shp).unwrap();
    unigis_shapefile::sbn::write(std::path::Path::new(&a[2]), records, extent, &entries).unwrap();
}
