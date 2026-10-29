# Excel To Table

Public reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/excel-to-table.html
Native reader: calamine 0.37 (MIT, pure Rust; Rust 1.88),
https://docs.rs/calamine/0.37.0/calamine/ . No GDAL runtime or Esri SDK.

## Controls and implementation

`make_fixture.py` uses openpyxl/xlwt to create saved .xlsx/.xls inputs in
`testdata/geoproc/excel_input.*`. Do not regenerate fixtures merely to rerun
controls. `probe.py` uses public ArcGIS Pro 3.7 calls on those files; 26 results
(18 successes, 8 failures) in `results.json`, with both dBASE and FileGDB outputs.
Run using `uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py
scripts/research/excel_to_table/probe.py`. Disposable outputs `/tmp/unigis-excel-probe`.
Generator `excel_*` cases provide 13 portable dBASE references (9 successes,
4 expected failures); the CLI integration test also compares native FileGDB
schema, aliases, types, values and NULLs with the 8 supported saved GDB controls.

Observed:
- Default first sheet; explicit sheet or single rectangular .xlsx named range.
- Headers are absolute one-based worksheet rows, skipped wherever they occur;
  row0 means column-letter names. Rows before a later header remain data.
- Cell ranges select data and type inference, but use the configured absolute
  header row even if outside the data rectangle. Selected end rows beyond used
  rows are clipped. Named ranges use their first row as header.
- Default range starts at row1/columnA; interior and leading blank rows retained.
- Numeric integral columns are Long, then BigInteger outside signed32; dBASE
  maps BigInteger to Double. Fractional values make a Double column. Boolean
  columns are Short. Blank numeric values stay NULL in FileGDB, become0 in DBF.
- Mixed numeric/text columns stringify values (`3.0` becomes `3`), and blanks
  become empty strings. Actual Excel #N/A errors become NULL. Other errors fail.
- Text fields are255 FileGDB/254 DBF in these controls. Headers are validated,
  deduplicated and retain aliases in FileGDB; DBF does not retain aliases.
- dBASE date output drops time. Time-only values use 1899-12-30 in the control.
- Empty worksheet, unknown sheet, reversed range and out-of-bounds header fail.
  Native failure messages do not reproduce ArcGIS Python traceback text.

The shared reference harness now honors `table_output` metadata for failed cases
with no saved DBF, and accepts a handled CLI error for ArcGIS failures without a
numeric error code (still requires matching codes when present; panics rejected).
The initial run matched all9 successful Excel cases but exposed these harness
limitations on the4 failures; no reference outputs were regenerated for that fix.

Independent native output verification: `native_reader.py` uses ArcGIS public
ListFields/SearchCursor on8 native-created FileGDB tables, requiring exact full
field metadata and rows against the original ArcGIS outputs. All8 matched;
`native_reader_results.json`. Disposable source config under
`/tmp/unigis-excel-native-config.json`, native workspace path in
`/tmp/unigis-excel-native-path.txt`; do not recreate/repeat merely for a handoff.

## Deliberate limits

FileGDB date/time and mixed date/text columns fail explicitly: calamine identifies
Excel date values but does not preserve enough number-format detail to distinguish
DateOnly, TimeOnly and Date safely. Duration cells also refused. DBF date controls
cover modern dates and time-only values; Excel1904/early1900 dates unverified.
Text exceeding default output widths is refused before writing (DBF byte limit;
GDB currently also uses this conservative UTF-8 byte limit). No formula evaluator:
only saved cached values accepted; missing caches refused. Macro workbooks,
password encryption, external/discontiguous/single-cell named ranges, arbitrary
Excel display formats, merged/header numeric/date/Unicode naming edge cases,
legacy BIFF variants, hidden-sheet ordering, custom environments and files with
styled-only trailing cells remain unverified/unsupported. Name validation covers
common ASCII characters and SQL reserved names; full ArcGIS Unicode/reserved-word
rules are unverified. Only .dbf and root FileGDB table outputs; existing destination
handling follows the shared table writer. Selection cap10million cells (after
workbook read; not a bound on workbook parser allocation).
