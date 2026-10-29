# Add GPS Metadata Fields

Public documentation:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-gps-metadata-fields.html

Public ArcGIS Pro 3.7 black-box probes (`probe.py`, `probe_existing.py` and saved
JSON results) establish field order, aliases, types, nullability, domains, repeat
behavior and existing-schema handling. No proprietary SDK was used.

Observed schema: points add22 fields by default,68 with INCLUDE_OFFSET_FIELDS;
polylines, polygons and multipoints add6 fields and ignore the offset option.
Unlike the current web table, AVG_POSITIONS is SHORT and the position-source
domain also has5=Snapped. Offset method codes use lowercase hyphenated strings;
their descriptions use title case. Number-of-satellites has SHORT range0–99.
Repeats succeed without duplicate fields. Existing fields (even differently
typed) keep their definition and values. Table-only input fails000732;
shapefiles fail000656. MultiPatch is refused; its ArcGIS behavior is unverified.

Usage: `ug run add_gps_metadata_fields input=/path/data.gdb/points` (updates
in place); add `include_offset_fields=INCLUDE_OFFSET_FIELDS` for point offsets.

Implementation: `unigis-tools/src/gps_metadata.rs` builds the schema, and optional
registry `Context::add_schema_fields` supplies an explicit mutation capability.
The CLI maps portable domains to native FileGDB types. Read-only hosts return an
error; no application integration or runtime GDAL dependency is added.
`unigis-filegdb/src/writer/schema_add.rs` creates absent domains, appends all
absent fields with NULL values in a bulk row rewrite, and assigns domains while
retaining table identity, OIDs, existing fields, geometry, Z/M, spatial reference
and indexes. Domain-bound aliases are also written into GPFieldInfoEx using
the existing Alter Field metadata path; one native journal covers every write.

Compatible existing metadata domains are retained. Conflicting coded values,
range or field types are refused with000355 before mutation. ArcGIS can partially
add fields before failing in these cases; we preserve the original dataset.
Custom domains with different descriptions/policies but matching codes/types
are retained. Exotic schemas/subtypes, feature services, enterprise/mobile
geodatabases, selections and alternate input formats remain unverified or
unsupported. No claim of physical FileGDB byte identity with ArcGIS is made.

Fixture creation: `make_fixture.py` created gps_metadata_base.gdb once. The14
`gps_*` entries in the main ArcGIS manifest cover4 geometry types, point Z/M,
existing receiver fields, default/offset modes and2 refusals. Generator schema
snapshots retain full fields/domains for geodatabase-only edits instead of
converting them to shapefiles. The generic schema comparator checks fields,
domains and repeat behavior against ArcGIS, and source definitions/rows/OIDs/
geometry/flags/CRS plus new-column NULLs against the unchanged input fixture.
The generator also checks ArcGIS row/geometry preservation and schema/domain
idempotence. No existing reference cases or tolerances were changed.

Validation commands (outcomes in docs/HANDOFF.md):

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'gps_*'
    cargo test -j3 -p unigis-cli --test gps_metadata_safety --test arcgis_reference
    cargo test -j3 -p unigis-tools gps_metadata
    scripts/check

Safety tests cover explicit read-only-host failure, conflicting-domain preflight
and rollback after domain creation/row rewrite followed by a failed assignment.
Use saved probes/references; do not repeat external actions for context recovery.

An independent ArcGIS read of a native offset output exposed missing aliases
in GPFieldInfoEx entries, despite correct physical aliases. The schema edit now
updates both representations. `native_reader_before_fix.json` records the
failure; `native_reader_results.json` confirms all71 fields (including source
OID/Shape/KEY),4 domains and2 source rows after the fix. Recipe:
`probe_native_reader.py`, on the disposable native output named inside it.
The comparator now checks raw catalog XML aliases and GDAL file-format aliases
as well as the native field view. GDAL remains a file reader only.
