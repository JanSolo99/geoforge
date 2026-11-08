# App priorities

Basic functionality and smooth operation of the UniGIS app come before more geoprocessing tools
(user, 2026-11-07: "we've put heaps of time into tools and not enough into basic functionality and
smooth operation"). `scripts/work_queue.py` copies the open items below to the top of
`docs/WORK_QUEUE.md`, and `scripts/lane claim` hands them out before any tool. Work them in order.

Format: one item per `- [ ] id: title` line, details indented under it. Tick the box (`- [x]`) when
the item has landed; the queue then skips it. Add new items in priority order. Behavior follows
ArcGIS Pro: settle questions with the ArcGIS MCP bridge, and check every fix in the real desktop app
(`cargo run -p rustgis`), not only through `ug`.

- [x] gp-empty-map: Geoprocessing tools run on an empty map
  - Bug: Create File Geodatabase from the Catalog fails with "the map has no layers".
  - Cause: `gp.run` is gated by `need_layers` (`app/crates/engine/src/cmd/gp.rs:12`), so every tool
    fails without a layer, including those that take none (Create Folder/File GDB/Mobile GDB,
    Create Random Raster, ASCII To Raster). Drop the gate; each tool checks its own inputs. Keep
    `hostile_params_never_panic` green and add a regression test.
- [x] catalog-no-freeze: Browsing big folders never freezes the app
  - Bug: opening folders under `/mnt/media/Data` (imagery, NTFS) made the app unresponsive.
  - Cause: the Catalog folder browser (`app/crates/ui-egui/src/panes.rs`, `folder_browser`) calls
    `read_dir` plus `is_dir` on every entry on the UI thread every frame. List each folder once on a
    background thread, cache it (refresh on folder mtime change, after New, and with a Refresh
    command), show "Reading…" meanwhile, and page long listings.
  - Its file filter misses imagery ArcGIS lists (`.jp2`, `.img`, `.sid`, `.ecw`, `.vrt`, `.ntf`,
    `.dem`, `.bil`, …): use the same raster extensions as Add Data (GDAL's list).
- [x] add-data-browser: Add Data browses like ArcGIS Pro (a .gdb is an item, not a folder)
  - Bug: Add Data uses the OS file picker (`rfd::pick_file`, `app/crates/ui-egui/src/lib.rs`
    near `ui.addGeodatabase`), which can only open a `.gdb` as a folder.
  - Build ArcGIS's Add Data browse dialog in the app: folders, `.gdb` / `.gpkg` / `.sde`-style
    containers shown as one item that expands to feature datasets, feature classes, tables and
    raster datasets; multi-select; filter by type; favourite/recent folders; and the same
    background listing as the Catalog. The OS picker can stay as a secondary "Browse…".
- [x] basemap-gallery: One built-in Basemap gallery on the Map ribbon
  - Basemaps must not need the Basemaps plugin. Today there is a separate Basemaps ribbon tab
    (plugin) and a Basemap button on the Map ribbon; fold everything into Map › Basemap like
    ArcGIS Pro's gallery, and remove the extra tab.
- [x] create-features-pane: ArcGIS Pro's Create Features pane
  - Today Edit › Create creates a feature for the layer selected in Contents (keep that working).
  - Add the Create Features pane: feature templates per layer (default attributes, auto-generated
    from symbology classes), templates from `.lyrx` files, a template search box, and the
    construction tools ArcGIS offers per geometry type (Point; Line, Polygon with Line, Arc,
    Trace, Right Angle, Midpoint, Endpoint Arc, Tangent, Bezier, Freehand, Stream; Rectangle,
    Circle, Ellipse, Auto-Complete Polygon). Snapping and the Edit ribbon follow Pro.
- [x] gdb-domains-subtypes (read side): Geodatabase domains and subtypes
  - Feature classes have no domains, subtypes or related schema yet. Read and write coded value
    and range domains and subtypes in file geodatabases (clean-room, GDAL OpenFileGDB docs; GDAL
    as the format oracle), apply them in the attribute table and editing (pick lists, validation,
    subtype-specific defaults), and add the Domains/Subtypes views and the geoprocessing tools
    (Create Domain, Add Coded Value To Domain, Assign Domain To Field, Create Subtype, …).
    Relationship classes and attribute rules come after.
- [x] gdb-design-write-back (domains): Write domains and subtypes back to the geodatabase
  - The read side landed (a class's domains and subtypes load into the layer design, the table
    shows descriptions and a pick list). Still open: Save Edits and the design panes write domain
    and subtype changes to the file geodatabase (the writer already has `create_table_with_subtypes`,
    `set_subtype_field`, `assign_domain_to_field`, domain editing; GDAL as the format oracle), read
    relationship classes, and offer the pick list in the Attributes popup and the Create pane.
- [ ] gdb-subtypes-relationships: Subtypes write-back, relationship classes, pick lists everywhere
  - Domains are written back on Save Edits and read on load. Still open: write subtypes (the
    subtype field, subtypes, their defaults and per-subtype domains: the writer has
    `set_subtype_field`, `add_subtype`, `assign_domain_to_field` with subtype codes), remove a
    domain from a field on Save Edits, read relationship classes, and offer the coded-value pick
    list in the Attributes popup and the Create Features pane.
- [ ] unigis-naming: One `unigis` program; the app is UniGIS everywhere
  - The app is called UniGIS: not RustGIS, not GeoForge. Installed program `unigis`: with no
    arguments or with files it opens the app, with a subcommand it is the CLI (today's merged `ug`
    in `app/apps/rustgis-cli`); `ug` is a shortcut to the CLI. Windows needs a console twin
    (`unigis-cli.exe`/`ug.exe`) because a GUI-subsystem program can't print to the terminal.
  - Rename the installed names (`rustgis` desktop entry, icon, MIME, AppImage, .deb/.rpm, Windows
    installers, `RustGIS-*` artifacts) to `unigis`. `ugrep` also installs `/usr/bin/ug`, so the
    distro packages ship `ug` only where it doesn't conflict.
  - The logo is UniGIS's own now (`app/crates/render/src/logo.rs`: the vector layer as a U holding
    a globe, on raster and table layers). The web app still loads RustGIS's favicon, PWA and
    Apple touch icons from `assets.rustgis.app` (`app/apps/rustgis-web/index.html`,
    `manifest.webmanifest`, `sw.js`, and `packaging/downloads/index.html`): render them from
    `render::logo` in `cargo xtask web-pwa` and serve them with the app instead.
