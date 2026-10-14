//! Buffers written to a geodatabase keep their round parts as true circular arcs,
//! as ArcGIS Pro stores them (testdata/gdb-reference/buffers.gdb): the same vertices
//! and the same arcs (end points, midpoint, minor/major) up to where each ring starts.

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::{Geometry, geometry::SegmentKind};
use geoforge_filegdb::Geodatabase;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Vertices and arcs of a polygon, ring starts normalized away: every vertex, and
/// every arc as (start, interior point, minor?, end).
fn shape(g: &Geometry) -> (Vec<[f64; 2]>, Vec<[f64; 8]>) {
    let mut vertices = Vec::new();
    let mut arcs = Vec::new();
    for r in g.part_ranges() {
        let end = r.end - 1; // the closing vertex repeats the first
        vertices.extend_from_slice(&g.xy[r.start..end]);
        for s in &g.curves {
            let i = s.start as usize;
            if (r.start..end).contains(&i) {
                let SegmentKind::CircularArc { point, bits } = &s.kind else {
                    panic!("not a circular arc")
                };
                let next = g.xy[i + 1];
                arcs.push([
                    g.xy[i][0],
                    g.xy[i][1],
                    point[0],
                    point[1],
                    f64::from(bits & geoforge_core::geometry::ARC_MINOR != 0),
                    f64::from(bits & geoforge_core::geometry::ARC_CCW != 0),
                    next[0],
                    next[1],
                ]);
            }
        }
    }
    (vertices, arcs)
}

/// How many items of `a` and of `b` find no unused partner in the other within `tol`.
fn unmatched<const N: usize>(a: &[[f64; N]], b: &[[f64; N]], tol: f64) -> (usize, usize) {
    let mut used = vec![false; b.len()];
    let mut missing = 0;
    for x in a {
        match (0..b.len())
            .find(|&j| !used[j] && x.iter().zip(&b[j]).all(|(p, q)| (p - q).abs() <= tol))
        {
            Some(j) => used[j] = true,
            None => missing += 1,
        }
    }
    (missing, used.iter().filter(|u| !**u).count())
}

/// Known differences: (case, arcs only in one output, vertices only in one output).
/// ArcGIS keeps a 2° stretch of one 60 m circle as four plain vertices where the
/// union meets another circle's chord; GeoForge makes it part of an arc.
/// Where ArcGIS starts a polygon buffer's ring can fall inside a corner arc, which it
/// then stores as two arcs; the rule is not known, GeoForge starts rings at an arc's end.
const KNOWN: &[(&str, usize, usize)] = &[
    ("wells_60_all", 2, 5),
    ("parcels_10", 2, 2),
    ("parcels_neg3", 0, 1),
];

#[test]
fn buffers_into_geodatabase_have_arcs_like_arcgis() {
    let dir = std::env::temp_dir().join(format!("geoforge-buffer-arcs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cases: [(&str, &str, &[&str]); 5] = [
        ("wells_30", "wells", &["30"]),
        (
            "wells_200_all",
            "wells",
            &["200", "--dissolve-option", "ALL"],
        ),
        ("wells_60_all", "wells", &["60", "--dissolve-option", "ALL"]),
        ("parcels_10", "parcels", &["10"]),
        ("parcels_neg3", "parcels", &["-3"]),
    ];
    let reference = Geodatabase::open(root().join("testdata/gdb-reference/buffers.gdb")).unwrap();
    for (name, source, args) in cases {
        let gdb = dir.join(format!("{name}.gdb"));
        let run = Command::new(env!("CARGO_BIN_EXE_gf"))
            .arg("buffer")
            .arg(root().join(format!("testdata/geoproc/{source}.shp")))
            .arg(gdb.join(name))
            .args(args)
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        let ours = Geodatabase::open(&gdb).unwrap().open_table(name).unwrap();
        let theirs = reference.open_table(name).unwrap();
        let (a, b): (Vec<_>, Vec<_>) = (
            ours.features().map(|f| f.unwrap()).collect(),
            theirs.features().map(|f| f.unwrap()).collect(),
        );
        assert_eq!(a.len(), b.len(), "{name}: features");
        for (x, y) in a.iter().zip(&b) {
            let (gx, gy) = (x.geometry.as_ref().unwrap(), y.geometry.as_ref().unwrap());
            let ((vx, ax), (vy, ay)) = (shape(gx), shape(gy));
            assert_eq!(
                gx.parts.len(),
                gy.parts.len(),
                "{name} fid {}: rings",
                y.oid
            );
            let (arc_slack, vertex_slack) = KNOWN
                .iter()
                .find(|k| k.0 == name)
                .map_or((0, 0), |k| (k.1, k.2));
            let (m, n) = unmatched(&ax, &ay, 0.05);
            assert!(
                m <= arc_slack && n <= arc_slack,
                "{name} fid {}: arcs {m}/{n} differ",
                y.oid
            );
            let (m, n) = unmatched(&vx, &vy, 0.05);
            assert!(
                m <= vertex_slack && n <= vertex_slack,
                "{name} fid {}: vertices {m}/{n} differ",
                y.oid
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}
