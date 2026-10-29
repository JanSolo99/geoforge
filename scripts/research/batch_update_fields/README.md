# Batch Update Fields (management.BatchUpdateFields), ArcGIS Pro 3.7 — probe notes

Not implemented yet. Probed with `arcpy.management.BatchUpdateFields(parcels.shp, out.shp,
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
Next: probe source-less rows without script (null?), DATE type, default text length rule,
script result rounding, script_file parameter; then implement with crate::pycalc.
