//! `cargo run -p rustgis-data --example write_gdb -- out.gdb`: a sample layer written as a File
//! Geodatabase feature class, to open in other software.
use rustgis_data::{Dataset, Field, FieldType, Geometry, GeometryType, Value, io};
use rustgis_geom::Coord;

fn main() {
    let path = std::env::args().nth(1).expect("output .gdb path");
    let mut d = Dataset::new(
        "Parcels 1",
        GeometryType::Polygon,
        vec![Field::new("Lot No", FieldType::Integer), Field::new("owner", FieldType::Text)],
    );
    for i in 0..5 {
        let x = 10.0 + f64::from(i) * 0.01;
        let ring = vec![
            Coord::new(x, 50.0),
            Coord::new(x, 50.005),
            Coord::new(x + 0.005, 50.005),
            Coord::new(x + 0.005, 50.0),
            Coord::new(x, 50.0),
        ];
        d.push(Some(Geometry::Polygon(vec![vec![ring]])), vec![Value::Int(i64::from(i)), Value::Text(format!("owner {i}"))]);
    }
    io::write_path(&d, std::path::Path::new(&path)).expect("write");
}
