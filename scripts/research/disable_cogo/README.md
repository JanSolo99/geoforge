# Disable COGO

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/disable-cogo.html

Eight public ArcGIS Pro3.7 controls cover enabled/empty-enabled, plain and custom
lines, points, polygons, a table and a shapefile. EnableCOGO was used only to
prepare the source fixture; it is not implemented by this landing.

DisableCOGO retains Direction, Distance, Radius, ArcLength and Radius2, their
aliases/types/nullability and all values/geometry. It changes the five fields
from required to deletable, and Describe.isCOGOEnabled becomes false. Repeats
succeed without further changes. Plain/custom lines succeed unchanged. Point
and polygon inputs fail003917; a table fails000732. A shapefile succeeds unchanged
with five002051 warnings; the native no-op does not reproduce these warnings.

Public arcpy SearchCursor on GDB_Items returned only its root record, so the
initial probe's Definition entries are null. A separate public file-format read
through GDAL CSV exposed the actual saved catalog definitions. The metadata change
is removal of Required children from the five GPFieldInfoEx entries. GDAL was
used only to read the file format; ArcGIS is the geoprocessing behavior oracle.
The ArcGIS workspace copy also left a different HasSpatialIndex catalog marker;
native Disable COGO preserves this unrelated marker and physical spatial indexes.
Raw snapshots: probe_results.json and catalog_format_results.json.

Implementation: native filegdb/writer/cogo.rs splices the five Required elements
inside one existing journal transaction. It does not rewrite user tables or
remove fields. Registry Context has an optional disable_cogo capability; CLI
supports FileGDB datasets and the observed shapefile no-op. Other hosts explicitly
fail without that capability. No runtime GDAL dependency or app changes.

Eight dcogo_* reference cases use an immutable cogo_base.gdb source saved once
before disabling. The generic schema edit generator operates on copies and
asserts ArcGIS row/geometry preservation and repeat behavior. The comparator
checks catalog Required markers against ArcGIS as well as fields/rows/OIDs,
geometry/CRS, domains, indexes and repeat results. Safety checks require all user
data files unchanged, byte-identical second run, and invalid-geometry refusal
without changes. A host test protects explicit capability failure.

Scope/gaps: native FileGDB paths and shapefiles. Layer labeling/symbology is not
changed; no layer model is involved. Enterprise/mobile geodatabases, selections,
advanced class extensions, Z/M/curves and unusual schemas remain unverified.
Shapefile warning text and unrelated catalog normalization are not reproduced.

Recipes/results are adjacent. probe.py creates disposable controls and the source
fixture once; probe_catalog.py records the restricted public catalog view;
probe_native.py independently reads a disposable native output with ArcGIS.
Do not repeat completed probes merely for context recovery.

Validation (outcomes in docs/HANDOFF.md):

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'dcogo_*'
    cargo test -j3 -p unigis-cli --test arcgis_reference --test disable_cogo_safety
    scripts/check
