//! `cargo run -p geoforge-filegdb --example mvals -- <x.gdb> <layer>`: prints M values.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let gdb = geoforge_filegdb::Geodatabase::open(&a[1]).unwrap();
    let t = gdb.open_table(&a[2]).unwrap();
    let d = t.geometry_def().unwrap();
    println!("morigin {} mscale {}", d.m_origin, d.m_scale);
    for f in t.features().take(3) {
        let g = f.unwrap().geometry.unwrap();
        let m = g.m.unwrap_or_default();
        println!("{:?}", &m[..m.len().min(8)]);
    }
}
