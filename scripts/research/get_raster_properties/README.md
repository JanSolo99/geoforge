# Get Raster Properties (ArcGIS Pro 3.7 probes, 2026-11-08)
- Numbers print as `%.15G`; cell size is derived from the extent (geographic 9.99999999999979E-05).
- VALUETYPE: U8 3, I8 4, U16 5, I16 6, U32 7, I32 8, F32 9, F64 10.
- MINIMUM/MAXIMUM/MEAN/STD return the *stored* statistics text (TIFF GDAL_METADATA / .aux.xml),
  never computed; no stored stats -> ERROR 001100 "no statistics are available".
- ANYNODATA over all bands when no band is given (later_nodata.tif: band 2 only -> 1; Band_1 -> 0).
  ALLNODATA multiband without a band: all-bands rule is inferred, not probed.
- Metadata properties print UNKNOWN (SOURCETYPE `Generic`). UNIQUEVALUECOUNT is rejected (ERROR 000864).
- Band must be `Band_N`; `1` or a missing band gives ERROR 000864/000800. Lowercase property
  names are accepted by ArcGIS (`top` returned the minimum); we require upper case.
