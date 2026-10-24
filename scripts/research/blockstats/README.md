# Block Statistics — ArcGIS Pro 3.7 probes

`scripts/make-arcgis-reference.py 'blk_*'` defines 75 black-box runs against
ArcGIS Pro 3.7. The manifest records 74 successful outputs and one rejected
zero-width rectangle (000622/000628). Inputs live in `testdata/raster/`; outputs
are `testdata/arcgis-reference/blk_*.tif`. No proprietary SDK was used.

`crates/unigis-raster/tests/blockops.rs::block_statistics_geotiffs_equal_arcgis`
reads both input and ArcGIS output with our GeoTIFF reader. It compares every
band, dimensions, grid, CRS, output pixel type, NoData and cell values (integer
values exact, floats within 1e-5 relative with a 1e-5 absolute floor). The older
`block_tools_equal_arcgis` JSON comparisons remain in place.

Observed behavior:

- Blocks start at the upper left. Circular/annular masks sample inside their
  footprint and assign the result to the entire rectangular block.
- Integer median takes the lower middle value. Integer output uses
  `trunc(value + 0.5)` even for negative whole values: -3 becomes -2. This is
  specific to Block Statistics; the shared pixel conversion stays unchanged.
- MEAN/STD and supported statistics on float input produce F32; integer SUM
  produces S32 with NoData 2147483647. MIN/MAX/MAJORITY/MINORITY/MEDIAN retain
  the integer input type; RANGE uses the unsigned type of the input width.
- VARIETY output depends on footprint capacity. Observed capacities: 1 and 15
  produce U8/NoData 16; 4, 9 and 12 produce U8/15; 16, 25, 29, 36 and 64 produce
  S8/-128; 256 produces U16/65535. Other capacity boundaries are extrapolated
  and have not been checked against ArcGIS.
- Float MAJORITY/MEDIAN/MINORITY/VARIETY are refused (000864).
- MAP rectangle dimensions round to the nearest cell (100x130 on a 30-unit grid
  becomes 3x4; 75x45 becomes 3x2). Circle MAP radii truncate (45, 75, 100 become
  1, 2, 3 cells). Annulus MAP radii round (30..75 becomes 1..3).
  The annulus probe uses an exactly aligned inner radius; fractional inner MAP
  rounding is extrapolated and remains unverified. CELL radius 2.5 truncates to 2.
  Circular MAP conversion is currently limited
  to square cells; rectangular conversion uses each axis's cell size.
- Multiband maximum is checked on all three bands of `rgb_pixel_u8.tif`
  (`blk_u16` is a historical probe name; its input is actually U8). App conversion
  also retains all bands for in-memory and non-TIFF inputs. The regression
  `in_memory_multiband_block_statistics_equal_arcgis` passes the RGB raster through
  both fallback paths, then compares the encoded result with this same ArcGIS
  output: all three bands, every cell, type, NoData, grid and CRS.

`analyze.py` is the inherited exploratory NumPy/GDAL analysis script. GDAL reads
file values only; behavior conclusions come from the ArcGIS outputs. Rust
regression tests do not depend on this script or GDAL.

CLI entry point (also exercised for all 75 probes by the existing ArcGIS CLI
comparison harness):

```sh
ug block-statistics testdata/raster/landuse_u8.tif /tmp/blocks.tif \
  --neighborhood "Rectangle 90 120 MAP" --statistics-type MAXIMUM
```

Not covered: irregular/weighted/wedge neighborhoods, non-square circular MAP
cells, raster environments, extreme integer overflow, all VARIETY capacity
boundaries, or exhaustive NoData collision cases. These are not compatibility
claims. Do not regenerate references merely to run the Rust tests.
