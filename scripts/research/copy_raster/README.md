# Copy Raster: every band and source-type NoData defaults

ArcGIS Pro 3.7 black-box probes in `observations.json` exposed two native defects:
Copy Raster constructed a single-band output, and several untagged source pixel
types received incorrect default NoData values. A native copy of `rgb_pixel_u8.tif`
had one band; ArcGIS retained all three bands. GDAL inspects file contents only.

Saved reference cases:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'cprbands_*'
```

24 cases cover RGB pixel-interleaved U8 and planar U16, default/F32/U16 output,
every one of eight source pixel types with default/F64 output, and two-band F32
with NoData in only the first or later band. Pixels are compared exactly in every
band, along with type, grid, CRS, NoData and per-band statistics.

For untagged inputs the source-type defaults are U8/I8=256, U16=65536,
I16=32767, U32/I32=2147483647, F32=3.4e38, F64=1.79e308. F32 output stores the
rounded float32 marker; F32-to-F64 uses the double representation of 3.4e38.
Do not clamp integer NoData tags to the cell range: valid U8 value255 must survive
when its metadata marker is256. Existing explicit/input NoData overrides still
apply. Every band undergoes the requested pixel conversion in its original order.

The original multiband probes write statistics because their inputs have no cached
statistics. The saved single-band DEM-with-NoData input is cached, and its copy
has no output statistics.22 isolated controls now explain this distinction:
[statistics/README.md](statistics/README.md). Size, actual missing cells and band
count do not select this policy; cached tagged sources omit statistics only when
the effective type and marker remain unchanged.

Input RGB/type fixtures are reused from `testdata/raster` and `composite_matrix`.
`make_inputs.py` uses GDAL solely for file IO to create the tiny first/later-band
NoData fixtures. Run from repo root with GDAL Python bindings and NumPy installed.
Probes copy source files to /tmp to keep exploratory ArcGIS writes isolated.
A first full-RGB-array probe stalled waiting for its large response; its own
worker was cancelled. Compact probes record band counts, shape and eight samples
per band, while the saved TIFF references retain all cells.

Scope remains GeoTIFF; colormaps/pyramids/histograms/covariances, scaling flags and
nondefault storage/statistics environments are not implemented. Unverified storage flags and tagged-input differences are recorded in docs/KNOWN_GAPS.md.

Downcast probes in `downcast_observations.json` now have16 saved `cprcast_*` cases.
Regenerate with `make-arcgis-reference.py 'cprcast_*'`. They cover every source type
to F32 and signed16; Infinity (stored as the string "inf" in strict JSON) is the observed F64-to-F32 default marker. F32/F64
to signed16 retain3.4e38/1.79e308 tags even though those cannot be stored as cells.
`downcast_native_before.txt` records the four original mismatches: the writer
saturated huge tags to i64MAX, and I32/U32-to-F32 statistics failed to exclude the
rounded marker2147483648. NoData is now normalized before statistics, and integer
metadata beyond i64 is serialized without saturation. GDAL/native file tests cover
large positive/negative sentinels separately from the ArcGIS operation oracle.

Further tagged-input probes remain outside this slice: U32 NoData metadata exports
through signed32 in ArcGIS; extreme/fractional explicit markers can map cells
differently. Native Copy Raster still differs there; see KNOWN_GAPS. The former
tiny-input statistics gap is resolved by the cached-source/type/marker policy.
