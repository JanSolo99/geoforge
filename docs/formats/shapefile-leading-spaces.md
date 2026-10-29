# Leading spaces in DBF character fields

The GARS input fixture includes `" 391MH32 "`. ArcGIS's generated shapefile
retains its leading space (trailing spaces become indistinguishable from DBF
field padding). Native reading and copying preserve the literal `" 391MH32"`.
GDAL 3.13 reports `"391MH32"` for the same file, including through `ogrinfo -json`.

The raw `COORD` character field at FID 54 (ID 55) in
`testdata/arcgis-reference/ccn_grid_input_gars_dd_numeric.dbf` begins with bytes
`b' 391MH32          '`. Reading the DBF field descriptor offsets and record
width confirms that the initial space is in the stored value, rather than a
text-display separator. The shared testkit already uses JSON properties.

[Shapelib's public DBF reader](https://github.com/OSGeo/shapelib/blob/master/dbfopen.c)
has a `TRIM_DBF_WHITESPACE` branch in `DBFReadAttribute` that strips leading and
trailing ASCII spaces. This agrees with the observed GDAL output. No GDAL or
Esri code was copied into the native implementation; its runtime behavior is
unchanged. GDAL is a file-format test oracle, not the geoprocessing reference.

The shapefile-only test helper accounts for exactly this observed difference:
when the native value starts with ASCII spaces and GDAL's string is precisely
the same value with those spaces removed, the oracle comparison restores them.
All other fields, content differences, geometry and record counts are still
compared. The independent original-versus-copy check requires exact strings,
and the geoprocessing references compare against ArcGIS's stored values.
No files or whole records are exempted from comparison.

Reproduction:

```
mkdir -p /tmp/unigis-grid-leading
./target/debug/ug run convert_coordinate_notation input=testdata/geoproc/grid_input_gars.csv x_field=COORD input_format=GARS output_format=DD_NUMERIC output=/tmp/unigis-grid-leading/native.shp
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/coordinate_notation/grid/probe_leading_spaces.py
ogrinfo -ro -al -json -features testdata/arcgis-reference/ccn_grid_input_gars_dd_numeric.shp
```

The saved ArcGIS read-back is in
`scripts/research/coordinate_notation/grid/probe_leading_spaces_results.txt`.
The failure that exposed this distinction is recorded in
`/tmp/unigis-ccn-grid-input-check2.log` (three copied fixtures, FID 54, COORD).

ArcGIS Pro 3.7 read-back confirms `" 391MH32"` on both the generated reference
and native output at ID 55. The saved read-only probe contains both values.
