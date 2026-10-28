# Line Statistics

ArcGIS Pro 3.7 black-box observations recovered from coordinator run
`20261028T094627-4d2f2b6c`. `probe_weights.py` compares six sampled LENGTH cells
against ArcGIS geometry intersections with a radius-60 circle. Road field values
are 1, 2, 3, 4. LENGTH sums each clipped length times its field value; the
unweighted sum of lengths does not match. MEAN is weighted by clipped length.
MAXIMUM and VARIETY sampled values were also inspected. No GDAL geoprocessing
semantics are used.

Reproduce the independent oracle probe:

```
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/line_statistics/probe_weights.py
```

The 22 `ls_*` reference cases include nine road/stream cases exercising all eight
statistics and several cell sizes/radii, plus 13 controlled integer/double
coincident-line cases for types, ties and empty-neighborhood behavior. Regenerate with:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ls_*'
```

Implemented scope: planar straight multipart lines, numeric fields, explicit
positive cell size and radius, default grid. Geographic distances, raster
geoprocessing environments, default cell size/radius, curves, Z/M and null-field
behavior are not established. The controlled integer/double fields and mode ties have independent oracle
controls; extreme field values and nearly tied lengths remain unverified.

Additional Codex controls:

- `probe_parameters.py` / `parameter_observations.jsonl`: zero/negative radii
  produce ArcGIS errors 000864/000892; zero/negative cell sizes fail 000628.
- `probe_ties.py` / `tie_observations.jsonl`: two coincident diagonal lines with
  equal clipped lengths, integer values 1 and 2, double values 1.25 and 2.25.
  Both integer majority/minority ties choose 1; variety counts two and writes
  ordinary zero in cells reached by no line. Integer extrema/range/variety/modes
  are S32 with NoData -2147483647. All five allowed double statistics are F32
  with NoData -FLT_MAX. Double majority/minority/variety are refused (000800).
  A preliminary horizontal-line control failed with 010092 (zero-height extent);
  the diagonal control avoids that degenerate extent.
- `make_inputs.py` writes the controlled input shapefile used by saved cases.
  It uses ArcGIS feature creation, independently of native statistics code.

Full-cell baseline validation found five failing cases, all involving contacts or
clipping precision. LENGTH emits ordinary zero when a segment merely touches a
circle; extrema, variety and modes include zero-length contacts. MEAN with only
zero-length contacts stays NoData. Treating every zero clip as an absent segment
therefore loses participants and changes minority results.

The remaining LENGTH mismatch is a precision fingerprint. At column29,row25 of
`ls_length`, full double input clipping gives 0.9674599320544552; rounding vertex
offsets from the lower-left grid origin to F32 before clipping gives
0.967590494254302, which rounds to ArcGIS's exact stored F32 0.9675905108451843.
The offset representation is inferred from that fingerprint and checked across
all saved cells; it is not an assertion about Esri's internal source code.
`compare_native.py` compares all saved ls_* cells without computing GDAL stats.
