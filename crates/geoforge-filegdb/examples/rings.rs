fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = geoforge_filegdb::Geodatabase::open(&a[1])
        .unwrap()
        .open_table(&a[2])
        .unwrap();
    let g = t
        .get(a[3].parse().unwrap())
        .unwrap()
        .unwrap()
        .geometry
        .unwrap();
    let mut l: Vec<usize> = g.part_ranges().map(|r| r.len()).collect();
    l.sort();
    println!("ours {} {:?}", l.len(), l);
}
