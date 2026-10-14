//! Prints the vertices and arcs of feature N (default the first) of a geodatabase feature class.
use geoforge_filegdb::Geodatabase;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = Geodatabase::open(&a[1]).unwrap().open_table(&a[2]).unwrap();
    let n: usize = a.get(3).map_or(0, |s| s.parse().unwrap());
    let f = t.features().nth(n).unwrap().unwrap();
    let g = f.geometry.unwrap();
    println!(
        "parts {:?} npts {} ncurves {}",
        g.parts,
        g.xy.len(),
        g.curves.len()
    );
    for (i, p) in g.xy.iter().enumerate() {
        let arc = g.curves.iter().find(|c| c.start as usize == i);
        println!(
            "{i:3} {:.4} {:.4} {}",
            p[0],
            p[1],
            arc.map_or(String::new(), |c| format!("{:?}", c.kind))
        );
    }
}
