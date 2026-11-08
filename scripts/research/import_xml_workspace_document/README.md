# Import XML Workspace Document

Public black-box ArcGIS Pro 3.7 build 1901 (Advanced), 2026-11-08. No proprietary SDK.
Tool documentation: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/import-xml-workspace-document.html

`probe.py` creates a standalone table and UTM 32N points, exports DATA in both
BINARY and NORMALIZED storage with NO_METADATA, and imports DATA/SCHEMA_ONLY.
`probe_extended.py` adds multipart lines, polygons, multipoints, numeric/text/date/GUID
fields, a numeric default and a deleted OBJECTID. Run either with:

```
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/import_xml_workspace_document/probe.py
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/import_xml_workspace_document/probe_extended.py
```

Observations are in `results.txt` and `extended_results.txt`. Fixtures copied verbatim
from these exports live in `testdata/geoproc/xml_workspace/`. Default environments
were used (no filters or coordinate transformations), except overwrite was enabled for
scratch setup and disabled for the collision control. ArcGIS retains OBJECTIDs 1,3,
field aliases and defaults, nulls, Unicode and XML-escaped text. Both encodings import
the same geometries and values. DATA is the default; a schema-only document can also be
imported with DATA. Repeated imports with overwrite disabled create `labels_1`, `points_1`.

Six portable `gsc_xml_*` scenarios in `scripts/make-arcgis-reference.py` record ArcGIS
imported schema and tool statuses. Regenerate together:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'gsc_xml_*'
UNIGIS_REFERENCE_CASES=gsc_xml_ cargo test --locked -j3 -p unigis-cli --test arcgis_reference tools_match_arcgis_output
cargo test --locked -j3 -p unigis-cli --test import_xml_workspace
```

`verify_native.py` reads the native `native.gdb` (created with `ug run create_file_gdb`
and imported from binary.xml then extended_binary.xml) in ArcGIS. `native_results.txt`
records equal schemas, rows including geometry JSON, and complete spatial-reference
strings for all six tables/classes. The first read-back was started before native
creation finished; it was rerun after the writes completed.

The dedicated CLI regressions compare records, OBJECTID gaps, multipart coordinates,
coordinate domain/precision and shape measurements, as well as schema-only behavior,
collision names, input preservation and rollback after a later table fails to write.

Implemented scope: an existing FileGDB target; plain XML up to 64 MiB; DATA and
SCHEMA_ONLY; default/DEFAULTS configuration; standalone tables and simple 2D
point/multipoint/polyline/polygon classes with standard OBJECTID/Shape names.
SHORT/LONG/FLOAT/DOUBLE/TEXT/DATE/GUID attributes, field aliases, nullability and
numeric/text defaults are supported. Coordinate domains and precision are retained.

Limits: compressed ZIP/Z, mobile/enterprise targets, feature datasets, domains,
subtypes, relationships, sequences, GlobalIDs, rasters, Z/M/curves, custom dataset aliases,
custom indexes, tracking/versioning and other field types are refused. OBJECTIDs above
16777216 are refused to bound sparse-index allocations. Existing datasets are always
retained using unique names (no overwrite environment). Dataset metadata/lineage is not
restored. Shape_Length/Shape_Area are recomputed by the standard native writer; stale
measure values and empty normalized shapes have not been probed. Other ArcGIS versions,
nondefault environments and schema dialects remain unverified. Failure errors are descriptive;
exact ArcGIS error numbers beyond parameter/target validation are not claimed.
