//! Compares GeoForge's geoprocessing tools with ArcGIS Pro's output on the same
//! inputs. The ArcGIS results live in `testdata/arcgis-reference/` (regenerate with
//! `scripts/make-arcgis-reference.py` through the ArcGIS bridge), so this test runs
//! anywhere, including CI.

use std::path::Path;
use std::process::Command;

use geoforge_core::{FieldType, GeometryKind, Value};
use geoforge_shapefile::ShapefileReader;
use geoforge_testkit::{esri_area, esri_length, hausdorff};

/// The `gf` arguments reproducing each ArcGIS case.
fn gf_args(case: &str) -> Vec<&'static str> {
    let d = "testdata/geoproc";
    let _ = d;
    match case {
        "clip_parcels" => vec![
            "clip",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "clip_roads" => vec![
            "clip",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "clip_wells" => vec![
            "clip",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "buffer_roads_25" => vec!["buffer", "testdata/geoproc/roads.shp", "<out>", "25 Meters"],
        "buffer_roads_25_flat" => vec![
            "buffer",
            "testdata/geoproc/roads.shp",
            "<out>",
            "25 Meters",
            "--line-end-type",
            "FLAT",
        ],
        "buffer_roads_25_all" => vec![
            "buffer",
            "testdata/geoproc/roads.shp",
            "<out>",
            "25 Meters",
            "--dissolve-option",
            "ALL",
        ],
        "buffer_wells_30" => vec!["buffer", "testdata/geoproc/wells.shp", "<out>", "30 Meters"],
        "buffer_parcels_10" => vec![
            "buffer",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "10 Meters",
        ],
        "buffer_parcels_neg10" => vec![
            "buffer",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "-10 Meters",
        ],
        "intersect_parcels_clip" => vec![
            "intersect",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "intersect_roads_parcels" => vec![
            "intersect",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/parcels.shp",
        ],
        "intersect_wells_parcels" => vec![
            "intersect",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/parcels.shp",
        ],
        "erase_parcels" => vec![
            "erase",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "erase_roads" => vec![
            "erase",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "union_parcels_clip" => vec![
            "union",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "dissolve_parcels_zone" => vec![
            "dissolve",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--dissolve-field",
            "ZONE",
            "--statistics-fields",
            "VALUE SUM;PARCEL_ID COUNT",
        ],
        "dissolve_parcels_all" => vec!["dissolve", "testdata/geoproc/parcels.shp", "<out>"],
        "dissolve_parcels_zone_single" => vec![
            "dissolve",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--dissolve-field",
            "ZONE",
            "--multi-part",
            "SINGLE_PART",
        ],
        "dissolve_roads_class" => vec![
            "dissolve",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--dissolve-field",
            "CLASS",
        ],
        "identity_parcels_clip" => vec![
            "identity",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "identity_roads_parcels" => vec![
            "identity",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/parcels.shp",
        ],
        "identity_wells_clip" => vec![
            "identity",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "merge_parcels_clip" => vec![
            "merge",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "merge_parcels_clip_source" => vec![
            "merge",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--add-source",
            "ADD_SOURCE_INFO",
        ],
        other => panic!("no gf command for case {other}"),
    }
}

fn type_name(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "SmallInteger",
        FieldType::Integer => "Integer",
        FieldType::Single => "Single",
        FieldType::Double => "Double",
        FieldType::String => "String",
        FieldType::Date => "Date",
        _ => "other",
    }
}

fn same_value(a: &Value, b: &Value) -> bool {
    let num = |v: &Value| match v {
        Value::SmallInteger(n) => Some(f64::from(*n)),
        Value::Integer(n) => Some(f64::from(*n)),
        Value::Single(n) => Some(f64::from(*n)),
        Value::Double(n) => Some(*n),
        _ => None,
    };
    // MERGE_SRC holds the input path as each system spells it (ArcGIS: a Wine
    // path); compare the file names.
    let file_name = |v: &Value| match v {
        Value::String(s) if s.contains(['/', '\\']) => {
            s.rsplit(['/', '\\']).next().map(str::to_owned)
        }
        _ => None,
    };
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x == y || (x - y).abs() <= 1e-11 * x.abs().max(1.0),
        _ => match (file_name(a), file_name(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}

#[test]
fn tools_match_arcgis_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reference = root.join("testdata/arcgis-reference");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(reference.join("manifest.json")).unwrap())
            .unwrap();
    let out_dir = std::env::temp_dir().join(format!("gf-arcgis-ref-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut failures = Vec::new();
    for (case, info) in manifest["cases"].as_object().unwrap() {
        let out = out_dir.join(format!("{case}.shp"));
        let mut args: Vec<String> = gf_args(case).iter().map(|a| a.to_string()).collect();
        match args.iter().position(|a| a == "<out>") {
            Some(i) => args[i] = out.display().to_string(),
            None => args.push(out.display().to_string()),
        }
        let run = Command::new(env!("CARGO_BIN_EXE_gf"))
            .current_dir(&root)
            .args(&args)
            .output()
            .unwrap();
        if !run.status.success() {
            failures.push(format!(
                "{case}: gf failed: {}",
                String::from_utf8_lossy(&run.stderr)
            ));
            continue;
        }
        let ours = ShapefileReader::open(&out).unwrap();
        let theirs = ShapefileReader::open(reference.join(format!("{case}.shp"))).unwrap();
        let mut diffs = Vec::new();

        // Schema: names and types as ArcGIS reports them (FID and Shape excluded).
        let expected: Vec<(String, String)> = info["fields"].as_array().unwrap()[2..]
            .iter()
            .map(|f| {
                (
                    f[0].as_str().unwrap().to_owned(),
                    f[1].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let got: Vec<(String, String)> = ours
            .fields()
            .iter()
            .map(|f| (f.name.clone(), type_name(f.field_type).to_owned()))
            .collect();
        if got != expected {
            diffs.push(format!("schema: ours {got:?}, ArcGIS {expected:?}"));
        }

        let a: Vec<_> = ours.features().map(Result::unwrap).collect();
        let b: Vec<_> = theirs.features().map(Result::unwrap).collect();
        if a.len() != b.len() {
            diffs.push(format!("count: ours {}, ArcGIS {}", a.len(), b.len()));
        }
        // Clip agrees within ArcGIS's own XY tolerance (0.001 m: coordinates closer
        // than that are the same point to ArcGIS). Buffers are densified differently
        // (vertex placement on arcs), so compare shape within 2 × the 0.1 m deviation.
        let is_buffer = case.starts_with("buffer");
        let (shape_tol, area_tol) = if is_buffer {
            (0.2, 1e-4)
        } else {
            (0.001, 1e-7)
        };
        for (i, (fa, fb)) in a.iter().zip(&b).enumerate() {
            for (k, (va, vb)) in fa.attributes.iter().zip(&fb.attributes).enumerate() {
                if !same_value(va, vb) {
                    diffs.push(format!("feature {i} field {k}: ours {va:?}, ArcGIS {vb:?}"));
                }
            }
            let (Some(ga), Some(gb)) = (&fa.geometry, &fb.geometry) else {
                if fa.geometry.is_some() != fb.geometry.is_some() {
                    diffs.push(format!("feature {i}: null geometry mismatch"));
                }
                continue;
            };
            let h = hausdorff(ga, gb);
            if h > shape_tol {
                diffs.push(format!(
                    "feature {i}: shapes differ by {h:.6} (tolerance {shape_tol})"
                ));
            }
            let (ma, mb) = match ga.kind {
                GeometryKind::Polygon => (esri_area(ga), esri_area(gb)),
                GeometryKind::Polyline => (esri_length(ga), esri_length(gb)),
                _ => (0.0, 0.0),
            };
            // Within ArcGIS's 0.001 m XY tolerance an outline may move by that much,
            // changing the area by up to perimeter × 0.001.
            let slack = if ga.kind == GeometryKind::Polygon && !is_buffer {
                esri_length(gb) * 0.001
            } else {
                0.0
            };
            if (ma - mb).abs() > area_tol * mb.abs().max(1.0) + slack {
                diffs.push(format!(
                    "feature {i}: area/length ours {ma:.6}, ArcGIS {mb:.6}"
                ));
            }
        }
        if !diffs.is_empty() {
            failures.push(format!(
                "{case}:\n    {}",
                diffs
                    .iter()
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n    ")
            ));
        }
    }
    std::fs::remove_dir_all(&out_dir).ok();
    failures.iter().for_each(|f| eprintln!("✗ {f}"));
    assert!(
        failures.is_empty(),
        "{} cases differ from ArcGIS",
        failures.len()
    );
}
