# Current handoff

Updated 2026-11-07T23:16:52.182697+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `1e8a83be`.

## Single item: app.gdb-domains-subtypes (read side) — implemented, validated, landing (Claude, 2026-11-08)

Previous items landed: basemap gallery e0276cbc, Create Features pane b0ccd980.
- The priority was large, so it is split: this landing is the read side; the write side is the new
  open priority `gdb-design-write-back`.
- Data: `FeatureSource::schema()` -> `SourceSchema` (domains per field, subtypes); the File
  Geodatabase reader fills it. Engine: `layer.rs::apply_schema` puts it into `layer.design`
  (domains, subtypes with per-subtype defaults) for streamed and loaded layers, so the existing
  edit validation and Data Design pane see them. UI: the attribute table shows coded-value
  descriptions and edits them through a pick list (`TableState::edit_cell` starts an edit).
- Tests: engine `geodatabase_domains_and_subtypes_become_the_layers_data_design` (reads
  `testdata/gdb-reference/{domain_base,subtypes}.gdb` through the repo-relative path, skipped
  when absent), smoke `attribute_table_shows_coded_descriptions_and_picks_from_the_list`.
- Validation: app fmt, clippy `-D warnings`, layers, wasm32 `rustgis-web` build, data/ui-egui
  tests, engine tests (`--test-threads=1`, known parallel plugin flake). Root `scripts/check` result
  in the landing report. No interactive native GUI session.
- Docs: editing guide, roadmap, KNOWN_GAPS, priorities (item ticked as read side, new open item
  `gdb-design-write-back`), queue regenerated.
- Inherited uncommitted work (raster umintfloat hunks, dashboard.html, agent files) stays out.

Next: `gdb-design-write-back` (or `unigis-naming`; priorities are in order, so write-back first).
