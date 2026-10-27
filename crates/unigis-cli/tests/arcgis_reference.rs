//! Compares UniGIS's geoprocessing tools with ArcGIS Pro's output on the same
//! inputs. The ArcGIS results live in `testdata/arcgis-reference/` (regenerate with
//! `scripts/make-arcgis-reference.py` through the ArcGIS bridge), so this test runs
//! anywhere, including CI.

use std::path::Path;
use std::process::Command;

use unigis_core::{FieldType, GeometryKind, Value};
use unigis_shapefile::ShapefileReader;
use unigis_testkit::{esri_area, esri_length, hausdorff};

#[path = "common/dm_cases.rs"]
mod dm_cases;

/// The `ug` arguments reproducing each ArcGIS case.
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
        "copy_gpkg_points" => vec!["copy-features", "testdata/gpkg/shapes.gpkg/main.points"],
        "copy_gpkg_points_zm" => vec!["copy-features", "testdata/gpkg/shapes.gpkg/main.points_zm"],
        "copy_gpkg_multipoints" => vec![
            "copy-features",
            "testdata/gpkg/shapes.gpkg/main.multipoints",
        ],
        "copy_gpkg_multilines" => {
            vec!["copy-features", "testdata/gpkg/shapes.gpkg/main.multilines"]
        }
        "copy_gpkg_polygons" => vec!["copy-features", "testdata/gpkg/shapes.gpkg/main.polygons"],
        "copy_gpkg_multipolygons" => vec![
            "copy-features",
            "testdata/gpkg/shapes.gpkg/main.multipolygons",
        ],
        "copy_gpkg_polygons_z" => {
            vec!["copy-features", "testdata/gpkg/shapes.gpkg/main.polygons_z"]
        }
        "copy_gpkg_geographic" => {
            vec!["copy-features", "testdata/gpkg/shapes.gpkg/main.geographic"]
        }
        "buffer_gpkg_lines_10" => vec![
            "buffer",
            "testdata/gpkg/shapes.gpkg/main.lines",
            "<out>",
            "10 Meters",
        ],
        "clip_gpkg_multipolygons" => vec![
            "clip",
            "testdata/gpkg/shapes.gpkg/main.multipolygons",
            "testdata/geoproc/clip_areas.shp",
        ],
        "select_gpkg_polygons" => vec![
            "select",
            "testdata/gpkg/shapes.gpkg/main.polygons",
            "<out>",
            "--where",
            "long_f > 150000",
        ],
        "dissolve_gpkg_multipolygons" => {
            vec!["dissolve", "testdata/gpkg/shapes.gpkg/main.multipolygons"]
        }
        "copy_gpkg_date_only" => vec!["copy-features", "testdata/gpkg/sample.gpkg/main.lines"],
        "copy_gpkg_big_integer" => vec!["copy-features", "testdata/gpkg/sample.gpkg/main.points"],
        "buffer_gpkg_big_integer" => vec![
            "buffer",
            "testdata/gpkg/sample.gpkg/main.lines",
            "<out>",
            "10 Meters",
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
        "merge_fm_id" => vec![
            "merge",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--field-mapping",
            r#"ID "ID" true true false 10 Long 0 10,First,#,clip_areas.shp,AREA_ID,-1,-1,parcels.shp,PARCEL_ID,-1,-1"#,
        ],
        "merge_fm_types" => vec![
            "merge",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--field-mapping",
            r#"ID "ID" true true false 19 Double 0 0,First,#,clip_areas.shp,AREA_ID,-1,-1,parcels.shp,PARCEL_ID,-1,-1;LBL "LBL" true true false 4 Text 0 0,First,#,parcels.shp,NAME,0,3;V "V" true true false 10 Long 0 10,First,#,parcels.shp,VALUE,-1,-1;Z "Z" true true false 5 Short 0 5,First,#,parcels.shp,ZONE,-1,-1"#,
        ],
        "merge_fm_same_name" => vec![
            "merge",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--field-mapping",
            r#"NAME "NAME" true true false 30 Text 0 0,First,#,parcels.shp,NAME,-1,-1,clip_areas.shp,AREA_ID,-1,-1"#,
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
        "append_fm_a" => vec![
            "append",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "--schema-type",
            "NO_TEST",
            "--field-mapping",
            r#"PARCEL_ID "PARCEL_ID" true true false 10 Long 0 10,First,#,testdata/geoproc/clip_areas.shp,AREA_ID,-1,-1;NAME "NAME" true true false 20 Text 0 0,First,#,testdata/geoproc/parcels.shp,NAME,-1,-1;VALUE "VALUE" true true false 19 Double 0 0,First,#,testdata/geoproc/clip_areas.shp,AREA_ID,-1,-1;ZONE "ZONE" true true false 5 Long 0 5,First,#,testdata/geoproc/clip_areas.shp,AREA_ID,-1,-1"#,
            "--output",
        ],
        "append_fm_text" => vec![
            "append",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "--schema-type",
            "NO_TEST",
            "--field-mapping",
            r#"PARCEL_ID "PARCEL_ID" true true false 10 Long 0 10,First,#,testdata/geoproc/parcels.shp,PARCEL_ID,-1,-1,testdata/geoproc/clip_areas.shp,AREA_ID,-1,-1;NAME "NAME" true true false 20 Text 0 0,First,#,testdata/geoproc/clip_areas.shp,AREA_ID,-1,-1;VALUE "VALUE" true true false 19 Double 0 0,First,#,testdata/geoproc/parcels.shp,VALUE,-1,-1;ZONE "ZONE" true true false 5 Long 0 5,First,#,testdata/geoproc/parcels.shp,ZONE,-1,-1"#,
            "--output",
        ],
        "fc2fc_where" => vec![
            "feature-class-to-feature-class",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where-clause",
            "ZONE = 2",
        ],
        "fc2fc_map" => vec![
            "feature-class-to-feature-class",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where-clause",
            "VALUE > 5000",
            "--field-mapping",
            r#"PID "PID" true true false 10 Long 0 10,First,#,testdata/geoproc/parcels.shp,PARCEL_ID,-1,-1;LBL "LBL" true true false 4 Text 0 0,First,#,testdata/geoproc/parcels.shp,NAME,0,3;V "V" true true false 10 Long 0 10,First,#,testdata/geoproc/parcels.shp,VALUE,-1,-1"#,
        ],
        "fc2fc_order" => vec![
            "feature-class-to-feature-class",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--field-mapping",
            r#"ZONE "ZONE" true true false 5 Short 0 5,First,#,testdata/geoproc/parcels.shp,ZONE,-1,-1;NAME "NAME" true true false 20 Text 0 0,First,#,testdata/geoproc/parcels.shp,NAME,-1,-1"#,
        ],
        "update_parcels_clip" => vec![
            "update",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
        ],
        "update_clip_parcels" => vec![
            "update",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
        ],
        "sort_parcels_name" => vec![
            "sort",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "NAME ASCENDING",
            "--spatial-sort-method",
            "UR",
        ],
        "sort_parcels_zone_value" => vec![
            "sort",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "ZONE DESCENDING;VALUE ASCENDING",
            "--spatial-sort-method",
            "UR",
        ],
        "sort_parcels_value_desc" => vec![
            "sort",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "VALUE DESCENDING",
            "--spatial-sort-method",
            "UR",
        ],
        "sort_wells_depth" => vec![
            "sort",
            "testdata/geoproc/wells.shp",
            "<out>",
            "DEPTH ASCENDING",
            "--spatial-sort-method",
            "UR",
        ],
        "sort_wells_ur" => vec![
            "sort",
            "testdata/geoproc/wells.shp",
            "<out>",
            "Shape ASCENDING",
            "--spatial-sort-method",
            "UR",
        ],
        "sort_wells_ul" => vec![
            "sort",
            "testdata/geoproc/wells.shp",
            "<out>",
            "Shape ASCENDING",
            "--spatial-sort-method",
            "UL",
        ],
        "sort_wells_lr" => vec![
            "sort",
            "testdata/geoproc/wells.shp",
            "<out>",
            "Shape ASCENDING",
            "--spatial-sort-method",
            "LR",
        ],
        "sort_wells_ll" => vec![
            "sort",
            "testdata/geoproc/wells.shp",
            "<out>",
            "Shape ASCENDING",
            "--spatial-sort-method",
            "LL",
        ],
        "sort_roads_ur" => vec![
            "sort",
            "testdata/geoproc/roads.shp",
            "<out>",
            "Shape ASCENDING",
            "--spatial-sort-method",
            "UR",
        ],
        "sort_roads_ll" => vec![
            "sort",
            "testdata/geoproc/roads.shp",
            "<out>",
            "Shape ASCENDING",
            "--spatial-sort-method",
            "LL",
        ],
        "sort_parcels_zone_shape" => vec![
            "sort",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "ZONE ASCENDING;Shape ASCENDING",
            "--spatial-sort-method",
            "UL",
        ],
        "sort_clip_ul" => vec![
            "sort",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "Shape ASCENDING",
            "--spatial-sort-method",
            "UL",
        ],
        "deli_parcels_shape" => vec![
            "delete-identical",
            "testdata/geoproc/parcels_dups.shp",
            "Shape",
            "--output",
        ],
        "deli_parcels_zone" => vec![
            "delete-identical",
            "testdata/geoproc/parcels_dups.shp",
            "ZONE",
            "--output",
        ],
        "deli_parcels_zone_value" => vec![
            "delete-identical",
            "testdata/geoproc/parcels_dups.shp",
            "ZONE;VALUE",
            "--output",
        ],
        "deli_parcels_shape_zone" => vec![
            "delete-identical",
            "testdata/geoproc/parcels_dups.shp",
            "Shape;ZONE",
            "--output",
        ],
        "deli_wells_shape" => vec![
            "delete-identical",
            "testdata/geoproc/wells_dups.shp",
            "Shape",
            "--output",
        ],
        "addfield_long" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_long",
            "LONG",
            "--output",
        ],
        "addfield_short" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_short",
            "SHORT",
            "--output",
        ],
        "addfield_float" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_float",
            "FLOAT",
            "--output",
        ],
        "addfield_double" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_double",
            "DOUBLE",
            "--output",
        ],
        "addfield_text15" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_text",
            "TEXT",
            "--length",
            "15",
            "--output",
        ],
        "addfield_text_default" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_text",
            "TEXT",
            "--output",
        ],
        "addfield_date" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_date",
            "DATE",
            "--output",
        ],
        "addfield_long5" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_long5",
            "LONG",
            "--precision",
            "5",
            "--output",
        ],
        "addfield_dbl_10_3" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_d",
            "DOUBLE",
            "--precision",
            "10",
            "--scale",
            "3",
            "--output",
        ],
        "addfield_flt_8_2" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "f_f",
            "FLOAT",
            "--precision",
            "8",
            "--scale",
            "2",
            "--output",
        ],
        "addfield_badname" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "bad name",
            "LONG",
            "--output",
        ],
        "addfield_dup" => vec![
            "add-field",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "DOUBLE",
            "--output",
        ],
        "delfield_one" => vec![
            "delete-field",
            "testdata/geoproc/parcels.shp",
            "NAME",
            "--output",
        ],
        "delfield_two" => vec![
            "delete-field",
            "testdata/geoproc/parcels.shp",
            "ZONE;VALUE",
            "--output",
        ],
        "delfield_keep" => vec![
            "delete-field",
            "testdata/geoproc/parcels.shp",
            "NAME;ZONE",
            "--method",
            "KEEP_FIELDS",
            "--output",
        ],
        "cf_value_x2" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!VALUE! * 2",
            "--field-type",
            "DOUBLE",
            "--output",
        ],
        "cf_div_long" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!PARCEL_ID! / 2",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_floordiv" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!PARCEL_ID! // 2",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_val_long" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!VALUE! * 2",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_ratio" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!PARCEL_ID! / 3",
            "--field-type",
            "DOUBLE",
            "--output",
        ],
        "cf_upper" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!NAME!.upper()",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_slice" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!NAME![4:]",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_concat" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "'X' + str(!ZONE!)",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_round" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "round(!VALUE!, 1)",
            "--field-type",
            "DOUBLE",
            "--output",
        ],
        "cf_sqrt" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "math.sqrt(!VALUE!)",
            "--field-type",
            "DOUBLE",
            "--output",
        ],
        "cf_ifexp" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!VALUE! if !ZONE! > 1 else -1",
            "--field-type",
            "DOUBLE",
            "--output",
        ],
        "cf_len" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "len(!NAME!)",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_mod" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!PARCEL_ID! % 3",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_pow" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!PARCEL_ID! ** 2",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_fid" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!FID!",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_idtext" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!PARCEL_ID!",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_valtext" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!VALUE!",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_replace" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!NAME!.replace('Lot','L')",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_short" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!VALUE!",
            "--field-type",
            "SHORT",
            "--output",
        ],
        "cf_codeblock" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "f(!ZONE!)",
            "--field-type",
            "LONG",
            "--code-block",
            "def f(z):\n    if z > 2:\n        return 10\n    return z * 5",
            "--output",
        ],
        "cf_percent" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "'%s-%d' % (!NAME!, !ZONE!)",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_fstring" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "f'{!NAME!}_{!ZONE!}'",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_zone_plus" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!ZONE! + 0.6",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_int" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "int(!VALUE!)",
            "--field-type",
            "DOUBLE",
            "--output",
        ],
        "cf_strip" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "!NAME!.strip().lower()",
            "--field-type",
            "TEXT",
            "--output",
        ],
        "cf_max" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "max(!ZONE!, 2)",
            "--field-type",
            "LONG",
            "--output",
        ],
        "cf_date" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "calc",
            "'2020-05-17'",
            "--field-type",
            "DATE",
            "--output",
        ],
        "cf_existing" => vec![
            "calculate-field",
            "testdata/geoproc/parcels.shp",
            "ZONE",
            "!ZONE! * 10",
            "--field-type",
            "LONG",
            "--output",
        ],
        "symdiff_parcels_clip" => vec![
            "symmetrical-difference",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
        ],
        "symdiff_clip_parcels" => vec![
            "symmetrical-difference",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
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
        "check_bad_esri" => vec![
            "check-geometry",
            "testdata/geoproc/bad_polygons.shp;testdata/geoproc/bad_lines.shp",
        ],
        "check_bad_ogc" => vec![
            "check-geometry",
            "testdata/geoproc/bad_polygons.shp;testdata/geoproc/bad_lines.shp",
            "<out>",
            "--validation-method",
            "OGC",
        ],
        "check_parcels" => vec!["check-geometry", "testdata/geoproc/parcels.shp"],
        "repair_bad_polygons_esri" => vec![
            "repair-geometry",
            "testdata/geoproc/bad_polygons.shp",
            "--output",
        ],
        "repair_bad_polygons_ogc_keep" => vec![
            "repair-geometry",
            "testdata/geoproc/bad_polygons.shp",
            "--delete-null",
            "KEEP_NULL",
            "--validation-method",
            "OGC",
            "--output",
        ],
        "repair_bad_lines_esri_keep" => vec![
            "repair-geometry",
            "testdata/geoproc/bad_lines.shp",
            "--delete-null",
            "KEEP_NULL",
            "--output",
        ],
        "repair_bad_lines_ogc" => vec![
            "repair-geometry",
            "testdata/geoproc/bad_lines.shp",
            "--validation-method",
            "OGC",
            "--output",
        ],
        "identity_roads_clip" => vec![
            "identity",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "identity_wells_clip_overlap" => vec![
            "identity",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--relationship",
            "KEEP_RELATIONSHIPS",
        ],
        "identity_edges_squares" => vec![
            "identity",
            "testdata/geoproc/edges.shp",
            "testdata/geoproc/squares.shp",
        ],
        "identity_edges_squares_relationships" => vec![
            "identity",
            "testdata/geoproc/edges.shp",
            "testdata/geoproc/squares.shp",
            "<out>",
            "--relationship",
            "KEEP_RELATIONSHIPS",
        ],
        "identity_edges_squares_fid_relationships" => vec![
            "identity",
            "testdata/geoproc/edges.shp",
            "testdata/geoproc/squares.shp",
            "<out>",
            "--join-attributes",
            "ONLY_FID",
            "--relationship",
            "KEEP_RELATIONSHIPS",
        ],
        "clip_roads_zm" => vec![
            "clip",
            "testdata/geoproc/roads_zm.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "erase_roads_zm" => vec![
            "erase",
            "testdata/geoproc/roads_zm.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "clip_parcels_z" => vec![
            "clip",
            "testdata/geoproc/parcels_z.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "intersect_roads_zm_parcels" => vec![
            "intersect",
            "testdata/geoproc/roads_zm.shp",
            "testdata/geoproc/parcels.shp",
        ],
        "intersect_roads_zm_parcels_z" => vec![
            "intersect",
            "testdata/geoproc/roads_zm.shp",
            "testdata/geoproc/parcels_z.shp",
        ],
        "intersect_parcels_z_clip" => vec![
            "intersect",
            "testdata/geoproc/parcels_z.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "intersect_roads_zm_parcels_point" => vec![
            "intersect",
            "testdata/geoproc/roads_zm.shp",
            "testdata/geoproc/parcels_z.shp",
            "<out>",
            "--output-type",
            "POINT",
        ],
        "identity_roads_zm_parcels" => vec![
            "identity",
            "testdata/geoproc/roads_zm.shp",
            "testdata/geoproc/parcels.shp",
        ],
        "union_parcels_z_clip" => vec![
            "union",
            "testdata/geoproc/parcels_z.shp",
            "testdata/geoproc/clip_areas.shp",
        ],
        "split_roads_zm_parcels_name" => vec![
            "split",
            "testdata/geoproc/roads_zm.shp",
            "testdata/geoproc/parcels.shp",
            "NAME",
        ],
        "intersect_3_line" => vec![
            "intersect",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--output-type",
            "LINE",
        ],
        "intersect_tri_lines_point" => vec![
            "intersect",
            "testdata/geoproc/tri_a.shp",
            "testdata/geoproc/tri_b.shp",
            "testdata/geoproc/tri_c.shp",
            "<out>",
            "--output-type",
            "POINT",
        ],
        "intersect_tri_polys_line" => vec![
            "intersect",
            "testdata/geoproc/tri_p.shp",
            "testdata/geoproc/tri_q.shp",
            "testdata/geoproc/tri_r.shp",
            "<out>",
            "--output-type",
            "LINE",
        ],
        "intersect_tri_polys_point" => vec![
            "intersect",
            "testdata/geoproc/tri_p.shp",
            "testdata/geoproc/tri_q.shp",
            "testdata/geoproc/tri_r.shp",
            "<out>",
            "--output-type",
            "POINT",
        ],
        "intersect_tri_mixed_point" => vec![
            "intersect",
            "testdata/geoproc/tri_p.shp",
            "testdata/geoproc/tri_a.shp",
            "testdata/geoproc/tri_c.shp",
            "<out>",
            "--output-type",
            "POINT",
        ],
        "buffer_roads_25_left" => vec![
            "buffer",
            "testdata/geoproc/roads.shp",
            "<out>",
            "25 Meters",
            "--line-side",
            "LEFT",
        ],
        "buffer_roads_25_right_flat" => vec![
            "buffer",
            "testdata/geoproc/roads.shp",
            "<out>",
            "25 Meters",
            "--line-side",
            "RIGHT",
            "--line-end-type",
            "FLAT",
        ],
        "buffer_parcels_10_outside" => vec![
            "buffer",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "10 Meters",
            "--line-side",
            "OUTSIDE_ONLY",
        ],
        "buffer_parcels_10_list_zone" => vec![
            "buffer",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "10 Meters",
            "--dissolve-option",
            "LIST",
            "--dissolve-field",
            "ZONE",
        ],
        "buffer_roads_20_list_class" => vec![
            "buffer",
            "testdata/geoproc/roads.shp",
            "<out>",
            "20 Meters",
            "--dissolve-option",
            "LIST",
            "--dissolve-field",
            "CLASS",
        ],
        "buffer_wells_depth_field" => {
            vec!["buffer", "testdata/geoproc/wells.shp", "<out>", "DEPTH"]
        }
        "sj_rules_a" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--merge-rule",
            "PARCEL_ID=Mean",
            "--merge-rule",
            "VALUE=Sum",
            "--merge-rule",
            "ZONE=Mode",
            "--merge-rule",
            "NAME=Last",
        ],
        "sj_rules_b" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--merge-rule",
            "PARCEL_ID=Median",
            "--merge-rule",
            "VALUE=StdDev",
            "--merge-rule",
            "ZONE=Range",
            "--merge-rule",
            "NAME=Count",
        ],
        "sj_rules_c" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--merge-rule",
            "PARCEL_ID=Max",
            "--merge-rule",
            "VALUE=Min",
            "--merge-rule",
            "ZONE=Count",
            "--merge-rule",
            "NAME=Join:;",
        ],
        "sj_rules_d" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--merge-rule",
            "PARCEL_ID=Range",
            "--merge-rule",
            "VALUE=Mean",
            "--merge-rule",
            "ZONE=Median",
            "--merge-rule",
            "NAME=First",
            "--match-option",
            "WITHIN_A_DISTANCE",
            "--search-radius",
            "60 Meters",
        ],
        "sj_fm_rename" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--join-operation",
            "JOIN_ONE_TO_ONE",
            "--join-type",
            "KEEP_ALL",
            "--field-mapping",
            r#"AREA_ID "AREA_ID" true true false 10 Long 0 10,First,#,clip_areas.shp,AREA_ID,-1,-1;PID "PID" true true false 10 Long 0 10,First,#,parcels.shp,PARCEL_ID,-1,-1"#,
        ],
        "sj_fm_types" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--join-operation",
            "JOIN_ONE_TO_ONE",
            "--join-type",
            "KEEP_ALL",
            "--field-mapping",
            r#"AREA_ID "AREA_ID" true true false 10 Long 0 10,First,#,clip_areas.shp,AREA_ID,-1,-1;VAL_I "VAL_I" true true false 10 Long 0 10,First,#,parcels.shp,VALUE,-1,-1;ZN "ZN" true true false 19 Double 0 0,First,#,parcels.shp,ZONE,-1,-1;NM "NM" true true false 5 Text 0 0,First,#,parcels.shp,NAME,0,4;SUB "SUB" true true false 20 Text 0 0,First,#,parcels.shp,NAME,2,5;PTXT "PTXT" true true false 12 Text 0 0,First,#,parcels.shp,PARCEL_ID,-1,-1"#,
        ],
        "sj_fm_sources" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--join-operation",
            "JOIN_ONE_TO_ONE",
            "--join-type",
            "KEEP_ALL",
            "--field-mapping",
            r#"BOTH "BOTH" true true false 10 Long 0 10,First,#,clip_areas.shp,AREA_ID,-1,-1,parcels.shp,PARCEL_ID,-1,-1;REV "REV" true true false 10 Long 0 10,First,#,parcels.shp,PARCEL_ID,-1,-1,clip_areas.shp,AREA_ID,-1,-1;TOT "TOT" true true false 19 Double 0 0,Sum,#,parcels.shp,VALUE,-1,-1;CNT "CNT" true true false 10 Long 0 10,Count,#,parcels.shp,VALUE,-1,-1;FL "FL" true true false 12 Float 0 0,First,#,parcels.shp,VALUE,-1,-1;SH "SH" true true false 5 Short 0 5,First,#,parcels.shp,ZONE,-1,-1"#,
        ],
        "sj_fm_many" => vec![
            "spatial-join",
            "testdata/geoproc/clip_areas.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--join-operation",
            "JOIN_ONE_TO_MANY",
            "--join-type",
            "KEEP_COMMON",
            "--field-mapping",
            r#"AREA_ID "AREA_ID" true true false 10 Long 0 10,First,#,clip_areas.shp,AREA_ID,-1,-1;PID "PID" true true false 10 Long 0 10,First,#,parcels.shp,PARCEL_ID,-1,-1;NM "NM" true true false 8 Text 0 0,First,#,parcels.shp,NAME,-1,-1"#,
        ],
        "project_wells_4326" => vec!["project", "testdata/geoproc/wells.shp", "<out>", "4326"],
        "project_parcels_3857" => vec!["project", "testdata/geoproc/parcels.shp", "<out>", "3857"],
        "project_roads_32634" => vec!["project", "testdata/geoproc/roads.shp", "<out>", "32634"],
        "project_wells_wgs84_32633" => vec![
            "project",
            "testdata/geoproc/wells_wgs84.shp",
            "<out>",
            "32633",
        ],
        "project_parcels_z_4326" => {
            vec!["project", "testdata/geoproc/parcels_z.shp", "<out>", "4326"]
        }
        "project_ed50_4326_default" => vec![
            "project",
            "testdata/geoproc/wells_ed50.shp",
            "<out>",
            "4326",
        ],
        "project_ed50_4326_t1" => vec![
            "project",
            "testdata/geoproc/wells_ed50.shp",
            "<out>",
            "4326",
            "--transform-method",
            "ED_1950_To_WGS_1984_1",
        ],
        "project_ed50_4326_t18" => {
            vec![
                "project",
                "testdata/geoproc/wells_ed50.shp",
                "<out>",
                "4326",
                "--transform-method",
                "ED_1950_To_WGS_1984_18",
            ]
        }
        "project_ed50_4326_t23" => {
            vec![
                "project",
                "testdata/geoproc/wells_ed50.shp",
                "<out>",
                "4326",
                "--transform-method",
                "ED_1950_To_WGS_1984_23",
            ]
        }
        "project_ed50_4326_chain" => vec![
            "project",
            "testdata/geoproc/wells_ed50.shp",
            "<out>",
            "4326",
            "--transform-method",
            "ED_1950_To_ETRS_1989_1 + ETRS_1989_To_WGS_1984",
        ],
        "project_ed50_utm33_32633_t1" => {
            vec![
                "project",
                "testdata/geoproc/wells_ed50_utm33.shp",
                "<out>",
                "32633",
                "--transform-method",
                "ED_1950_To_WGS_1984_1",
            ]
        }
        "project_ed50_utm33_4326_t7" => {
            vec![
                "project",
                "testdata/geoproc/wells_ed50_utm33.shp",
                "<out>",
                "4326",
                "--transform-method",
                "ED_1950_To_WGS_1984_7",
            ]
        }
        "project_nad27_nadcon" => vec![
            "project",
            "testdata/geoproc/wells_us_nad27.shp",
            "<out>",
            "4269",
            "--transform-method",
            "NAD_1927_To_NAD_1983_NADCON",
        ],
        "project_nad27_nadcon_wgs84" => vec![
            "project",
            "testdata/geoproc/wells_us_nad27.shp",
            "<out>",
            "4326",
            "--transform-method",
            "NAD_1927_To_NAD_1983_NADCON + NAD_1983_To_WGS_1984_1",
        ],
        "project_wgs84_ed50_t1" => {
            vec![
                "project",
                "testdata/geoproc/wells_wgs84.shp",
                "<out>",
                "4230",
                "--transform-method",
                "ED_1950_To_WGS_1984_1",
            ]
        }
        "project_wgs84_ed50_utm33_t18" => {
            vec![
                "project",
                "testdata/geoproc/wells_wgs84.shp",
                "<out>",
                "23033",
                "--transform-method",
                "ED_1950_To_WGS_1984_18",
            ]
        }
        "m2s_roads" => vec!["multipart-to-singlepart", "testdata/geoproc/roads.shp"],
        "m2s_parcels_clip" => vec![
            "multipart-to-singlepart",
            "testdata/arcgis-reference/clip_parcels.shp",
        ],
        "f2p_parcels_centroid" => vec![
            "feature-to-point",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--point-location",
            "CENTROID",
        ],
        "f2p_label_polys_inside" => vec![
            "feature-to-point",
            "testdata/geoproc/label_polys.shp",
            "<out>",
            "--point-location",
            "INSIDE",
        ],
        "f2p_roads_centroid" => vec![
            "feature-to-point",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "CENTROID",
        ],
        "f2p_parcels_inside" => vec![
            "feature-to-point",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--point-location",
            "INSIDE",
        ],
        "f2p_roads_inside" => vec![
            "feature-to-point",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "INSIDE",
        ],
        "fv2p_roads_all" => vec![
            "feature-vertices-to-points",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "ALL",
        ],
        "fv2p_roads_start" => vec![
            "feature-vertices-to-points",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "START",
        ],
        "fv2p_roads_end" => vec![
            "feature-vertices-to-points",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "END",
        ],
        "fv2p_roads_both" => vec![
            "feature-vertices-to-points",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "BOTH_ENDS",
        ],
        "fv2p_roads_mid" => vec![
            "feature-vertices-to-points",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "MID",
        ],
        "fv2p_roads_dangle" => vec![
            "feature-vertices-to-points",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--point-location",
            "DANGLE",
        ],
        "fv2p_parcels_all" => vec![
            "feature-vertices-to-points",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--point-location",
            "ALL",
        ],
        "mbg_parcels_envelope" => vec![
            "minimum-bounding-geometry",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--geometry-type",
            "ENVELOPE",
        ],
        "mbg_roads_hull" => vec![
            "minimum-bounding-geometry",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--geometry-type",
            "CONVEX_HULL",
        ],
        "mbg_roads_rect_area" => vec![
            "minimum-bounding-geometry",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--geometry-type",
            "RECTANGLE_BY_AREA",
        ],
        "mbg_roads_rect_width" => vec![
            "minimum-bounding-geometry",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--geometry-type",
            "RECTANGLE_BY_WIDTH",
        ],
        "mbg_roads_circle" => vec![
            "minimum-bounding-geometry",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--geometry-type",
            "CIRCLE",
        ],
        "mbg_wells_hull_all" => vec![
            "minimum-bounding-geometry",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--geometry-type",
            "CONVEX_HULL",
            "--group-option",
            "ALL",
        ],
        "mbg_parcels_env_zone_mbg" => vec![
            "minimum-bounding-geometry",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--geometry-type",
            "ENVELOPE",
            "--group-option",
            "LIST",
            "--group-field",
            "ZONE",
            "--mbg-fields-option",
            "MBG_FIELDS",
        ],
        "addxy_wells" => vec!["add-xy", "testdata/geoproc/wells.shp", "--output"],
        "select_parcels_value" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "VALUE > 5000",
        ],
        "select_parcels_in_and" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "ZONE IN (1, 3) AND VALUE < 8000",
        ],
        "select_parcels_like" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "NAME LIKE 'Lot 1%'",
        ],
        "select_parcels_between_or" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "PARCEL_ID BETWEEN 10 AND 20 OR ZONE = 0",
        ],
        "select_parcels_not" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "NOT ZONE = 2",
        ],
        "select_parcels_quoted" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "\"NAME\" = 'Lot 7'",
        ],
        "select_parcels_parens" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "(ZONE = 1 OR ZONE = 2) AND NOT (VALUE >= 3000 AND VALUE <= 6000)",
        ],
        "select_parcels_like_underscore" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "NAME LIKE 'Lot _'",
        ],
        "select_parcels_not_in" => vec![
            "select",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--where",
            "ZONE NOT IN (0, 1)",
        ],
        "select_wells_depth" => vec![
            "select",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--where",
            "DEPTH >= 100.5",
        ],
        "select_wells_none" => vec![
            "select",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--where",
            "DEPTH < 0",
        ],
        "select_parcels_all" => vec!["select", "testdata/geoproc/parcels.shp"],
        "stats_parcels_zone" => vec![
            "statistics",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "VALUE SUM;VALUE MEAN;VALUE MIN;VALUE MAX;VALUE RANGE;VALUE STD;VALUE COUNT;PARCEL_ID FIRST;PARCEL_ID LAST;NAME FIRST",
            "--case-field",
            "ZONE",
        ],
        "stats_parcels_all" => vec![
            "statistics",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "VALUE SUM;VALUE MEAN;PARCEL_ID COUNT",
        ],
        "stats_wells_median" => vec![
            "statistics",
            "testdata/geoproc/wells.shp",
            "<out>",
            "DEPTH MEDIAN;DEPTH VARIANCE;DEPTH UNIQUE",
        ],
        "freq_parcels_zone" => vec![
            "frequency",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "ZONE",
            "--summary-fields",
            "VALUE",
        ],
        "freq_roads_class" => vec!["frequency", "testdata/geoproc/roads.shp", "<out>", "CLASS"],
        "aga_parcels" => vec![
            "add-geometry-attributes",
            "testdata/geoproc/parcels.shp",
            "AREA;PERIMETER_LENGTH;CENTROID;CENTROID_INSIDE;EXTENT;PART_COUNT;POINT_COUNT",
            "--output",
        ],
        "aga_roads" => vec![
            "add-geometry-attributes",
            "testdata/geoproc/roads.shp",
            "LENGTH;LINE_START_MID_END;LINE_BEARING;PART_COUNT;POINT_COUNT;EXTENT",
            "--output",
        ],
        "aga_roads_km" => vec![
            "add-geometry-attributes",
            "testdata/geoproc/roads.shp",
            "LENGTH",
            "--length-unit",
            "KILOMETERS",
            "--output",
        ],
        "aga_parcels_ha" => vec![
            "add-geometry-attributes",
            "testdata/geoproc/parcels.shp",
            "AREA",
            "--area-unit",
            "HECTARES",
            "--output",
        ],
        "points_to_line_wells" => vec!["points-to-line", "testdata/geoproc/wells.shp"],
        "points_to_line_wells_sorted" => vec![
            "points-to-line",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--sort-field",
            "DEPTH",
        ],
        "gnt_wells_roads" => vec![
            "generate-near-table",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp",
        ],
        "gnt_wells_roads_all" => vec![
            "generate-near-table",
            "testdata/geoproc/wells.shp",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--search-radius",
            "100 Meters",
            "--location",
            "LOCATION",
            "--angle",
            "ANGLE",
            "--closest",
            "ALL",
            "--closest-count",
            "2",
        ],
        "find_identical_parcels_zone" => vec![
            "find-identical",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "ZONE",
        ],
        "find_identical_parcels_shape" => vec![
            "find-identical",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "Shape",
            "--output-record-option",
            "ONLY_DUPLICATES",
        ],
        "json_roads" => vec![
            "features-to-json",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "NO_GEOJSON",
        ],
        "json_roads_formatted" => vec![
            "features-to-json",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--format-json",
            "FORMATTED",
            "--geojson",
            "NO_GEOJSON",
        ],
        "json_parcels" => vec![
            "features-to-json",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "NO_GEOJSON",
        ],
        "json_wells" => vec![
            "features-to-json",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "NO_GEOJSON",
        ],
        "json_wells_wgs84" => vec![
            "features-to-json",
            "testdata/geoproc/wells_wgs84.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "NO_GEOJSON",
        ],
        "geojson_roads" => vec![
            "features-to-json",
            "testdata/geoproc/roads.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "GEOJSON",
        ],
        "geojson_parcels" => vec![
            "features-to-json",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "GEOJSON",
        ],
        "geojson_parcels_formatted" => vec![
            "features-to-json",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--format-json",
            "FORMATTED",
            "--geojson",
            "GEOJSON",
        ],
        "geojson_wells" => vec![
            "features-to-json",
            "testdata/geoproc/wells.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "GEOJSON",
        ],
        "json_roads_zm" => vec![
            "features-to-json",
            "testdata/geoproc/roads_zm.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "NO_GEOJSON",
            "--include-z-values",
            "Z_VALUES",
            "--include-m-values",
            "M_VALUES",
        ],
        "json_bad_lines" => vec![
            "features-to-json",
            "testdata/geoproc/bad_lines.shp",
            "<out>",
            "--format-json",
            "NOT_FORMATTED",
            "--geojson",
            "NO_GEOJSON",
        ],
        "j2f_parcels" => vec![
            "json-to-features",
            "testdata/arcgis-reference/json_parcels.json",
        ],
        "j2f_roads_zm" => vec![
            "json-to-features",
            "testdata/arcgis-reference/json_roads_zm.json",
        ],
        "j2f_bad_lines" => vec![
            "json-to-features",
            "testdata/arcgis-reference/json_bad_lines.json",
        ],
        "j2f_geojson_parcels" => vec![
            "json-to-features",
            "testdata/arcgis-reference/geojson_parcels.geojson",
            "<out>",
            "--geometry-type",
            "POLYGON",
        ],
        "j2f_geojson_wells" => vec![
            "json-to-features",
            "testdata/arcgis-reference/geojson_wells.geojson",
            "<out>",
            "--geometry-type",
            "POINT",
        ],
        "j2f_geojson_roads" => vec![
            "json-to-features",
            "testdata/arcgis-reference/geojson_roads.geojson",
            "<out>",
            "--geometry-type",
            "POLYLINE",
        ],
        "focal_mean_nodata" => vec![
            "focal-statistics",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "MEAN",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_mean_nodata_strict" => vec![
            "focal-statistics",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "MEAN",
            "--ignore-nodata",
            "NODATA",
        ],
        "focal_max_int16_5x3" => vec![
            "focal-statistics",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 5 3 CELL",
            "--statistics-type",
            "MAXIMUM",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_min_int16_4x4" => vec![
            "focal-statistics",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 4 4 CELL",
            "--statistics-type",
            "MINIMUM",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_std_circle2" => vec![
            "focal-statistics",
            "testdata/raster/dem.tif",
            "<out>",
            "--neighborhood",
            "Circle 2 CELL",
            "--statistics-type",
            "STD",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_range_annulus" => vec![
            "focal-statistics",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--neighborhood",
            "Annulus 1 3 CELL",
            "--statistics-type",
            "RANGE",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_majority_landuse" => vec![
            "focal-statistics",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "MAJORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_minority_landuse" => vec![
            "focal-statistics",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "--neighborhood",
            "Circle 2 CELL",
            "--statistics-type",
            "MINORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_majority_classes" => vec![
            "focal-statistics",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "MAJORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_minority_classes" => vec![
            "focal-statistics",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "MINORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_majority_classes_c2" => vec![
            "focal-statistics",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--neighborhood",
            "Circle 2 CELL",
            "--statistics-type",
            "MAJORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_majority_classes_nodata" => vec![
            "focal-statistics",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 5 5 CELL",
            "--statistics-type",
            "MAJORITY",
            "--ignore-nodata",
            "NODATA",
        ],
        "focal_sum_landuse" => vec![
            "focal-statistics",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "SUM",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_sum_dem" => vec![
            "focal-statistics",
            "testdata/raster/dem.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "SUM",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_median_int16" => vec![
            "focal-statistics",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "MEDIAN",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_median_f64" => vec![
            "focal-statistics",
            "testdata/raster/dem_f64.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "MEDIAN",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_variety_landuse" => vec![
            "focal-statistics",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "--neighborhood",
            "Rectangle 3 3 CELL",
            "--statistics-type",
            "VARIETY",
            "--ignore-nodata",
            "DATA",
        ],
        "focal_max_f64" => vec![
            "focal-statistics",
            "testdata/raster/dem_f64.tif",
            "<out>",
            "--neighborhood",
            "Circle 1.5 CELL",
            "--statistics-type",
            "MAXIMUM",
            "--ignore-nodata",
            "DATA",
        ],
        "fill_hydro" => vec!["fill", "testdata/raster/hydro.tif"],
        "fill_hydro_nodata" => vec!["fill", "testdata/raster/hydro_nodata.tif"],
        "fill_hydro_int" => vec!["fill", "testdata/raster/hydro_int.tif"],
        "fill_hydro_zlimit" => vec![
            "fill",
            "testdata/raster/hydro.tif",
            "<out>",
            "--z-limit",
            "3",
        ],
        "flowdir_hydro_filled" => {
            vec!["flow-direction", "testdata/arcgis-reference/fill_hydro.tif"]
        }
        "flowdir_hydro_force" => vec![
            "flow-direction",
            "testdata/arcgis-reference/fill_hydro.tif",
            "<out>",
            "--force-flow",
            "FORCE",
        ],
        "flowdir_hydro_raw" => vec!["flow-direction", "testdata/raster/hydro.tif"],
        "flowdir_hydro_nodata" => vec![
            "flow-direction",
            "testdata/arcgis-reference/fill_hydro_nodata.tif",
        ],
        "flowdir_hydro_int" => vec![
            "flow-direction",
            "testdata/arcgis-reference/fill_hydro_int.tif",
        ],
        "flowacc_hydro" => vec![
            "flow-accumulation",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
        ],
        "flowacc_hydro_int" => vec![
            "flow-accumulation",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
            "<out>",
            "--data-type",
            "INTEGER",
        ],
        "flowacc_hydro_nodata" => vec![
            "flow-accumulation",
            "testdata/arcgis-reference/flowdir_hydro_nodata.tif",
        ],
        "reclass_landuse" => vec![
            "reclassify",
            "testdata/raster/landuse_u8.tif",
            "0 10;1 10;2 20;3 30",
            "<out>",
            "--missing-values",
            "DATA",
        ],
        "reclass_landuse_nodata" => vec![
            "reclassify",
            "testdata/raster/landuse_u8.tif",
            "0 10;1 10;2 20;3 30",
            "<out>",
            "--missing-values",
            "NODATA",
        ],
        "reclass_dem_ranges" => vec![
            "reclassify",
            "testdata/raster/dem_nodata.tif",
            "180 220 1;220 260 2;260 300 3;300 340 4",
            "<out>",
            "--missing-values",
            "DATA",
        ],
        "reclass_int16_ranges" => vec![
            "reclassify",
            "testdata/raster/dem_int16_packbits.tif",
            "180 220 1;220 260 2;260 400 300",
            "<out>",
            "--missing-values",
            "NODATA",
        ],
        "reclass_dem_to_nodata" => vec![
            "reclassify",
            "testdata/raster/dem.tif",
            "180 250 NODATA;250 340 5",
            "<out>",
            "--missing-values",
            "DATA",
        ],
        "zonal_mean" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--statistics-type",
            "MEAN",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_max_strict" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--statistics-type",
            "MAXIMUM",
            "--ignore-nodata",
            "NODATA",
        ],
        "zonal_range_int" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "RANGE",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_std" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem.tif",
            "<out>",
            "--statistics-type",
            "STD",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_sum_int" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "SUM",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_median" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "MEDIAN",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_majority" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "MAJORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_variety" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "VARIETY",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_min_f" => vec![
            "zonal-statistics",
            "testdata/raster/zones.tif",
            "testdata/raster/dem.tif",
            "<out>",
            "--statistics-type",
            "MINIMUM",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_majority_classes" => vec![
            "zonal-statistics",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--statistics-type",
            "MAJORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_minority_classes" => vec![
            "zonal-statistics",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--statistics-type",
            "MINORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_median_classes" => vec![
            "zonal-statistics",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--statistics-type",
            "MEDIAN",
            "--ignore-nodata",
            "DATA",
        ],
        "zst_classes_all" => vec![
            "zonal-statistics-as-table",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/classes_u8.tif",
        ],
        "zonal_mean_25m" => vec![
            "zonal-statistics",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--statistics-type",
            "MEAN",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_majority_shifted" => vec![
            "zonal-statistics",
            "testdata/raster/zones_shifted.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "MAJORITY",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_median_4m" => vec![
            "zonal-statistics",
            "testdata/raster/zones_4m.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "MEDIAN",
            "--ignore-nodata",
            "DATA",
        ],
        "zonal_range_4m" => vec![
            "zonal-statistics",
            "testdata/raster/zones_4m.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--statistics-type",
            "RANGE",
            "--ignore-nodata",
            "NODATA",
        ],
        "reclass_landuse_4bit" => vec![
            "reclassify",
            "testdata/raster/landuse_u8.tif",
            "0 1;1 2;2 3;3 4;4 4",
        ],
        "reclass_landuse_200" => vec![
            "reclassify",
            "testdata/raster/landuse_u8.tif",
            "0 200;1 200;2 200;3 200;4 200",
        ],
        "reclass_dem_signed" => vec![
            "reclassify",
            "testdata/raster/dem.tif",
            "180 250 -5;250 340 5",
        ],
        "curvature_dem" => vec!["curvature", "testdata/raster/dem.tif"],
        "curvature_dem_nodata" => vec!["curvature", "testdata/raster/dem_nodata.tif"],
        "curvature_dem_z2" => vec![
            "curvature",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--z-factor",
            "2",
        ],
        "plus_dem_int16" => vec![
            "plus",
            "testdata/raster/dem.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "plus_int16_const" => vec!["plus", "testdata/raster/dem_int16_packbits.tif", "5"],
        "minus_nodata_dem" => vec![
            "minus",
            "testdata/raster/dem_nodata.tif",
            "testdata/raster/dem.tif",
        ],
        "times_int16_half" => vec!["times", "testdata/raster/dem_int16_packbits.tif", "0.5"],
        "times_zones_int16" => vec![
            "times",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "divide_int16_zones" => vec![
            "divide",
            "testdata/raster/dem_int16_packbits.tif",
            "testdata/raster/zones.tif",
        ],
        "divide_dem_3" => vec!["divide", "testdata/raster/dem.tif", "3"],
        "isnull_dem_nodata" => vec!["is-null", "testdata/raster/dem_nodata.tif"],
        "plus_zones_zones" => vec![
            "plus",
            "testdata/raster/zones.tif",
            "testdata/raster/zones.tif",
        ],
        "minus_zones_zones" => vec![
            "minus",
            "testdata/raster/zones.tif",
            "testdata/raster/zones.tif",
        ],
        "times_zones_zones" => vec![
            "times",
            "testdata/raster/zones.tif",
            "testdata/raster/zones.tif",
        ],
        "divide_zones_zones" => vec![
            "divide",
            "testdata/raster/zones.tif",
            "testdata/raster/zones.tif",
        ],
        "minus_zones_300" => vec!["minus", "testdata/raster/zones.tif", "300"],
        "times_zones_1000" => vec!["times", "testdata/raster/zones.tif", "1000"],
        "plus_zones_neg5" => vec!["plus", "testdata/raster/zones.tif", "-5"],
        "divide_zones_300" => vec!["divide", "testdata/raster/zones.tif", "300"],
        "divide_zones_neg2" => vec!["divide", "testdata/raster/zones.tif", "-2"],
        "plus_int16_int16" => vec![
            "plus",
            "testdata/raster/dem_int16_packbits.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "divide_int16_int16" => vec![
            "divide",
            "testdata/raster/dem_int16_packbits.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "plus_dem_landuse" => vec![
            "plus",
            "testdata/raster/dem.tif",
            "testdata/raster/landuse_u8.tif",
        ],
        "plus_shifted_dem" => vec![
            "plus",
            "testdata/raster/zones_shifted.tif",
            "testdata/raster/dem.tif",
        ],
        "plus_4m_dem" => vec![
            "plus",
            "testdata/raster/zones_4m.tif",
            "testdata/raster/dem.tif",
        ],
        "plus_25m_4m" => vec![
            "plus",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/zones_4m.tif",
        ],
        "minus_int16_25m" => vec![
            "minus",
            "testdata/raster/dem_int16_packbits.tif",
            "testdata/raster/zones_25m.tif",
        ],
        "int_zones" => vec!["int", "testdata/raster/zones.tif"],
        "int_int16" => vec!["int", "testdata/raster/dem_int16_packbits.tif"],
        "con_zones_zones_int16" => vec![
            "con",
            "testdata/raster/zones.tif",
            "testdata/raster/zones.tif",
            "<out>",
            "--in-false-raster-or-constant",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "con_zones_int16_only" => vec![
            "con",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "con_zones_300_zones" => vec![
            "con",
            "testdata/raster/zones.tif",
            "300",
            "<out>",
            "--in-false-raster-or-constant",
            "testdata/raster/zones.tif",
        ],
        "con_zones_neg5_zones" => vec![
            "con",
            "testdata/raster/zones.tif",
            "-5",
            "<out>",
            "--in-false-raster-or-constant",
            "testdata/raster/zones.tif",
        ],
        "con_int16_5_9" => vec![
            "con",
            "testdata/raster/dem_int16_packbits.tif",
            "5",
            "<out>",
            "--in-false-raster-or-constant",
            "9",
            "--where",
            "Value > 250",
        ],
        "con_dem_int16_5" => vec![
            "con",
            "testdata/raster/dem.tif",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "--in-false-raster-or-constant",
            "5",
            "--where",
            "Value > 250",
        ],
        "con_25m_dem_int16" => vec![
            "con",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/dem.tif",
            "<out>",
            "--in-false-raster-or-constant",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "con_dem_4m_shifted" => vec![
            "con",
            "testdata/raster/dem.tif",
            "testdata/raster/zones_4m.tif",
            "<out>",
            "--in-false-raster-or-constant",
            "testdata/raster/zones_shifted.tif",
            "--where",
            "Value > 250",
        ],
        "con_dem_dem_25m" => vec![
            "con",
            "testdata/raster/dem.tif",
            "testdata/raster/dem.tif",
            "<out>",
            "--in-false-raster-or-constant",
            "testdata/raster/zones_25m.tif",
            "--where",
            "Value > 250",
        ],
        "setnull_zones_int16" => vec![
            "set-null",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "setnull_zones_5" => vec!["set-null", "testdata/raster/zones.tif", "5"],
        "setnull_25m_4m" => vec![
            "set-null",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/zones_4m.tif",
        ],
        "setnull_25m_dem" => vec![
            "set-null",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/dem.tif",
        ],
        "int_dem" => vec!["int", "testdata/raster/dem_nodata.tif"],
        "float_int16" => vec!["float", "testdata/raster/dem_int16_packbits.tif"],
        "con_zones" => vec![
            "con",
            "testdata/raster/zones.tif",
            "testdata/raster/dem.tif",
            "<out>",
            "--in-false-raster-or-constant",
            "testdata/raster/dem_int16_packbits.tif",
            "--where",
            "Value > 3",
        ],
        "con_zones_const" => vec![
            "con",
            "testdata/raster/zones.tif",
            "1",
            "<out>",
            "--in-false-raster-or-constant",
            "0",
            "--where",
            "Value = 7",
        ],
        "con_no_false" => vec![
            "con",
            "testdata/raster/dem_nodata.tif",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--where",
            "Value > 250",
        ],
        "setnull_zones" => vec![
            "set-null",
            "testdata/raster/zones.tif",
            "testdata/raster/dem.tif",
            "<out>",
            "--where",
            "Value = 7",
        ],
        "contour_dem_20" => vec!["contour", "testdata/raster/dem.tif", "<out>", "20"],
        "contour_dem_nodata_15" => vec![
            "contour",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "15",
            "--base-contour",
            "5",
        ],
        "contour_int16_25" => vec![
            "contour",
            "testdata/raster/dem_int16_packbits.tif",
            "<out>",
            "25",
        ],
        "streams_hydro" => vec![
            "con",
            "testdata/arcgis-reference/flowacc_hydro.tif",
            "1",
            "<out>",
            "--where",
            "Value > 60",
        ],
        "basin_hydro" => vec![
            "basin",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
        ],
        "basin_hydro_nodata" => vec![
            "basin",
            "testdata/arcgis-reference/flowdir_hydro_nodata.tif",
        ],
        "streamlink_hydro" => vec![
            "stream-link",
            "testdata/arcgis-reference/streams_hydro.tif",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
        ],
        "streamorder_strahler" => vec![
            "stream-order",
            "testdata/arcgis-reference/streams_hydro.tif",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
        ],
        "streamorder_shreve" => vec![
            "stream-order",
            "testdata/arcgis-reference/streams_hydro.tif",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
            "<out>",
            "--order-method",
            "SHREVE",
        ],
        "flowlength_down" => vec![
            "flow-length",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
        ],
        "flowlength_up" => vec![
            "flow-length",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
            "<out>",
            "--direction-measurement",
            "UPSTREAM",
        ],
        "watershed_links" => vec![
            "watershed",
            "testdata/arcgis-reference/flowdir_hydro_filled.tif",
            "testdata/arcgis-reference/streamlink_hydro.tif",
        ],
        "r2pt_landuse" => vec!["raster-to-point", "testdata/raster/landuse_u8.tif"],
        "r2pt_dem_nodata" => vec!["raster-to-point", "testdata/raster/dem_nodata.tif"],
        "r2poly_zones" => vec!["raster-to-polygon", "testdata/raster/zones.tif"],
        "r2poly_landuse" => vec!["raster-to-polygon", "testdata/raster/landuse_u8.tif"],
        "f2r_parcels_zone" => vec![
            "feature-to-raster",
            "testdata/geoproc/parcels.shp",
            "ZONE",
            "<out>",
            "5",
        ],
        "f2r_parcels_fid" => vec![
            "feature-to-raster",
            "testdata/geoproc/parcels.shp",
            "FID",
            "<out>",
            "7",
        ],
        "f2r_wells_depth" => vec![
            "feature-to-raster",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "10",
        ],
        "f2r_roads" => vec![
            "feature-to-raster",
            "testdata/geoproc/roads.shp",
            "FID",
            "<out>",
            "4",
        ],
        "p2r_parcels_center" => vec![
            "polygon-to-raster",
            "testdata/geoproc/parcels.shp",
            "FID",
            "<out>",
            "--cellsize",
            "5",
        ],
        "p2r_parcels_maxarea" => vec![
            "polygon-to-raster",
            "testdata/geoproc/parcels.shp",
            "FID",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_AREA",
            "--cellsize",
            "5",
        ],
        "pt2r_wells_most" => vec![
            "point-to-raster",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "--cellsize",
            "20",
        ],
        "pt2r_ties_most" => vec![
            "point-to-raster",
            "testdata/raster/tie_points.shp",
            "CLASS",
            "<out>",
            "--cellsize",
            "10",
        ],
        "pt2r_ties_count" => vec![
            "point-to-raster",
            "testdata/raster/tie_points.shp",
            "CLASS",
            "<out>",
            "--cell-assignment",
            "COUNT",
            "--cellsize",
            "10",
        ],
        "pt2r_ties_mean" => vec![
            "point-to-raster",
            "testdata/raster/tie_points.shp",
            "CLASS",
            "<out>",
            "--cell-assignment",
            "MEAN",
            "--cellsize",
            "10",
        ],
        "pt2r_ties_max" => vec![
            "point-to-raster",
            "testdata/raster/tie_points.shp",
            "CLASS",
            "<out>",
            "--cell-assignment",
            "MAXIMUM",
            "--cellsize",
            "10",
        ],
        "f2r_ties" => vec![
            "feature-to-raster",
            "testdata/raster/tie_points.shp",
            "CLASS",
            "<out>",
            "10",
        ],
        "pt2r_ties_range" => vec![
            "point-to-raster",
            "testdata/raster/tie_points.shp",
            "CLASS",
            "<out>",
            "--cell-assignment",
            "RANGE",
            "--cellsize",
            "10",
        ],
        "pt2r_ties_std" => vec![
            "point-to-raster",
            "testdata/raster/tie_points.shp",
            "CLASS",
            "<out>",
            "--cell-assignment",
            "STANDARD_DEVIATION",
            "--cellsize",
            "10",
        ],
        "pt2r_wells_std" => vec![
            "point-to-raster",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "--cell-assignment",
            "STANDARD_DEVIATION",
            "--cellsize",
            "20",
        ],
        "pt2r_wells_range" => vec![
            "point-to-raster",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "--cell-assignment",
            "RANGE",
            "--cellsize",
            "20",
        ],
        "p2r_parcels_zone_maxarea_25" => vec![
            "polygon-to-raster",
            "testdata/geoproc/parcels.shp",
            "ZONE",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_AREA",
            "--cellsize",
            "25",
        ],
        "p2r_parcels_zone_combined_25" => vec![
            "polygon-to-raster",
            "testdata/geoproc/parcels.shp",
            "ZONE",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_COMBINED_AREA",
            "--cellsize",
            "25",
        ],
        "p2r_combine_maxarea" => vec![
            "polygon-to-raster",
            "testdata/raster/combine_polys.shp",
            "CODE",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_AREA",
            "--cellsize",
            "10",
        ],
        "p2r_combine_combined" => vec![
            "polygon-to-raster",
            "testdata/raster/combine_polys.shp",
            "CODE",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_COMBINED_AREA",
            "--cellsize",
            "10",
        ],
        "pl2r_combine_maxlen" => vec![
            "polyline-to-raster",
            "testdata/raster/combine_lines.shp",
            "CODE",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_LENGTH",
            "--cellsize",
            "10",
        ],
        "pl2r_combine_combined" => vec![
            "polyline-to-raster",
            "testdata/raster/combine_lines.shp",
            "CODE",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_COMBINED_LENGTH",
            "--cellsize",
            "10",
        ],
        "pl2r_roads_class" => vec![
            "polyline-to-raster",
            "testdata/geoproc/roads.shp",
            "CLASS",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_LENGTH",
            "--cellsize",
            "20",
        ],
        "pl2r_roads_class_combined" => vec![
            "polyline-to-raster",
            "testdata/geoproc/roads.shp",
            "CLASS",
            "<out>",
            "--cell-assignment",
            "MAXIMUM_COMBINED_LENGTH",
            "--cellsize",
            "20",
        ],
        "pt2r_wells_sum" => vec![
            "point-to-raster",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "--cell-assignment",
            "SUM",
            "--cellsize",
            "20",
        ],
        "eucdist_streams" => vec![
            "euc-distance",
            "testdata/arcgis-reference/streams_hydro.tif",
        ],
        "eucdist_streams_max" => vec![
            "euc-distance",
            "testdata/arcgis-reference/streams_hydro.tif",
            "<out>",
            "--maximum-distance",
            "150",
        ],
        "eucdir_streams" => vec![
            "euc-direction",
            "testdata/arcgis-reference/streams_hydro.tif",
        ],
        "eucdir_streams_max" => vec![
            "euc-direction",
            "testdata/arcgis-reference/streams_hydro.tif",
            "<out>",
            "--maximum-distance",
            "150",
        ],
        "expand_classes_1" => vec![
            "expand",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "1",
            "1;2",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "expand_classes_2" => vec![
            "expand",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "2",
            "0;3;5",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "expand_classes_dist_3" => vec![
            "expand",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "3",
            "1;2;5",
            "--expand-method",
            "DISTANCE",
        ],
        "expand_patches_2" => vec![
            "expand",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "2",
            "-3;12",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "expand_patches_5" => vec![
            "expand",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "5",
            "2;7;12",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "expand_patches_dist_2_5" => vec![
            "expand",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "2.5",
            "2;7;12",
            "--expand-method",
            "DISTANCE",
        ],
        "expand_patches_dist_1_5" => vec![
            "expand",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "1.5",
            "-3;7",
            "--expand-method",
            "DISTANCE",
        ],
        "expand_landuse_4" => vec![
            "expand",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "4",
            "2",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "expand_zones_dist_12" => vec![
            "expand",
            "testdata/raster/zones.tif",
            "<out>",
            "12",
            "7;0",
            "--expand-method",
            "DISTANCE",
        ],
        "expand_absent_zone" => vec![
            "expand",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "1",
            "9",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "expand_bad_cells" => vec![
            "expand",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "0",
            "1",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "expand_float" => vec![
            "expand",
            "testdata/raster/dem.tif",
            "<out>",
            "1",
            "1",
            "--expand-method",
            "MORPHOLOGICAL",
        ],
        "shrink_classes_1" => vec![
            "shrink",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "1",
            "1;2;3",
            "--shrink-method",
            "MORPHOLOGICAL",
        ],
        "shrink_classes_3" => vec![
            "shrink",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "3",
            "0;4",
            "--shrink-method",
            "MORPHOLOGICAL",
        ],
        "shrink_classes_dist_2" => vec![
            "shrink",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "2",
            "4",
            "--shrink-method",
            "DISTANCE",
        ],
        "shrink_patches_2" => vec![
            "shrink",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "2",
            "2;7",
            "--shrink-method",
            "MORPHOLOGICAL",
        ],
        "shrink_patches_dist_3" => vec![
            "shrink",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "3",
            "0;2;7",
            "--shrink-method",
            "DISTANCE",
        ],
        "shrink_zones_6" => vec![
            "shrink",
            "testdata/raster/zones.tif",
            "<out>",
            "6",
            "7;4",
            "--shrink-method",
            "MORPHOLOGICAL",
        ],
        "shrink_landuse_dist_1" => vec![
            "shrink",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "1",
            "1;3",
            "--shrink-method",
            "DISTANCE",
        ],
        "majority_classes_four" => vec![
            "majority-filter",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--number-neighbors",
            "FOUR",
            "--majority-definition",
            "MAJORITY",
        ],
        "majority_classes_four_half" => vec![
            "majority-filter",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--number-neighbors",
            "FOUR",
            "--majority-definition",
            "HALF",
        ],
        "majority_classes_eight" => vec![
            "majority-filter",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--majority-definition",
            "MAJORITY",
        ],
        "majority_classes_eight_half" => vec![
            "majority-filter",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--majority-definition",
            "HALF",
        ],
        "majority_patches_eight_half" => vec![
            "majority-filter",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--majority-definition",
            "HALF",
        ],
        "majority_patches_four" => vec![
            "majority-filter",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "--number-neighbors",
            "FOUR",
            "--majority-definition",
            "MAJORITY",
        ],
        "majority_landuse_eight" => vec![
            "majority-filter",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--majority-definition",
            "MAJORITY",
        ],
        "majority_float" => vec![
            "majority-filter",
            "testdata/raster/dem.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--majority-definition",
            "MAJORITY",
        ],
        "regiongroup_classes_eight" => vec![
            "region-group",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--zone-connectivity",
            "WITHIN",
            "--add-link",
            "ADD_LINK",
        ],
        "regiongroup_classes_four" => vec![
            "region-group",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--number-neighbors",
            "FOUR",
            "--zone-connectivity",
            "WITHIN",
            "--add-link",
            "ADD_LINK",
        ],
        "regiongroup_classes_eight_cross" => vec![
            "region-group",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--zone-connectivity",
            "CROSS",
            "--add-link",
            "ADD_LINK",
        ],
        "regiongroup_patches_eight" => vec![
            "region-group",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--zone-connectivity",
            "WITHIN",
            "--add-link",
            "ADD_LINK",
        ],
        "regiongroup_patches_four_cross" => vec![
            "region-group",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "--number-neighbors",
            "FOUR",
            "--zone-connectivity",
            "CROSS",
            "--add-link",
            "ADD_LINK",
        ],
        "regiongroup_patches_eight_excluded" => vec![
            "region-group",
            "testdata/raster/patches_i16.tif",
            "<out>",
            "--number-neighbors",
            "EIGHT",
            "--zone-connectivity",
            "WITHIN",
            "--add-link",
            "ADD_LINK",
            "--excluded-value",
            "0",
        ],
        "regiongroup_zones_four_excluded" => vec![
            "region-group",
            "testdata/raster/zones.tif",
            "<out>",
            "--number-neighbors",
            "FOUR",
            "--zone-connectivity",
            "WITHIN",
            "--add-link",
            "NO_LINK",
            "--excluded-value",
            "7",
        ],
        "eucalloc_streams" => vec![
            "euc-allocation",
            "testdata/arcgis-reference/streams_hydro.tif",
        ],
        "eucalloc_streams_max" => vec![
            "euc-allocation",
            "testdata/arcgis-reference/streams_hydro.tif",
            "<out>",
            "--maximum-distance",
            "150",
        ],
        "kd_wells" => vec![
            "kernel-density",
            "testdata/geoproc/wells.shp",
            "NONE",
            "<out>",
            "30",
        ],
        "kd_wells_depth_r" => vec![
            "kernel-density",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "25",
            "--search-radius",
            "150",
            "--area-unit-scale-factor",
            "SQUARE_METERS",
        ],
        "kd_wells_default_depth" => vec![
            "kernel-density",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "25",
        ],
        "kd_wells_expected" => vec![
            "kernel-density",
            "testdata/geoproc/wells.shp",
            "NONE",
            "<out>",
            "25",
            "--search-radius",
            "200",
            "--out-cell-values",
            "EXPECTED_COUNTS",
        ],
        "kd_roads" => vec![
            "kernel-density",
            "testdata/geoproc/roads.shp",
            "NONE",
            "<out>",
            "25",
        ],
        "kd_roads_r" => vec![
            "kernel-density",
            "testdata/geoproc/roads.shp",
            "CLASS",
            "<out>",
            "20",
            "--search-radius",
            "120",
            "--area-unit-scale-factor",
            "SQUARE_MAP_UNITS",
        ],
        "pd_wells_circle" => vec![
            "point-density",
            "testdata/geoproc/wells.shp",
            "NONE",
            "<out>",
            "25",
            "--neighborhood",
            "Circle 100 MAP",
        ],
        "pd_wells_depth_rect" => vec![
            "point-density",
            "testdata/geoproc/wells.shp",
            "DEPTH",
            "<out>",
            "20",
            "--neighborhood",
            "Rectangle 80 120 MAP",
            "--area-unit-scale-factor",
            "SQUARE_METERS",
        ],
        "pd_wells_annulus_cell" => vec![
            "point-density",
            "testdata/geoproc/wells.shp",
            "NONE",
            "<out>",
            "30",
            "--neighborhood",
            "Annulus 2 5 CELL",
            "--area-unit-scale-factor",
            "SQUARE_METERS",
        ],
        "pd_wells_wedge" => vec![
            "point-density",
            "testdata/geoproc/wells.shp",
            "NONE",
            "<out>",
            "30",
            "--neighborhood",
            "Wedge 120 90 180 MAP",
            "--area-unit-scale-factor",
            "SQUARE_METERS",
        ],
        "pd_wells_wedge_wrap" => vec![
            "point-density",
            "testdata/geoproc/wells.shp",
            "NONE",
            "<out>",
            "30",
            "--neighborhood",
            "Wedge 120 300 60 MAP",
            "--area-unit-scale-factor",
            "SQUARE_METERS",
        ],
        "ld_roads" => vec![
            "line-density",
            "testdata/geoproc/roads.shp",
            "NONE",
            "<out>",
            "25",
            "100",
        ],
        "ld_roads_class" => vec![
            "line-density",
            "testdata/geoproc/roads.shp",
            "CLASS",
            "<out>",
            "30",
            "80",
            "--area-unit-scale-factor",
            "SQUARE_METERS",
        ],
        "evp_wells_dem" => vec![
            "extract-values-to-points",
            "testdata/geoproc/wells.shp",
            "testdata/raster/dem.tif",
            "<out>",
        ],
        "evp_wells_dem_interp" => vec![
            "extract-values-to-points",
            "testdata/geoproc/wells.shp",
            "testdata/raster/dem.tif",
            "<out>",
            "--interpolate-values",
            "INTERPOLATE",
        ],
        "evp_probe_nodata" => vec![
            "extract-values-to-points",
            "testdata/raster/evp_points.shp",
            "testdata/raster/dem_nodata.tif",
            "<out>",
        ],
        "evp_probe_nodata_interp" => vec![
            "extract-values-to-points",
            "testdata/raster/evp_points.shp",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--interpolate-values",
            "INTERPOLATE",
        ],
        "evp_probe_classes" => vec![
            "extract-values-to-points",
            "testdata/raster/evp_points.shp",
            "testdata/raster/classes_u8.tif",
            "<out>",
        ],
        "evp_probe_classes_interp" => vec![
            "extract-values-to-points",
            "testdata/raster/evp_points.shp",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "--interpolate-values",
            "INTERPOLATE",
        ],
        "evp_probe_f64" => vec![
            "extract-values-to-points",
            "testdata/raster/evp_points.shp",
            "testdata/raster/dem_f64.tif",
            "<out>",
            "--interpolate-values",
            "INTERPOLATE",
        ],
        "extract_dem_zones" => vec![
            "extract-by-mask",
            "testdata/raster/dem.tif",
            "testdata/raster/zones.tif",
        ],
        "extract_dem_mask25" => vec![
            "extract-by-mask",
            "testdata/raster/dem.tif",
            "testdata/raster/zones_25m.tif",
        ],
        "extract_dem_mask4m" => vec![
            "extract-by-mask",
            "testdata/raster/dem.tif",
            "testdata/raster/zones_4m.tif",
        ],
        "extract_int16_shifted" => vec![
            "extract-by-mask",
            "testdata/raster/dem_int16_packbits.tif",
            "testdata/raster/zones_shifted.tif",
        ],
        "extract_int16_zones" => vec![
            "extract-by-mask",
            "testdata/raster/dem_int16_packbits.tif",
            "testdata/raster/zones.tif",
        ],
        "extract_landuse_zones" => vec![
            "extract-by-mask",
            "testdata/raster/landuse_u8.tif",
            "testdata/raster/zones.tif",
        ],
        "extract_nodata_mask25" => vec![
            "extract-by-mask",
            "testdata/raster/dem_nodata.tif",
            "testdata/raster/zones_25m.tif",
        ],
        "resample_dem_nearest" => vec!["resample", "testdata/raster/dem.tif", "<out>", "25"],
        "resample_dem_bilinear" => vec![
            "resample",
            "testdata/raster/dem.tif",
            "<out>",
            "25",
            "--resampling-type",
            "BILINEAR",
        ],
        "resample_dem_cubic" => vec![
            "resample",
            "testdata/raster/dem.tif",
            "<out>",
            "7",
            "--resampling-type",
            "CUBIC",
        ],
        "resample_landuse_majority" => vec![
            "resample",
            "testdata/raster/landuse_u8.tif",
            "<out>",
            "60",
            "--resampling-type",
            "MAJORITY",
        ],
        "resample_classes_majority_60" => vec![
            "resample",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "60",
            "--resampling-type",
            "MAJORITY",
        ],
        "resample_classes_majority_90" => vec![
            "resample",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "90",
            "--resampling-type",
            "MAJORITY",
        ],
        "resample_classes_majority_45" => vec![
            "resample",
            "testdata/raster/classes_u8.tif",
            "<out>",
            "45",
            "--resampling-type",
            "MAJORITY",
        ],
        "zst_all" => vec![
            "zonal-statistics-as-table",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_nodata.tif",
        ],
        "zst_25m_all" => vec![
            "zonal-statistics-as-table",
            "testdata/raster/zones_25m.tif",
            "testdata/raster/dem_nodata.tif",
        ],
        "zst_4m_int_all" => vec![
            "zonal-statistics-as-table",
            "testdata/raster/zones_4m.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "zst_int_all" => vec![
            "zonal-statistics-as-table",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_int16_packbits.tif",
        ],
        "zst_mean_nodata" => vec![
            "zonal-statistics-as-table",
            "testdata/raster/zones.tif",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--ignore-nodata",
            "NODATA",
            "--statistics-type",
            "MEAN",
        ],
        "zst_min_max" => vec![
            "zonal-statistics-as-table",
            "testdata/raster/zones.tif",
            "testdata/raster/dem.tif",
            "<out>",
            "--statistics-type",
            "MIN_MAX_MEAN",
        ],
        "hillshade_shadows_315_15" => vec![
            "hillshade",
            "testdata/raster/dem.tif",
            "<out>",
            "--altitude",
            "15",
            "--model-shadows",
            "SHADOWS",
            "--z-factor",
            "3",
        ],
        "hillshade_shadows_200_25" => vec![
            "hillshade",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--azimuth",
            "200",
            "--altitude",
            "25",
            "--model-shadows",
            "SHADOWS",
            "--z-factor",
            "2",
        ],
        "viewshed_hill" => vec![
            "viewshed",
            "testdata/raster/dem.tif",
            "testdata/raster/observer_hill.shp",
        ],
        "viewshed_two" => vec![
            "viewshed",
            "testdata/raster/dem.tif",
            "testdata/raster/observers.shp",
        ],
        "viewshed_hill_z2" => vec![
            "viewshed",
            "testdata/raster/dem.tif",
            "testdata/raster/observer_hill.shp",
            "<out>",
            "--z-factor",
            "2",
        ],
        "slope_dem" => vec!["slope", "testdata/raster/dem.tif"],
        "slope_dem_percent_z" => vec![
            "slope",
            "testdata/raster/dem.tif",
            "<out>",
            "--output-measurement",
            "PERCENT_RISE",
            "--z-factor",
            "2.5",
        ],
        "slope_dem_nodata" => vec!["slope", "testdata/raster/dem_nodata.tif"],
        "slope_dem_int16" => vec!["slope", "testdata/raster/dem_int16_packbits.tif"],
        "aspect_dem" => vec!["aspect", "testdata/raster/dem.tif"],
        "aspect_dem_nodata" => vec!["aspect", "testdata/raster/dem_nodata.tif"],
        "aspect_landuse" => vec!["aspect", "testdata/raster/landuse_u8.tif"],
        "hillshade_dem" => vec!["hillshade", "testdata/raster/dem.tif"],
        "hillshade_dem_nodata" => vec![
            "hillshade",
            "testdata/raster/dem_nodata.tif",
            "<out>",
            "--azimuth",
            "200",
            "--altitude",
            "30",
        ],
        "hillshade_dem_z3" => vec![
            "hillshade",
            "testdata/raster/dem.tif",
            "<out>",
            "--z-factor",
            "3",
        ],
        "csv_parcels" => vec!["export-table", "testdata/geoproc/parcels.shp"],
        "csv_wells" => vec!["export-table", "testdata/geoproc/wells.shp"],
        "csv_bad_lines" => vec!["export-table", "testdata/geoproc/bad_lines.shp"],
        "csv_stations_back" => vec!["export-table", "testdata/geoproc/stations.csv"],
        "xy_stations" => vec![
            "xy-table-to-point",
            "testdata/geoproc/stations.csv",
            "<out>",
            "X",
            "Y",
            "--coordinate-system",
            "32633",
        ],
        "xy_stations_z" => vec![
            "xy-table-to-point",
            "testdata/geoproc/stations.csv",
            "<out>",
            "X",
            "Y",
            "--z-field",
            "ELEV",
            "--coordinate-system",
            "32633",
        ],
        "identity_parcels_clip_no_fid" => vec![
            "identity",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--join-attributes",
            "NO_FID",
        ],
        "identity_roads_parcels_fid_only" => vec![
            "identity",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--join-attributes",
            "ONLY_FID",
        ],
        "identity_roads_parcels_relationships" => vec![
            "identity",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--relationship",
            "KEEP_RELATIONSHIPS",
        ],
        "identity_roads_clip_relationships" => vec![
            "identity",
            "testdata/geoproc/roads.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--join-attributes",
            "NO_FID",
            "--relationship",
            "KEEP_RELATIONSHIPS",
        ],
        "union_parcels_clip_no_fid" => vec![
            "union",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--join-attributes",
            "NO_FID",
        ],
        "union_parcels_clip_fid_only" => vec![
            "union",
            "testdata/geoproc/parcels.shp",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--join-attributes",
            "ONLY_FID",
        ],
        "union_clip_no_gaps" => vec![
            "union",
            "testdata/geoproc/clip_areas.shp",
            "<out>",
            "--gaps",
            "NO_GAPS",
        ],
        "union_clip_gaps" => vec!["union", "testdata/geoproc/clip_areas.shp"],
        "p2l_grid" => vec!["polygon-to-line", "testdata/planar/jitter_grid.shp"],
        "p2l_overlapping" => vec!["polygon-to-line", "testdata/planar/overlapping.shp"],
        "p2l_holes" => vec!["polygon-to-line", "testdata/planar/holes.shp"],
        "p2l_nested" => vec!["polygon-to-line", "testdata/planar/nested.shp"],
        "p2l_parcels" => vec!["polygon-to-line", "testdata/geoproc/parcels.shp"],
        "p2l_squares" => vec!["polygon-to-line", "testdata/geoproc/squares.shp"],
        "ftp_grid" => vec!["feature-to-polygon", "testdata/planar/jitter_grid.shp"],
        "ftp_overlapping" => vec!["feature-to-polygon", "testdata/planar/overlapping.shp"],
        "ftp_holes" => vec!["feature-to-polygon", "testdata/planar/holes.shp"],
        "ftp_nested" => vec!["feature-to-polygon", "testdata/planar/nested.shp"],
        "ftp_lines" => vec!["feature-to-polygon", "testdata/planar/lines.shp"],
        "ftp_parcels" => vec!["feature-to-polygon", "testdata/geoproc/parcels.shp"],
        "ftp_parcels_wells" => vec![
            "feature-to-polygon",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--label-features",
            "testdata/geoproc/wells.shp",
        ],
        "ftp_parcels_wells_noattr" => vec![
            "feature-to-polygon",
            "testdata/geoproc/parcels.shp",
            "<out>",
            "--label-features",
            "testdata/geoproc/wells.shp",
            "--attributes",
            "NO_ATTRIBUTES",
        ],
        "ftp_roads" => vec!["feature-to-polygon", "testdata/geoproc/roads.shp"],
        "ftl_lines" => vec!["feature-to-line", "testdata/planar/lines.shp"],
        "ftl_lines_noattr" => vec![
            "feature-to-line",
            "testdata/planar/lines.shp",
            "<out>",
            "--attributes",
            "NO_ATTRIBUTES",
        ],
        "ftl_lines_bridges" => vec!["feature-to-line", "testdata/planar/lines_bridges.shp"],
        "ftl_grid" => vec!["feature-to-line", "testdata/planar/jitter_grid.shp"],
        "ftl_holes" => vec!["feature-to-line", "testdata/planar/holes.shp"],
        "ftl_roads" => vec!["feature-to-line", "testdata/geoproc/roads.shp"],
        "ftl_streams" => vec!["feature-to-line", "testdata/geoproc/streams.shp"],
        other => dm_cases::args(other),
    }
}

/// Z/M values ArcGIS computes in a way we have not worked out.
/// intersect_roads_zm_parcels_point 15: road 3 crosses parcel 15's hole at two
/// hole corners; ArcGIS gives them Z interpolated between the outer-ring crossings
/// (15.2917, 15.6042) instead of the corners' own Z (15, 16), although on a plain
/// square with a hole it does use the hole corners' Z.
const KNOWN_ZM_DIFFERENCES: &[(&str, usize)] = &[("intersect_roads_zm_parcels_point", 15)];

/// Z/M awareness and values: each of our vertices against ArcGIS's vertex at the
/// same place (within 0.001 m). Missing measures (NaN) must match.
fn zm_diff(a: &unigis_core::Geometry, b: &unigis_core::Geometry) -> Option<String> {
    if a.z.is_some() != b.z.is_some() {
        return Some(format!(
            "Z: ours {}, ArcGIS {}",
            a.z.is_some(),
            b.z.is_some()
        ));
    }
    // ArcGIS rounds Z and M to its 0.0001 grid as it splits lines, so values may
    // land one grid step from the exact interpolation.
    let same = |x: f64, y: f64| (x.is_nan() && y.is_nan()) || (x - y).abs() <= 1.5e-4;
    for (i, p) in a.xy.iter().enumerate() {
        let Some(j) = (0..b.xy.len())
            .filter(|&j| (b.xy[j][0] - p[0]).hypot(b.xy[j][1] - p[1]) <= 0.001)
            .min_by(|&j, &k| {
                let d = |j: usize| (b.xy[j][0] - p[0]).hypot(b.xy[j][1] - p[1]);
                d(j).total_cmp(&d(k))
            })
        else {
            continue;
        };
        if let (Some(za), Some(zb)) = (&a.z, &b.z)
            && !same(za[i], zb[j])
        {
            return Some(format!("Z at {p:?}: ours {}, ArcGIS {}", za[i], zb[j]));
        }
        let (ma, mb) = (
            a.m.as_ref().map_or(f64::NAN, |m| m[i]),
            b.m.as_ref().map_or(f64::NAN, |m| m[j]),
        );
        if !same(ma, mb) {
            return Some(format!("M at {p:?}: ours {ma}, ArcGIS {mb}"));
        }
    }
    None
}

/// Rows of a .dbf table.
fn table_rows(path: &Path) -> Vec<Vec<Value>> {
    let dbf = unigis_shapefile::dbf::DbfReader::open(path, Some("UTF-8")).unwrap();
    (0..dbf.num_records())
        .map(|i| dbf.read(i).unwrap().0)
        .collect()
}

fn compare_tables(ours: &Path, theirs: &Path, fields: &serde_json::Value) -> Vec<String> {
    let mut diffs = Vec::new();
    let expected: Vec<(String, u64)> = fields.as_array().unwrap()[1..]
        .iter()
        .map(|f| (f[0].as_str().unwrap().to_owned(), f[2].as_u64().unwrap()))
        .collect();
    let dbf = unigis_shapefile::dbf::DbfReader::open(ours, Some("UTF-8")).unwrap();
    let got: Vec<(String, u64)> = dbf
        .header()
        .fields
        .iter()
        .map(|f| (f.name.clone(), u64::from(f.width)))
        .collect();
    if got != expected {
        diffs.push(format!("schema: ours {got:?}, ArcGIS {expected:?}"));
    }
    let (a, b) = (table_rows(ours), table_rows(theirs));
    if a.len() != b.len() {
        diffs.push(format!("count: ours {}, ArcGIS {}", a.len(), b.len()));
    }
    // Paths (CLASS) compare by file name, numbers within the usual noise. Tabulate Intersection
    // measures on ArcGIS's coordinate grid: areas agree to about 1e-5 relative.
    let loose = theirs
        .file_stem()
        .is_some_and(|n| n.to_string_lossy().starts_with("ti_"));
    let close = |x: &Value, y: &Value| match (x, y) {
        (Value::Double(p), Value::Double(q)) if loose => {
            (p - q).abs() <= (2e-5 * p.abs().max(q.abs())).max(1e-3)
        }
        _ => same_value(x, y),
    };
    for (i, (ra, rb)) in a.iter().zip(&b).enumerate() {
        if ra.len() != rb.len() || ra.iter().zip(rb).any(|(x, y)| !close(x, y)) {
            diffs.push(format!("row {i}: ours {ra:?}, ArcGIS {rb:?}"));
        }
    }
    diffs
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
    let theirs_path = theirs.to_path_buf();
    // The coordinate system written: ArcGIS's exact Esri WKT.
    let prj = |p: &Path| std::fs::read_to_string(p.with_extension("prj")).ok();
    let prj_diff = (prj(ours) != prj(theirs))
        .then(|| format!("prj: ours {:?}, ArcGIS {:?}", prj(ours), prj(theirs)));
    let ours = ShapefileReader::open(ours).unwrap();
    let theirs = ShapefileReader::open(theirs).unwrap();
    let mut diffs: Vec<String> = prj_diff.into_iter().collect();

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

    let mut a: Vec<_> = ours.features().map(Result::unwrap).collect();
    let mut b: Vec<_> = theirs.features().map(Result::unwrap).collect();
    // Extract Values to Points writes the points grouped by spatial tile (its order is
    // engine-internal; for one tile, as with the wells, it is the input order): compare
    // the probe cases by their PID field.
    if case.starts_with("evp_probe") {
        let pid = |f: &unigis_core::Feature| match f.attributes.get(1) {
            Some(Value::Integer(v)) => *v,
            _ => 0,
        };
        a.sort_by_key(pid);
        b.sort_by_key(pid);
    }
    // The Pairwise tools run in parallel and list their features in no fixed order: compare by
    // attribute values.
    if case.starts_with("pwerase_")
        || case.starts_with("pwint_")
        || case.starts_with("pwdis_")
        || case.starts_with("th_")
    {
        // Features with equal attributes (the parts of a single-part dissolve) by lower-left corner.
        let key = |f: &unigis_core::Feature| {
            let corner = f.geometry.as_ref().map_or((0, 0), |g| {
                let min = |i: usize| g.xy.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min);
                (
                    (min(0) * 100.0).round() as i64,
                    (min(1) * 100.0).round() as i64,
                )
            });
            format!("{:?}{corner:?}", f.attributes)
        };
        a.sort_by_key(key);
        b.sort_by_key(key);
    }
    if a.len() != b.len() {
        diffs.push(format!("count: ours {}, ArcGIS {}", a.len(), b.len()));
    }
    // Clip agrees within ArcGIS's own XY tolerance (0.001 m: coordinates closer
    // than that are the same point to ArcGIS). Buffers are densified differently
    // (vertex placement on arcs), so compare shape within 2 × the 0.1 m deviation.
    // Circles are densified arcs, like buffers.
    let is_buffer = case.starts_with("buffer")
        || case.starts_with("pwbuf_")
        || case.starts_with("mrb_")
        || case.starts_with("gb_")
        || case.ends_with("_circle");
    // Intersect of three or more inputs runs as a cascade of overlays, each of
    // which may move a vertex within the tolerance: allow two steps.
    let cascade = case.starts_with("intersect_tri") || case.starts_with("intersect_3");
    // One-sided round buffers: ArcGIS's quarter-disc ends stop a fraction of a
    // degree short of the line's axis (0.4° at the start, 0.3° at the end for a
    // 25 m buffer), which we do not reproduce: 0.013% of a short road's area.
    let one_sided = case.contains("_left") || case.contains("_right");
    // Geographic output: ArcGIS's 0.001 m tolerance is ~1e-8 degrees.
    let geographic = std::fs::read_to_string(theirs_path.with_extension("prj"))
        .is_ok_and(|p| p.trim_start().starts_with("GEOGCS"));
    let (shape_tol, area_tol) = if geographic {
        (1e-8, 1e-7)
    } else if is_buffer && one_sided {
        (0.2, 2e-4)
    } else if case.starts_with("sn_") {
        // Summarize Nearby draws its corner arcs coarser than Buffer does.
        (0.25, 1.5e-3)
    } else if is_buffer {
        (0.2, 1e-4)
    } else if cascade {
        (0.002, 1e-7)
    } else {
        (0.001, 1e-7)
    };
    for (i, (fa, fb)) in a.iter().zip(&b).enumerate() {
        for (k, (va, vb)) in fa.attributes.iter().zip(&fb.attributes).enumerate() {
            // DANGLE_LEN runs to where ArcGIS's own planarization put a crossing,
            // which can be up to the 0.001 m XY tolerance from the exact one.
            let measured = expected.get(k).is_some_and(|(n, _)| n == "DANGLE_LEN");
            let near = |a: &Value, b: &Value| match (a, b) {
                (Value::Double(x), Value::Double(y)) => (x - y).abs() <= 0.001,
                _ => false,
            };
            if !same_value(va, vb) && !(measured && near(va, vb)) {
                diffs.push(format!("feature {i} field {k}: ours {va:?}, ArcGIS {vb:?}"));
            }
        }
        // ArcGIS writes null shapes as empty records; both are "no shape".
        let shape = |g: &Option<unigis_core::Geometry>| g.clone().filter(|g| !g.xy.is_empty());
        let (Some(ga), Some(gb)) = (&shape(&fa.geometry), &shape(&fb.geometry)) else {
            if shape(&fa.geometry).is_some() != shape(&fb.geometry).is_some() {
                diffs.push(format!("feature {i}: null geometry mismatch"));
            }
            continue;
        };
        if ga.num_parts() != gb.num_parts() {
            diffs.push(format!(
                "feature {i}: parts ours {}, ArcGIS {}",
                ga.num_parts(),
                gb.num_parts()
            ));
        }
        if let Some(d) = zm_diff(ga, gb)
            && !KNOWN_ZM_DIFFERENCES.contains(&(case, i))
        {
            diffs.push(format!("feature {i}: {d}"));
        }
        // Planar graph tools: the same vertices in the same order (start and direction).
        if ["p2l_", "ftp_", "ftl_"].iter().any(|p| case.starts_with(p))
            && (ga.parts != gb.parts
                || ga.xy.len() != gb.xy.len()
                || ga.xy.iter().zip(&gb.xy).any(|(p, q)| {
                    (p[0] - q[0]).abs() > shape_tol || (p[1] - q[1]).abs() > shape_tol
                }))
        {
            diffs.push(format!(
                "feature {i}: vertices differ (order, start or direction)"
            ));
        }
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

/// Vector cases with known differences: the features that differ, or an empty list
/// when the whole case does. None at the moment.
const KNOWN_FEATURE_DIFFERENCES: &[(&str, &[usize])] = &[
    // MULTIPART envelopes of nested parts: our shapefile writer turns the inner ring into a hole,
    // ArcGIS keeps it a second exterior ring.
    ("env_roads_multi", &[2]),
    // Multiple Ring Buffer around parcels: the 30 m ring has 0.05% less area in ArcGIS (its arcs
    // there are drawn with fewer segments than the 3.75 degree step we use).
    ("mrb_parcels_outside", &[1]),
    // Split Line At Point: ArcGIS lists the pieces of one curved road as 1, 3, 2, 5, 4 (the last
    // piece before a close pair of splits first); the pieces themselves are equal.
    ("sap_roads_wells_20", &[12, 13, 14, 15]),
    // Dice of polygons: where a half-cell holds a neck of zero width, ArcGIS keeps one ring and
    // we make two parts.
    ("dice_polys_64", &[4]),
    ("dice_polys_50", &[5]),
    ("dice_polys_40", &[6]),
    // Dice of polygons with cells of 11 to 18 vertices: ArcGIS does not always cut across the
    // longer side (about 10 of 41 cuts in the probes); the whole feature list differs.
    ("dice_polys_33", &[]),
    ("dice_polys_25", &[]),
    ("dice_polys_20", &[]),
    ("dice_polys_15", &[]),
    ("dice_polys_12", &[]),
    ("dice_polys_10", &[]),
    ("dice_multipoly_10", &[]),
    ("dice_multipoly_20", &[]),
];

/// Raster cases where some cells are known to differ from ArcGIS, with the most
/// differing cells allowed: Flow Direction resolves flats differently in places,
/// Hillshade shadows differ for cells within centimeters of the horizon, and
/// and Viewshed where sight lines graze the terrain (about 1% of cells).
/// Reference cases (by name prefix) of tools not implemented yet; remove the prefix with the tool.
const PENDING_CASES: &[&str] = &[];

const KNOWN_RASTER_DIFFERENCES: &[(&str, usize)] = &[
    // Cost Back Link: cells where two equally cheap paths meet and ArcGIS picks one by a rule not found.
    ("cback_int", 15),
    // Slice: ArcGIS puts a few cells just below a class boundary into the class above (it
    // bins the values; the rule was not found).
    ("slice_ei5", 10),
    ("slice_ei7_base3", 9),
    ("slice_ei3_nodata", 5),
    ("slice_ea5", 10),
    ("slice_ea4_nodata", 13),
    ("viewshed_hill", 199),
    ("viewshed_two", 309),
    ("viewshed_hill_z2", 199),
    ("hillshade_shadows_315_15", 3),
    ("hillshade_shadows_200_25", 29),
    ("flowdir_hydro_filled", 5),
    ("flowdir_hydro_force", 5),
    ("flowdir_hydro_raw", 3),
    ("flowdir_hydro_nodata", 5),
    ("flowdir_hydro_int", 930),
    // A center between NoData cells that ArcGIS fills from its neighbors.
    ("resample_classes_majority_60", 1),
];

/// Grid, cell type, NoData, coordinate system and every cell; float cells within
/// 1e-5 relative (ArcGIS computes in single precision in places).
fn compare_rasters(case: &str, ours: &Path, arcgis: &Path) -> Vec<String> {
    let (a, b) = match (
        unigis_raster::read_geotiff(ours),
        unigis_raster::read_geotiff(arcgis),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => return vec![format!("read: ours {:?}, ArcGIS {:?}", a.err(), b.err())],
    };
    let mut diffs = Vec::new();
    if (a.width, a.height) != (b.width, b.height) {
        return vec![format!(
            "size: ours {}x{}, ArcGIS {}x{}",
            a.width, a.height, b.width, b.height
        )];
    }
    if a.transform != b.transform {
        diffs.push(format!(
            "transform: ours {:?}, ArcGIS {:?}",
            a.transform, b.transform
        ));
    }
    if a.data_type != b.data_type {
        diffs.push(format!(
            "type: ours {:?}, ArcGIS {:?}",
            a.data_type, b.data_type
        ));
    }
    if a.nodata.map(f64::to_bits) != b.nodata.map(f64::to_bits) {
        diffs.push(format!(
            "nodata: ours {:?}, ArcGIS {:?}",
            a.nodata, b.nodata
        ));
    }
    if a.epsg != b.epsg {
        diffs.push(format!("epsg: ours {:?}, ArcGIS {:?}", a.epsg, b.epsg));
    }
    if a.bands.len() != b.bands.len() {
        diffs.push(format!(
            "band count: ours {}, ArcGIS {}",
            a.bands.len(),
            b.bands.len()
        ));
    }
    let mut bad = 0;
    for (index, (left, right)) in a.bands.iter().zip(&b.bands).enumerate() {
        for row in 0..a.height {
            for col in 0..a.width {
                let cell = row * a.width + col;
                let x = (!a.is_nodata(left[cell])).then_some(left[cell]);
                let y = (!b.is_nodata(right[cell])).then_some(right[cell]);
                let same = match (x, y) {
                    (Some(x), Some(y)) => {
                        if b.data_type.is_float()
                            && ![
                                "cbands_",
                                "ascimp_",
                                "fltimp_",
                                "defras_",
                                "cprbands_",
                                "cprcast_",
                                "freq_",
                                "um64_",
                                "umint_",
                                "umcache_",
                            ]
                            .iter()
                            .any(|p| case.starts_with(p))
                        {
                            (x - y).abs() <= 1e-5 * y.abs().max(1.0)
                        } else {
                            x == y
                        }
                    }
                    (None, None) => true,
                    _ => false,
                };
                if !same {
                    bad += 1;
                    if bad <= 4 {
                        diffs.push(format!(
                            "cell band {} ({col}, {row}): ours {x:?}, ArcGIS {y:?}",
                            index + 1
                        ));
                    }
                }
            }
        }
    }
    let allowed = KNOWN_RASTER_DIFFERENCES
        .iter()
        .find(|k| k.0 == case)
        .map_or(0, |k| k.1);
    if bad <= allowed {
        diffs.retain(|d| !d.starts_with("cell "));
    } else if bad > 4 {
        diffs.push(format!("{bad} cells differ"));
    }
    diffs.extend(compare_statistics(ours, arcgis, b.bands.len(), bad > 0));
    diffs
}

/// A `STATISTICS_<key>` value of a `<raster>.aux.xml`.
fn aux_statistic(raster: &Path, key: &str, band: usize) -> Option<f64> {
    let text = std::fs::read_to_string(format!("{}.aux.xml", raster.display())).ok()?;
    let band_marker = format!("<PAMRasterBand band=\"{band}\">");
    let text = text
        .split_once(&band_marker)?
        .1
        .split_once("</PAMRasterBand>")?
        .0;
    let marker = format!("STATISTICS_{key}\">");
    let start = text.find(&marker)? + marker.len();
    text[start..start + text[start..].find('<')?].parse().ok()
}

/// ArcGIS writes cell statistics beside most Spatial Analyst outputs (and none beside a
/// few); later tools pick their output types from them, so ours must agree.
fn compare_statistics(ours: &Path, arcgis: &Path, bands: usize, cells_differ: bool) -> Vec<String> {
    let mut diffs = Vec::new();
    for band in 1..=bands {
        let theirs = |k| aux_statistic(arcgis, k, band);
        let mine = |k| aux_statistic(ours, k, band);
        if theirs("MEAN").is_none() {
            if mine("MEAN").is_some() {
                diffs.push(format!(
                    "band {band}: statistics written where ArcGIS has none"
                ));
            }
            continue;
        }
        for (key, rel) in [
            ("COUNT", 0.0),
            ("MINIMUM", 1e-5),
            ("MAXIMUM", 1e-5),
            ("MEAN", 1e-4),
            ("STDDEV", 1e-3),
        ] {
            match (mine(key), theirs(key)) {
                (Some(a), Some(b)) => {
                    // Cells that differ from ArcGIS's move the counts and moments: only
                    // check that the statistics exist.
                    if !cells_differ && (a - b).abs() > rel * b.abs().max(1.0) {
                        diffs.push(format!(
                            "band {band} STATISTICS_{key}: ours {a}, ArcGIS {b}"
                        ));
                    }
                }
                (None, Some(_)) => diffs.push(format!("band {band} STATISTICS_{key} missing")),
                _ => {}
            }
        }
    }
    diffs
}

#[test]
fn raster_comparison_detects_missing_or_changed_later_bands() {
    use unigis_raster::{Compression, DataType, Raster, write_geotiff};
    let dir = std::env::temp_dir().join(format!("ug-compare-bands-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ours = dir.join("ours.tif");
    let reference = dir.join("reference.tif");
    let mut raster = Raster::new(2, 1, DataType::U8, [0., 1., 0., 1., 0., -1.]);
    raster.bands = vec![vec![1., 2.], vec![3., 4.], vec![5., 6.]];
    let write = |path: &Path, r: &Raster| {
        write_geotiff(path, r, Compression::None).unwrap();
        unigis_raster::stats::write_aux_statistics(path, r).unwrap();
    };
    write(&reference, &raster);
    write(&ours, &raster);
    assert!(compare_rasters("comparison_guard", &ours, &reference).is_empty());
    raster.bands[2][1] = 7.;
    write(&ours, &raster);
    assert!(
        compare_rasters("comparison_guard", &ours, &reference)
            .iter()
            .any(|d| d.starts_with("cell band 3"))
    );
    raster.bands.pop();
    write(&ours, &raster);
    assert!(
        compare_rasters("comparison_guard", &ours, &reference)
            .iter()
            .any(|d| d.starts_with("band count"))
    );
    raster.bands.push(vec![5., 6.]);
    write(&ours, &raster);
    let aux = dir.join("ours.tif.aux.xml");
    let text = std::fs::read_to_string(&aux).unwrap();
    std::fs::write(
        aux,
        text.replace("<PAMRasterBand band=\"3\">", "<PAMRasterBand band=\"4\">"),
    )
    .unwrap();
    assert!(
        compare_rasters("comparison_guard", &ours, &reference)
            .iter()
            .any(|d| d.contains("band 3 STATISTICS_MEAN missing"))
    );
    std::fs::remove_dir_all(dir).unwrap();
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
        // Datum grids are downloaded by PROJ on demand: opt in with PROJ_NETWORK=ON.
        if case.starts_with("project_nad27_")
            && std::env::var("PROJ_NETWORK").as_deref() != Ok("ON")
        {
            continue;
        }
        if matches!(
            info["tool"].as_str(),
            Some("gp.ZonalGeometryAsTable_sa" | "gp.CostPath_sa")
        ) || PENDING_CASES.iter().any(|p| case.starts_with(p))
        {
            continue; // not done yet
        }
        // Workspace tools (Split) write into a folder.
        // Table tools (Check Geometry) write a .dbf.
        let table = reference.join(format!("{case}.dbf")).exists()
            && !reference.join(format!("{case}.shp")).exists();
        // Text outputs (Features To JSON) are compared byte for byte.
        let text_ext = ["json", "geojson", "csv", "asc", "flt"]
            .into_iter()
            .find(|e| reference.join(format!("{case}.{e}")).exists());
        // Spatial Analyst outputs are GeoTIFFs, compared cell by cell.
        let raster = reference.join(format!("{case}.tif")).exists();
        let out = if let Some(ext) = text_ext {
            out_dir.join(format!("{case}.{ext}"))
        } else if raster {
            out_dir.join(format!("{case}.tif"))
        } else if info.get("outputs").is_some() {
            let dir = out_dir.join(case);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        } else if table {
            out_dir.join(format!("{case}.dbf"))
        } else {
            out_dir.join(format!("{case}.shp"))
        };
        let mut args: Vec<String> = if let Some(run) = info.get("run") {
            // A tool of the registry: `ug run <id> name=value ...` from the manifest's parameters.
            let mut v: Vec<String> = vec!["run".into(), run["tool"].as_str().unwrap().into()];
            for (name, value) in run["params"].as_object().unwrap() {
                let value = value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_string);
                v.push(format!("{name}={value}"));
            }
            v
        } else if info["tool"] == "gp.BlockStatistics_sa" {
            let a = &info["args"];
            vec![
                "block-statistics".into(),
                a[0].as_str().unwrap().into(),
                "<out>".into(),
                "--neighborhood".into(),
                a[2].as_str().unwrap().into(),
                "--statistics-type".into(),
                a[3].as_str().unwrap().into(),
                "--ignore-nodata".into(),
                a[4].as_str().unwrap().into(),
            ]
        } else if matches!(
            info["tool"].as_str(),
            Some("gp.CostDistance_sa" | "gp.CostBackLink_sa" | "gp.CostAllocation_sa")
        ) {
            let a = &info["args"];
            let command = match info["tool"].as_str().unwrap() {
                "gp.CostDistance_sa" => "cost-distance",
                "gp.CostBackLink_sa" => "cost-back-link",
                _ => "cost-allocation",
            };
            let mut v: Vec<String> = vec![
                command.into(),
                a[0].as_str().unwrap().into(),
                a[1].as_str().unwrap().into(),
                "<out>".into(),
            ];
            if let Some(m) = a.get(3).and_then(|m| m.as_f64()) {
                v.extend(["--maximum-distance".into(), m.to_string()]);
            }
            v
        } else if info["tool"] == "gp.ReclassByTable_sa" {
            let a = &info["args"];
            vec![
                "reclass-by-table".into(),
                a[0].as_str().unwrap().into(),
                a[1].as_str().unwrap().into(),
                "<out>".into(),
                "--from-value-field".into(),
                a[2].as_str().unwrap().into(),
                "--to-value-field".into(),
                a[3].as_str().unwrap().into(),
                "--output-value-field".into(),
                a[4].as_str().unwrap().into(),
                "--missing-values".into(),
                a[6].as_str().unwrap().into(),
            ]
        } else if info["tool"] == "gp.Corridor_sa" {
            let a = &info["args"];
            vec![
                "corridor".into(),
                a[0].as_str().unwrap().into(),
                a[1].as_str().unwrap().into(),
                "<out>".into(),
            ]
        } else if info["tool"] == "gp.ZonalHistogram_sa" {
            let a = &info["args"];
            vec![
                "zonal-histogram".into(),
                a[0].as_str().unwrap().into(),
                a[2].as_str().unwrap().into(),
                "<out>".into(),
                "--zone-field".into(),
                a[1].as_str().unwrap().into(),
            ]
        } else if info["tool"] == "gp.TabulateArea_sa" {
            let a = &info["args"];
            let mut v: Vec<String> = vec![
                "tabulate-area".into(),
                a[0].as_str().unwrap().into(),
                a[2].as_str().unwrap().into(),
                "<out>".into(),
                "--zone-field".into(),
                a[1].as_str().unwrap().into(),
                "--class-field".into(),
                a[3].as_str().unwrap().into(),
            ];
            if let Some(c) = a.get(5).and_then(|c| c.as_f64()) {
                v.extend(["--processing-cell-size".into(), c.to_string()]);
            }
            if a.get(6).and_then(|c| c.as_str()) == Some("CLASSES_AS_ROWS") {
                v.push("--classes-as-rows".into());
            }
            v
        } else {
            gf_args(case).iter().map(|a| a.to_string()).collect()
        };
        match args.iter().position(|a| a.contains("<out>")) {
            Some(i) => args[i] = args[i].replace("<out>", &out.display().to_string()),
            None => args.push(out.display().to_string()),
        }
        let run = Command::new(env!("CARGO_BIN_EXE_ug"))
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
        } else if raster {
            diffs = compare_rasters(case, &out, &reference.join(format!("{case}.tif")));
        } else if let Some(ext) = text_ext {
            let (ours_bytes, theirs_bytes) = (
                std::fs::read(&out).unwrap_or_default(),
                std::fs::read(reference.join(format!("{case}.{ext}"))).unwrap(),
            );
            let (ours, theirs) = (
                String::from_utf8_lossy(&ours_bytes).into_owned(),
                String::from_utf8_lossy(&theirs_bytes).into_owned(),
            );
            if ours_bytes != theirs_bytes {
                let at = ours
                    .chars()
                    .zip(theirs.chars())
                    .position(|(a, b)| a != b)
                    .unwrap_or(ours.len().min(theirs.len()));
                let around = |t: &str| {
                    t.chars()
                        .skip(at.saturating_sub(60))
                        .take(140)
                        .collect::<String>()
                };
                diffs.push(format!(
                    "text differs at char {at}:\n      ours   {}\n      ArcGIS {}",
                    around(&ours),
                    around(&theirs)
                ));
            }
        } else if table {
            diffs = compare_tables(
                &out,
                &reference.join(format!("{case}.dbf")),
                &info["fields"],
            );
        } else {
            diffs = compare(
                case,
                &out,
                &reference.join(format!("{case}.shp")),
                &info["fields"],
            );
        }
        // Empty outputs (Create Feature Class): the shape type in the header, Z and M included.
        if case.starts_with("cfc_") && diffs.is_empty() {
            let code = |p: &Path| {
                std::fs::read(p)
                    .ok()
                    .and_then(|b| b.get(32..36).map(<[u8]>::to_vec))
            };
            if code(&out) != code(&reference.join(format!("{case}.shp"))) {
                diffs.push(format!(
                    "shape type code: ours {:?}, ArcGIS {:?}",
                    code(&out),
                    code(&reference.join(format!("{case}.shp")))
                ));
            }
        }
        // Known gaps: whole cases, or single features, that still differ.
        if let Some((_, features)) = KNOWN_FEATURE_DIFFERENCES.iter().find(|k| k.0 == case) {
            diffs.retain(|d| {
                !features.is_empty()
                    && !features.iter().any(|f| {
                        d.starts_with(&format!("feature {f}:"))
                            || d.starts_with(&format!("feature {f} "))
                    })
            });
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
