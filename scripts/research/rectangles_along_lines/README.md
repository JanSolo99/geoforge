# Generate Rectangles Along Lines — deferred

Public ArcGIS Pro3.7 controls in probe.py and probe_results.json (25 controls).
Source fixtures testdata/geoproc/ral_*.shp were created once; no native implementation.

The observed algorithm needs adaptive cartographic rectangle placement and chain/
branch grouping not implemented in the geometry engine. An L-shaped50+50m line
with30m along/20m across gives3 rectangles, middle centroid(44.0425,4.5531),
angle137.6 from UR; simple30m stationing gives4 and cannot reproduce those corners.
Connected50m lines form one chain, but branch side arms get different groups and
Previous/Next cross-group links. Multipart chains reset SeqId while sharing GroupId.
Default omitted sort starts at the opposite end to explicit UR in this control.
Output fields are PageNumber/GroupId/SeqId/Previous/Next/Angle (not the documented ID).

Defer rather than implementing only straight lines or fitting constants to one bend.
Zero/negative lengths fail000398. Public documentation:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/generate-rectangles-along-lines.html
