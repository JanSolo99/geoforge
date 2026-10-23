fn main() {
    let t = unigis_filegdb::Table::open(std::env::args().nth(1).unwrap()).unwrap();
    println!("{:#?}", t.geometry_def());
    println!(
        "{:?}",
        t.features()
            .map(|f| f.unwrap().geometry)
            .collect::<Vec<_>>()
    );
}
