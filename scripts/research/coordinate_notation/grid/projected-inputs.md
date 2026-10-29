# UTM_BANDS / UTM_ZONES / MGRS / USNG input (ArcGIS Pro 3.7)

Black-box probes (arcpy ConvertCoordinateNotation, memory output, WGS84) and the reference
cases `ccn_grid_input_{mgrs,usng,utm_bands,utm_zones}_*`.

Fixtures: `testdata/geoproc/grid_input_{mgrs,usng,utm_bands,utm_zones}.dbf`, typed DBFs made
by `make_projected_inputs.py` from the recipe CSVs `projected_input_*.csv`: ArcGIS's own
output strings for the 39 `lonlat_grid.csv` points and 150 random points (seed 20261029),
then hand-written variants and invalid strings. (A CSV would gain ArcGIS's virtual
COORD_X/COORD_Y fields, see carry.md.)

Parsing (whitespace removed, case-insensitive):
- MGRS/USNG: 1-2 digit zone (1..60) + band, or a polar band A/B/Y/Z alone; two 100 km
  letters; an even number of digits, at most 10. Fewer digits give the south-west corner
  of the coarser square (`33TWL` is E 500 000, N 5 000 000). The band picks the 2 000 km
  cycle of the row letter: the smallest northing at or above the band's bottom on the
  central meridian, floored to 100 km (`33XWL...` lands at 81.58N). Odd digit counts,
  11+ digits, unknown letters fail.
- UTM_BANDS: zone + band letter (N and above = northern), UTM_ZONES: zone + N/S; polar:
  A/B/Y/Z without a zone, easting/northing relative to the MGRS polar grid origin
  (north 1 300 000 m, south 800 000 m). The digits (spaces optional) split in half, the
  easting taking the shorter half: `33T 509614` is E 509, N 614. Decimals fail.
- Zones 1 and 60 wrap across the antimeridian (`01PAM7107106909` is 179.99999758E).
  The pole has longitude 0.
- A failure keeps the row: null geometry, DDLat/DDLon 0, empty text, WARNING 001293.

Precision: the point is the exact cell corner projected back. ArcGIS's inverse transverse
Mercator differs from the exact (Krüger n^6 / PROJ) result by up to ~2.6e-10 degrees at
6 degrees from the central meridian (7.9e-12 at 2.5 degrees); latitude near the equator
agrees to ~1e-16. Neither Snyder's series nor truncated Krüger orders (n^2..n^6) reproduce
the longitude error. The difference shows in the 8th decimal of DDM minutes and DMS seconds,
so only DD_NUMERIC (8 decimals), DD_1 and DD_2 outputs are enabled for these inputs.

## Inverse transverse Mercator error survey

`arcpy.PointGeometry(...).projectAs(GCS_WGS_1984)` from WGS 1984 UTM reproduces Convert
Coordinate Notation's coordinates exactly, so the projection engine can be sampled directly.
`projected_inverse_samples.json` holds 70 samples (zone 31N, easting offset 0..600 km,
northing 0..9 300 km; values are Python `repr` of ArcGIS's X/Y). Against the exact inverse
(Krüger n^6, agrees with PROJ to ~1e-14 deg):
- On the central meridian and within ~100 km of it ArcGIS is exact to ~1e-14 deg.
- On the equator the longitude error grows like x^7 (1.0e-13 at 150 km, 1.3e-11 at 300 km,
  1.8e-9 deg at 600 km); latitude stays exact there.
- Away from the equator the longitude error changes sign with latitude (negative near 27-45N,
  positive near 63N) and explodes towards the pole (-1.6e-5 deg at 600 km, 79.6N); latitude
  errors are ~20x smaller.
This looks like a truncated power series in the easting (a Thomas/Redfearn-type expansion),
but Snyder's D^5 series is ~25x too large at the equator and truncated Krüger orders do not
fit. Not modelled; DDM/DMS outputs stay refused for projected inputs.
