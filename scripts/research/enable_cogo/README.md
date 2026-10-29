# Enable COGO

Public tool reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/enable-cogo.html

Eleven public ArcGIS Pro 3.7 controls cover plain/empty lines, partial and complete
existing dimension fields with custom aliases, an already-enabled class, incorrect
String/Integer Direction fields, points/polygons/a table, and a shapefile.
`probe.py` saved the untouched source once as enable_cogo_base.gdb before modifying
a disposable workspace. Do not repeat fixture creation just to recover context.
Snapshots: `probe_results.json`.

ArcGIS adds only missing fields, in Direction/Distance/Radius/ArcLength/Radius2
order, as nullable doubles with NULL values. Existing dimension values and OIDs
(including deleted OID 2 in the populated controls) remain. The five aliases are
reset to Direction/Distance/Radius/Arc Length/Radius2, even when custom aliases
existed. All five fields become required and Describe.isCOGOEnabled becomes true.
Repeated calls succeed unchanged. Wrong field types fail 002045; points/polygons
003917, a table 000732, a shapefile 000156. Fields are not calculated from geometry.

`catalog_format_results.json` records saved catalog XML read with public GDAL
`ogrinfo -ro -q -sql 'SELECT Name,Definition FROM GDB_Items' <workspace.gdb>`.
The five GPFieldInfoEx entries carry canonical AliasName and Required=true while
other properties remain. `read_flags.py`/`field_flag_results.json` inspect saved
physical field descriptors: most affected flags change 5 to 7 (required mask
0x02); new fields have 7. In the complete-existing control, ArcGIS left Radius2's
physical flag at 5 despite Required=true in XML/Describe, including after repeat.
Native normalizes all five to required; this incidental descriptor difference
is not reproduced. ArcGIS workspace Copy also normalizes unrelated catalog
extent/index metadata, which native preserves. GDAL is a file-format reader only;
ArcGIS is the geoprocessing oracle. No runtime GDAL dependency or proprietary SDK.

Native writer/cogo.rs validates all types first, adds missing double fields and
resets aliases in one existing journal transaction. A single rewrite retains
rows, geometries, OID holes/sequence, defaults/domains and surviving indexes.
Unrelated field flags are restored after rewrite. Required flags and catalog
entries are set together. Existing matching fields need metadata changes only;
already-enabled/repeated calls are byte-idempotent. Registry ENABLE and optional
Context::enable_cogo capability supply the CLI FileGDB path operation. Unsupported
hosts fail explicitly. The shared line validator is also used by Disable COGO.

11 ecogo_* portable schema references cover five successes and six refusals.
The comparator checks ArcGIS field schema and required markers, existing rows/OIDs,
geometry/CRS, domains/indexes, NULL new values and repeat schema; only the five
observed alias resets are allowed when comparing original field definitions.
Safety tests check original values/OID holes, unrelated file bytes, repeat/already-
enabled byte identity and wrong-type/geometry/table refusal without changes.
`probe_native.py` independently checks native output through public arcpy.

Scope/gaps: FileGDB line feature class paths. No layer symbology/label expression
model, enterprise/mobile data or selections. Unusual class extensions, shared
schema restrictions, curves/Z/M and custom dimensions with defaults/domains or
nonstandard casing are unverified. Binary layout/index bytes can differ from
ArcGIS because adding fields rewrites through the native writer. Existing Radius2
physical-required quirk and unrelated ArcGIS catalog normalization are not matched.

Validation outcomes are recorded in docs/HANDOFF.md:

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ecogo_*'
    cargo test -j3 -p unigis-cli --test arcgis_reference --test enable_cogo_safety
    scripts/check
