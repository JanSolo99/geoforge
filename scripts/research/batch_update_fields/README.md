# Batch Update Fields (management.BatchUpdateFields), ArcGIS Pro 3.7

Implemented as `batch_update_fields` (crates/unigis-tools/src/batch_fields.rs); reference cases
`bfu_*` with definition tables `testdata/geoproc/bfu_def_*.csv`. Probed with `arcpy.management.BatchUpdateFields(parcels.shp, out.shp,
definition.csv, None, 'NAME', 'SOURCE', 'TYPE', 'LEN', 'ALIAS', 'SCRIPT')`.

- The output holds only the fields listed in the definition table, in its row order (plus
  OID/shape). The definition-column parameters must be given: leaving them out fails with
  ERROR 999999 (CGPBatchUpdateFields::ReadFieldD).
- Blank type: the source field's type and length (PARCEL_ID -> Long 10; NAME -> Text 20).
  SHORT -> Integer width 5, LONG -> Integer 10, TEXT with a length -> that width, TEXT without
  length from a Double source -> width 19, FLOAT/DOUBLE with decimals d -> Double 19 wide,
  precision 18, scale d; DOUBLE without decimals -> scale 0 (values stored rounded: 8597.94 ->
  8598). Values are converted to the new type (Double -> Text '8597.94'); Text 'Lot 1' -> LONG
  fails with ERROR 080466 "Failed to change the data type for target field 'E'".
- Scripts are Python expressions over OUTPUT field names (`!PID! * 10`, `!VRAW! / 1000`),
  evaluated in dependency order; a script referencing a missing output field fails with
  ERROR 080469 "'VAL_K' has a reference to a missing field(s): VALUE"; a self reference fails
  with ERROR 080464 "Cyclic reference detected for Target Field 'VAL_K'".
- Shapefile output drops aliases.
- Doubles with fixed decimals are stored rounded half away from zero (1823.35 at one decimal
  -> 1823.4, 5741.45 -> 5741.5), and scripts see the stored value (VRAW 1754.62 stored as 1755,
  so `!VRAW! / 1000` at two decimals is 1.76).
- A Date converted to TEXT uses the machine's locale (`07/02/2000 00:00:00` under Wine) and
  widened the 30-character field to 32; not reproduced (we write `2000-02-07 00:00:00`).
- A failing run still leaves a (partial) output in ArcGIS; we write nothing.
- Not done: script_file (code blocks), DATE parsing from text, aliases (kept in our field
  metadata, dropped by shapefiles like ArcGIS).
