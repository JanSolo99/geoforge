# Disable display rounding in the WKT file oracle

The coordinate-notation precision controls include real positions extremely close
to integer degrees. Copying them to a FileGDB at XY resolution 1e-9 succeeds and
the native reader retains those coordinates. GDAL 3.13's default WKT display can
round these values to an integer, producing a false file-oracle mismatch larger
than the storage resolution.

Isolated reproduction (from the 148-row precision reference):

```
./target/debug/ug copy-features testdata/arcgis-reference/ccn_grid_input_precision_ddm_1.shp /tmp/unigis-carry-wkt.gdb/copy
ogrinfo -q -geom=ISO_WKT -fields=NO -fid 71 /tmp/unigis-carry-wkt.gdb copy
# POINT (25.0 17.0)
ogrinfo --config OGR_WKT_ROUND NO -q -geom=ISO_WKT -fields=NO -fid 71 /tmp/unigis-carry-wkt.gdb copy
# POINT (25.0 16.999999998)
```

[GDAL's configuration documentation](https://gdal.org/en/stable/drivers/vector/csv.html#configuration-options)
describes the heuristic controlled by OGR_WKT_ROUND, enabled by default, that
removes trailing decimal runs resembling numerical noise. Disabling it preserves
the stored near-integer value in the observed WKT output. No source geometry,
storage resolution, comparison tolerance or geoprocessing arithmetic was changed.

`unigis-testkit::ogr_layer` now sets OGR_WKT_ROUND=NO only on its ogrinfo text
subprocess. Attributes still come from JSON; ISO WKT still preserves M/curve
structure. The normal comparisons apply to all coordinates and fields. No file
or record is exempted.

The focused `ogr_keeps_stored_coordinates_close_to_integers` regression writes
one known coordinate using GDAL OpenFileGDB at resolution 1e-9 and checks that the
file oracle retains it within 1e-10. GDAL is used only for file-format IO. It uses
-xyRes and is version-gated to GDAL 3.9+ (the current 3.13 run executed the test).
The four coordinate precision reference shapefiles independently cover this
case through the full native writer/reader/GDAL geodatabase-copy cross-check.

Initial failure: `/tmp/unigis-carry-check2.log`, four precision cases with nine
GDAL WKT mismatches each. Focused regression PASS:
`cargo test -j3 -p unigis-testkit ogr_keeps_stored_coordinates_close_to_integers`,
`/tmp/unigis-carry-wkt-unit.log`. Full revalidation is recorded in HANDOFF.md.
