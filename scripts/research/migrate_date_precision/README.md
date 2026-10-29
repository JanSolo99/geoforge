# Migrate Date Field To High Precision

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/migratedatefieldtohighprecision.html

Public ArcGIS Pro3.7 controls in probe.py cover one/multiple fields, table/class,
empty/NULL data, an already-high-precision field containing123ms, wrong/missing/
mixed types, duplicates, case-insensitive names, empty fields and a shapefile.
Initial preparation using AlterField DATEHIGHPRECISION failed000800 (not accepted
by that API); no fixture was saved. Corrected preparation uses the public migration
tool for the already-high field, in a separate disposable workspace. The untouched
migrate_date_base.gdb was saved once before controls; do not recreate for context.
Shapefile fixture is copied from the disposable public control. probe_results.json
records outcomes; catalog_format_results.json records saved catalog definitions.

Valid Date fields change precision0 to1 (millisecond capability). Existing values,
NULLs, aliases, schema and geometry remain. Already-high fields and repeats succeed
unchanged. Fields are matched case-insensitively. Wrong type fails000640, missing
field000728, duplicate input000400, empty list000735; shapefile000214. ArcGIS can
partially migrate the first Date before failing on a later wrong-type field.
Native preflights the complete list and changes nothing on such a refusal; this
intentional compatibility difference is recorded in KNOWN_GAPS.

The saved file-format change is HighPrecision=true in the selected GPFieldInfoEx
entries. Other entry properties are retained; absent entries are added in field
order. The observed table version3 and field-section version4 remain unchanged,
including already-high fields. Raw doubles remain in the user table. Catalog
reads used `ogrinfo -ro -q -sql 'SELECT Name,Definition FROM GDB_Items' <gdb>` only
as a format oracle; ArcGIS is the GP behavior oracle. No runtime GDAL or SDK use.
Unrelated workspace-copy extent/index catalog normalization is not reproduced.

Native writer/date_precision.rs splices the markers within the existing journal.
No user table/header/index/geometry/value bytes are changed; no-op/repeat operations
are byte-identical. Registry date_precision::MIGRATE and optional host capability
supply CLI FileGDB table/class paths; read-only hosts explicitly refuse. Date field
lists use semicolons, with optional quotes and case-insensitive canonical matching.
Native decoding already honors the catalog precision marker.

12 mdhp_* references cover6 successes/6 refusals. The generic schema comparator
checks actual native precision flags against public field.precision, plus existing
schema/values/OIDs/geometry/CRS/domains/index definitions/repeat results. Failure
cases compare primary error codes; the native mixed-list refusal deliberately
preserves data rather than matching ArcGIS's partial mutation. Two safety tests
check exact user data/index/header bytes, no-op/repeat identity and atomic refusal.
probe_native.py independently checks native output through public arcpy.

Scope/gaps: FileGDB class/table paths. Other hosts/formats, layers/selections,
enterprise/mobile data and unusual class extensions are unsupported/unverified.
Legacy stored fractions below declared second precision, unusual field domains/
defaults/subtypes, Z/M/curves and rare invalid list syntax are unverified. No reverse
migration is provided. ArcGIS partial-failure mutation and unrelated catalog
normalization are not reproduced.

Validation commands (outcomes in docs/HANDOFF.md):

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'mdhp_*'
    cargo test -j3 -p unigis-cli --test migrate_date_precision_safety --test arcgis_reference
    scripts/check
