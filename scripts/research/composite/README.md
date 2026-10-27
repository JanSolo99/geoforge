# Composite Bands black-box references

Behavior reference: ArcGIS Pro 3.7, public documentation and black-box runs only.
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/composite-bands.html

The `cbands_*` batch in `scripts/make-arcgis-reference.py` saves 86 ArcGIS outputs (18 original, a complete 64-pair type matrix, and four padding cases).
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
- All eight pixel types are covered by the complete pair matrix, with signed/unsigned
  extrema and values beyond f32 integer precision. Mixed types promote: U8 + I16 -> I16, U8 + U16 -> U16, U16 + I16 -> I32,
  I16 + F32 -> F32, I32 + F32 -> F64, and F64 + F32 -> F64, independent of order. U8+I8 -> I16, I8+U16 -> I32, U32+any signed
  integer or F32 -> F64. Composite output cells are compared exactly, including floats;
  these are stored values/resampling, not floating-point arithmetic estimates.
- Stored cell values are retained; the global TIFF NoData comes from the **last**
  input when any input has a NoData tag. If the last input has none, the output
  uses the type's default (observed F32 3.4e38). Thus reversing dem/dem_nodata can
  expose -9999 cells as valid data. Statistics reflect that exposure. This is an
  observed ArcGIS quirk, not an attempted normalization of missing values.
- Statistics are calculated for every band. The standard native statistics writer
  now emits one PAMRasterBand section per valid band, preserving its single-band API.

Validation also improves the generic CLI comparison: band count, **every band's**
cells, and every band's count/min/max/mean/stddev are checked. Previously it only
called Raster.get(), checking band 1. A deliberate third-band corruption, missing
band, and missing third-band metadata exercise that blind spot in a regression test.
No numerical tolerances were raised; existing named raster differences remain intact.

Current limits: GeoTIFF inputs/output; matching EPSG identifiers; north-up grids and
square first-input cells. Band selectors/layers, custom or differing coordinate systems/reprojection,
nondefault raster environments and resampling choices are not implemented. Pyramids,
histograms, covariance/median metadata and ancillary ArcGIS XML/world files are not
replicated. Mixed-type NoData defaults when the last input lacks a tag still need additional
probes; only the equal-type F32 fallback is currently saved.

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
and `probe_padding.py` preserve the direct black-box observations. The nine source
TIFFs are stored in testdata/raster/composite_matrix (all without NoData tags).
