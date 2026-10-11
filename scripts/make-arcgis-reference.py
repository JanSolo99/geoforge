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
RASTER = ROOT / "testdata" / "raster"
OUT = ROOT / "testdata" / "arcgis-reference"

def rel(a):
    """A case argument as recorded in the manifest: paths relative to the repository."""
    if isinstance(a, Path):
        return str(a.relative_to(ROOT))
    if isinstance(a, str):
        return a.replace(str(ROOT) + "/", "")
    return a


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
    "j2f_parcels": ("conversion.JSONToFeatures", [OUT / "json_parcels.json"], []),
    "j2f_roads_zm": ("conversion.JSONToFeatures", [OUT / "json_roads_zm.json"], []),
    "j2f_bad_lines": ("conversion.JSONToFeatures", [OUT / "json_bad_lines.json"], []),
    "j2f_geojson_parcels": ("conversion.JSONToFeatures", [OUT / "geojson_parcels.geojson"], ["POLYGON"]),
    "j2f_geojson_wells": ("conversion.JSONToFeatures", [OUT / "geojson_wells.geojson"], ["POINT"]),
    "j2f_geojson_roads": ("conversion.JSONToFeatures", [OUT / "geojson_roads.geojson"], ["POLYLINE"]),
    "csv_parcels": ("conversion.ExportTable", [DATA / "parcels.shp"], []),
    "csv_wells": ("conversion.ExportTable", [DATA / "wells.shp"], []),
    "csv_bad_lines": ("conversion.ExportTable", [DATA / "bad_lines.shp"], []),
    "csv_stations_back": ("conversion.ExportTable", [DATA / "stations.csv"], []),
    "xy_stations": ("management.XYTableToPoint", [DATA / "stations.csv"], ["X", "Y", "", 32633]),
    "xy_stations_z": ("management.XYTableToPoint", [DATA / "stations.csv"], ["X", "Y", "ELEV", 32633]),
    "slope_dem": ("gp.Slope_sa", [RASTER / "dem.tif"], ["DEGREE", 1, "PLANAR", "METER"]),
    "slope_dem_percent_z": ("gp.Slope_sa", [RASTER / "dem.tif"], ["PERCENT_RISE", 2.5, "PLANAR", "METER"]),
    "slope_dem_nodata": ("gp.Slope_sa", [RASTER / "dem_nodata.tif"], ["DEGREE", 1, "PLANAR", "METER"]),
    "slope_dem_int16": ("gp.Slope_sa", [RASTER / "dem_int16_packbits.tif"], ["DEGREE", 1, "PLANAR", "METER"]),
    "aspect_dem": ("gp.Aspect_sa", [RASTER / "dem.tif"], ["PLANAR", "METER"]),
    "aspect_dem_nodata": ("gp.Aspect_sa", [RASTER / "dem_nodata.tif"], ["PLANAR", "METER"]),
    "aspect_landuse": ("gp.Aspect_sa", [RASTER / "landuse_u8.tif"], ["PLANAR", "METER"]),
    "hillshade_dem": ("gp.HillShade_sa", [RASTER / "dem.tif"], [315, 45, "NO_SHADOWS", 1]),
    "hillshade_dem_nodata": ("gp.HillShade_sa", [RASTER / "dem_nodata.tif"], [200, 30, "NO_SHADOWS", 1]),
    "hillshade_dem_z3": ("gp.HillShade_sa", [RASTER / "dem.tif"], [315, 45, "NO_SHADOWS", 3]),
    "focal_mean_nodata": ("gp.FocalStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "MEAN", "DATA"]),
    "focal_mean_nodata_strict": ("gp.FocalStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "MEAN", "NODATA"]),
    "focal_max_int16_5x3": ("gp.FocalStatistics_sa", [RASTER / "dem_int16_packbits.tif"], ["Rectangle 5 3 CELL", "MAXIMUM", "DATA"]),
    "focal_min_int16_4x4": ("gp.FocalStatistics_sa", [RASTER / "dem_int16_packbits.tif"], ["Rectangle 4 4 CELL", "MINIMUM", "DATA"]),
    "focal_std_circle2": ("gp.FocalStatistics_sa", [RASTER / "dem.tif"], ["Circle 2 CELL", "STD", "DATA"]),
    "focal_range_annulus": ("gp.FocalStatistics_sa", [RASTER / "dem_nodata.tif"], ["Annulus 1 3 CELL", "RANGE", "DATA"]),
    "focal_majority_landuse": ("gp.FocalStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 3 CELL", "MAJORITY", "DATA"]),
    "focal_minority_landuse": ("gp.FocalStatistics_sa", [RASTER / "landuse_u8.tif"], ["Circle 2 CELL", "MINORITY", "DATA"]),
    "focal_sum_landuse": ("gp.FocalStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 3 CELL", "SUM", "DATA"]),
    "focal_sum_dem": ("gp.FocalStatistics_sa", [RASTER / "dem.tif"], ["Rectangle 3 3 CELL", "SUM", "DATA"]),
    "focal_median_int16": ("gp.FocalStatistics_sa", [RASTER / "dem_int16_packbits.tif"], ["Rectangle 3 3 CELL", "MEDIAN", "DATA"]),
    "focal_median_f64": ("gp.FocalStatistics_sa", [RASTER / "dem_f64.tif"], ["Rectangle 3 3 CELL", "MEDIAN", "DATA"]),
    "focal_variety_landuse": ("gp.FocalStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 3 CELL", "VARIETY", "DATA"]),
    "focal_max_f64": ("gp.FocalStatistics_sa", [RASTER / "dem_f64.tif"], ["Circle 1.5 CELL", "MAXIMUM", "DATA"]),
    "fill_hydro": ("gp.Fill_sa", [RASTER / "hydro.tif"], []),
    "fill_hydro_nodata": ("gp.Fill_sa", [RASTER / "hydro_nodata.tif"], []),
    "fill_hydro_int": ("gp.Fill_sa", [RASTER / "hydro_int.tif"], []),
    "fill_hydro_zlimit": ("gp.Fill_sa", [RASTER / "hydro.tif"], [3]),
    "flowdir_hydro_filled": ("gp.FlowDirection_sa", [OUT / "fill_hydro.tif"], ["NORMAL", "", "D8"]),
    "flowdir_hydro_force": ("gp.FlowDirection_sa", [OUT / "fill_hydro.tif"], ["FORCE", "", "D8"]),
    "flowdir_hydro_raw": ("gp.FlowDirection_sa", [RASTER / "hydro.tif"], ["NORMAL", "", "D8"]),
    "flowdir_hydro_nodata": ("gp.FlowDirection_sa", [OUT / "fill_hydro_nodata.tif"], ["NORMAL", "", "D8"]),
    "flowdir_hydro_int": ("gp.FlowDirection_sa", [OUT / "fill_hydro_int.tif"], ["NORMAL", "", "D8"]),
    "flowacc_hydro": ("gp.FlowAccumulation_sa", [OUT / "flowdir_hydro_filled.tif"], ["", "FLOAT", "D8"]),
    "flowacc_hydro_int": ("gp.FlowAccumulation_sa", [OUT / "flowdir_hydro_filled.tif"], ["", "INTEGER", "D8"]),
    "flowacc_hydro_nodata": ("gp.FlowAccumulation_sa", [OUT / "flowdir_hydro_nodata.tif"], ["", "FLOAT", "D8"]),
    "reclass_landuse": ("gp.Reclassify_sa", [RASTER / "landuse_u8.tif", "Value", "0 10;1 10;2 20;3 30"], ["DATA"]),
    "reclass_landuse_nodata": ("gp.Reclassify_sa", [RASTER / "landuse_u8.tif", "Value", "0 10;1 10;2 20;3 30"], ["NODATA"]),
    "reclass_dem_ranges": ("gp.Reclassify_sa", [RASTER / "dem_nodata.tif", "Value", "180 220 1;220 260 2;260 300 3;300 340 4"], ["DATA"]),
    "reclass_int16_ranges": ("gp.Reclassify_sa", [RASTER / "dem_int16_packbits.tif", "Value", "180 220 1;220 260 2;260 400 300"], ["NODATA"]),
    "reclass_dem_to_nodata": ("gp.Reclassify_sa", [RASTER / "dem.tif", "Value", "180 250 NODATA;250 340 5"], ["DATA"]),
    "reclass_landuse_4bit": ("gp.Reclassify_sa", [RASTER / "landuse_u8.tif", "Value", "0 1;1 2;2 3;3 4;4 4"], ["DATA"]),
    "reclass_landuse_200": ("gp.Reclassify_sa", [RASTER / "landuse_u8.tif", "Value", "0 200;1 200;2 200;3 200;4 200"], ["DATA"]),
    "reclass_dem_signed": ("gp.Reclassify_sa", [RASTER / "dem.tif", "Value", "180 250 -5;250 340 5"], ["DATA"]),
    "curvature_dem": ("gp.Curvature_sa", [RASTER / "dem.tif"], [1]),
    "curvature_dem_nodata": ("gp.Curvature_sa", [RASTER / "dem_nodata.tif"], [1]),
    "curvature_dem_z2": ("gp.Curvature_sa", [RASTER / "dem_int16_packbits.tif"], [2]),
    "plus_dem_int16": ("gp.Plus_sa", [RASTER / "dem.tif", RASTER / "dem_int16_packbits.tif"], []),
    "plus_int16_const": ("gp.Plus_sa", [RASTER / "dem_int16_packbits.tif", 5], []),
    "minus_nodata_dem": ("gp.Minus_sa", [RASTER / "dem_nodata.tif", RASTER / "dem.tif"], []),
    "times_int16_half": ("gp.Times_sa", [RASTER / "dem_int16_packbits.tif", 0.5], []),
    "times_zones_int16": ("gp.Times_sa", [RASTER / "zones.tif", RASTER / "dem_int16_packbits.tif"], []),
    "divide_int16_zones": ("gp.Divide_sa", [RASTER / "dem_int16_packbits.tif", RASTER / "zones.tif"], []),
    "divide_dem_3": ("gp.Divide_sa", [RASTER / "dem.tif", 3], []),
    "isnull_dem_nodata": ("gp.IsNull_sa", [RASTER / "dem_nodata.tif"], []),
    "int_dem": ("gp.Int_sa", [RASTER / "dem_nodata.tif"], []),
    "float_int16": ("gp.Float_sa", [RASTER / "dem_int16_packbits.tif"], []),
    "con_zones": ("gp.Con_sa", [RASTER / "zones.tif", RASTER / "dem.tif"], [RASTER / "dem_int16_packbits.tif", "Value > 3"]),
    "con_zones_const": ("gp.Con_sa", [RASTER / "zones.tif", 1], [0, "Value = 7"]),
    "con_no_false": ("gp.Con_sa", [RASTER / "dem_nodata.tif", RASTER / "dem_nodata.tif"], ["", "Value > 250"]),
    "setnull_zones": ("gp.SetNull_sa", [RASTER / "zones.tif", RASTER / "dem.tif"], ["Value = 7"]),
    "contour_dem_20": ("gp.Contour_sa", [RASTER / "dem.tif"], [20, 0]),
    "contour_dem_nodata_15": ("gp.Contour_sa", [RASTER / "dem_nodata.tif"], [15, 5]),
    "contour_int16_25": ("gp.Contour_sa", [RASTER / "dem_int16_packbits.tif"], [25, 0]),
    "streams_hydro": ("gp.Con_sa", [OUT / "flowacc_hydro.tif", 1], ["", "Value > 60"]),
    "basin_hydro": ("gp.Basin_sa", [OUT / "flowdir_hydro_filled.tif"], []),
    "basin_hydro_nodata": ("gp.Basin_sa", [OUT / "flowdir_hydro_nodata.tif"], []),
    "streamlink_hydro": ("gp.StreamLink_sa", [OUT / "streams_hydro.tif", OUT / "flowdir_hydro_filled.tif"], []),
    "streamorder_strahler": ("gp.StreamOrder_sa", [OUT / "streams_hydro.tif", OUT / "flowdir_hydro_filled.tif"], ["STRAHLER"]),
    "streamorder_shreve": ("gp.StreamOrder_sa", [OUT / "streams_hydro.tif", OUT / "flowdir_hydro_filled.tif"], ["SHREVE"]),
    "flowlength_down": ("gp.FlowLength_sa", [OUT / "flowdir_hydro_filled.tif"], ["DOWNSTREAM"]),
    "flowlength_up": ("gp.FlowLength_sa", [OUT / "flowdir_hydro_filled.tif"], ["UPSTREAM"]),
    "watershed_links": ("gp.Watershed_sa", [OUT / "flowdir_hydro_filled.tif", OUT / "streamlink_hydro.tif"], []),
    "r2pt_landuse": ("conversion.RasterToPoint", [RASTER / "landuse_u8.tif"], ["Value"]),
    "r2pt_dem_nodata": ("conversion.RasterToPoint", [RASTER / "dem_nodata.tif"], ["Value"]),
    "r2poly_zones": ("conversion.RasterToPolygon", [RASTER / "zones.tif"], ["NO_SIMPLIFY", "Value"]),
    "r2poly_landuse": ("conversion.RasterToPolygon", [RASTER / "landuse_u8.tif"], ["NO_SIMPLIFY", "Value"]),
    "zonal_mean": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_nodata.tif"], ["MEAN", "DATA"]),
    "zonal_max_strict": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_nodata.tif"], ["MAXIMUM", "NODATA"]),
    "zonal_range_int": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["RANGE", "DATA"]),
    "zonal_std": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem.tif"], ["STD", "DATA"]),
    "zonal_sum_int": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["SUM", "DATA"]),
    "zonal_median": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["MEDIAN", "DATA"]),
    "zonal_majority": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["MAJORITY", "DATA"]),
    "zonal_variety": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["VARIETY", "DATA"]),
    "zonal_min_f": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem.tif"], ["MINIMUM", "DATA"]),
}
# Spatial Analyst tools: output is a GeoTIFF, <name>.tif.
RASTER_OUT = {"gp.Slope_sa", "gp.Aspect_sa", "gp.HillShade_sa", "gp.FocalStatistics_sa", "gp.Fill_sa",
              "gp.FlowDirection_sa", "gp.FlowAccumulation_sa", "gp.Reclassify_sa", "gp.ZonalStatistics_sa",
              "gp.Curvature_sa", "gp.Plus_sa", "gp.Minus_sa", "gp.Times_sa", "gp.Divide_sa", "gp.IsNull_sa",
              "gp.Int_sa", "gp.Float_sa", "gp.Con_sa", "gp.SetNull_sa", "gp.Basin_sa", "gp.StreamLink_sa",
              "gp.StreamOrder_sa", "gp.FlowLength_sa", "gp.Watershed_sa"}
WORKSPACE_OUT = {"analysis.Split"}
# Tools that write a text file: compared byte for byte.
TEXT_OUT = {"conversion.FeaturesToJSON", "conversion.ExportTable"}
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
    w.call("python", code="arcpy.CheckOutExtension('Spatial')")
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
        if tool in RASTER_OUT:
            out = OUT / f"{name}.tif"
        if tool in TEXT_OUT:
            out = OUT / (f"{name}.geojson" if "GEOJSON" in after else f"{name}.json")
            if tool == "conversion.ExportTable":
                out = OUT / f"{name}.csv"
        if tool in WORKSPACE_OUT:
            out = OUT / name
            out.mkdir()
            args = [str(a) for a in before] + [str(out)] + [str(a) if isinstance(a, Path) else a for a in after]
        elif tool in IN_PLACE:
            k = IN_PLACE[tool]
            w.call("run_tool", tool="management.CopyFeatures", args=[str(before[k]), str(out)],
                   env={"overwriteOutput": True})
            args = [str(out) if i == k else str(a) for i, a in enumerate(before)] + after
        else:
            args = [str(a) for a in before] + [str(out)] + [str(a) if isinstance(a, Path) else a for a in after]
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
        d = w.call("describe", path=str(out)) if r.get("status") == "succeeded" and tool not in TEXT_OUT | RASTER_OUT else {}
        manifest["cases"][name] = {
            "tool": tool,
            "args": [rel(a) for a in before] + ["<out>"] + [rel(a) for a in after],
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
