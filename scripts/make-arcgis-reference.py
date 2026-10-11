"""Regenerates testdata/arcgis-reference/ by running ArcGIS Pro tools through the
ArcGIS bridge (tools/arcgis-bridge). Needs ArcGIS Pro on this machine:

    uv run --with "mcp>=2" python scripts/make-arcgis-reference.py [case ...]

With case names, only those cases are rerun and the rest of the manifest is kept.

Each case writes a shapefile and records ArcGIS's messages and output schema in
manifest.json, which the Rust tests compare GeoForge against.
"""

import json
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools" / "arcgis-bridge"))
import server  # noqa: E402

DATA = ROOT / "testdata" / "geoproc"
OUT = ROOT / "testdata" / "arcgis-reference"

def win(path):
    """A path as ArcGIS under Wine sees it (for paths inside parameter strings)."""
    return "Z:" + str(path).replace("/", "\\")


def field_mapping(target, join, rules):
    """A Spatial Join field mapping string: the target's fields (First), then the join
    fields with the given merge rules (Join uses ';')."""
    def entry(src, name, ftype, length, rule):
        delim = '";"' if rule == "Join" else "#"
        span = f"0,{length - 1}" if ftype == "Text" else "-1,-1"
        prec = 0 if ftype in ("Text", "Double") else length
        return (f'{name} "{name}" true true false {length} {ftype} 0 {prec},{rule},{delim},'
                f"{win(src)},{name},{span}")
    parts = [entry(target, n, t, l, "First") for n, t, l in TARGET_FIELDS[target.stem]]
    parts += [entry(join, n, t, l, rules.get(n, "First")) for n, t, l in TARGET_FIELDS[join.stem]]
    return ";".join(parts)


TARGET_FIELDS = {
    "clip_areas": [("AREA_ID", "Long", 10)],
    "parcels": [("PARCEL_ID", "Long", 10), ("NAME", "Text", 20), ("VALUE", "Double", 19), ("ZONE", "Long", 5)],
}

CASES = {
    # name: (tool, args before the output, args after the output)
    "clip_parcels": ("analysis.Clip", [DATA / "parcels.shp", DATA / "clip_areas.shp"], []),
    "clip_roads": ("analysis.Clip", [DATA / "roads.shp", DATA / "clip_areas.shp"], []),
    "clip_wells": ("analysis.Clip", [DATA / "wells.shp", DATA / "clip_areas.shp"], []),
    "buffer_roads_25": ("analysis.Buffer", [DATA / "roads.shp"], ["25 Meters"]),
    "buffer_roads_25_flat": ("analysis.Buffer", [DATA / "roads.shp"], ["25 Meters", "FULL", "FLAT"]),
    "buffer_roads_25_all": ("analysis.Buffer", [DATA / "roads.shp"], ["25 Meters", "FULL", "ROUND", "ALL"]),
    "buffer_wells_30": ("analysis.Buffer", [DATA / "wells.shp"], ["30 Meters"]),
    "buffer_parcels_10": ("analysis.Buffer", [DATA / "parcels.shp"], ["10 Meters"]),
    "buffer_parcels_neg10": ("analysis.Buffer", [DATA / "parcels.shp"], ["-10 Meters"]),
    # Overlay: list-of-inputs tools take a ';'-joined string as their first argument.
    "intersect_parcels_clip": ("analysis.Intersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], []),
    "intersect_roads_parcels": ("analysis.Intersect", [f"{DATA / 'roads.shp'};{DATA / 'parcels.shp'}"], []),
    "intersect_wells_parcels": ("analysis.Intersect", [f"{DATA / 'wells.shp'};{DATA / 'parcels.shp'}"], []),
    "erase_parcels": ("analysis.Erase", [DATA / "parcels.shp", DATA / "clip_areas.shp"], []),
    "erase_roads": ("analysis.Erase", [DATA / "roads.shp", DATA / "clip_areas.shp"], []),
    "union_parcels_clip": ("analysis.Union", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], []),
    "dissolve_parcels_zone": ("management.Dissolve", [DATA / "parcels.shp"], ["ZONE", "VALUE SUM;PARCEL_ID COUNT"]),
    "dissolve_parcels_all": ("management.Dissolve", [DATA / "parcels.shp"], []),
    "dissolve_parcels_zone_single": ("management.Dissolve", [DATA / "parcels.shp"], ["ZONE", "", "SINGLE_PART"]),
    "dissolve_roads_class": ("management.Dissolve", [DATA / "roads.shp"], ["CLASS"]),
    "identity_parcels_clip": ("analysis.Identity", [DATA / "parcels.shp", DATA / "clip_areas.shp"], []),
    "identity_roads_parcels": ("analysis.Identity", [DATA / "roads.shp", DATA / "parcels.shp"], []),
    "identity_wells_clip": ("analysis.Identity", [DATA / "wells.shp", DATA / "clip_areas.shp"], []),
    "identity_parcels_clip_no_fid": ("analysis.Identity", [DATA / "parcels.shp", DATA / "clip_areas.shp"], ["NO_FID"]),
    "identity_roads_parcels_fid_only": ("analysis.Identity", [DATA / "roads.shp", DATA / "parcels.shp"], ["ONLY_FID"]),
    "identity_roads_parcels_relationships": ("analysis.Identity", [DATA / "roads.shp", DATA / "parcels.shp"], ["ALL", "", "KEEP_RELATIONSHIPS"]),
    "identity_roads_clip": ("analysis.Identity", [DATA / "roads.shp", DATA / "clip_areas.shp"], []),
    "identity_wells_clip_overlap": ("analysis.Identity", [DATA / "wells.shp", DATA / "clip_areas.shp"], ["ALL", "", "KEEP_RELATIONSHIPS"]),
    "identity_edges_squares_relationships": ("analysis.Identity", [DATA / "edges.shp", DATA / "squares.shp"], ["ALL", "", "KEEP_RELATIONSHIPS"]),
    "identity_edges_squares_fid_relationships": ("analysis.Identity", [DATA / "edges.shp", DATA / "squares.shp"], ["ONLY_FID", "", "KEEP_RELATIONSHIPS"]),
    "identity_edges_squares": ("analysis.Identity", [DATA / "edges.shp", DATA / "squares.shp"], []),
    "identity_roads_clip_relationships": ("analysis.Identity", [DATA / "roads.shp", DATA / "clip_areas.shp"], ["NO_FID", "", "KEEP_RELATIONSHIPS"]),
    "union_parcels_clip_no_fid": ("analysis.Union", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["NO_FID"]),
    "union_parcels_clip_fid_only": ("analysis.Union", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["ONLY_FID"]),
    "union_clip_no_gaps": ("analysis.Union", [f"{DATA / 'clip_areas.shp'}"], ["ALL", "", "NO_GAPS"]),
    "union_clip_gaps": ("analysis.Union", [f"{DATA / 'clip_areas.shp'}"], ["ALL", "", "GAPS"]),
    "merge_parcels_clip": ("management.Merge", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], []),
    # In-place tools (IN_PLACE) run on a copy of the first input, written as the output.
    "near_wells_roads": ("analysis.Near", [DATA / "wells.shp", DATA / "roads.shp"], []),
    "near_wells_roads_loc": ("analysis.Near", [DATA / "wells.shp", DATA / "roads.shp"], ["", "LOCATION", "ANGLE"]),
    "near_wells_roads_100": ("analysis.Near", [DATA / "wells.shp", DATA / "roads.shp"], ["100 Meters"]),
    "near_parcels_wells": ("analysis.Near", [DATA / "parcels.shp", DATA / "wells.shp"], ["", "LOCATION", "ANGLE"]),
    "sj_wells_parcels": ("analysis.SpatialJoin", [DATA / "wells.shp", DATA / "parcels.shp"], []),
    "sj_parcels_wells": ("analysis.SpatialJoin", [DATA / "parcels.shp", DATA / "wells.shp"], []),
    "sj_parcels_wells_common": ("analysis.SpatialJoin", [DATA / "parcels.shp", DATA / "wells.shp"], ["JOIN_ONE_TO_ONE", "KEEP_COMMON"]),
    "sj_parcels_wells_many": ("analysis.SpatialJoin", [DATA / "parcels.shp", DATA / "wells.shp"], ["JOIN_ONE_TO_MANY"]),
    "sj_roads_parcels": ("analysis.SpatialJoin", [DATA / "roads.shp", DATA / "parcels.shp"], []),
    "sj_wells_roads_closest": ("analysis.SpatialJoin", [DATA / "wells.shp", DATA / "roads.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL", "", "CLOSEST", "", "DIST"]),
    "sj_wells_parcels_within": ("analysis.SpatialJoin", [DATA / "wells.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL", "", "WITHIN"]),
    "sj_parcels_wells_contains": ("analysis.SpatialJoin", [DATA / "parcels.shp", DATA / "wells.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL", "", "CONTAINS"]),
    "sj_wells_roads_50": ("analysis.SpatialJoin", [DATA / "wells.shp", DATA / "roads.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL", "", "WITHIN_A_DISTANCE", "50 Meters", "DIST"]),
    "append_clip_to_parcels": ("management.Append", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["NO_TEST"]),
    "append_parcels_to_clip": ("management.Append", [DATA / "parcels.shp", DATA / "clip_areas.shp"], ["NO_TEST"]),
    "append_parcels_to_parcels": ("management.Append", [DATA / "parcels.shp", DATA / "parcels.shp"], ["TEST"]),
    "near_wells_roads_clip": ("analysis.Near", [DATA / "wells.shp", f"{DATA / 'roads.shp'};{DATA / 'clip_areas.shp'}"], ["", "LOCATION", "ANGLE"]),
    # Split writes one shapefile per split value into a folder (WORKSPACE_OUT).
    "split_parcels_clip": ("analysis.Split", [DATA / "parcels.shp", DATA / "clip_areas.shp", "AREA_ID"], []),
    "split_roads_parcels_name": ("analysis.Split", [DATA / "roads.shp", DATA / "parcels.shp", "NAME"], []),
    "split_wells_parcels_name": ("analysis.Split", [DATA / "wells.shp", DATA / "parcels.shp", "NAME"], []),
    "split_parcels_parcels_name": ("analysis.Split", [DATA / "parcels.shp", DATA / "parcels.shp", "NAME"], []),
    # Intersect output_type: args after the output are join_attributes, cluster_tolerance, output_type.
    "intersect_roads_streams": ("analysis.Intersect", [f"{DATA / 'roads.shp'};{DATA / 'streams.shp'}"], []),
    "intersect_roads_streams_point": ("analysis.Intersect", [f"{DATA / 'roads.shp'};{DATA / 'streams.shp'}"], ["ALL", "", "POINT"]),
    "intersect_roads_parcels_point": ("analysis.Intersect", [f"{DATA / 'roads.shp'};{DATA / 'parcels.shp'}"], ["ALL", "", "POINT"]),
    "intersect_parcels_clip_line": ("analysis.Intersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["ALL", "", "LINE"]),
    "intersect_parcels_clip_point": ("analysis.Intersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["ALL", "", "POINT"]),
    "intersect_parcels_clip_fid_only": ("analysis.Intersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["ONLY_FID"]),
    "intersect_parcels_clip_no_fid": ("analysis.Intersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["NO_FID"]),
    "merge_parcels_clip_source": ("management.Merge", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["", "ADD_SOURCE_INFO"]),
    "check_bad_esri": ("management.CheckGeometry", [f"{DATA / 'bad_polygons.shp'};{DATA / 'bad_lines.shp'}"], ["ESRI"]),
    "check_bad_ogc": ("management.CheckGeometry", [f"{DATA / 'bad_polygons.shp'};{DATA / 'bad_lines.shp'}"], ["OGC"]),
    "check_parcels": ("management.CheckGeometry", [DATA / "parcels.shp"], []),
    "repair_bad_polygons_esri": ("management.RepairGeometry", [DATA / "bad_polygons.shp"], ["DELETE_NULL", "ESRI"]),
    "repair_bad_polygons_ogc_keep": ("management.RepairGeometry", [DATA / "bad_polygons.shp"], ["KEEP_NULL", "OGC"]),
    "repair_bad_lines_esri_keep": ("management.RepairGeometry", [DATA / "bad_lines.shp"], ["KEEP_NULL", "ESRI"]),
    "repair_bad_lines_ogc": ("management.RepairGeometry", [DATA / "bad_lines.shp"], ["DELETE_NULL", "OGC"]),
    "clip_roads_zm": ("analysis.Clip", [DATA / "roads_zm.shp", DATA / "clip_areas.shp"], []),
    "erase_roads_zm": ("analysis.Erase", [DATA / "roads_zm.shp", DATA / "clip_areas.shp"], []),
    "clip_parcels_z": ("analysis.Clip", [DATA / "parcels_z.shp", DATA / "clip_areas.shp"], []),
    "intersect_roads_zm_parcels": ("analysis.Intersect", [f"{DATA / 'roads_zm.shp'};{DATA / 'parcels.shp'}"], []),
    "intersect_roads_zm_parcels_z": ("analysis.Intersect", [f"{DATA / 'roads_zm.shp'};{DATA / 'parcels_z.shp'}"], []),
    "intersect_parcels_z_clip": ("analysis.Intersect", [f"{DATA / 'parcels_z.shp'};{DATA / 'clip_areas.shp'}"], []),
    "intersect_roads_zm_parcels_point": ("analysis.Intersect", [f"{DATA / 'roads_zm.shp'};{DATA / 'parcels_z.shp'}"], ["ALL", "", "POINT"]),
    "identity_roads_zm_parcels": ("analysis.Identity", [DATA / "roads_zm.shp", DATA / "parcels.shp"], []),
    "union_parcels_z_clip": ("analysis.Union", [f"{DATA / 'parcels_z.shp'};{DATA / 'clip_areas.shp'}"], []),
    "split_roads_zm_parcels_name": ("analysis.Split", [DATA / "roads_zm.shp", DATA / "parcels.shp", "NAME"], []),
    "intersect_3_line": ("analysis.Intersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'};{DATA / 'roads.shp'}"], ["ALL", "", "LINE"]),
    "intersect_tri_lines_point": ("analysis.Intersect", [f"{DATA / 'tri_a.shp'};{DATA / 'tri_b.shp'};{DATA / 'tri_c.shp'}"], ["ALL", "", "POINT"]),
    "intersect_tri_polys_line": ("analysis.Intersect", [f"{DATA / 'tri_p.shp'};{DATA / 'tri_q.shp'};{DATA / 'tri_r.shp'}"], ["ALL", "", "LINE"]),
    "intersect_tri_polys_point": ("analysis.Intersect", [f"{DATA / 'tri_p.shp'};{DATA / 'tri_q.shp'};{DATA / 'tri_r.shp'}"], ["ALL", "", "POINT"]),
    "intersect_tri_mixed_point": ("analysis.Intersect", [f"{DATA / 'tri_p.shp'};{DATA / 'tri_a.shp'};{DATA / 'tri_c.shp'}"], ["ALL", "", "POINT"]),
    "buffer_roads_25_left": ("analysis.Buffer", [DATA / "roads.shp"], ["25 Meters", "LEFT"]),
    "buffer_roads_25_right_flat": ("analysis.Buffer", [DATA / "roads.shp"], ["25 Meters", "RIGHT", "FLAT"]),
    "buffer_parcels_10_outside": ("analysis.Buffer", [DATA / "parcels.shp"], ["10 Meters", "OUTSIDE_ONLY"]),
    "buffer_parcels_10_list_zone": ("analysis.Buffer", [DATA / "parcels.shp"], ["10 Meters", "FULL", "ROUND", "LIST", "ZONE"]),
    "buffer_roads_20_list_class": ("analysis.Buffer", [DATA / "roads.shp"], ["20 Meters", "FULL", "ROUND", "LIST", "CLASS"]),
    "buffer_wells_depth_field": ("analysis.Buffer", [DATA / "wells.shp"], ["DEPTH"]),
    "sj_rules_a": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL",
        field_mapping(DATA / "clip_areas.shp", DATA / "parcels.shp", {"PARCEL_ID": "Mean", "VALUE": "Sum", "ZONE": "Mode", "NAME": "Last"}), "INTERSECT"]),
    "sj_rules_b": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL",
        field_mapping(DATA / "clip_areas.shp", DATA / "parcels.shp", {"PARCEL_ID": "Median", "VALUE": "StdDev", "ZONE": "Range", "NAME": "Count"}), "INTERSECT"]),
    "sj_rules_c": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL",
        field_mapping(DATA / "clip_areas.shp", DATA / "parcels.shp", {"PARCEL_ID": "Max", "VALUE": "Min", "ZONE": "Count", "NAME": "Join"}), "INTERSECT"]),
    "sj_rules_d": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL",
        field_mapping(DATA / "clip_areas.shp", DATA / "parcels.shp", {"PARCEL_ID": "Range", "VALUE": "Mean", "ZONE": "Median", "NAME": "First"}), "WITHIN_A_DISTANCE", "60 Meters"]),
    "project_wells_4326": ("management.Project", [DATA / "wells.shp"], [4326]),
    "project_parcels_3857": ("management.Project", [DATA / "parcels.shp"], [3857]),
    "project_roads_32634": ("management.Project", [DATA / "roads.shp"], [32634]),
    "project_wells_wgs84_32633": ("management.Project", [DATA / "wells_wgs84.shp"], [32633]),
    "project_parcels_z_4326": ("management.Project", [DATA / "parcels_z.shp"], [4326]),
    "m2s_roads": ("management.MultipartToSinglepart", [DATA / "roads.shp"], []),
    "m2s_parcels_clip": ("management.MultipartToSinglepart", [OUT / "clip_parcels.shp"], []),
    "f2p_parcels_centroid": ("management.FeatureToPoint", [DATA / "parcels.shp"], ["CENTROID"]),
    "f2p_parcels_inside": ("management.FeatureToPoint", [DATA / "parcels.shp"], ["INSIDE"]),
    "f2p_roads_centroid": ("management.FeatureToPoint", [DATA / "roads.shp"], ["CENTROID"]),
    "f2p_roads_inside": ("management.FeatureToPoint", [DATA / "roads.shp"], ["INSIDE"]),
    "fv2p_roads_all": ("management.FeatureVerticesToPoints", [DATA / "roads.shp"], ["ALL"]),
    "fv2p_roads_start": ("management.FeatureVerticesToPoints", [DATA / "roads.shp"], ["START"]),
    "fv2p_roads_end": ("management.FeatureVerticesToPoints", [DATA / "roads.shp"], ["END"]),
    "fv2p_roads_both": ("management.FeatureVerticesToPoints", [DATA / "roads.shp"], ["BOTH_ENDS"]),
    "fv2p_roads_mid": ("management.FeatureVerticesToPoints", [DATA / "roads.shp"], ["MID"]),
    "fv2p_roads_dangle": ("management.FeatureVerticesToPoints", [DATA / "roads.shp"], ["DANGLE"]),
    "fv2p_parcels_all": ("management.FeatureVerticesToPoints", [DATA / "parcels.shp"], ["ALL"]),
    "mbg_parcels_envelope": ("management.MinimumBoundingGeometry", [DATA / "parcels.shp"], ["ENVELOPE"]),
    "mbg_roads_hull": ("management.MinimumBoundingGeometry", [DATA / "roads.shp"], ["CONVEX_HULL"]),
    "mbg_roads_rect_area": ("management.MinimumBoundingGeometry", [DATA / "roads.shp"], ["RECTANGLE_BY_AREA"]),
    "mbg_roads_rect_width": ("management.MinimumBoundingGeometry", [DATA / "roads.shp"], ["RECTANGLE_BY_WIDTH"]),
    "mbg_roads_circle": ("management.MinimumBoundingGeometry", [DATA / "roads.shp"], ["CIRCLE"]),
    "mbg_wells_hull_all": ("management.MinimumBoundingGeometry", [DATA / "wells.shp"], ["CONVEX_HULL", "ALL"]),
    "mbg_parcels_env_zone_mbg": ("management.MinimumBoundingGeometry", [DATA / "parcels.shp"], ["ENVELOPE", "LIST", "ZONE", "MBG_FIELDS"]),
    "addxy_wells": ("management.AddXY", [DATA / "wells.shp"], []),
    "select_parcels_value": ("analysis.Select", [DATA / "parcels.shp"], ["VALUE > 5000"]),
    "select_parcels_in_and": ("analysis.Select", [DATA / "parcels.shp"], ["ZONE IN (1, 3) AND VALUE < 8000"]),
    "select_parcels_like": ("analysis.Select", [DATA / "parcels.shp"], ["NAME LIKE 'Lot 1%'"]),
    "select_parcels_between_or": ("analysis.Select", [DATA / "parcels.shp"], ["PARCEL_ID BETWEEN 10 AND 20 OR ZONE = 0"]),
    "select_parcels_not": ("analysis.Select", [DATA / "parcels.shp"], ["NOT ZONE = 2"]),
    "select_parcels_quoted": ("analysis.Select", [DATA / "parcels.shp"], ['"NAME" = \'Lot 7\'']),
    "select_parcels_parens": ("analysis.Select", [DATA / "parcels.shp"], ["(ZONE = 1 OR ZONE = 2) AND NOT (VALUE >= 3000 AND VALUE <= 6000)"]),
    "select_parcels_like_underscore": ("analysis.Select", [DATA / "parcels.shp"], ["NAME LIKE 'Lot _'"]),
    "select_parcels_not_in": ("analysis.Select", [DATA / "parcels.shp"], ["ZONE NOT IN (0, 1)"]),
    "select_wells_depth": ("analysis.Select", [DATA / "wells.shp"], ["DEPTH >= 100.5"]),
    "select_wells_none": ("analysis.Select", [DATA / "wells.shp"], ["DEPTH < 0"]),
    "select_parcels_all": ("analysis.Select", [DATA / "parcels.shp"], []),
    "stats_parcels_zone": ("analysis.Statistics", [DATA / "parcels.shp"], ["VALUE SUM;VALUE MEAN;VALUE MIN;VALUE MAX;VALUE RANGE;VALUE STD;VALUE COUNT;PARCEL_ID FIRST;PARCEL_ID LAST;NAME FIRST", "ZONE"]),
    "stats_parcels_all": ("analysis.Statistics", [DATA / "parcels.shp"], ["VALUE SUM;VALUE MEAN;PARCEL_ID COUNT"]),
    "stats_wells_median": ("analysis.Statistics", [DATA / "wells.shp"], ["DEPTH MEDIAN;DEPTH VARIANCE;DEPTH UNIQUE"]),
    "freq_parcels_zone": ("analysis.Frequency", [DATA / "parcels.shp"], ["ZONE", "VALUE"]),
    "freq_roads_class": ("analysis.Frequency", [DATA / "roads.shp"], ["CLASS"]),
    "aga_parcels": ("management.AddGeometryAttributes", [DATA / "parcels.shp"], ["AREA;PERIMETER_LENGTH;CENTROID;CENTROID_INSIDE;EXTENT;PART_COUNT;POINT_COUNT"]),
    "aga_roads": ("management.AddGeometryAttributes", [DATA / "roads.shp"], ["LENGTH;LINE_START_MID_END;LINE_BEARING;PART_COUNT;POINT_COUNT;EXTENT"]),
    "aga_roads_km": ("management.AddGeometryAttributes", [DATA / "roads.shp"], ["LENGTH", "KILOMETERS"]),
    "aga_parcels_ha": ("management.AddGeometryAttributes", [DATA / "parcels.shp"], ["AREA", "", "HECTARES"]),
    "points_to_line_wells": ("management.PointsToLine", [DATA / "wells.shp"], []),
    "points_to_line_wells_sorted": ("management.PointsToLine", [DATA / "wells.shp"], ["", "DEPTH"]),
    "gnt_wells_roads": ("analysis.GenerateNearTable", [DATA / "wells.shp", DATA / "roads.shp"], []),
    "gnt_wells_roads_all": ("analysis.GenerateNearTable", [DATA / "wells.shp", DATA / "roads.shp"], ["100 Meters", "LOCATION", "ANGLE", "ALL", 2]),
    "find_identical_parcels_zone": ("management.FindIdentical", [DATA / "parcels.shp"], ["ZONE"]),
    "find_identical_parcels_shape": ("management.FindIdentical", [DATA / "parcels.shp"], ["Shape", "", "", "ONLY_DUPLICATES"]),
    "json_roads": ("conversion.FeaturesToJSON", [DATA / "roads.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "NO_GEOJSON"]),
    "json_roads_formatted": ("conversion.FeaturesToJSON", [DATA / "roads.shp"], ["FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "NO_GEOJSON"]),
    "json_parcels": ("conversion.FeaturesToJSON", [DATA / "parcels.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "NO_GEOJSON"]),
    "json_wells": ("conversion.FeaturesToJSON", [DATA / "wells.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "NO_GEOJSON"]),
    "json_wells_wgs84": ("conversion.FeaturesToJSON", [DATA / "wells_wgs84.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "NO_GEOJSON"]),
    "geojson_roads": ("conversion.FeaturesToJSON", [DATA / "roads.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "GEOJSON"]),
    "geojson_parcels": ("conversion.FeaturesToJSON", [DATA / "parcels.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "GEOJSON"]),
    "geojson_parcels_formatted": ("conversion.FeaturesToJSON", [DATA / "parcels.shp"], ["FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "GEOJSON"]),
    "geojson_wells": ("conversion.FeaturesToJSON", [DATA / "wells.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "GEOJSON"]),
    "json_roads_zm": ("conversion.FeaturesToJSON", [DATA / "roads_zm.shp"], ["NOT_FORMATTED", "Z_VALUES", "M_VALUES", "NO_GEOJSON"]),
    "json_bad_lines": ("conversion.FeaturesToJSON", [DATA / "bad_lines.shp"], ["NOT_FORMATTED", "NO_Z_VALUES", "NO_M_VALUES", "NO_GEOJSON"]),
}
WORKSPACE_OUT = {"analysis.Split"}
# Tools that write a text file: compared byte for byte.
TEXT_OUT = {"conversion.FeaturesToJSON"}
# Tools whose output is a table, written as <name>.dbf.
TABLE_OUT = {"management.CheckGeometry", "analysis.Statistics", "analysis.Frequency", "analysis.GenerateNearTable", "management.FindIdentical"}

# In-place tools: the index of the input they edit (run on a copy written as the output).
IN_PLACE = {"analysis.Near": 0, "management.Append": 1, "management.RepairGeometry": 0, "management.AddXY": 0, "management.AddGeometryAttributes": 0}

# Select Layer By Location cases, saved with Copy Features:
# name: (in_layer, overlap_type, select_features, search_distance, invert)
SELECT = {
    "sel_parcels_intersect_clip": ("parcels", "INTERSECT", "clip_areas", "", False),
    "sel_parcels_intersect_clip_invert": ("parcels", "INTERSECT", "clip_areas", "", True),
    "sel_parcels_intersect_roads_10": ("parcels", "INTERSECT", "roads", "10 Meters", False),
    "sel_wells_dist_roads_30": ("wells", "WITHIN_A_DISTANCE", "roads", "30 Meters", False),
    "sel_parcels_dist_wells_5": ("parcels", "WITHIN_A_DISTANCE", "wells", "5 Meters", False),
    "sel_parcels_within_clip": ("parcels", "WITHIN", "clip_areas", "", False),
    "sel_parcels_completely_within_clip": ("parcels", "COMPLETELY_WITHIN", "clip_areas", "", False),
    "sel_parcels_within_clementini_clip": ("parcels", "WITHIN_CLEMENTINI", "clip_areas", "", False),
    "sel_wells_within_parcels": ("wells", "WITHIN", "parcels", "", False),
    "sel_wells_within_clementini_parcels": ("wells", "WITHIN_CLEMENTINI", "parcels", "", False),
    "sel_parcels_contains_wells": ("parcels", "CONTAINS", "wells", "", False),
    "sel_parcels_completely_contains_wells": ("parcels", "COMPLETELY_CONTAINS", "wells", "", False),
    "sel_parcels_contains_clementini_wells": ("parcels", "CONTAINS_CLEMENTINI", "wells", "", False),
    "sel_parcels_center_in_clip": ("parcels", "HAVE_THEIR_CENTER_IN", "clip_areas", "", False),
    "sel_parcels_touch_clip": ("parcels", "BOUNDARY_TOUCHES", "clip_areas", "", False),
    "sel_parcels_crossed_by_roads": ("parcels", "CROSSED_BY_THE_OUTLINE_OF", "roads", "", False),
    "sel_parcels_share_segment_clip": ("parcels", "SHARE_A_LINE_SEGMENT_WITH", "clip_areas", "", False),
    "sel_roads_crossed_by_parcels": ("roads", "CROSSED_BY_THE_OUTLINE_OF", "parcels", "", False),
    "sel_parcels_identical_parcels": ("parcels", "ARE_IDENTICAL_TO", "parcels", "", False),
}


def main():
    only = set(sys.argv[1:])
    unknown = only - set(CASES) - set(SELECT)
    if unknown:
        sys.exit(f"unknown cases: {sorted(unknown)}")
    w = server.worker
    info = w.call("info")
    manifest = {"arcgis": {k: info.get(k) for k in ("product", "version", "build", "license")}, "cases": {}}
    if only:
        old = json.loads((OUT / "manifest.json").read_text())
        manifest["cases"] = old["cases"]
        for name in only:
            for f in OUT.glob(f"{name}.*"):
                f.unlink()
            if (OUT / name).is_dir():
                shutil.rmtree(OUT / name)
    else:
        if OUT.exists():
            shutil.rmtree(OUT)
        OUT.mkdir(parents=True)
    for name, (tool, before, after) in CASES.items():
        if only and name not in only:
            continue
        out = OUT / (f"{name}.dbf" if tool in TABLE_OUT else f"{name}.shp")
        if tool in TEXT_OUT:
            out = OUT / (f"{name}.geojson" if "GEOJSON" in after else f"{name}.json")
        if tool in WORKSPACE_OUT:
            out = OUT / name
            out.mkdir()
            args = [str(a) for a in before] + [str(out)] + after
        elif tool in IN_PLACE:
            k = IN_PLACE[tool]
            w.call("run_tool", tool="management.CopyFeatures", args=[str(before[k]), str(out)],
                   env={"overwriteOutput": True})
            args = [str(out) if i == k else str(a) for i, a in enumerate(before)] + after
        else:
            args = [str(a) for a in before] + [str(out)] + after
        r = w.call("run_tool", tool=tool, args=args, env={"overwriteOutput": True})
        if tool in WORKSPACE_OUT:
            outputs = {}
            for shp in sorted(out.glob("*.shp")):
                d = w.call("describe", path=str(shp))
                outputs[shp.name] = {
                    "fields": [[f["name"], f["type"], f["length"], f["precision"], f["scale"]] for f in d.get("fields", [])],
                    "shapeType": d.get("shapeType"),
                }
            manifest["cases"][name] = {
                "tool": tool,
                "args": [str(Path(a).relative_to(ROOT)) if isinstance(a, Path) else a for a in before] + ["<out>"] + after,
                "status": r.get("status"),
                "messages": [m["text"] for m in r.get("messages", []) if m["severity"] > 0],
                "outputs": outputs,
            }
            print(f"{name}: {r.get('status')} {sorted(outputs)} {manifest['cases'][name]['messages']}")
            continue
        d = w.call("describe", path=str(out)) if r.get("status") == "succeeded" and tool not in TEXT_OUT else {}
        manifest["cases"][name] = {
            "tool": tool,
            "args": [str(Path(a).relative_to(ROOT)) if isinstance(a, Path) else a.replace(str(ROOT) + "/", "") for a in before] + ["<out>"] + after,
            "status": r.get("status"),
            "messages": [m["text"] for m in r.get("messages", []) if m["severity"] > 0],
            "fields": [[f["name"], f["type"], f["length"], f["precision"], f["scale"]] for f in d.get("fields", [])],
            "shapeType": d.get("shapeType"),
        }
        print(f"{name}: {r.get('status')} {manifest['cases'][name]['messages']}")
    for name, (layer, overlap, select, distance, invert) in SELECT.items():
        if only and name not in only:
            continue
        out = OUT / f"{name}.shp"
        lyr = f"lyr_{name}"
        w.call("run_tool", tool="management.MakeFeatureLayer", args=[str(DATA / f"{layer}.shp"), lyr],
               env={"overwriteOutput": True})
        r = w.call("run_tool", tool="management.SelectLayerByLocation",
                   args=[lyr, overlap, str(DATA / f"{select}.shp"), distance, "NEW_SELECTION",
                         "INVERT" if invert else "NOT_INVERT"])
        if r.get("status") == "succeeded":
            r = w.call("run_tool", tool="management.CopyFeatures", args=[lyr, str(out)],
                       env={"overwriteOutput": True})
        d = w.call("describe", path=str(out)) if r.get("status") == "succeeded" and tool not in TEXT_OUT else {}
        manifest["cases"][name] = {
            "tool": "management.SelectLayerByLocation",
            "args": [f"testdata/geoproc/{layer}.shp", overlap, f"testdata/geoproc/{select}.shp",
                     distance, "NEW_SELECTION", "INVERT" if invert else "NOT_INVERT"],
            "status": r.get("status"),
            "messages": [m["text"] for m in r.get("messages", []) if m["severity"] > 0],
            "fields": [[f["name"], f["type"], f["length"], f["precision"], f["scale"]] for f in d.get("fields", [])],
            "shapeType": d.get("shapeType"),
        }
        print(f"{name}: {r.get('status')} {manifest['cases'][name]['messages']}")
        w.call("run_tool", tool="management.Delete", args=[lyr])
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    w.stop()


if __name__ == "__main__":
    main()
