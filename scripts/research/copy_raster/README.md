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

Multiband probes write statistics for each band even when a band contains NoData.
The existing single-band DEM-with-NoData reference has histograms but no statistics;
that behavior remains separate. ArcGIS's general statistics-generation policy
across sizes/input caches/environments is not fully established.

Input RGB/type fixtures are reused from `testdata/raster` and `composite_matrix`.
`make_inputs.py` uses GDAL solely for file IO to create the tiny first/later-band
NoData fixtures. Run from repo root with GDAL Python bindings and NumPy installed.
Probes copy source files to /tmp to keep exploratory ArcGIS writes isolated.
A first full-RGB-array probe stalled waiting for its large response; its own
worker was cancelled. Compact probes record band counts, shape and eight samples
per band, while the saved TIFF references retain all cells.

Scope remains GeoTIFF; colormaps/pyramids/histograms/covariances, scaling flags and
nondefault storage/statistics environments are not implemented. Existing float
to signed16 NoData behavior is still a documented gap in docs/KNOWN_GAPS.md.

Additional exploratory downcasts are preserved in `downcast_observations.json`
(16 cases, not yet saved in the reference manifest). ArcGIS uses Infinity for
untagged F64-to-F32 NoData and keeps3.4e38/1.79e308 tags on integer output.
The integer writer still clamps those large tags to i64; a subsequent slice should
fix and test it. Float conversion also requires normalizing output NoData before
computing statistics (I32/U32 default marker rounds to2147483648 in F32).
