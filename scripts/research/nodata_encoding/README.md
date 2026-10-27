# Out-of-range integer NoData encoding

ArcGIS frequency with255 comparisons produces U8, NoData256. A missing result
is physically zero; GDAL reports mask flags1/all valid. Cached integer math also
can retain NoData -f64MAX on S32 and write missing/overflow cells as zero. Their
statistics count logical valid cells before encoding, not every physical zero.
Evidence: research/frequency/interior_observations.json and unary_types/cached32.

WriteOptions.zero_out_of_range_nodata is an explicit GP output policy. It maps
logical NoData outside integer sample bounds to zero in every band/level, keeps
the metadata tag, and leaves the input unchanged for statistics. Default format
writes retain the previous sample conversion. CLI GP writers opt in. Floating
rasters and representable integer markers retain their existing conversion.

Three freq_*_count255_nd references cover Equal/Greater/Less Than Frequency;
regular and BigTIFF tiled/striped tests compare cells/tags with GDAL and verify
that logical cells/statistics are preserved. The frequency255 refusal is removed.

Copy Raster probes (copy_observations.json) remain separate tool behavior:
explicit out-of-range NoData can leave the old sentinel payload as ordinary data,
while in-range fractional markers are rounded by ArcGIS. Native override/remapping
and tiny-input statistics still need work; this writer policy does not solve those
Copy Raster parameter differences. No fractional formatting behavior was changed.

Raw GDAL inspection confirmed all six probe arrays/tags match the printed fields;
NoData300.5 on U8 becomes300, default -1.25 on S16 becomes-1, and -9999.5 becomes
-10000. These observations are not guessed from the output pixel range.
