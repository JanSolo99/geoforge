# Get Cell Value (ArcGIS Pro 3.7 probes, 2026-11-08)
- Result is a string: `NoData` outside the extent, on NoData cells, or for a band that does not exist (0, 3 on 2 bands).
- Integers print plainly; F32 prints shortest round-trip (`204.9949`); F64 `1` for 1.0.
- XMax / YMin edges are inside (last column/row); +0.001 beyond is NoData.
- Band index must be an integer (`Band_1` fails). Location must be "x y": `abc`, `1`, `500005` give ERROR 000622 / 000628.
- Missing raster: ERROR 000732.
Gaps: GeoTIFF only, `ug run` prints the value (no Python result object).
