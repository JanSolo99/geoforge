# Current handoff

Updated 2026-11-07T23:16:52.182697+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `1e8a83be`.

## Single item: app.basemap-gallery — implemented, validated, landing (Claude, 2026-11-08)

Finished Codex's unvalidated start (`ui-egui/src/basemaps.rs`, ribbon/startup/plugin cleanup,
15 translated catalogs, smoke tests).
- Map › Basemap is one gallery (search, 5 offline, 34 online, 9 overlays, tile-layer list,
  Clear Basemaps/Overlays); the pinned Basemaps plugin download is gone; a saved
  `plugin:basemaps` tab opens Map and its tab is hidden; the installed plugin is untouched.
- Fixes this turn: the gallery menu uses new `widgets::big_menu_stay` (CloseOnClickOutside) —
  with the default, clicking the search box closed the menu; search text resets on choosing;
  framed search box (visible in the headless screenshot); smoke test types via the TextInput
  role. Add Tile Layer… closes the menu.
- Validation: app `cargo fmt --check`, clippy `-D warnings` (workspace, all targets),
  `cargo xtask layers`, wasm32 `rustgis-web` build, `rustgis-ui-egui` tests (52 + 28 pass;
  one Add Data folders test failed once, then passed 4 reruns: timing flake, not changed here),
  engine/plugins tests pass with `--test-threads=1` (the known parallel flake). Headless
  screenshot checked (`ui_shot`). No interactive native GUI session was run.
- Docs: guide (`app/docs/user-guide/maps.md`, getting-started), roadmap, priority ticked,
  queue regenerated. Root `scripts/check` result in the landing report.
- Inherited uncommitted work (raster umintfloat hunks, dashboard.html, agent files) stays out.

Next: app priority `create-features-pane`.
