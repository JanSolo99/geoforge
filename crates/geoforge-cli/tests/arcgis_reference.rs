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
        "near_wells_roads" => vec![
            "near",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp",
        ],
        "near_wells_roads_loc" => vec![
            "near",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--location",
            "LOCATION",
            "--angle",
            "ANGLE",
        ],
        "near_wells_roads_100" => vec![
            "near",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--search-radius",
            "100 Meters",
        ],
        "near_parcels_wells" => vec![
            "near",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--location",
            "LOCATION",
            "--angle",
            "ANGLE",
        ],
        "sj_wells_parcels" => vec![
            "spatial-join",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/parcels.shp",
        ],
        "sj_parcels_wells" => vec![
            "spatial-join",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/wells.shp",
        ],
        "sj_parcels_wells_common" => vec![
            "spatial-join",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--join-type",
            "KEEP_COMMON",
        ],
        "sj_parcels_wells_many" => vec![
            "spatial-join",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--join-operation",
            "JOIN_ONE_TO_MANY",
        ],
        "sj_roads_parcels" => vec![
            "spatial-join",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/parcels.shp",
        ],
        "sj_wells_roads_closest" => vec![
            "spatial-join",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--match-option",
            "CLOSEST",
            "--distance-field-name",
            "DIST",
        ],
        "sj_wells_parcels_within" => vec![
            "spatial-join",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--match-option",
            "WITHIN",
        ],
        "sj_parcels_wells_contains" => vec![
            "spatial-join",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--match-option",
            "CONTAINS",
        ],
        "sj_wells_roads_50" => vec![
            "spatial-join",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--match-option",
            "WITHIN_A_DISTANCE",
            "--search-radius",
            "50 Meters",
            "--distance-field-name",
            "DIST",
        ],
        "sel_parcels_intersect_clip" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "INTERSECT",
            "testdata/geoproc/clip_areas.shp",
        ],
        "sel_parcels_intersect_clip_invert" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "INTERSECT",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--invert-spatial-relationship",
            "INVERT",
        ],
        "sel_parcels_intersect_roads_10" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "INTERSECT",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--search-distance",
            "10 Meters",
        ],
        "sel_wells_dist_roads_30" => vec![
            "select-by-location",
            "testdata/geoproc/wells.shp",
            "WITHIN_A_DISTANCE",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--search-distance",
            "30 Meters",
        ],
        "sel_parcels_dist_wells_5" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "WITHIN_A_DISTANCE",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--search-distance",
            "5 Meters",
        ],
        "sel_parcels_within_clip" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "WITHIN",
            "testdata/geoproc/clip_areas.shp",
        ],
        "sel_parcels_completely_within_clip" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "COMPLETELY_WITHIN",
            "testdata/geoproc/clip_areas.shp",
        ],
        "sel_parcels_within_clementini_clip" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "WITHIN_CLEMENTINI",
            "testdata/geoproc/clip_areas.shp",
        ],
        "sel_wells_within_parcels" => vec![
            "select-by-location",
            "testdata/geoproc/wells.shp",
            "WITHIN",
            "testdata/geoproc/parcels.shp",
        ],
        "sel_wells_within_clementini_parcels" => vec![
            "select-by-location",
            "testdata/geoproc/wells.shp",
            "WITHIN_CLEMENTINI",
            "testdata/geoproc/parcels.shp",
        ],
        "sel_parcels_contains_wells" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "CONTAINS",
            "testdata/geoproc/wells.shp",
        ],
        "sel_parcels_completely_contains_wells" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "COMPLETELY_CONTAINS",
            "testdata/geoproc/wells.shp",
        ],
        "sel_parcels_contains_clementini_wells" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "CONTAINS_CLEMENTINI",
            "testdata/geoproc/wells.shp",
        ],
        "sel_parcels_center_in_clip" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "HAVE_THEIR_CENTER_IN",
            "testdata/geoproc/clip_areas.shp",
        ],
        "sel_parcels_touch_clip" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "BOUNDARY_TOUCHES",
            "testdata/geoproc/clip_areas.shp",
        ],
        "sel_parcels_crossed_by_roads" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "CROSSED_BY_THE_OUTLINE_OF",
            "testdata/geoproc/roads.shp",
        ],
        "sel_parcels_share_segment_clip" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "SHARE_A_LINE_SEGMENT_WITH",
            "testdata/geoproc/clip_areas.shp",
        ],
        "sel_roads_crossed_by_parcels" => vec![
            "select-by-location",
            "testdata/geoproc/roads.shp",
            "CROSSED_BY_THE_OUTLINE_OF",
            "testdata/geoproc/parcels.shp",
        ],
        "sel_parcels_identical_parcels" => vec![
            "select-by-location",
            "testdata/geoproc/parcels.shp",
            "ARE_IDENTICAL_TO",
            "testdata/geoproc/parcels.shp",
        ],
        "append_clip_to_parcels" => vec![
            "append",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "--schema-type",
            "NO_TEST",
            "--output",
        ],
        "append_parcels_to_clip" => vec![
            "append",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "--schema-type",
            "NO_TEST",
            "--output",
        ],
        "append_parcels_to_parcels" => vec![
            "append",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/parcels.shp",
            "--output",
        ],
        "near_wells_roads_clip" => vec![
            "near",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp;testdata/geoproc/clip_areas.shp",
            "<out>",
            "--location",
            "LOCATION",
            "--angle",
            "ANGLE",
        ],
        "split_parcels_clip" => vec![
            "split",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "AREA_ID",
        ],
        "split_roads_parcels_name" => vec![
            "split",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/parcels.shp",
            "NAME",
        ],
        "split_wells_parcels_name" => vec![
            "split",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/parcels.shp",
            "NAME",
        ],
        "split_parcels_parcels_name" => vec![
            "split",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/parcels.shp",
            "NAME",
        ],
        "intersect_roads_streams" => vec![
            "intersect",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/streams.shp",
        ],
        "intersect_roads_streams_point" => vec![
            "intersect",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/streams.shp",
            "<out>",
            "--output-type",
            "POINT",
        ],
        "intersect_roads_parcels_point" => vec![
            "intersect",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--output-type",
            "POINT",
        ],
        "intersect_parcels_clip_line" => vec![
            "intersect",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--output-type",
            "LINE",
        ],
        "intersect_parcels_clip_point" => vec![
            "intersect",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--output-type",
            "POINT",
        ],
        "intersect_parcels_clip_fid_only" => vec![
            "intersect",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--join-attributes",
            "ONLY_FID",
        ],
        "intersect_parcels_clip_no_fid" => vec![
            "intersect",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--join-attributes",
            "NO_FID",
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
        // Measured values (Near distances and angles) agree to ~1e-9: ArcGIS
        // computes them on its own coordinate grid.
        (Some(x), Some(y)) => x == y || (x - y).abs() <= 1e-11 * x.abs().max(1.0).max(1e5),
        _ => match (file_name(a), file_name(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}

/// Differences between our output and ArcGIS's for one output file.
fn compare(case: &str, ours: &Path, theirs: &Path, fields: &serde_json::Value) -> Vec<String> {
    let ours = ShapefileReader::open(ours).unwrap();
    let theirs = ShapefileReader::open(theirs).unwrap();
    let mut diffs = Vec::new();

    // Schema: names and types as ArcGIS reports them (FID and Shape excluded).
    let expected: Vec<(String, String)> = fields.as_array().unwrap()[2..]
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
    diffs
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
        // Workspace tools (Split) write into a folder.
        let out = if info.get("outputs").is_some() {
            let dir = out_dir.join(case);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        } else {
            out_dir.join(format!("{case}.shp"))
        };
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
        let mut diffs = Vec::new();
        if info["status"] == "failed" {
            // ArcGIS refused: gf must refuse with the same error code.
            let stderr = String::from_utf8_lossy(&run.stderr);
            let codes: Vec<&str> = info["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|m| m.as_str()?.strip_prefix("ERROR ")?.get(..6))
                .collect();
            if run.status.success() || !codes.iter().any(|c| stderr.contains(c)) {
                diffs.push(format!(
                    "ArcGIS failed with {codes:?}; gf: {}",
                    if run.status.success() {
                        "succeeded".into()
                    } else {
                        stderr
                    }
                ));
            }
        } else if let Some(outputs) = info["outputs"].as_object() {
            // Workspace outputs (Split): the same files, each matching.
            let mut ours: Vec<String> = std::fs::read_dir(&out)
                .unwrap()
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|n| n.ends_with(".shp"))
                .collect();
            ours.sort();
            let theirs: Vec<String> = outputs.keys().cloned().collect();
            if ours != theirs {
                diffs.push(format!("outputs: ours {ours:?}, ArcGIS {theirs:?}"));
            }
            for (name, o) in outputs {
                if out.join(name).exists() {
                    diffs.extend(
                        compare(
                            case,
                            &out.join(name),
                            &reference.join(case).join(name),
                            &o["fields"],
                        )
                        .into_iter()
                        .map(|d| format!("{name}: {d}")),
                    );
                }
            }
        } else if !run.status.success() {
            diffs.push(format!(
                "gf failed: {}",
                String::from_utf8_lossy(&run.stderr)
            ));
        } else {
            diffs = compare(
                case,
                &out,
                &reference.join(format!("{case}.shp")),
                &info["fields"],
            );
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
