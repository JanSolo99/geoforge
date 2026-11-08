# Current handoff

Updated 2026-11-07T23:16:52.182697+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `1e8a83be`.

## Single item: app.create-features-pane — implemented, validated, landing (Claude, 2026-11-08)

Previous item (basemap gallery) landed as e0276cbc.
- New Create Features dock pane (`ui-egui/src/createpane.rs`, `Pane::CreateFeatures`, command
  `ui.pane.createFeatures`, Edit › Features › Create Features): editable layers with templates
  (saved ones, else automatic from unique-values classes or the layer name), search box,
  construction tools. Choosing a template selects its layer, saves an automatic template, sets
  the tool. Edit › Create is unchanged.
- Engine: `Tool::CreateShape(rectangle|circle|ellipse)` (codes createRectangle/…), `tool.set`
  accepts them on polygon layers, `edit.addFeature` accepts `shape` + two `points` (reuses the
  graphics geometry builder); mapview drags them like graphic shapes. Tests: engine
  `polygon_features_from_construction_shapes`, pane unit tests, smoke test.
- 10 new strings in all 15 catalogs. Docs: editing guide, roadmap, KNOWN_GAPS (gaps: `.lyrx`
  templates, Arc/Trace/Right Angle/Midpoint/Endpoint Arc/Tangent/Bezier/Freehand/Stream/
  Auto-Complete Polygon tools, in-pane template properties), priority ticked, queue regenerated.
- Validation: app fmt, clippy `-D warnings` (all targets), `cargo xtask layers`, wasm32
  `rustgis-web` build, ui-egui tests (55 + 29), engine tests (126, `--test-threads=1` because
  of the known parallel plugin flake); headless screenshot checked. No interactive native GUI
  session. Root `scripts/check` result in the landing report.
- Inherited uncommitted work (raster umintfloat hunks, dashboard.html, agent files) stays out.

Next: app priority `gdb-domains-subtypes` (large: read/write coded value and range domains and
subtypes in file geodatabases, then the tools). Split it before starting.
