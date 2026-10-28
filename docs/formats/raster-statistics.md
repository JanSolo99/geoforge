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

Copy Raster can store a physical NoData tag that differs from the marker used for
its cached statistics. U8 arguments -0.25/-0.5 retain that raw TIFF tag but exclude
rounded0 from statistics; the original127 sentinel becomes ordinary data. The6
raw cells are valid by the tag, while COUNT is5. WriteOptions.nodata_tag provides
an opt-in physical override without changing logical sample encoding or cache
computation. See [fractional controls](../../scripts/research/copy_raster/fraction/README.md).

For tagged same-type Copy Raster, cached minimum AND maximum select embedded
statistics even if other fields are missing. If either mean or standard deviation
is missing, ArcGIS resets BOTH to0/-1. Missing count becomes0, median becomesnan,
excluded-values becomes an empty field, and skip factors become1. Existing values survive, including stale complete
moments/counts and existing median/skip/covariance/excluded-value fields. Missing
minimum OR maximum instead triggers fresh PAM statistics. Evidence:
[11 partial/stale cache controls](../../scripts/research/copy_raster/cache/README.md).

Cache planning requires minimum and maximum in EVERY band of the default domain.
A missing extremum anywhere makes Copy Raster recompute ALL bands into PAM and
keeps F64 for Abs/Negate/Square. Complete caches permit preserved embedded copies
and F32 narrowing. Keys cannot be paired across different bands or custom domains.
The same parser handles valid single/double-quoted XML and embedded/PAM per-key
precedence. Raster.statistics exposes the union of all stored band extrema for
a complete cache, used for the dataset's common output type. Per-band stored
metadata remains independent and is preserved by stats_metadata::read.
See [20 multiband controls](../../scripts/research/copy_raster/multicache/README.md).


Cached integer Abs/Negate/Square now apply that union to every band's planner.
A nonnegative output range reaching127 promotesS8 toU8/NoData255; signed ranges
can retain127. This matches the existing strict nonnegative signed32 endpoint
rule. Direct GP tool execution can differ from SA Raster.save: when an integer result
band is entirely missing and its NoData marker cannot fit storage, GP recomputes
statistics from stored zeros for every band. Counts include those pixels. GP
initializes covariance vectors with sample variance on the diagonal and zeros
off-diagonal, verified with correlated bands. This also applies to single-band
all-empty results. The native Output covariance_statistics policy requests that
PAM initialization. Existing source cross-covariances stay untouched.
Complete multiband caches with nonfinite/inverted extrema retain presence via
NaN planning extrema and are refused by registered integer unary tools. The
single-band reader behavior and per-band metadata are unchanged. Missing extrema
still select uncached planning; stale/empty-source cache semantics need probes.
See [36 cached integer controls](../../scripts/research/unary_multiband/cached/README.md).
