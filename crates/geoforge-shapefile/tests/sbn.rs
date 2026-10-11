//! Our `.sbn`/`.sbx` against the ones ArcGIS Pro wrote for the reference outputs.

use std::path::Path;

use geoforge_shapefile::sbn;

fn files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|e| e == "sbn") {
            out.push(p);
        }
    }
}

#[test]
fn matches_arcgis_indexes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/arcgis-reference");
    let mut sbns = Vec::new();
    files(&dir, &mut sbns);
    sbns.sort();
    let (mut same, mut total, mut differing) = (0, 0, Vec::new());
    for p in &sbns {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        // In-place tools (Repair Geometry, Append) leave ArcGIS's earlier index.
        if name.starts_with("repair_") || name.starts_with("append_") {
            continue;
        }
        let shp = std::fs::read(p.with_extension("shp")).unwrap();
        let (records, extent, entries) = sbn::entries_from_shp(&shp).unwrap();
        let (ours, ours_x) = sbn::build(records, extent, &entries);
        let (theirs, theirs_x) = (
            std::fs::read(p).unwrap(),
            std::fs::read(p.with_extension("sbx")).unwrap(),
        );
        total += 1;
        if ours == theirs && ours_x == theirs_x {
            same += 1;
        } else {
            differing.push(name);
        }
    }
    eprintln!("{same} of {total} indexes byte-identical; differing: {differing:?}");
    assert!(total > 50);
    assert_eq!(same, total, "differing: {differing:?}");
}
