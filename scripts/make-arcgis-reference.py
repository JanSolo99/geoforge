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
    "merge_parcels_clip_source": ("management.Merge", [f"{DATA / 'parcels.shp'};{DATA / 'clip_areas.shp'}"], ["", "ADD_SOURCE_INFO"]),
}
IN_PLACE = {"analysis.Near"}


def main():
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    w = server.worker
    info = w.call("info")
    manifest = {"arcgis": {k: info.get(k) for k in ("product", "version", "build", "license")}, "cases": {}}
    for name, (tool, before, after) in CASES.items():
        out = OUT / f"{name}.shp"
        if tool in IN_PLACE:
            w.call("run_tool", tool="management.CopyFeatures", args=[str(before[0]), str(out)],
                   env={"overwriteOutput": True})
            args = [str(out)] + [str(a) for a in before[1:]] + after
        else:
            args = [str(a) for a in before] + [str(out)] + after
        r = w.call("run_tool", tool=tool, args=args, env={"overwriteOutput": True})
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
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    w.stop()


if __name__ == "__main__":
    main()
