# Raster projection assignment and GeoTIFF CRS keys

ArcGIS Pro 3.7 black-box observations are saved in `define_observations.json`.
`probe_define.py` copies a tiny imported raster before calling DefineProjection.
All five codes (4326, 32633, 3857, 7844, 7856) preserve pixels, grid, S32 type
and NoData while assigning the requested coordinate system.

Regenerate saved reference cases (requires the ArcGIS bridge):

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'defras_*'
```

The generator copies the TIFF and statistics sidecar as files before the in-place
ArcGIS operation. CopyRaster is deliberately avoided because it can replace NoData.
Native use: `ug run define_projection input=in.tif output=out.tif coordinate_system=7844`.
The EPSG: prefix is also accepted. Native output is a rewritten TIFF; coordinates
are unchanged. This is assignment, not reprojection.

Supported scope: readable GeoTIFFs, geographic/projected EPSG codes representable
inline in GeoTIFF keys. Invalid/unknown codes, custom WKT/PRJ definitions, geocentric
CRSs and auxiliary XML SRS overrides are refused before writing. Output can equal
input. Ancillary TIFF tags, colormaps, compression/layout, pyramids and arbitrary
sidecars are not preserved wholesale. Existing source files remain untouched when
writing a distinct output; statistics are rewritten when present in the source.

The previous 4000..4999 classification misses geographic EPSG:7844 (GDA2020),
6318 (NAD83 2011), and others. `make_geographic_codes.py` derives a sorted static
list of geographic 2D/3D EPSG identifiers from public PROJ `proj.db` metadata.
Run it from the repo root, followed by `cargo fmt --all`, to regenerate. It adds
no runtime PROJ/GDAL dependency to the raster crate. See the generated file header
for the database version/date. EPSG types are identifiers, not numeric ranges.

Tests inspect raw ModelType/GeographicType/ProjectedCSType GeoKeys as well as GDAL's
resolved CRS: GDAL can repair the old inconsistent GDA2020 keys using EPSG lookup,
so resolved CRS alone would miss the bug. CLI tests cover two F64 bands, exact
values/grid/type/NoData, replacement, in-place assignment, mixed-case suffix/prefix,
invalid codes and source/destination auxiliary SRS refusal without file mutation.

Primary public references:
- https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/define-projection.html
- https://proj.org/en/stable/resource_files.html#proj-db
- https://github.com/OSGeo/PROJ/blob/master/data/sql/geodetic_crs.sql
