//! Refusals preserve existing outputs; source formats retain allocation/CID semantics.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_core::{GeometryKind, Value};
use unigis_filegdb::GeodatabaseWriter;
use unigis_shapefile::ShapefileReader;
fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/ssl_areas.shp")
}
fn temp(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ug-sampling-{}-{label}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}
fn snapshot(path: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
fn run(input: &Path, out: &Path, params: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "create_spatial_sampling_locations"])
        .arg(format!("input={}", input.display()))
        .arg(format!("output={}", out.display()))
        .args(params)
        .output()
        .unwrap()
}
#[test]
fn refused_parameters_preserve_existing_output() {
    let dir = temp("refusals");
    let out = dir.join("existing.shp");
    for ext in ["shp", "shx", "dbf", "prj"] {
        std::fs::write(out.with_extension(ext), format!("existing {ext}")).unwrap();
    }
    let before = snapshot(&dir);
    for args in [
        vec!["num_samples=0"],
        vec!["num_samples=-1"],
        vec!["num_samples=1000001"],
        vec!["sampling_method=STRAT_ID"],
        vec![
            "sampling_method=STRAT_POLY",
            "strata_count_method=FIELD",
            "population_field=ABSENT",
        ],
        vec!["sampling_method=CLUSTER"],
        vec!["sampling_method=SYSTEMATIC", "bin_shape=H3_HEXAGON"],
        vec![
            "sampling_method=SYSTEMATIC",
            "bin_shape=SQUARE",
            "bin_size=-1 SquareMeters",
        ],
        vec!["seed=bad"],
    ] {
        let r = run(&source(), &out, &args);
        assert!(!r.status.success(), "{r:?}");
        assert_eq!(snapshot(&dir), before);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn filegdb_source_allocation_and_uniform_whole_area_sampling() {
    let dir = temp("filegdb");
    let gdb = dir.join("source.gdb");
    let shp = ShapefileReader::open(source()).unwrap();
    let mut w = GeodatabaseWriter::create(&gdb).unwrap();
    let mut t = w
        .create_table(
            "areas",
            Some(GeometryKind::Polygon),
            false,
            false,
            shp.fields(),
            shp.spatial_reference_wkt(),
        )
        .unwrap();
    for f in shp.features() {
        t.write(&f.unwrap()).unwrap();
    }
    w.finish_table(t).unwrap();
    w.finish().unwrap();
    let before = snapshot(&gdb);
    let input = gdb.join("areas");
    for (name, args, expected) in [
        (
            "equal",
            vec!["sampling_method=STRAT_POLY", "num_samples_per_strata=2"],
            BTreeMap::from([(1, 2), (2, 2), (3, 2)]),
        ),
        (
            "area",
            vec![
                "sampling_method=STRAT_POLY",
                "strata_count_method=PROP_AREA",
                "num_samples=7",
            ],
            BTreeMap::from([(0, 1), (1, 4), (2, 2)]),
        ),
    ] {
        let out = dir.join(format!("{name}.shp"));
        let r = run(&input, &out, &args);
        assert!(r.status.success(), "{r:?}");
        let actual = ShapefileReader::open(out).unwrap();
        let mut counts = BTreeMap::new();
        for row in actual.features() {
            let row = row.unwrap();
            let Value::Integer(cid) = row.attributes[0] else {
                panic!("CID type")
            };
            *counts.entry(cid).or_insert(0) += 1;
        }
        assert_eq!(counts, expected);
    }
    let out = dir.join("random.shp");
    let r = run(&input, &out, &["num_samples=600"]);
    assert!(r.status.success(), "{r:?}");
    let out = ShapefileReader::open(out).unwrap();
    let rows = out.features().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(rows.len(), 600);
    let mut bins = [0; 3];
    for row in rows {
        assert_eq!(row.attributes, [Value::Integer(1)]);
        let xy = row.geometry.unwrap().xy[0];
        assert!((5000000.0..=5000010.0).contains(&xy[1]));
        let x = xy[0] - 500000.0;
        let i = if (0.0..=10.0).contains(&x) {
            0
        } else if (20.0..=50.0).contains(&x) {
            1
        } else {
            assert!((60.0..=80.0).contains(&x));
            2
        };
        bins[i] += 1;
    }
    assert!(
        (65..135).contains(&bins[0])
            && (255..345).contains(&bins[1])
            && (160..240).contains(&bins[2]),
        "area-weighted occupancy {bins:?}"
    );
    assert_eq!(snapshot(&gdb), before);
    std::fs::remove_dir_all(dir).unwrap();
}
