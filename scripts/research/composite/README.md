# Composite Bands black-box references

Behavior reference: ArcGIS Pro 3.7, public documentation and black-box runs only.
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/composite-bands.html

The `cbands_*` batch in `scripts/make-arcgis-reference.py` saves 115 ArcGIS outputs (18 original, a complete 64-pair type matrix, four padding cases and 29 NoData cases).
Regenerate with:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'cbands_*'
```

Exploratory scripts `probe.py` and `probe_types.py` run through
`tools/arcgis-bridge/run_code.py`; they write to /tmp. `observations.json` preserves
status/messages/input lists. GDAL inspects output file metadata only; it is not a
runtime dependency or an oracle for the operation.

Observed behavior implemented in `unigis-raster::composite` and exposed through
`ug run composite_bands inputs='a.tif;b.tif' output=out.tif`:

- All bands are concatenated in input order. A three-band RGB input remains three
  bands; repeating it makes six. The first input determines output extent/cell size.
- When all inputs lack NoData tags, uncovered cells are zero and the output tag
  stays absent, including when valid cells contain the type maximum.
- Different grids use nearest-neighbor sampling at output cell centers. The saved
  cases cover a shifted grid, outside padding and a 30 m raster resampled to 10 m.
- All eight native pixel types are covered by the complete pair matrix, with signed/unsigned
  extrema and values beyond f32 integer precision. Mixed types promote: U8 + I16 -> I16, U8 + U16 -> U16, U16 + I16 -> I32,
  I16 + F32 -> F32, I32 + F32 -> F64, and F64 + F32 -> F64, independent of order. U8+I8 -> I16, I8+U16 -> I32, U32+any signed
  integer or F32 -> F64. Composite output cells are compared exactly, including floats;
  these are stored values/resampling, not floating-point arithmetic estimates.
- Stored cell values are retained; the global TIFF NoData comes from the **last**
  input when any input has a NoData tag. If the last input has none, the output
  uses the type's default (observed F32 3.4e38). Thus reversing dem/dem_nodata can
  expose -9999 cells as valid data. Statistics reflect that exposure. Mixed-type NoData probes establish defaults of
  the highest-ranked tagged input, using U8<I8<U16<I16<U32<I32<F32<F64, rather
  than the promoted output type. That input type supplies
  256 for U8/I8, 65536 for U16, 32767 for I16, 2147483647 for U32/I32,
  f32-rounded 3.4e38 for F32 and 1.79e308 for F64. Integer metadata tags can lie
  outside the pixel range and must not be clamped. A U32 sentinel promoted to F64
  passes through signed32 (observed 2^31 -> -2^31 in the stored TIFF).
- Padding uses each source's sentinel or zero, independently of the final global
  output tag; a three-input case verifies middle-band padding. This is an
  observed ArcGIS quirk, not an attempted normalization of missing values.
- Statistics are calculated for every band. The standard native statistics writer
  now emits one PAMRasterBand section per valid band, preserving its single-band API.

Validation also improves the generic CLI comparison: band count, **every band's**
cells, and every band's count/min/max/mean/stddev are checked. Previously it only
called Raster.get(), checking band 1. A deliberate third-band corruption, missing
band, and missing third-band metadata exercise that blind spot in a regression test.
No numerical tolerances were raised; existing named raster differences remain intact.

Current limits: packed/complex input types are unverified; GeoTIFF inputs/output; matching EPSG identifiers; north-up grids and
square first-input cells. Band selectors/layers, custom or differing coordinate systems/reprojection,
nondefault raster environments and resampling choices are not implemented. Pyramids,
histograms, covariance/median metadata and ancillary ArcGIS XML/world files are not
replicated. Padding with an input integer NoData tag outside that input pixel range still
needs probes; current padding cases use representable source sentinels.

Local validation logs (ephemeral):
- /tmp/unigis-composite-reference-generation.log: all 18 ArcGIS cases succeed.
- /tmp/unigis-composite-ref-first.log: full manifest comparison passes, including all
  18 new cases and every band/statistics section (before adding the corruption test).
- /tmp/unigis-composite-check.log: initial full root gate passed (178 tests), including
  corruption test, before the type-matrix/padding follow-up.
- /tmp/unigis-composite-matrix-generation.log: all 68 further oracle cases succeeded.
- /tmp/unigis-composite-final-check.log: final full root gate on the 86-case slice.

`matrix_inputs.json` defines the tiny source rasters; `make_matrix.py` recreates them
using GDAL for file conversion only. `matrix_observations.json`, `probe_matrix.py`
and `probe_padding.py` preserve the direct black-box observations. The 18 source
TIFFs (normal/tagged variants plus shifted grids) are stored in testdata/raster/composite_matrix (all without NoData tags).

Precision fixture correction (follow-up after d29a79a): the original AAIGrid-based
writer narrowed F64/U32 values before TIFF creation. The initial comparisons matched
those stored inputs, but did not exercise all intended boundaries. GDAL documents
AAIGrid inference as Int32/Float32 by default:
https://gdal.org/en/stable/drivers/raster/aaigrid.html
`make_matrix.py` now writes explicitly typed GDAL/NumPy arrays and reads every file
back, asserting values, dtype, NoData tag, grid and EPSG. Use `--check` for read-only
verification. Corrected matrix/padding outputs were regenerated, along with all new
NoData cases. In particular the U32 inputs now retain 2147483648/4294967294, and
F64 retains 0.1/16777217.25/4000000000.125. NoData probes use these corrected inputs.
`nodata_observations.json`, `probe_nodata.py` and `probe_padding_nodata.py` preserve
raw observations. Full follow-up gate: /tmp/unigis-composite-nodata-check.log.

Three-input probes distinguish fallback ranking from both first/last input and
numeric maximum: tagged I16+U16 followed by an untagged input chooses I16's 32767,
while tagged I32+F32 chooses F32's already-rounded 3.4e38, even for F64 output.
`probe_nodata_first.py` and `probe_nodata_combo.py` preserve those distinctions.
