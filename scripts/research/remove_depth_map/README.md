# Remove Depth Map — deferred

Public black-box ArcGIS Pro 3.7 build 1901, Advanced licence, 2026-11-08.
[Tool documentation](https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/remove-depth-map.html).

The documented input is an adjusted mosaic dataset augmented by Compute Depth Map.
The tool removes per-image `.dm` files next to the source imagery; `where_clause`
selects mosaic items. It has no geoprocessing environments. This is an external-file
operation whose targets must be resolved from the mosaic's individual raster items.

## Controls and observations

Run the isolated probe and the metadata control, in that order:

```
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/remove_depth_map/probe.py
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/remove_depth_map/probe_history.py
```

The probe copies `testdata/raster/dem.tif` and `dem_nodata.tif` into ignored
`scratch/images/`, creates a fresh UTM 33N mosaic with those two items, and writes
invalid sentinel payloads named `dem.dm`, `dem_nodata.dm`, and `unrelated.dm` only
in that scratch directory. It never attempts removal against existing imagery or
geodatabases. The `.dm` sentinels are deliberately **not valid depth maps**.

`results.txt` records:

- ArcGIS item IDs 1 (`dem`) and 2 (`dem_nodata`).
- Empty selection (`OBJECTID < 0`), first-item selection, a Name selection, and default
  all-items selection succeed. Every sentinel stays byte-identical. This establishes
  no-op controls; it does **not** establish actual depth-map naming or deletion.
- An unknown query field fails with ERROR 160144 and changes no recorded files.
- An empty mosaic succeeds. An ordinary TIFF input fails with ERROR 999999 in this probe.
- Successful no-op calls change `GDB_Items` and `timestamps`, even though no imagery
  or sidecar bytes change. Other geodatabase file hashes remain unchanged in these calls.

`history_results.txt` isolates the `GDB_Items` change: the mosaic metadata gains a
`Process` entry naming RemoveDepthMap. Both tested settings of `arcpy.SetLogHistory`
(True and False) still append that entry. Other metadata logging settings were not tested.
The documentation's statement that the mosaic is otherwise unchanged does not imply
byte-identical geodatabase files when metadata logging occurs.

## Concrete prerequisites

The existing mosaic format blocker is reproduced by the native dump utility:

```
target/debug/examples/dump testdata/geoproc/md_items.gdb AMD_md_CAT
```

`native_catalog.txt` records the current result: the catalog opens and reports its
fields/three rows, but decoding each row fails (`count 56`, `54`, or `65 exceeds
remaining data`). The inline Raster/Uri layout needs to be decoded before evaluating
item queries and resolving each selected item's source/depth-map files. See
`../mosaic_datasets/README.md` for the saved format investigation. This is not a
request to use GDAL as a runtime or behavior reference.

No adjusted imagery mosaic with genuine Compute Depth Map output was available in
this investigation. Ordinary rasters plus dummy `.dm` names cannot resolve that
missing removal behavior. The tool is deferred in `docs/WORK_QUEUE_DEFERRED.txt`;
no registry entry or success-only implementation was added.

Resume only after native mosaic catalog rows and item paths can be read, and valid
depth-map controls can establish selected/all-item removal, preservation of unrelated
files, and behavior when a referenced sidecar is missing or cannot be removed.
