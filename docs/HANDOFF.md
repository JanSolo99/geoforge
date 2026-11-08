# Current handoff

Updated 2026-11-07T23:16:52.182697+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `1e8a83be`.

## Single item: app.gdb-design-write-back (domains) — implemented, validated, landing (Claude, 2026-11-08)

Previous items landed: basemap gallery e0276cbc, Create Features pane b0ccd980, geodatabase
domains/subtypes read side 10208d09.
- `filegdb::save_domains` (data crate) writes a layer's field domains through the writer's
  ArcGIS-validated operations: create a missing domain (`<class>_<field>` when the app made it),
  update a coded domain's codes/descriptions or a range's limits, assign it to its field. Save
  Edits (`catalog.saveEdits`) calls it after the rows; a failure leaves the rows saved and
  notifies. `design::Domain` gained `name` (the geodatabase domain it came from, serde default).
- Tests: engine `domains_are_saved_back_to_geodatabase_classes` (changed, idempotent second
  save, created + assigned, on copies of `testdata/gdb-reference`); the existing Save Edits test
  still passes.
- Not done (new open priority `gdb-subtypes-relationships`): subtypes write-back, removing a
  domain from a field, relationship classes, pick list in the Attributes popup / Create pane.
- Validation: app fmt, clippy `-D warnings`, layers, wasm32 `rustgis-web` build, map/data/ui tests,
  engine tests (`--test-threads=1`, known parallel plugin flake). Root `scripts/check` result in the
  landing report. No GDAL cross-check of the written files in the app tests (the writer functions
  are validated against ArcGIS at the root).
- Inherited uncommitted work (raster umintfloat hunks, dashboard.html, agent files) stays out.

Next: `gdb-subtypes-relationships`, or `unigis-naming` (priorities are in order).
