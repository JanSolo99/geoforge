"""Regenerates testdata/arcgis-reference/ by running ArcGIS Pro tools through the
ArcGIS bridge (tools/arcgis-bridge). Needs ArcGIS Pro on this machine:

    uv run --with "mcp>=2" python scripts/make-arcgis-reference.py [case ...]

With case names, only those cases are rerun and the rest of the manifest is kept.

Each case writes a shapefile and records ArcGIS's messages and output schema in
manifest.json, which the Rust tests compare UniGIS against.
"""

import json
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools" / "arcgis-bridge"))
import server  # noqa: E402

DATA = ROOT / "testdata" / "geoproc"
RASTER = ROOT / "testdata" / "raster"
GPKG = ROOT / "testdata" / "gpkg" / "shapes.gpkg"
GPKG_BIG = ROOT / "testdata" / "gpkg" / "sample.gpkg"
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



def fm(*entries, linux=False):
    """A Spatial Join field mapping string from (name, type, length, rule, delimiter,
    [(table, field, start, end), ...]) entries (tables are given as paths)."""
    out = []
    for name, ftype, length, rule, delim, sources in entries:
        prec = 0 if ftype in ("Text", "Double", "Float") else length
        where = str if linux else win
        src = ",".join(f"{where(t)},{f},{a},{b}" for t, f, a, b in sources)
        out.append(f'{name} "{name}" true true false {length} {ftype} 0 {prec},{rule},{delim},{src}')
    return ";".join(out)

TARGET_FIELDS = {
    "clip_areas": [("AREA_ID", "Long", 10)],
    "parcels": [("PARCEL_ID", "Long", 10), ("NAME", "Text", 20), ("VALUE", "Double", 19), ("ZONE", "Long", 5)],
}

CLIP = DATA / "clip_areas.shp"
PLANAR = ROOT / "testdata" / "planar"
PARCELS = DATA / "parcels.shp"
DM = ROOT / "testdata" / "dm"

CASES = {
    # name: (tool, args before the output, args after the output)
    # Workspace creation, run in a copy of testdata/geoproc/workspace_create (<workspace>).
    **{f"wsc_{name}": (tool, [*args], [])
       for name, tool, args in [
        ("folder", "management.CreateFolder", ["<workspace>", "f1"]),
        ("folder_existing", "management.CreateFolder", ["<workspace>", "existing"]),
        ("folder_spaces", "management.CreateFolder", ["<workspace>", "two words"]),
        ("folder_missing", "management.CreateFolder", ["<workspace>/missing", "f"]),
        ("folder_bad_name", "management.CreateFolder", ["<workspace>", "a*b"]),
        ("folder_on_file", "management.CreateFolder", ["<workspace>", "taken.geodatabase"]),
        ("folder_empty_name", "management.CreateFolder", ["<workspace>", ""]),
        ("fgdb", "management.CreateFileGDB", ["<workspace>", "g1"]),
        ("fgdb_suffix", "management.CreateFileGDB", ["<workspace>", "g2.gdb"]),
        ("fgdb_extension", "management.CreateFileGDB", ["<workspace>", "g8.txt"]),
        ("fgdb_v10", "management.CreateFileGDB", ["<workspace>", "g3", "10.0"]),
        ("fgdb_current", "management.CreateFileGDB", ["<workspace>", "g4", "CURRENT"]),
        ("fgdb_bad_version", "management.CreateFileGDB", ["<workspace>", "g5", "11"]),
        ("fgdb_taken", "management.CreateFileGDB", ["<workspace>", "taken"]),
        ("fgdb_missing", "management.CreateFileGDB", ["<workspace>/missing", "g"]),
        ("fgdb_in_gdb", "management.CreateFileGDB", ["<workspace>/taken.gdb", "g"]),
        ("fgdb_bad_name", "management.CreateFileGDB", ["<workspace>", "a*b"]),
        ("mgdb", "management.CreateMobileGDB", ["<workspace>", "m1"]),
        ("mgdb_suffix", "management.CreateMobileGDB", ["<workspace>", "m2.geodatabase"]),
        ("mgdb_extension", "management.CreateMobileGDB", ["<workspace>", "m3.sqlite"]),
        ("mgdb_taken", "management.CreateMobileGDB", ["<workspace>", "taken"]),
        ("mgdb_missing", "management.CreateMobileGDB", ["<workspace>/missing", "m"]),
        ("mgdb_in_gdb", "management.CreateMobileGDB", ["<workspace>/taken.gdb", "m"]),
    ]},
    **{f"fctg_{name}": ("conversion.FeatureClassToGeodatabase", [[str(DATA / item) for item in items]], [])
       for name, items in [
        ("single", ["parcels.shp"]),
        ("batch", ["parcels.shp", "roads.shp", "wells.shp"]),
        ("repeat", ["parcels.shp", "parcels.shp"]),
        ("collision", ["parcels.shp"]),
        ("dataset", ["parcels.shp"]),
        ("gdb", ["globalids_base.gdb/points"]),
        ("empty", ["globalids_base.gdb/empty"]),
        ("properties", ["extract_base.gdb/sites"]),
        ("polygon_gdb", ["fctg_seed.gdb/parcels"]),
        ("name", ["fctg_names/9 bad-name.shp"]),
        ("table", ["globalids_base.gdb/records"]),
        ("bad_batch", ["parcels.shp", "missing.shp"]),
        ("missing_output", ["parcels.shp"]),
    ]},
    **{f"ttg_{name}": ("conversion.TableToGeodatabase", [[str(DATA / item) for item in items]], [])
       for name, items in [
        ("single", ["dates.dbf"]),
        ("batch", ["dates.dbf", "bfu_def_basic.csv", "split_attribute_table.dbf"]),
        ("repeat", ["dates.dbf", "dates.dbf"]),
        ("gdb", ["globalids_base.gdb/records"]),
        ("collision", ["parcels.shp", "dates.dbf"]),
        ("dataset", ["dates.dbf"]),
        ("bad_batch", ["dates.dbf", "missing.dbf"]),
        ("missing_output", ["dates.dbf"]),
    ]},
    # Extract Locations From Document: in_template, then the notation switches (DD lat/lon,
    # DD xy degrees, DD plain, DM, DM xy, DMS, DMS xy, DMS separated, UTM, UPS N, UPS S, MGRS).
    "elt_basic": ("conversion.ExtractLocationsText", ["Camp at 34.0522 N, 118.2437 W on 2024-03-15.\nThen 18SUJ2348306479 on March 21, 2024."], []),
    "elt_mixed": ("conversion.ExtractLocationsText", [(DATA / "eld_mixed.txt").read_text()], []),
    "elt_none": ("conversion.ExtractLocationsText", ["No coordinates here, only 2023-12-01 and 3.14159."], []),
    "elt_no_grids": ("conversion.ExtractLocationsText", [(DATA / "eld_mixed.txt").read_text()],
                     ["", "", "", "", "", "", "", "", "", "DONT_FIND_UTM_MAINWORLD", "", "", "DONT_FIND_MGRS_MAINWORLD"]),
    "eld_mixed": ("conversion.ExtractLocationsDocument", [DATA / "eld_mixed.txt"], []),
    "eld_plain": ("conversion.ExtractLocationsDocument", [DATA / "eld_plain.txt"], []),
    "eld_none": ("conversion.ExtractLocationsDocument", [DATA / "eld_none.txt"], []),
    "eld_long": ("conversion.ExtractLocationsDocument", [DATA / "eld_long.txt"], []),
    "eld_no_grids": ("conversion.ExtractLocationsDocument", [DATA / "eld_mixed.txt"],
                     ["", "", "", "", "", "", "", "", "", "DONT_FIND_UTM_MAINWORLD", "", "", "DONT_FIND_MGRS_MAINWORLD"]),
    "eld_no_dms": ("conversion.ExtractLocationsDocument", [DATA / "eld_mixed.txt"],
                   ["", "", "", "", "DONT_FIND_DM_LATLON", "", "DONT_FIND_DMS_LATLON"]),
    **{f"excel_{name}": ("conversion.ExcelToTable", [DATA / ("excel_input." + ext)], [sheet, header, cells])
       for name, ext, sheet, header, cells in [
        ("default", "xlsx", "", 1, ""),
        ("no_header", "xlsx", "Data", 0, "A1:C3"),
        ("range", "xlsx", "Data", 1, "B2:D3"),
        ("names", "xlsx", "Names", 1, ""),
        ("offset", "xlsx", "Offset", 3, ""),
        ("dates", "xlsx", "Dates", 1, ""),
        ("named", "xlsx", "Picked", 1, ""),
        ("xls", "xls", "", 1, ""),
        ("empty", "xlsx", "Empty", 1, ""),
        ("bad_sheet", "xlsx", "Missing", 1, ""),
        ("reverse", "xlsx", "Data", 1, "D3:B2"),
        ("beyond", "xlsx", "Data", 1, "A3:C7"),
        ("header_outside", "xlsx", "Data", 8, ""),
    ]},
    **{f"gids_{name}": ("management.AddGlobalIDs", [DATA / "globalids_base.gdb" / name], [])
       for name in ["points", "repeat", "records", "empty", "collision", "group", "missing"]},
    "gids_batch": ("management.AddGlobalIDs", [DATA / "globalids_base.gdb" / n for n in ["points", "records"]], []),
    **{f"subf_{name}": ("management.SubsetFeatures", [DATA / "subset_base.gdb" / item], [test, size, units])
       for name, item, test, size, units in [
        ("default", "points", True, "", ""),
        ("percent33", "points", True, "33.3", "PERCENTAGE_OF_INPUT"),
        ("abs3", "points", True, "3", "ABSOLUTE_VALUE"),
        ("abs36", "points", True, "3.6", "ABSOLUTE_VALUE"),
        ("abs19", "points", True, "1.9", "ABSOLUTE_VALUE"),
        ("tiny", "points", True, "1", "PERCENTAGE_OF_INPUT"),
        ("all", "points", True, "100", "PERCENTAGE_OF_INPUT"),
        ("too_many", "points", True, "99", "ABSOLUTE_VALUE"),
        ("train_only", "points", False, "3", "ABSOLUTE_VALUE"),
        ("records", "records", True, "50", "PERCENTAGE_OF_INPUT"),
        ("records_all", "records", True, "100", "PERCENTAGE_OF_INPUT"),
        ("records_train_only", "records", False, "3", "ABSOLUTE_VALUE"),
        ("zero", "points", True, "0", "ABSOLUTE_VALUE"),
        ("negative", "points", True, "-1", "ABSOLUTE_VALUE"),
        ("over_percent", "points", True, "101", "PERCENTAGE_OF_INPUT"),
        ("empty", "empty", True, "50", "PERCENTAGE_OF_INPUT"),
    ]},
    **{f"ssl_{name}": ("management.CreateSpatialSamplingLocations", [DATA / "ssl_areas.shp"],
        [method, field, allocation, shape, size, "7", count, per_stratum, population, geometry, gap, relation])
       for name, method, field, allocation, shape, size, count, per_stratum, population, geometry, gap, relation in [
        ("default", "", "", "", "", "", "", "", "", "", "", ""),
        ("random", "RANDOM", "", "EQUAL", "HEXAGON", "", "7", "", "", "POINT", "0", "HAVE_THEIR_CENTER_IN"),
        ("poly_equal", "STRAT_POLY", "", "EQUAL", "", "", "", "3", "", "", "0", ""),
        ("poly_area", "STRAT_POLY", "", "PROP_AREA", "", "", "7", "", "", "", "0", ""),
        ("poly_field", "STRAT_POLY", "", "FIELD", "", "", "", "", "POP", "", "0", ""),
        ("poly_pop", "STRAT_POLY", "", "PROP_FIELD", "", "", "9", "", "POP", "", "0", ""),
        ("id_equal", "STRAT_ID", "STRATUM", "EQUAL", "", "", "", "2", "", "", "0", ""),
        ("id_area", "STRAT_ID", "STRATUM", "PROP_AREA", "", "", "7", "", "", "", "0", ""),
        ("id_field", "STRAT_ID", "STRATUM", "FIELD", "", "", "", "", "POP", "", "0", ""),
        ("id_pop", "STRAT_ID", "STRATUM", "PROP_FIELD", "", "", "9", "", "POP", "", "0", ""),
        ("spaced", "RANDOM", "", "", "", "", "6", "", "", "", "3 Meters", ""),
        ("impossible", "RANDOM", "", "", "", "", "4", "", "", "", "100 Meters", ""),
        ("zero", "RANDOM", "", "", "", "", "0", "", "", "", "", ""),
        ("negative", "RANDOM", "", "", "", "", "-1", "", "", "", "", ""),
        ("id_missing", "STRAT_ID", "", "EQUAL", "", "", "", "2", "", "", "", ""),
        *[(f"system_{shape.lower()}", "SYSTEMATIC", "", "", shape, "100 SquareMeters", "", "", "", "POINT", "", "HAVE_THEIR_CENTER_IN") for shape in ["SQUARE", "DIAMOND", "HEXAGON", "TRANSVERSE_HEXAGON", "TRIANGLE"]],
        ("system_polygon", "SYSTEMATIC", "", "", "SQUARE", "100 SquareMeters", "", "", "", "POLYGON", "", "HAVE_THEIR_CENTER_IN"),
        ("system_intersect", "SYSTEMATIC", "", "", "SQUARE", "100 SquareMeters", "", "", "", "POLYGON", "", "INTERSECT"),
        ("system_within", "SYSTEMATIC", "", "", "SQUARE", "100 SquareMeters", "", "", "", "POLYGON", "", "COMPLETELY_WITHIN"),
    ]},
    **{f"rnp_{name}": ("management.CreateRandomPoints", [], [fc, extent, n, gap, multi, size])
       for name, fc, extent, n, gap, multi, size in [
        ("extent", "", "0 0 20 10", "7", "0", "POINT", "10"),
        ("zero", "", "0 0 20 10", "0", "0", "POINT", "10"),
        ("negative", "", "0 0 20 10", "-2", "0", "POINT", "10"),
        ("areas", DATA / "rnp_areas.shp", "", "2", "0", "POINT", "10"),
        ("field", DATA / "rnp_areas.shp", "", "N", "GAP", "POINT", "10"),
        ("lines", DATA / "rnp_lines.shp", "", "3", "2 Meters", "POINT", "10"),
        ("line_field", DATA / "rnp_lines.shp", "", "N", "0", "POINT", "10"),
        ("points", DATA / "rnp_points.shp", "", "3", "0", "POINT", "10"),
        ("points_over", DATA / "rnp_points.shp", "", "10", "0", "POINT", "10"),
        ("multi_extent", "", "0 0 20 10", "7", "0", "MULTIPOINT", "3"),
        ("multi_areas", DATA / "rnp_areas.shp", "", "4", "0", "MULTIPOINT", "3"),
        ("impossible", DATA / "rnp_areas.shp", "", "3", "100 Meters", "POINT", "10"),
        ("ignored_extent", DATA / "rnp_areas.shp", "0 0 1 1", "1", "0", "POINT", "10"),
        ("extent_source", "", DATA / "rnp_areas.shp", "4", "0", "POINT", "10"),
        ("default_extent", "", "", "3", "0", "POINT", "10"),
        ("negative_gap", DATA / "rnp_areas.shp", "", "3", "-1 Meters", "POINT", "10"),
    ]},
    # Extraction cases save a complete output FileGDB, including schema and relationships.
    **{f"edg_{name}": ("management.ExtractDataFromGeodatabase",
        [";".join("'" + str(DATA / "extract_base.gdb" / item) + "'" for item in items)],
        [extract, out_type, expand, related, all_rows])
       for name, items, extract, out_type, expand, related, all_rows in [
        ("plain", ["plain"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("table_default", ["unrelated"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("table_all", ["unrelated"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "ALL_RECORDS_FOR_TABLES"),
        ("related", ["sites"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("reverse", ["observations"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("schema", ["sites"], "SCHEMA_ONLY", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "ALL_RECORDS_FOR_TABLES"),
        ("no_expand", ["sites"], "DATA", "NEW_FILE_GEODATABASE", "DO_NOT_ADD", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("no_related", ["sites"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "DO_NOT_GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("schema_expanded", ["sites"], "DATA", "NEW_FILE_GEODATABASE", "ADD_WITH_SCHEMA_ONLY", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("all_expanded", ["sites"], "DATA", "NEW_FILE_GEODATABASE", "ALL_ROWS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("both", ["sites", "observations"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "ALL_RECORDS_FOR_TABLES"),
        ("both_no_expand", ["sites", "observations"], "DATA", "NEW_FILE_GEODATABASE", "DO_NOT_ADD", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("duplicate", ["plain", "plain"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("existing", ["sites"], "DATA", "GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("missing", ["absent"], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
        ("empty", [], "DATA", "NEW_FILE_GEODATABASE", "USE_DEFAULTS", "GET_RELATED", "SCHEMA_ONLY_FOR_TABLES"),
    ]},
    "mdhp_single": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "single"], ['D1']),
    "mdhp_multiple": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "multiple"], ['D1;D2']),
    "mdhp_class_dates": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "class_dates"], ['D1;D2']),
    "mdhp_empty": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "empty"], ['D1']),
    "mdhp_already": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "already"], ['D1']),
    "mdhp_wrong": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "wrong"], ['KEY']),
    "mdhp_missing": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "missing"], ['absent']),
    "mdhp_mixed": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "mixed"], ['D1;KEY']),
    "mdhp_duplicate": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "duplicate"], ['D1;D1']),
    "mdhp_case_names": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "case_names"], ['d1;d2']),
    "mdhp_no_fields": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_base.gdb" / "no_fields"], ['']),
    "mdhp_shapefile": ("management.MigrateDateFieldToHighPrecision", [DATA / "migrate_date_shape.shp"], ["D1"]),
    "etrack_add_all": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "add_all"], ['maker', 'made', 'changer', 'changed', 'ADD_FIELDS']),
    "etrack_table_add": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "table_add"], ['maker', 'made', 'changer', 'changed', 'ADD_FIELDS']),
    "etrack_creator_only": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "creator_only"], ['maker', '', '', '', 'ADD_FIELDS']),
    "etrack_dates_only": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "dates_only"], ['', 'made', '', 'changed', 'ADD_FIELDS']),
    "etrack_local_time": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "local_time"], ['maker', 'made', 'changer', 'changed', 'ADD_FIELDS', 'DATABASE_TIME']),
    "etrack_existing": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "existing"], ['maker', 'made', 'changer', 'changed']),
    "etrack_missing": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "missing"], ['maker', 'made', 'changer', 'changed']),
    "etrack_wrong": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "wrong"], ['maker', '', '', '', 'ADD_FIELDS']),
    "etrack_no_roles": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "no_roles"], []),
    "etrack_active_same": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "active_same"], ['maker', 'made', '', '']),
    "etrack_active_extra": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "active_extra"], ['', '', 'changer', 'changed']),
    "etrack_active_switch": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "active_switch"], ['other', 'made', '', '', 'ADD_FIELDS']),
    "etrack_active_zone": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "active_zone"], ['maker', 'made', '', '', 'NO_ADD_FIELDS', 'DATABASE_TIME']),
    "etrack_shared": ("management.EnableEditorTracking", [DATA / "enable_tracking_base.gdb" / "shared"], ['maker', 'made', 'maker', 'made']),
    **{f"ecogo_{name}": ("management.EnableCOGO", [DATA / "enable_cogo_base.gdb" / name], [])
       for name in ["plain", "empty", "partial", "existing", "wrong_string", "wrong_integer", "point", "polygon", "table_only", "enabled"]},
    "ecogo_shapefile": ("management.EnableCOGO", [DATA / "roads.shp"], []),
    **{f"dtrack_{name}": ("management.DisableEditorTracking", [DATA / "editor_tracking_base.gdb" / name], [])
       for name in ["all_off", "table_all", "plain", "local_time"]},
    **{f"dtrack_{name}": ("management.DisableEditorTracking", [DATA / "editor_tracking_base.gdb" / name], flags)
       for name, flags in {
           "creator_off": ["DISABLE_CREATOR", "NO_DISABLE_CREATION_DATE", "NO_DISABLE_LAST_EDITOR", "NO_DISABLE_LAST_EDIT_DATE"],
           "created_off": ["NO_DISABLE_CREATOR", "DISABLE_CREATION_DATE", "NO_DISABLE_LAST_EDITOR", "NO_DISABLE_LAST_EDIT_DATE"],
           "editor_off": ["NO_DISABLE_CREATOR", "NO_DISABLE_CREATION_DATE", "DISABLE_LAST_EDITOR", "NO_DISABLE_LAST_EDIT_DATE"],
           "edited_off": ["NO_DISABLE_CREATOR", "NO_DISABLE_CREATION_DATE", "NO_DISABLE_LAST_EDITOR", "DISABLE_LAST_EDIT_DATE"],
           "keep_all": ["NO_DISABLE_CREATOR", "NO_DISABLE_CREATION_DATE", "NO_DISABLE_LAST_EDITOR", "NO_DISABLE_LAST_EDIT_DATE"],
           "keep_dates": ["DISABLE_CREATOR", "NO_DISABLE_CREATION_DATE", "DISABLE_LAST_EDITOR", "NO_DISABLE_LAST_EDIT_DATE"],
       }.items()},
    "dtrack_custom": ("management.DisableEditorTracking", [DATA / "editor_tracking_extra.gdb" / "custom"], []),
    "dtrack_shapefile": ("management.DisableEditorTracking", [DATA / "roads.shp"], []),
    "dtrack_bad_flag": ("management.DisableEditorTracking", [DATA / "editor_tracking_base.gdb" / "all_off"], ["garbage"]),
    **{f"dcogo_{name}": ("management.DisableCOGO", [DATA / "cogo_base.gdb" / name], [])
       for name in ["enabled", "empty_enabled", "plain", "custom", "point", "polygon", "table_only"]},
    "dcogo_shapefile": ("management.DisableCOGO", [DATA / "roads.shp"], []),
    'dml_shape_table': ("management.DeleteMultiple", ["'<workspace>/points.shp' #;'<workspace>/table.dbf' #"], []),
    'dml_gdb_items': ("management.DeleteMultiple", ["'<workspace>/data.gdb/points' FeatureClass;'<workspace>/data.gdb/table' #"], []),
    'dml_gdb_workspace': ("management.DeleteMultiple", ["'<workspace>/data.gdb' #"], []),
    'dml_duplicate': ("management.DeleteMultiple", ["'<workspace>/points.shp';'<workspace>/points.shp'"], []),
    'dml_dbf_of_shape': ("management.DeleteMultiple", ["'<workspace>/points.dbf'"], []),
    'dml_typed': ("management.DeleteMultiple", ["'<workspace>/points.shp' FeatureClass"], []),
    'dml_type_hint': ("management.DeleteMultiple", ["'<workspace>/points.shp' RelationshipClass"], []),
    'dml_quoted': ("management.DeleteMultiple", ["'<workspace>/points spaced.shp' #;'<workspace>/table.dbf' #"], []),
    "dml_raster": ("management.DeleteMultiple", ["'<workspace>/grid.tif' #"], []),
    'dml_plain_file': ("management.DeleteMultiple", ["'<workspace>/keep.txt' #"], []),
    'dml_err_missing': ("management.DeleteMultiple", ["'<workspace>/absent.shp'"], []),
    'dml_err_mixed_missing': ("management.DeleteMultiple", ["'<workspace>/points.shp';'<workspace>/absent.shp';'<workspace>/table.dbf'"], []),
    'dml_err_empty': ("management.DeleteMultiple", [''], []),

    # Schema edits retain their geodatabase-only fields/domains in manifest snapshots.
    **{f"gps_{name}{suffix}": ("management.AddGPSMetadataFields", [DATA / "gps_metadata_base.gdb" / name], options)
       for name in ["point", "point_zm", "line", "polygon", "multipoint", "existing_receiver"]
       for suffix, options in [("", []), ("_offset", ["INCLUDE_OFFSET_FIELDS"])]},
    "gps_err_table": ("management.AddGPSMetadataFields", [DATA / "gps_metadata_base.gdb" / "table_only"], []),
    "gps_err_shapefile": ("management.AddGPSMetadataFields", [DATA / "wells.shp"], []),
    "clip_parcels": ("analysis.Clip", [DATA / "parcels.shp", DATA / "clip_areas.shp"], []),
    "clip_roads": ("analysis.Clip", [DATA / "roads.shp", DATA / "clip_areas.shp"], []),
    "clip_wells": ("analysis.Clip", [DATA / "wells.shp", DATA / "clip_areas.shp"], []),
    "copy_gpkg_points": ("management.CopyFeatures", [GPKG / "main.points"], []),
    "copy_gpkg_points_zm": ("management.CopyFeatures", [GPKG / "main.points_zm"], []),
    "copy_gpkg_multipoints": ("management.CopyFeatures", [GPKG / "main.multipoints"], []),
    "copy_gpkg_multilines": ("management.CopyFeatures", [GPKG / "main.multilines"], []),
    "copy_gpkg_polygons": ("management.CopyFeatures", [GPKG / "main.polygons"], []),
    "copy_gpkg_multipolygons": ("management.CopyFeatures", [GPKG / "main.multipolygons"], []),
    "copy_gpkg_polygons_z": ("management.CopyFeatures", [GPKG / "main.polygons_z"], []),
    "copy_gpkg_geographic": ("management.CopyFeatures", [GPKG / "main.geographic"], []),
    "buffer_gpkg_lines_10": ("analysis.Buffer", [GPKG / "main.lines"], ["10 Meters"]),
    "clip_gpkg_multipolygons": ("analysis.Clip", [GPKG / "main.multipolygons", DATA / "clip_areas.shp"], []),
    "select_gpkg_polygons": ("analysis.Select", [GPKG / "main.polygons"], ["long_f > 150000"]),
    "dissolve_gpkg_multipolygons": ("management.Dissolve", [GPKG / "main.multipolygons"], []),
    "copy_gpkg_date_only": ("management.CopyFeatures", [GPKG_BIG / "main.lines"], []),
    "copy_gpkg_big_integer": ("management.CopyFeatures", [GPKG_BIG / "main.points"], []),
    "buffer_gpkg_big_integer": ("analysis.Buffer", [GPKG_BIG / "main.lines"], ["10 Meters"]),
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
    "update_parcels_clip": ("analysis.Update", [PARCELS, CLIP], []),
    "update_clip_parcels": ("analysis.Update", [CLIP, PARCELS], []),
    "sort_parcels_name": ("management.Sort", [DATA / "parcels.shp"], ["NAME ASCENDING", "UR"]),
    "sort_parcels_zone_value": ("management.Sort", [DATA / "parcels.shp"], ["ZONE DESCENDING;VALUE ASCENDING", "UR"]),
    "sort_parcels_value_desc": ("management.Sort", [DATA / "parcels.shp"], ["VALUE DESCENDING", "UR"]),
    "sort_wells_depth": ("management.Sort", [DATA / "wells.shp"], ["DEPTH ASCENDING", "UR"]),
    "sort_wells_ur": ("management.Sort", [DATA / "wells.shp"], ["Shape ASCENDING", "UR"]),
    "sort_wells_ul": ("management.Sort", [DATA / "wells.shp"], ["Shape ASCENDING", "UL"]),
    "sort_wells_lr": ("management.Sort", [DATA / "wells.shp"], ["Shape ASCENDING", "LR"]),
    "sort_wells_ll": ("management.Sort", [DATA / "wells.shp"], ["Shape ASCENDING", "LL"]),
    "sort_roads_ur": ("management.Sort", [DATA / "roads.shp"], ["Shape ASCENDING", "UR"]),
    "sort_roads_ll": ("management.Sort", [DATA / "roads.shp"], ["Shape ASCENDING", "LL"]),
    "sort_parcels_zone_shape": ("management.Sort", [DATA / "parcels.shp"], ["ZONE ASCENDING;Shape ASCENDING", "UL"]),
    "sort_clip_ul": ("management.Sort", [DATA / "clip_areas.shp"], ["Shape ASCENDING", "UL"]),
    "deli_parcels_shape": ("management.DeleteIdentical", [DATA / "parcels_dups.shp"], ["Shape"]),
    "deli_parcels_zone": ("management.DeleteIdentical", [DATA / "parcels_dups.shp"], ["ZONE"]),
    "deli_parcels_zone_value": ("management.DeleteIdentical", [DATA / "parcels_dups.shp"], ["ZONE;VALUE"]),
    "deli_parcels_shape_zone": ("management.DeleteIdentical", [DATA / "parcels_dups.shp"], ["Shape;ZONE"]),
    "deli_wells_shape": ("management.DeleteIdentical", [DATA / "wells_dups.shp"], ["Shape"]),
    "addfield_long": ("management.AddField", [DATA / "wells.shp"], ["f_long", "LONG"]),
    "addfield_short": ("management.AddField", [DATA / "wells.shp"], ["f_short", "SHORT"]),
    "addfield_float": ("management.AddField", [DATA / "wells.shp"], ["f_float", "FLOAT"]),
    "addfield_double": ("management.AddField", [DATA / "wells.shp"], ["f_double", "DOUBLE"]),
    "addfield_text15": ("management.AddField", [DATA / "wells.shp"], ["f_text", "TEXT", "", "", 15]),
    "addfield_text_default": ("management.AddField", [DATA / "wells.shp"], ["f_text", "TEXT"]),
    "addfield_date": ("management.AddField", [DATA / "wells.shp"], ["f_date", "DATE"]),
    "addfield_long5": ("management.AddField", [DATA / "wells.shp"], ["f_long5", "LONG", 5]),
    "addfield_dbl_10_3": ("management.AddField", [DATA / "wells.shp"], ["f_d", "DOUBLE", 10, 3]),
    "addfield_flt_8_2": ("management.AddField", [DATA / "wells.shp"], ["f_f", "FLOAT", 8, 2]),
    "addfield_badname": ("management.AddField", [DATA / "wells.shp"], ["bad name", "LONG"]),
    "addfield_dup": ("management.AddField", [DATA / "wells.shp"], ["DEPTH", "DOUBLE"]),
    "delfield_one": ("management.DeleteField", [DATA / "parcels.shp"], ["NAME"]),
    "delfield_two": ("management.DeleteField", [DATA / "parcels.shp"], ["ZONE;VALUE"]),
    "delfield_keep": ("management.DeleteField", [DATA / "parcels.shp"], ["NAME;ZONE", "KEEP_FIELDS"]),
    "cf_value_x2": ("management.CalculateField", [PARCELS], ["calc", "!VALUE! * 2", "PYTHON3", "", "DOUBLE"]),
    "cf_div_long": ("management.CalculateField", [PARCELS], ["calc", "!PARCEL_ID! / 2", "PYTHON3", "", "LONG"]),
    "cf_floordiv": ("management.CalculateField", [PARCELS], ["calc", "!PARCEL_ID! // 2", "PYTHON3", "", "LONG"]),
    "cf_val_long": ("management.CalculateField", [PARCELS], ["calc", "!VALUE! * 2", "PYTHON3", "", "LONG"]),
    "cf_ratio": ("management.CalculateField", [PARCELS], ["calc", "!PARCEL_ID! / 3", "PYTHON3", "", "DOUBLE"]),
    "cf_upper": ("management.CalculateField", [PARCELS], ["calc", "!NAME!.upper()", "PYTHON3", "", "TEXT"]),
    "cf_slice": ("management.CalculateField", [PARCELS], ["calc", "!NAME![4:]", "PYTHON3", "", "TEXT"]),
    "cf_concat": ("management.CalculateField", [PARCELS], ["calc", "'X' + str(!ZONE!)", "PYTHON3", "", "TEXT"]),
    "cf_round": ("management.CalculateField", [PARCELS], ["calc", "round(!VALUE!, 1)", "PYTHON3", "", "DOUBLE"]),
    "cf_sqrt": ("management.CalculateField", [PARCELS], ["calc", "math.sqrt(!VALUE!)", "PYTHON3", "", "DOUBLE"]),
    "cf_ifexp": ("management.CalculateField", [PARCELS], ["calc", "!VALUE! if !ZONE! > 1 else -1", "PYTHON3", "", "DOUBLE"]),
    "cf_len": ("management.CalculateField", [PARCELS], ["calc", "len(!NAME!)", "PYTHON3", "", "LONG"]),
    "cf_mod": ("management.CalculateField", [PARCELS], ["calc", "!PARCEL_ID! % 3", "PYTHON3", "", "LONG"]),
    "cf_pow": ("management.CalculateField", [PARCELS], ["calc", "!PARCEL_ID! ** 2", "PYTHON3", "", "LONG"]),
    "cf_fid": ("management.CalculateField", [PARCELS], ["calc", "!FID!", "PYTHON3", "", "LONG"]),
    "cf_idtext": ("management.CalculateField", [PARCELS], ["calc", "!PARCEL_ID!", "PYTHON3", "", "TEXT"]),
    "cf_valtext": ("management.CalculateField", [PARCELS], ["calc", "!VALUE!", "PYTHON3", "", "TEXT"]),
    "cf_replace": ("management.CalculateField", [PARCELS], ["calc", "!NAME!.replace('Lot','L')", "PYTHON3", "", "TEXT"]),
    "cf_short": ("management.CalculateField", [PARCELS], ["calc", "!VALUE!", "PYTHON3", "", "SHORT"]),
    "cf_codeblock": ("management.CalculateField", [PARCELS], ["calc", "f(!ZONE!)", "PYTHON3", "def f(z):\n    if z > 2:\n        return 10\n    return z * 5", "LONG"]),
    "cf_percent": ("management.CalculateField", [PARCELS], ["calc", "'%s-%d' % (!NAME!, !ZONE!)", "PYTHON3", "", "TEXT"]),
    "cf_fstring": ("management.CalculateField", [PARCELS], ["calc", "f'{!NAME!}_{!ZONE!}'", "PYTHON3", "", "TEXT"]),
    "cf_zone_plus": ("management.CalculateField", [PARCELS], ["calc", "!ZONE! + 0.6", "PYTHON3", "", "LONG"]),
    "cf_int": ("management.CalculateField", [PARCELS], ["calc", "int(!VALUE!)", "PYTHON3", "", "DOUBLE"]),
    "cf_strip": ("management.CalculateField", [PARCELS], ["calc", "!NAME!.strip().lower()", "PYTHON3", "", "TEXT"]),
    "cf_max": ("management.CalculateField", [PARCELS], ["calc", "max(!ZONE!, 2)", "PYTHON3", "", "LONG"]),
    "cf_date": ("management.CalculateField", [PARCELS], ["calc", "'2020-05-17'", "PYTHON3", "", "DATE"]),
    "cf_existing": ("management.CalculateField", [PARCELS], ["ZONE", "!ZONE! * 10", "PYTHON3", "", "LONG"]),
    "symdiff_parcels_clip": ("analysis.SymDiff", [PARCELS, CLIP], []),
    "symdiff_clip_parcels": ("analysis.SymDiff", [CLIP, PARCELS], []),
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
    "asj_parcels_wells": ("management.AddSpatialJoin", [DATA / "parcels.shp", DATA / "wells.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL", None, "INTERSECT", None, None, "PERMANENT_FIELDS"]),
    "asj_parcels_wells_dist": ("management.AddSpatialJoin", [DATA / "parcels.shp", DATA / "wells.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL", None, "WITHIN_A_DISTANCE", "50 Meters", "DIST", "PERMANENT_FIELDS"]),
    "asj_wells_parcels_closest": ("management.AddSpatialJoin", [DATA / "wells.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL", None, "CLOSEST", None, "NEAR_D", "PERMANENT_FIELDS"]),
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
    "append_fm_a": ("management.Append", [CLIP, PARCELS], ["NO_TEST", fm(("PARCEL_ID", "Long", 10, "First", "#", [(CLIP, "AREA_ID", -1, -1)]),
        ("NAME", "Text", 20, "First", "#", [(PARCELS, "NAME", -1, -1)]), ("VALUE", "Double", 19, "First", "#", [(CLIP, "AREA_ID", -1, -1)]),
        ("ZONE", "Long", 5, "First", "#", [(CLIP, "AREA_ID", -1, -1)]), linux=True)]),
    "append_fm_text": ("management.Append", [CLIP, PARCELS], ["NO_TEST", fm(("PARCEL_ID", "Long", 10, "First", "#", [(PARCELS, "PARCEL_ID", -1, -1), (CLIP, "AREA_ID", -1, -1)]),
        ("NAME", "Text", 20, "First", "#", [(CLIP, "AREA_ID", -1, -1)]), ("VALUE", "Double", 19, "First", "#", [(PARCELS, "VALUE", -1, -1)]),
        ("ZONE", "Long", 5, "First", "#", [(PARCELS, "ZONE", -1, -1)]), linux=True)]),
    "fc2fc_where": ("conversion.FeatureClassToFeatureClass", [PARCELS], ["ZONE = 2"]),
    "fc2fc_map": ("conversion.FeatureClassToFeatureClass", [PARCELS], ["VALUE > 5000", fm(("PID", "Long", 10, "First", "#", [(PARCELS, "PARCEL_ID", -1, -1)]),
        ("LBL", "Text", 4, "First", "#", [(PARCELS, "NAME", 0, 3)]), ("V", "Long", 10, "First", "#", [(PARCELS, "VALUE", -1, -1)]), linux=True)]),
    "fc2fc_order": ("conversion.FeatureClassToFeatureClass", [PARCELS], ["", fm(("ZONE", "Short", 5, "First", "#", [(PARCELS, "ZONE", -1, -1)]),
        ("NAME", "Text", 20, "First", "#", [(PARCELS, "NAME", -1, -1)]), linux=True)]),
    "append_parcels_to_clip": ("management.Append", [DATA / "parcels.shp", DATA / "clip_areas.shp"], ["NO_TEST"]),
    "append_parcels_to_parcels": ("management.Append", [DATA / "parcels.shp", DATA / "parcels.shp"], ["TEST"]),
    "near_wells_roads_clip": ("analysis.Near", [DATA / "wells.shp", f"{DATA / 'roads.shp'};{DATA / 'clip_areas.shp'}"], ["", "LOCATION", "ANGLE"]),
    # Split writes one shapefile per split value into a folder (WORKSPACE_OUT).
    "sba_text": ("analysis.SplitByAttributes", [DATA / "split_attributes.shp"], ['REGION']),
    "sba_integer": ("analysis.SplitByAttributes", [DATA / "split_attributes.shp"], ['CLASS']),
    "sba_multi": ("analysis.SplitByAttributes", [DATA / "split_attributes.shp"], ['REGION;CLASS']),
    "sba_reversed_fields": ("analysis.SplitByAttributes", [DATA / "split_attributes.shp"], ['CLASS;REGION']),
    "sba_names": ("analysis.SplitByAttributes", [DATA / "split_attribute_names.shp"], ['REGION']),
    "sba_table_text": ("analysis.SplitByAttributes", [DATA / "split_attribute_table.dbf"], ['REGION']),
    "sba_table_multi": ("analysis.SplitByAttributes", [DATA / "split_attribute_table.dbf"], ['REGION;CLASS']),
    "sba_polygons": ("analysis.SplitByAttributes", [DATA / "pairwise_neighbors.shp"], ['GRP']),
    "sba_err_oid": ("analysis.SplitByAttributes", [DATA / "split_attributes.shp"], ['FID']),
    "sba_err_field": ("analysis.SplitByAttributes", [DATA / "split_attributes.shp"], ['NO_SUCH']),
    "sba_err_no_fields": ("analysis.SplitByAttributes", [DATA / "split_attributes.shp"], []),
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
    "merge_fm_id": ("management.Merge", [f"{PARCELS};{CLIP}"], [fm(("ID", "Long", 10, "First", "#", [(DATA / "clip_areas.shp", "AREA_ID", -1, -1), (DATA / "parcels.shp", "PARCEL_ID", -1, -1)]), linux=True), "NO_SOURCE_INFO"]),
    "merge_fm_types": ("management.Merge", [f"{CLIP};{PARCELS}"], [fm(("ID", "Double", 19, "First", "#", [(DATA / "clip_areas.shp", "AREA_ID", -1, -1), (DATA / "parcels.shp", "PARCEL_ID", -1, -1)]),
        ("LBL", "Text", 4, "First", "#", [(DATA / "parcels.shp", "NAME", 0, 3)]), ("V", "Long", 10, "First", "#", [(DATA / "parcels.shp", "VALUE", -1, -1)]),
        ("Z", "Short", 5, "First", "#", [(DATA / "parcels.shp", "ZONE", -1, -1)]), linux=True), "NO_SOURCE_INFO"]),
    "merge_fm_same_name": ("management.Merge", [f"{CLIP};{PARCELS}"], [fm(("NAME", "Text", 30, "First", "#", [(DATA / "parcels.shp", "NAME", -1, -1), (DATA / "clip_areas.shp", "AREA_ID", -1, -1)]), linux=True), "NO_SOURCE_INFO"]),
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
    "sj_fm_rename": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL",
        fm(("AREA_ID", "Long", 10, "First", "#", [(DATA / "clip_areas.shp", "AREA_ID", -1, -1)]),
           ("PID", "Long", 10, "First", "#", [(DATA / "parcels.shp", "PARCEL_ID", -1, -1)])), "INTERSECT"]),
    "sj_fm_types": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL",
        fm(("AREA_ID", "Long", 10, "First", "#", [(DATA / "clip_areas.shp", "AREA_ID", -1, -1)]),
           ("VAL_I", "Long", 10, "First", "#", [(DATA / "parcels.shp", "VALUE", -1, -1)]),
           ("ZN", "Double", 19, "First", "#", [(DATA / "parcels.shp", "ZONE", -1, -1)]),
           ("NM", "Text", 5, "First", "#", [(DATA / "parcels.shp", "NAME", 0, 4)]),
           ("SUB", "Text", 20, "First", "#", [(DATA / "parcels.shp", "NAME", 2, 5)]),
           ("PTXT", "Text", 12, "First", "#", [(DATA / "parcels.shp", "PARCEL_ID", -1, -1)])), "INTERSECT"]),
    "sj_fm_sources": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_ONE", "KEEP_ALL",
        fm(("BOTH", "Long", 10, "First", "#", [(DATA / "clip_areas.shp", "AREA_ID", -1, -1), (DATA / "parcels.shp", "PARCEL_ID", -1, -1)]),
           ("REV", "Long", 10, "First", "#", [(DATA / "parcels.shp", "PARCEL_ID", -1, -1), (DATA / "clip_areas.shp", "AREA_ID", -1, -1)]),
           ("TOT", "Double", 19, "Sum", "#", [(DATA / "parcels.shp", "VALUE", -1, -1)]),
           ("CNT", "Long", 10, "Count", "#", [(DATA / "parcels.shp", "VALUE", -1, -1)]),
           ("FL", "Float", 12, "First", "#", [(DATA / "parcels.shp", "VALUE", -1, -1)]),
           ("SH", "Short", 5, "First", "#", [(DATA / "parcels.shp", "ZONE", -1, -1)])), "INTERSECT"]),
    "sj_fm_many": ("analysis.SpatialJoin", [DATA / "clip_areas.shp", DATA / "parcels.shp"], ["JOIN_ONE_TO_MANY", "KEEP_COMMON",
        fm(("AREA_ID", "Long", 10, "First", "#", [(DATA / "clip_areas.shp", "AREA_ID", -1, -1)]),
           ("PID", "Long", 10, "First", "#", [(DATA / "parcels.shp", "PARCEL_ID", -1, -1)]),
           ("NM", "Text", 8, "First", "#", [(DATA / "parcels.shp", "NAME", -1, -1)])), "INTERSECT"]),
    "project_wells_4326": ("management.Project", [DATA / "wells.shp"], [4326]),
    "project_parcels_3857": ("management.Project", [DATA / "parcels.shp"], [3857]),
    "project_roads_32634": ("management.Project", [DATA / "roads.shp"], [32634]),
    "project_wells_wgs84_32633": ("management.Project", [DATA / "wells_wgs84.shp"], [32633]),
    "project_parcels_z_4326": ("management.Project", [DATA / "parcels_z.shp"], [4326]),
    "project_ed50_4326_default": ("management.Project", [DATA / "wells_ed50.shp"], [4326]),
    "project_ed50_4326_t1": ("management.Project", [DATA / "wells_ed50.shp"], [4326, "ED_1950_To_WGS_1984_1"]),
    "project_ed50_4326_t18": ("management.Project", [DATA / "wells_ed50.shp"], [4326, "ED_1950_To_WGS_1984_18"]),
    "project_ed50_4326_t23": ("management.Project", [DATA / "wells_ed50.shp"], [4326, "ED_1950_To_WGS_1984_23"]),
    "project_ed50_4326_chain": ("management.Project", [DATA / "wells_ed50.shp"], [4326, "ED_1950_To_ETRS_1989_1 + ETRS_1989_To_WGS_1984"]),
    "project_ed50_utm33_32633_t1": ("management.Project", [DATA / "wells_ed50_utm33.shp"], [32633, "ED_1950_To_WGS_1984_1"]),
    "project_ed50_utm33_4326_t7": ("management.Project", [DATA / "wells_ed50_utm33.shp"], [4326, "ED_1950_To_WGS_1984_7"]),
    "project_wgs84_ed50_t1": ("management.Project", [DATA / "wells_wgs84.shp"], [4230, "ED_1950_To_WGS_1984_1"]),
    "project_nad27_nadcon": ("management.Project", [DATA / "wells_us_nad27.shp"], [4269, "NAD_1927_To_NAD_1983_NADCON"]),
    "project_nad27_nadcon_wgs84": ("management.Project", [DATA / "wells_us_nad27.shp"], [4326, "NAD_1927_To_NAD_1983_NADCON + NAD_1983_To_WGS_1984_1"]),
    "project_wgs84_ed50_utm33_t18": ("management.Project", [DATA / "wells_wgs84.shp"], [23033, "ED_1950_To_WGS_1984_18"]),
    "m2s_roads": ("management.MultipartToSinglepart", [DATA / "roads.shp"], []),
    "m2s_parcels_clip": ("management.MultipartToSinglepart", [OUT / "clip_parcels.shp"], []),
    "f2p_parcels_centroid": ("management.FeatureToPoint", [DATA / "parcels.shp"], ["CENTROID"]),
    "f2p_parcels_inside": ("management.FeatureToPoint", [DATA / "parcels.shp"], ["INSIDE"]),
    "f2p_label_polys_inside": ("management.FeatureToPoint", [DATA / "label_polys.shp"], ["INSIDE"]),
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
    "ccn_grid_input_gars_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "DD_NUMERIC", "", 4326]),
    "ccn_grid_input_georef_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "DD_NUMERIC", "", 4326]),
    "ccn_grid_input_gars_georef": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "GEOREF", "", 4326]),
    "ccn_grid_input_georef_georef": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "GEOREF", "", 4326]),
    "ccn_grid_input_gars_dd_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "DD_1", "", 4326]),
    "ccn_grid_input_georef_dd_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "DD_1", "", 4326]),
    "ccn_grid_input_gars_ddm_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "DDM_1", "", 4326]),
    "ccn_grid_input_gars_ddm_2": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "DDM_2", "", 4326]),
    "ccn_grid_input_gars_dms_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "DMS_1", "", 4326]),
    "ccn_grid_input_gars_dms_2": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "DMS_2", "", 4326]),
    "ccn_grid_input_georef_ddm_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "DDM_1", "", 4326]),
    "ccn_grid_input_georef_ddm_2": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "DDM_2", "", 4326]),
    "ccn_grid_input_georef_dms_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "DMS_1", "", 4326]),
    "ccn_grid_input_georef_dms_2": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "DMS_2", "", 4326]),
    "ccn_grid_input_precision_ddm_1": ("management.ConvertCoordinateNotation", [DATA / "grid_carry_precision.dbf"], ["COORD", None, "GEOREF", "DDM_1", "", 4326]),
    "ccn_grid_input_precision_ddm_2": ("management.ConvertCoordinateNotation", [DATA / "grid_carry_precision.dbf"], ["COORD", None, "GEOREF", "DDM_2", "", 4326]),
    "ccn_grid_input_precision_dms_1": ("management.ConvertCoordinateNotation", [DATA / "grid_carry_precision.dbf"], ["COORD", None, "GEOREF", "DMS_1", "", 4326]),
    "ccn_grid_input_precision_dms_2": ("management.ConvertCoordinateNotation", [DATA / "grid_carry_precision.dbf"], ["COORD", None, "GEOREF", "DMS_2", "", 4326]),
    "ccn_grid_input_gars_gars": ("management.ConvertCoordinateNotation", [DATA / "grid_input_gars.csv"], ["COORD", None, "GARS", "GARS", "", 4326]),
    "ccn_grid_input_mgrs_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "grid_input_mgrs.dbf"], ["COORD", None, "MGRS", "DD_NUMERIC", "", 4326]),
    "ccn_grid_input_mgrs_dd_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_mgrs.dbf"], ["COORD", None, "MGRS", "DD_1", "", 4326]),
    "ccn_grid_input_mgrs_dd_2": ("management.ConvertCoordinateNotation", [DATA / "grid_input_mgrs.dbf"], ["COORD", None, "MGRS", "DD_2", "", 4326]),
    "ccn_grid_input_usng_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "grid_input_usng.dbf"], ["COORD", None, "USNG", "DD_NUMERIC", "", 4326]),
    "ccn_grid_input_usng_dd_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_usng.dbf"], ["COORD", None, "USNG", "DD_1", "", 4326]),
    "ccn_grid_input_utm_bands_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "grid_input_utm_bands.dbf"], ["COORD", None, "UTM_BANDS", "DD_NUMERIC", "", 4326]),
    "ccn_grid_input_utm_bands_dd_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_utm_bands.dbf"], ["COORD", None, "UTM_BANDS", "DD_1", "", 4326]),
    "ccn_grid_input_utm_zones_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "grid_input_utm_zones.dbf"], ["COORD", None, "UTM_ZONES", "DD_NUMERIC", "", 4326]),
    "ccn_grid_input_utm_zones_dd_1": ("management.ConvertCoordinateNotation", [DATA / "grid_input_utm_zones.dbf"], ["COORD", None, "UTM_ZONES", "DD_1", "", 4326]),
    "ccn_grid_input_boundary_gars_gars": ("management.ConvertCoordinateNotation", [DATA / "gars_boundary_gars.dbf"], ["COORD", None, "GARS", "GARS", "", 4326]),
    "ccn_grid_input_georef_gars": ("management.ConvertCoordinateNotation", [DATA / "grid_input_georef.csv"], ["COORD", None, "GEOREF", "GARS", "", 4326]),
    "ccn_grid_input_boundary_georef_gars": ("management.ConvertCoordinateNotation", [DATA / "gars_boundary_georef.dbf"], ["COORD", None, "GEOREF", "GARS", "", 4326]),
    "ccn_input_dd_1_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "angular_dd_1.csv"], ["COORDX", None, "DD_1", "DD_NUMERIC", "", 4326]),
    "ccn_input_dd_1_dms_1": ("management.ConvertCoordinateNotation", [DATA / "angular_dd_1.csv"], ["COORDX", None, "DD_1", "DMS_1", "", 4326]),
    "ccn_input_dd_2_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "angular_dd_2.csv"], ["COORDX", "COORDY", "DD_2", "DD_NUMERIC", "", 4326]),
    "ccn_input_dd_2_dms_1": ("management.ConvertCoordinateNotation", [DATA / "angular_dd_2.csv"], ["COORDX", "COORDY", "DD_2", "DMS_1", "", 4326]),
    "ccn_input_ddm_1_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "angular_ddm_1.csv"], ["COORDX", None, "DDM_1", "DD_NUMERIC", "", 4326]),
    "ccn_input_ddm_1_dms_1": ("management.ConvertCoordinateNotation", [DATA / "angular_ddm_1.csv"], ["COORDX", None, "DDM_1", "DMS_1", "", 4326]),
    "ccn_input_ddm_2_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "angular_ddm_2.csv"], ["COORDX", "COORDY", "DDM_2", "DD_NUMERIC", "", 4326]),
    "ccn_input_ddm_2_dms_1": ("management.ConvertCoordinateNotation", [DATA / "angular_ddm_2.csv"], ["COORDX", "COORDY", "DDM_2", "DMS_1", "", 4326]),
    "ccn_input_dms_1_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "angular_dms_1.csv"], ["COORDX", None, "DMS_1", "DD_NUMERIC", "", 4326]),
    "ccn_input_dms_1_dms_1": ("management.ConvertCoordinateNotation", [DATA / "angular_dms_1.csv"], ["COORDX", None, "DMS_1", "DMS_1", "", 4326]),
    "ccn_input_dms_2_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "angular_dms_2.csv"], ["COORDX", "COORDY", "DMS_2", "DD_NUMERIC", "", 4326]),
    "ccn_input_dms_2_dms_1": ("management.ConvertCoordinateNotation", [DATA / "angular_dms_2.csv"], ["COORDX", "COORDY", "DMS_2", "DMS_1", "", 4326]),
    "ccn_dd_1": ("management.ConvertCoordinateNotation", [DATA / "lonlat.csv"], ["LON", "LAT", "DD_2", "DD_1", "", 4326]),
    "ccn_dd_2": ("management.ConvertCoordinateNotation", [DATA / "lonlat.csv"], ["LON", "LAT", "DD_2", "DD_2", "", 4326]),
    "ccn_ddm_1": ("management.ConvertCoordinateNotation", [DATA / "lonlat.csv"], ["LON", "LAT", "DD_2", "DDM_1", "", 4326]),
    "ccn_ddm_2": ("management.ConvertCoordinateNotation", [DATA / "lonlat.csv"], ["LON", "LAT", "DD_2", "DDM_2", "", 4326]),
    "ccn_dms_1": ("management.ConvertCoordinateNotation", [DATA / "lonlat.csv"], ["LON", "LAT", "DD_2", "DMS_1", "", 4326]),
    "ccn_dms_2": ("management.ConvertCoordinateNotation", [DATA / "lonlat.csv"], ["LON", "LAT", "DD_2", "DMS_2", "", 4326]),
    "ccn_dd_numeric": ("management.ConvertCoordinateNotation", [DATA / "lonlat.csv"], ["LON", "LAT", "DD_2", "DD_NUMERIC", "", 4326]),
    "ccn_utm_bands": ("management.ConvertCoordinateNotation", [DATA / "lonlat_grid.csv"], ["LON", "LAT", "DD_2", "UTM_BANDS", "", 4326]),
    "ccn_utm_zones": ("management.ConvertCoordinateNotation", [DATA / "lonlat_grid.csv"], ["LON", "LAT", "DD_2", "UTM_ZONES", "", 4326]),
    "ccn_mgrs": ("management.ConvertCoordinateNotation", [DATA / "lonlat_grid.csv"], ["LON", "LAT", "DD_2", "MGRS", "", 4326]),
    "ccn_usng": ("management.ConvertCoordinateNotation", [DATA / "lonlat_grid.csv"], ["LON", "LAT", "DD_2", "USNG", "", 4326]),
    "ccn_gars": ("management.ConvertCoordinateNotation", [DATA / "lonlat_grid.csv"], ["LON", "LAT", "DD_2", "GARS", "", 4326]),
    "ccn_georef": ("management.ConvertCoordinateNotation", [DATA / "lonlat_grid.csv"], ["LON", "LAT", "DD_2", "GEOREF", "", 4326]),
    "ellipse_poly": ("management.TableToEllipse", [DATA / "ellipses.csv"], ["X", "Y", "MAJ", "MIN", "Meters", "AZ", "Decimal degrees", "", 32633, "NO_ATTRIBUTES", "Polygon", "Planar"]),
    "ellipse_line": ("management.TableToEllipse", [DATA / "ellipses.csv"], ["X", "Y", "MAJ", "MIN", "Meters", "AZ", "Decimal degrees", "", 32633, "NO_ATTRIBUTES", "Line", "Planar"]),
    "ellipse_mils": ("management.TableToEllipse", [DATA / "ellipses.csv"], ["X", "Y", "MAJ", "MIN", "Meters", "AZ", "Mils", "", 32633, "NO_ATTRIBUTES", "Polygon", "Planar"]),
    "ellipse_feet_rad": ("management.TableToEllipse", [DATA / "ellipses.csv"], ["X", "Y", "MAJ", "MIN", "Feet", "AZ", "Radians", "", 32633, "NO_ATTRIBUTES", "Polygon", "Planar"]),
    "ellipse_noaz": ("management.TableToEllipse", [DATA / "ellipses.csv"], ["X", "Y", "MAJ", "MIN", "Meters", "", "Decimal degrees", "", 32633, "NO_ATTRIBUTES", "Polygon", "Planar"]),
    "gdd_lines": ("management.GeodeticDensify", [DATA / "geodens_lines.shp"], ["Geodesic", "500000"]),
    "gdd_lines_km": ("management.GeodeticDensify", [DATA / "geodens_lines.shp"], ["Geodesic", "500 Kilometers"]),
    "gdd_lines_small": ("management.GeodeticDensify", [DATA / "geodens_lines.shp"], ["Geodesic", "80000"]),
    "gdd_poly": ("management.GeodeticDensify", [DATA / "geodens_poly.shp"], ["Geodesic", "1000000"]),
    "gdd_utm": ("management.GeodeticDensify", [DATA / "geodens_utm.shp"], ["Geodesic", "100000"]),
    "gdd_utm_mi": ("management.GeodeticDensify", [DATA / "geodens_utm.shp"], ["Geodesic", "20 Miles"]),
    "enc_name_hot": ("management.EncodeField", [DATA / "streams.shp"], ["NAME", "One-hot"]),
    "enc_name_cold": ("management.EncodeField", [DATA / "streams.shp"], ["NAME", "One-cold"]),
        "enc_zone_hot": ("management.EncodeField", [DATA / "parcels.shp"], ["ZONE", "One-hot"]),
    "enc_zone_cold": ("management.EncodeField", [DATA / "parcels.shp"], ["ZONE", "One-cold"]),
    "odl_default": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_origins.shp", DATA / "od_destinations.shp"], []),
    "odl_groups": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_origins.shp", DATA / "od_destinations.shp"], ["GRP", "GRP"]),
    "odl_nearest": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_origins.shp", DATA / "od_destinations.shp"], ["", "", "PLANAR", 1]),
    "odl_search": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_origins.shp", DATA / "od_destinations.shp"], ["", "", "PLANAR", "", 100]),
    "odl_km": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_origins.shp", DATA / "od_destinations.shp"], ["", "", "PLANAR", 2, 0.15, "KILOMETERS"]),
    "odl_same": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_origins.shp", DATA / "od_origins.shp"], []),
    "odl_reordered_nearest": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_more_origins.shp", DATA / "od_more_destinations.shp"], ["", "", "PLANAR", 1]),
    "odl_reordered_groups": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_more_origins.shp", DATA / "od_more_destinations.shp"], ["GRP", "GRP"]),
    "odl_numeric_groups": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_more_origins.shp", DATA / "od_more_destinations.shp"], ["NUM", "NUM"]),
    "odl_group_nearest": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_more_origins.shp", DATA / "od_more_destinations.shp"], ["GRP", "GRP", "PLANAR", 1]),
    "odl_polygon": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_polygon.shp", DATA / "od_more_destinations.shp"], []),
    "odl_line": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_line.shp", DATA / "od_more_destinations.shp"], []),
    "odl_zero_limits": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_origins.shp", DATA / "od_destinations.shp"], ["", "", "PLANAR", 0, 0]),
    "odl_empty": ("analysis.GenerateOriginDestinationLinks", [DATA / "od_polygon.shp", DATA / "od_destinations.shp"], ["", "", "PLANAR", "", 0.001]),
    "awp_default": ("analysis.AssignWeightsByPairwiseComparison", ["A;B;C"], []),
    "awp_three": ("analysis.AssignWeightsByPairwiseComparison", ["A;B;C"], ["ADD_MATRIX", "Variables A B 3; A C 5; B C 2"]),
    "awp_three_weights_only": ("analysis.AssignWeightsByPairwiseComparison", ["A;B;C"], ["WEIGHTS_ONLY", "Variables A B 3; A C 5; B C 2"]),
    "awp_four": ("analysis.AssignWeightsByPairwiseComparison", ["A;B;C;D"], ["ADD_MATRIX", "Variables A B 2; A C 1; A D 3; B C 4; B D 0.5; C D 4"]),
    "awp_fraction": ("analysis.AssignWeightsByPairwiseComparison", ["elev;landuse;dist_road"], ["ADD_MATRIX", "Variables elev landuse 1/3; elev dist_road 2.2; landuse dist_road 0.333"]),
    "awp_nine": ("analysis.AssignWeightsByPairwiseComparison", ["Slope;Landuse;Roads;Water;Soil;Aspect;Noise;Rail;Parks"], ["ADD_MATRIX", "Variables Slope Landuse 3; Slope Roads 1/5; Slope Water 9; Slope Soil 1/9; Slope Aspect 2; Slope Noise 0.5; Slope Rail 4; Slope Parks 7; Landuse Roads 1/3; Landuse Water 6; Landuse Soil 1; Landuse Aspect 8; Landuse Noise 1/7; Landuse Rail 5; Landuse Parks 0.25; Roads Water 2.2; Roads Soil 3; Roads Aspect 1/5; Roads Noise 9; Roads Rail 1/9; Roads Parks 2; Water Soil 0.5; Water Aspect 4; Water Noise 7; Water Rail 1/3; Water Parks 6; Soil Aspect 1; Soil Noise 8; Soil Rail 1/7; Soil Parks 5; Aspect Noise 0.25; Aspect Rail 2.2; Aspect Parks 3; Noise Rail 1/5; Noise Parks 9; Rail Parks 1/9"]),
    "awp_err_count": ("analysis.AssignWeightsByPairwiseComparison", ["A"], []),
    "awp_err_name": ("analysis.AssignWeightsByPairwiseComparison", ["1A;B;C"], []),
    "awp_err_order": ("analysis.AssignWeightsByPairwiseComparison", ["A;B;C"], ["ADD_MATRIX", "Variables A C 5; A B 3; B C 2"]),
    "awp_err_value": ("analysis.AssignWeightsByPairwiseComparison", ["A;B;C"], ["ADD_MATRIX", "Variables A B 9.5; A C 1; B C 1"]),
    "bfu_parcels_basic": ("management.BatchUpdateFields", [DATA / "parcels.shp"], [DATA / "bfu_def_basic.csv", None, "NAME", "SOURCE", "TYPE", "LEN", "ALIAS", "SCRIPT"]),
    "bfu_parcels_types": ("management.BatchUpdateFields", [DATA / "parcels.shp"], [DATA / "bfu_def_types.csv", None, "NAME", "SOURCE", "TYPE", "LEN", "ALIAS", "SCRIPT"]),
    "bfu_dates": ("management.BatchUpdateFields", [DATA / "dates.dbf"], [DATA / "bfu_def_dates.csv", None, "NAME", "SOURCE", "TYPE", "LEN", "ALIAS", "SCRIPT"]),
    "bfu_err_cycle": ("management.BatchUpdateFields", [DATA / "parcels.shp"], [DATA / "bfu_def_cycle.csv", None, "NAME", "SOURCE", "TYPE", "LEN", "ALIAS", "SCRIPT"]),
    "bfu_err_missing": ("management.BatchUpdateFields", [DATA / "parcels.shp"], [DATA / "bfu_def_missing.csv", None, "NAME", "SOURCE", "TYPE", "LEN", "ALIAS", "SCRIPT"]),
    "bfu_err_badtype": ("management.BatchUpdateFields", [DATA / "parcels.shp"], [DATA / "bfu_def_badtype.csv", None, "NAME", "SOURCE", "TYPE", "LEN", "ALIAS", "SCRIPT"]),
    "ctf_date_iso": ("management.ConvertTimeField", [DATA / "dates.dbf"], ["D", None, "OUT", "TEXT", "yyyy-MM-dd"]),
    "ctf_date_us": ("management.ConvertTimeField", [DATA / "dates.dbf"], ["D", None, "OUT", "TEXT", "MM/dd/yyyy"]),
    "ctf_date_default_text": ("management.ConvertTimeField", [DATA / "dates.dbf"], ["D", None, "OUT", "TEXT"]),
    "ctf_date_names": ("management.ConvertTimeField", [DATA / "dates.dbf"], ["E", None, "OUT", "TEXT", "dddd, dd MMMM yyyy"]),
    "ctf_text_compact": ("management.ConvertTimeField", [DATA / "ctf_text.dbf"], ["S1", "yyyyMMdd", "OUT", "DATE"]),
    "ctf_text_us": ("management.ConvertTimeField", [DATA / "ctf_text.dbf"], ["S2", "MM/dd/yyyy", "OUT", "DATE"]),
    "ctf_text_time": ("management.ConvertTimeField", [DATA / "ctf_text.dbf"], ["S3", "yyyy-MM-dd HH:mm:ss", "OUT", "DATE"]),
    "ctf_text_month": ("management.ConvertTimeField", [DATA / "ctf_text.dbf"], ["S4", "d MMMM yyyy", "OUT", "DATE"]),
    "ctf_long_date": ("management.ConvertTimeField", [DATA / "ctf_text.dbf"], ["N1", "yyyyMMdd", "OUT", "DATE"]),
    "ctf_text_to_text": ("management.ConvertTimeField", [DATA / "ctf_text.dbf"], ["S1", "yyyyMMdd", "OUT", "TEXT", "dd.MM.yyyy"]),
    "ctf_err_none": ("management.ConvertTimeField", [DATA / "ctf_text.dbf"], ["S3", None, "OUT", "DATE"]),
    "ctz_utc_pacific": ("management.ConvertTimeZone", [DATA / "dates.dbf"], ["D", "UTC", "OUT", "Pacific Standard Time", "INPUT_ADJUSTED_FOR_DST", "OUTPUT_ADJUSTED_FOR_DST"]),
    "ctz_utc_pacific_std": ("management.ConvertTimeZone", [DATA / "dates.dbf"], ["D", "UTC", "OUT", "Pacific Standard Time", "INPUT_ADJUSTED_FOR_DST", "OUTPUT_NOT_ADJUSTED_FOR_DST"]),
    "ctz_tonga_utc": ("management.ConvertTimeZone", [DATA / "dates.dbf"], ["E", "Tonga Standard Time", "OUT", "UTC", "INPUT_ADJUSTED_FOR_DST", "OUTPUT_ADJUSTED_FOR_DST"]),
    "ctz_utc_tokyo": ("management.ConvertTimeZone", [DATA / "dates.dbf"], ["E", "UTC", "OUT", "Tokyo Standard Time", "INPUT_ADJUSTED_FOR_DST", "OUTPUT_ADJUSTED_FOR_DST"]),
    "vj_parcels_dates": ("management.ValidateJoin", [DATA / "parcels.shp", "ZONE", DATA / "dates.dbf", "GRP"], []),
    "vj_dates_parcels": ("management.ValidateJoin", [DATA / "dates.dbf", "GRP", DATA / "parcels.shp", "ZONE"], []),
    "vj_parcels_dates_few": ("management.ValidateJoin", [DATA / "parcels.shp", "PARCEL_ID", DATA / "dates.dbf", "GRP"], []),
    "vj_parcels_dates_none": ("management.ValidateJoin", [DATA / "parcels.shp", "VALUE", DATA / "dates.dbf", "ID"], []),
    "vj_err_self": ("management.ValidateJoin", [DATA / "parcels.shp", "ZONE", DATA / "parcels.shp", "ZONE"], []),
    "a3z_reverse": ("management.Adjust3DZ", [ROOT / "testdata" / "shp" / "pointz.shp"], ["REVERSE"]),
    "a3z_add": ("management.Adjust3DZ", [ROOT / "testdata" / "shp" / "pointz.shp"], ["NO_REVERSE", 10]),
    "a3z_field": ("management.Adjust3DZ", [ROOT / "testdata" / "shp" / "pointz.shp"], ["NO_REVERSE", "count"]),
    "a3z_m_ft": ("management.Adjust3DZ", [ROOT / "testdata" / "shp" / "polylinez.shp"], ["NO_REVERSE", None, "METERS", "FEET"]),
    "a3z_all": ("management.Adjust3DZ", [ROOT / "testdata" / "shp" / "polygonz.shp"], ["REVERSE", 2.5, "FEET", "METERS"]),
    "a3z_multipoint": ("management.Adjust3DZ", [ROOT / "testdata" / "shp" / "multipointz.shp"], ["REVERSE", -1, "INCHES", "CENTIMETERS"]),
    "cga_parcels": ("management.CalculateGeometryAttributes", [DATA / "cga_parcels.shp"], ["AREA_D AREA;PERIM_D PERIMETER_LENGTH;CX CENTROID_X;CY CENTROID_Y;IX INSIDE_X;IY INSIDE_Y;XMIN EXTENT_MIN_X;YMAX EXTENT_MAX_Y;PARTS PART_COUNT;PNTS POINT_COUNT;HOLES HOLE_COUNT;AREA_L AREA"]),
    "cga_parcels_units": ("management.CalculateGeometryAttributes", [DATA / "cga_parcels.shp"], ["AREA_D AREA;PERIM_D PERIMETER_LENGTH", "KILOMETERS", "HECTARES"]),
    "cga_roads": ("management.CalculateGeometryAttributes", [DATA / "cga_roads.shp"], ["LEN_D LENGTH;SX LINE_START_X;SY LINE_START_Y;EX LINE_END_X;EY LINE_END_Y;BEAR LINE_BEARING;PNTS POINT_COUNT"]),
    "cga_roads_feet": ("management.CalculateGeometryAttributes", [DATA / "cga_roads.shp"], ["LEN_D LENGTH", "FEET_US"]),
    "cga_wells": ("management.CalculateGeometryAttributes", [DATA / "cga_wells.shp"], ["PX POINT_X;PY POINT_Y"]),
    "fc2shp_batch": ("conversion.FeatureClassToShapefile", [[DATA / "extract_base.gdb" / "sites", DATA / "globalids_base.gdb" / "points", DATA / "parcels.shp", DATA / "globalids_base.gdb" / "empty"]], []),
    "fc2shp_repeat": ("conversion.FeatureClassToShapefile", [[DATA / "parcels.shp", DATA / "parcels.shp", DATA / "roads.shp"]], []),
    "fc2shp_err_missing": ("conversion.FeatureClassToShapefile", [[DATA / "parcels.shp", DATA / "missing.shp"]], []),
    "tt2xl_dbf": ("conversion.TableToExcel", [[DM / "sales.dbf"]], []),
    "tt2xl_multi": ("conversion.TableToExcel", [[DATA / "extract_base.gdb" / "sites", DATA / "globalids_base.gdb" / "records", DATA / "stations.csv"]], []),
    "tt2xl_alias_domain": ("conversion.TableToExcel", [[DATA / "extract_base.gdb" / "sites"]], [True, True]),
    "tt2xl_repeat": ("conversion.TableToExcel", [[DM / "sales.dbf", DM / "sales.dbf"]], []),
    "tt2xl_dates": ("conversion.TableToExcel", [[DATA / "migrate_date_base.gdb" / "class_dates"]], []),
    "tt2xl_err_missing": ("conversion.TableToExcel", [[DM / "sales.dbf", DATA / "missing.dbf"]], []),
    "tt2db_batch": ("conversion.TableToDBASE", [[DM / "sales.dbf", DATA / "globalids_base.gdb" / "records", DATA / "parcels.shp"]], []),
    "tt2db_repeat": ("conversion.TableToDBASE", [[DM / "sales.dbf", DM / "sales.dbf", DATA / "globalids_base.gdb" / "records"]], []),
    "tt2db_collision": ("conversion.TableToDBASE", [[DATA / "parcels.shp", DATA / "parcels.dbf"]], []),
    "tt2db_csv": ("conversion.TableToDBASE", [[DATA / "stations.csv"]], []),
    "tt2db_err_missing": ("conversion.TableToDBASE", [[DM / "sales.dbf", DATA / "missing.dbf"]], []),
    "gpx_points": ("conversion.GPXtoFeatures", [DATA / "gpx_sample.gpx"], ["POINTS"]),
    "gpx_lines": ("conversion.GPXtoFeatures", [DATA / "gpx_sample.gpx"], ["TRACKS_AS_LINES"]),
    "gpx_sticky_points": ("conversion.GPXtoFeatures", [DATA / "gpx_sticky.gpx"], ["POINTS"]),
    "gpx_sticky_lines": ("conversion.GPXtoFeatures", [DATA / "gpx_sticky.gpx"], ["TRACKS_AS_LINES"]),
    "mpr_default": ("conversion.MultipatchToRaster", [ROOT / "testdata" / "shp" / "multipatch.shp"], []),
    "mpr_max1": ("conversion.MultipatchToRaster", [ROOT / "testdata" / "shp" / "multipatch.shp"], [1, "MAXIMUM_HEIGHT"]),
    "mpr_min1": ("conversion.MultipatchToRaster", [ROOT / "testdata" / "shp" / "multipatch.shp"], [1, "MINIMUM_HEIGHT"]),
    "rtof_default": ("conversion.RasterToOtherFormat", [[RASTER / "landuse_u8.tif", RASTER / "dem.tif"]], []),
    "rtof_tiff": ("conversion.RasterToOtherFormat", [RASTER / "dem_f64.tif"], ["TIFF"]),
    "fst_parcels_num": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["VALUE;ZONE;PARCEL_ID", "NUMERIC"]),
    "fst_parcels_text": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["NAME", "TEXT"]),
    "fst_streams_text": ("management.FieldStatisticsToTable", [DATA / "streams.shp"], ["NAME", "TEXT"]),
    "fst_wells_num": ("management.FieldStatisticsToTable", [DATA / "wells.shp"], ["DEPTH", "NUMERIC"]),
    "fst_dates_date": ("management.FieldStatisticsToTable", [DATA / "dates.dbf"], ["D;E;ID", "DATE"]),
    "fst_dates_date_by_grp": ("management.FieldStatisticsToTable", [DATA / "dates.dbf"], ["D", "DATE", "GRP"]),
    "fst_dates_date_stats": ("management.FieldStatisticsToTable", [DATA / "dates.dbf"], ["D;E", "DATE", "", "MODE Mo;RANGE;MEAN Avg;FIELDNAME"]),
    "fst_err_date_numeric_stat": ("management.FieldStatisticsToTable", [DATA / "dates.dbf"], ["D", "DATE", "", "SUM S"]),
    "fst_parcels_num_by_zone": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["VALUE;PARCEL_ID;ZONE", "NUMERIC", "ZONE"]),
    "fst_parcels_text_by_zone": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["NAME", "TEXT", "ZONE"]),
    "fst_parcels_num_by_name": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["VALUE", "NUMERIC", "NAME"]),
    "fst_parcels_num_stats": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["VALUE;PARCEL_ID", "NUMERIC", "", "MEDIAN Med;SUM Total;FIELDNAME;COUNT"]),
    "fst_parcels_num_stats_by_zone": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["VALUE", "NUMERIC", "ZONE", "KURTOSIS;MEDIAN #;SUM 'My Sum';BOGUS B"]),
    "fst_parcels_text_stats_by_zone": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["NAME", "TEXT", "ZONE", "MODE Most;FIELDTYPE"]),
    "fst_err_duplicate_stat": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["VALUE", "NUMERIC", "", "MEDIAN A;MEDIAN B"]),
    "fst_err_text_numeric_stat": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["NAME", "TEXT", "", "MEAN M"]),
    "fst_err_double_group": ("management.FieldStatisticsToTable", [DATA / "parcels.shp"], ["ZONE", "NUMERIC", "VALUE"]),
    "xy_stations": ("management.XYTableToPoint", [DATA / "stations.csv"], ["X", "Y", "", 32633]),
    "xy_stations_z": ("management.XYTableToPoint", [DATA / "stations.csv"], ["X", "Y", "ELEV", 32633]),
    # Rescale by Function: every function at its defaults, then parameters and thresholds.
    "rbf_mssmall": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["MSSMALL # # # # # #", 1, 10]),
    "rbf_mslarge": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["MSLARGE # # # # # #", 1, 10]),
    "rbf_linear": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LINEAR # # # # # #", 1, 10]),
    "rbf_exponential": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["EXPONENTIAL # # # # # #", 1, 10]),
    "rbf_gaussian": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["GAUSSIAN # # # # # #", 1, 10]),
    "rbf_large": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LARGE # # # # # #", 1, 10]),
    "rbf_logarithm": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LOGARITHM # # # # # #", 1, 10]),
    "rbf_logisticdecay": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LOGISTICDECAY # # # # # # #", 1, 10]),
    "rbf_logisticgrowth": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LOGISTICGROWTH # # # # # # #", 1, 10]),
    "rbf_near": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["NEAR # # # # # #", 1, 10]),
    "rbf_power": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["POWER # # # # # #", 1, 10]),
    "rbf_small": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["SMALL # # # # # #", 1, 10]),
    "rbf_symmetriclinear": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["SYMMETRICLINEAR # # # # # #", 1, 10]),
    "rbf_linear_params": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LINEAR 220 280 # # # #", 1, 10]),
    "rbf_gaussian_params": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["GAUSSIAN 250 0.001 # # # #", 0, 100]),
    "rbf_mssmall_mult": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["MSSMALL 1.2 0.5 # # # #", 1, 10]),
    "rbf_linear_thresholds": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LINEAR # # 220 0 280 NoData", 1, 10]),
    "rbf_exponential_thresholds": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["EXPONENTIAL # # 220 # 280 #", 1, 10]),
    "rbf_large_params": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LARGE 250 3 # # # #", 1, 10]),
    "rbf_logistic_y": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LOGISTICGROWTH # # 5 # # # #", 1, 10]),
    "rbf_power_zero": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["POWER # # # # # #", 0, 5]),
    "rbf_reversed": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["LINEAR # # # # # #", 10, 1]),
    "rbf_near_thresholds_outside": ("gp.RescaleByFunction_sa", [RASTER / "dem_nodata.tif"], ["NEAR # # 150 # 400 #", 1, 10]),
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
    "focal_majority_classes": ("gp.FocalStatistics_sa", [RASTER / "classes_u8.tif"], ["Rectangle 3 3 CELL", "MAJORITY", "DATA"]),
    "focal_minority_classes": ("gp.FocalStatistics_sa", [RASTER / "classes_u8.tif"], ["Rectangle 3 3 CELL", "MINORITY", "DATA"]),
    "focal_majority_classes_c2": ("gp.FocalStatistics_sa", [RASTER / "classes_u8.tif"], ["Circle 2 CELL", "MAJORITY", "DATA"]),
    "focal_majority_classes_nodata": ("gp.FocalStatistics_sa", [RASTER / "classes_u8.tif"], ["Rectangle 5 5 CELL", "MAJORITY", "NODATA"]),
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
    "plus_int16_const": ("gp.Plus_sa", [RASTER / "dem_int16_packbits.tif", "5"], []),
    "plus_zones_zones": ("gp.Plus_sa", [RASTER / "zones.tif", RASTER / "zones.tif"], []),
    "minus_zones_zones": ("gp.Minus_sa", [RASTER / "zones.tif", RASTER / "zones.tif"], []),
    "times_zones_zones": ("gp.Times_sa", [RASTER / "zones.tif", RASTER / "zones.tif"], []),
    "divide_zones_zones": ("gp.Divide_sa", [RASTER / "zones.tif", RASTER / "zones.tif"], []),
    "minus_zones_300": ("gp.Minus_sa", [RASTER / "zones.tif", "300"], []),
    "times_zones_1000": ("gp.Times_sa", [RASTER / "zones.tif", "1000"], []),
    "plus_zones_neg5": ("gp.Plus_sa", [RASTER / "zones.tif", "-5"], []),
    "divide_zones_300": ("gp.Divide_sa", [RASTER / "zones.tif", "300"], []),
    "divide_zones_neg2": ("gp.Divide_sa", [RASTER / "zones.tif", "-2"], []),
    "plus_int16_int16": ("gp.Plus_sa", [RASTER / "dem_int16_packbits.tif", RASTER / "dem_int16_packbits.tif"], []),
    "divide_int16_int16": ("gp.Divide_sa", [RASTER / "dem_int16_packbits.tif", RASTER / "dem_int16_packbits.tif"], []),
    "con_zones_zones_int16": ("gp.Con_sa", [RASTER / "zones.tif", RASTER / "zones.tif"], [RASTER / "dem_int16_packbits.tif"]),
    "con_zones_int16_only": ("gp.Con_sa", [RASTER / "zones.tif", RASTER / "dem_int16_packbits.tif"], []),
    "con_zones_300_zones": ("gp.Con_sa", [RASTER / "zones.tif", "300"], [RASTER / "zones.tif"]),
    "con_zones_neg5_zones": ("gp.Con_sa", [RASTER / "zones.tif", "-5"], [RASTER / "zones.tif"]),
    "con_int16_5_9": ("gp.Con_sa", [RASTER / "dem_int16_packbits.tif", "5"], ["9", "Value > 250"]),
    "con_dem_int16_5": ("gp.Con_sa", [RASTER / "dem.tif", RASTER / "dem_int16_packbits.tif"], ["5", "Value > 250"]),
    "con_25m_dem_int16": ("gp.Con_sa", [RASTER / "zones_25m.tif", RASTER / "dem.tif"], [RASTER / "dem_int16_packbits.tif"]),
    "con_dem_4m_shifted": ("gp.Con_sa", [RASTER / "dem.tif", RASTER / "zones_4m.tif"], [RASTER / "zones_shifted.tif", "Value > 250"]),
    "con_dem_dem_25m": ("gp.Con_sa", [RASTER / "dem.tif", RASTER / "dem.tif"], [RASTER / "zones_25m.tif", "Value > 250"]),
    "setnull_zones_int16": ("gp.SetNull_sa", [RASTER / "zones.tif", RASTER / "dem_int16_packbits.tif"], []),
    "setnull_zones_5": ("gp.SetNull_sa", [RASTER / "zones.tif", "5"], []),
    "setnull_25m_4m": ("gp.SetNull_sa", [RASTER / "zones_25m.tif", RASTER / "zones_4m.tif"], []),
    "setnull_25m_dem": ("gp.SetNull_sa", [RASTER / "zones_25m.tif", RASTER / "dem.tif"], []),
    "int_zones": ("gp.Int_sa", [RASTER / "zones.tif"], []),
    "int_int16": ("gp.Int_sa", [RASTER / "dem_int16_packbits.tif"], []),
    "plus_dem_landuse": ("gp.Plus_sa", [RASTER / "dem.tif", RASTER / "landuse_u8.tif"], []),
    "plus_shifted_dem": ("gp.Plus_sa", [RASTER / "zones_shifted.tif", RASTER / "dem.tif"], []),
    "plus_4m_dem": ("gp.Plus_sa", [RASTER / "zones_4m.tif", RASTER / "dem.tif"], []),
    "plus_25m_4m": ("gp.Plus_sa", [RASTER / "zones_25m.tif", RASTER / "zones_4m.tif"], []),
    "minus_int16_25m": ("gp.Minus_sa", [RASTER / "dem_int16_packbits.tif", RASTER / "zones_25m.tif"], []),
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
    "r2line_landuse": ("conversion.RasterToPolyline", [RASTER / "landuse_u8.tif"], []),
    "r2line_nosimplify": ("conversion.RasterToPolyline", [RASTER / "landuse_u8.tif"], ["ZERO", 0, "NO_SIMPLIFY", "Value"]),
    "r2line_nodata": ("conversion.RasterToPolyline", [RASTER / "landuse_u8.tif"], ["NODATA", 0, "SIMPLIFY", "Value"]),
    "r2line_dangle": ("conversion.RasterToPolyline", [RASTER / "landuse_u8.tif"], ["ZERO", 100, "SIMPLIFY", "Value"]),
    "r2line_float": ("conversion.RasterToPolyline", [RASTER / "dem.tif"], []),
    "f2r_parcels_zone": ("conversion.FeatureToRaster", [DATA / "parcels.shp", "ZONE"], [5]),
    "f2r_parcels_fid": ("conversion.FeatureToRaster", [DATA / "parcels.shp", "FID"], [7]),
    "f2r_wells_depth": ("conversion.FeatureToRaster", [DATA / "wells.shp", "DEPTH"], [10]),
    "f2r_roads": ("conversion.FeatureToRaster", [DATA / "roads.shp", "FID"], [4]),
    "p2r_parcels_center": ("conversion.PolygonToRaster", [DATA / "parcels.shp", "FID"], ["CELL_CENTER", "", 5]),
    "p2r_parcels_maxarea": ("conversion.PolygonToRaster", [DATA / "parcels.shp", "FID"], ["MAXIMUM_AREA", "", 5]),
    "p2r_parcels_zone_maxarea_25": ("conversion.PolygonToRaster", [DATA / "parcels.shp", "ZONE"], ["MAXIMUM_AREA", "", 25]),
    "p2r_parcels_zone_combined_25": ("conversion.PolygonToRaster", [DATA / "parcels.shp", "ZONE"], ["MAXIMUM_COMBINED_AREA", "", 25]),
    "p2r_combine_maxarea": ("conversion.PolygonToRaster", [RASTER / "combine_polys.shp", "CODE"], ["MAXIMUM_AREA", "", 10]),
    "p2r_combine_combined": ("conversion.PolygonToRaster", [RASTER / "combine_polys.shp", "CODE"], ["MAXIMUM_COMBINED_AREA", "", 10]),
    "pl2r_combine_maxlen": ("conversion.PolylineToRaster", [RASTER / "combine_lines.shp", "CODE"], ["MAXIMUM_LENGTH", "", 10]),
    "pl2r_combine_combined": ("conversion.PolylineToRaster", [RASTER / "combine_lines.shp", "CODE"], ["MAXIMUM_COMBINED_LENGTH", "", 10]),
    "pl2r_roads_class": ("conversion.PolylineToRaster", [DATA / "roads.shp", "CLASS"], ["MAXIMUM_LENGTH", "", 20]),
    "pl2r_roads_class_combined": ("conversion.PolylineToRaster", [DATA / "roads.shp", "CLASS"], ["MAXIMUM_COMBINED_LENGTH", "", 20]),
    "pt2r_wells_most": ("conversion.PointToRaster", [DATA / "wells.shp", "DEPTH"], ["MOST_FREQUENT", "", 20]),
    "pt2r_ties_most": ("conversion.PointToRaster", [RASTER / "tie_points.shp", "CLASS"], ["MOST_FREQUENT", "", 10]),
    "pt2r_ties_count": ("conversion.PointToRaster", [RASTER / "tie_points.shp", "CLASS"], ["COUNT", "", 10]),
    "pt2r_ties_mean": ("conversion.PointToRaster", [RASTER / "tie_points.shp", "CLASS"], ["MEAN", "", 10]),
    "pt2r_ties_max": ("conversion.PointToRaster", [RASTER / "tie_points.shp", "CLASS"], ["MAXIMUM", "", 10]),
    "pt2r_ties_range": ("conversion.PointToRaster", [RASTER / "tie_points.shp", "CLASS"], ["RANGE", "", 10]),
    "pt2r_ties_std": ("conversion.PointToRaster", [RASTER / "tie_points.shp", "CLASS"], ["STANDARD_DEVIATION", "", 10]),
    "pt2r_wells_std": ("conversion.PointToRaster", [DATA / "wells.shp", "DEPTH"], ["STANDARD_DEVIATION", "", 20]),
    "pt2r_wells_range": ("conversion.PointToRaster", [DATA / "wells.shp", "DEPTH"], ["RANGE", "", 20]),
    "f2r_ties": ("conversion.FeatureToRaster", [RASTER / "tie_points.shp", "CLASS"], [10]),
    "pt2r_wells_sum": ("conversion.PointToRaster", [DATA / "wells.shp", "DEPTH"], ["SUM", "", 20]),
    "eucdist_streams": ("gp.EucDistance_sa", [OUT / "streams_hydro.tif"], []),
    "eucdist_streams_max": ("gp.EucDistance_sa", [OUT / "streams_hydro.tif"], [150]),
    "eucdir_streams": ("gp.EucDirection_sa", [OUT / "streams_hydro.tif"], []),
    "eucdir_streams_max": ("gp.EucDirection_sa", [OUT / "streams_hydro.tif"], [150]),
    "eucalloc_streams": ("gp.EucAllocation_sa", [OUT / "streams_hydro.tif"], []),
    "eucalloc_streams_max": ("gp.EucAllocation_sa", [OUT / "streams_hydro.tif"], [150]),
    "kd_wells": ("gp.KernelDensity_sa", [DATA / "wells.shp", "NONE"], [30]),
    "kd_wells_depth_r": ("gp.KernelDensity_sa", [DATA / "wells.shp", "DEPTH"], [25, 150, "SQUARE_METERS"]),
    "kd_wells_default_depth": ("gp.KernelDensity_sa", [DATA / "wells.shp", "DEPTH"], [25]),
    "kd_wells_expected": ("gp.KernelDensity_sa", [DATA / "wells.shp", "NONE"], [25, 200, "SQUARE_KILOMETERS", "EXPECTED_COUNTS"]),
    "kd_roads": ("gp.KernelDensity_sa", [DATA / "roads.shp", "NONE"], [25]),
    "kd_roads_r": ("gp.KernelDensity_sa", [DATA / "roads.shp", "CLASS"], [20, 120, "SQUARE_MAP_UNITS"]),
    "pd_wells_circle": ("gp.PointDensity_sa", [DATA / "wells.shp", "NONE"], [25, "Circle 100 MAP", "SQUARE_KILOMETERS"]),
    "pd_wells_depth_rect": ("gp.PointDensity_sa", [DATA / "wells.shp", "DEPTH"], [20, "Rectangle 80 120 MAP", "SQUARE_METERS"]),
    "pd_wells_annulus_cell": ("gp.PointDensity_sa", [DATA / "wells.shp", "NONE"], [30, "Annulus 2 5 CELL", "SQUARE_METERS"]),
    "pd_wells_wedge": ("gp.PointDensity_sa", [DATA / "wells.shp", "NONE"], [30, "Wedge 120 90 180 MAP", "SQUARE_METERS"]),
    "pd_wells_wedge_wrap": ("gp.PointDensity_sa", [DATA / "wells.shp", "NONE"], [30, "Wedge 120 300 60 MAP", "SQUARE_METERS"]),
    "ld_roads": ("gp.LineDensity_sa", [DATA / "roads.shp", "NONE"], [25, 100, "SQUARE_KILOMETERS"]),
    "ld_roads_class": ("gp.LineDensity_sa", [DATA / "roads.shp", "CLASS"], [30, 80, "SQUARE_METERS"]),
    # Add Surface Information (in place; points and lines).
    "asi_wells_z": ("gp.AddSurfaceInformation_sa", [DATA / "wells.shp", RASTER / "dem.tif", "Z"], []),
    "asi_wells_bilinear": ("gp.AddSurfaceInformation_sa", [DATA / "wells.shp", RASTER / "dem.tif", "Z"], ["BILINEAR"]),
    "asi_wells_zfactor": ("gp.AddSurfaceInformation_sa", [DATA / "wells.shp", RASTER / "dem.tif", "Z"], ["LINEAR", "", 0.3048]),
    "asi_wells_nodata": ("gp.AddSurfaceInformation_sa", [DATA / "wells.shp", RASTER / "dem_nodata.tif", "Z"], []),
    "asi_roads_all": ("gp.AddSurfaceInformation_sa", [DATA / "roads.shp", RASTER / "dem.tif", "Z_MIN;Z_MAX;Z_MEAN;SURFACE_LENGTH;MIN_SLOPE;MAX_SLOPE;AVG_SLOPE"], []),
    "asi_roads_some": ("gp.AddSurfaceInformation_sa", [DATA / "roads.shp", RASTER / "dem.tif", "AVG_SLOPE;Z_MEAN;SURFACE_LENGTH"], []),
    "asi_roads_sample": ("gp.AddSurfaceInformation_sa", [DATA / "roads.shp", RASTER / "dem.tif", "Z_MIN;Z_MAX;Z_MEAN;SURFACE_LENGTH;MIN_SLOPE;MAX_SLOPE;AVG_SLOPE"], ["LINEAR", 25]),
    "asi_roads_nodata": ("gp.AddSurfaceInformation_sa", [DATA / "roads.shp", RASTER / "dem_nodata.tif", "Z_MIN;Z_MAX;Z_MEAN;SURFACE_LENGTH;MIN_SLOPE;MAX_SLOPE;AVG_SLOPE"], []),
    "asi_roads_bad": ("gp.AddSurfaceInformation_sa", [DATA / "roads.shp", RASTER / "dem.tif", "Z"], []),
    "evp_wells_dem": ("gp.ExtractValuesToPoints_sa", [DATA / "wells.shp", RASTER / "dem.tif"], []),
    "evp_wells_dem_interp": ("gp.ExtractValuesToPoints_sa", [DATA / "wells.shp", RASTER / "dem.tif"], ["INTERPOLATE"]),
    "evp_probe_nodata": ("gp.ExtractValuesToPoints_sa", [RASTER / "evp_points.shp", RASTER / "dem_nodata.tif"], []),
    "evp_probe_nodata_interp": ("gp.ExtractValuesToPoints_sa", [RASTER / "evp_points.shp", RASTER / "dem_nodata.tif"], ["INTERPOLATE"]),
    "evp_probe_classes": ("gp.ExtractValuesToPoints_sa", [RASTER / "evp_points.shp", RASTER / "classes_u8.tif"], []),
    "evp_probe_classes_interp": ("gp.ExtractValuesToPoints_sa", [RASTER / "evp_points.shp", RASTER / "classes_u8.tif"], ["INTERPOLATE"]),
    "evp_probe_f64": ("gp.ExtractValuesToPoints_sa", [RASTER / "evp_points.shp", RASTER / "dem_f64.tif"], ["INTERPOLATE"]),
    "ish_points": ("gp.InterpolateShape_sa", [RASTER / "dem.tif", DATA / "wells.shp"], ["", 1, "BILINEAR", "VERTICES_ONLY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "ish_nodata_bilinear": ("gp.InterpolateShape_sa", [RASTER / "dem_nodata.tif", DATA / "interpolate_pts.shp"], ["", 1, "BILINEAR", "VERTICES_ONLY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "ish_nodata_nearest": ("gp.InterpolateShape_sa", [RASTER / "dem_nodata.tif", DATA / "interpolate_pts.shp"], ["", 1, "NEAREST", "VERTICES_ONLY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "ish_nodata_lines": ("gp.InterpolateShape_sa", [RASTER / "dem_nodata.tif", DATA / "interpolate_lines.shp"], ["", 1, "BILINEAR", "VERTICES_ONLY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "ish_lines": ("gp.InterpolateShape_sa", [RASTER / "dem.tif", DATA / "roads.shp"], [100, 2, "BILINEAR", "DENSIFY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "ish_lines_nearest": ("gp.InterpolateShape_sa", [RASTER / "dem.tif", DATA / "roads.shp"], ["", 1, "NEAREST", "VERTICES_ONLY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "ish_polygons": ("gp.InterpolateShape_sa", [RASTER / "dem.tif", DATA / "parcels.shp"], ["", 1, "BILINEAR", "VERTICES_ONLY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "ish_zm": ("gp.InterpolateShape_sa", [RASTER / "dem.tif", DATA / "roads_zm.shp"], [100, 1, "BILINEAR", "DENSIFY", 0, "EXCLUDE", "INTERPOLATE_M_VALUE"]),
    "ish_zm_no_m": ("gp.InterpolateShape_sa", [RASTER / "dem.tif", DATA / "roads_zm.shp"], [100, 1, "BILINEAR", "DENSIFY", 0, "EXCLUDE", "NO_INTERPOLATE_M_VALUE"]),
    "extract_dem_zones": ("gp.ExtractByMask_sa", [RASTER / "dem.tif", RASTER / "zones.tif"], []),
    "extract_dem_mask25": ("gp.ExtractByMask_sa", [RASTER / "dem.tif", RASTER / "zones_25m.tif"], []),
    "extract_dem_mask4m": ("gp.ExtractByMask_sa", [RASTER / "dem.tif", RASTER / "zones_4m.tif"], []),
    "extract_int16_shifted": ("gp.ExtractByMask_sa", [RASTER / "dem_int16_packbits.tif", RASTER / "zones_shifted.tif"], []),
    "extract_int16_zones": ("gp.ExtractByMask_sa", [RASTER / "dem_int16_packbits.tif", RASTER / "zones.tif"], []),
    "extract_landuse_zones": ("gp.ExtractByMask_sa", [RASTER / "landuse_u8.tif", RASTER / "zones.tif"], []),
    "extract_nodata_mask25": ("gp.ExtractByMask_sa", [RASTER / "dem_nodata.tif", RASTER / "zones_25m.tif"], []),
    "resample_dem_nearest": ("management.Resample", [RASTER / "dem.tif"], ["25", "NEAREST"]),
    "resample_dem_bilinear": ("management.Resample", [RASTER / "dem.tif"], ["25", "BILINEAR"]),
    "resample_dem_cubic": ("management.Resample", [RASTER / "dem.tif"], ["7", "CUBIC"]),
    "resample_landuse_majority": ("management.Resample", [RASTER / "landuse_u8.tif"], ["60", "MAJORITY"]),
    "resample_classes_majority_60": ("management.Resample", [RASTER / "classes_u8.tif"], ["60", "MAJORITY"]),
    "resample_classes_majority_90": ("management.Resample", [RASTER / "classes_u8.tif"], ["90", "MAJORITY"]),
    "resample_classes_majority_45": ("management.Resample", [RASTER / "classes_u8.tif"], ["45", "MAJORITY"]),
    "zst_all": ("gp.ZonalStatisticsAsTable_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_nodata.tif"], ["DATA", "ALL"]),
    "zst_int_all": ("gp.ZonalStatisticsAsTable_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["DATA", "ALL"]),
    "zst_mean_nodata": ("gp.ZonalStatisticsAsTable_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_nodata.tif"], ["NODATA", "MEAN"]),
    "zst_min_max": ("gp.ZonalStatisticsAsTable_sa", [RASTER / "zones.tif", "Value", RASTER / "dem.tif"], ["DATA", "MIN_MAX_MEAN"]),
    "zst_classes_all": ("gp.ZonalStatisticsAsTable_sa", [RASTER / "zones_25m.tif", "Value", RASTER / "classes_u8.tif"], ["DATA", "ALL"]),
    "zst_25m_all": ("gp.ZonalStatisticsAsTable_sa", [RASTER / "zones_25m.tif", "Value", RASTER / "dem_nodata.tif"], ["DATA", "ALL"]),
    "zst_4m_int_all": ("gp.ZonalStatisticsAsTable_sa", [RASTER / "zones_4m.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["DATA", "ALL"]),
    "hillshade_shadows_315_15": ("gp.HillShade_sa", [RASTER / "dem.tif"], [315, 15, "SHADOWS", 3]),
    "hillshade_shadows_200_25": ("gp.HillShade_sa", [RASTER / "dem_nodata.tif"], [200, 25, "SHADOWS", 2]),
    "viewshed_hill": ("gp.Viewshed_sa", [RASTER / "dem.tif", RASTER / "observer_hill.shp"], []),
    "viewshed_two": ("gp.Viewshed_sa", [RASTER / "dem.tif", RASTER / "observers.shp"], []),
    "viewshed_hill_z2": ("gp.Viewshed_sa", [RASTER / "dem.tif", RASTER / "observer_hill.shp"], [2]),
    "zonal_mean": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_nodata.tif"], ["MEAN", "DATA"]),
    "zonal_max_strict": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_nodata.tif"], ["MAXIMUM", "NODATA"]),
    "zonal_range_int": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["RANGE", "DATA"]),
    "zonal_std": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem.tif"], ["STD", "DATA"]),
    "zonal_sum_int": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["SUM", "DATA"]),
    "zonal_median": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["MEDIAN", "DATA"]),
    "zonal_majority": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["MAJORITY", "DATA"]),
    "zonal_variety": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["VARIETY", "DATA"]),
    "zonal_min_f": ("gp.ZonalStatistics_sa", [RASTER / "zones.tif", "Value", RASTER / "dem.tif"], ["MINIMUM", "DATA"]),
    "zonal_majority_classes": ("gp.ZonalStatistics_sa", [RASTER / "zones_25m.tif", "Value", RASTER / "classes_u8.tif"], ["MAJORITY", "DATA"]),
    "zonal_minority_classes": ("gp.ZonalStatistics_sa", [RASTER / "zones_25m.tif", "Value", RASTER / "classes_u8.tif"], ["MINORITY", "DATA"]),
    "zonal_median_classes": ("gp.ZonalStatistics_sa", [RASTER / "zones_25m.tif", "Value", RASTER / "classes_u8.tif"], ["MEDIAN", "DATA"]),
    "zonal_mean_25m": ("gp.ZonalStatistics_sa", [RASTER / "zones_25m.tif", "Value", RASTER / "dem_nodata.tif"], ["MEAN", "DATA"]),
    "zonal_majority_shifted": ("gp.ZonalStatistics_sa", [RASTER / "zones_shifted.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["MAJORITY", "DATA"]),
    "zonal_median_4m": ("gp.ZonalStatistics_sa", [RASTER / "zones_4m.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["MEDIAN", "DATA"]),
    "zonal_range_4m": ("gp.ZonalStatistics_sa", [RASTER / "zones_4m.tif", "Value", RASTER / "dem_int16_packbits.tif"], ["RANGE", "NODATA"]),
    # Expand, Shrink, Majority Filter, Region Group (rules in unigis-raster::generalize).
    "expand_classes_1": ("gp.Expand_sa", [RASTER / "landuse_u8.tif"], [1, "1;2", "MORPHOLOGICAL"]),
    "expand_classes_2": ("gp.Expand_sa", [RASTER / "landuse_u8.tif"], [2, "0;3;5", "MORPHOLOGICAL"]),
    "expand_classes_dist_3": ("gp.Expand_sa", [RASTER / "classes_u8.tif"], [3, "1;2;5", "DISTANCE"]),
    "expand_patches_2": ("gp.Expand_sa", [RASTER / "patches_i16.tif"], [2, "-3;12", "MORPHOLOGICAL"]),
    "expand_patches_5": ("gp.Expand_sa", [RASTER / "patches_i16.tif"], [5, "2;7;12", "MORPHOLOGICAL"]),
    "expand_patches_dist_2_5": ("gp.Expand_sa", [RASTER / "patches_i16.tif"], [2.5, "2;7;12", "DISTANCE"]),
    "expand_patches_dist_1_5": ("gp.Expand_sa", [RASTER / "patches_i16.tif"], [1.5, "-3;7", "DISTANCE"]),
    "expand_landuse_4": ("gp.Expand_sa", [RASTER / "landuse_u8.tif"], [4, "2", "MORPHOLOGICAL"]),
    "expand_zones_dist_12": ("gp.Expand_sa", [RASTER / "zones.tif"], [12, "7;0", "DISTANCE"]),
    "expand_absent_zone": ("gp.Expand_sa", [RASTER / "landuse_u8.tif"], [1, "9", "MORPHOLOGICAL"]),
    "expand_bad_cells": ("gp.Expand_sa", [RASTER / "landuse_u8.tif"], [0, "1", "MORPHOLOGICAL"]),
    "expand_float": ("gp.Expand_sa", [RASTER / "dem.tif"], [1, "1", "MORPHOLOGICAL"]),
    "shrink_classes_1": ("gp.Shrink_sa", [RASTER / "landuse_u8.tif"], [1, "1;2;3", "MORPHOLOGICAL"]),
    "shrink_classes_3": ("gp.Shrink_sa", [RASTER / "landuse_u8.tif"], [3, "0;4", "MORPHOLOGICAL"]),
    "shrink_classes_dist_2": ("gp.Shrink_sa", [RASTER / "landuse_u8.tif"], [2, "4", "DISTANCE"]),
    "shrink_patches_2": ("gp.Shrink_sa", [RASTER / "patches_i16.tif"], [2, "2;7", "MORPHOLOGICAL"]),
    "shrink_patches_dist_3": ("gp.Shrink_sa", [RASTER / "patches_i16.tif"], [3, "0;2;7", "DISTANCE"]),
    "shrink_zones_6": ("gp.Shrink_sa", [RASTER / "zones.tif"], [6, "7;4", "MORPHOLOGICAL"]),
    "shrink_landuse_dist_1": ("gp.Shrink_sa", [RASTER / "landuse_u8.tif"], [1, "1;3", "DISTANCE"]),
    "majority_classes_four": ("gp.MajorityFilter_sa", [RASTER / "landuse_u8.tif"], ["FOUR", "MAJORITY"]),
    "majority_classes_four_half": ("gp.MajorityFilter_sa", [RASTER / "landuse_u8.tif"], ["FOUR", "HALF"]),
    "majority_classes_eight": ("gp.MajorityFilter_sa", [RASTER / "landuse_u8.tif"], ["EIGHT", "MAJORITY"]),
    "majority_classes_eight_half": ("gp.MajorityFilter_sa", [RASTER / "landuse_u8.tif"], ["EIGHT", "HALF"]),
    "majority_patches_eight_half": ("gp.MajorityFilter_sa", [RASTER / "patches_i16.tif"], ["EIGHT", "HALF"]),
    "majority_patches_four": ("gp.MajorityFilter_sa", [RASTER / "patches_i16.tif"], ["FOUR", "MAJORITY"]),
    "majority_landuse_eight": ("gp.MajorityFilter_sa", [RASTER / "landuse_u8.tif"], ["EIGHT", "MAJORITY"]),
    "majority_float": ("gp.MajorityFilter_sa", [RASTER / "dem.tif"], ["EIGHT", "MAJORITY"]),
    "regiongroup_classes_eight": ("gp.RegionGroup_sa", [RASTER / "landuse_u8.tif"], ["EIGHT", "WITHIN", "ADD_LINK"]),
    "regiongroup_classes_four": ("gp.RegionGroup_sa", [RASTER / "landuse_u8.tif"], ["FOUR", "WITHIN", "ADD_LINK"]),
    "regiongroup_classes_eight_cross": ("gp.RegionGroup_sa", [RASTER / "landuse_u8.tif"], ["EIGHT", "CROSS", "ADD_LINK"]),
    "regiongroup_patches_eight": ("gp.RegionGroup_sa", [RASTER / "patches_i16.tif"], ["EIGHT", "WITHIN", "ADD_LINK"]),
    "regiongroup_patches_four_cross": ("gp.RegionGroup_sa", [RASTER / "patches_i16.tif"], ["FOUR", "CROSS", "ADD_LINK"]),
    "regiongroup_patches_eight_excluded": ("gp.RegionGroup_sa", [RASTER / "patches_i16.tif"], ["EIGHT", "WITHIN", "ADD_LINK", 0]),
    "regiongroup_zones_four_excluded": ("gp.RegionGroup_sa", [RASTER / "zones.tif"], ["FOUR", "WITHIN", "NO_LINK", 7]),
    # Planar graph tools (testdata/planar: probes from scripts/research/planar).
    "p2l_grid": ("management.PolygonToLine", [PLANAR / "jitter_grid.shp"], ["IDENTIFY_NEIGHBORS"]),
    "p2l_overlapping": ("management.PolygonToLine", [PLANAR / "overlapping.shp"], ["IDENTIFY_NEIGHBORS"]),
    "p2l_holes": ("management.PolygonToLine", [PLANAR / "holes.shp"], ["IDENTIFY_NEIGHBORS"]),
    "p2l_nested": ("management.PolygonToLine", [PLANAR / "nested.shp"], ["IDENTIFY_NEIGHBORS"]),
    "p2l_parcels": ("management.PolygonToLine", [PARCELS], ["IDENTIFY_NEIGHBORS"]),
    "p2l_squares": ("management.PolygonToLine", [DATA / "squares.shp"], ["IDENTIFY_NEIGHBORS"]),
    "ftp_grid": ("management.FeatureToPolygon", [PLANAR / "jitter_grid.shp"], []),
    "ftp_overlapping": ("management.FeatureToPolygon", [PLANAR / "overlapping.shp"], []),
    "ftp_holes": ("management.FeatureToPolygon", [PLANAR / "holes.shp"], []),
    "ftp_nested": ("management.FeatureToPolygon", [PLANAR / "nested.shp"], []),
    "ftp_lines": ("management.FeatureToPolygon", [PLANAR / "lines.shp"], []),
    "ftp_parcels": ("management.FeatureToPolygon", [PARCELS], []),
    "ftp_parcels_wells": ("management.FeatureToPolygon", [PARCELS], ["", "ATTRIBUTES", DATA / "wells.shp"]),
    "ftp_parcels_wells_noattr": ("management.FeatureToPolygon", [PARCELS], ["", "NO_ATTRIBUTES", DATA / "wells.shp"]),
    "ftp_roads": ("management.FeatureToPolygon", [DATA / "roads.shp"], []),
    "ftl_lines": ("management.FeatureToLine", [PLANAR / "lines.shp"], []),
    "ftl_lines_noattr": ("management.FeatureToLine", [PLANAR / "lines.shp"], ["", "NO_ATTRIBUTES"]),
    "ftl_lines_bridges": ("management.FeatureToLine", [PLANAR / "lines_bridges.shp"], []),
    "ftl_grid": ("management.FeatureToLine", [PLANAR / "jitter_grid.shp"], []),
    "ftl_holes": ("management.FeatureToLine", [PLANAR / "holes.shp"], []),
    "ftl_roads": ("management.FeatureToLine", [DATA / "roads.shp"], []),
    "ftl_streams": ("management.FeatureToLine", [DATA / "streams.shp"], []),
    # Data Management breadth tools (testdata/dm: scripts/make-dm-testdata.sh, make-dm-chain.py).
    "env_roads": ("management.FeatureEnvelopeToPolygon", [DATA / "roads.shp"], []),
    "env_roads_multi": ("management.FeatureEnvelopeToPolygon", [DATA / "roads.shp"], ["MULTIPART"]),
    "env_parcels": ("management.FeatureEnvelopeToPolygon", [PARCELS], []),
    "env_holes": ("management.FeatureEnvelopeToPolygon", [PLANAR / "holes.shp"], []),
    "env_chain_multi": ("management.FeatureEnvelopeToPolygon", [DM / "chain.shp"], ["MULTIPART"]),
    "env_parts_multi": ("management.FeatureEnvelopeToPolygon", [DM / "parts.shp"], ["MULTIPART"]),
    "env_parts": ("management.FeatureEnvelopeToPolygon", [DM / "parts.shp"], []),
    "slv_chain": ("management.SplitLine", [DM / "chain.shp"], []),
    "slv_roads": ("management.SplitLine", [DATA / "roads.shp"], []),
    "slv_streams": ("management.SplitLine", [DATA / "streams.shp"], []),
    "usl_none": ("management.UnsplitLine", [DM / "chain_split.shp"], []),
    "usl_kind": ("management.UnsplitLine", [DM / "chain_split.shp"], ["KIND"]),
    "usl_class": ("management.UnsplitLine", [DM / "chain_split.shp"], ["CLASS"]),
    "usl_kind_class": ("management.UnsplitLine", [DM / "chain_split.shp"], ["KIND;CLASS"]),
    "usl_line_id": ("management.UnsplitLine", [DM / "chain_split.shp"], ["LINE_ID"]),
    "dice_lines_10": ("management.Dice", [DM / "dice_lines.shp"], [10]),
    "dice_lines_15": ("management.Dice", [DM / "dice_lines.shp"], [15]),
    "dice_multi_10": ("management.Dice", [DM / "dice_multi.shp"], [10]),
    "dice_multi_16": ("management.Dice", [DM / "dice_multi.shp"], [16]),
    "dice_polys_129": ("management.Dice", [DM / "dice_polys.shp"], [129]),
    "dice_polys_100": ("management.Dice", [DM / "dice_polys.shp"], [100]),
    "dice_polys_80": ("management.Dice", [DM / "dice_polys.shp"], [80]),
    "dice_polys_64": ("management.Dice", [DM / "dice_polys.shp"], [64]),
    "dice_polys_50": ("management.Dice", [DM / "dice_polys.shp"], [50]),
    "dice_polys_40": ("management.Dice", [DM / "dice_polys.shp"], [40]),
    "dice_polys_33": ("management.Dice", [DM / "dice_polys.shp"], [33]),
    "dice_polys_25": ("management.Dice", [DM / "dice_polys.shp"], [25]),
    "dice_polys_20": ("management.Dice", [DM / "dice_polys.shp"], [20]),
    "dice_polys_15": ("management.Dice", [DM / "dice_polys.shp"], [15]),
    "dice_polys_12": ("management.Dice", [DM / "dice_polys.shp"], [12]),
    "dice_polys_10": ("management.Dice", [DM / "dice_polys.shp"], [10]),
    "dice_multipoly_10": ("management.Dice", [DM / "dice_multipoly.shp"], [10]),
    "dice_multipoly_20": ("management.Dice", [DM / "dice_multipoly.shp"], [20]),
    "dice_multipoly_30": ("management.Dice", [DM / "dice_multipoly.shp"], [30]),
    "blk_u8_mean": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "MEAN", "DATA"]),
    "blk_u8_majority": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "MAJORITY", "DATA"]),
    "blk_u8_maximum": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "MAXIMUM", "DATA"]),
    "blk_u8_median": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "MEDIAN", "DATA"]),
    "blk_u8_minimum": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "MINIMUM", "DATA"]),
    "blk_u8_minority": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "MINORITY", "DATA"]),
    "blk_u8_range": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "RANGE", "DATA"]),
    "blk_u8_std": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "STD", "DATA"]),
    "blk_u8_sum": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "SUM", "DATA"]),
    "blk_u8_variety": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 4 CELL", "VARIETY", "DATA"]),
    "blk_i16_mean": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "MEAN", "DATA"]),
    "blk_i16_majority": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "MAJORITY", "DATA"]),
    "blk_i16_maximum": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "MAXIMUM", "DATA"]),
    "blk_i16_median": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "MEDIAN", "DATA"]),
    "blk_i16_minimum": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "MINIMUM", "DATA"]),
    "blk_i16_minority": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "MINORITY", "DATA"]),
    "blk_i16_range": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "RANGE", "DATA"]),
    "blk_i16_std": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "STD", "DATA"]),
    "blk_i16_sum": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "SUM", "DATA"]),
    "blk_i16_variety": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 5 3 CELL", "VARIETY", "DATA"]),
    "blk_f32_mean": ("gp.BlockStatistics_sa", [RASTER / "dem.tif"], ["Rectangle 4 4 CELL", "MEAN", "DATA"]),
    "blk_nod_mean": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "MEAN", "DATA"]),
    "blk_f32_maximum": ("gp.BlockStatistics_sa", [RASTER / "dem.tif"], ["Rectangle 4 4 CELL", "MAXIMUM", "DATA"]),
    "blk_nod_maximum": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "MAXIMUM", "DATA"]),
    "blk_f32_minimum": ("gp.BlockStatistics_sa", [RASTER / "dem.tif"], ["Rectangle 4 4 CELL", "MINIMUM", "DATA"]),
    "blk_nod_minimum": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "MINIMUM", "DATA"]),
    "blk_f32_range": ("gp.BlockStatistics_sa", [RASTER / "dem.tif"], ["Rectangle 4 4 CELL", "RANGE", "DATA"]),
    "blk_nod_range": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "RANGE", "DATA"]),
    "blk_f32_std": ("gp.BlockStatistics_sa", [RASTER / "dem.tif"], ["Rectangle 4 4 CELL", "STD", "DATA"]),
    "blk_nod_std": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "STD", "DATA"]),
    "blk_f32_sum": ("gp.BlockStatistics_sa", [RASTER / "dem.tif"], ["Rectangle 4 4 CELL", "SUM", "DATA"]),
    "blk_nod_sum": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "SUM", "DATA"]),
    "blk_nod_strict_mean": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 3 3 CELL", "MEAN", "NODATA"]),
    "blk_nod_strict_max": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Rectangle 4 4 CELL", "MAXIMUM", "NODATA"]),
    "blk_u8_circle2_mean": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Circle 2 CELL", "MEAN", "DATA"]),
    "blk_u8_circle3_maj": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Circle 3 CELL", "MAJORITY", "DATA"]),
    "blk_u8_annulus_sum": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Annulus 1 3 CELL", "SUM", "DATA"]),
    "blk_u8_map_max": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 90 120 MAP", "MAXIMUM", "DATA"]),
    "blk_i16_nodata_maj": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Rectangle 3 3 CELL", "MAJORITY", "NODATA"]),
    "blk_u8_1x1": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 1 1 CELL", "MEAN", "DATA"]),
    "blk_u8_big": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 7 9 CELL", "MEDIAN", "DATA"]),
    "blk_classes_maj": ("gp.BlockStatistics_sa", [RASTER / "classes_u8.tif"], ["Rectangle 3 3 CELL", "MAJORITY", "DATA"]),
    "blk_classes_min": ("gp.BlockStatistics_sa", [RASTER / "classes_u8.tif"], ["Rectangle 4 2 CELL", "MINORITY", "DATA"]),
    "blk_var_1": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 1 1 CELL", "VARIETY", "DATA"]),
    "blk_var_2": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 2 2 CELL", "VARIETY", "DATA"]),
    "blk_var_3": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 3 3 CELL", "VARIETY", "DATA"]),
    "blk_var_4": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 4 4 CELL", "VARIETY", "DATA"]),
    "blk_var_5": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 5 5 CELL", "VARIETY", "DATA"]),
    "blk_var_6": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 6 6 CELL", "VARIETY", "DATA"]),
    "blk_var_8": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 8 8 CELL", "VARIETY", "DATA"]),
    "blk_var_16": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 16 16 CELL", "VARIETY", "DATA"]),
    "blk_i32_mean": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "MEAN", "DATA"]),
    "blk_i32_majority": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "MAJORITY", "DATA"]),
    "blk_i32_maximum": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "MAXIMUM", "DATA"]),
    "blk_i32_median": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "MEDIAN", "DATA"]),
    "blk_i32_minimum": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "MINIMUM", "DATA"]),
    "blk_i32_minority": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "MINORITY", "DATA"]),
    "blk_i32_range": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "RANGE", "DATA"]),
    "blk_i32_std": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "STD", "DATA"]),
    "blk_i32_sum": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "SUM", "DATA"]),
    "blk_i32_variety": ("gp.BlockStatistics_sa", [RASTER / "dem_bigtiff_i32.tif"], ["Rectangle 4 3 CELL", "VARIETY", "DATA"]),
    "blk_i16_var_circle": ("gp.BlockStatistics_sa", [RASTER / "patches_i16.tif"], ["Circle 3 CELL", "VARIETY", "DATA"]),
    "blk_f32_circle_mean": ("gp.BlockStatistics_sa", [RASTER / "dem.tif"], ["Circle 4 CELL", "MEAN", "DATA"]),
    "blk_f32_annulus_std": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Annulus 2 4 CELL", "STD", "DATA"]),
    "blk_f32_circle_strict": ("gp.BlockStatistics_sa", [RASTER / "dem_nodata.tif"], ["Circle 2 CELL", "SUM", "NODATA"]),
    "blk_map_rect_odd": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 100 130 MAP", "MAXIMUM", "DATA"]),
    "blk_map_rect_half": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 75 45 MAP", "SUM", "DATA"]),
    "blk_map_circle_15": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Circle 45 MAP", "SUM", "DATA"]),
    "blk_map_circle_25": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Circle 75 MAP", "SUM", "DATA"]),
    "blk_map_circle_33": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Circle 100 MAP", "SUM", "DATA"]),
    "blk_map_annulus": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Annulus 30 75 MAP", "SUM", "DATA"]),
    "blk_circle_frac": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Circle 2.5 CELL", "SUM", "DATA"]),
    "blk_rect_zero": ("gp.BlockStatistics_sa", [RASTER / "landuse_u8.tif"], ["Rectangle 0 3 CELL", "SUM", "DATA"]),
    "blk_f64": ("gp.BlockStatistics_sa", [RASTER / "dem_f64.tif"], ["Rectangle 3 3 CELL", "MEAN", "DATA"]),
    "blk_u16": ("gp.BlockStatistics_sa", [RASTER / "rgb_pixel_u8.tif"], ["Rectangle 3 3 CELL", "MAXIMUM", "DATA"]),
    "zh_i32_i16": ("gp.ZonalHistogram_sa", [RASTER / "tab_zones_i32.tif", "Value", RASTER / "tab_classes_i16.tif"], []),
    "zh_i32_f32": ("gp.ZonalHistogram_sa", [RASTER / "tab_zones_i32.tif", "Value", RASTER / "tab_values_f32.tif"], []),
    "zh_zones25_landuse": ("gp.ZonalHistogram_sa", [RASTER / "zt_zones25.tif", "Value", RASTER / "zt_landuse.tif"], []),
    "zh_landuse_zones4": ("gp.ZonalHistogram_sa", [RASTER / "zt_landuse.tif", "Value", RASTER / "zt_zones4.tif"], []),
    "zh_classes_landuse": ("gp.ZonalHistogram_sa", [RASTER / "zt_classes.tif", "Value", RASTER / "zt_landuse.tif"], []),
    "ta_i32_i16": ("gp.TabulateArea_sa", [RASTER / "tab_zones_i32.tif", "Value", RASTER / "tab_classes_i16.tif", "Value"], []),
    "ta_i32_i16_rows": ("gp.TabulateArea_sa", [RASTER / "tab_zones_i32.tif", "Value", RASTER / "tab_classes_i16.tif", "Value"], ["", "CLASSES_AS_ROWS"]),
    "ta_i32_i16_cell60": ("gp.TabulateArea_sa", [RASTER / "tab_zones_i32.tif", "Value", RASTER / "tab_classes_i16.tif", "Value"], [60]),
    "ta_i32_i16_cell20": ("gp.TabulateArea_sa", [RASTER / "tab_zones_i32.tif", "Value", RASTER / "tab_classes_i16.tif", "Value"], [20]),
    "ta_zones25_landuse": ("gp.TabulateArea_sa", [RASTER / "zt_zones25.tif", "Value", RASTER / "zt_landuse.tif", "Value"], []),
    "ta_landuse_zones4": ("gp.TabulateArea_sa", [RASTER / "zt_landuse.tif", "Value", RASTER / "zt_zones4.tif", "Value"], []),
    "ta_classes_landuse_rows": ("gp.TabulateArea_sa", [RASTER / "zt_classes.tif", "Value", RASTER / "zt_landuse.tif", "Value"], ["", "CLASSES_AS_ROWS"]),
    "zgt_i32": ("gp.ZonalGeometryAsTable_sa", [RASTER / "tab_zones_i32.tif", "Value"], []),
    "zgt_zones25": ("gp.ZonalGeometryAsTable_sa", [RASTER / "zt_zones25.tif", "Value"], []),
    "zgt_landuse": ("gp.ZonalGeometryAsTable_sa", [RASTER / "zt_landuse.tif", "Value"], []),
    "zgt_classes": ("gp.ZonalGeometryAsTable_sa", [RASTER / "zt_classes.tif", "Value"], []),
    "zgt_zones4": ("gp.ZonalGeometryAsTable_sa", [RASTER / "zt_zones4.tif", "Value"], []),
    "cdist_basic": ("gp.CostDistance_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_f32.tif"], []),
    "cdist_int": ("gp.CostDistance_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_int16.tif"], []),
    "cdist_const": ("gp.CostDistance_sa", [RASTER / "cost_src_one.tif", RASTER / "cost_const1.tif"], []),
    "cdist_max": ("gp.CostDistance_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_f32.tif"], [400]),
    "cdist_max_int": ("gp.CostDistance_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_int16.tif"], [300]),
    "cdist_one_f32": ("gp.CostDistance_sa", [RASTER / "cost_src_one.tif", RASTER / "cost_f32.tif"], []),
    "cback_basic": ("gp.CostBackLink_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_f32.tif"], []),
    "cback_const": ("gp.CostBackLink_sa", [RASTER / "cost_src_one.tif", RASTER / "cost_const1.tif"], []),
    "cback_int": ("gp.CostBackLink_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_int16.tif"], []),
    "cback_max": ("gp.CostBackLink_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_f32.tif"], [400]),
    "calloc_basic": ("gp.CostAllocation_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_f32.tif"], []),
    "calloc_const": ("gp.CostAllocation_sa", [RASTER / "cost_src_one.tif", RASTER / "cost_const1.tif"], []),
    "calloc_int": ("gp.CostAllocation_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_int16.tif"], []),
    "calloc_max": ("gp.CostAllocation_sa", [RASTER / "cost_src_i16.tif", RASTER / "cost_f32.tif"], [400]),
    "corridor_basic": ("gp.Corridor_sa", [OUT / "cdist_basic.tif", OUT / "cdist_one_f32.tif"], []),
    "corridor_int": ("gp.Corridor_sa", [OUT / "cdist_int.tif", OUT / "cdist_max.tif"], []),
    "cpath_each_cell": ("gp.CostPath_sa", [RASTER / "cost_dest_i16.tif", OUT / "cdist_basic.tif", OUT / "cback_basic.tif"], ["EACH_CELL"]),
    "cpath_each_zone": ("gp.CostPath_sa", [RASTER / "cost_dest_i16.tif", OUT / "cdist_basic.tif", OUT / "cback_basic.tif"], ["EACH_ZONE"]),
    "cpath_best": ("gp.CostPath_sa", [RASTER / "cost_dest_i16.tif", OUT / "cdist_basic.tif", OUT / "cback_basic.tif"], ["BEST_SINGLE"]),
    "cpath_const": ("gp.CostPath_sa", [RASTER / "cost_dest_i16.tif", OUT / "cdist_const.tif", OUT / "cback_const.tif"], ["EACH_CELL"]),
    "filter_low_dem": ("gp.Filter_sa", [RASTER / "dem.tif"], ["LOW", "DATA"]),
    "filter_low_nodata": ("gp.Filter_sa", [RASTER / "dem_nodata.tif"], ["LOW", "DATA"]),
    "filter_low_nodata_strict": ("gp.Filter_sa", [RASTER / "dem_nodata.tif"], ["LOW", "NODATA"]),
    "filter_low_landuse": ("gp.Filter_sa", [RASTER / "landuse_u8.tif"], ["LOW", "DATA"]),
    "filter_low_i16": ("gp.Filter_sa", [RASTER / "patches_i16.tif"], ["LOW", "DATA"]),
    "filter_high_dem": ("gp.Filter_sa", [RASTER / "dem.tif"], ["HIGH", "DATA"]),
    "filter_high_nodata": ("gp.Filter_sa", [RASTER / "dem_nodata.tif"], ["HIGH", "DATA"]),
    "filter_high_nodata_strict": ("gp.Filter_sa", [RASTER / "dem_nodata.tif"], ["HIGH", "NODATA"]),
    "filter_high_landuse": ("gp.Filter_sa", [RASTER / "landuse_u8.tif"], ["HIGH", "DATA"]),
    "filter_high_i16": ("gp.Filter_sa", [RASTER / "patches_i16.tif"], ["HIGH", "DATA"]),
    "nibble_all": ("gp.Nibble_sa", [RASTER / "landuse_u8.tif", RASTER / "nibble_mask_i16.tif"], ["ALL_VALUES", "PRESERVE_NODATA"]),
    "nibble_data": ("gp.Nibble_sa", [RASTER / "landuse_u8.tif", RASTER / "nibble_mask_i16.tif"], ["DATA_ONLY", "PRESERVE_NODATA"]),
    "nibble_process": ("gp.Nibble_sa", [RASTER / "landuse_u8.tif", RASTER / "nibble_mask_i16.tif"], ["ALL_VALUES", "PROCESS_NODATA"]),
    "nibble_i16": ("gp.Nibble_sa", [RASTER / "patches_i16.tif", RASTER / "nibble_mask_i16.tif"], ["ALL_VALUES", "PRESERVE_NODATA"]),
    "rbt_dem": ("gp.ReclassByTable_sa", [RASTER / "dem.tif", RASTER / "rbt_dem.dbf", "FROMV", "TOV", "OUTV"], ["DATA"]),
    "rbt_dem_nodata": ("gp.ReclassByTable_sa", [RASTER / "dem.tif", RASTER / "rbt_dem.dbf", "FROMV", "TOV", "OUTV"], ["NODATA"]),
    "rbt_int": ("gp.ReclassByTable_sa", [RASTER / "landuse_u8.tif", RASTER / "rbt_int.dbf", "FROMV", "TOV", "OUTV"], ["DATA"]),
    "rbt_int_nodata": ("gp.ReclassByTable_sa", [RASTER / "landuse_u8.tif", RASTER / "rbt_int.dbf", "FROMV", "TOV", "OUTV"], ["NODATA"]),
    "rbt_floatout": ("gp.ReclassByTable_sa", [RASTER / "dem.tif", RASTER / "rbt_float_out.dbf", "FROMV", "TOV", "OUTV"], ["DATA"]),
    "rbt_negative": ("gp.ReclassByTable_sa", [RASTER / "dem.tif", RASTER / "rbt_negative.dbf", "FROMV", "TOV", "OUTV"], ["DATA"]),
    "rbt_i16": ("gp.ReclassByTable_sa", [RASTER / "patches_i16.tif", RASTER / "rbt_int.dbf", "FROMV", "TOV", "OUTV"], ["DATA"]),
    "slice_ei5": ("gp.Slice_sa", [RASTER / "dem.tif"], [5, "EQUAL_INTERVAL", 1]),
    "slice_ei7_base3": ("gp.Slice_sa", [RASTER / "dem.tif"], [7, "EQUAL_INTERVAL", 3]),
    "slice_ea5": ("gp.Slice_sa", [RASTER / "dem.tif"], [5, "EQUAL_AREA", 1]),
    "slice_ea4_nodata": ("gp.Slice_sa", [RASTER / "dem_nodata.tif"], [4, "EQUAL_AREA", 1]),
    "slice_ei3_nodata": ("gp.Slice_sa", [RASTER / "dem_nodata.tif"], [3, "EQUAL_INTERVAL", 1]),
    "slice_ei4_i16": ("gp.Slice_sa", [RASTER / "patches_i16.tif"], [4, "EQUAL_INTERVAL", 1]),
    "ebattr_gt": ("gp.ExtractByAttributes_sa", [RASTER / "dem.tif"] + ["VALUE > 250"], []),
    "ebattr_range": ("gp.ExtractByAttributes_sa", [RASTER / "dem.tif"] + ["VALUE >= 220 AND VALUE <= 260"], []),
    "ebattr_int": ("gp.ExtractByAttributes_sa", [RASTER / "landuse_u8.tif"] + ["VALUE = 2 OR VALUE = 3"], []),
    "ebattr_nodata": ("gp.ExtractByAttributes_sa", [RASTER / "dem_nodata.tif"] + ["VALUE < 240"], []),
    "ebrect_in": ("gp.ExtractByRectangle_sa", [RASTER / "dem.tif", "500203 5000305 500796 5000893"], ["INSIDE"]),
    "ebrect_out": ("gp.ExtractByRectangle_sa", [RASTER / "dem.tif", "500203 5000305 500796 5000893"], ["OUTSIDE"]),
    "ebrect_int": ("gp.ExtractByRectangle_sa", [RASTER / "landuse_u8.tif", "500200 5000400 500600 5001000"], ["INSIDE"]),
    "ebcirc_in": ("gp.ExtractByCircle_sa", [RASTER / "dem.tif", "500600 5000600", 250], ["INSIDE"]),
    "ebcirc_out": ("gp.ExtractByCircle_sa", [RASTER / "dem.tif", "500600 5000600", 250], ["OUTSIDE"]),
    "ebcirc_int": ("gp.ExtractByCircle_sa", [RASTER / "landuse_u8.tif", "500400 5000800", 183], ["INSIDE"]),
    "lookup_value": ("gp.Lookup_sa", [RASTER / "zt_landuse.tif"] + ["Value"], []),
    "lookup_count": ("gp.Lookup_sa", [RASTER / "zt_landuse.tif"] + ["COUNT"], []),
    "lookup_zones_count": ("gp.Lookup_sa", [RASTER / "zt_zones25.tif"] + ["COUNT"], []),
    "sink_hydro": ("gp.Sink_sa", [OUT / "flowdir_hydro_raw.tif"], []),
    "sink_filled": ("gp.Sink_sa", [OUT / "flowdir_hydro_filled.tif"], []),
    "pick_const": ("gp.Pick_sa", [RASTER / "landuse_u8.tif", "10;20;30;40;50"], []),
    "pick_float": ("gp.Pick_sa", [RASTER / "landuse_u8.tif", "1.5;2.5;3.5;4.5;5.5"], []),
    "pick_raster": ("gp.Pick_sa", [RASTER / "landuse_u8.tif", str(RASTER / "patches_i16.tif") + ";7;" + str(RASTER / "classes_u8.tif")], []),
    "lp_u8": ("gp.LowestPosition_sa", [f"{RASTER / 'landuse_u8.tif'};{RASTER / 'classes_u8.tif'}"], []),
    "lp_float": ("gp.LowestPosition_sa", [f"{RASTER / 'dem.tif'};{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'dem_f64.tif'}"], []),
    "lp_nodata": ("gp.LowestPosition_sa", [f"{RASTER / 'dem_nodata.tif'};{RASTER / 'dem.tif'}"], []),
    "lp_tie": ("gp.LowestPosition_sa", [f"{RASTER / 'landuse_u8.tif'};{RASTER / 'landuse_u8.tif'};{RASTER / 'landuse_u8.tif'}"], []),
    "flip_dem": ("management.Flip", [RASTER / "dem.tif"], []),
    "flip_landuse": ("management.Flip", [RASTER / "landuse_u8.tif"], []),
    "mirror_dem": ("management.Mirror", [RASTER / "dem.tif"], []),
    "mirror_nodata": ("management.Mirror", [RASTER / "dem_nodata.tif"], []),
    "shift_dem": ("management.Shift", [RASTER / "dem.tif"], [25, -35]),
    "shift_landuse": ("management.Shift", [RASTER / "landuse_u8.tif"], [-12.5, 7]),
    "rclip_rect": ("management.Clip", [RASTER / "dem.tif", "500203 5000305 500796 5000893"], ["", "", "NONE", "NO_MAINTAIN_EXTENT"]),
    "rclip_rect_int": ("management.Clip", [RASTER / "landuse_u8.tif", "500100 5000500 500400 5001000"], ["", "", "NONE", "NO_MAINTAIN_EXTENT"]),
    "rclip_rect_maintain": ("management.Clip", [RASTER / "dem.tif", "500203 5000305 500796 5000893"], ["", "", "NONE", "MAINTAIN_EXTENT"]),
    "rclip_rect_nodata": ("management.Clip", [RASTER / "dem_nodata.tif", "500203 5000305 500796 5000893"], ["", "-5", "NONE", "NO_MAINTAIN_EXTENT"]),
    "rclip_beyond": ("management.Clip", [RASTER / "landuse_u8.tif", "499900 5000900 500300 5001400"], ["", "", "NONE", "NO_MAINTAIN_EXTENT"]),
    "pwbuf_roads_25": ("analysis.PairwiseBuffer", [DATA / "roads.shp"], ["25 Meters"]),
    "pwbuf_roads_25_all": ("analysis.PairwiseBuffer", [DATA / "roads.shp"], ["25 Meters", "ALL"]),
    "pwbuf_parcels_10": ("analysis.PairwiseBuffer", [DATA / "parcels.shp"], ["10 Meters"]),
    "pwbuf_parcels_zone": ("analysis.PairwiseBuffer", [DATA / "parcels.shp"], ["10 Meters", "LIST", "ZONE"]),
    "pwbuf_wells_30": ("analysis.PairwiseBuffer", [DATA / "wells.shp"], ["30 Meters"]),
    "pwdis_parcels": ("analysis.PairwiseDissolve", [DATA / "parcels.shp"], []),
    "pwdis_parcels_zone": ("analysis.PairwiseDissolve", [DATA / "parcels.shp"], ["ZONE"]),
    "pwdis_parcels_zone_stats": ("analysis.PairwiseDissolve", [DATA / "parcels.shp"], ["ZONE", "VALUE SUM;VALUE MEAN"]),
    "pwdis_parcels_single": ("analysis.PairwiseDissolve", [DATA / "parcels.shp"], ["ZONE", "", "SINGLE_PART"]),
    "pwint_parcels_clip": ("analysis.PairwiseIntersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], []),
    "pwint_roads_parcels": ("analysis.PairwiseIntersect", [f"{DATA / 'roads.shp'};{DATA / 'parcels.shp'}"], []),
    "pwint_roads_parcels_point": ("analysis.PairwiseIntersect", [f"{DATA / 'roads.shp'};{DATA / 'parcels.shp'}"], ["ALL", "", "POINT"]),
    "pwint_wells_parcels": ("analysis.PairwiseIntersect", [f"{DATA / 'wells.shp'};{DATA / 'parcels.shp'}"], []),
    "pwint_parcels_clip_nofid": ("analysis.PairwiseIntersect", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["NO_FID"]),
    "pwclip_parcels": ("analysis.PairwiseClip", [DATA / "parcels.shp", DATA / "clip_areas.shp"], []),
    "pwclip_roads": ("analysis.PairwiseClip", [DATA / "roads.shp", DATA / "clip_areas.shp"], []),
    "pwclip_wells": ("analysis.PairwiseClip", [DATA / "wells.shp", DATA / "clip_areas.shp"], []),
    "pwerase_parcels": ("analysis.PairwiseErase", [DATA / "parcels.shp", DATA / "clip_areas.shp"], []),
    "pwerase_roads": ("analysis.PairwiseErase", [DATA / "roads.shp", DATA / "clip_areas.shp"], []),
    "cr_sales": ("management.CopyRows", [DM / "sales.dbf"], []),
    "cr_parcels": ("management.CopyRows", [DATA / "parcels.shp"], []),
    "mrb_wells": ("analysis.MultipleRingBuffer", [DATA / "wells.shp"], ["50;100", "Meters"]),
    "mrb_wells_all": ("analysis.MultipleRingBuffer", [DATA / "wells.shp"], ["50;100", "Meters", "dist", "ALL"]),
    "mrb_parcels_outside": ("analysis.MultipleRingBuffer", [DATA / "parcels.shp"], ["10;30", "Meters", "dist", "ALL", "OUTSIDE_ONLY"]),
    "mrb_roads": ("analysis.MultipleRingBuffer", [DATA / "roads.shp"], ["20;40;60", "Meters"]),
    "ppn_squares": ("analysis.PairwisePolygonNeighbors", [DATA / "squares.shp"], []),
    "ppn_squares_fields": ("analysis.PairwisePolygonNeighbors", [DATA / "squares.shp"], ['SQ_ID;NAME']),
    "ppn_edges": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], []),
    "ppn_overlap": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['NAME', 'AREA_OVERLAP']),
    "ppn_one_side": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['NAME', 'AREA_OVERLAP', 'NO_BOTH_SIDES']),
    "ppn_groups": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['GRP', 'AREA_OVERLAP']),
    "ppn_groups_no_area": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['GRP']),
    "ppn_groups_one_side": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['GRP', 'AREA_OVERLAP', 'NO_BOTH_SIDES']),
    "ppn_multi_fields": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['GRP;NAME', 'AREA_OVERLAP']),
    "ppn_oid_field": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['FID', 'AREA_OVERLAP']),
    "ppn_units": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['NAME', 'AREA_OVERLAP', 'BOTH_SIDES', '', 'KILOMETERS', 'HECTARES']),
    "ppn_units_feet": ("analysis.PairwisePolygonNeighbors", [DATA / "pairwise_neighbors.shp"], ['NAME', 'AREA_OVERLAP', 'BOTH_SIDES', '', 'FEET_INTERNATIONAL', 'SQUARE_FEET']),
    "ppn_err_line": ("analysis.PairwisePolygonNeighbors", [DATA / "roads.shp"], []),
    "ppn_err_field": ("analysis.PairwisePolygonNeighbors", [DATA / "squares.shp"], ['NO_SUCH']),
    "pn_squares": ("analysis.PolygonNeighbors", [DATA / "squares.shp"], ["SQ_ID", "NO_AREA_OVERLAP", "BOTH_SIDES"]),
    "pn_squares_area": ("analysis.PolygonNeighbors", [DATA / "squares.shp"], ["SQ_ID;NAME", "AREA_OVERLAP", "BOTH_SIDES"]),
    "cof_clip": ("analysis.CountOverlappingFeatures", [str(DATA / "clip_areas.shp")], []),
    "sbd_squares_3": ("management.SubdividePolygon", [DATA / "squares.shp"], ["NUMBER_OF_EQUAL_PARTS", 3]),
    "sbd_clip_4": ("management.SubdividePolygon", [DATA / "clip_areas.shp"], ["NUMBER_OF_EQUAL_PARTS", 4]),
    "sbd_clip_area": ("management.SubdividePolygon", [DATA / "clip_areas.shp"], ["EQUAL_AREAS", "", 5000]),
    "sbd_parcels_4": ("management.SubdividePolygon", [DATA / "parcels.shp"], ["NUMBER_OF_EQUAL_PARTS", 4]),
    "ti_parcels_clip": ("analysis.TabulateIntersection", [DATA / "parcels.shp", "ZONE", DATA / "clip_areas.shp"], ["AREA_ID"]),
    "ti_clip_parcels": ("analysis.TabulateIntersection", [DATA / "clip_areas.shp", "AREA_ID", DATA / "parcels.shp"], ["ZONE"]),
    "ti_clip_roads": ("analysis.TabulateIntersection", [DATA / "clip_areas.shp", "AREA_ID", DATA / "roads.shp"], ["", "", "", "METERS"]),
    "ti_multi": ("analysis.TabulateIntersection", [DATA / "parcels.shp", "ZONE;NAME", DATA / "clip_areas.shp"], ["AREA_ID", "", "", "SQUARE_METERS"]),
    "ti_nofields": ("analysis.TabulateIntersection", [DATA / "parcels.shp", "ZONE", DATA / "clip_areas.shp"], []),
    "th_wells": ("analysis.CreateThiessenPolygons", [DATA / "wells.shp"], ["ONLY_FID"]),
    "th_wells_all": ("analysis.CreateThiessenPolygons", [DATA / "wells.shp"], ["ALL"]),
    "th_pts": ("analysis.CreateThiessenPolygons", [DM / "pts.shp"], ["ALL"]),
    "sl_streams_5": ("cartography.SimplifyLine", [DATA / "streams.shp"], ["POINT_REMOVE", "5 Meters", "RESOLVE_ERRORS", "KEEP_COLLAPSED_POINTS", "CHECK"]),
    "sl_roads_5": ("cartography.SimplifyLine", [DATA / "roads.shp"], ["POINT_REMOVE", "5 Meters", "RESOLVE_ERRORS", "KEEP_COLLAPSED_POINTS", "CHECK"]),
    "sl_roads_20": ("cartography.SimplifyLine", [DATA / "roads.shp"], ["POINT_REMOVE", "20 Meters", "RESOLVE_ERRORS", "KEEP_COLLAPSED_POINTS", "CHECK"]),
    "r2a_dem": ("conversion.RasterToASCII", [RASTER / "dem.tif"], []),
    "r2a_int16": ("conversion.RasterToASCII", [RASTER / "dem_int16_packbits.tif"], []),
    "r2a_u8": ("conversion.RasterToASCII", [RASTER / "landuse_u8.tif"], []),
    "r2a_nodata": ("conversion.RasterToASCII", [RASTER / "dem_nodata.tif"], []),
    "r2a_f64": ("conversion.RasterToASCII", [RASTER / "dem_f64.tif"], []),
    "r2a_zones": ("conversion.RasterToASCII", [RASTER / "zones.tif"], []),
    "tsel_sales_2020": ("analysis.TableSelect", [DM / "sales.dbf"], ["YEAR = 2020"]),
    "tsel_sales_north": ("analysis.TableSelect", [DM / "sales.dbf"], ["REGION = 'north' AND AMOUNT > 10"]),
    "tsel_sales_all": ("analysis.TableSelect", [DM / "sales.dbf"], []),
    "tsel_parcels": ("analysis.TableSelect", [DATA / "parcels.shp"], ["ZONE = 1"]),
    "ef_where": ("conversion.ExportFeatures", [DATA / "parcels.shp"], ["ZONE = 1"]),
    "ef_all": ("conversion.ExportFeatures", [DATA / "wells.shp"], []),
    "gt_roads_100": ("management.GenerateTransectsAlongLines", [DATA / "roads.shp"], ["100 Meters", "20 Meters", "NO_END_POINTS"]),
    "gt_roads_80_end": ("management.GenerateTransectsAlongLines", [DATA / "roads.shp"], ["80 Meters", "30 Meters", "END_POINTS"]),
    "gt_streams_50": ("management.GenerateTransectsAlongLines", [DATA / "streams.shp"], ["50 Meters", "10 Meters"]),
    "gix_parcels": ("cartography.GridIndexFeatures", [DATA / "parcels.shp"], ["INTERSECTFEATURE", "NO_USEPAGEUNIT", "", "150 Meters", "150 Meters"]),
    "gix_wells_all": ("cartography.GridIndexFeatures", [DATA / "wells.shp"], ["NO_INTERSECTFEATURE", "NO_USEPAGEUNIT", "", "200 Meters", "100 Meters"]),
    "gix_wells": ("cartography.GridIndexFeatures", [DATA / "wells.shp"], ["INTERSECTFEATURE", "NO_USEPAGEUNIT", "", "90 Meters", "90 Meters"]),
    "gix_roads": ("cartography.GridIndexFeatures", [DATA / "roads.shp"], ["INTERSECTFEATURE", "NO_USEPAGEUNIT", "", "120 Meters", "120 Meters"]),
    "rba_dem": ("gp.ReclassByASCIIFile_sa", [RASTER / "dem.tif", RASTER / "remap_ascii1.txt"], ["DATA"]),
    "rba_dem_nodata": ("gp.ReclassByASCIIFile_sa", [RASTER / "dem.tif", RASTER / "remap_ascii1.txt"], ["NODATA"]),
    "rba_int": ("gp.ReclassByASCIIFile_sa", [RASTER / "landuse_u8.tif", RASTER / "remap_ascii2.txt"], ["DATA"]),
    "rba_int_nodata": ("gp.ReclassByASCIIFile_sa", [RASTER / "landuse_u8.tif", RASTER / "remap_ascii2.txt"], ["NODATA"]),
    "ext_15_feature": ("edit.ExtendLine", [DATA / "extend_lines.shp"], ["15 Meters", "FEATURE"]),
    "ext_50_feature": ("edit.ExtendLine", [DATA / "extend_lines.shp"], ["50 Meters", "FEATURE"]),
    "ext_100_feature": ("edit.ExtendLine", [DATA / "extend_lines.shp"], ["100 Meters", "FEATURE"]),
    "ext_15_extension": ("edit.ExtendLine", [DATA / "extend_lines.shp"], ["15 Meters", "EXTENSION"]),
    "ext_25_extension": ("edit.ExtendLine", [DATA / "extend_lines.shp"], ["25 Meters", "EXTENSION"]),
    "ext_50_extension": ("edit.ExtendLine", [DATA / "extend_lines.shp"], ["50 Meters", "EXTENSION"]),
    "trim_15_delete": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["15 Meters", "DELETE_SHORT"]),
    "trim_15_keep": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["15 Meters", "KEEP_SHORT"]),
    "trim_25_delete": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["25 Meters", "DELETE_SHORT"]),
    "trim_25_keep": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["25 Meters", "KEEP_SHORT"]),
    "trim_35_delete": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["35 Meters", "DELETE_SHORT"]),
    "trim_35_keep": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["35 Meters", "KEEP_SHORT"]),
    "trim_60_delete": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["60 Meters", "DELETE_SHORT"]),
    "trim_60_keep": ("edit.TrimLine", [DATA / "trim_lines.shp"], ["60 Meters", "KEEP_SHORT"]),
    "flip_roads": ("edit.FlipLine", [DATA / "roads.shp"], []),
    "flip_streams": ("edit.FlipLine", [DATA / "streams.shp"], []),
    "gen_roads_20": ("edit.Generalize", [DATA / "roads.shp"], ["20 Meters"]),
    "gen_roads_5": ("edit.Generalize", [DATA / "roads.shp"], ["5 Meters"]),
    "gen_streams_30": ("edit.Generalize", [DATA / "streams.shp"], ["30 Meters"]),
    "r2f_dem": ("conversion.RasterToFloat", [RASTER / "dem.tif"], []),
    "r2f_int16": ("conversion.RasterToFloat", [RASTER / "dem_int16_packbits.tif"], []),
    "r2f_u8": ("conversion.RasterToFloat", [RASTER / "landuse_u8.tif"], []),
    "r2f_nodata": ("conversion.RasterToFloat", [RASTER / "dem_nodata.tif"], []),
    "r2f_f64": ("conversion.RasterToFloat", [RASTER / "dem_f64.tif"], []),
    "cprbands_u8_default": ("management.CopyRaster", [RASTER / "composite_matrix/u8.tif"], []),
    "cprbands_u8_f64": ("management.CopyRaster", [RASTER / "composite_matrix/u8.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_i8_default": ("management.CopyRaster", [RASTER / "composite_matrix/i8.tif"], []),
    "cprbands_i8_f64": ("management.CopyRaster", [RASTER / "composite_matrix/i8.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_u16_default": ("management.CopyRaster", [RASTER / "composite_matrix/u16.tif"], []),
    "cprbands_u16_f64": ("management.CopyRaster", [RASTER / "composite_matrix/u16.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_i16_default": ("management.CopyRaster", [RASTER / "composite_matrix/i16.tif"], []),
    "cprbands_i16_f64": ("management.CopyRaster", [RASTER / "composite_matrix/i16.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_u32_default": ("management.CopyRaster", [RASTER / "composite_matrix/u32.tif"], []),
    "cprbands_u32_f64": ("management.CopyRaster", [RASTER / "composite_matrix/u32.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_i32_default": ("management.CopyRaster", [RASTER / "composite_matrix/i32.tif"], []),
    "cprbands_i32_f64": ("management.CopyRaster", [RASTER / "composite_matrix/i32.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_f32_default": ("management.CopyRaster", [RASTER / "composite_matrix/f32.tif"], []),
    "cprbands_f32_f64": ("management.CopyRaster", [RASTER / "composite_matrix/f32.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_f64_default": ("management.CopyRaster", [RASTER / "composite_matrix/f64.tif"], []),
    "cprbands_f64_f64": ("management.CopyRaster", [RASTER / "composite_matrix/f64.tif"], ['', '', '', '', '', '64_BIT']),
    "cprbands_rgb8_default": ("management.CopyRaster", [RASTER / "rgb_pixel_u8.tif"], []),
    "cprbands_rgb8_f32": ("management.CopyRaster", [RASTER / "rgb_pixel_u8.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprbands_rgb8_u16": ("management.CopyRaster", [RASTER / "rgb_pixel_u8.tif"], ['', '', '', '', '', '16_BIT_UNSIGNED']),
    "cprbands_rgb16_default": ("management.CopyRaster", [RASTER / "rgb_planar_u16.tif"], []),
    "cprbands_rgb16_f32": ("management.CopyRaster", [RASTER / "rgb_planar_u16.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprbands_rgb16_u16": ("management.CopyRaster", [RASTER / "rgb_planar_u16.tif"], ['', '', '', '', '', '16_BIT_UNSIGNED']),
    "cprbands_first_nodata": ("management.CopyRaster", [RASTER / "copy_raster/first_nodata.tif"], []),
    "cprbands_later_nodata": ("management.CopyRaster", [RASTER / "copy_raster/later_nodata.tif"], []),
    "cprcast_u8_f32": ("management.CopyRaster", [RASTER / "composite_matrix/u8.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_u8_i16": ("management.CopyRaster", [RASTER / "composite_matrix/u8.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "cprcast_i8_f32": ("management.CopyRaster", [RASTER / "composite_matrix/i8.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_i8_i16": ("management.CopyRaster", [RASTER / "composite_matrix/i8.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "cprcast_u16_f32": ("management.CopyRaster", [RASTER / "composite_matrix/u16.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_u16_i16": ("management.CopyRaster", [RASTER / "composite_matrix/u16.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "cprcast_i16_f32": ("management.CopyRaster", [RASTER / "composite_matrix/i16.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_i16_i16": ("management.CopyRaster", [RASTER / "composite_matrix/i16.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "cprcast_u32_f32": ("management.CopyRaster", [RASTER / "composite_matrix/u32.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_u32_i16": ("management.CopyRaster", [RASTER / "composite_matrix/u32.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "cprcast_i32_f32": ("management.CopyRaster", [RASTER / "composite_matrix/i32.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_i32_i16": ("management.CopyRaster", [RASTER / "composite_matrix/i32.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "cprcast_f32_f32": ("management.CopyRaster", [RASTER / "composite_matrix/f32.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_f32_i16": ("management.CopyRaster", [RASTER / "composite_matrix/f32.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "cprcast_f64_f32": ("management.CopyRaster", [RASTER / "composite_matrix/f64.tif"], ['', '', '', '', '', '32_BIT_FLOAT']),
    "cprcast_f64_i16": ("management.CopyRaster", [RASTER / "composite_matrix/f64.tif"], ['', '', '', '', '', '16_BIT_SIGNED']),
    "freq_eq_basic": ("gp.EqualToFrequency_sa", [RASTER / 'composite_matrix/u8.tif', f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/f32.tif'}"], ['SINGLE_BAND']),
    "freq_eq_value_nd": ("gp.EqualToFrequency_sa", [RASTER / 'composite_matrix/nd_u8.tif', f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], ['SINGLE_BAND']),
    "freq_eq_list_nd": ("gp.EqualToFrequency_sa", [RASTER / 'composite_matrix/u8.tif', f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], ['SINGLE_BAND']),
    "freq_eq_multi_single": ("gp.EqualToFrequency_sa", [RASTER / 'copy_raster/first_nodata.tif', f"{RASTER / 'copy_raster/later_nodata.tif'}"], ['SINGLE_BAND']),
    "freq_eq_multi_multi": ("gp.EqualToFrequency_sa", [RASTER / 'copy_raster/first_nodata.tif', f"{RASTER / 'copy_raster/later_nodata.tif'}"], ['MULTI_BAND']),
    "freq_eq_f64_precision": ("gp.EqualToFrequency_sa", [RASTER / 'composite_matrix/f64.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], ['SINGLE_BAND']),
    "freq_eq_f32_precision": ("gp.EqualToFrequency_sa", [RASTER / 'composite_matrix/f32.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], ['SINGLE_BAND']),
    "freq_eq_u32_precision": ("gp.EqualToFrequency_sa", [RASTER / 'composite_matrix/u32.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/u32.tif'}"], ['SINGLE_BAND']),
    "freq_eq_val_broadcast": ("gp.EqualToFrequency_sa", [RASTER / 'frequency/single.tif', f"{RASTER / 'frequency/list2.tif'}"], ['MULTI_BAND']),
    "freq_eq_list_broadcast": ("gp.EqualToFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/single.tif'}"], ['MULTI_BAND']),
    "freq_eq_multi_pair": ("gp.EqualToFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'}"], ['MULTI_BAND']),
    "freq_eq_single_flat": ("gp.EqualToFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'}"], ['SINGLE_BAND']),
    "freq_eq_two_lists": ("gp.EqualToFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'};{RASTER / 'frequency/single.tif'}"], ['MULTI_BAND']),
    "freq_eq_unequal": ("gp.EqualToFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/three.tif'}"], ['MULTI_BAND']),
    "freq_gt_basic": ("gp.GreaterThanFrequency_sa", [RASTER / 'composite_matrix/u8.tif', f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/f32.tif'}"], ['SINGLE_BAND']),
    "freq_gt_value_nd": ("gp.GreaterThanFrequency_sa", [RASTER / 'composite_matrix/nd_u8.tif', f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], ['SINGLE_BAND']),
    "freq_gt_list_nd": ("gp.GreaterThanFrequency_sa", [RASTER / 'composite_matrix/u8.tif', f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], ['SINGLE_BAND']),
    "freq_gt_multi_single": ("gp.GreaterThanFrequency_sa", [RASTER / 'copy_raster/first_nodata.tif', f"{RASTER / 'copy_raster/later_nodata.tif'}"], ['SINGLE_BAND']),
    "freq_gt_multi_multi": ("gp.GreaterThanFrequency_sa", [RASTER / 'copy_raster/first_nodata.tif', f"{RASTER / 'copy_raster/later_nodata.tif'}"], ['MULTI_BAND']),
    "freq_gt_f64_precision": ("gp.GreaterThanFrequency_sa", [RASTER / 'composite_matrix/f64.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], ['SINGLE_BAND']),
    "freq_gt_f32_precision": ("gp.GreaterThanFrequency_sa", [RASTER / 'composite_matrix/f32.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], ['SINGLE_BAND']),
    "freq_gt_u32_precision": ("gp.GreaterThanFrequency_sa", [RASTER / 'composite_matrix/u32.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/u32.tif'}"], ['SINGLE_BAND']),
    "freq_gt_val_broadcast": ("gp.GreaterThanFrequency_sa", [RASTER / 'frequency/single.tif', f"{RASTER / 'frequency/list2.tif'}"], ['MULTI_BAND']),
    "freq_gt_list_broadcast": ("gp.GreaterThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/single.tif'}"], ['MULTI_BAND']),
    "freq_gt_multi_pair": ("gp.GreaterThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'}"], ['MULTI_BAND']),
    "freq_gt_single_flat": ("gp.GreaterThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'}"], ['SINGLE_BAND']),
    "freq_gt_two_lists": ("gp.GreaterThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'};{RASTER / 'frequency/single.tif'}"], ['MULTI_BAND']),
    "freq_gt_unequal": ("gp.GreaterThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/three.tif'}"], ['MULTI_BAND']),
    "freq_lt_basic": ("gp.LessThanFrequency_sa", [RASTER / 'composite_matrix/u8.tif', f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/f32.tif'}"], ['SINGLE_BAND']),
    "freq_lt_value_nd": ("gp.LessThanFrequency_sa", [RASTER / 'composite_matrix/nd_u8.tif', f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], ['SINGLE_BAND']),
    "freq_lt_list_nd": ("gp.LessThanFrequency_sa", [RASTER / 'composite_matrix/u8.tif', f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], ['SINGLE_BAND']),
    "freq_lt_multi_single": ("gp.LessThanFrequency_sa", [RASTER / 'copy_raster/first_nodata.tif', f"{RASTER / 'copy_raster/later_nodata.tif'}"], ['SINGLE_BAND']),
    "freq_lt_multi_multi": ("gp.LessThanFrequency_sa", [RASTER / 'copy_raster/first_nodata.tif', f"{RASTER / 'copy_raster/later_nodata.tif'}"], ['MULTI_BAND']),
    "freq_lt_f64_precision": ("gp.LessThanFrequency_sa", [RASTER / 'composite_matrix/f64.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], ['SINGLE_BAND']),
    "freq_lt_f32_precision": ("gp.LessThanFrequency_sa", [RASTER / 'composite_matrix/f32.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], ['SINGLE_BAND']),
    "freq_lt_u32_precision": ("gp.LessThanFrequency_sa", [RASTER / 'composite_matrix/u32.tif', f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/u32.tif'}"], ['SINGLE_BAND']),
    "freq_lt_val_broadcast": ("gp.LessThanFrequency_sa", [RASTER / 'frequency/single.tif', f"{RASTER / 'frequency/list2.tif'}"], ['MULTI_BAND']),
    "freq_lt_list_broadcast": ("gp.LessThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/single.tif'}"], ['MULTI_BAND']),
    "freq_lt_multi_pair": ("gp.LessThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'}"], ['MULTI_BAND']),
    "freq_lt_single_flat": ("gp.LessThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'}"], ['SINGLE_BAND']),
    "freq_lt_two_lists": ("gp.LessThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/list2.tif'};{RASTER / 'frequency/single.tif'}"], ['MULTI_BAND']),
    "freq_lt_unequal": ("gp.LessThanFrequency_sa", [RASTER / 'frequency/value2.tif', f"{RASTER / 'frequency/three.tif'}"], ['MULTI_BAND']),
    "freq_eq_count1": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count2": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count3": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count4": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count5": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count7": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count14": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count15": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count16": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count127": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count128": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count254": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count255": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_count256": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_eq_single_then_two": ("gp.EqualToFrequency_sa", [RASTER / "frequency/single.tif", f"{RASTER / 'frequency/single.tif'};{RASTER / 'frequency/list2.tif'}"], ["MULTI_BAND"]),
    "freq_eq_two_then_single": ("gp.EqualToFrequency_sa", [RASTER / "frequency/single.tif", f"{RASTER / 'frequency/list2.tif'};{RASTER / 'frequency/single.tif'}"], ["MULTI_BAND"]),
    "freq_eq_two_then_three": ("gp.EqualToFrequency_sa", [RASTER / "frequency/single.tif", f"{RASTER / 'frequency/list2.tif'};{RASTER / 'frequency/three.tif'}"], ["MULTI_BAND"]),
    "freq_eq_three_then_two": ("gp.EqualToFrequency_sa", [RASTER / "frequency/single.tif", f"{RASTER / 'frequency/three.tif'};{RASTER / 'frequency/list2.tif'}"], ["MULTI_BAND"]),
    "freq_eq_three_value": ("gp.EqualToFrequency_sa", [RASTER / "frequency/three.tif", f"{RASTER / 'frequency/value2.tif'}"], ["MULTI_BAND"]),
    "freq_gt_zero4": ("gp.GreaterThanFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "freq_lt_zero4": ("gp.LessThanFrequency_sa", [RASTER / "composite_matrix/u8.tif", f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "um64_abs_f64": ("gp.Abs_sa", [RASTER / "composite_matrix/f64.tif"], []),
    "um64_abs_nd_f64": ("gp.Abs_sa", [RASTER / "composite_matrix/nd_f64.tif"], []),
    "um64_negate_f64": ("gp.Negate_sa", [RASTER / "composite_matrix/f64.tif"], []),
    "um64_negate_nd_f64": ("gp.Negate_sa", [RASTER / "composite_matrix/nd_f64.tif"], []),
    "um64_square_f64": ("gp.Square_sa", [RASTER / "composite_matrix/f64.tif"], []),
    "um64_square_nd_f64": ("gp.Square_sa", [RASTER / "composite_matrix/nd_f64.tif"], []),
    "um64_abs_f64_cached": ("gp.Abs_sa", [RASTER / "unary_types/f64_cached.tif"], []),
    "um64_abs_f64_nd_cached": ("gp.Abs_sa", [RASTER / "unary_types/f64_nd_cached.tif"], []),
    "um64_negate_f64_cached": ("gp.Negate_sa", [RASTER / "unary_types/f64_cached.tif"], []),
    "um64_negate_f64_nd_cached": ("gp.Negate_sa", [RASTER / "unary_types/f64_nd_cached.tif"], []),
    "um64_square_f64_cached": ("gp.Square_sa", [RASTER / "unary_types/f64_cached.tif"], []),
    "um64_square_f64_nd_cached": ("gp.Square_sa", [RASTER / "unary_types/f64_nd_cached.tif"], []),
    **{f"umcachemulti_{op.lower()}_{name}":
       (f"gp.{op}_sa", [RASTER / "unary_cached_multiband" / f"{name}.tif"], [])
       for op in ["Abs", "Negate", "Square"]
       for name in ["u8", "i8", "u16", "i16", "u32", "i32", "i16_reverse", "i8_nonpositive", "i16_nonpositive", "u32_overflow", "u32_three_bands", "u32_single_overflow"]},
    "umfloatnext_log2_precision": ("gp.Log2_sa", [RASTER / "unary_log2_precision/input.tif"], []),
    **{f"umfloatnext_{op.lower()}_{name}":
       (f"gp.{op}_sa", [RASTER / "unary_float_multiband" / f"{name}.tif"], [])
       for op in ["Log2", "Sin", "RoundUp"]
       for name in ["float32_raw", "float32_cached", "float64_raw", "float64_cached"]},
    **{f"umfloatmore_{op.lower()}_{name}":
       (f"gp.{op}_sa", [RASTER / "unary_float_multiband" / f"{name}.tif"], [])
       for op in ["SquareRoot", "Ln", "Exp"]
       for name in ["float32_raw", "float32_cached", "float64_raw", "float64_cached"]},
    **{f"umrawmulti_{op.lower()}_{name}":
       (f"gp.{op}_sa", [RASTER / "unary_multiband" / f"{name}.tif"], [])
       for op in ["Abs", "Negate", "Square"]
       for name in ["u8", "i8", "u16", "i16", "u32", "i32", "i16_reverse"]},
    "umint_abs_u8": ("gp.Abs_sa", [RASTER / "composite_matrix/u8.tif"], []),
    "umint_abs_nd_u8": ("gp.Abs_sa", [RASTER / "composite_matrix/nd_u8.tif"], []),
    "umint_abs_i8": ("gp.Abs_sa", [RASTER / "composite_matrix/i8.tif"], []),
    "umint_abs_nd_i8": ("gp.Abs_sa", [RASTER / "composite_matrix/nd_i8.tif"], []),
    "umint_abs_u16": ("gp.Abs_sa", [RASTER / "composite_matrix/u16.tif"], []),
    "umint_abs_nd_u16": ("gp.Abs_sa", [RASTER / "composite_matrix/nd_u16.tif"], []),
    "umint_abs_i16": ("gp.Abs_sa", [RASTER / "composite_matrix/i16.tif"], []),
    "umint_abs_nd_i16": ("gp.Abs_sa", [RASTER / "composite_matrix/nd_i16.tif"], []),
    "umint_abs_u32": ("gp.Abs_sa", [RASTER / "composite_matrix/u32.tif"], []),
    "umint_abs_nd_u32": ("gp.Abs_sa", [RASTER / "composite_matrix/nd_u32.tif"], []),
    "umint_abs_i32": ("gp.Abs_sa", [RASTER / "composite_matrix/i32.tif"], []),
    "umint_abs_nd_i32": ("gp.Abs_sa", [RASTER / "composite_matrix/nd_i32.tif"], []),
    "umint_negate_u8": ("gp.Negate_sa", [RASTER / "composite_matrix/u8.tif"], []),
    "umint_negate_nd_u8": ("gp.Negate_sa", [RASTER / "composite_matrix/nd_u8.tif"], []),
    "umint_negate_i8": ("gp.Negate_sa", [RASTER / "composite_matrix/i8.tif"], []),
    "umint_negate_nd_i8": ("gp.Negate_sa", [RASTER / "composite_matrix/nd_i8.tif"], []),
    "umint_negate_u16": ("gp.Negate_sa", [RASTER / "composite_matrix/u16.tif"], []),
    "umint_negate_nd_u16": ("gp.Negate_sa", [RASTER / "composite_matrix/nd_u16.tif"], []),
    "umint_negate_i16": ("gp.Negate_sa", [RASTER / "composite_matrix/i16.tif"], []),
    "umint_negate_nd_i16": ("gp.Negate_sa", [RASTER / "composite_matrix/nd_i16.tif"], []),
    "umint_negate_u32": ("gp.Negate_sa", [RASTER / "composite_matrix/u32.tif"], []),
    "umint_negate_nd_u32": ("gp.Negate_sa", [RASTER / "composite_matrix/nd_u32.tif"], []),
    "umint_negate_i32": ("gp.Negate_sa", [RASTER / "composite_matrix/i32.tif"], []),
    "umint_negate_nd_i32": ("gp.Negate_sa", [RASTER / "composite_matrix/nd_i32.tif"], []),
    "umint_square_u8": ("gp.Square_sa", [RASTER / "composite_matrix/u8.tif"], []),
    "umint_square_nd_u8": ("gp.Square_sa", [RASTER / "composite_matrix/nd_u8.tif"], []),
    "umint_square_i8": ("gp.Square_sa", [RASTER / "composite_matrix/i8.tif"], []),
    "umint_square_nd_i8": ("gp.Square_sa", [RASTER / "composite_matrix/nd_i8.tif"], []),
    "umint_square_u16": ("gp.Square_sa", [RASTER / "composite_matrix/u16.tif"], []),
    "umint_square_nd_u16": ("gp.Square_sa", [RASTER / "composite_matrix/nd_u16.tif"], []),
    "umint_square_i16": ("gp.Square_sa", [RASTER / "composite_matrix/i16.tif"], []),
    "umint_square_nd_i16": ("gp.Square_sa", [RASTER / "composite_matrix/nd_i16.tif"], []),
    "umint_square_u32": ("gp.Square_sa", [RASTER / "composite_matrix/u32.tif"], []),
    "umint_square_nd_u32": ("gp.Square_sa", [RASTER / "composite_matrix/nd_u32.tif"], []),
    "umint_square_i32": ("gp.Square_sa", [RASTER / "composite_matrix/i32.tif"], []),
    "umint_square_nd_i32": ("gp.Square_sa", [RASTER / "composite_matrix/nd_i32.tif"], []),
    "umint_boundary_square_i32": ("gp.Square_sa", [RASTER / "unary_types/square_i32.tif"], []),
    "umint_boundary_square_u32": ("gp.Square_sa", [RASTER / "unary_types/square_u32.tif"], []),
    "umint_boundary_negate_u32": ("gp.Negate_sa", [RASTER / "unary_types/negate_u32.tif"], []),
    "freq_eq_count255_nd": ("gp.EqualToFrequency_sa", [RASTER / "composite_matrix/nd_u8.tif", f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'}"], []),
    "freq_gt_count255_nd": ("gp.GreaterThanFrequency_sa", [RASTER / "composite_matrix/nd_u8.tif", f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'}"], []),
    "freq_lt_count255_nd": ("gp.LessThanFrequency_sa", [RASTER / "composite_matrix/nd_u8.tif", f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'}"], []),
    "umcache_abs_i8_narrow": ("gp.Abs_sa", [RASTER / "unary_cached/i8_narrow.tif"], []),
    "umcache_abs_i16_narrow": ("gp.Abs_sa", [RASTER / "unary_cached/i16_narrow.tif"], []),
    "umcache_abs_u16_narrow": ("gp.Abs_sa", [RASTER / "unary_cached/u16_narrow.tif"], []),
    "umcache_abs_i8_wide": ("gp.Abs_sa", [RASTER / "unary_cached/i8_wide.tif"], []),
    "umcache_abs_i16_wide": ("gp.Abs_sa", [RASTER / "unary_cached/i16_wide.tif"], []),
    "umcache_abs_i16_ndmin": ("gp.Abs_sa", [RASTER / "unary_cached/i16_ndmin.tif"], []),
    "umcache_abs_i16_tiny": ("gp.Abs_sa", [RASTER / "unary_cached/i16_tiny.tif"], []),
    "umcache_abs_i16_bit": ("gp.Abs_sa", [RASTER / "unary_cached/i16_bit.tif"], []),
    "umcache_abs_i16_negative": ("gp.Abs_sa", [RASTER / "unary_cached/i16_negative.tif"], []),
    "umcache_abs_u8": ("gp.Abs_sa", [RASTER / "unary_cached/u8.tif"], []),
    "umcache_abs_nd_u8": ("gp.Abs_sa", [RASTER / "unary_cached/nd_u8.tif"], []),
    "umcache_abs_u32": ("gp.Abs_sa", [RASTER / "unary_cached/u32.tif"], []),
    "umcache_abs_nd_u32": ("gp.Abs_sa", [RASTER / "unary_cached/nd_u32.tif"], []),
    "umcache_abs_i32": ("gp.Abs_sa", [RASTER / "unary_cached/i32.tif"], []),
    "umcache_abs_nd_i32": ("gp.Abs_sa", [RASTER / "unary_cached/nd_i32.tif"], []),
    "umcache_abs_i32_positive_max": ("gp.Abs_sa", [RASTER / "unary_cached/i32_positive_max.tif"], []),
    "umcache_abs_i32_negative_min": ("gp.Abs_sa", [RASTER / "unary_cached/i32_negative_min.tif"], []),
    "umcache_abs_i32_negative_max": ("gp.Abs_sa", [RASTER / "unary_cached/i32_negative_max.tif"], []),
    "umcache_abs_i32_ndmin": ("gp.Abs_sa", [RASTER / "unary_cached/i32_ndmin.tif"], []),
    "umcache_abs_u16": ("gp.Abs_sa", [RASTER / "unary_cached/u16.tif"], []),
    "umcache_abs_nd_u16": ("gp.Abs_sa", [RASTER / "unary_cached/nd_u16.tif"], []),
    "umcache_abs_i16_cross3": ("gp.Abs_sa", [RASTER / "unary_cached/i16_cross3.tif"], []),
    "umcache_abs_i16_cross1": ("gp.Abs_sa", [RASTER / "unary_cached/i16_cross1.tif"], []),
    "umcache_abs_i16_neg1to3": ("gp.Abs_sa", [RASTER / "unary_cached/i16_neg1to3.tif"], []),
    "umcache_abs_u32_max": ("gp.Abs_sa", [RASTER / "unary_cached/u32_max.tif"], []),
    "umcache_abs_constant1": ("gp.Abs_sa", [RASTER / "unary_cached/constant1.tif"], []),
    "umcache_negate_i8_narrow": ("gp.Negate_sa", [RASTER / "unary_cached/i8_narrow.tif"], []),
    "umcache_negate_i16_narrow": ("gp.Negate_sa", [RASTER / "unary_cached/i16_narrow.tif"], []),
    "umcache_negate_u16_narrow": ("gp.Negate_sa", [RASTER / "unary_cached/u16_narrow.tif"], []),
    "umcache_negate_i8_wide": ("gp.Negate_sa", [RASTER / "unary_cached/i8_wide.tif"], []),
    "umcache_negate_i16_wide": ("gp.Negate_sa", [RASTER / "unary_cached/i16_wide.tif"], []),
    "umcache_negate_i16_ndmin": ("gp.Negate_sa", [RASTER / "unary_cached/i16_ndmin.tif"], []),
    "umcache_negate_i16_tiny": ("gp.Negate_sa", [RASTER / "unary_cached/i16_tiny.tif"], []),
    "umcache_negate_i16_bit": ("gp.Negate_sa", [RASTER / "unary_cached/i16_bit.tif"], []),
    "umcache_negate_i16_negative": ("gp.Negate_sa", [RASTER / "unary_cached/i16_negative.tif"], []),
    "umcache_negate_u8": ("gp.Negate_sa", [RASTER / "unary_cached/u8.tif"], []),
    "umcache_negate_nd_u8": ("gp.Negate_sa", [RASTER / "unary_cached/nd_u8.tif"], []),
    "umcache_negate_u32": ("gp.Negate_sa", [RASTER / "unary_cached/u32.tif"], []),
    "umcache_negate_nd_u32": ("gp.Negate_sa", [RASTER / "unary_cached/nd_u32.tif"], []),
    "umcache_negate_i32": ("gp.Negate_sa", [RASTER / "unary_cached/i32.tif"], []),
    "umcache_negate_nd_i32": ("gp.Negate_sa", [RASTER / "unary_cached/nd_i32.tif"], []),
    "umcache_negate_i32_positive_max": ("gp.Negate_sa", [RASTER / "unary_cached/i32_positive_max.tif"], []),
    "umcache_negate_i32_negative_min": ("gp.Negate_sa", [RASTER / "unary_cached/i32_negative_min.tif"], []),
    "umcache_negate_i32_negative_max": ("gp.Negate_sa", [RASTER / "unary_cached/i32_negative_max.tif"], []),
    "umcache_negate_i32_ndmin": ("gp.Negate_sa", [RASTER / "unary_cached/i32_ndmin.tif"], []),
    "umcache_negate_u16": ("gp.Negate_sa", [RASTER / "unary_cached/u16.tif"], []),
    "umcache_negate_nd_u16": ("gp.Negate_sa", [RASTER / "unary_cached/nd_u16.tif"], []),
    "umcache_negate_i16_cross3": ("gp.Negate_sa", [RASTER / "unary_cached/i16_cross3.tif"], []),
    "umcache_negate_i16_cross1": ("gp.Negate_sa", [RASTER / "unary_cached/i16_cross1.tif"], []),
    "umcache_negate_i16_neg1to3": ("gp.Negate_sa", [RASTER / "unary_cached/i16_neg1to3.tif"], []),
    "umcache_negate_u32_max": ("gp.Negate_sa", [RASTER / "unary_cached/u32_max.tif"], []),
    "umcache_negate_constant1": ("gp.Negate_sa", [RASTER / "unary_cached/constant1.tif"], []),
    "umcache_square_i8_narrow": ("gp.Square_sa", [RASTER / "unary_cached/i8_narrow.tif"], []),
    "umcache_square_i16_narrow": ("gp.Square_sa", [RASTER / "unary_cached/i16_narrow.tif"], []),
    "umcache_square_u16_narrow": ("gp.Square_sa", [RASTER / "unary_cached/u16_narrow.tif"], []),
    "umcache_square_i8_wide": ("gp.Square_sa", [RASTER / "unary_cached/i8_wide.tif"], []),
    "umcache_square_i16_wide": ("gp.Square_sa", [RASTER / "unary_cached/i16_wide.tif"], []),
    "umcache_square_i16_ndmin": ("gp.Square_sa", [RASTER / "unary_cached/i16_ndmin.tif"], []),
    "umcache_square_i16_tiny": ("gp.Square_sa", [RASTER / "unary_cached/i16_tiny.tif"], []),
    "umcache_square_i16_bit": ("gp.Square_sa", [RASTER / "unary_cached/i16_bit.tif"], []),
    "umcache_square_i16_negative": ("gp.Square_sa", [RASTER / "unary_cached/i16_negative.tif"], []),
    "umcache_square_u8": ("gp.Square_sa", [RASTER / "unary_cached/u8.tif"], []),
    "umcache_square_nd_u8": ("gp.Square_sa", [RASTER / "unary_cached/nd_u8.tif"], []),
    "umcache_square_u32": ("gp.Square_sa", [RASTER / "unary_cached/u32.tif"], []),
    "umcache_square_nd_u32": ("gp.Square_sa", [RASTER / "unary_cached/nd_u32.tif"], []),
    "umcache_square_i32": ("gp.Square_sa", [RASTER / "unary_cached/i32.tif"], []),
    "umcache_square_nd_i32": ("gp.Square_sa", [RASTER / "unary_cached/nd_i32.tif"], []),
    "umcache_square_i32_positive_max": ("gp.Square_sa", [RASTER / "unary_cached/i32_positive_max.tif"], []),
    "umcache_square_i32_negative_min": ("gp.Square_sa", [RASTER / "unary_cached/i32_negative_min.tif"], []),
    "umcache_square_i32_negative_max": ("gp.Square_sa", [RASTER / "unary_cached/i32_negative_max.tif"], []),
    "umcache_square_i32_ndmin": ("gp.Square_sa", [RASTER / "unary_cached/i32_ndmin.tif"], []),
    "umcache_square_u16": ("gp.Square_sa", [RASTER / "unary_cached/u16.tif"], []),
    "umcache_square_nd_u16": ("gp.Square_sa", [RASTER / "unary_cached/nd_u16.tif"], []),
    "umcache_square_i16_cross3": ("gp.Square_sa", [RASTER / "unary_cached/i16_cross3.tif"], []),
    "umcache_square_i16_cross1": ("gp.Square_sa", [RASTER / "unary_cached/i16_cross1.tif"], []),
    "umcache_square_i16_neg1to3": ("gp.Square_sa", [RASTER / "unary_cached/i16_neg1to3.tif"], []),
    "umcache_square_u32_max": ("gp.Square_sa", [RASTER / "unary_cached/u32_max.tif"], []),
    "umcache_square_constant1": ("gp.Square_sa", [RASTER / "unary_cached/constant1.tif"], []),
    "umcache_abs_symmetric": ("gp.Abs_sa", [RASTER / "unary_cached/symmetric.tif"], []),
    "umcache_negate_symmetric": ("gp.Negate_sa", [RASTER / "unary_cached/symmetric.tif"], []),
    "umcache_square_symmetric": ("gp.Square_sa", [RASTER / "unary_cached/symmetric.tif"], []),
    **{f"cprmulti_{name}":
       ("management.CopyRaster", [RASTER / f"copy_multicache/{name}.tif"], [])
       for name in ["first_full","second_full","split_extrema","first_full_second_extrema",
                    "first_extrema_second_full","second_missing_extrema","domain_only","single_quotes"]},
    **{f"ummulti_{op.lower()}_f32_first_full":
       (f"gp.{op}_sa", [RASTER / "copy_multicache/first_full.tif"], [])
       for op in ["Abs","Negate","Square"]},
    **{f"ummulti_{op.lower()}_{name}":
       (f"gp.{op}_sa", [RASTER / f"copy_multicache/{name}_f64.tif"], [])
       for op in ["Abs","Negate","Square"] for name in ["first_full","second_full","single_quotes"]},
    **{f"cprcache_{name}":
       ("management.CopyRaster", [RASTER / f"copy_cache/{name}.tif"], [])
       for name in ["minmax","meanstd","stale","all","missing_minimum","missing_maximum",
                    "missing_mean","missing_stddev","missing_count","extra","missing_mean_extra"]},
    **{f"cprfrac_{name}":
       ("management.CopyRaster", [RASTER / f"copy_fraction/{src}.tif"], ["","",nd,"","",typ])
       for name,src,nd,typ in [
           ("pixels_i16","pixel_ties","","16_BIT_SIGNED"),
           ("pixels_u8","pixel_ties","127","8_BIT_UNSIGNED"),
           ("pixels_collision","pixel_ties","1.5","16_BIT_SIGNED"),
           ("pixels_quarters","pixel_quarters","","16_BIT_SIGNED"),
           ("payload_tie","payload_tie","3.4e38","16_BIT_SIGNED")]},
    **{f"cprfrac_i16_arg_{marker}":
       ("management.CopyRaster", [RASTER / "composite_matrix/nd_f32.tif"], ["","",str(marker),"","","16_BIT_SIGNED"])
       for marker in [-9999.25,-9999.5,-9999.75,0.5,1.5,-0.5,-1.5]},
    **{f"cprfrac_u8_arg_{marker}":
       ("management.CopyRaster", [RASTER / "composite_matrix/nd_u8.tif"], ["","",str(marker),"","","8_BIT_UNSIGNED"])
       for marker in [126.25,126.5,126.75,127.5,128.5,255.5,300.25,300.5,300.75,-0.5]},
    **{f"cprfrac_i16_{mode}_{marker}":
       ("management.CopyRaster", [RASTER / f"copy_fraction/source_{marker}.tif"], ["","",str(marker) if mode=="equal" else "","","","16_BIT_SIGNED"])
       for marker in [-1.5,-1.75,1.5,1.75,0.5] for mode in ["default","equal"]},
    **{f"cprfrac_u8_cached_{marker}":
       ("management.CopyRaster", [RASTER / "copy_fraction/cached_u8.tif"], ["","",str(marker),"","","8_BIT_UNSIGNED"])
       for marker in [126.75,127.25]},
    **{f"cprfrac_u8_edge_{marker}":
       ("management.CopyRaster", [RASTER / "composite_matrix/nd_u8.tif"], ["","",str(marker),"","","8_BIT_UNSIGNED"])
       for marker in [255.25,-0.25]},
    **{f"cprfrac_i16_edge_{marker}":
       ("management.CopyRaster", [RASTER / "composite_matrix/nd_f32.tif"], ["","",str(marker),"","","16_BIT_SIGNED"])
       for marker in [-32768.25,-32768.5,-32768.75,32767.25,32767.5,32767.75]},
    **{f"cprfrac_u8_zero_{name}":
       ("management.CopyRaster", [RASTER / "composite_matrix/nd_u8.tif"], ["","",value,"","","8_BIT_UNSIGNED"])
       for name,value in [("negative","-0.0"),("positive","0.0")]},
    **{f"cpru32_{marker}_{cache}":
       ("management.CopyRaster", [RASTER / f"copy_u32/nd_{marker}_{cache}.tif"], [])
       for marker in [0,1,2147483647,2147483648,2147483649,4294967294,4294967295]
       for cache in ["cached", "uncached"]},
    **{f"cpru32_{cache}_{option}":
       ("management.CopyRaster", [RASTER / f"copy_u32/nd_2147483648_{cache}.tif"], args)
       for cache in ["cached", "uncached"]
       for option,args in [
           ("same",["","","","","","32_BIT_UNSIGNED"]),
           ("f64",["","","","","","64_BIT"]),
           ("f32",["","","","","","32_BIT_FLOAT"]),
           ("i32",["","","","","","32_BIT_SIGNED"]),
           ("i16",["","","","","","16_BIT_SIGNED"]),
           ("override_low",["","","1000000000"]),
           ("override_high",["","","4294967294"]),
           ("equal",["","","2147483648"])]},
    **{f"cpru32_next_{option}":
       ("management.CopyRaster", [RASTER / "copy_u32/nd_2147483649_uncached.tif"], ["","","","","",typ])
       for option,typ in [("f64","64_BIT"),("i32","32_BIT_SIGNED")]},
    **{f"cpru32_{profile}":
       ("management.CopyRaster", [RASTER / f"copy_u32/{profile}.tif"], [])
       for profile in ["neg_float_edge","neg_near_min","positive_edge"]},
    **{f"cprtag_{typ}":
       ("management.CopyRaster", [RASTER / f"composite_matrix/nd_{typ}.tif"], [])
       for typ in ["u8","i8","u16","i16","u32","i32","f32","f64"]},
    **{f"cprnd_{name}":
       ("management.CopyRaster", [RASTER / f"composite_matrix/{src}.tif"], ["","",nd,"","",typ])
       for name,src,nd,typ in [
           ("u8_big","nd_u8","300","8_BIT_UNSIGNED"),
           ("u8_fraction","nd_u8","300.5","8_BIT_UNSIGNED"),
           ("i16_big","nd_f32","3.4e38","16_BIT_SIGNED"),
           ("i16_double","nd_f64","-1.7976931348623157e308","16_BIT_SIGNED"),
           ("i16_default","nd_f32","","16_BIT_SIGNED")]},
    **{f"cprstats_{profile}_{cache}":
       ("management.CopyRaster", [RASTER / f"copy_statistics/{profile}_{cache}.tif"], [])
       for profile in ["large", "small", "multi", "later", "unused", "large_valid"]
       for cache in ["cached", "uncached"]},
    **{f"cprstats_small_{cache}_{option}":
       ("management.CopyRaster", [RASTER / f"copy_statistics/small_{cache}.tif"], args)
       for cache in ["cached", "uncached"]
       for option, args in [
           ("f64", ["", "", "", "", "", "64_BIT"]),
           ("i16", ["", "", "", "", "", "16_BIT_SIGNED"]),
           ("override", ["", "", "-9999"]),
           ("same", ["", "", "", "", "", "32_BIT_FLOAT"])]},
    "cprstats_small_cached_equal": ("management.CopyRaster", [RASTER / "copy_statistics/small_cached.tif"], ["", "", "-1.25"]),
    "cprstats_small_cached_equal_type": ("management.CopyRaster", [RASTER / "copy_statistics/small_cached.tif"], ["", "", "-1.25", "", "", "32_BIT_FLOAT"]),
    "cpr_dem": ("management.CopyRaster", [RASTER / "dem.tif"], []),
    "cpr_dem_nodata": ("management.CopyRaster", [RASTER / "dem.tif"], ["", "", "-1", "", "", "32_BIT_FLOAT"]),
    "cpr_u8_float": ("management.CopyRaster", [RASTER / "landuse_u8.tif"], ["", "", "", "", "", "32_BIT_FLOAT"]),
    "cpr_u8_nodata": ("management.CopyRaster", [RASTER / "landuse_u8.tif"], ["", "", "7"]),
    "cpr_dem_nodata_in": ("management.CopyRaster", [RASTER / "dem_nodata.tif"], []),
    "cpr_i16_u8": ("management.CopyRaster", [RASTER / "dem_int16_packbits.tif"], ["", "", "", "", "", "8_BIT_UNSIGNED"]),
    "cpr_i16_64": ("management.CopyRaster", [RASTER / "dem_int16_packbits.tif"], ["", "", "", "", "", "64_BIT"]),
    "mos_zones_last": ("management.MosaicToNewRaster", [f"{RASTER / 'zones.tif'};{RASTER / 'zones_shifted.tif'}"], ["", "8_BIT_UNSIGNED", "", "1", "LAST", "FIRST"]),
    "mos_zones_first": ("management.MosaicToNewRaster", [f"{RASTER / 'zones_shifted.tif'};{RASTER / 'zones.tif'}"], ["", "8_BIT_UNSIGNED", "", "1", "FIRST", "FIRST"]),
    "mos_zones_max": ("management.MosaicToNewRaster", [f"{RASTER / 'zones_shifted.tif'};{RASTER / 'zones.tif'}"], ["", "8_BIT_UNSIGNED", "", "1", "MAXIMUM", "FIRST"]),
    "mos_dem_mean": ("management.MosaicToNewRaster", [f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'}"], ["", "32_BIT_FLOAT", "", "1", "MEAN", "FIRST"]),
    "mos_zones_float": ("management.MosaicToNewRaster", [f"{RASTER / 'zones.tif'};{RASTER / 'zones_shifted.tif'}"], ["", "32_BIT_FLOAT", "", "1", "LAST", "FIRST"]),
    "ebpoly_in": ("gp.ExtractByPolygon_sa", [RASTER / "dem.tif", "500205 5000205;501105 5000305;500805 5001005;500305 5000905"], ["INSIDE"]),
    "ebpoly_out": ("gp.ExtractByPolygon_sa", [RASTER / "dem.tif", "500205 5000205;501105 5000305;500805 5001005;500305 5000905"], ["OUTSIDE"]),
    "ebpoly_int": ("gp.ExtractByPolygon_sa", [RASTER / "landuse_u8.tif", "500100 5000100;501000 5000300;500600 5001000"], ["INSIDE"]),
    "ebpts_in": ("gp.ExtractByPoints_sa", [RASTER / "dem.tif", "500205 5000205;501105 5000305;500805 5001005;500305 5000905;500300 5000300"], ["INSIDE"]),
    "ebpts_out": ("gp.ExtractByPoints_sa", [RASTER / "dem.tif", "500205 5000205;501105 5000305;500805 5001005;500305 5000905;500300 5000300"], ["OUTSIDE"]),
    "tpi_dem": ("gp.TopographicPositionIndex_sa", [RASTER / "dem.tif"], []),
    "tpi_dem_circle": ("gp.TopographicPositionIndex_sa", [RASTER / "dem.tif"], ["Circle 3 CELL"]),
    "tpi_dem_annulus": ("gp.TopographicPositionIndex_sa", [RASTER / "dem.tif"], ["Annulus 2 4 CELL"]),
    "tpi_dem_rect": ("gp.TopographicPositionIndex_sa", [RASTER / "dem.tif"], ["Rectangle 5 3 CELL"]),
    "tpi_dem_nodata": ("gp.TopographicPositionIndex_sa", [RASTER / "dem_nodata.tif"], []),
    "tpi_u8": ("gp.TopographicPositionIndex_sa", [RASTER / "landuse_u8.tif"], ["Circle 2 CELL"]),
    "test_dem": ("gp.Test_sa", [RASTER / "dem.tif", "VALUE > 220"], []),
    "test_u8": ("gp.Test_sa", [RASTER / "landuse_u8.tif", "VALUE = 2 OR VALUE = 3"], []),
    "test_nodata": ("gp.Test_sa", [RASTER / "dem_nodata.tif", "VALUE < 250"], []),
    "tophat_dem": ("gp.TopHatTransform_sa", [RASTER / "dem.tif"], []),
    "tophat_dem_valley": ("gp.TopHatTransform_sa", [RASTER / "dem.tif"], ["Rectangle 3 3 CELL", "VALLEY"]),
    "tophat_dem_ridge_rect": ("gp.TopHatTransform_sa", [RASTER / "dem.tif"], ["Rectangle 5 3 CELL", "RIDGE"]),
    "tophat_dem_annulus": ("gp.TopHatTransform_sa", [RASTER / "dem.tif"], ["Annulus 1 3 CELL", "VALLEY"]),
    "tophat_nodata": ("gp.TopHatTransform_sa", [RASTER / "dem_nodata.tif"], ["Circle 2 CELL", "RIDGE"]),
    "tophat_u8": ("gp.TopHatTransform_sa", [RASTER / "landuse_u8.tif"], ["Circle 2 CELL", "VALLEY"]),
    "trend_wells_1": ("gp.Trend_sa", [DATA / "wells.shp", "DEPTH"], [10, 1]),
    "trend_wells_2": ("gp.Trend_sa", [DATA / "wells.shp", "DEPTH"], [10, 2]),
    "trend_wells_default": ("gp.Trend_sa", [DATA / "wells.shp", "DEPTH"], []),
    "rk_dem_1": ("gp.Rank_sa", [1, f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'};{RASTER / 'dem_point.tif'}"], []),
    "rk_dem_2": ("gp.Rank_sa", [2, f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'};{RASTER / 'dem_point.tif'}"], []),
    "rk_dem_3": ("gp.Rank_sa", [3, f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'};{RASTER / 'dem_point.tif'}"], []),
    "rk_i16_1": ("gp.Rank_sa", [1, f"{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'dem_int16_packbits.tif'}"], []),
    "pop_dem_1": ("gp.Popularity_sa", [1, f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'};{RASTER / 'dem_point.tif'}"], []),
    "pop_dem_2": ("gp.Popularity_sa", [2, f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'};{RASTER / 'dem_point.tif'}"], []),
    "pop_dem_3": ("gp.Popularity_sa", [3, f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'};{RASTER / 'dem_point.tif'}"], []),
    "pop_i16_1": ("gp.Popularity_sa", [1, f"{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'dem_int16_packbits.tif'}"], []),
    "rk_mix_1": ("gp.Rank_sa", [1, f"{RASTER / 'landuse_u8.tif'};{RASTER / 'patches_i16.tif'};{RASTER / 'nibble_mask_i16.tif'}"], []),
    "rk_mix_2": ("gp.Rank_sa", [2, f"{RASTER / 'landuse_u8.tif'};{RASTER / 'patches_i16.tif'};{RASTER / 'nibble_mask_i16.tif'}"], []),
    "rk_mix_3": ("gp.Rank_sa", [3, f"{RASTER / 'landuse_u8.tif'};{RASTER / 'patches_i16.tif'};{RASTER / 'nibble_mask_i16.tif'}"], []),
    "pop_mix_1": ("gp.Popularity_sa", [1, f"{RASTER / 'landuse_u8.tif'};{RASTER / 'patches_i16.tif'};{RASTER / 'nibble_mask_i16.tif'}"], []),
    "pop_mix_2": ("gp.Popularity_sa", [2, f"{RASTER / 'landuse_u8.tif'};{RASTER / 'patches_i16.tif'};{RASTER / 'nibble_mask_i16.tif'}"], []),
    "pop_mix_3": ("gp.Popularity_sa", [3, f"{RASTER / 'landuse_u8.tif'};{RASTER / 'patches_i16.tif'};{RASTER / 'nibble_mask_i16.tif'}"], []),
    "inl_u8_const": ("gp.InList_sa", [RASTER / "landuse_u8.tif", "1;2"], []),
    "inl_u8_one": ("gp.InList_sa", [RASTER / "landuse_u8.tif", "2"], []),
    "inl_u8_raster": ("gp.InList_sa", [RASTER / "landuse_u8.tif", str(RASTER / "patches_i16.tif")], []),
    "inl_u8_mixed": ("gp.InList_sa", [RASTER / "landuse_u8.tif", f"{RASTER / 'patches_i16.tif'};3"], []),
    "inl_patches": ("gp.InList_sa", [RASTER / "patches_i16.tif", "2;7"], []),
    "inl_dem": ("gp.InList_sa", [RASTER / "dem.tif", "200;250.5"], []),
    # Create Normal Raster: random values, compared statistically (cnr_ in arcgis_reference.rs).
    "cnr_default": ("gp.CreateNormalRaster_sa", [], []),
    "cnr_cell5": ("gp.CreateNormalRaster_sa", [], [5]),
    "cnr_extent": ("gp.CreateNormalRaster_sa", [], [10, "0 0 1000 500"]),
    "cnr_odd": ("gp.CreateNormalRaster_sa", [], [7, "1 2 50 33"]),
    "cnr_small": ("gp.CreateNormalRaster_sa", [], [0.5, "0 0 3 2"]),
    "cc_int5": ("gp.CreateConstantRaster_sa", [5], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_float": ("gp.CreateConstantRaster_sa", [2.5], ["FLOAT", 10, "500000 5000000 500100 5000050"]),
    "cc_neg7": ("gp.CreateConstantRaster_sa", [-7], ["INTEGER", 5, "500003 5000002 500103 5000052"]),
    "cc_big": ("gp.CreateConstantRaster_sa", [70000], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_frac": ("gp.CreateConstantRaster_sa", [3.7], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_zero": ("gp.CreateConstantRaster_sa", [0], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_one": ("gp.CreateConstantRaster_sa", [1], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_u300": ("gp.CreateConstantRaster_sa", [300], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_neg200": ("gp.CreateConstantRaster_sa", [-200], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_odd7": ("gp.CreateConstantRaster_sa", [5], ["INTEGER", 7, "500000 5000000 500100 5000050"]),
    "cc_odd6": ("gp.CreateConstantRaster_sa", [5], ["INTEGER", 6, "500000 5000000 500100 5000050"]),
    "cc_v200": ("gp.CreateConstantRaster_sa", [200], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "cc_neg1": ("gp.CreateConstantRaster_sa", [-1], ["INTEGER", 10, "500000 5000000 500100 5000050"]),
    "sw_parcels_wells": ("analysis.SummarizeWithin", [DATA / "parcels.shp", DATA / "wells.shp"], ["KEEP_ALL", "DEPTH SUM;DEPTH MEAN"]),
    "sw_parcels_wells_only": ("analysis.SummarizeWithin", [DATA / "parcels.shp", DATA / "wells.shp"], ["ONLY_INTERSECTING"]),
    "sn_parcels_wells": ("analysis.SummarizeNearby", [DATA / "parcels.shp", DATA / "wells.shp"], ["STRAIGHT_LINE", "100", "Meters"]),
    "sn_parcels_wells_close": ("analysis.SummarizeNearby", [DATA / "parcels.shp", DATA / "wells.shp"], ["STRAIGHT_LINE", "10", "Meters"]),
    "ascimp_small_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/small.asc"], ["INTEGER"]),
    "ascimp_small_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/small.asc"], ["FLOAT"]),
    "ascimp_signed_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/signed.asc"], ["INTEGER"]),
    "ascimp_signed_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/signed.asc"], ["FLOAT"]),
    "ascimp_float_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/float.asc"], ["INTEGER"]),
    "ascimp_float_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/float.asc"], ["FLOAT"]),
    "ascimp_wide_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/wide.asc"], ["INTEGER"]),
    "ascimp_wide_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/wide.asc"], ["FLOAT"]),
    "ascimp_center_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/center.asc"], ["INTEGER"]),
    "ascimp_center_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/center.asc"], ["FLOAT"]),
    "ascimp_no_nd_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/no_nd.asc"], ["INTEGER"]),
    "ascimp_no_nd_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/no_nd.asc"], ["FLOAT"]),
    "ascimp_short_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/short.asc"], ["INTEGER"]),
    "ascimp_short_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/short.asc"], ["FLOAT"]),
    "ascimp_extra_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/extra.asc"], ["INTEGER"]),
    "ascimp_extra_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/extra.asc"], ["FLOAT"]),
    "ascimp_near_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/near.asc"], ["INTEGER"]),
    "ascimp_near_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/near.asc"], ["FLOAT"]),
    "ascimp_near_header_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/near_header.asc"], ["INTEGER"]),
    "ascimp_near_header_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/near_header.asc"], ["FLOAT"]),
    "ascimp_all_nd_integer": ("conversion.ASCIIToRaster", [RASTER / "grid_import/all_nd.asc"], ["INTEGER"]),
    "ascimp_all_nd_float": ("conversion.ASCIIToRaster", [RASTER / "grid_import/all_nd.asc"], ["FLOAT"]),
    "ascimp_default": ("conversion.ASCIIToRaster", [RASTER / "grid_import/small.asc"], []),
    "ascimp_existing_dem": ("conversion.ASCIIToRaster", [OUT / "r2a_dem.asc"], ["FLOAT"]),
    "fltimp_missing_header": ("conversion.FloatToRaster", [RASTER / "grid_import/missing_header.flt"], []),
    "fltimp_little": ("conversion.FloatToRaster", [RASTER / "grid_import/little.flt"], []),
    "fltimp_big": ("conversion.FloatToRaster", [RASTER / "grid_import/big.flt"], []),
    "fltimp_float_no_byteorder": ("conversion.FloatToRaster", [RASTER / "grid_import/float_no_byteorder.flt"], []),
    "fltimp_float_no_nd": ("conversion.FloatToRaster", [RASTER / "grid_import/float_no_nd.flt"], []),
    "fltimp_float_center": ("conversion.FloatToRaster", [RASTER / "grid_import/float_center.flt"], []),
    "fltimp_float_near_header": ("conversion.FloatToRaster", [RASTER / "grid_import/float_near_header.flt"], []),
    "fltimp_float_short": ("conversion.FloatToRaster", [RASTER / "grid_import/float_short.flt"], []),
    "fltimp_float_extra": ("conversion.FloatToRaster", [RASTER / "grid_import/float_extra.flt"], []),
    "fltimp_all_nd": ("conversion.FloatToRaster", [RASTER / "grid_import/all_nd.flt"], []),
    "fltimp_existing_dem": ("conversion.FloatToRaster", [OUT / "r2f_dem.flt"], []),
    "cbands_oor_u8_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/oor_u8.tif'}"], []),
    "cbands_oor_u8_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/oor_u8.tif'}"], []),
    "cbands_oor_i8_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/oor_i8.tif'}"], []),
    "cbands_oor_i8_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/oor_i8.tif'}"], []),
    "cbands_oor_u16_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/oor_u16.tif'}"], []),
    "cbands_oor_u16_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/oor_u16.tif'}"], []),
    "cbands_oor300_u8_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/oor300_u8.tif'}"], []),
    "cbands_oor300_u8_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/oor300_u8.tif'}"], []),
    "cbands_oorneg_u8_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/oorneg_u8.tif'}"], []),
    "cbands_oorneg_u8_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/oorneg_u8.tif'}"], []),
    "cbands_nd_fallback_combo_i8_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i8.tif'};{RASTER / 'composite_matrix/nd_u16.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_combo_i16_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i16.tif'};{RASTER / 'composite_matrix/nd_u16.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_combo_i8_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i8.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_fallback_combo_i32_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i32.tif'};{RASTER / 'composite_matrix/nd_f32.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_combo_u32_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u32.tif'};{RASTER / 'composite_matrix/nd_i8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_first_none_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/nd_f64.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_fallback_first_none_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_fallback_first_u8_last_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/nd_f64.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_fallback_first_f64_last_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_f64.tif'};{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_padding_u8nd_shifted": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/shifted.tif'}"], []),
    "cbands_nd_padding_f64nd_shifted": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_f64.tif'};{RASTER / 'composite_matrix/shifted.tif'}"], []),
    "cbands_nd_padding_u8_shiftednd": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/shifted_nd_u8.tif'}"], []),
    "cbands_nd_padding_middle_nd": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/shifted_nd_u8.tif'};{RASTER / 'composite_matrix/nd_f64.tif'}"], []),
    "cbands_nd_fallback_f64_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_f64.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_f32_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_f32.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_i8_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i8.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_nd_fallback_u8_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_fallback_u32_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u32.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_i32_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i32.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_fallback_i16_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i16.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_u16_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u16.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_fallback_i8_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_nd_fallback_u8_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_nd_last_f64_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/nd_u32.tif'}"], []),
    "cbands_nd_both_u16_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u16.tif'};{RASTER / 'composite_matrix/nd_i16.tif'}"], []),
    "cbands_nd_both_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_i8.tif'};{RASTER / 'composite_matrix/nd_i8.tif'}"], []),
    "cbands_nd_both_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_u32.tif'};{RASTER / 'composite_matrix/nd_u32.tif'}"], []),
    "cbands_nd_both_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/nd_f64.tif'};{RASTER / 'composite_matrix/nd_f64.tif'}"], []),
    "cbands_nd_last_i32_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/nd_u32.tif'}"], []),
    "cbands_matrix_u8_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_u8_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_u8_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_u8_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_u8_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_u8_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_u8_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_u8_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_matrix_i8_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_i8_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_i8_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_i8_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_i8_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_i8_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_i8_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_i8_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i8.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_matrix_u16_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_u16_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_u16_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_u16_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_u16_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_u16_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_u16_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_u16_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u16.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_matrix_i16_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_i16_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_i16_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_i16_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_i16_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_i16_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_i16_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_i16_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_matrix_u32_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_u32_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_u32_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_u32_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_u32_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_u32_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_u32_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_u32_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_matrix_i32_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_i32_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_i32_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_i32_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_i32_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_i32_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_i32_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_i32_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_matrix_f32_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_f32_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_f32_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_f32_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_f32_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_f32_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_f32_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_f32_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_matrix_f64_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_matrix_f64_i8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/i8.tif'}"], []),
    "cbands_matrix_f64_u16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/u16.tif'}"], []),
    "cbands_matrix_f64_i16": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/i16.tif'}"], []),
    "cbands_matrix_f64_u32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/u32.tif'}"], []),
    "cbands_matrix_f64_i32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/i32.tif'}"], []),
    "cbands_matrix_f64_f32": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/f32.tif'}"], []),
    "cbands_matrix_f64_f64": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f64.tif'};{RASTER / 'composite_matrix/f64.tif'}"], []),
    "cbands_pad_u8_shifted": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/u8.tif'};{RASTER / 'composite_matrix/shifted.tif'}"], []),
    "cbands_pad_shifted_u8": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/shifted.tif'};{RASTER / 'composite_matrix/u8.tif'}"], []),
    "cbands_pad_f32_shifted": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/f32.tif'};{RASTER / 'composite_matrix/shifted.tif'}"], []),
    "cbands_pad_i16_shifted": ("management.CompositeBands", [f"{RASTER / 'composite_matrix/i16.tif'};{RASTER / 'composite_matrix/shifted.tif'}"], []),
    "cbands_dem": ("management.CompositeBands", [f"{RASTER / 'dem.tif'}"], []),
    "cbands_dem_twice": ("management.CompositeBands", [f"{RASTER / 'dem.tif'};{RASTER / 'dem.tif'}"], []),
    "cbands_float_nodata": ("management.CompositeBands", [f"{RASTER / 'dem.tif'};{RASTER / 'dem_nodata.tif'}"], []),
    "cbands_nodata_float": ("management.CompositeBands", [f"{RASTER / 'dem_nodata.tif'};{RASTER / 'dem.tif'}"], []),
    "cbands_int_float": ("management.CompositeBands", [f"{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'dem.tif'}"], []),
    "cbands_float_int": ("management.CompositeBands", [f"{RASTER / 'dem.tif'};{RASTER / 'dem_int16_packbits.tif'}"], []),
    "cbands_int_mix": ("management.CompositeBands", [f"{RASTER / 'landuse_u8.tif'};{RASTER / 'patches_i16.tif'}"], []),
    "cbands_rgb": ("management.CompositeBands", [f"{RASTER / 'rgb_pixel_u8.tif'}"], []),
    "cbands_rgb_twice": ("management.CompositeBands", [f"{RASTER / 'rgb_pixel_u8.tif'};{RASTER / 'rgb_pixel_u8.tif'}"], []),
    "cbands_rgb_u16": ("management.CompositeBands", [f"{RASTER / 'rgb_pixel_u8.tif'};{RASTER / 'rgb_planar_u16.tif'}"], []),
    "cbands_shifted": ("management.CompositeBands", [f"{RASTER / 'dem.tif'};{RASTER / 'zones_shifted.tif'}"], []),
    "cbands_different_cell": ("management.CompositeBands", [f"{RASTER / 'dem.tif'};{RASTER / 'landuse_u8.tif'}"], []),
    "cbands_u16_i16": ("management.CompositeBands", [f"{RASTER / 'rgb_planar_u16.tif'};{RASTER / 'dem_int16_packbits.tif'}"], []),
    "cbands_i16_u16": ("management.CompositeBands", [f"{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'rgb_planar_u16.tif'}"], []),
    "cbands_f64_f32": ("management.CompositeBands", [f"{RASTER / 'dem_f64.tif'};{RASTER / 'dem.tif'}"], []),
    "cbands_f32_f64": ("management.CompositeBands", [f"{RASTER / 'dem.tif'};{RASTER / 'dem_f64.tif'}"], []),
    "cbands_i32_u16": ("management.CompositeBands", [f"{RASTER / 'dem_bigtiff_i32.tif'};{RASTER / 'rgb_planar_u16.tif'}"], []),
    "cbands_i32_float": ("management.CompositeBands", [f"{RASTER / 'dem_bigtiff_i32.tif'};{RASTER / 'dem.tif'}"], []),
    "gb_parcels_sq_miter": ("analysis.GraphicBuffer", [DATA / "parcels.shp"], ["20 Meters", "SQUARE", "MITER"]),
    "gb_roads_sq_miter": ("analysis.GraphicBuffer", [DATA / "roads.shp"], ["20 Meters", "SQUARE", "MITER"]),
    "gb_roads_butt_bevel": ("analysis.GraphicBuffer", [DATA / "roads.shp"], ["20 Meters", "BUTT", "BEVEL"]),
    "gb_roads_round": ("analysis.GraphicBuffer", [DATA / "roads.shp"], ["20 Meters", "ROUND", "ROUND"]),
    "gb_wells_sq": ("analysis.GraphicBuffer", [DATA / "wells.shp"], ["15 Meters", "SQUARE", "MITER"]),
    "gb_wells_round": ("analysis.GraphicBuffer", [DATA / "wells.shp"], ["15 Meters", "ROUND", "ROUND"]),
    "gb_clip_sq_miter": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["25 Meters", "SQUARE", "MITER"]),
    "gb_clip_butt_bevel": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["25 Meters", "BUTT", "BEVEL"]),
    "gb_clip_round": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["25 Meters", "ROUND", "ROUND"]),
    "gb_streams_sq_bevel": ("analysis.GraphicBuffer", [DATA / "streams.shp"], ["30 Meters", "SQUARE", "BEVEL"]),
    "gb_field_numeric": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["DIST", "SQUARE", "MITER"]),
    "gb_field_lowercase": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["dist", "SQUARE", "MITER"]),
    "gb_field_text_units": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["TEXT_DIST", "SQUARE", "MITER"]),
    "gb_field_zero": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["ZERO", "SQUARE", "MITER"]),
    "gb_field_negative": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["NEG", "SQUARE", "MITER"]),
    "gb_field_mixed": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["MIX", "SQUARE", "MITER"]),
    "gb_field_nullable": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["NULL_DIST", "SQUARE", "MITER"]),
    "gb_field_text_zero": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["TEXT_MIX", "SQUARE", "MITER"]),
    "gb_field_text_negative": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["TEXT_NEG", "SQUARE", "MITER"]),
    "gb_field_missing": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["Missing", "SQUARE", "MITER"]),
    "gb_field_bad_constant_unit": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["10 Blargs", "SQUARE", "MITER"]),
    "gb_field_unit_feet": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["30 Feet", "SQUARE", "MITER"]),
    "gb_field_unit_feet_int": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["30 FeetInt", "SQUARE", "MITER"]),
    "gb_field_unit_feet_us": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["30 FeetUS", "SQUARE", "MITER"]),
    "gb_field_unit_miles": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["1 Miles", "SQUARE", "MITER"]),
    "gb_field_unit_nautical": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["1 NauticalMiles", "SQUARE", "MITER"]),
    "gb_field_unit_inches": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["1 Inches", "SQUARE", "MITER"]),
    "gb_field_unit_yards": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_fields.shp"], ["1 Yards", "SQUARE", "MITER"]),
    "gb_field_poly_negative": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_poly_fields.shp"], ["DIST", "SQUARE", "MITER"]),
    "gb_field_reserved": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_reserved.shp"], ["BUFF_DIST", "SQUARE", "MITER"]),
    "gb_more_wells_butt": ("analysis.GraphicBuffer", [DATA / "wells.shp"], ["15 Meters", "BUTT", "MITER"]),
    "gb_more_wells_round30": ("analysis.GraphicBuffer", [DATA / "wells.shp"], ["30 Meters", "ROUND", "ROUND"]),
    "gb_more_wells_round1000": ("analysis.GraphicBuffer", [DATA / "wells.shp"], ["1000 Meters", "ROUND", "ROUND"]),
    "gb_more_roads_miter1": ("analysis.GraphicBuffer", [DATA / "roads.shp"], ["20 Meters", "SQUARE", "MITER", 1]),
    "gb_more_roads_miter2": ("analysis.GraphicBuffer", [DATA / "roads.shp"], ["20 Meters", "SQUARE", "MITER", 2]),
    "gb_more_clip_miter2": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["25 Meters", "SQUARE", "MITER", 2]),
    "gb_more_clip_negative": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["-20 Meters", "SQUARE", "MITER"]),
    "gb_more_clip_zero": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["0 Meters", "SQUARE", "MITER"]),
    "gb_more_wells_negative": ("analysis.GraphicBuffer", [DATA / "wells.shp"], ["-5 Meters", "SQUARE", "MITER"]),
    "gb_more_wells_zero": ("analysis.GraphicBuffer", [DATA / "wells.shp"], ["0 Meters", "SQUARE", "MITER"]),
    "gb_more_reserved": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_reserved.shp"], ["15 Meters", "SQUARE", "MITER"]),
    "gb_more_angles_miter125": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_angles.shp"], ["20 Meters", "SQUARE", "MITER", 1.25]),
    "gb_more_angles_miter2": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_angles.shp"], ["20 Meters", "SQUARE", "MITER", 2]),
    "gb_more_angles_miter10": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_angles.shp"], ["20 Meters", "SQUARE", "MITER", 10]),
    "gb_more_roads_miter125": ("analysis.GraphicBuffer", [DATA / "roads.shp"], ["20 Meters", "SQUARE", "MITER", 1.25]),
    "gb_more_angles_bevel": ("analysis.GraphicBuffer", [DATA / "graphic_buffer_angles.shp"], ["20 Meters", "SQUARE", "BEVEL"]),
    "gb_more_clip_negative_miter1": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["-20 Meters", "SQUARE", "MITER", 1]),
    "gb_more_clip_negative_miter2": ("analysis.GraphicBuffer", [DATA / "clip_areas.shp"], ["-20 Meters", "SQUARE", "MITER", 2]),
    "apo_parcels_clip": ("analysis.ApportionPolygon", [DATA / "parcels.shp", "VALUE SUM;PARCEL_ID SUM", DATA / "clip_areas.shp"], ["AREA"]),
    "apo_clip_parcels": ("analysis.ApportionPolygon", [DATA / "clip_areas.shp", "AREA_ID SUM", DATA / "parcels.shp"], ["AREA"]),
    "apo_parcels_squares": ("analysis.ApportionPolygon", [DATA / "parcels.shp", "VALUE SUM", DATA / "squares.shp"], ["AREA"]),
    "ebd_src_one": ("gp.EucBackDirection_sa", [RASTER / "cost_src_one.tif"], []),
    "ebd_src_i16": ("gp.EucBackDirection_sa", [RASTER / "cost_src_i16.tif"], []),
    "ebd_src_max": ("gp.EucBackDirection_sa", [RASTER / "cost_src_i16.tif"], ["#", 400]),
    "ebd_landuse": ("gp.EucBackDirection_sa", [RASTER / "landuse_u8.tif"], []),
    "dacc_src_one": ("gp.DistanceAccumulation_sa", [RASTER / "cost_src_one.tif"], []),
    "dacc_src_i16": ("gp.DistanceAccumulation_sa", [RASTER / "cost_src_i16.tif"], []),
    "dacc_landuse": ("gp.DistanceAccumulation_sa", [RASTER / "landuse_u8.tif"], []),
    "dalloc_src_one": ("gp.DistanceAllocation_sa", [RASTER / "cost_src_one.tif"], []),
    "dalloc_src_i16": ("gp.DistanceAllocation_sa", [RASTER / "cost_src_i16.tif"], []),
    "dalloc_landuse": ("gp.DistanceAllocation_sa", [RASTER / "landuse_u8.tif"], []),
    "ff_dem": ("gp.FocalFlow_sa", [RASTER / "dem.tif"], []),
    "ff_dem_2": ("gp.FocalFlow_sa", [RASTER / "dem.tif"], [2]),
    "ff_dem_nodata": ("gp.FocalFlow_sa", [RASTER / "dem_nodata.tif"], [0.5]),
    "ff_hydro": ("gp.FocalFlow_sa", [RASTER / "hydro.tif"], []),
    "ff_u8": ("gp.FocalFlow_sa", [RASTER / "landuse_u8.tif"], [1]),
    "spp_30": ("gp.SnapPourPoint_sa", [DATA / "pour_pts.shp", OUT / "flowacc_hydro.tif"], [30]),
    "spp_100": ("gp.SnapPourPoint_sa", [DATA / "pour_pts.shp", OUT / "flowacc_hydro.tif"], [100]),
    "spp_100_field": ("gp.SnapPourPoint_sa", [DATA / "pour_pts.shp", OUT / "flowacc_hydro.tif"], [100, "PP_ID"]),
    "spp_0": ("gp.SnapPourPoint_sa", [DATA / "pour_pts.shp", OUT / "flowacc_hydro.tif"], [0]),
    "spp_300_int": ("gp.SnapPourPoint_sa", [DATA / "pour_pts.shp", OUT / "flowacc_hydro_int.tif"], [300, "PP_ID"]),
    "smp_dem": ("gp.Sample_sa", [RASTER / "dem.tif", DATA / "wells.shp"], ["NEAREST"]),
    "smp_dem_bil": ("gp.Sample_sa", [RASTER / "dem.tif", DATA / "wells.shp"], ["BILINEAR"]),
    "smp_u8": ("gp.Sample_sa", [RASTER / "landuse_u8.tif", DATA / "wells.shp"], ["NEAREST"]),
    "smp_multi": ("gp.Sample_sa", [f"{RASTER / 'dem.tif'};{RASTER / 'landuse_u8.tif'};{RASTER / 'dem_int16_packbits.tif'}", DATA / "wells.shp"], ["NEAREST"]),
    "smp_nodata_bil": ("gp.Sample_sa", [RASTER / "dem_nodata.tif", DATA / "wells.shp"], ["BILINEAR"]),
    "smp_f64_id": ("gp.Sample_sa", [RASTER / "dem_f64.tif", DATA / "wells.shp"], ["NEAREST", "WELL_ID"]),
    "smp_hydro_parcels": ("gp.Sample_sa", [RASTER / "hydro.tif", DATA / "pour_pts.shp"], ["NEAREST"]),
    "emv_nearest": ("gp.ExtractMultiValuesToPoints_sa", [DATA / "wells.shp", f"{RASTER / 'dem.tif'};{RASTER / 'landuse_u8.tif'};{RASTER / 'dem_int16_packbits.tif'};{RASTER / 'dem_f64.tif'}"], ["NONE"]),
    "emv_bilinear": ("gp.ExtractMultiValuesToPoints_sa", [DATA / "wells.shp", f"{RASTER / 'dem.tif'};{RASTER / 'landuse_u8.tif'};{RASTER / 'dem_nodata.tif'};{RASTER / 'hydro.tif'}"], ["BILINEAR"]),
    "ps_wells_mean": ("gp.PointStatistics_sa", [DATA / "wells.shp", "DEPTH"], [50, "Circle 100 MAP", "MEAN"]),
    "ps_wells_sum": ("gp.PointStatistics_sa", [DATA / "wells.shp", "DEPTH"], [25, "Circle 60 MAP", "SUM"]),
    "ps_wells_max": ("gp.PointStatistics_sa", [DATA / "wells.shp", "DEPTH"], [50, "Circle 4 CELL", "MAXIMUM"]),
    "ps_wells_min": ("gp.PointStatistics_sa", [DATA / "wells.shp", "DEPTH"], [50, "Rectangle 150 100 MAP", "MINIMUM"]),
    "ps_wells_range": ("gp.PointStatistics_sa", [DATA / "wells.shp", "DEPTH"], [40, "Annulus 20 90 MAP", "RANGE"]),
    "ps_wells_std": ("gp.PointStatistics_sa", [DATA / "wells.shp", "DEPTH"], [50, "Circle 120 MAP", "STD"]),
    "ps_pour_mean": ("gp.PointStatistics_sa", [DATA / "pour_pts.shp", "PP_ID"], [100, "Circle 300 MAP", "MEAN"]),
    "ps_wells_rect_cell": ("gp.PointStatistics_sa", [DATA / "wells.shp", "DEPTH"], [30, "Rectangle 3 5 CELL", "MEAN"]),
    **{f"ls_tie_{field.lower()}_{stat.lower()}":
       ("gp.LineStatistics_sa", [DATA / "line_stats_ties.shp", field], [10, 6, stat])
       for field in ["IVAL", "DVAL"]
       for stat in (["LENGTH", "MEAN", "MAXIMUM", "MINIMUM", "RANGE", "VARIETY", "MAJORITY", "MINORITY"]
                    if field == "IVAL" else ["LENGTH", "MEAN", "MAXIMUM", "MINIMUM", "RANGE"])},
    "ls_length": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [20, 60, "LENGTH"]),
    "ls_mean": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [20, 60, "MEAN"]),
    "ls_max": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [25, 80, "MAXIMUM"]),
    "ls_min": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [25, 80, "MINIMUM"]),
    "ls_range": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [30, 100, "RANGE"]),
    "ls_variety": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [30, 100, "VARIETY"]),
    "ls_majority": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [20, 90, "MAJORITY"]),
    "ls_minority": ("gp.LineStatistics_sa", [DATA / "roads.shp", "ROAD_ID"], [20, 90, "MINORITY"]),
    "ls_streams_mean": ("gp.LineStatistics_sa", [DATA / "streams.shp", "STREAM_ID"], [25, 70, "MEAN"]),
    "diff_dem_i16": ("gp.Diff_ia", [RASTER / "dem.tif", RASTER / "dem_int16_packbits.tif"], []),
    "diff_u8_u8": ("gp.Diff_ia", [RASTER / "landuse_u8.tif", RASTER / "landuse_u8.tif"], []),
    "diff_i16_c200": ("gp.Diff_ia", [RASTER / "dem_int16_packbits.tif", "200"], []),
    "diff_u8_c2": ("gp.Diff_ia", [RASTER / "landuse_u8.tif", "2"], []),
    "diff_dem_c": ("gp.Diff_ia", [RASTER / "dem.tif", "200.5"], []),
    "diff_u8_patch": ("gp.Diff_ia", [RASTER / "landuse_u8.tif", RASTER / "patches_i16.tif"], []),
    "diff_patch_u8": ("gp.Diff_ia", [RASTER / "patches_i16.tif", RASTER / "landuse_u8.tif"], []),
    "over_dem_i16": ("gp.Over_ia", [RASTER / "dem.tif", RASTER / "dem_int16_packbits.tif"], []),
    "over_u8_u8": ("gp.Over_ia", [RASTER / "landuse_u8.tif", RASTER / "landuse_u8.tif"], []),
    "over_i16_c200": ("gp.Over_ia", [RASTER / "dem_int16_packbits.tif", "200"], []),
    "over_u8_c2": ("gp.Over_ia", [RASTER / "landuse_u8.tif", "2"], []),
    "over_dem_c": ("gp.Over_ia", [RASTER / "dem.tif", "200.5"], []),
    "over_u8_patch": ("gp.Over_ia", [RASTER / "landuse_u8.tif", RASTER / "patches_i16.tif"], []),
    "over_patch_u8": ("gp.Over_ia", [RASTER / "patches_i16.tif", RASTER / "landuse_u8.tif"], []),
    "cc2_difference_dem_i16": ("gp.ComputeChangeRaster_ia", [RASTER / "dem.tif", RASTER / "dem_int16_packbits.tif"], ["DIFFERENCE"]),
    "cc2_difference_i16_dem": ("gp.ComputeChangeRaster_ia", [RASTER / "dem_int16_packbits.tif", RASTER / "dem.tif"], ["DIFFERENCE"]),
    "cc2_difference_u8_patch": ("gp.ComputeChangeRaster_ia", [RASTER / "landuse_u8.tif", RASTER / "patches_i16.tif"], ["DIFFERENCE"]),
    "cc2_difference_patch_u8": ("gp.ComputeChangeRaster_ia", [RASTER / "patches_i16.tif", RASTER / "landuse_u8.tif"], ["DIFFERENCE"]),
    "cc2_difference_dem_nodata": ("gp.ComputeChangeRaster_ia", [RASTER / "dem.tif", RASTER / "dem_nodata.tif"], ["DIFFERENCE"]),
    "cc2_relative_difference_dem_i16": ("gp.ComputeChangeRaster_ia", [RASTER / "dem.tif", RASTER / "dem_int16_packbits.tif"], ["RELATIVE_DIFFERENCE"]),
    "cc2_relative_difference_i16_dem": ("gp.ComputeChangeRaster_ia", [RASTER / "dem_int16_packbits.tif", RASTER / "dem.tif"], ["RELATIVE_DIFFERENCE"]),
    "cc2_relative_difference_u8_patch": ("gp.ComputeChangeRaster_ia", [RASTER / "landuse_u8.tif", RASTER / "patches_i16.tif"], ["RELATIVE_DIFFERENCE"]),
    "cc2_relative_difference_patch_u8": ("gp.ComputeChangeRaster_ia", [RASTER / "patches_i16.tif", RASTER / "landuse_u8.tif"], ["RELATIVE_DIFFERENCE"]),
    "cc2_relative_difference_dem_nodata": ("gp.ComputeChangeRaster_ia", [RASTER / "dem.tif", RASTER / "dem_nodata.tif"], ["RELATIVE_DIFFERENCE"]),
    "bmask_u8": ("gp.CreateBinaryMask_ia", [RASTER / "landuse_u8.tif"], []),
    "bmask_u8_bg2": ("gp.CreateBinaryMask_ia", [RASTER / "landuse_u8.tif"], [2]),
    "bmask_dem": ("gp.CreateBinaryMask_ia", [RASTER / "dem.tif"], []),
    "bmask_nodata": ("gp.CreateBinaryMask_ia", [RASTER / "dem_nodata.tif"], []),
    "bmask_patches": ("gp.CreateBinaryMask_ia", [RASTER / "patches_i16.tif"], [7]),
    "tess_sq_a": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "SQUARE", "100 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_sq_b": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "100 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_sq_c": ("management.GenerateTessellation", [], ["500037 5000011 500390 5000333", "SQUARE", "100 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_sq_d": ("management.GenerateTessellation", [], ["500037 5000011 500390 5000333", "SQUARE", "2500 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_sq_e": ("management.GenerateTessellation", [], ["500100 5000200 501000 5000500", "SQUARE", "10000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_dia_a": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "DIAMOND", "10000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_dia_b": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "DIAMOND", "10000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_dia_c": ("management.GenerateTessellation", [], ["500037 5000011 500390 5000333", "DIAMOND", "5000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_dia_d": ("management.GenerateTessellation", [], ["500100 5000200 501000 5000500", "DIAMOND", "20000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_hex_a": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "HEXAGON", "10000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_hex_b": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "HEXAGON", "10000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_hex_c": ("management.GenerateTessellation", [], ["500037 5000011 500390 5000333", "HEXAGON", "5000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_hex_d": ("management.GenerateTessellation", [], ["500100 5000200 501000 5000500", "HEXAGON", "20000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_area_sqm": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "10000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_area_km": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "0.01 SquareKilometers", str(DATA / "parcels.shp")]),
    "tess_area_ha": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "1 Hectares", str(DATA / "parcels.shp")]),
    "tess_area_acre": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "2.471053814671653 Acres", str(DATA / "parcels.shp")]),
    "tess_area_sqft": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "107639.10416709722 SquareFeet", str(DATA / "parcels.shp")]),
    "tess_area_sqyard": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "11959.900463010803 SquareYards", str(DATA / "parcels.shp")]),
    "tess_area_sqmile": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "0.003861021585424458 SquareMiles", str(DATA / "parcels.shp")]),
    "tess_area_sqcm": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "100000000 SquareCentimeters", str(DATA / "parcels.shp")]),
    "tess_area_are": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "100 Ares", str(DATA / "parcels.shp")]),
    "tess_area_bare": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "10000", str(DATA / "parcels.shp")]),
    "tess_area_feet_sqm": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "10000 SquareMeters", str(ROOT / "testdata/tessellation/feet_template.shp")]),
    "tess_area_feet_bare": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "10000", str(ROOT / "testdata/tessellation/feet_template.shp")]),
    "tess_area_feet_sqft": ("management.GenerateTessellation", [], ["500000 5000000 540000 5030000", "SQUARE", "100000000 SquareFeet", str(ROOT / "testdata/tessellation/feet_template.shp")]),
    "tess_area_feet_us": ("management.GenerateTessellation", [], ["500000 5000000 540000 5030000", "SQUARE", "100000000 SquareFeetUS", str(ROOT / "testdata/tessellation/feet_template.shp")]),
    "tess_area_unknown_sr": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "10000 SquareMeters"]),
    "tess_area_sqmm": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "10000000000 SquareMillimeters", str(DATA / "parcels.shp")]),
    "tess_area_sqdm": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "1000000 SquareDecimeters", str(DATA / "parcels.shp")]),
    "tess_area_sqin": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "15500031.000062 SquareInches", str(DATA / "parcels.shp")]),
    "tess_area_sqnaut": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "0.002915533495981229 SquareNauticalMiles", str(DATA / "parcels.shp")]),
    "tess_area_sqyard_us": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "11959.85262345679 SquareYardsUS", str(DATA / "parcels.shp")]),
    "tess_area_acre_us": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "2.4710439304662795 AcresUS", str(DATA / "parcels.shp")]),
    "tess_area_sqmile_us": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "0.003861006141353561 SquareMilesUS", str(DATA / "parcels.shp")]),
    "tess_area_sqin_us": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "SQUARE", "15499969.0 SquareInchesUS", str(DATA / "parcels.shp")]),
    "tess_area_mile_factor": ("management.GenerateTessellation", [], ["0 0 1 1", "SQUARE", "1 SquareMiles", str(DATA / "parcels.shp")]),
    "tess_area_mile_us_factor": ("management.GenerateTessellation", [], ["0 0 1 1", "SQUARE", "1 SquareMilesUS", str(DATA / "parcels.shp")]),
    "tess_area_acre_factor": ("management.GenerateTessellation", [], ["0 0 1 1", "SQUARE", "1 Acres", str(DATA / "parcels.shp")]),
    "tess_area_acre_us_factor": ("management.GenerateTessellation", [], ["0 0 1 1", "SQUARE", "1 AcresUS", str(DATA / "parcels.shp")]),
    "tess_area_hexagon_ha": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "HEXAGON", "1 Hectares", str(DATA / "parcels.shp")]),
    "tess_area_transverse_hexagon_ha": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "TRANSVERSE_HEXAGON", "1 Hectares", str(DATA / "parcels.shp")]),
    "tess_area_triangle_ha": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "TRIANGLE", "1 Hectares", str(DATA / "parcels.shp")]),
    "tess_area_diamond_ha": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "DIAMOND", "1 Hectares", str(DATA / "parcels.shp")]),
    **{f"tess_trhex_{name}":
       ("management.GenerateTessellation", [], [extent,"TRANSVERSE_HEXAGON",size,str(DATA / "parcels.shp")])
       for name,extent,size in [
           ("a","500000 5000000 500400 5000300","10000 SquareMeters"),
           ("b","500000 5000000 500413 5000277","10000 SquareMeters"),
           ("c","500037 5000011 500390 5000333","5000 SquareMeters"),
           ("d","500100 5000200 501000 5000500","20000 SquareMeters"),
           ("tiny","500000 5000000 500001 5000001","10000 SquareMeters"),
           ("thin","500000 5000000 500400 5000001","10000 SquareMeters")]},
    "tess_tri_a": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "TRIANGLE", "5000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_tri_b": ("management.GenerateTessellation", [], ["500000 5000000 500413 5000277", "TRIANGLE", "5000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_tri_c": ("management.GenerateTessellation", [], ["500037 5000011 500390 5000333", "TRIANGLE", "5000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_tri_d": ("management.GenerateTessellation", [], ["500100 5000200 501000 5000500", "TRIANGLE", "20000 SquareMeters", str(DATA / "parcels.shp")]),
    "tess_tri_e": ("management.GenerateTessellation", [], ["500000 5000000 500400 5000300", "TRIANGLE", "20000 SquareMeters", str(DATA / "parcels.shp")]),
    "gpal_roads_50": ("management.GeneratePointsAlongLines", [DATA / "roads.shp"], ["DISTANCE", "50 Meters", "", "NO_END_POINTS"]),
    "gpal_roads_50_end": ("management.GeneratePointsAlongLines", [DATA / "roads.shp"], ["DISTANCE", "50 Meters", "", "END_POINTS"]),
    "gpal_roads_pct": ("management.GeneratePointsAlongLines", [DATA / "roads.shp"], ["PERCENTAGE", "", 25, "NO_END_POINTS"]),
    "gpal_streams_80_chain": ("management.GeneratePointsAlongLines", [DATA / "streams.shp"], ["DISTANCE", "80 Meters", "", "END_POINTS", "ADD_CHAINAGE"]),
    "sap_chain_10":("management.SplitLineAtPoint", [DM / "chain.shp", DM / "pts.shp"], ["10 Meters"]),
    "sap_chain_1": ("management.SplitLineAtPoint", [DM / "chain.shp", DM / "pts.shp"], ["1 Meters"]),
    "sap_chain_25": ("management.SplitLineAtPoint", [DM / "chain.shp", DM / "pts.shp"], ["25 Meters"]),
    "sap_roads_wells_20": ("management.SplitLineAtPoint", [DATA / "roads.shp", DATA / "wells.shp"], ["20 Meters"]),
    "sap_streams_wells_40": ("management.SplitLineAtPoint", [DATA / "streams.shp", DATA / "wells.shp"], ["40 Meters"]),
    "epp_area_500": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["AREA", 500, 0, "CONTAINED_ONLY"]),
    "epp_area_30": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["AREA", 30, 0, "CONTAINED_ONLY"]),
    "epp_area_any_500": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["AREA", 500, 0, "ANY"]),
    "epp_area_any_1000": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["AREA", 1000, 0, "ANY"]),
    "epp_pct_5": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["PERCENT", 0, 5, "CONTAINED_ONLY"]),
    "epp_pct_any_30": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["PERCENT", 0, 30, "ANY"]),
    "epp_pct_any_85": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["PERCENT", 0, 8.5, "ANY"]),
    "epp_pct_any_9": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["PERCENT", 0, 9.5, "ANY"]),
    "epp_pct_3": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["PERCENT", 0, 4.5, "CONTAINED_ONLY"]),
    "epp_pct_38": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["PERCENT", 0, 3.8, "CONTAINED_ONLY"]),
    "epp_and": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["AREA_AND_PERCENT", 500, 20, "CONTAINED_ONLY"]),
    "epp_or": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["AREA_OR_PERCENT", 30, 10, "CONTAINED_ONLY"]),
    "epp_or_any": ("management.EliminatePolygonPart", [DM / "parts.shp"], ["AREA_OR_PERCENT", 30, 10, "ANY"]),
    "epp_holes": ("management.EliminatePolygonPart", [PLANAR / "holes.shp"], ["AREA", 1700, 0, "CONTAINED_ONLY"]),
    "xy2l_attr": ("management.XYToLine", [DM / "segments.dbf"], ["X1", "Y1", "X2", "Y2", "GEODESIC", "", DM / "chain.prj", "ATTRIBUTES"]),
    "xy2l_id": ("management.XYToLine", [DM / "segments.dbf"], ["X1", "Y1", "X2", "Y2", "GEODESIC", "SEG_ID", DM / "chain.prj", "NO_ATTRIBUTES"]),
    "xy2l_plain": ("management.XYToLine", [DM / "segments.dbf"], ["X1", "Y1", "X2", "Y2", "NORMAL_SECTION", "", DM / "chain.prj", "NO_ATTRIBUTES"]),
    "cfs_new": ("management.CalculateFields", [PARCELS, "PYTHON3", [["NEWF", "!VALUE!+1"], ["NEWS", "!NAME!.upper()"]]], []),
    "cfs_existing": ("management.CalculateFields", [PARCELS, "PYTHON3", [["ZONE", "!ZONE!+1"], ["PARCEL_ID", "!ZONE!*100"]]], []),
    "cfs_types": ("management.CalculateFields", [PARCELS, "PYTHON3", [["A", "!VALUE!*2", "", "DOUBLE"], ["B", "!PARCEL_ID!*3", "", "LONG"], ["C", "!ZONE!", "", "SHORT"], ["D", "!NAME!", "", "TEXT"], ["E", "!VALUE!", "", "FLOAT"]]], []),
    "cfs_where": ("management.CalculateFields", [PARCELS, "PYTHON3", [["ZONE", "!ZONE!*10", "ZONE > 2"], ["X", "1", "NAME = 'Lot 3'", "LONG"]]], []),
    "cfs_codeblock": ("management.CalculateFields", [PARCELS, "PYTHON3", [["ZONE", "f(!ZONE!)"], ["NAME", "g(!NAME!)"]]], ["def f(x):\n    return x * 7\ndef g(s):\n    return s[::-1]"]),
    "cfs_bad_field": ("management.CalculateFields", [PARCELS, "PYTHON3", [["ZONE", "!NOPE!"]]], []),
    "std_z": ("management.StandardizeField", [PARCELS, [["VALUE"], ["ZONE"]]], ["Z-SCORE"]),
    "std_minmax": ("management.StandardizeField", [PARCELS, [["VALUE", "VMM"], ["ZONE"]]], ["MIN-MAX", 0, 10]),
    "std_maxabs": ("management.StandardizeField", [PARCELS, [["VALUE"]]], ["MAXABS"]),
    "std_robust": ("management.StandardizeField", [PARCELS, [["VALUE"]]], ["ROBUST"]),
    "std_default": ("management.StandardizeField", [DATA / "wells.shp", [["DEPTH"]]], []),
    "tf_log": ("management.TransformField", [PARCELS, [["VALUE"]]], ["LOG"]),
    "tf_sqrt": ("management.TransformField", [PARCELS, [["VALUE"], ["ZONE", "ZSQ"]]], ["SQRT"]),
    "tf_invx": ("management.TransformField", [PARCELS, [["VALUE"]]], ["INVX"]),
    "tf_invsqrt": ("management.TransformField", [PARCELS, [["VALUE"]]], ["INV_SQRT"]),
    "tf_invlog": ("management.TransformField", [PARCELS, [["ZONE"]]], ["INV_LOG"]),
    "tf_boxcox_1": ("management.TransformField", [PARCELS, [["VALUE"]]], ["BOX-COX", 0.5]),
    "tf_log_zone": ("management.TransformField", [PARCELS, [["ZONE"]]], ["LOG"]),
    "tf_sqrt_zone": ("management.TransformField", [PARCELS, [["ZONE"]]], ["SQRT"]),
    "tf_boxcox_zone": ("management.TransformField", [PARCELS, [["ZONE"]]], ["BOX-COX", 0.5]),
    "tf_invx_zone": ("management.TransformField", [PARCELS, [["ZONE"]]], ["INVX"]),
    "tf_invsqrt_shift": ("management.TransformField", [PARCELS, [["ZONE"]]], ["INV_SQRT", "#", 1]),
    "tf_invboxcox": ("management.TransformField", [PARCELS, [["ZONE"]]], ["INV_BOX-COX", 2]),
    "tf_log_shift": ("management.TransformField", [PARCELS, [["VALUE"]]], ["LOG", "#", 5]),
    "tf_log_zero": ("management.TransformField", [DATA / "wells.shp", [["DEPTH"]]], ["LOG"]),
    "rf_equal": ("management.ReclassifyField", [PARCELS, "VALUE"], ["EQUAL_INTERVAL", 4]),
    "rf_defined": ("management.ReclassifyField", [PARCELS, "VALUE"], ["DEFINED_INTERVAL", "#", 1500]),
    "rf_quantile": ("management.ReclassifyField", [PARCELS, "VALUE"], ["QUANTILE", 5]),
    "rf_unique_text": ("management.ReclassifyField", [PARCELS, "NAME"], ["UNIQUE_VALUES", "#", "#", "#", "#", "DESC", "RC"]),
    "rf_manual": ("management.ReclassifyField", [PARCELS, "VALUE"], ["MANUAL", "#", "#", "#", [[3000, 1], [6000, 2], [10000, 3]]]),
    "rf_equal_desc": ("management.ReclassifyField", [PARCELS, "VALUE"], ["EQUAL_INTERVAL", 3, "#", "#", "#", "DESC", "CLS"]),
    "transpose_all": ("management.TransposeFields", [PARCELS, "VALUE Val;ZONE Zn"], ["FLD", "VAL", "PARCEL_ID;NAME"]),
    "transpose_sales": ("management.TransposeFields", [DM / "sales.dbf", "AMOUNT;UNITS"], ["WHAT", "NUM", "NAME"]),
    "transpose_noattr": ("management.TransposeFields", [DM / "sales.dbf", "AMOUNT;UNITS"], ["WHAT", "NUM"]),
    "pivot_sales": ("management.PivotTable", [DM / "sales.dbf", "NAME", "NAME", "AMOUNT"], []),
    "pivot_year": ("management.PivotTable", [DM / "sales.dbf", "REGION;YEAR", "NAME", "UNITS"], []),
    "pivot_text": ("management.PivotTable", [DM / "sales.dbf", "NAME", "YEAR", "AMOUNT"], []),
    "bdl_attr": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "DIST", "METERS", "BEARING", "DEGREES", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_plain": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "DIST", "METERS", "BEARING", "DEGREES", "GEODESIC", "", DM / "chain.prj", "NO_ATTRIBUTES"]),
    "bdl_id": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "DIST", "METERS", "BEARING", "DEGREES", "GEODESIC", "SEG_ID", DM / "chain.prj", "NO_ATTRIBUTES"]),
    "bdl_km": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "D_KM", "KILOMETERS", "BEARING", "DEGREES", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_mi": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "D_MI", "MILES", "BEARING", "DEGREES", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_nm": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "D_NM", "NAUTICAL_MILES", "BEARING", "DEGREES", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_ft": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "D_FT", "FEET", "BEARING", "DEGREES", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_usft": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "D_USFT", "US_SURVEY_FEET", "BEARING", "DEGREES", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_mils": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "DIST", "METERS", "B_MIL", "MILS", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_rads": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "DIST", "METERS", "B_RAD", "RADS", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "bdl_grads": ("management.BearingDistanceToLine", [DM / "segments.dbf"], ["X1", "Y1", "DIST", "METERS", "B_GRAD", "GRADS", "GEODESIC", "SEG_ID", DM / "chain.prj", "ATTRIBUTES"]),
    "cfc_polygon": ("management.CreateFeatureclass", ["POLYGON", "#", "DISABLED", "DISABLED", DM / "chain.prj"], []),
    "cfc_point_zm": ("management.CreateFeatureclass", ["POINT", "#", "ENABLED", "ENABLED", DM / "chain.prj"], []),
    "cfc_polyline_m": ("management.CreateFeatureclass", ["POLYLINE", "#", "ENABLED", "DISABLED", DM / "chain.prj"], []),
    "cfc_polyline_nosr": ("management.CreateFeatureclass", ["POLYLINE"], []),
    "cfc_multipoint": ("management.CreateFeatureclass", ["MULTIPOINT", "#", "DISABLED", "ENABLED", DM / "chain.prj"], []),
    "cfc_template": ("management.CreateFeatureclass", ["POLYGON", PARCELS, "DISABLED", "DISABLED", DM / "chain.prj"], []),
    "cfc_template_same": ("management.CreateFeatureclass", ["POLYLINE", DATA / "roads_zm.shp", "SAME_AS_TEMPLATE", "SAME_AS_TEMPLATE"], []),
    "ct_default": ("management.CreateTable", [], []),
    "ct_template": ("management.CreateTable", [PARCELS], []),
    "defras_4326": ("management.DefineProjection", [OUT / "ascimp_small_integer.tif", 4326], []),
    "defras_32633": ("management.DefineProjection", [OUT / "ascimp_small_integer.tif", 32633], []),
    "defras_3857": ("management.DefineProjection", [OUT / "ascimp_small_integer.tif", 3857], []),
    "defras_7844": ("management.DefineProjection", [OUT / "ascimp_small_integer.tif", 7844], []),
    "defras_7856": ("management.DefineProjection", [OUT / "ascimp_small_integer.tif", 7856], []),
    "defproj_prj": ("management.DefineProjection", [DM / "noprj.shp", DM / "chain.prj"], []),
    "defproj_wgs84": ("management.DefineProjection", [DM / "noprj.shp", DATA / "wells_wgs84.prj"], []),
    "defproj_replace": ("management.DefineProjection", [PARCELS, DATA / "wells_wgs84.prj"], []),
    "copy_parcels": ("management.Copy", [PARCELS], []),
    "copy_roads_zm": ("management.Copy", [DATA / "roads_zm.shp"], []),
    "copy_wells": ("management.Copy", [DATA / "wells.shp"], []),
    "copy_sales": ("management.Copy", [DM / "sales.dbf"], []),
    "cet_plain": ("management.CalculateEndTime", [DM / "events.shp", "STARTD", "ENDD"], []),
    "cet_name": ("management.CalculateEndTime", [DM / "events.shp", "STARTD", "ENDD"], ["NAME"]),
    "cet_name_grp": ("management.CalculateEndTime", [DM / "events.shp", "STARTD", "ENDD"], ["NAME;GRP"]),
    "cet_grp": ("management.CalculateEndTime", [DM / "events.shp", "STARTD", "ENDD"], ["GRP"]),
}
# Tools run through the registry (`ug run <id> name=value ...`): the ArcGIS tool and the registry
# parameter each of its arguments goes to (`output` for the result, None to leave one out). A new
# tool in the raster registry needs only a line here; the reference test builds the command from it.
# Spatial Analyst Math tools on one raster: (ArcGIS tool, registry id)
UNARY = [("Abs", "abs"), ("Negate", "negate"), ("Square", "square"), ("SquareRoot", "square_root"), ("Exp", "exp"),
         ("Exp2", "exp2"), ("Exp10", "exp10"), ("Ln", "ln"), ("Log2", "log2"), ("Log10", "log10"), ("Sin", "sin"),
         ("Cos", "cos"), ("Tan", "tan"), ("ASin", "asin"), ("ACos", "acos"), ("ATan", "atan"), ("SinH", "sinh"),
         ("CosH", "cosh"), ("TanH", "tanh"), ("ASinH", "asinh"), ("ACosH", "acosh"), ("ATanH", "atanh"),
         ("RoundDown", "round_down"), ("RoundUp", "round_up"), ("BooleanNot", "boolean_not"), ("BitwiseNot", "bitwise_not")]
for _tool, _id in UNARY:
    for _in in ("dem", "dem_int16_packbits", "landuse_u8"):
        CASES[f"um_{_id}_{_in}"] = (f"gp.{_tool}_sa", [RASTER / f"{_in}.tif"], [])


# Two-operand Spatial Analyst Math tools: (ArcGIS tool, registry id)
BINARY = [("ATan2", "atan2"), ("Power", "power"), ("Mod", "mod"), ("BooleanAnd", "boolean_and"), ("BooleanOr", "boolean_or"),
          ("BooleanXOr", "boolean_xor"), ("EqualTo", "equal_to"), ("GreaterThan", "greater_than"),
          ("GreaterThanEqual", "greater_than_equal"), ("LessThan", "less_than"), ("LessThanEqual", "less_than_equal"),
          ("NotEqual", "not_equal"), ("BitwiseAnd", "bitwise_and"), ("BitwiseOr", "bitwise_or"),
          ("BitwiseXOr", "bitwise_xor"), ("BitwiseLeftShift", "bitwise_left_shift"),
          ("BitwiseRightShift", "bitwise_right_shift")]
_OPERANDS = {"dem_i16": ("dem.tif", "dem_int16_packbits.tif"), "i16_u8": ("dem_int16_packbits.tif", "landuse_u8.tif"),
             "u8_u8": ("landuse_u8.tif", "landuse_u8.tif"), "u8_c2": ("landuse_u8.tif", "2"),
             "dem_c200": ("dem.tif", "200"), "i16_c200": ("dem_int16_packbits.tif", "200"),
             "u8_c05": ("landuse_u8.tif", "0.5"), "c7_i16": ("7", "dem_int16_packbits.tif")}
for _tool, _id in BINARY:
    for _k, _ops in _OPERANDS.items():
        CASES[f"bm_{_id}_{_k}"] = (f"gp.{_tool}_sa", [RASTER / o if o.endswith(".tif") else o for o in _ops], [])


COMBINATORIAL = [("CombinatorialAnd", "combinatorial_and"), ("CombinatorialOr", "combinatorial_or"),
                 ("CombinatorialXOr", "combinatorial_xor")]
for _tool, _id in COMBINATORIAL:
    for _k, (_a, _b) in {"lu_patch": ("landuse_u8", "patches_i16"), "lu_mask": ("landuse_u8", "nibble_mask_i16"),
                         "patch_lu": ("patches_i16", "landuse_u8")}.items():
        CASES[f"cb_{_id}_{_k}"] = (f"gp.{_tool}_sa", [RASTER / f"{_a}.tif", RASTER / f"{_b}.tif"], [])


RUN = {
    "gp.AddSurfaceInformation_sa": ("add_surface_information", ["input", "in_surface", "out_property", "output", "method", "sample_distance", "z_factor"]),
    "gp.RescaleByFunction_sa": ("rescale_by_function", ["input", "output", "transformation_function", "from_scale", "to_scale"]),
    "gp.CreateNormalRaster_sa": ("create_normal_raster", ["output", "cell_size", "extent"]),
    "conversion.ExtractLocationsText": ("extract_locations_text", ["in_text", "output", "", "coord_dd_latlon", "", "coord_dd_xyplain", "coord_dm_latlon", "", "coord_dms_latlon", "", "", "coord_utm", "", "", "coord_mgrs"]),
    "conversion.ExtractLocationsDocument": ("extract_locations_document", ["in_file", "output", "", "coord_dd_latlon", "", "coord_dd_xyplain", "coord_dm_latlon", "", "coord_dms_latlon", "", "", "coord_utm", "", "", "coord_mgrs"]),
    "conversion.TableToGeodatabase": ("table_to_geodatabase", ["input_table", "output_geodatabase"]),
    "management.CreateFolder": ("create_folder", ["out_folder_path", "out_name"]),
    "management.CreateFileGDB": ("create_file_gdb", ["out_folder_path", "out_name", "out_version"]),
    "management.CreateMobileGDB": ("create_mobile_gdb", ["out_folder_path", "out_name"]),
    "conversion.FeatureClassToGeodatabase": ("feature_class_to_geodatabase", ["input_features", "output_geodatabase"]),
    "conversion.ExcelToTable": ("excel_to_table", ["input", "output", "sheet", "field_names_row", "cell_range"]),
    "conversion.TableToExcel": ("table_to_excel", ["input_table", "output_excel_file", "use_field_alias_as_column_header", "use_domain_and_subtype_description"]),
    "management.AddGlobalIDs": ("add_global_ids", ["in_datasets"]),
    "management.SubsetFeatures": ("subset_features", ["input", "out_training_feature_class", "out_test_feature_class", "size_of_training_dataset", "subset_size_units"]),
    "management.CreateSpatialSamplingLocations": ("create_spatial_sampling_locations", ["input", "output", "sampling_method", "strata_id_field", "strata_count_method", "bin_shape", "bin_size", "h3_resolution", "num_samples", "num_samples_per_strata", "population_field", "geometry_type", "min_distance", "spatial_relationship"]),
    "management.CreateRandomPoints": ("create_random_points", ["output", "constraining_feature_class", "constraining_extent", "number_of_points_or_field", "minimum_allowed_distance", "create_multipoint_output", "multipoint_size"]),
    "management.ExtractDataFromGeodatabase": ("extract_data_from_geodatabase", ["in_data", "extract_type", "out_type", "out_geodatabase", "out_xml", "out_folder_path", "out_name", "expand_feature_classes_and_tables", "reuse_schema", "get_related_data", "extract_using_geometry_features", "geometry_filter_type", "all_records_for_tables"]),
    "gp.Nibble_sa": ("nibble", ["input", "mask", "output", "values", "nodata"]),
    "gp.Filter_sa": ("filter", ["input", "output", "filter_type", "ignore_nodata"]),
    "gp.Slice_sa": ("slice", ["input", "output", "number_zones", "slice_type", "base_output_zone"]),
    "gp.ExtractByAttributes_sa": ("extract_by_attributes", ["input", "where_clause", "output"]),
    "gp.ExtractByRectangle_sa": ("extract_by_rectangle", ["input", "rectangle", "output", "extraction_area"]),
    "gp.ExtractByCircle_sa": ("extract_by_circle", ["input", "center_point", "radius", "output", "extraction_area"]),
    "gp.Sink_sa": ("sink", ["input", "output"]),
    "gp.Pick_sa": ("pick", ["position", "items", "output"]),
    "gp.LowestPosition_sa": ("lowest_position", ["inputs", "output"]),
    "management.Flip": ("flip", ["input", "output"]),
    "management.Mirror": ("mirror", ["input", "output"]),
    "management.Shift": ("shift", ["input", "output", "x_value", "y_value"]),
    "management.Clip": ("clip_raster", ["input", "rectangle", "output", None, "nodata_value", "clipping_geometry", "maintain_extent"]),
    "analysis.PairwiseBuffer": ("pairwise_buffer", ["input", "output", "distance", "dissolve_option", "dissolve_field", "method"]),
    "analysis.PairwiseDissolve": ("pairwise_dissolve", ["input", "output", "dissolve_field", "statistics_fields", "multi_part"]),
    "analysis.PairwiseIntersect": ("pairwise_intersect", ["inputs", "output", "join_attributes", None, "output_type"]),
    "analysis.PairwiseClip": ("pairwise_clip", ["input", "clip_features", "output"]),
    "analysis.PairwiseErase": ("pairwise_erase", ["input", "erase_features", "output"]),
    "management.GeneratePointsAlongLines": ("generate_points_along_lines", ["input", "output", "point_placement", "distance", "percentage", "include_end_points", "add_chainage_fields"]),
    "analysis.MultipleRingBuffer": ("multiple_ring_buffer", ["input", "output", "distances", "buffer_unit", "field_name", "dissolve_option", "outside_polygons_only"]),
    "management.CopyRows": ("copy_rows", ["input", "output"]),
    "management.SubdividePolygon": ("subdivide_polygon", ["input", "output", "method", "number_of_areas", "target_area"]),
    "analysis.TabulateIntersection": ("tabulate_intersection", ["input", "zone_fields", "class_features", "output", "class_fields", "sum_fields", "xy_tolerance", "output_units"]),
    "analysis.CreateThiessenPolygons": ("create_thiessen_polygons", ["input", "output", "fields_to_copy"]),
    "cartography.SimplifyLine": ("simplify_line", ["input", "output", "algorithm", "tolerance"]),
    **{f"gp.{t}_sa": (i, ["input", "output"]) for t, i in UNARY},
    **{f"gp.{t}_sa": (i, ["in1", "in2", "output"]) for t, i in BINARY},
    **{f"gp.{t}_sa": (i, ["in1", "in2", "output"]) for t, i in COMBINATORIAL},
    "conversion.RasterToASCII": ("raster_to_ascii", ["input", "output"]),
    "conversion.RasterToFloat": ("raster_to_float", ["input", "output"]),
    "conversion.RasterToPolyline": ("raster_to_polyline", ["input", "output", "background_value", "minimum_dangle_length", "simplify", "raster_field"]),
    "analysis.TableSelect": ("table_select", ["input", "output", "where_clause"]),
    "conversion.ExportFeatures": ("export_features", ["input", "output", "where_clause", None, "field_mapping", "sort_field"]),
    "management.GenerateTransectsAlongLines": ("generate_transects_along_lines", ["input", "output", "interval", "transect_length", "include_end_points"]),
    "cartography.GridIndexFeatures": ("grid_index_features", ["input", "output", "intersect_feature", None, None, "polygon_width", "polygon_height"]),
    "gp.ReclassByASCIIFile_sa": ("reclass_by_ascii_file", ["input", "remap_file", "output", "missing_values"]),
    "edit.ExtendLine": ("extend_line", ["input", "output", "length", "extend_to"]),
    "edit.FlipLine": ("flip_line", ["input", "output"]),
    "edit.TrimLine": ("trim_line", ["input", "output", "dangle_length", "delete_shorts"]),
    "analysis.GenerateOriginDestinationLinks": ("generate_origin_destination_links", ["origin_features", "destination_features", "output", "origin_group_field", "destination_group_field", "line_type", "num_nearest", "search_distance", "distance_unit", "aggregate_links", "sum_fields"]),
    "analysis.AssignWeightsByPairwiseComparison": ("assign_weights_by_pairwise_comparison", ["input_variables", "output", "add_comparison_matrix", "comparison_matrix"]),
    "management.AddSpatialJoin": ("add_spatial_join", ["target_features", "join_features", "output", "join_operation", "join_type", None, "match_option", "search_radius", "distance_field_name", "permanent_join"]),
    "management.BatchUpdateFields": ("batch_update_fields", ["input", "output", "field_definition_table", None, "output_field_name", "source_field_name", "output_field_type", "output_field_decimals_or_length", "output_field_alias", "output_field_script"]),
    "management.ConvertTimeField": ("convert_time_field", ["input", "output", "input_time_field", "input_time_format", "output_time_field", "output_time_type", "output_time_format"]),
    "management.ConvertTimeZone": ("convert_time_zone", ["input", "output", "input_time_field", "input_time_zone", "output_time_field", "output_time_zone", "input_dst", "output_dst"]),
    "management.ValidateJoin": ("validate_join", ["input", "in_field", "join_table", "join_field", "output"]),
    "management.Adjust3DZ": ("adjust_3d_z", ["input", "output", "reverse_sign", "adjust_value", "from_units", "to_units"]),
    "management.CalculateGeometryAttributes": ("calculate_geometry_attributes", ["input", "output", "geometry_property", "length_unit", "area_unit"]),
    "conversion.FeatureClassToShapefile": ("feature_class_to_shapefile", ["input_features", "output_folder"]),
    "conversion.TableToDBASE": ("table_to_dbase", ["input_tables", "output_folder"]),
    "conversion.GPXtoFeatures": ("gpx_to_features", ["input_gpx_file", "output", "output_type"]),
    "conversion.MultipatchToRaster": ("multipatch_to_raster", ["input", "output", "cell_size", "cell_assignment_method"]),
    "conversion.RasterToOtherFormat": ("raster_to_other_format", ["input_rasters", "output_workspace", "raster_format"]),
    "management.FieldStatisticsToTable": ("field_statistics_to_table", ["input", "output", "fields", "field_type", "group_by", "statistics"]),
    "management.EncodeField": ("encode_field", ["input", "output", "field", "method"]),
    "management.GeodeticDensify": ("geodetic_densify", ["input", "output", "densify_method", "max_segment_length"]),
    "management.TableToEllipse": ("table_to_ellipse", ["input", "output", "x_field", "y_field", "major_field", "minor_field", "distance_units", "azimuth_field", "azimuth_units", None, "spatial_reference", None, "shape_type", "method"]),
    "management.ConvertCoordinateNotation": ("convert_coordinate_notation", ["input", "output", "x_field", "y_field", "input_format", "output_format"]),
    "edit.Generalize": ("generalize", ["input", "output", "tolerance"]),
    "management.CopyRaster": ("copy_raster", ["input", "output", None, None, "nodata_value", None, None, "pixel_type"]),
    "management.MosaicToNewRaster": ("mosaic_to_new_raster", ["inputs", "output", None, "pixel_type", "cellsize", None, "mosaic_method"]),
    "gp.ExtractByPolygon_sa": ("extract_by_polygon", ["input", "polygon", "output", "extraction_area"]),
    "gp.ExtractByPoints_sa": ("extract_by_points", ["input", "points", "output", "extraction_area"]),
    "gp.TopographicPositionIndex_sa": ("topographic_position_index", ["input", "output", "neighborhood"]),
    "gp.TopHatTransform_sa": ("top_hat_transform", ["input", "output", "neighborhood", "analysis_type"]),
    "gp.Test_sa": ("test", ["input", "where_clause", "output"]),
    "gp.Trend_sa": ("trend", ["input", "z_field", "output", "cell_size", "order"]),
    "gp.Rank_sa": ("rank", ["rank", "rasters", "output"]),
    "gp.Popularity_sa": ("popularity", ["rank", "rasters", "output"]),
    "gp.EqualToFrequency_sa": ("equal_to_frequency", ["input", "rasters", "output", "process_as_multiband"]),
    "gp.GreaterThanFrequency_sa": ("greater_than_frequency", ["input", "rasters", "output", "process_as_multiband"]),
    "gp.LessThanFrequency_sa": ("less_than_frequency", ["input", "rasters", "output", "process_as_multiband"]),
    "gp.InList_sa": ("in_list", ["input", "list", "output"]),
    "gp.CreateConstantRaster_sa": ("create_constant_raster", ["constant", "output", "data_type", "cell_size", "extent"]),
    "analysis.SummarizeWithin": ("summarize_within", ["input", "sum_features", "output", "keep_all", "summary_fields", "add_shape_sum", "shape_unit"]),
    "analysis.SummarizeNearby": ("summarize_nearby", ["input", "sum_features", "output", "near_type", "distances", "distance_unit"]),
    "conversion.ASCIIToRaster": ("ascii_to_raster", ["input", "output", "data_type"]),
    "conversion.FloatToRaster": ("float_to_raster", ["input", "output"]),
    "management.CompositeBands": ("composite_bands", ["inputs", "output"]),
    "analysis.GraphicBuffer": ("graphic_buffer", ["input", "output", "distance", "line_caps", "line_joins", "miter_limit"]),
    "analysis.ApportionPolygon": ("apportion_polygon", ["input", "apportion_fields", "target_features", "output", "apportion_by"]),
    "gp.EucBackDirection_sa": ("euclidean_back_direction", ["input", "output", None, "maximum_distance"]),
    "gp.DistanceAccumulation_sa": ("distance_accumulation", ["input", "output"]),
    "gp.DistanceAllocation_sa": ("distance_allocation", ["input", "output"]),
    "gp.FocalFlow_sa": ("focal_flow", ["input", "output", "threshold_value"]),
    "gp.SnapPourPoint_sa": ("snap_pour_point", ["input", "accumulation", "output", "snap_distance", "pour_point_field"]),
    "gp.Sample_sa": ("sample", ["input", "locations", "output", "resampling", "unique_id_field"]),
    "gp.ExtractMultiValuesToPoints_sa": ("extract_multi_values_to_points", ["input", "rasters", "output", "interpolate"]),
    "gp.InterpolateShape_sa": ("interpolate_shape", ["input", "features", "output", "sample_distance", "z_factor", "method", "vertices_only", "pyramid_level_resolution", "preserve_features", "interpolate_m_value"]),
    "gp.PointStatistics_sa": ("point_statistics", ["input", "field", "output", "cell_size", "neighborhood", "statistic"]),
    "gp.LineStatistics_sa": ("line_statistics", ["input", "field", "output", "cell_size", "search_radius", "statistic"]),
    "gp.Diff_ia": ("diff", ["in1", "in2", "output"]),
    "gp.Over_ia": ("over", ["in1", "in2", "output"]),
    "gp.ComputeChangeRaster_ia": ("compute_change_raster", ["from_raster", "to_raster", "output", "change_type"]),
    "gp.CreateBinaryMask_ia": ("create_binary_mask", ["input", "output", "background_value"]),
    "management.GenerateTessellation": ("generate_tessellation", ["output", "extent", "shape", "size", "template"]),
    "management.AddGPSMetadataFields": ("add_gps_metadata_fields", ["input", "output", "include_offset_fields"]),
    "management.DisableCOGO": ("disable_cogo", ["input", "output"]),
    "management.EnableCOGO": ("enable_cogo", ["input", "output"]),
    "management.MigrateDateFieldToHighPrecision": ("migrate_date_field_to_high_precision", ["input", "output", "date_fields"]),
    "management.DisableEditorTracking": ("disable_editor_tracking", ["input", "output", "creator", "creation_date", "last_editor", "last_edit_date"]),
    "management.EnableEditorTracking": ("enable_editor_tracking", ["input", "output", "creator_field", "creation_date_field", "last_editor_field", "last_edit_date_field", "add_fields", "record_dates_in"]),
    "management.DeleteMultiple": ("delete_multiple", ["in_data"]),
    "analysis.SplitByAttributes": ("split_by_attributes", ["input", "target_workspace", "split_fields"]),
    "analysis.PairwisePolygonNeighbors": ("pairwise_polygon_neighbors", ["input", "output", "fields", "area_overlap", "both_sides", "cluster_tolerance", "out_linear_units", "out_area_units", "precision"]),
    "analysis.PolygonNeighbors": ("polygon_neighbors", ["input", "output", "fields", "area_overlap", "both_sides"]),
    "analysis.CountOverlappingFeatures": ("count_overlapping_features", ["input", "output", "minimum_overlap_count"]),
    "gp.Lookup_sa": ("lookup", ["input", "lookup_field", "output"]),
}


def run_spec(tool, args):
    """The `run` entry of a manifest case: registry id and parameters (None and empty are left out)."""
    if tool == "management.DefineProjection" and Path(str(args[0])).suffix.lower() in (".tif", ".tiff"):
        return {"tool": "define_projection", "params": dict(zip(("input", "coordinate_system", "output"), args))}
    if tool not in RUN:
        return None
    rid, names = RUN[tool]
    return {"tool": rid, "params": {n: a for n, a in zip(names, args) if n and a not in (None, "")}}


# Spatial Analyst tools: output is a GeoTIFF, <name>.tif.
RASTER_OUT = {"gp.RescaleByFunction_sa", "gp.CreateNormalRaster_sa", "gp.CreateBinaryMask_ia", "gp.ComputeChangeRaster_ia", "gp.Diff_ia", "gp.Over_ia", "gp.LineStatistics_sa", "gp.PointStatistics_sa", "gp.SnapPourPoint_sa", "gp.FocalFlow_sa", "gp.DistanceAccumulation_sa", "gp.DistanceAllocation_sa", "gp.EucBackDirection_sa", "gp.EqualToFrequency_sa", "gp.GreaterThanFrequency_sa", "gp.LessThanFrequency_sa", "conversion.ASCIIToRaster", "conversion.FloatToRaster", "conversion.MultipatchToRaster", "management.CompositeBands", "gp.CreateConstantRaster_sa", "gp.InList_sa", "gp.LowestPosition_sa", "gp.Rank_sa", "gp.Popularity_sa", "gp.Trend_sa", "gp.TopHatTransform_sa", "gp.TopographicPositionIndex_sa", "gp.Test_sa", "gp.ExtractByPolygon_sa", "gp.ExtractByPoints_sa", "management.MosaicToNewRaster", "management.CopyRaster", "gp.ReclassByASCIIFile_sa", *[f"gp.{t}_sa" for t, _ in UNARY], *[f"gp.{t}_sa" for t, _ in BINARY], *[f"gp.{t}_sa" for t, _ in COMBINATORIAL], "gp.Slope_sa", "gp.Aspect_sa", "gp.HillShade_sa", "gp.FocalStatistics_sa", "gp.Fill_sa",
              "gp.FlowDirection_sa", "gp.FlowAccumulation_sa", "gp.Reclassify_sa", "gp.ZonalStatistics_sa",
              "gp.Curvature_sa", "gp.Plus_sa", "gp.Minus_sa", "gp.Times_sa", "gp.Divide_sa", "gp.IsNull_sa",
              "gp.Int_sa", "gp.Float_sa", "gp.Con_sa", "gp.SetNull_sa", "gp.Basin_sa", "gp.StreamLink_sa",
              "gp.StreamOrder_sa", "gp.FlowLength_sa", "gp.Watershed_sa", "conversion.FeatureToRaster",
              "conversion.PolygonToRaster", "conversion.PolylineToRaster", "conversion.PointToRaster", "gp.EucDistance_sa", "gp.EucDirection_sa", "gp.EucAllocation_sa", "gp.KernelDensity_sa", "gp.PointDensity_sa", "gp.LineDensity_sa", "gp.ExtractByMask_sa",
              "management.Resample", "gp.Viewshed_sa", "gp.Expand_sa", "gp.Shrink_sa", "gp.MajorityFilter_sa",
              "gp.RegionGroup_sa", "gp.BlockStatistics_sa",
              "gp.CostDistance_sa", "gp.CostBackLink_sa", "gp.CostAllocation_sa", "gp.CostPath_sa", "gp.Corridor_sa", "gp.Filter_sa", "gp.Nibble_sa", "gp.ReclassByTable_sa",
              "gp.Sink_sa", "gp.Pick_sa", "management.Flip", "management.Mirror", "management.Shift", "management.Clip",
              "gp.Slice_sa", "gp.ExtractByAttributes_sa", "gp.ExtractByRectangle_sa", "gp.ExtractByCircle_sa", "gp.Lookup_sa"}
WORKSPACE_OUT = {"conversion.FeatureClassToShapefile", "conversion.TableToDBASE", "conversion.RasterToOtherFormat", "analysis.Split", "analysis.SplitByAttributes"}
EXCEL_OUT = {"conversion.TableToExcel"}
# Tools that write a text file: compared byte for byte.
TEXT_OUT = {"conversion.FeaturesToJSON", "conversion.ExportTable", "conversion.RasterToASCII", "conversion.RasterToFloat"}
# Tools whose output is a table, written as <name>.dbf.
TABLE_OUT = {"conversion.ExcelToTable", "management.ValidateJoin", "analysis.PairwisePolygonNeighbors", "analysis.AssignWeightsByPairwiseComparison", "management.FieldStatisticsToTable", "gp.Sample_sa", "management.CopyRows", "gp.ZonalHistogram_sa", "gp.TabulateArea_sa", "gp.ZonalGeometryAsTable_sa", "gp.ZonalStatisticsAsTable_sa", "management.CheckGeometry", "analysis.Statistics", "analysis.Frequency", "analysis.GenerateNearTable", "management.FindIdentical",
             "management.TransposeFields", "management.PivotTable", "management.CreateTable", "analysis.PolygonNeighbors", "analysis.TabulateIntersection", "analysis.TableSelect"}
# Tools whose ArcGIS output must be a geodatabase feature class or memory (copied to a shapefile).
MEMORY_OUT = {"analysis.SummarizeWithin", "analysis.SummarizeNearby", "analysis.ApportionPolygon"}
# Tools that take the output folder and name rather than a path.
CREATE_TOOLS = {"management.CreateRandomPoints", "management.CreateFeatureclass", "management.CreateTable"}

# In-place tools: the index of the input they edit (run on a copy written as the output).
IN_PLACE = {"gp.AddSurfaceInformation_sa": 0, "management.CalculateGeometryAttributes": 0, "management.Adjust3DZ": 0, "management.MigrateDateFieldToHighPrecision": 0, "management.EnableCOGO": 0, "management.DisableEditorTracking": 0, "management.DisableCOGO": 0, "management.AddGPSMetadataFields": 0, "management.ConvertTimeZone": 0, "management.ConvertTimeField": 0, "management.AddSpatialJoin": 0, "gp.ExtractMultiValuesToPoints_sa": 0, "edit.ExtendLine": 0, "edit.FlipLine": 0, "edit.TrimLine": 0, "edit.Generalize": 0, "management.CalculateField": 0, "management.AddField": 0, "management.DeleteField": 0, "management.DeleteIdentical": 0, "analysis.Near": 0, "management.Append": 1, "management.RepairGeometry": 0, "management.AddXY": 0, "management.EncodeField": 0, "management.AddGeometryAttributes": 0,
            "management.CalculateFields": 0, "management.StandardizeField": 0, "management.TransformField": 0, "management.ReclassifyField": 0, "management.DefineProjection": 0, "management.CalculateEndTime": 0}

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


def annotate():
    """Adds the `run` entries to the manifest without running ArcGIS."""
    path = OUT / "manifest.json"
    manifest = json.loads(path.read_text())
    for case in manifest["cases"].values():
        if "args" not in case:
            continue  # cases recorded by their own runner (Add Global IDs)
        args = case["args"]
        if case.get("schema_edit"):
            args = [args[0], "<out>", *args[1:]]
        spec = run_spec(case["tool"], args)
        if spec:
            case["run"] = spec
    path.write_text(json.dumps(manifest, indent=2) + "\n")


def delete_multiple_case(w, template):
    """Destructive tools run exclusively against a disposable copy of their fixture."""
    source = DATA / "delete_multiple"
    with tempfile.TemporaryDirectory(prefix="unigis-delete-reference-") as temp:
        copied = Path(temp) / "workspace"
        shutil.copytree(source, copied, ignore=shutil.ignore_patterns("*.lock"))
        actual = template.replace("<workspace>", win(copied))
        r = w.call("run_tool", tool="management.DeleteMultiple", args=[actual])
        response = w.call("python", code=f"""
import os
folder = {win(copied)!r}
files = []
catalogs = {{}}
for base, dirs, names in os.walk(folder):
    for name in list(dirs):
        if name.lower().endswith('.gdb'):
            gdb = os.path.join(base,name)
            rel = os.path.relpath(gdb,folder).replace(chr(92),'/')
            files.append(rel+'/')
            catalogs[rel] = sorted(c.name for c in arcpy.Describe(gdb).children)
            dirs.remove(name)
    files.extend(os.path.relpath(os.path.join(base,n),folder).replace(chr(92),'/') for n in names if not n.endswith('.lock'))
result = dict(files=sorted(files),catalogs=catalogs)
""")
        return dict(tool="management.DeleteMultiple",args=[template],status=r.get("status"),
            messages=[m["text"] for m in r.get("messages",[]) if m["severity"]>0],
            catalog_edit=dict(source=rel(source),expected=response["result"]),
            run=run_spec("management.DeleteMultiple",[template]))


def global_ids_case(w, tool, before):
    """Random UUID values use a strict schema/identity contract, with idempotence."""
    source_gdb = Path(before[0]).parent
    with tempfile.TemporaryDirectory(prefix="unigis-globalids-reference-") as temp:
        copied = Path(temp) / source_gdb.name
        shutil.copytree(source_gdb,copied,ignore=shutil.ignore_patterns("*.lock"))
        targets = [copied / Path(p).name for p in before]
        response = w.call("run_tool",tool=tool,args=[";".join(map(str,targets))])
        entry = dict(tool=tool,status=response.get("status"),global_ids_edit=True,
                     messages=[m["text"] for m in response.get("messages",[]) if m["severity"]>0],
                     run=run_spec(tool,[";".join(rel(p) for p in before)]))
        if entry["status"] == "succeeded":
            paths = []
            for target in targets:
                d = w.call("describe",path=str(target))
                if d.get("dataType") == "FeatureDataset":
                    paths.append(target / "child")
                else:
                    paths.append(target)
            code = f"""
result = {{}}
for path in {list(map(win,paths))!r}:
 d=arcpy.Describe(path)
 fields=[dict(name=f.name,type=f.type,nullable=f.isNullable,required=f.required,editable=f.editable,length=f.length) for f in arcpy.ListFields(path)]
 ids=[r[0] for r in arcpy.da.SearchCursor(path,[d.globalIDFieldName])]
 result[os.path.basename(path)]=dict(fields=fields,globalIDFieldName=d.globalIDFieldName,hasGlobalID=d.hasGlobalID,ids=ids,indexes=[dict(name=i.name,fields=[f.name for f in i.fields]) for i in arcpy.ListIndexes(path)])
"""
            first=w.call("python",code="import os\n"+code)["result"]
            repeat=w.call("run_tool",tool=tool,args=[";".join(map(str,targets))])
            if repeat.get("status")!="succeeded" or w.call("python",code=code)["result"] != first:
                raise RuntimeError("GlobalID repeat changed identities or schema")
            entry["targets"]=first
        return entry


def subset_case(w, name, tool, before, after):
    """Save both sides of a random record partition, without demanding RNG parity."""
    source = Path(before[0])
    table = source.name == "records"
    folder = OUT / name
    folder.mkdir(exist_ok=False)
    extension = "dbf" if table else "shp"
    train = folder / f"training.{extension}"
    test = folder / f"testing.{extension}"
    requested, size, units = after
    r = w.call("run_tool", tool=tool, args=[str(source), str(train), str(test) if requested else "", size, units])
    args = [rel(source), f"<out>/training.{extension}", f"<out>/testing.{extension}" if requested else "", size, units]
    return dict(tool=tool,args=args,status=r.get("status"),subset_partition=True,table_output=table,
                messages=[m["text"] for m in r.get("messages",[]) if m["severity"]>0],run=run_spec(tool,args))


WORKSPACE_CREATE = {"management.CreateFolder", "management.CreateFileGDB", "management.CreateMobileGDB"}


def workspace_create_case(w, tool, template):
    """Create Folder / File GDB / Mobile GDB in a disposable copy of their fixture folder: the
    folder's listing afterwards and the derived output, both relative to the copy."""
    source = DATA / "workspace_create"
    with tempfile.TemporaryDirectory(prefix="unigis-create-reference-") as temp:
        copied = Path(temp) / "workspace"
        shutil.copytree(source, copied)
        actual = [a.replace("<workspace>", str(copied)) for a in template]
        r = w.call("run_tool", tool=tool, args=actual)
        out = r.get("outputs") or []
        derived = str(out[0]).replace("//", "/") if r.get("status") == "succeeded" and out else None
        wine_derived = win(derived) if derived else None
        response = w.call("python", code=f"""
import os
arcpy.management.ClearWorkspaceCache()
folder = {win(copied)!r}
derived = {wine_derived!r}
entries = []
for base, dirs, names in os.walk(folder):
    for name in list(dirs):
        entries.append(os.path.relpath(os.path.join(base, name), folder).replace(chr(92), '/') + '/')
        if name.lower().endswith('.gdb'):
            dirs.remove(name)
    for name in names:
        if not name.endswith(('.lock', '-shm', '-wal', '-conch')):
            entries.append(os.path.relpath(os.path.join(base, name), folder).replace(chr(92), '/'))
workspace = None
if derived and derived.lower().endswith(('.gdb', '.geodatabase')):
    d = arcpy.Describe(derived)
    workspace = dict(children=sorted(c.name for c in d.children), factory=d.workspaceFactoryProgID)
arcpy.management.ClearWorkspaceCache()
result = dict(entries=sorted(entries), workspace=workspace)
""")
        output = derived.replace(str(copied), "<workspace>") if derived else None
        return dict(tool=tool, args=template, status=r.get("status"),
                    messages=[m["text"] for m in r.get("messages", []) if m["severity"] > 0],
                    workspace_create=dict(source=rel(source), expected=response["result"], output=output),
                    run=run_spec(tool, template))


# Geodatabase scenarios: tool steps run one after the other on a new file geodatabase, then the
# geodatabase's whole schema compared. Each step: (arcpy tool, keyword arguments, registry id,
# registry parameters replacing some of the keywords). `<ws>` is a new, empty folder.
GDB = "<ws>/t.gdb"
GDB_SCENARIOS = {
    "gsc_build": [
        ("management.CreateFileGDB", dict(out_folder_path="<ws>", out_name="t"), "create_file_gdb", None),
        ("management.CreateFeatureDataset", dict(out_dataset_path=GDB, out_name="fds", spatial_reference=4326), "create_feature_dataset", None),
        ("management.CreateFeatureclass", dict(out_path=GDB + "/fds", out_name="pts", geometry_type="POINT", spatial_reference=4326), "create_feature_class", dict(output=GDB + "/fds/pts", geometry_type="POINT", spatial_reference="4326")),
        ("management.CreateFeatureclass", dict(out_path=GDB, out_name="pol", geometry_type="POLYGON", spatial_reference=4326), "create_feature_class", dict(output=GDB + "/pol", geometry_type="POLYGON", spatial_reference="4326")),
        ("management.CreateTable", dict(out_path=GDB, out_name="tab"), "create_table", dict(output=GDB + "/tab")),
        ("management.AddField", dict(in_table=GDB + "/fds/pts", field_name="kind", field_type="SHORT"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/fds/pts", field_name="label", field_type="TEXT", field_length=40, field_alias="A label"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="pid", field_type="LONG"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="pid", field_type="LONG"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="tmp", field_type="DOUBLE"), "add_field", None),
        ("management.DeleteField", dict(in_table=GDB + "/tab", drop_field="tmp"), "delete_field", None),
        ("management.CreateDomain", dict(in_workspace=GDB, domain_name="kinds", domain_description="Kinds"), "create_domain", None),
        ("management.AddCodedValueToDomain", dict(in_workspace=GDB, domain_name="kinds", code="1", code_description="One"), "add_coded_value_to_domain", None),
        ("management.AddCodedValueToDomain", dict(in_workspace=GDB, domain_name="kinds", code="2", code_description="Two"), "add_coded_value_to_domain", None),
        ("management.AddCodedValueToDomain", dict(in_workspace=GDB, domain_name="kinds", code="3", code_description="Three"), "add_coded_value_to_domain", None),
        ("management.DeleteCodedValueFromDomain", dict(in_workspace=GDB, domain_name="kinds", code="3"), "delete_coded_value_from_domain", None),
        ("management.SortCodedValueDomain", dict(in_workspace=GDB, domain_name="kinds", sort_by="DESCRIPTION", sort_order="DESCENDING"), "sort_coded_value_domain", None),
        ("management.AlterDomain", dict(in_workspace=GDB, domain_name="kinds", new_domain_description="Kinds of points"), "alter_domain", None),
        ("management.CreateDomain", dict(in_workspace=GDB, domain_name="rng", domain_description="Range", field_type="LONG", domain_type="RANGE"), "create_domain", None),
        ("management.SetValueForRangeDomain", dict(in_workspace=GDB, domain_name="rng", min_value="0", max_value="100"), "set_value_for_range_domain", None),
        ("management.AssignDomainToField", dict(in_table=GDB + "/fds/pts", field_name="kind", domain_name="kinds"), "assign_domain_to_field", None),
        ("management.SetSubtypeField", dict(in_table=GDB + "/fds/pts", field="kind"), "set_subtype_field", None),
        ("management.AddSubtype", dict(in_table=GDB + "/fds/pts", subtype_code=1, subtype_description="One"), "add_subtype", None),
        ("management.AddSubtype", dict(in_table=GDB + "/fds/pts", subtype_code=2, subtype_description="Two"), "add_subtype", None),
        ("management.AddSubtype", dict(in_table=GDB + "/fds/pts", subtype_code=1, subtype_description="Again"), "add_subtype", None),
        ("management.SetDefaultSubtype", dict(in_table=GDB + "/fds/pts", subtype_code=2), "set_default_subtype", None),
        ("management.AssignDefaultToField", dict(in_table=GDB + "/fds/pts", field_name="label", default_value="none"), "assign_default_to_field", None),
        ("management.AddIndex", dict(in_table=GDB + "/tab", fields="pid", index_name="pid_idx"), "add_index", None),
        ("management.AlterField", dict(in_table=GDB + "/tab", field="pid", new_field_name="parent", new_field_alias="Parent"), "alter_field", None),
        ("management.CreateRelationshipClass", dict(origin_table=GDB + "/fds/pts", destination_table=GDB + "/tab", out_relationship_class=GDB + "/pts_tab", relationship_type="SIMPLE", forward_label="has", backward_label="of", message_direction="NONE", cardinality="ONE_TO_MANY", attributed="NONE", origin_primary_key="OBJECTID", origin_foreign_key="parent"), "create_relationship_class", None),
        ("management.DomainToTable", dict(in_workspace=GDB, domain_name="kinds", out_table="<ws>/kinds.dbf", code_field="code", description_field="descr"), "domain_to_table", dict(in_workspace=GDB, domain_name="kinds", output="<ws>/kinds.dbf", code_field="code", description_field="descr")),
        ("management.CreateFeatureDataset", dict(out_dataset_path=GDB, out_name="fds", spatial_reference=4326), "create_feature_dataset", None),
    ],
    "gsc_edit": [
        ("management.CreateFileGDB", dict(out_folder_path="<ws>", out_name="t"), "create_file_gdb", None),
        ("management.CreateTable", dict(out_path=GDB, out_name="tab"), "create_table", dict(output=GDB + "/tab")),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="a", field_type="SHORT"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="b", field_type="TEXT", field_alias="Bee"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="c", field_type="DOUBLE", field_is_nullable="NON_NULLABLE"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="d", field_type="BIGINTEGER"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="e", field_type="GUID"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="f", field_type="DATEONLY"), "add_field", None),
        ("management.DeleteField", dict(in_table=GDB + "/tab", drop_field="a;b;d;e;f", method="KEEP_FIELDS"), "delete_field", None),
        ("management.CreateDomain", dict(in_workspace=GDB, domain_name="dom", field_type="SHORT"), "create_domain", None),
        ("management.AssignDomainToField", dict(in_table=GDB + "/tab", field_name="a", domain_name="dom"), "assign_domain_to_field", None),
        ("management.DeleteDomain", dict(in_workspace=GDB, domain_name="dom"), "delete_domain", None),
        ("management.RemoveDomainFromField", dict(in_table=GDB + "/tab", field_name="a"), "remove_domain_from_field", None),
        ("management.DeleteDomain", dict(in_workspace=GDB, domain_name="dom"), "delete_domain", None),
        ("management.AddIndex", dict(in_table=GDB + "/tab", fields="a", index_name="a_idx"), "add_index", None),
        ("management.AddIndex", dict(in_table=GDB + "/tab", fields="d", index_name="d_idx"), "add_index", None),
        ("management.RemoveIndex", dict(in_table=GDB + "/tab", index_name="a_idx"), "remove_index", None),
        ("management.SetSubtypeField", dict(in_table=GDB + "/tab", field="a"), "set_subtype_field", None),
        ("management.AddSubtype", dict(in_table=GDB + "/tab", subtype_code=1, subtype_description="x"), "add_subtype", None),
        ("management.AddSubtype", dict(in_table=GDB + "/tab", subtype_code=2, subtype_description="y"), "add_subtype", None),
        ("management.RemoveSubtype", dict(in_table=GDB + "/tab", subtype_code="1"), "remove_subtype", None),
        ("management.AlterField", dict(in_table=GDB + "/tab", field="b", clear_field_alias="CLEAR_ALIAS"), "alter_field", None),
        ("management.AlterField", dict(in_table=GDB + "/tab", field="f", new_field_name="g", new_field_alias="Gee"), "alter_field", None),
        ("management.AddField", dict(in_table=GDB + "/tab", field_name="9bad", field_type="LONG"), "add_field", None),
        ("management.AddField", dict(in_table=GDB + "/missing", field_name="z", field_type="LONG"), "add_field", None),
    ],
}

GDB_SNAPSHOT = """
def gdb_snapshot(gdb):
    arcpy.management.ClearWorkspaceCache()
    out = dict(items=[], tables={}, domains=[], relationships=[])
    def walk(desc, prefix):
        for c in desc.children:
            out["items"].append(prefix + c.name + ("/" if c.dataType == "FeatureDataset" else ""))
            path = c.catalogPath
            if c.dataType == "FeatureDataset":
                walk(c, prefix + c.name + "/")
            elif c.dataType in ("FeatureClass", "Table"):
                subtypes = [dict(code=k, name=v["Name"], default=v["Default"], field=v["SubtypeField"])
                            for k, v in sorted(arcpy.da.ListSubtypes(path).items()) if v["SubtypeField"]]
                out["tables"][c.name] = dict(
                    fields=[dict(name=f.name, type=f.type, length=f.length, alias=f.aliasName, domain=f.domain,
                                 default=f.defaultValue, nullable=f.isNullable) for f in arcpy.ListFields(path)],
                    indexes=sorted([i.name, [f.name for f in i.fields]] for i in arcpy.ListIndexes(path)
                                   if not i.name.upper().startswith("FDO_")),
                    subtypes=subtypes)
            elif c.dataType == "RelationshipClass":
                out["relationships"].append(dict(name=c.name, cardinality=c.cardinality,
                    origin=list(c.originClassNames), destination=list(c.destinationClassNames),
                    forward=c.forwardPathLabel, backward=c.backwardPathLabel, composite=c.isComposite,
                    notification=c.notification, keys=[[k[0], k[1]] for k in c.originClassKeys] + [[k[0], k[1]] for k in c.destinationClassKeys]))
    walk(arcpy.Describe(gdb), "")
    out["items"].sort()
    for d in sorted(arcpy.da.ListDomains(gdb), key=lambda d: d.name):
        out["domains"].append(dict(name=d.name, description=d.description, type=d.type, kind=d.domainType,
            split=d.splitPolicy, merge=d.mergePolicy,
            values=[[str(k), v] for k, v in d.codedValues.items()] if d.domainType == "CodedValue" else [str(v) for v in d.range]))
    arcpy.management.ClearWorkspaceCache()
    return out
"""


def gdb_scenario_case(w, steps):
    """Runs the steps in ArcGIS in a new folder; records each step's status and messages, then
    the geodatabase's schema."""
    with tempfile.TemporaryDirectory(prefix="unigis-gdb-scenario-") as temp:
        place = lambda v: v.replace("<ws>", temp) if isinstance(v, str) else v
        recorded = []
        for tool, kwargs, rid, params in steps:
            r = w.call("run_tool", tool=tool, kwargs={k: place(v) for k, v in kwargs.items()})
            run = {k: str(v) for k, v in (params or kwargs).items()}
            recorded.append(dict(tool=tool, status=r.get("status"),
                                 messages=[m["text"].replace(temp, "<ws>") for m in r.get("messages", []) if m["severity"] > 1],
                                 run=dict(tool=rid, params=run)))
        response = w.call("python", code=GDB_SNAPSHOT + f"\nresult = gdb_snapshot({win(Path(temp) / 't.gdb')!r})")
        if "result" not in response:
            raise RuntimeError(f"snapshot failed: {response}")
        return dict(tool="scenario", gdb_scenario=dict(steps=recorded, snapshot=response["result"]))


def feature_import_case(w, name, tool, before):
    """Retain complete ArcGIS output and immutable setup workspace for batch imports."""
    kind = name.split("_", 1)[1]
    seeded = kind in {"collision", "dataset"}
    seed = DATA / ("fctg_seed.gdb" if seeded else "fctg_empty.gdb")
    suffix = "/group" if kind == "dataset" else ""
    out = OUT / f"{name}.gdb"
    if out.exists():
        shutil.rmtree(out)
    missing = kind == "missing_output"
    if not missing:
        w.call("run_tool", tool="management.Copy", args=[str(seed), str(out)])
    target = str(out) + suffix
    r = w.call("run_tool", tool=tool, args=[before[0], target])
    args = [";".join(rel(p) for p in before[0]), "<out>" + suffix]
    if out.exists():
        for lock in out.glob("*.lock"):
            try:
                lock.unlink()
            except PermissionError:
                pass
    return dict(tool=tool, args=args, status=r.get("status"),
                workspace_reference=dict(seed=None if missing else rel(seed)),
                messages=[m["text"] for m in r.get("messages", []) if m["severity"] > 0],
                run=run_spec(tool, args))


def extract_data_case(w, name, tool, before, after):
    """Public workspace output retained for portable native schema/value comparison."""
    extract, out_type, expand, related, all_rows = after
    out = OUT / f"{name}.gdb"
    if out.exists():
        shutil.rmtree(out)
    if out_type == "GEODATABASE":
        w.call("run_tool", tool="management.CreateFileGDB", args=[str(OUT), out.name])
    public = [before[0].replace(str(ROOT), win(ROOT)).replace("/", chr(92)),
              extract, out_type, str(out) if out_type == "GEODATABASE" else "", "",
              str(OUT) if out_type != "GEODATABASE" else "", out.name if out_type != "GEODATABASE" else "",
              expand, "DO_NOT_REUSE", related, "", "INTERSECTS", all_rows]
    r = w.call("run_tool", tool=tool, args=public)
    args = [rel(before[0]), extract, out_type, "<out>" if out_type == "GEODATABASE" else "", "",
            "<folder>" if out_type != "GEODATABASE" else "", "<name>" if out_type != "GEODATABASE" else "",
            expand, "DO_NOT_REUSE", related, "", "INTERSECTS", all_rows]
    if out.exists():
        # ArcGIS locks are process-local and must not become reference artifacts.
        for lock in out.glob("*.lock"):
            try:
                lock.unlink()
            except PermissionError:
                pass
    return dict(tool=tool, args=args, status=r.get("status"), workspace_extract=True,
                messages=[m["text"] for m in r.get("messages", []) if m["severity"] > 0],
                run=run_spec(tool,args))


def schema_edit_case(w, tool, before, after):
    """Save geodatabase schema-only edits as portable snapshots, retaining the source."""
    source = Path(before[0])
    with tempfile.TemporaryDirectory(prefix="unigis-schema-reference-") as temp:
        copied = Path(temp) / source.parent.name
        shutil.copytree(source.parent, copied, ignore=shutil.ignore_patterns("*.lock"))
        target = copied / source.name
        args = [str(target)] + after
        preserve_code = f"""
preserved_fields = [f.name for f in arcpy.ListFields({win(target)!r}) if f.type != 'Geometry']
preserved_fields += ['SHAPE@JSON'] if hasattr(arcpy.Describe({win(target)!r}), 'shapeType') else []
preserved_rows = list(arcpy.da.SearchCursor({win(target)!r}, preserved_fields))
"""
        w.call("python", code=preserve_code)
        r = w.call("run_tool", tool=tool, args=args, env={"overwriteOutput": True})
        entry = {
            "tool": tool, "args": [rel(source)] + after, "status": r.get("status"),
            "messages": [m["text"] for m in r.get("messages", []) if m["severity"] > 0],
            "schema_edit": True,
        }
        if r.get("status") == "succeeded":
            described = w.call("describe", path=str(target))
            entry["schema"] = {k: described[k] for k in ("fields", "shapeType", "hasZ", "hasM", "OIDFieldName", "shapeFieldName", "spatialReference") if k in described}
            code = f"""
result = []
for d in arcpy.da.ListDomains({win(copied)!r}):
    result.append(dict(name=d.name,description=d.description,type=d.type,kind=d.domainType,
        split=d.splitPolicy,merge=d.mergePolicy,
        values=d.codedValues if d.domainType=='CodedValue' else list(d.range)))
"""
            response = w.call("python", code=code)
            entry["domains"] = response["result"]
            if tool in ("management.DisableCOGO", "management.EnableCOGO"):
                flags = w.call("python", code=f"result = dict(enabled=arcpy.Describe({win(target)!r}).isCOGOEnabled, required={{f.name:f.required for f in arcpy.ListFields({win(target)!r}) if f.name.lower() in ['direction','distance','radius','arclength','radius2']}})")
                entry["cogo"] = flags["result"]
            if tool in ("management.DisableEditorTracking", "management.EnableEditorTracking"):
                tracking = w.call("python", code=f"d=arcpy.Describe({win(target)!r}); result = dict(properties={{p:getattr(d,p) for p in ['editorTrackingEnabled','creatorFieldName','createdAtFieldName','editorFieldName','editedAtFieldName','isTimeInUTC']}}, editable={{f.name:f.editable for f in arcpy.ListFields({win(target)!r})}})")
                entry["editor_tracking"] = tracking["result"]
            if tool == "management.MigrateDateFieldToHighPrecision":
                precision = w.call("python", code=f"result = {{f.name:f.precision for f in arcpy.ListFields({win(target)!r}) if f.type=='Date'}}")
                entry["date_precision"] = precision["result"]
            # A second call must be idempotent; snapshot both schema and domains.
            repeated = w.call("run_tool", tool=tool, args=args, env={"overwriteOutput": True})
            if repeated.get("status") != "succeeded":
                raise RuntimeError(f"repeat schema edit failed: {repeated}")
            repeated_schema = w.call("describe", path=str(target))
            for key in ("fields", "shapeType", "hasZ", "hasM", "OIDFieldName", "shapeFieldName"):
                if repeated_schema.get(key) != entry["schema"].get(key):
                    raise RuntimeError(f"repeated schema edit changed {key}")
            if w.call("python", code=code)["result"] != entry["domains"]:
                raise RuntimeError("repeated schema edit changed domains")
            preserved = w.call("python", code=f"result = preserved_rows == list(arcpy.da.SearchCursor({win(target)!r}, preserved_fields))")
            if preserved["result"] is not True:
                raise RuntimeError("schema edit changed existing rows or geometries")
        entry["run"] = run_spec(tool, [rel(source), "<out>"] + after)
        return entry


def main():
    import fnmatch
    if sys.argv[1:] == ["--annotate"]:
        return annotate()
    # Arguments may be glob patterns, such as `blk_*`.
    only = {n for a in sys.argv[1:] for n in (fnmatch.filter([*CASES, *SELECT, *GDB_SCENARIOS], a) or [a])}
    unknown = only - set(CASES) - set(SELECT) - set(GDB_SCENARIOS)
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
                if f.is_dir():
                    shutil.rmtree(f)
                else:
                    f.unlink()
            if (OUT / name).is_dir():
                shutil.rmtree(OUT / name)
    else:
        if OUT.exists():
            shutil.rmtree(OUT)
        OUT.mkdir(parents=True)
    for name, steps in GDB_SCENARIOS.items():
        if only and name not in only:
            continue
        manifest["cases"][name] = gdb_scenario_case(w, steps)
        print(f"{name}: {[s['status'] for s in manifest['cases'][name]['gdb_scenario']['steps']]}")
    for name, (tool, before, after) in CASES.items():
        if only and name not in only:
            continue
        if tool in ("gp.ZonalHistogram_sa", "gp.TabulateArea_sa", "gp.ZonalGeometryAsTable_sa", "gp.Lookup_sa"):
            # These tools treat an integer raster as discrete only when it has an attribute table
            # (without one ZonalHistogram bins the values into 256 classes) and name columns after
            # its value field: build the tables first. The inputs are copies (zt_*, tab_*) because
            # the sidecar files change what other tools infer from a raster.
            for a in before:
                if isinstance(a, Path) and a.suffix == ".tif":
                    w.call("python", code=f"arcpy.management.BuildRasterAttributeTable(r'{win(a)}', 'Overwrite')")
        if tool in ("conversion.FeatureClassToGeodatabase", "conversion.TableToGeodatabase"):
            manifest["cases"][name] = feature_import_case(w, name, tool, before)
            print(f"{name}: {manifest['cases'][name]['status']}")
            continue
        if tool == "management.AddGlobalIDs":
            manifest["cases"][name] = global_ids_case(w,tool,before)
            print(f"{name}: {manifest['cases'][name]['status']}")
            continue
        if tool == "management.SubsetFeatures":
            manifest["cases"][name] = subset_case(w, name, tool, before, after)
            print(f"{name}: {manifest['cases'][name]['status']}")
            continue
        if tool == "management.ExtractDataFromGeodatabase":
            manifest["cases"][name] = extract_data_case(w, name, tool, before, after)
            print(f"{name}: {manifest['cases'][name]['status']}")
            continue
        if tool in WORKSPACE_CREATE:
            manifest["cases"][name] = workspace_create_case(w, tool, before)
            print(f"{name}: {manifest['cases'][name]['status']}")
            continue
        if tool == "management.DeleteMultiple":
            manifest["cases"][name] = delete_multiple_case(w, before[0])
            print(f"{name}: {manifest['cases'][name]['status']}")
            continue
        if tool in ("management.AddGPSMetadataFields", "management.DisableCOGO", "management.EnableCOGO", "management.DisableEditorTracking", "management.EnableEditorTracking", "management.MigrateDateFieldToHighPrecision") and Path(before[0]).parent.suffix == ".gdb":
            manifest["cases"][name] = schema_edit_case(w, tool, before, after)
            print(f"{name}: {manifest['cases'][name]['status']}")
            continue
        out = OUT / (f"{name}.dbf" if tool in TABLE_OUT else f"{name}.shp")
        if tool in ("management.BatchUpdateFields", "management.ConvertTimeField", "management.ConvertTimeZone") and str(before[0]).endswith(".dbf"):
            out = OUT / f"{name}.dbf"  # a table in, a table out
        raster_projection = tool == "management.DefineProjection" and Path(str(before[0])).suffix.lower() in (".tif", ".tiff")
        if tool in RASTER_OUT or raster_projection:
            out = OUT / f"{name}.tif"
        if tool in EXCEL_OUT:
            out = OUT / f"{name}.xlsx"
        if tool in TEXT_OUT:
            out = OUT / (f"{name}.geojson" if "GEOJSON" in after else f"{name}.json")
            if tool == "conversion.ExportTable":
                out = OUT / f"{name}.csv"
            if tool == "conversion.RasterToASCII":
                out = OUT / f"{name}.asc"
            if tool == "conversion.RasterToFloat":
                out = OUT / f"{name}.flt"
        if tool in WORKSPACE_OUT:
            out = OUT / name
            out.mkdir()
        if tool in WORKSPACE_OUT | EXCEL_OUT:
            # A list argument is a multivalue (several input datasets); the bridge translates
            # each path of a list, not paths inside a joined string.
            args = [list(map(str, a)) if isinstance(a, list) else str(a) for a in before] + [str(out)] + [str(a) if isinstance(a, Path) else a for a in after]
        elif tool in ("cartography.GridIndexFeatures", "gp.CreateConstantRaster_sa", "management.GenerateTessellation"):
            args = [str(out)] + [str(a) if isinstance(a, Path) else a for a in before + after]
        elif tool == "management.FieldStatisticsToTable":
            # Writes <input>_FieldStatisticsToTable_<type> into the output location: run into
            # memory, then copy that table to the output below.
            args = [str(before[0]), after[0], "memory", after[1]] + after[2:]
        elif tool in IN_PLACE:
            k = IN_PLACE[tool]
            if tool in ("management.DisableCOGO", "management.EnableCOGO", "management.DisableEditorTracking", "management.MigrateDateFieldToHighPrecision"):
                source = Path(before[k])
                for sidecar in source.parent.glob(source.stem + ".*"):
                    if sidecar.is_file() and not sidecar.name.endswith(".lock"):
                        shutil.copy2(sidecar, out.with_name(out.stem + sidecar.name[len(source.stem):]))
            elif raster_projection:
                # CopyRaster can replace the NoData value; preserve source bytes.
                shutil.copy2(before[k], out)
                aux = Path(str(before[k]) + ".aux.xml")
                if aux.exists():
                    shutil.copy2(aux, Path(str(out) + ".aux.xml"))
            else:
                copy_tool = "management.CopyRows" if str(before[k]).endswith(".dbf") else "management.CopyFeatures"
                w.call("run_tool", tool=copy_tool, args=[str(before[k]), str(out)],
                       env={"overwriteOutput": True})
            args = [str(out) if i == k else (a if isinstance(a, (list, int, float)) else str(a)) for i, a in enumerate(before)] + after
        elif tool == "management.MosaicToNewRaster":
            args = [before[0], str(out.parent), out.name] + after
        elif tool in CREATE_TOOLS:
            args = [str(out.parent), out.name] + [str(a) if isinstance(a, Path) else a for a in before + after]
        elif tool == "conversion.FeatureClassToFeatureClass":
            # (in_features, out_path, out_name, where_clause, field_mapping)
            args = [str(before[0]), str(out.parent), out.stem] + after
        else:
            args = [str(a) for a in before] + [str(out)] + [str(a) if isinstance(a, Path) else a for a in after]
        if tool in MEMORY_OUT:
            # These tools write only to a geodatabase or memory: run into memory, then copy.
            mem = f"memory/{name}"
            args = [mem if a == str(out) else a for a in args]
        if tool in {"conversion.ASCIIToRaster", "conversion.FloatToRaster"}:
            # A failed FloatToRaster can delete its input .hdr. Never let an
            # oracle run mutate the checked-in source fixture or its sidecars.
            source = Path(before[0])
            with tempfile.TemporaryDirectory(prefix="unigis-grid-reference-") as temporary:
                copied = Path(temporary) / source.name
                shutil.copy2(source, copied)
                for suffix in (".hdr", ".HDR", ".prj", ".PRJ"):
                    sidecar = source.with_suffix(suffix)
                    if sidecar.is_file():
                        shutil.copy2(sidecar, copied.with_suffix(suffix))
                safe_args = [str(copied) if a == str(source) else a for a in args]
                r = w.call("run_tool", tool=tool, args=safe_args, env={"overwriteOutput": True})
        else:
            r = w.call("run_tool", tool=tool, args=args, env={"overwriteOutput": True})
        if tool == "management.FieldStatisticsToTable" and r.get("status") == "succeeded":
            w.call("python", code=(
                "arcpy.env.workspace = 'memory'\n"
                "tb = [t for t in arcpy.ListTables() if 'FieldStatisticsToTable' in t]\n"
                f"arcpy.management.CopyRows('memory/' + tb[0], r'{out}')\n"
                "for t in tb: arcpy.management.Delete('memory/' + t)\n"))
        if tool in MEMORY_OUT and r.get("status") == "succeeded":
            r2 = w.call("run_tool", tool="management.CopyFeatures", args=[mem, str(out)], env={"overwriteOutput": True})
            w.call("python", code=f"arcpy.management.Delete('{mem}')")
            if r2.get("status") != "succeeded":
                r = r2
        if tool in WORKSPACE_OUT:
            outputs = {}
            datasets = [*out.glob("*.shp"), *(p for p in out.glob("*.dbf") if not p.with_suffix(".shp").exists()), *out.glob("*.tif")]
            for shp in sorted(datasets):
                d = w.call("describe", path=str(shp))
                outputs[shp.name] = {
                    "fields": [[f["name"], f["type"], f["length"], f["precision"], f["scale"]] for f in d.get("fields", [])],
                    "shapeType": d.get("shapeType"),
                }
            manifest["cases"][name] = {
                "tool": tool,
                "args": [";".join(rel(x) for x in a) if isinstance(a, list) else str(Path(a).relative_to(ROOT)) if isinstance(a, Path) else a for a in before] + ["<out>"] + after,
                "status": r.get("status"),
                "messages": [m["text"] for m in r.get("messages", []) if m["severity"] > 0],
                "outputs": outputs,
            }
            print(f"{name}: {r.get('status')} {sorted(outputs)} {manifest['cases'][name]['messages']}")
            continue
        d = w.call("describe", path=str(out)) if r.get("status") == "succeeded" and tool not in TEXT_OUT | RASTER_OUT | EXCEL_OUT and not raster_projection else {}
        manifest["cases"][name] = {
            "tool": tool,
            "args": [";".join(rel(x) for x in a) if isinstance(a, list) else rel(a) for a in before] + ["<out>"] + [rel(a) for a in after],
            "status": r.get("status"),
            "messages": [m["text"] for m in r.get("messages", []) if m["severity"] > 0],
            "fields": [[f["name"], f["type"], f["length"], f["precision"], f["scale"]] for f in d.get("fields", [])],
            "shapeType": d.get("shapeType"),
        }
        if tool in TABLE_OUT:
            manifest["cases"][name]["table_output"] = True
        if tool in EXCEL_OUT:
            manifest["cases"][name]["excel_output"] = True
        if tool == "management.CreateSpatialSamplingLocations" and (not after[0] or after[0] != "SYSTEMATIC"):
            manifest["cases"][name]["stochastic_sampling"] = True
        if tool == "management.CreateRandomPoints":
            manifest["cases"][name]["stochastic_points"] = True
        if tool in ("management.DisableCOGO", "management.EnableCOGO", "management.DisableEditorTracking", "management.MigrateDateFieldToHighPrecision"):
            manifest["cases"][name]["report_in_place"] = True
        spec = run_spec(tool, manifest["cases"][name]["args"])
        if spec:
            manifest["cases"][name]["run"] = spec
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
