# Enable Editing Templates (Data Management) — ArcGIS Pro 3.7

Probed 2026-11-08 on copies of every geodatabase in `testdata/gdb-reference` and
`testdata/geoproc`, and on a fresh `CreateFileGDB`.

Observed:
- On a file geodatabase the tool succeeds in ~0.01 s and **changes no file** (every file's MD5 is
  the same afterwards, also on a second run and on a new geodatabase). Parameters: `in_workspace`,
  derived `out_workspace`.
- A folder that is not a geodatabase, a missing path or a missing `.gpkg`: `ERROR 000584:
  Implementation of this Tool's Validate is invalid.` (twice). A shapefile or `memory`:
  `ERROR 160632: This functionality is only supported on a Geodatabase release of ArcGIS 10.0 or
  greater.` A feature class path as workspace: `ERROR 000464` (schema lock; not reproduced).
- The tool's effect is not visible on disk for a file geodatabase, so nothing is written here either.

Implemented as `ug run enable_editing_templates in_workspace=…`; the reference scenario
`gsc_templates` (create gdb, create class, enable twice) compares the final geodatabase with
ArcGIS's, and `crates/unigis-cli/tests/enable_editing_templates.rs` checks the byte-identical
result and the error codes.
