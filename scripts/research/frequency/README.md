# Equal To / Greater Than / Less Than Frequency

Native registry ids: equal_to_frequency, greater_than_frequency, less_than_frequency.
ArcGIS Pro 3.7 is the behavior oracle. GDAL creates/inspects fixture files only;
there is no new runtime dependency. The initial ug inventory lacked all three.

Regenerate the63 saved cases:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'freq_*'
```

SINGLE_BAND (default) compares the first value band against every band of every
comparison raster. Duplicate inputs count separately. MULTI_BAND compares matching
band numbers: a single value band repeats across the greatest comparison band
count; a multiband value raster sets the output band count. Missing comparison
bands produce an entirely NoData result band; extra comparison bands truncate.
There is no automatic repetition of a single comparison band. Any participating
NoData propagates; unused value bands do not affect SINGLE_BAND results.

Comparisons retain source precision: F32 0.1 and F64 0.1 differ. Greater/Less count
list values greater/less than the value raster. Count types/tags depend on the
number of comparisons (flattened bands for SINGLE_BAND, list items for MULTI_BAND):
1/3/15 use U8/16;2 uses U8/3;4..14 U8/15;16..127 I8/-128;128..254 U8/255;
255 U8/256;256 U16/65535. Boundary/interior probes verify each transition.
Entirely NoData bands carry ArcGIS statistics count/min/mean0 and max/stddev-1;
this policy is explicit and does not alter other tools' empty-band behavior.

Scope: GeoTIFFs, exactly matching grids/CRS, at most256 comparisons and finite data
or NoData. Nondefault environments, resampling/reprojection and multidimensional
CRF processing are unsupported. The255-comparison case with NoData is refused:
ArcGIS stores physical zero with an out-of-range256 tag, while the current native
writer saturates that missing marker. GDAL sees no standard mask (flags1/all valid).
Do not assume mask support or silently write an incorrect255 cell; probe the
encoding separately before extending support.255 comparisons without NoData work.

62 preliminary probes are saved in *_observations.json and probe_*.py: baseline15,
boundaries8, precision9, broadcast18, order5, interior7.63 reference cases additionally
cover zero counts, output type transitions, NoData, varied band values and missing
bands. The CLI safety tests verify empty/oversized lists, CRS mismatch and the
unsupported255-with-NoData case preserve inputs and an existing output. Empty
Rank/Popularity lists now return000735 instead of panicking (separate regression).

Run make_inputs.py from repo root with GDAL Python bindings/NumPy to reproduce
four tiny typed band fixtures. Existing copy_raster/composite_matrix fixtures are
reused. Probe scripts use private /tmp copies and source cells from these fixtures.

Primary public references:
- https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/equal-to-frequency.html
- https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/greater-than-frequency.html
- https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/less-than-frequency.html
