//! `cargo run -p geoforge-filegdb --example curve -- <x.gdb> <layer> <oid>`
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let gdb = geoforge_filegdb::Geodatabase::open(&a[1]).unwrap();
    let t = gdb.open_table(&a[2]).unwrap();
    let f = t.get(a[3].parse().unwrap()).unwrap().unwrap();
    let g = f.geometry.unwrap();
    println!("parts {:?} npts {}", g.parts, g.xy.len());
    for s in &g.curves {
        let i = s.start as usize;
        println!("seg {:?}\n   from {:?} to {:?}", s, g.xy[i], g.xy[i + 1]);
    }
}
