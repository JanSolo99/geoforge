# Current handoff

Updated 2026-11-07T23:16:52.182697+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `1e8a83be`.

## Single item: app.unigis-naming (first slice) — implemented, validated, landing (Claude, 2026-11-08)

Previous items landed: ... domains write-back 7bfd11c5, subtypes write-back 09ec41b0.
- `rustgis-cli` is now lib (`main_with`, `wants_cli`) + thin `ug`; the desktop `unigis` binary
  runs the CLI when its first argument is not a flag and not an existing path (`unigis tools`,
  `unigis run …`); otherwise it opens the app. `unigis --version` prints `unigis <v>`. Unit test
  `dispatch_tests`; the real binaries checked (`unigis --version`, `unigis tools`, `ug`).
- User docs: `rustgis-cli` -> `ug` in the command lines; cli.md explains `unigis <subcommand>`.
- Still open in `unigis-naming` (APP_PRIORITIES): Windows console twin (GUI-subsystem exe cannot
  print), renaming installed names (desktop entry, icon, MIME, AppImage, deb/rpm, Windows
  installers, `RustGIS-*` artifacts) to `unigis`, the Linux settings dir `~/.config/rustgis`
  (migrate), web icons from `render::logo`.
- Validation: app fmt, clippy `-D warnings`, layers, wasm32 `rustgis-web` build, tests (engine
  with `--test-threads=1`, known flake). Root `scripts/check` result in the landing report.
- Inherited uncommitted work (raster umintfloat hunks, dashboard.html, agent files) stays out.

Next: continue `unigis-naming` (packaging renames: packaging/*, release workflow) or
`gdb-subtypes-relationships`.
