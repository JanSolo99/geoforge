# Extract Data From Geodatabase

Public ArcGIS Pro 3.7 behavior controls; no SDK or runtime GDAL dependency.
Reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/extract-data-from-geodatabase.html

`probe.py` creates the immutable `testdata/geoproc/extract_base.gdb` once, then
extracts disposable outputs. Do not regenerate the source merely to inspect it.
`probe_options.py` checks standalone table defaults, expansion and refusals.
`probe_defaults.py` resolves an ArcGIS dynamic default. Their saved JSON results
retain schema, rows, OIDs, defaults, domains, aliases, indexed fields and relationships.
Run each through `uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py <script>`.

The default copies feature rows and only the schema of an unfiltered table.
USE_DEFAULTS expands simple relationships; GET_RELATED includes matching rows
(NULL keys never join), in either direction. ALL_RECORDS_FOR_TABLES includes all
table rows; ALL_ROWS includes all expanded rows. SCHEMA_ONLY suppresses all rows.
DO_NOT_ADD suppresses relationship classes even if both endpoints are explicit.
ADD_WITH_SCHEMA_ONLY initially excludes expanded rows: when get_related_data is
omitted, ArcGIS dynamically defaults it to DO_NOT_GET_RELATED; explicit GET_RELATED
still includes related rows (0 versus 3 observations in defaults_results.json).
An initial native reference run missed that distinction (15/16 cases matched);
the native runner now honors both cases.

The native implementation copies source storage fields/coordinate resolution,
preserves OIDs and gaps (sites 1,3,4), values/NULLs, aliases/defaults/domains, subtypes,
and single-field indexes. Simple key relationships are copied. It adds to an
existing FileGDB using its rollback journal, or creates a new FileGDB. It refuses
colliding output dataset names instead of replacing or reusing schemas.
Unsupported index expressions fail with rollback. Empty high-precision Date
fields keep their precision without relying on fractional values being present.

16 `edg_*` recipes in `scripts/make-arcgis-reference.py` save full public output
geodatabases: 14 successes and missing/empty input refusals (000732/000735).
`common/workspace_extract.rs` compares native output catalog, fields/defaults,
precision, domains, subtypes, relationships, index expressions, OIDs, rows,
geometry and CRS/resolution. Index names are not compared: ArcGIS assigns new
DSID-dependent names; native extraction retains source names. `prepare_native.py`
creates independent disposable native outputs; `probe_native.py` reads those with
public arcpy and compares complete field properties, rows, domains, relationships
and indexed fields with the saved public outputs. Validation outcomes are recorded
in docs/HANDOFF.md; scripts here do not imply a validation run.

Scope: standalone classes/tables in one FileGDB, DATA/SCHEMA_ONLY, existing or new
FileGDB output and simple non-OBJECTID key relationships. Feature datasets,
attributed/many-to-many/OBJECTID-key relationships, editor tracking, COGO and
attachments are refused. Multiple source workspaces, XML/mobile outputs, schema
reuse and geometry filters are explicitly unsupported. Layer selections,
definition queries, environment extent/output reprojection, advanced/custom
extensions, composite cascades, relationship chains with unusual key casing/types,
GlobalID/replica semantics, raster/annotation/network/topology and complex Z/M/curves
are unverified. Relationship chains use a native fixed-point traversal; only a
single simple relationship is in the public controls. All rows are loaded in memory.
Class-level aliases/extension metadata and regenerated index names are not copied;
field aliases are retained. No claim of byte-identical output geodatabases.
