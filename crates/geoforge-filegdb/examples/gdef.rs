fn main() {
    let t = geoforge_filegdb::Table::open(std::env::args().nth(1).unwrap()).unwrap();
    println!("{:#?}", t.geometry_def());
    println!(
        "{:?}",
        t.features()
            .map(|f| f.unwrap().geometry)
            .collect::<Vec<_>>()
    );
}
