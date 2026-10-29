# Create Random Points — ArcGIS Pro 3.7 controls

Reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-random-points.html

`probe.py` prepared immutable `testdata/geoproc/rnp_{areas,lines,points}.shp`.
Its initial controls reused disposable input/output basenames, and the snapshot
wrongly assumed CID on extent outputs; initial_probe_results.json records those
failures. The repo fixtures had already been saved and were not overwritten.
`probe_controls.py` uses those saved sources and distinct disposable outputs;
probe_results.json is the corrected behavioral evidence. Do not regenerate sources.

Confirmed behavior:

- Extent sampling: no input attributes, `Id` integer width6 containing0. An extent
  supplied as a dataset path supplies its CRS. Constraints override the extent.
- Polygon/line sampling: per-feature count, `CID` integer width10 containing the
  original OID; count/distance fields allowed. N=2.6 rounds to3; N=0 yields none.
  Line sampling is length weighted. Input points instead produce a random subset,
  capped by available points, with original OIDs in CID and no new locations.
- Multipoint output: chunk size3 gives3,3,1 points for a7-point extent. Polygon
  chunks restart per source feature. Public shapefile output CID is0 on all
  multipoint rows (including later polygons); native matches this observation.
- Infeasible spacing reduces the count and warns000983; zero output warns000117.
  Negative scalar counts fail000963; negative distances fail000673.
- Defaults introspection: count100, spacing0 Unknown, POINT, chunk size10. Extent
  reports DEFAULT; controls/sample behavior and public examples use0 0 250 250.
- Independent ArcGIS repetitions give different coordinates. Exact coordinate
  equality is not a valid oracle. Fractional-distance controls confirm both field
  GAP=0.6 and literal0.6 Meters produce minimum pair spacing between0.6 and1.0;
  neither rounds to1.0 in this installation despite the general wording in docs.

16 rnp_* reference cases (14 success,2 expected failures) save actual public output
shapefiles. Only this tool gets a `stochastic_points` comparison: exact schema,
CRS, count/chunk sizes and per-source counts, plus independent membership, original
point-subset identity, uniqueness and minimum spacing checks. No existing reference
comparisons or tolerances are weakened. Unit tests check uniform quadrant occupancy,
polygon-hole exclusion, line-length weighting, deterministic seeds, spacing,
saturation and resource limits. CLI safety protects an existing output on refusal.

Native scope: planar sampling in extents, polygons (holes respected), lines and
point/multipoint source subsets; output POINT or MULTIPOINT for extent/line/polygon.
A native SplitMix64 generator uses seed0 by default; `seed` chooses a reproducible
native stream, not ArcGIS's randomNumberGenerator environment/algorithm. Coordinate
and point-subset identity differences are expected. Minimum-distance packing uses
bounded rejection (10000 consecutive failed proposals) and may produce fewer
points than ArcGIS under dense/very thin constraints. Spacing is enforced across
all parts of a source feature, where ArcGIS may relax it between parts. Rows are
loaded in memory; at most1million requested/output points are accepted.

Curves and multipoint output from point constraints explicitly refused. Output
coordinate-system/extent/selection environments, geodesic/geographic unit behavior,
GlobalID/64-bit CID, Z/M, rare invalid inputs and unusual multipart constraints are
unverified. No GDAL runtime dependency or Esri SDK. Independent public reader recipe
probe_native.py validates the saved native outputs; its first run had an ArcPy
multipoint iterator error, corrected to read public geometry JSON. Actual validation
outcomes and logs are recorded in docs/HANDOFF.md.
