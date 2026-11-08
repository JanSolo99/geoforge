# Set Raster Properties (ArcGIS Pro 3.7)
Probes: `probe.py`, `probe_extra.py` (results `*.jsonl`) on copied TIFFs; references via
`scripts/make-arcgis-reference.py 'srp_*'` (testdata/arcgis-reference/srp_*).
- Data type -> PAM `DataType` item (Generic, Elevation, Thematic, Processed, Scientific, Vector-UV,
  Vector-MagDir, StdTime for DATE, SAR). Statistics -> `STATISTICS_*` per band; values `%.14g`.
- NoData -> TIFF tag 42113 (single value). Out-of-range stats: ERROR 001192; bad band 000622/000800;
  bad type ERROR 000800.
- Multi-band: setting statistics on one band removes the empty EXCLUDEDVALUES item of the others.
