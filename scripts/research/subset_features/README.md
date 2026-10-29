# Subset Features — ArcGIS Pro 3.7 controls

Reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/subset-features.html

`probe.py` creates `testdata/geoproc/subset_base.gdb` once, before any controls.
It has points and records tables with7 records (OID2 deleted), KEY/LABEL/SCORE,
field aliases and a NULL text value, plus an empty point class. Do not regenerate.
Initial preparation used reserved table name `table` and failed before saving the
fixture; the corrected recipe uses `records` in a separate fixed temporary root.

Observed size and identity behavior:

- Defaults:50 PERCENTAGE_OF_INPUT. Percentages round to nearest, ties upward:
  7*50%=4,5*50%=3,7*33.3%=2. Very small positive percentages may create0 training.
- ABSOLUTE_VALUE truncates (3.6->3,1.9->1) and caps at input count. Zero/negative
  sizes and percent>100 fail000854. Empty source fails040303. Empty percentage
  partitions warn040303; excess absolute count takes all records without that warning.
- Output rows retain input order and attributes subject to the format rule below;
  output OIDs are newly assigned. Whole records, including multipart geometry,
  are partitioned. Optional test output is the disjoint exhaustive complement.

Format-specific NULL behavior was settled by `probe_nulls.py` and
`probe_null_dates.py` (results saved). Source/GDB outputs retain NULLs. For SHP/DBF,
Subset Features carries the previous written text/numeric value forward for NULLs
within EACH output partition; initial text is blank and numeric is0. The state
resets for training versus test. Date NULLs stay NULL. This is a behavior of this
tool's export, not a general dBASE rule. Native normalization is local to Subset
Features, only for .shp/.dbf paths. It does not change the source or other tools.

16 `subf_*` public reference cases (12 successes,4 refusals) save both outputs.
The partition contract checks schema, CRS, count, source-record geometry/attributes,
order, uniqueness, disjointness and coverage without demanding identical random
membership. Full all-record exports provide format baselines; source NULL flags
and the independently observed per-output carry rule determine expected NULL
values for the actual selected order. No broad field exemptions or relaxed old
comparisons. Public FileGDB outputs and supplemental NULL controls provide an
independent reader check on native results, beyond the portable references.

Native implementation uses reproducible SplitMix64/Fisher-Yates seed0 by default,
with a `seed` option; it does not reproduce ArcGIS Mersenne Twister streams or RNG
environments. Hosts need the optional `write_datasets` capability and otherwise
fail explicitly. CLI supports new SHP/DBF datasets in one folder or root classes/
tables in one existing FileGDB. Both output paths are preflighted, including aliases
and existing families. Folder writes are staged and published with exclusive hard
links, with cleanup on returned error; FileGDB additions share one journaled writer.
No selected record may be skipped silently by format conversion. Source domains
are copied to a compatible FileGDB destination when represented by input fields.

Scope/gaps: existing outputs, different workspaces, GeoPackage/JSON output and
feature-dataset destinations are refused. Folder staging requires hard-link support;
crash recovery/concurrent external writers are unverified. No selection/extent/
output-CRS environments, layers or ArcGIS RNG parity. Subtypes, relationships,
attachments, tracking, high-precision dates, 64-bit OIDs, exotic field/geometry
schemas, Unicode edge cases and Z/M/null-geometry behavior are unverified. The
partition keeps whole geometry objects, but public controls here are2D points/tables.
Native FileGDB geometry field is SHAPE versus ArcGIS Shape; independent schema
checks normalize only system-field/default-alias casing and retain raw names in
results. Attribute names/aliases remain exact. Inputs are capped at1million rows.
No runtime GDAL dependency or Esri SDK. Gate and reader outcomes are in HANDOFF.md.
