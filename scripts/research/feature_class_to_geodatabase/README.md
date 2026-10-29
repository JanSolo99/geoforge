# Feature Class To Geodatabase

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/feature-class-to-geodatabase.html

ArcGIS Pro 3.7 black-box controls; no proprietary SDK or runtime GDAL.
`probe.py`: 10 controls (7 success/3 expected000732 refusals), disposable outputs
`/tmp/unigis-fctg-probe`. `probe_options.py`: 3 additional successes, defaults,
aliases/domains/indexes and invalid-character naming. Saved JSON retains evidence.
Run scripts through `uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py`.

`make_fixture.py` creates immutable `testdata/geoproc/fctg_empty.gdb`,
`fctg_seed.gdb` (existing parcels and empty group), and `fctg_names` once; refuses
to overwrite. It derives the seed from the initial public single output. Do not
regenerate fixtures to rerun tests. The generator's13 `fctg_*` cases (10 successes,
3 expected failures) save complete output workspaces. All2415 old cases unchanged.

Observed default behavior:
- Existing output FileGDB required; root or feature-dataset destination.
- Output names from input basenames, in input order. Repeated/existing names get
  _1, _2 etc; existing datasets retained. `9 bad-name` becomes `T9_bad_name`.
- New consecutive ObjectIDs1..n replace source IDs, including gaps.
- Attribute aliases, defaults, NULLs retained. Domains and attribute indexes are
  omitted with default transfer-field-properties environment. Relationships not
  copied. Geometry field is `Shape`; polygons add `Shape_Length`/`Shape_Area`,
  polylines add `Shape_Length`, required and noneditable but nullable. Measures
  computed after coordinate snapping to the output resolution.
- Tables, missing inputs and nonexistent output workspace fail000732. A batch
  containing a valid input followed by missing input creates no output classes.

Native implementation: registry report + optional writable-host capability,
SHP/FileGDB input paths, one FileGDB rollback transaction for the whole batch.
Input rows loaded before class writes; batch encoding/projection failure rolls
back earlier classes too. Same-workspace copies read all source data first.
Opt-in writer mode adds standard geometry spelling/measure metadata/calculation;
other writer users retain their existing mode. Namespace checks include empty
feature datasets through the writer catalog (reader's empty-dataset enumeration
is limited). Quoted semicolon-separated paths accepted.

Validation artifacts: generic `tests/common/workspace_reference.rs` checks catalog,
class membership, complete attribute fields/defaults, rows/OIDs, geometry and exact
XY storage. CRS comparison uses canonical Esri WKT: ArcGIS binary WKT degree-unit
factor is0.017453292519943295, a PRJ source rounds it to0.0174532925199433; native
retains source WKT precision. The initial reference run matched all schema/rows/
geometry but identified that raw-string comparison. Native independent public
ArcGIS reader matched10 workspaces (15 classes), exact field metadata including
required/editable flags, indexed fields and rows; `native_reader_results.json`.
Its disposable config is `/tmp/unigis-fctg-native-config.json`; prepare recipe
was `/tmp/unigis-fctg-native-prepare.py`. No public outputs were regenerated to
adjust comparisons.

Limits: no overwrite/transfer-field-properties/preserve-GlobalIDs or other
nondefault environments, session layers/selections, enterprise/mobile inputs or
outputs. GlobalID/blob/raster/XML fields and multipatch imports refused. Nonstandard
geometry-measure field names (length on points, area on lines, etc.) are refused
to prevent user values from being interpreted as calculated geometry fields. ASCII
input-derived names up to150 characters; full reserved-word/Unicode naming and
schema qualification unverified. Subtypes/rules/attachments/controller metadata,
class-level aliases and advanced default conversions not transferred/verified.
Cross-CRS dataset projection uses existing native PROJ bridge; datum transforms,
custom CRS, Z/M/curves, high-precision dates and64-bit OID cases unverified. Loads
all rows in memory. Native catalog/XML/index layouts need not be byte-identical
and WKT spelling/precision may differ while the CRS is equivalent. No new output
workspace is created; existing destination handling is additive and transactional.
