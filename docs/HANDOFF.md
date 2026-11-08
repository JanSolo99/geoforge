# Current handoff

Updated 2026-11-07T23:16:52.182697+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `1e8a83be`.

## Single item: app.gdb-subtypes write-back — implemented, validated, landing (Claude, 2026-11-08)

Previous items landed: basemap gallery e0276cbc, Create Features pane b0ccd980, domains/subtypes
read 10208d09, domains write-back 7bfd11c5.
- `filegdb::save_subtypes` (data crate), run by Save Edits after the rows and domains: sets the
  subtype field when the table has none (refuses a different existing field), removes dropped
  subtypes, adds new ones, assigns differing per-subtype defaults. Errors leave the rows saved and
  notify. Test `subtypes_are_saved_back_to_geodatabase_classes` (no-op save, add + drop +
  default, idempotent re-save) on a copy of `testdata/gdb-reference/subtypes.gdb`.
- The remaining scope of the priority `gdb-subtypes-relationships` is reduced: domain removal on
  save, per-subtype domains/default subtype, relationship classes, pick list in the Attributes
  popup and Create pane (KNOWN_GAPS lists them).
- Validation: app fmt, clippy `-D warnings`, layers, wasm32 `rustgis-web` build, map/data/ui tests,
  engine tests (`--test-threads=1`, known parallel plugin flake). Root `scripts/check` result in the
  landing report.
- Inherited uncommitted work (raster umintfloat hunks, dashboard.html, agent files) stays out.

Next: `gdb-subtypes-relationships` (remaining parts) or `unigis-naming`; naming is a packaging
rename with clear steps and may be the better next item.
