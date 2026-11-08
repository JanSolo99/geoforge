# Current handoff

Updated 2026-11-08T04:27:11.689662+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, product HEAD `be04da46425d66c170703c9e0d06fd125d4f3ab6`.

## Single item: app.gdb-subtypes-relationships

Complete and landed. Commit `be04da46` is pushed to `origin/main`; remote hash
verified equal to local HEAD after the push (exit0). This checkpoint is docs only. User authorizes commit/push for this one
item. Prior naming work was already landed/accepted. No agents/coordinator launched.
Root/app instructions and development skill read. No product edits inherited.

Changes: explicit field-domain and all-subtype-code removals; default subtype and
per-subtype domain/default persistence; typed choices in table/Create/Identify;
FileGDB relationship metadata, including junction keys. Legacy missing metadata is
preserved; tombstones distinguish deletes. Shared domain copies within a layer stay
consistent; conflicting named definitions refuse before row writes. Imports bound
copied domain metadata to 32 MiB. New subtypes ignore dataset-level rules; existing
legacy None metadata remains unknown. ArcGIS3.7 saved subtype_ops/relationship_tables
fixtures and official docs resolve behavior. Evidence and remaining limits:
`scripts/research/app_gdb_design/README.md`, `docs/KNOWN_GAPS.md`.

Landed 20 owned paths: app data/schema/tests, map design, engine design/layer/tests,
UI createpane/editpanes/mapview/table/widgets/smoke, editing guide; research README;
APP_PRIORITIES/ROADMAP/KNOWN_GAPS/WORK_QUEUE/HANDOFF. Inherited root source/reference/
agent/dashboard edits remain unstaged; untracked worktrees/research untouched.
Hashes `/tmp/gdb-design-inherited.json`; protected verifier hash
`/tmp/gdb-design-verifier.sha256`. Hash checks matched before staging.

Validation on final code (Cargo -j3, ext4 target cache):
- `cargo xtask ci`: fmt/strict workspace clippy PASS. Workspace tests stop at six
  documented unchanged plugin-fixture failures (`Forward/Make Points: nope`),
  engine 129 pass/6 fail/1 ignored. `/tmp/gdb-design-app-ci.log`; not full app CI PASS.
- `cargo test -j3 -p rustgis-engine --features unigis gdb_`: PASS3.
  Existing Save Edits regression filter: PASS3. Hostile domain-fanout test: PASS1.
- `cargo test -j3 -p rustgis-ui-egui --features unigis -- --test-threads=1`:
  PASS55unit+31smoke. Final logs `/tmp/gdb-design-final-{engine2,ui2}.log`.
- `cargo xtask layers`: PASS; `cargo check -j3 -p rustgis-web --target
  wasm32-unknown-unknown`: PASS. Release locked desktop build PASS.
- Native release check with actual X11 controls on disposable fixture copies:
  default selector saved2; att4/mn junction metadata visible; Create made fid51
  Int2/Float20; Identify switched to1 and offered only Shallow, saving Float10.
  Save Edits plus fresh source reopen confirmed51rows, Int1/Float10, default2 and
  both subtype domains/defaults. `/tmp/gdb-design-native/` holds local screenshots,
  projects/logs. All XDG paths isolated; own GUI exited, relay/token cleaned up.
- Root `CARGO_TARGET_DIR=/home/jansolo/.cache/geoforge-target CARGO_BUILD_JOBS=3
  scripts/check`: PASS (PTY session10453 exit0, `/tmp/gdb-design-root-check2.log`,
  root-check2.exit=0). Covers final source edits; subsequent edits docs only.
  First non-PTY run ended143 with no failed tests; never counted as PASS.
- Product push pre-push `scripts/check`: PASS; push exit0, log
  `/tmp/gdb-design-push.log`. Remote `main` verified as `be04da46`.

No incomplete product edits or running app/build/relay processes. Inherited source,
reference and agent hashes still match; post-commit generated dashboard remains
unstaged. Protected verifier unchanged. Queue regenerated after landing.

Next queue item: Delete Raster Attribute Table. Do not start it in this turn;
coordinator checks acceptance first. Remaining app polish is documented in
KNOWN_GAPS and requires a user request.
