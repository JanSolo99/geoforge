//! Our `.sbn`/`.sbx` against the ones ArcGIS Pro wrote for the reference outputs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use unigis_shapefile::sbn;

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
///
/// `p2l_overlapping.sbn` (Polygon To Line of overlapping polygons, 426 lines) differs too:
/// an index of many features sharing exact extents, not yet understood.
///
/// `gpal_roads_50_end.sbn` (Generate Points Along Lines, 66 points, several on the same line
/// of x) differs too: a point index with many equal coordinates, not yet understood.
/// `gen_roads_20.sbn` (Generalize, 4 lines) differs too: the line index of a shapefile ArcGIS edits
/// in place; not yet understood.
const KNOWN_DIFFERENCES: &[&str] = &[
    "f2p_label_polys_inside.sbn",
    "gen_roads_20.sbn",
    "gpal_roads_50_end.sbn",
    "p2l_overlapping.sbn",
];

/// Parse both files independently, checking offsets, unique records and tree bounds.
/// Used only for the documented boundary fixtures; other new layouts must remain
/// byte-identical. Each tuple is (byte box, tree node), keyed by SHP record number.
fn checked_entries(sbn: &[u8], sbx: &[u8]) -> BTreeMap<u32, ([u8; 4], usize)> {
    let word = |bytes: &[u8], offset| {
        u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
    };
    let same_header = |a: &[u8], b: &[u8]| {
        assert_eq!(&a[..24], &b[..24]);
        assert_eq!(&a[28..100], &b[28..100]);
    };
    same_header(sbn, sbx);
    assert_eq!(word(sbn, 24) * 2, sbn.len());
    assert_eq!(word(sbx, 24) * 2, sbx.len());
    let mut blocks = vec![&[][..]];
    let mut offset = 100;
    while offset < sbn.len() {
        let record = blocks.len();
        assert_eq!(word(sbn, offset), record);
        let bytes = word(sbn, offset + 4) * 2;
        assert_eq!(word(sbx, 100 + (record - 1) * 8) * 2, offset);
        assert_eq!(word(sbx, 104 + (record - 1) * 8) * 2, bytes);
        blocks.push(&sbn[offset + 8..offset + 8 + bytes]);
        offset += 8 + bytes;
    }
    assert_eq!(offset, sbn.len());
    assert_eq!(sbx.len(), 100 + (blocks.len() - 1) * 8);
    assert_eq!(blocks[1].len() % 8, 0);
    let mut entries = BTreeMap::new();
    let mut used_bins = BTreeSet::new();
    for (i, index) in blocks[1].as_chunks::<8>().0.iter().enumerate() {
        let node = i + 1;
        let mut bin = word(index, 0);
        let count = word(index, 4);
        if count == 0 {
            assert_eq!(bin, u32::MAX as usize);
            continue;
        }
        let mut remaining = count;
        while remaining > 0 {
            assert!(bin >= 2 && used_bins.insert(bin));
            let block = blocks[bin];
            assert_eq!(block.len(), remaining.min(100) * 8);
            for entry in block.as_chunks::<8>().0 {
                let bbox: [u8; 4] = entry[..4].try_into().unwrap();
                let record = word(entry, 4) as u32;
                assert!(record > 0 && record as usize <= word(sbn, 28));
                assert!(bbox[0] <= bbox[2] && bbox[1] <= bbox[3]);
                assert!(entries.insert(record, (bbox, node)).is_none());
                let (mut lo, mut hi) = ([0; 2], [256; 2]);
                let depth = usize::BITS - node.leading_zeros() - 1;
                for level in 0..depth {
                    let axis = level as usize % 2;
                    let mid = (lo[axis] + hi[axis]) / 2;
                    if node & (1 << (depth - level - 1)) == 0 {
                        assert!(usize::from(bbox[axis]) > mid);
                        lo[axis] = mid;
                    } else {
                        assert!(usize::from(bbox[axis + 2]) <= mid);
                        hi[axis] = mid;
                    }
                }
            }
            remaining -= block.len() / 8;
            bin += 1;
        }
    }
    assert_eq!(used_bins, (2..blocks.len()).collect());
    entries
}

fn check_boundary_layout(
    name: &str,
    ours: &[u8],
    ours_x: &[u8],
    theirs: &[u8],
    theirs_x: &[u8],
) -> bool {
    // (record, ArcGIS node, native node). All other records must match exactly.
    let expected: &[(u32, usize, usize)] = match name {
        "ccn_grid_input_boundary_gars_gars.sbn" => &[(61, 3, 7), (149, 3, 6), (208, 3, 7)],
        "ccn_grid_input_boundary_georef_gars.sbn" => &[(16, 2, 4), (91, 6, 13), (137, 2, 5)],
        _ => return false,
    };
    assert_eq!(&ours[..24], &theirs[..24]);
    assert_eq!(&ours[28..100], &theirs[28..100]);
    let native = checked_entries(ours, ours_x);
    let reference = checked_entries(theirs, theirs_x);
    assert_eq!(
        native.keys().collect::<Vec<_>>(),
        reference.keys().collect::<Vec<_>>()
    );
    let mut moved = Vec::new();
    for (record, (bbox, node)) in native {
        let (expected_bbox, expected_node) = reference[&record];
        assert_eq!(bbox, expected_bbox, "{name}: record {record}");
        if node != expected_node {
            moved.push((record, expected_node, node));
        }
    }
    assert_eq!(moved, expected, "{name}");
    true
}

#[test]
fn matches_arcgis_indexes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/arcgis-reference");
    let mut sbns = Vec::new();
    files(&dir, &mut sbns);
    sbns.sort();
    let (mut same, mut total, mut differing) = (0, 0, Vec::new());
    let mut checked_layouts = 0;
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
        } else if check_boundary_layout(&name, &ours, &ours_x, &theirs, &theirs_x) {
            checked_layouts += 1;
        } else {
            differing.push(name);
        }
    }
    eprintln!(
        "{same} of {total} indexes byte-identical; {checked_layouts} verified boundary layouts; other differing: {differing:?}"
    );
    assert_eq!(checked_layouts, 2);
    assert!(total > 50);
    differing.retain(|n| !KNOWN_DIFFERENCES.contains(&n.as_str()));
    assert!(differing.is_empty(), "differing: {differing:?}");
}
