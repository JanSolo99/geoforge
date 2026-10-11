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
        other => panic!("no gf command for case {other}"),
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
fn zm_diff(a: &geoforge_core::Geometry, b: &geoforge_core::Geometry) -> Option<String> {
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
    let dbf = geoforge_shapefile::dbf::DbfReader::open(path, Some("UTF-8")).unwrap();
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
    let dbf = geoforge_shapefile::dbf::DbfReader::open(ours, Some("UTF-8")).unwrap();
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
    // Paths (CLASS) compare by file name, numbers within the usual noise.
    for (i, (ra, rb)) in a.iter().zip(&b).enumerate() {
        if ra.len() != rb.len() || ra.iter().zip(rb).any(|(x, y)| !same_value(x, y)) {
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

    let a: Vec<_> = ours.features().map(Result::unwrap).collect();
    let b: Vec<_> = theirs.features().map(Result::unwrap).collect();
    if a.len() != b.len() {
        diffs.push(format!("count: ours {}, ArcGIS {}", a.len(), b.len()));
    }
    // Clip agrees within ArcGIS's own XY tolerance (0.001 m: coordinates closer
    // than that are the same point to ArcGIS). Buffers are densified differently
    // (vertex placement on arcs), so compare shape within 2 × the 0.1 m deviation.
    // Circles are densified arcs, like buffers.
    let is_buffer = case.starts_with("buffer") || case.ends_with("_circle");
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
        let shape = |g: &Option<geoforge_core::Geometry>| g.clone().filter(|g| !g.xy.is_empty());
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
        // Table tools (Check Geometry) write a .dbf.
        let table = reference.join(format!("{case}.dbf")).exists()
            && !reference.join(format!("{case}.shp")).exists();
        // Text outputs (Features To JSON) are compared byte for byte.
        let text_ext = ["json", "geojson", "csv"]
            .into_iter()
            .find(|e| reference.join(format!("{case}.{e}")).exists());
        let out = if let Some(ext) = text_ext {
            out_dir.join(format!("{case}.{ext}"))
        } else if info.get("outputs").is_some() {
            let dir = out_dir.join(case);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        } else if table {
            out_dir.join(format!("{case}.dbf"))
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
        } else if let Some(ext) = text_ext {
            let (ours, theirs) = (
                std::fs::read_to_string(&out).unwrap_or_default(),
                std::fs::read_to_string(reference.join(format!("{case}.{ext}"))).unwrap(),
            );
            if ours != theirs {
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
