"""Regenerates testdata/arcgis-reference/ by running ArcGIS Pro tools through the
ArcGIS bridge (tools/arcgis-bridge). Needs ArcGIS Pro on this machine:

    uv run --with "mcp>=2" python scripts/make-arcgis-reference.py

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
}
WORKSPACE_OUT = {"analysis.Split"}
# Tools whose output is a table, written as <name>.dbf.
TABLE_OUT = {"management.CheckGeometry"}

# In-place tools: the index of the input they edit (run on a copy written as the output).
IN_PLACE = {"analysis.Near": 0, "management.Append": 1, "management.RepairGeometry": 0}

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
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    w = server.worker
    info = w.call("info")
    manifest = {"arcgis": {k: info.get(k) for k in ("product", "version", "build", "license")}, "cases": {}}
    for name, (tool, before, after) in CASES.items():
        out = OUT / (f"{name}.dbf" if tool in TABLE_OUT else f"{name}.shp")
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
        d = w.call("describe", path=str(out)) if r.get("status") == "succeeded" else {}
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
        d = w.call("describe", path=str(out)) if r.get("status") == "succeeded" else {}
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
