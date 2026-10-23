# UniGIS GUI design direction

North star: **ArcGIS Pro**. The app in `app/` is a vendored RustGIS (egui). The goal is "similar but
different": follow Pro wherever Pro has an opinion (structure, names, workflows), and make our own
choices where Pro is silent or where RustGIS's look would otherwise leak through (palette, icons,
metrics). If following Pro makes us resemble RustGIS, that is fine; RustGIS was itself modelled on Pro.

Sources and confidence: the inventory in section 1 comes from the code (`app/crates/ui-egui/src`)
and `docs/screenshots/`. Section 2 comes from public knowledge of Pro (docs, screenshots, UI
conventions). ArcGIS Pro 3.7 exists under Wine here (`tools/arcgis-bridge` drives it headless through
arcpy); it was **not** launched for screenshots, to avoid disturbing the running bridge, so every Pro
colour and metric below is an approximation to be checked later with a reference screenshot, not a
measurement.

## 1. Current app inventory

Layout (`docs/screenshots/unigis-map.png`):
- Title bar (34 px, `chrome.rs::header`): logo, "UniGIS", Quick Access Toolbar (new/open/save/undo/redo),
  centred project and map title, Command Search (Ctrl+K) on the right.
- Tab strip: a filled rust "Project" button (backstage) then ten tabs: Home, Data, Style, Edit, Analyze,
  View, Publish, Plugins, Settings, Help (`ribbon.rs::TABS`). Contextual tabs are folded into these.
  Old Pro names (Map, Insert, Analysis, Share...) are mapped to them for saved layouts.
- Ribbon (about 100 px): big buttons (icon above label) and stacks of three small buttons, groups
  separated by thin rules, group labels below, a collapse chevron. Groups scroll horizontally.
- Left dock: Layers (Pro "Contents") with Drawing Order, checkboxes, inline legends; Browser (Catalog).
- Centre: tabbed map views with a "+" tab, in-map zoom buttons, north arrow, scale bar, attribution.
- Right dock (`RightPane`): Tools (Geoprocessing), Symbology, History, Details, Editing, Design,
  Workflows, Script, Tasks, Jobs, Profile, Diagnostics, Infographics, Report, Presentation, Notebook,
  Assistant, Shares, Sql. Tools pane: Find Tools search, categories with counts, tool forms with the
  Pro parameter names and a back arrow.
- Status bar: scale combo, CRS, selected-feature count, frame time. No cursor coordinates, no
  progress or notifications area.

Theme (`theme.rs`): one `Tokens` struct, light and dark, all colours read from it (widgets do not
hard-code). Cool blue-slate neutrals (`#EEF1F5` chrome, `#FBFCFD` panes), accent blue `#2563EB`
(Tailwind blue-600), brand rust `#B84A2C` on the Project tab and logo, teal `#0E9384` for contextual
tab groups. Radii 5 (controls), 8 (menus), 10 (windows). Body 12.5 px, small 10.5, heading 17; a system
sans-serif plus a semibold face, with CJK fallbacks. Row height 22 px, item spacing 6x5.

Icons (`icons.rs`, about 1050 lines): original, drawn in code on a 24 unit grid, two-tone (neutral ink
strokes with a coloured fill accent). The screenshots show that most carry colour (blue, orange).

Assessment: the structure is already close to Pro. What reads as RustGIS is mostly the **skin**: the
slate/Tailwind-blue palette, large rounded controls and pill search box, 12.5 px type, the ten-tab
layout and several non-Pro pane names ("Layers", "Browser", "Tools").

## 2. What ArcGIS Pro looks like (reference)

- **Chrome**: a title bar with the Quick Access Toolbar (save, undo, redo), project name, search box
  ("Command Search") and the user/sign-in area. Below it the ribbon tab strip.
- **Ribbon tabs**: Project (backstage), Map, Insert, Analysis, View, Edit, Imagery, Share; Pro 3.x also
  shows Help. Contextual tab sets appear with a coloured header band over the tabs when a layer or
  layout is selected: Feature Layer (Appearance, Labeling, Data), Raster Layer, Table, Layout,
  Labeling, Data Design, Chart. Groups have a label below and a dialog-launcher arrow at the corner that
  opens the matching pane. Controls: large buttons, stacked small buttons, galleries, split buttons.
- **Panes**: dockable, tabbed, auto-hideable. Contents (left, with Drawing Order / List by Selection / by
  Editing etc. buttons), Catalog (left or right), Geoprocessing (right, Favorites/Toolboxes/Portal, a
  search box at the top, "Find Tools"), Symbology (right, opened from the layer, with Primary symbology,
  Vary symbology by attribute, Appearance tabs), Attributes, Create Features, Modify Features,
  Properties, History, Notifications. Pane header is a text title, a dropdown and pin / close.
- **Views**: tabbed documents (Map, Scene, Layout, Table, Chart) with the active tab underlined; the
  attribute table docks at the bottom as a view with a field-header row, a selection toolbar and a
  record-count footer.
- **Geoprocessing flow**: pick a tool from search, tool opens in the pane with Parameters and
  Environments tabs, required parameters marked with a red asterisk, a validation badge (red x / yellow
  triangle) with the message on hover, a split Run button at the bottom ("Run" with a dropdown for
  "Run in background"/"Schedule"), a progress bar, then a success or failure message in the pane
  footer that opens the History entry. Tool help flyout with the "i" icon.
- **Status bar** (bottom, about 24 px): scale combo with navigation arrows, cursor coordinates in the
  map CRS with a unit dropdown, selected-feature count, progress ring, notification bell, and an
  editing/geoprocessing task indicator.
- **Themes**: Light, Dark and the older "Pro" theme. Default accent: Esri blue `#0079C1`. Dark theme
  is neutral dark grey (around `#2B2B2B`-`#3C3C3C`), not blue-tinted.
- **Typography**: Segoe UI (Windows), 9 pt body (about 12 px), ribbon labels 9 pt, pane titles 10.5 pt.
- **Density/shape**: flat, squared (radius 0-2), hairline 1 px dividers, no drop-shadow cards, large
  16 px and 32 px two-tone icons that use blue plus a warm accent.

## 3. Direction: follow, and differ

### Deliberately follow Pro
1. **Ribbon structure and names.** Return to Pro's tab set: Project, Map, Insert, Analysis, View, Edit,
   Imagery, Share, Help, with the same group names (Navigate, Layer, Selection, Inquiry, Labeling,
   Offline, Geoprocessing, ...). Plugins and Settings move to Project (backstage) and an extra "Add-Ins"
   tab only when a plugin registers one. The existing alias code in `ribbon.rs::current_tab` already
   maps old ids, so saved layouts keep working.
2. **Pane names**: Contents, Catalog, Geoprocessing, Symbology, Attributes, Properties, History,
   Notifications; keep UniGIS-specific panes (Assistant, Sql) under their own names.
3. **Docking**: left Contents/Catalog tabbed together, right Geoprocessing/Symbology/Properties tabbed,
   bottom table; panes auto-hide with a pin; pane header with title, a dropdown for the pane's
   modes, pin and close.
4. **Geoprocessing flow** exactly as in section 2 (Parameters/Environments tabs, asterisks, validation
   badges, Run split button, footer messages, History link). The tool pane already uses Pro parameter
   names; the chrome around it is what is missing.
5. **Symbology pane** with Pro's labels: Primary symbology (Single Symbol, Unique Values, Graduated
   Colors, Graduated Symbols, ...), Vary symbology by attribute, Appearance tab for the labelled field.
6. **Status bar** with coordinates, unit dropdown, selection count, notifications.
7. **Terminology**: Contents (not Layers pane), Catalog (not Browser), Map frame, Layout, Project item.
8. **Themes**: Light, Dark and "Pro" (the legacy dark-blue Pro theme, optional later).

### Deliberately different from RustGIS
| Area | RustGIS look | UniGIS choice |
| --- | --- | --- |
| Palette | cool blue-slate, `#2563EB` accent | neutral greys with Esri-like blue `#0079C1`; the rust brand `#B84A2C` stays only on the logo and the Project tab |
| Shape | radii 5/8/10, pill search | radii 0/2/4, rectangular search, hairline borders |
| Type | 12.5 px, semibold heading 17 | 12 px body, 14 px pane titles, 16 px headings; weights: regular and semibold only |
| Density | 22 px rows, 6x5 spacing | 24 px rows, 4/8/12 spacing scale, 8 px pane padding |
| Icons | two-tone code-drawn set | stay code-drawn (licence safe, no Esri assets), but change language: 1.5 px strokes, mono ink plus one blue and one warm accent, 16 and 32 px variants, our own metaphors; never copy Esri glyph shapes |
| Ribbon | ten tabs, folded contextuals | Pro's tabs, real contextual tab sets with a coloured header band |
| Menus | RustGIS command grouping | Pro menu contents and order; Command Search keeps Ctrl+K and also Alt+Q |
| Branding | logo + rust Project tab | keep; only the logo and the Project tab carry the brand colour |

Never use Esri icon artwork, fonts (Segoe UI is not redistributable; use the system sans) or the
Esri name in the UI.

## 4. Design tokens

Implemented (not yet wired in) in `app/crates/ui-egui/src/theme_pro.rs`; same field names as
`theme::Tokens` so switching is one line. Pro values are approximations (section header).

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `header` / `tab_strip` | `#F1F1F1` | `#262626` | title bar, tab strip |
| `ribbon` | `#FAFAFA` | `#2F2F2F` | ribbon body |
| `canvas` | `#D9D9D9` | `#1B1B1B` | behind map and views |
| `pane` | `#FFFFFF` | `#303030` | docked panes |
| `pane_header` | `#F1F1F1` | `#383838` | pane title strips |
| `border` | `#D5D5D5` | `#454545` | hairlines |
| `border_strong` | `#B5B5B5` | `#5E5E5E` | input borders, focused panes |
| `text` | `#2B2B2B` | `#EDEDED` | body |
| `text_dim` | `#595959` | `#B3B3B3` | secondary |
| `text_disabled` | `#A6A6A6` | `#707070` | disabled |
| `accent` | `#0079C1` | `#3AA0E0` | selected tab underline, primary button, focus |
| `accent_text` | `#00659F` | `#6DBDEE` | links |
| `hover` / `pressed` / `checked` | `#E5F1F9` / `#C7E0F4` / `#D6E8F5` | `#3D4A55` / `#2A4A63` / `#2B4C66` | control states |
| `selection` | `#C7E0F4` | `#2A4A63` | selected rows |
| `status_bar` | `#F1F1F1` | `#262626` | bottom bar |
| `green` / `red` / `amber` | `#2E8B3D` / `#C62828` / `#D98E04` | `#4CC25D` / `#F06B6B` / `#F2B848` | success, error, warning |
| `context` | `#0E9384` | `#2CC2AE` | contextual tab band |
| brand | `#B84A2C` | same | logo, Project tab only |

| Scale | Values |
| --- | --- |
| Spacing | 2, 4, 8, 12, 16 (`space::XS..XL`) |
| Radius | 0 panes, 2 controls, 4 popups (`radius::*`) |
| Font size | 10.5 small, 12 body, 14 pane title, 16 heading |
| Row height | 24 (lists, tables, inputs); ribbon 96, title bar 32, status bar 24 |
| Stroke | 1 hairline; 2 selected-tab underline and focus ring |

Contrast is tested (`theme_pro` tests): body text at least 6.5:1 and secondary text 4.5:1 on all
surfaces, accent text and text-on-accent at least 4.5:1, both themes.

## 5. Gap list (ranked by impact / effort)

| # | Change | Impact | Effort |
| --- | --- | --- | --- |
| 1 | Switch to the Pro palette, radii, type scale (`theme_pro`) | high | S |
| 2 | Status bar: cursor coordinates with unit dropdown, notifications bell, progress | high | S |
| 3 | Rename panes and tabs to Pro's (Contents, Catalog, Geoprocessing; Map/Insert/Analysis/...) | high | M |
| 4 | Geoprocessing pane chrome: Parameters/Environments tabs, required asterisk, validation badges, split Run, footer message linking History | high | M |
| 5 | Selected-tab underline and group dialog-launcher arrows on the ribbon | medium | S |
| 6 | Contextual tab sets with coloured header band driven by the selected layer | high | M |
| 7 | Symbology pane restructure (Primary symbology, Vary by attribute, Appearance) | high | L |
| 8 | Docked attribute table at the bottom with selection toolbar and footer | medium | M |
| 9 | Pane auto-hide and pin, tabbed pane groups, drag between docks | medium | L |
| 10 | New icon pass (1.5 px stroke, 16/32 variants) | medium | L |
| 11 | Pro-style Contents toolbar (List By Drawing Order / Data Source / Selection / Editing) | medium | M |
| 12 | Project backstage layout (Info, Open, Save, Share, Settings, Licensing, About) | low | M |
| 13 | "Pro" legacy theme variant | low | S |

## 6. First slices

1. **Theme swap** (S). Done. Wire `theme_pro::{light,dark}` behind a setting (`Settings > Appearance >
   Theme: UniGIS / Classic`), apply `space`/`radius`/`type_size` in `theme::apply`, regenerate
   `docs/screenshots` and compare. Tests: the contrast tests plus a screenshot script run. Risk: places
   with hard-coded 5/8/10 radii in `widgets.rs` and `ribbon.rs`; grep and route through `radius::*`.
2. **Status bar** (S). Done: coordinate format dropdown (map units, decimal degrees, DMS), notifications bell with unread count over the notices queue, spinner and tool name while a background tool or install runs. Add cursor coordinates (map CRS, unit dropdown), a notifications bell with the
   existing `notices` queue, and a progress ring for running tools. Mostly `chrome.rs::status_bar`
   plus a mouse-position hook from `mapview.rs`.
3. **Ribbon rename and underline** (S-M). Done: tabs are Map, Insert, Analysis, View, Edit, Imagery, Style, Share, Plugins, Settings, Help (old names Home/Data/Analyze/Publish still resolve through `current_tab`); full-width 2 px accent underline; panes titled Contents / Catalog / Geoprocessing. Dialog-launcher arrows remain open. Replace `TABS` with Pro's tab set and split the "Home",
   "Data", "Style" and "Analyze" groups back to Map, Insert, Analysis, Edit, Imagery (groups already
   exist; the alias code makes it backward compatible); add the 2 px accent underline and the
   dialog-launcher arrow in `ribbon.rs`; rename panes in `panes.rs`.
4. **Geoprocessing pane chrome** (M). In the Tools pane form: Parameters/Environments tabs, red
   asterisk and validation badge per parameter (the engine's tool metadata already marks required
   parameters), split Run button, footer message that opens History. Verify against Pro's tool pane
   through the `arcgis` bridge when possible.

Once an ArcGIS Pro window can be captured (Wine, GE-Proton), take reference screenshots for the
ribbon, Contents, Geoprocessing and Symbology panes, store them under `docs/screenshots/arcgis-ref/`
(for private comparison, not for redistribution), and replace the approximated Pro values here with
sampled ones.
