# Validate Join (management.ValidateJoin), ArcGIS Pro 3.7

Implemented as `validate_join` (crates/unigis-tools/src/validate_join.rs); the output is the
optional `output_msg` table. Reference cases `vj_*` (parcels.shp with dates.dbf).

- match_count: input rows whose key equals some join key (one-to-one; numbers by value, text
  never matches numbers). parcels.ZONE (0..3) vs dates.GRP (0..2): 27; reversed: 25;
  PARCEL_ID 1..36 vs GRP: 2; VALUE vs ID: 0.
- output_msg (TYPE, TABLE_NAME, FIELD_NAME, DESC_, all text 254): for each join field without
  an attribute index a row whose TYPE is the field name; `GPM_NO_MATCH_JOIN` "Join had no
  matches." (also WARNING 003237); `GPM_ROW_COUNTS` "The input table has N and the join table
  has M records." TABLE_NAME is `<folder>.<file>`; FIELD_NAME is blank.
- A table joined to itself fails: ERROR 001706 "Cannot join a table to itself."
- Not modelled: geodatabase tables (indexes, names), invalid-character, workspace and OID
  checks, one-to-many cardinality messages, raster inputs; match_count/row_count are not
  returned as values.
