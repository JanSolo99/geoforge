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

/// ArcGIS's index differs from ours in the rare small-subtree case documented in
/// docs/formats/shapefile-arcgis.md. Here node 2 holds the 4 features of its child 4's
/// subtree and of child 5's own, in the order 5's, then 4's; 5's deeper features stay in
/// place. Neither "subtree of at most 8" nor merging a node with its children's own
/// features reproduces it (the second breaks r2pt_dem_nodata).
const KNOWN_DIFFERENCES: &[&str] = &["f2p_label_polys_inside.sbn"];

#[test]
fn matches_arcgis_indexes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/arcgis-reference");
    let mut sbns = Vec::new();
    files(&dir, &mut sbns);
    sbns.sort();
    let (mut same, mut total, mut differing) = (0, 0, Vec::new());
    for p in &sbns {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        // In-place tools (Repair Geometry, Append, Delete Identical, field tools) leave ArcGIS's earlier index.
        if [
            "repair_",
            "append_",
            "deli_",
            "addfield_",
            "delfield_",
            "cf_",
        ]
        .iter()
        .any(|p| name.starts_with(p))
        {
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
    differing.retain(|n| !KNOWN_DIFFERENCES.contains(&n.as_str()));
    assert!(differing.is_empty(), "differing: {differing:?}");
}
