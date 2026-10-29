# Disable Editor Tracking

Public ArcGIS Pro reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/disable-editor-tracking.html

Public ArcGIS Pro 3.7 controls (`probe.py`, `probe_extra.py`) cover default disable,
each single role, keeping all roles, keeping only dates, a plain class, a table,
UTC/database-time metadata, shapefile refusal, feature dataset members, custom
aliases/domains, boolean arguments and an invalid token. Results are saved in
`probe_results.json` and `extra_results.json`. The two source FileGDB fixtures
were saved once, before the controls mutated their disposable source workspaces.
Do not rerun these fixture-creation recipes merely to recover context.

The default clears all four role names and sets EditorTrackingEnabled false.
Selective disable clears just those roles; tracking remains enabled while any
role is retained. Existing fields/values, timestamps and geometry
remain, as does IsTimeInUTC (including false). Repeat calls and keeping all roles
succeed unchanged. Shapefiles fail 001216; invalid boolean tokens fail 000800.
Feature dataset input updates its feature classes. Disabling tracking restores
the affected fields' editability, retaining custom aliases and domain bindings.

Saved file-format catalog reads locate the change in GDB_Items.Definition:
CreatorFieldName/CreatedAtFieldName/EditorFieldName/EditedAtFieldName become empty;
EditorTrackingEnabled reflects the remaining names; the affected field entries
lose Editable=false, and entries with only Name/FieldType/IsNullable are dropped.
Custom AliasName/DomainName entries remain. The workspace Copy also resulted in
unrelated extent/spatial-index catalog normalization; native edits preserve that
metadata and physical indexes. The first metadata-only implementation passed
catalog comparisons but an independent public ArcGIS read still reported fields
read-only. The additional
read_flags.py/field_flag_results.json format controls located the missing flag
change (1 to 5 for disabled roles, retained roles stay at 1). This is why the
implementation and reference checks cover both representations.

Catalog states are recorded in
`catalog_format_results.json` and `extra_catalog_results.json`. The reads used
`ogrinfo -ro -q -sql 'SELECT Name,Definition FROM GDB_Items' <workspace.gdb>` only
as a file-format oracle. ArcGIS is the geoprocessing behavior oracle; there is
no runtime GDAL dependency or proprietary SDK use.

Implementation: `filegdb/writer/editor_tracking.rs` preflights all targets and
splices catalog XML within the existing journal transaction. It also sets the
editable mask (0x04) in the affected physical field descriptors. No rows/values are rewritten and no
fields are removed. A feature dataset edits all its simple table/class members in the same transaction; advanced member kinds are
refused. `Context::disable_editor_tracking` is an optional host capability;
read-only hosts fail explicitly. CLI supports FileGDB paths. The registry exposes
four default-true flags, accepting ArcGIS tokens and true/false (also 1/0).

13 dtrack_* references cover 11 successes and two refusals. The generic schema
comparator checks ArcGIS tracking properties/editability in addition to schema,
rows/OIDs, geometry/CRS, domain definitions, indexes and repeat behavior. It also
checks actual binary field editability against the catalog. Safety tests permit
only the exact field-flag byte changes observed in the public source/output
files; all other user data/index bytes remain identical. They also check
repeat/no-op byte identity, invalid-argument/missing-input
refusals and feature dataset member updates without touching an unrelated class.
`probe_native.py` independently reads disposable native outputs using public arcpy.

Scope/gaps: no enterprise/mobile databases, mosaic/topology/network datasets,
services or layer/selection model. Advanced feature dataset members are refused.
Unusual class extensions, shared tracking fields and Z/M/curves are unverified.
Native editing APIs do not automatically populate roles left enabled by selective
disable. This tool updates tracking metadata; it does not add that broader edit
engine behavior. Unrelated ArcGIS catalog normalization is not reproduced.

Validation outcomes are recorded in docs/HANDOFF.md. Commands:

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'dtrack_*'
    cargo test -j3 -p unigis-cli --test arcgis_reference --test disable_editor_tracking_safety
    scripts/check
