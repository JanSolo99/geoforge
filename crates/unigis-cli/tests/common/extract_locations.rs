//! Extract Locations From Document: every field but the ones that name the machine and the
//! time (Filename, Modified, Scanned), and the points to 1e-7 degrees.
use std::path::Path;
use unigis_core::Value;
use unigis_shapefile::ShapefileReader;

const VOLATILE: [&str; 3] = ["Filename", "Modified", "Scanned"];

pub fn compare(ours: &Path, theirs: &Path) -> Vec<String> {
    let a = ShapefileReader::open(ours).unwrap();
    let b = ShapefileReader::open(theirs).unwrap();
    let mut diffs = Vec::new();
    if a.fields() != b.fields() {
        diffs.push(format!("fields {:?} != {:?}", a.fields(), b.fields()));
    }
    if a.spatial_reference_wkt() != b.spatial_reference_wkt() {
        diffs.push("CRS differs".into());
    }
    let rows = |fc: &ShapefileReader| fc.features().map(Result::unwrap).collect::<Vec<_>>();
    let (ra, rb) = (rows(&a), rows(&b));
    if ra.len() != rb.len() {
        diffs.push(format!("{} points != {}", ra.len(), rb.len()));
    }
    let names: Vec<&str> = b.fields().iter().map(|f| f.name.as_str()).collect();
    for (k, (x, y)) in ra.iter().zip(&rb).enumerate() {
        for ((name, u), v) in names.iter().zip(&x.attributes).zip(&y.attributes) {
            if !VOLATILE.contains(name) && u != v {
                diffs.push(format!("point {k} {name}: {u:?} != {v:?}"));
            }
        }
        let p = |f: &unigis_core::Feature| f.geometry.as_ref().map(|g| g.xy[0]);
        match (p(x), p(y)) {
            (Some(u), Some(v)) if (u[0] - v[0]).abs() < 1e-7 && (u[1] - v[1]).abs() < 1e-7 => {}
            (u, v) => diffs.push(format!("point {k} at {u:?} != {v:?}")),
        }
        let text = |f: &unigis_core::Feature, n: &str| {
            names
                .iter()
                .position(|m| *m == n)
                .and_then(|i| match &f.attributes[i] {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
        };
        // The file's own name is not volatile.
        if text(x, "Name") != text(y, "Name") {
            diffs.push(format!("point {k} Name differs"));
        }
    }
    diffs
}
