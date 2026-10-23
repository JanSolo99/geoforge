# Spatial weights matrix files (`.swm`)

Worked out black-box by writing files with ArcGIS Pro 3.7 (Generate Spatial Weights Matrix) on small
inputs and reading the bytes; probes in `scripts/research/spatstat/weights/`, reader/writer in
`crates/unigis-tools/src/swm.rs`.

## Layout

1. A text header, `KEY@value` pairs joined by `;`, ended by `\n`. Keys, always all present and in
   this order (`#` = not applicable):
   `VERSION` (10.1), `UNIQUEID` (field name), `SPATIALREFNAME`, `INPUTFC` (input path), `WTYPE`
   (0 inverse distance, 1 fixed distance, 2 K nearest, 3 Delaunay, 4 contiguity edges, 5 contiguity
   edges+corners, 6 converted table, 9 space-time window), `DISTANCEMETHOD` (EUCLIDEAN/MANHATTAN),
   `EXPONENT` (`1`, `2`, `1.5000`), `THRESHOLD` (`%.6f`), `NUMNEIGHS` (K nearest; 0 for contiguity),
   `INPUTTABLE`, `TIMEFIELD`, `TIMETYPE` (DAYS...), `TIMEVALUE`, `INPUTNET`, `IMPEDANCEFIELD`,
   `BARRIERFC`, `UTURNPOLICY`, `RESTRICTIONS`, `USEHIERARCHY`, `SEARCHTOLERANCE`, `ADDCONCEPT`,
   `FIXEDWEIGHTS` (True when a row's weights are all equal), `HASZ` (None; False for tables),
   `HASID64`, `MASTERISOID` (the ID field is the OID), `WEIGHTFIELD`, `WEIGHTINGMETHOD`
   (UNWEIGHTED, BISQUARE, GAUSSIAN, TRIANGULAR, QUADRATIC, FIELD, BORDER), `ISSELECTION`,
   `ISDEFINTIONQUERY` (sic), `INCLUDEFOCALFEATURE` (None/True).
2. Feature count and row-standardization flag (0/1).
3. Per feature, in input order: ID, neighbor count `k`; if `k > 0`: `k` neighbor IDs, the weights
   (one `f64` when `FIXEDWEIGHTS@True`, else `k`), and the row's weight sum before standardization
   (`f64`).

Integers are little-endian `i32`, all `i64` (count, flag, IDs, `k`) with `HASID64@True` (a
BigInteger ID field). A converted table whose ID comes back after other IDs gets an extra record
after the counted ones.
