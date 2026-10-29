# Pairwise Polygon Neighbors

ArcGIS Pro 3.7 black-box evidence:14 `ppn_*` generator/manifest cases (12
successful tables,176 rows;2 invalid-input cases), plus independent aligned
boundary and reversed-report-order probes. Input recipe `make_inputs.py` creates
12 projected polygons: edge/node neighbors, overlapping and contained polygons,
duplicate geometry, a hole and its filler, and multipart edges. Saved probe JSON
files accompany recipes; all use public arcpy and preserve clean-room rules.

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/pairwise-polygon-neighbors.html

Observed policy:

- Omitted report fields use the source ObjectID field; explicit ObjectIDs also
  work. Each report field becomes adjacent src_/nbr_ columns, retaining widths.
- Crossing-only neighbors are excluded. With AREA_OVERLAP, positive-area pairs
  contribute area. Without it, these pairs are omitted even when their boundaries
  align (independent50/25/4 square-unit controls). Disjoint interiors can contribute
  edge length or isolated touching nodes. Holes and multipart edges are covered.
- Contributions aggregate by the full source/neighbor report tuples. Area takes
  precedence over edge/node information for the aggregated pair; otherwise edge
  length takes precedence over node counts. Within-group contacts aggregate into
  one self-key row; BOTH_SIDES doubles those contributions.
- Rows sort by source tuple then neighbor tuple, rather than interleaved column
  order. NO_BOTH_SIDES uses the lower report tuple as source; reversing labels
  and groups independently of OIDs confirms this. Self-key totals are halved.
- AREA/LENGTH are F19.11; NODE_COUNT is N10. Default units are coordinate units.
  Kilometer/hectare and international-foot/square-foot reference cases verify
  scaling. The native implementation also reuses existing projected area-unit
  conversions and offers US survey feet for lengths.

Implementation: native intersection/meeting primitives in a separate wrapper;
classic Polygon Neighbors is unchanged. Report fields currently require non-null
text or32-bit integers. Projected WKT1 inputs only; custom XY tolerance,
MAX_PRECISION, curves, null geometry, other report types/64-bit OIDs are refused.
Geographic/unknown CRS, selections/environments, mixed contacts across multipart
components, sub-tolerance clustering, empty
output warning text, field collisions and unprobed output-backend schemas remain
unsupported or unverified. Performance has not been benchmarked.

Reproduction (run repository root; do not repeat saved ArcGIS work needlessly):

    uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/pairwise_neighbors/make_inputs.py
    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ppn_*'
    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py --annotate
    cargo build -j3 -p unigis-cli
    python3 scripts/research/pairwise_neighbors/compare.py

`probe_overlap.py` and `probe_order.py` run through the same bridge and save
`/tmp/unigis-ppn-{overlap,order}.json`. Unit controls cover their key findings.
The native comparator reads tables using GDAL as a format oracle only; expected
behavior comes from the saved ArcGIS tables. Schemas/counts/text/order are exact;
numbers use the existing generic comparator's tolerance. No skips or relaxed
comparisons were added to the full gate.
