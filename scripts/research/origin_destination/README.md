# Generate Origin-Destination Links

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/generate-origin-destination-links.html

ArcGIS Pro 3.7 black-box recipes: probe.py, probe_more.py, probe_limits.py. Run
through tools/arcgis-bridge/run_code.py. Saved results accompany the recipes;
fixture creation uses public arcpy CreateFeatureclass/AddField/InsertCursor.
Probe results were saved before the interrupted coordinator turns; reuse them.

Default PLANAR: Cartesian product in origin-major/destination input order.
Point inputs use their XY; polygons use area centroids, lines length centroids.
Same-source inputs omit identical OIDs. Distinct co-located endpoints retain a
row with zero distance and null line geometry. Source fields are not copied.
Shapefile schema: Id N6=0, ORIG_FID N10, ORIG_X/Y F19.11, DEST_FID N10,
DEST_X/Y F19.11, LINK_DIST F19.11, optional GROUP_ID, COLOR_ID N10.
GROUP_ID is C254 for text, N10 for integer controls. Both group fields are
required. Color keys follow first output appearance (origin OID or group value),
cycling 1..8. Two repeated 12-origin runs verify the cycle despite the docs'
random-color wording. Empty outputs emit WARNING 000117.

PLANAR destination counts: two independently ordered fixtures show that an
integer num_nearest limits the qualifying destination input sequence, rather
than sorting by distance. Reordering the first destination from 50m to 424m
changes the selected destination accordingly; a nearer 0m point is bypassed.
Grouping and search filtering precede the limit. This observed ArcGIS 3.7
behavior is intentionally retained. Zero count/radius means unlimited. Limits
probe records surprising fractional/negative count behavior; those counts are
refused, not generalized from a single run. Radius units also govern LINK_DIST.

Scope: unaggregated planar links in a common projected WKT1 CRS; text/integer
matching groups; integer destination limits; search radius and six physical
units. Geodesic/geographic/mixed-CRS links, aggregation/statistics, noninteger
or negative counts, noninteger numeric/date groups and 64-bit OIDs are refused.
Null input geometry and Z/M inputs have not received separate oracle controls.
DBF schema is verified; other output-backend schema differences remain unverified.

Regenerate the reference family once fixtures exist:

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'odl_*'

Full generic reference comparisons remain exact for attributes/counts/shape
presence and use the existing 0.001m geometry tolerance. No field skips or new
tolerances are needed.
