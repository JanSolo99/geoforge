# Split By Attributes

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/split-by-attributes.html

Three public ArcGIS Pro3.7 probe batches and their saved JSON files cover feature
and table partitions, multiple fields, numeric/text keys, naming/collisions,
existing outputs and rejected OID split fields. The11 sba_* reference cases have
8 successful workspace results and3 errors. Source data and complete expected
folder outputs are saved in testdata; do not repeat completed oracle calls.

Partitions retain source field definitions, attributes, geometry and source row
order within a partition; output OIDs are fresh. Names join split values with
underscores. A blank value becomesT, whitespace/dots/separators are replaced,
and ordinary punctuation is retained. Leading digits, underscores and hyphens
are valid folder names. ../path becomesT___path rather than escaping the folder.
Case-insensitive tuple sorting is stable: normalization collisions are allocated
in that order, equal-case keys retain their first appearance. Name conflicts
(existing datasets included) receive suffix0,1,...; earlier data is preserved.
Independent reversed a_b/A B and a/A controls verify collision ownership.

Implementation: pure grouping in split_attributes.rs; optional write_workspace
capability on registry Context. CLI implements that capability with native
shapefile/DBF writers. Other hosts remain source-compatible and explicitly fail
if they lack workspace output support; they never report unwritten partitions
as success. No app changes or runtime GDAL dependency. The generic reference
workspace comparator now covers standalone DBFs and checks the process status,
including empty result sets; previous feature comparisons/tolerances remain.

Scope: existing folder targets, ordinary feature classes/tables, non-null TEXT/
SHORT/LONG split values, ASCII output names up to128 characters. Original fields
of supported output formats are retained. OIDs are not valid split fields
(ERROR003911). Null/floating/date/64-bit split values, geodatabase target workspaces,
selected reserved device names and non-ASCII/long output names are refused. Other hosts,
selection/environment behavior, Unicode collation and unprobed attribute types
remain unsupported/unverified. Orphan files also reserve stems conservatively.
Workspace writes preflight all partitions and allocate names before writing;
I/O failures during writing are not a multi-dataset transaction.

Reproduce only when needed:

    uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/split_by_attributes/probe.py
    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'sba_*'
    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py --annotate
    cargo build -j3 -p unigis-cli
    python3 scripts/research/split_by_attributes/compare.py

probe_collisions.py and probe_names.py capture additional naming evidence.
Unit tests cover naming/grouping and the unsupported-host fallback. CLI tests
verify that repeated splits preserve every existing file and that an invalid
partition makes no workspace changes. scripts/check also compares geometries
using the project's existing tolerance and all scalar/schema/row attributes.
