# Raster statistics storage

ArcGIS Pro 3.7 uses both PAM `.aux.xml` and TIFF `GDAL_METADATA` (tag42112).
A missing statistics sidecar does not imply an uncached raster. Cached Copy Raster
outputs reuse source statistics inside TIFF. Many Spatial Analyst outputs also
put newly computed statistics there. These are needed for subsequent tools'
statistics-dependent output-type choices.

The TIFF tag is ASCII GDAL metadata XML. Band indexes are zero-based:

```xml
<GDALMetadata>
  <Item name="STATISTICS_MINIMUM" sample="0">1</Item>
  <Item name="STATISTICS_MAXIMUM" sample="0">4</Item>
</GDALMetadata>
```

PAM uses one-based `PAMRasterBand band="1"`, with `Metadata/MDI key="..."` items.
The native reader merges embedded statistics with PAM overrides per band/key;
nondefault domains and out-of-range band indexes are ignored. Reading never
recalculates a cache. The writer's optional `WriteOptions.gdal_metadata` adds the
tag to the main image in classic TIFF or BigTIFF. `stats_metadata` reads/encodes
cached fields or computes a new per-band cache. XML text is escaped.

Cached counts and moments can differ from a fresh pixel calculation. Copy Raster
with UInt32 marker2147483648 reuses COUNT5 even though signed metadata makes all6
raw cells valid. A legacy GDAL cache lacking COUNT exports COUNT0. Expand DISTANCE
stores statistics including its right/bottom expansion workspace, so COUNT can
exceed the saved cell count. Preserve these observed semantics; do not normalize
them to the saved pixel population.

CLI output replacement removes previous PAM overrides after the TIFF write
succeeds, then writes new PAM when required. This also handles an all-NoData
result for which there are no new statistics. Cached in-place source metadata is
read before replacement. Other auxiliary formats are outside this lifecycle.

Evidence and reproduction: [Copy Raster research](../../scripts/research/copy_raster/u32/README.md).
The generic ArcGIS comparison checks cached values in both stores and keeps the
sidecar-placement check. Native/GDAL format tests cover band metadata, PAM
precedence, classic TIFF and tiled BigTIFF. Median/covariance/histogram computation
and wholesale ancillary metadata preservation are not implemented; existing
cached statistics fields are retained when copied.
