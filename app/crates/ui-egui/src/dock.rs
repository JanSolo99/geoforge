//! The dock: where panes live. Every pane sits in a tab group at the left, the right or the
//! bottom of the map. A tab is dragged by its title to another side or into another group, a
//! group can be auto-hidden to a strip of tabs at its edge, and the arrangement is remembered.
//!
//! The model ([`Dock`]) holds no egui state, so it is tested on its own; [`show`] paints it.

use std::collections::HashMap;

use egui::{Align2, Color32, CornerRadius, Id, Order, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::theme::{Tokens, semibold};
use crate::{RustGisApp, icons};

/// Everything that can be docked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Contents,
    Catalog,
    Sources,
    Geoprocessing,
    Symbology,
    History,
    Details,
    Editing,
    Design,
    Workflows,
    Script,
    Tasks,
    Jobs,
    Profile,
    Diagnostics,
    Infographics,
    Report,
    Presentation,
    Notebook,
    Assistant,
    Shares,
    Sql,
}

impl Pane {
    pub const ALL: [Pane; 22] = [
        Pane::Contents,
        Pane::Catalog,
        Pane::Sources,
        Pane::Geoprocessing,
        Pane::Symbology,
        Pane::History,
        Pane::Details,
        Pane::Editing,
        Pane::Design,
        Pane::Workflows,
        Pane::Script,
        Pane::Tasks,
        Pane::Jobs,
        Pane::Profile,
        Pane::Diagnostics,
        Pane::Infographics,
        Pane::Report,
        Pane::Presentation,
        Pane::Notebook,
        Pane::Assistant,
        Pane::Shares,
        Pane::Sql,
    ];

    /// The stable name: the last part of the `ui.pane.*` command and the saved layout's key.
    pub fn key(self) -> &'static str {
        match self {
            Pane::Contents => "contents",
            Pane::Catalog => "catalog",
            Pane::Sources => "sources",
            Pane::Geoprocessing => "geoprocessing",
            Pane::Symbology => "symbology",
            Pane::History => "history",
            Pane::Details => "details",
            Pane::Editing => "editing",
            Pane::Design => "design",
            Pane::Workflows => "workflows",
            Pane::Script => "script",
            Pane::Tasks => "tasks",
            Pane::Jobs => "jobs",
            Pane::Profile => "profile",
            Pane::Diagnostics => "diagnostics",
            Pane::Infographics => "infographics",
            Pane::Report => "report",
            Pane::Presentation => "presentation",
            Pane::Notebook => "notebook",
            Pane::Assistant => "assistant",
            Pane::Shares => "shares",
            Pane::Sql => "sql",
        }
    }

    pub fn from_key(k: &str) -> Option<Pane> {
        Pane::ALL.into_iter().find(|p| p.key() == k)
    }

    /// The command that toggles this pane.
    pub fn command(self) -> String {
        format!("ui.pane.{}", self.key())
    }

    pub fn title(self) -> &'static str {
        match self {
            Pane::Contents => "Contents",
            Pane::Catalog => "Catalog",
            Pane::Sources => "Data Sources",
            Pane::Geoprocessing => "Geoprocessing",
            Pane::Symbology => "Symbology",
            Pane::History => "History",
            Pane::Details => "Details",
            Pane::Editing => "Editing",
            Pane::Design => "Data Design",
            Pane::Workflows => "Workflows",
            Pane::Script => "Script",
            Pane::Tasks => "Tasks",
            Pane::Jobs => "Workflow Manager",
            Pane::Profile => "Profile",
            Pane::Diagnostics => "Diagnostic Monitor",
            Pane::Infographics => "Infographics",
            Pane::Report => "Report",
            Pane::Presentation => "Presentation",
            Pane::Notebook => "Notebook",
            Pane::Assistant => "AI Assistant",
            Pane::Shares => "Shared Items",
            Pane::Sql => "SQL",
        }
    }

    /// Where the pane opens until it has been moved.
    pub fn default_site(self) -> Site {
        match self {
            Pane::Contents | Pane::Catalog | Pane::Sources => Site::Left,
            _ => Site::Right,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Site {
    Left,
    Right,
    Bottom,
}

impl Site {
    pub const ALL: [Site; 3] = [Site::Left, Site::Right, Site::Bottom];

    pub fn key(self) -> &'static str {
        match self {
            Site::Left => "left",
            Site::Right => "right",
            Site::Bottom => "bottom",
        }
    }

    pub fn from_key(k: &str) -> Option<Site> {
        Site::ALL.into_iter().find(|s| s.key() == k)
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The tabs docked at one side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    pub tabs: Vec<Pane>,
    pub active: usize,
    /// Collapsed to a strip of tabs at the edge; a click on a tab slides the pane over the map.
    pub auto_hide: bool,
}

impl Group {
    pub fn active_pane(&self) -> Option<Pane> {
        self.tabs.get(self.active).copied()
    }
}

/// The arrangement alone: what a saved pane set and the preferences keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    groups: [Group; 3],
    /// The side a closed pane returns to.
    home: HashMap<Pane, Site>,
}

impl Default for Layout {
    fn default() -> Self {
        let mut l = Layout { groups: Default::default(), home: HashMap::new() };
        l.open(Pane::Contents);
        l
    }
}

impl Layout {
    /// A layout with nothing open.
    pub fn empty() -> Layout {
        Layout { groups: Default::default(), home: HashMap::new() }
    }

    pub fn group(&self, site: Site) -> &Group {
        &self.groups[site.index()]
    }

    pub fn site_of(&self, pane: Pane) -> Option<Site> {
        Site::ALL.into_iter().find(|s| self.group(*s).tabs.contains(&pane))
    }

    /// The pane showing (the selected tab of its group).
    pub fn is_open(&self, pane: Pane) -> bool {
        self.site_of(pane).is_some_and(|s| self.group(s).active_pane() == Some(pane))
    }

    pub fn is_docked(&self, pane: Pane) -> bool {
        self.site_of(pane).is_some()
    }

    /// Docks the pane where it was last (or where it belongs) and makes its tab the selected one.
    pub fn open(&mut self, pane: Pane) {
        if self.is_docked(pane) {
            self.activate(pane);
            return;
        }
        let site = self.site_of(pane).or_else(|| self.home.get(&pane).copied()).unwrap_or_else(|| pane.default_site());
        self.dock(pane, site, None);
    }

    /// Opens the pane, or closes it when it already shows.
    pub fn toggle(&mut self, pane: Pane) {
        if self.is_open(pane) {
            self.close(pane);
        } else {
            self.open(pane);
        }
    }

    pub fn close(&mut self, pane: Pane) {
        if let Some(site) = self.site_of(pane) {
            self.home.insert(pane, site);
            let g = &mut self.groups[site.index()];
            let i = g.tabs.iter().position(|p| *p == pane).unwrap_or(0);
            g.tabs.retain(|p| *p != pane);
            if g.active > i || g.active >= g.tabs.len() {
                g.active = g.active.saturating_sub(1).min(g.tabs.len().saturating_sub(1));
            }
            if g.tabs.is_empty() {
                g.auto_hide = false;
            }
        }
    }

    /// Closes every pane at a side.
    pub fn close_site(&mut self, site: Site) {
        for p in self.group(site).tabs.clone() {
            self.close(p);
        }
    }

    /// Moves the pane to a side as a tab at `index` (the end without one) and selects it.
    pub fn dock(&mut self, pane: Pane, site: Site, index: Option<usize>) {
        let mut index = index;
        if let Some(from) = self.site_of(pane) {
            // A tab moved right within its own group lands after the ones it passes.
            if from == site
                && let (Some(i), Some(old)) = (index, self.group(site).tabs.iter().position(|p| *p == pane))
                && old < i
            {
                index = Some(i - 1);
            }
            let g = &mut self.groups[from.index()];
            let was = g.tabs.iter().position(|p| *p == pane).unwrap_or(0);
            g.tabs.remove(was);
            if g.active > was || g.active >= g.tabs.len() {
                g.active = g.active.saturating_sub(1).min(g.tabs.len().saturating_sub(1));
            }
            // A group emptied by moving its last tab elsewhere is no longer auto-hidden.
            if g.tabs.is_empty() && from != site {
                g.auto_hide = false;
            }
        }
        let g = &mut self.groups[site.index()];
        let at = index.unwrap_or(g.tabs.len()).min(g.tabs.len());
        g.tabs.insert(at, pane);
        g.active = at;
        self.home.insert(pane, site);
    }

    /// Selects a docked pane's tab.
    pub fn activate(&mut self, pane: Pane) {
        if let Some(s) = self.site_of(pane) {
            let g = &mut self.groups[s.index()];
            g.active = g.tabs.iter().position(|p| *p == pane).unwrap_or(0);
        }
    }

    pub fn set_auto_hide(&mut self, site: Site, on: bool) {
        let g = &mut self.groups[site.index()];
        g.auto_hide = on && !g.tabs.is_empty();
    }

    /// The saved form: `{"left": {"tabs": ["contents"], "active": 0, "hide": false}, …, "home": {…}}`.
    pub fn to_json(&self) -> Value {
        let mut v = serde_json::Map::new();
        for s in Site::ALL {
            let g = self.group(s);
            v.insert(
                s.key().into(),
                json!({"tabs": g.tabs.iter().map(|p| p.key()).collect::<Vec<_>>(), "active": g.active, "hide": g.auto_hide}),
            );
        }
        let mut home: Vec<(&str, &str)> = self.home.iter().map(|(p, s)| (p.key(), s.key())).collect();
        home.sort();
        v.insert("home".into(), Value::Object(home.into_iter().map(|(p, s)| (p.to_string(), json!(s))).collect()));
        Value::Object(v)
    }

    /// Reads a saved layout; panes this version doesn't know are skipped, and a pane saved twice
    /// stays in its first place.
    pub fn from_json(v: &Value) -> Option<Layout> {
        let obj = v.as_object()?;
        let mut l = Layout::empty();
        for s in Site::ALL {
            let Some(g) = obj.get(s.key()) else { continue };
            for k in g.get("tabs").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                if let Some(p) = Pane::from_key(k)
                    && !l.is_docked(p)
                {
                    l.groups[s.index()].tabs.push(p);
                }
            }
            let grp = &mut l.groups[s.index()];
            let active = g.get("active").and_then(Value::as_u64).unwrap_or(0) as usize;
            grp.active = active.min(grp.tabs.len().saturating_sub(1));
            grp.auto_hide = !grp.tabs.is_empty() && g.get("hide").and_then(Value::as_bool).unwrap_or(false);
        }
        for (p, s) in obj.get("home").and_then(Value::as_object).into_iter().flatten() {
            if let (Some(p), Some(s)) = (Pane::from_key(p), s.as_str().and_then(Site::from_key)) {
                l.home.insert(p, s);
            }
        }
        Some(l)
    }
}

/// The layout plus what a drag in progress and an auto-hidden pane sliding out need.
#[derive(Default)]
pub struct Dock {
    pub layout: Layout,
    /// The layout last handed to the preferences.
    saved: String,
    /// A tab being dragged by its title.
    drag: Option<Pane>,
    /// The auto-hidden side whose pane is showing over the map.
    revealed: Option<Site>,
    /// Where the sides were drawn this frame (for the drop targets).
    rects: [Option<Rect>; 3],
    /// The tab strip of each side, and its tabs.
    headers: [Option<Rect>; 3],
    tab_rects: [Vec<(Pane, Rect)>; 3],
    /// What is left for the map.
    center: Option<Rect>,
}

impl std::ops::Deref for Dock {
    type Target = Layout;
    fn deref(&self) -> &Layout {
        &self.layout
    }
}

impl std::ops::DerefMut for Dock {
    fn deref_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

/// Where a dragged tab would land.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub site: Site,
    pub index: Option<usize>,
}

impl Dock {
    pub fn dragging(&self) -> Option<Pane> {
        self.drag
    }

    /// Opening an auto-hidden side's pane shows it over the map.
    pub fn open(&mut self, pane: Pane) {
        self.layout.open(pane);
        self.reveal_if_hidden(pane);
    }

    pub fn toggle(&mut self, pane: Pane) {
        if self.layout.is_open(pane) && self.revealed != self.layout.site_of(pane) && self.hidden(pane) {
            self.open(pane);
        } else if self.layout.is_open(pane) {
            self.layout.close(pane);
        } else {
            self.open(pane);
        }
    }

    fn hidden(&self, pane: Pane) -> bool {
        self.layout.site_of(pane).is_some_and(|s| self.layout.group(s).auto_hide)
    }

    fn reveal_if_hidden(&mut self, pane: Pane) {
        self.revealed = if self.hidden(pane) { self.layout.site_of(pane) } else { None };
    }

    /// Replaces the arrangement (a saved pane set, or the default).
    pub fn set_layout(&mut self, l: Layout) {
        self.layout = l;
        self.revealed = None;
    }

    /// Where the pointer would drop a tab, given the rectangles of the last frame.
    pub fn target_at(&self, p: egui::Pos2) -> Option<Target> {
        for s in Site::ALL {
            if self.rects[s.index()].is_some_and(|r| r.contains(p)) {
                let index = self.headers[s.index()].filter(|h| h.contains(p)).map(|_| {
                    let tabs = &self.tab_rects[s.index()];
                    tabs.iter()
                        .find(|(_, r)| p.x < r.center().x)
                        .and_then(|(pane, _)| self.group(s).tabs.iter().position(|p| p == pane))
                        .unwrap_or(self.group(s).tabs.len())
                });
                return Some(Target { site: s, index });
            }
        }
        let c = self.center?;
        if !c.contains(p) {
            return None;
        }
        Some(Target { site: edge_zone(c, p)?, index: None })
    }

    fn preview_rect(&self, t: Target) -> Option<Rect> {
        if let Some(r) = self.rects[t.site.index()] {
            return Some(r);
        }
        let c = self.center?;
        Some(match t.site {
            Site::Left => Rect::from_min_size(c.min, vec2(300.0_f32.min(c.width() * 0.4), c.height())),
            Site::Right => {
                let w = 320.0_f32.min(c.width() * 0.4);
                Rect::from_min_size(pos2(c.max.x - w, c.min.y), vec2(w, c.height()))
            }
            Site::Bottom => {
                let h = 250.0_f32.min(c.height() * 0.4);
                Rect::from_min_size(pos2(c.min.x, c.max.y - h), vec2(c.width(), h))
            }
        })
    }
}

/// The side whose edge band of the central area the point is in.
fn edge_zone(c: Rect, p: egui::Pos2) -> Option<Site> {
    let (fx, fy) = ((p.x - c.min.x) / c.width().max(1.0), (c.max.y - p.y) / c.height().max(1.0));
    if fx < 0.2 {
        Some(Site::Left)
    } else if fx > 0.8 {
        Some(Site::Right)
    } else if fy < 0.25 {
        Some(Site::Bottom)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Preferences

const PREF: &str = "dock_layout";

/// Reads the saved arrangement, if there is one.
pub fn restore(app: &mut RustGisApp) {
    let Some(text) = app.services.load_pref.as_ref().and_then(|l| l(PREF)) else { return };
    if let Some(l) = serde_json::from_str::<Value>(&text).ok().as_ref().and_then(Layout::from_json) {
        app.ui.dock.set_layout(l);
        app.ui.dock.saved = text;
    }
}

/// Keeps the arrangement with the preferences when it changed.
pub fn persist(app: &mut RustGisApp) {
    if app.ui.dock.drag.is_some() {
        return;
    }
    let text = app.ui.dock.layout.to_json().to_string();
    if text != app.ui.dock.saved {
        if let Some(save) = &app.services.save_pref {
            save(PREF, &text);
        }
        app.ui.dock.saved = text;
    }
}

// ---------------------------------------------------------------------------------------------
// Painting

const HEADER_H: f32 = 34.0;
const STRIP: f32 = 26.0;

pub fn show(app: &mut RustGisApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    app.ui.dock.rects = [None; 3];
    app.ui.dock.headers = [None; 3];
    app.ui.dock.tab_rects = Default::default();
    let frame = egui::Frame::NONE.fill(t.pane).inner_margin(egui::Margin::ZERO);
    for site in Site::ALL {
        let g = app.ui.dock.group(site).clone();
        if g.tabs.is_empty() {
            continue;
        }
        if g.auto_hide {
            strip(app, ui, site, &g, &t);
            continue;
        }
        let id = format!("dock_{}", site.key());
        let panel = match site {
            Site::Left => egui::Panel::left(id).default_size(270.0).size_range(200.0..=480.0),
            Site::Right => egui::Panel::right(id).default_size(320.0).size_range(260.0..=560.0),
            Site::Bottom => egui::Panel::bottom(id).default_size(250.0).size_range(120.0..=600.0),
        };
        let r = panel.resizable(true).frame(frame).show(ui, |ui| group_ui(app, ui, site, &t));
        app.ui.dock.rects[site.index()] = Some(r.response.rect);
    }
    // The pane sliding out of an auto-hidden side.
    if let Some(site) = app.ui.dock.revealed {
        if app.ui.dock.group(site).auto_hide && !app.ui.dock.group(site).tabs.is_empty() {
            reveal(app, ui, site, &t);
        } else {
            app.ui.dock.revealed = None;
        }
    }
    app.ui.dock.center = Some(ui.available_rect_before_wrap());
}

/// The tab strip and the selected pane of a group.
fn group_ui(app: &mut RustGisApp, ui: &mut Ui, site: Site, t: &Tokens) {
    let g = app.ui.dock.group(site).clone();
    let Some(pane) = g.active_pane() else { return };
    header(app, ui, site, &g, t);
    crate::panes::body(app, ui, pane, t);
}

/// The title row: one tab per pane (a lone pane shows just its title), then the group's menu and
/// close button. Dragging a tab starts a move; right-click opens the menu.
fn header(app: &mut RustGisApp, ui: &mut Ui, site: Site, g: &Group, t: &Tokens) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::click());
    ui.painter().rect_filled(r, 0.0, t.pane_header);
    ui.painter().line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, t.border));
    let active = g.active_pane();
    app.ui.dock.headers[site.index()] = Some(r);
    let tabs_rect = Rect::from_min_max(pos2(r.min.x + 4.0, r.min.y), pos2((r.max.x - 54.0).max(r.min.x + 4.0), r.max.y));
    let mut tabs_ui = ui.new_child(egui::UiBuilder::new().max_rect(tabs_rect));
    tabs_ui.set_clip_rect(tabs_rect.intersect(ui.clip_rect()));
    let mut rects = Vec::new();
    let mut clicked = None;
    let mut started = None;
    egui::ScrollArea::horizontal()
        .id_salt(("dock_tabs", site.key()))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(&mut tabs_ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for p in &g.tabs {
                    let label = crate::i18n::t(p.title());
                    let font = semibold(13.0);
                    let galley = ui.painter().layout_no_wrap(label.to_string(), font, if Some(*p) == active { t.text } else { t.text_dim });
                    let (tr, tresp) = ui.allocate_exact_size(vec2(galley.size().x + 20.0, HEADER_H), Sense::click_and_drag());
                    if tresp.hovered() && Some(*p) != active {
                        ui.painter().rect_filled(tr.shrink2(vec2(1.0, 4.0)), CornerRadius::same(4), t.hover);
                    }
                    ui.painter().galley(pos2(tr.min.x + 10.0, r.center().y - galley.size().y / 2.0), galley, t.text);
                    tresp.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), Some(*p) == active, label)
                    });
                    if tresp.clicked() {
                        clicked = Some(*p);
                    }
                    if tresp.drag_started() {
                        started = Some(*p);
                    }
                    tresp.on_hover_cursor(egui::CursorIcon::Grab);
                    if tr.intersects(tabs_rect) {
                        rects.push((*p, tr.intersect(tabs_rect)));
                    }
                }
            });
        });
    // Paint outside the scroll area's content clip, which can trim the header's bottom edge.
    if let Some((_, tr)) = rects.iter().find(|(pane, _)| Some(*pane) == active) {
        ui.painter().line_segment([pos2(tr.min.x + 6.0, r.max.y - 1.5), pos2(tr.max.x - 6.0, r.max.y - 1.5)], Stroke::new(2.0, t.accent));
    }
    app.ui.dock.tab_rects[site.index()] = rects;
    if let Some(p) = clicked {
        app.ui.dock.layout.activate(p);
    }
    if let Some(p) = started {
        app.ui.dock.layout.activate(p);
        app.ui.dock.drag = Some(p);
    }
    // Close.
    let cr = Rect::from_center_size(pos2(r.max.x - 18.0, r.center().y), vec2(22.0, 22.0));
    let close = ui.interact(cr, Id::new(("dock_close", site.key())), Sense::click());
    if close.hovered() {
        ui.painter().rect_filled(cr, CornerRadius::same(4), t.hover);
    }
    icons::paint(ui.painter(), cr.shrink(5.0), "close", t.text_dim, t.accent);
    // Menu.
    let mr = Rect::from_center_size(pos2(r.max.x - 40.0, r.center().y), vec2(22.0, 22.0));
    let menu = ui.interact(mr, Id::new(("dock_menu", site.key())), Sense::click());
    if menu.hovered() {
        ui.painter().rect_filled(mr, CornerRadius::same(4), t.hover);
    }
    for dy in [-4.0, 0.0, 4.0] {
        ui.painter().circle_filled(mr.center() + vec2(0.0, dy), 1.4, t.text_dim);
    }
    let Some(pane) = active else { return };
    if close.on_hover_text(tl!("Close")).clicked() {
        let _ = app.run(&pane.command(), Value::Null);
    }
    let mut act: Option<MenuAction> = None;
    egui::containers::Popup::menu(&menu).show(|ui| menu_items(ui, site, g, &mut act, true));
    // Keep drawing an open context menu after the frame that received the click.
    egui::Popup::context_menu(&resp).show(|ui| menu_items(ui, site, g, &mut act, false));
    match act {
        Some(MenuAction::Move(s)) => app.ui.dock.layout.dock(pane, s, None),
        Some(MenuAction::AutoHide(on)) => {
            app.ui.dock.layout.set_auto_hide(site, on);
            app.ui.dock.revealed = None;
        }
        Some(MenuAction::Reset) => app.ui.dock.set_layout(Layout::default()),
        Some(MenuAction::Close) => {
            let _ = app.run(&pane.command(), Value::Null);
        }
        None => {}
    }
}

fn menu_items(ui: &mut Ui, site: Site, g: &Group, act: &mut Option<MenuAction>, reset: bool) {
    ui.set_min_width(150.0);
    for s in Site::ALL.into_iter().filter(|s| *s != site) {
        let label = match s {
            Site::Left => tl!("Dock Left"),
            Site::Right => tl!("Dock Right"),
            Site::Bottom => tl!("Dock Bottom"),
        };
        if ui.button(label).clicked() {
            *act = Some(MenuAction::Move(s));
        }
    }
    if ui.button(if g.auto_hide { tl!("Keep Open") } else { tl!("Auto Hide") }).clicked() {
        *act = Some(MenuAction::AutoHide(!g.auto_hide));
    }
    if reset && ui.button(tl!("Reset Layout")).clicked() {
        *act = Some(MenuAction::Reset);
    }
    ui.separator();
    if ui.button(tl!("Close")).clicked() {
        *act = Some(MenuAction::Close);
    }
}

enum MenuAction {
    Move(Site),
    AutoHide(bool),
    Reset,
    Close,
}

/// The strip of an auto-hidden group: one button per pane, along the edge.
fn strip(app: &mut RustGisApp, ui: &mut Ui, site: Site, g: &Group, t: &Tokens) {
    let id = format!("dock_strip_{}", site.key());
    let panel = match site {
        Site::Left => egui::Panel::left(id).exact_size(STRIP),
        Site::Right => egui::Panel::right(id).exact_size(STRIP),
        Site::Bottom => egui::Panel::bottom(id).exact_size(STRIP),
    };
    let frame = egui::Frame::NONE.fill(t.pane_header);
    let r = panel.resizable(false).frame(frame).show(ui, |ui| {
        let full = ui.max_rect();
        let mut pos = full.min + vec2(0.0, 6.0);
        if site == Site::Bottom {
            pos = full.min + vec2(6.0, 0.0);
        }
        let mut hit: Vec<(Pane, Rect)> = Vec::new();
        for p in &g.tabs {
            let label = crate::i18n::t(p.title()).to_string();
            let galley = ui.painter().layout_no_wrap(label, semibold(12.0), t.text);
            let len = galley.size().x + 18.0;
            let tr = match site {
                Site::Bottom => Rect::from_min_size(pos, vec2(len, STRIP)),
                _ => Rect::from_min_size(pos2(full.min.x, pos.y), vec2(STRIP, len)),
            };
            let resp = ui.interact(tr, Id::new(("dock_strip_tab", p.key())), Sense::click());
            let live = app.ui.dock.revealed == Some(site) && app.ui.dock.group(site).active_pane() == Some(*p);
            if live || resp.hovered() {
                ui.painter().rect_filled(tr.shrink(2.0), CornerRadius::same(4), t.hover);
            }
            match site {
                Site::Bottom => {
                    ui.painter().galley(pos2(tr.min.x + 9.0, tr.center().y - galley.size().y / 2.0), galley, t.text);
                    pos.x += len;
                }
                _ => {
                    // Text reads upwards on the left edge and downwards on the right.
                    let (angle, origin) = if site == Site::Left {
                        (-std::f32::consts::FRAC_PI_2, pos2(tr.center().x - galley.size().y / 2.0, tr.max.y - 9.0))
                    } else {
                        (std::f32::consts::FRAC_PI_2, pos2(tr.center().x + galley.size().y / 2.0, tr.min.y + 9.0))
                    };
                    ui.painter().add(egui::epaint::TextShape::new(origin, galley, t.text).with_angle(angle));
                    pos.y += len;
                }
            }
            if resp.clicked() {
                hit.push((*p, tr));
            }
        }
        for (p, _) in hit {
            let shown = app.ui.dock.revealed == Some(site) && app.ui.dock.group(site).active_pane() == Some(p);
            app.ui.dock.layout.activate(p);
            app.ui.dock.revealed = if shown { None } else { Some(site) };
        }
    });
    app.ui.dock.rects[site.index()] = Some(r.response.rect);
}

/// An auto-hidden pane slid out over the map; a click elsewhere puts it away.
fn reveal(app: &mut RustGisApp, ui: &mut Ui, site: Site, t: &Tokens) {
    let c = ui.available_rect_before_wrap();
    let rect = match site {
        Site::Left => Rect::from_min_size(c.min, vec2(320.0_f32.min(c.width() * 0.6), c.height())),
        Site::Right => {
            let w = 340.0_f32.min(c.width() * 0.6);
            Rect::from_min_size(pos2(c.max.x - w, c.min.y), vec2(w, c.height()))
        }
        Site::Bottom => {
            let h = 260.0_f32.min(c.height() * 0.6);
            Rect::from_min_size(pos2(c.min.x, c.max.y - h), vec2(c.width(), h))
        }
    };
    let ctx = ui.ctx().clone();
    let area = egui::Area::new(Id::new(("dock_reveal", site.key()))).order(Order::Foreground).fixed_pos(rect.min).show(&ctx, |ui| {
        let frame = egui::Frame::NONE.fill(t.pane).stroke(Stroke::new(1.0, t.border_strong));
        frame.show(ui, |ui| {
            ui.set_min_size(rect.size() - vec2(2.0, 2.0));
            ui.set_max_size(rect.size() - vec2(2.0, 2.0));
            group_ui(app, ui, site, t);
        });
    });
    let outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.pointer_interact_pos().is_some_and(|p| !area.response.rect.contains(p));
    let on_strip = ctx.pointer_interact_pos().is_some_and(|p| app.ui.dock.rects[site.index()].is_some_and(|r| r.contains(p)));
    if outside && !on_strip {
        app.ui.dock.revealed = None;
    }
}

/// The drop targets and the preview while a tab is being dragged; the move itself on release.
pub fn drag_overlay(app: &mut RustGisApp, ctx: &egui::Context) {
    let Some(pane) = app.ui.dock.drag else { return };
    let t = Tokens::get(ctx);
    let released = ctx.input(|i| i.pointer.any_released());
    let cancelled = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let pos = ctx.pointer_latest_pos();
    if cancelled || (!released && !ctx.input(|i| i.pointer.any_down())) {
        app.ui.dock.drag = None;
        return;
    }
    let Some(pos) = pos else { return };
    let target = app.ui.dock.target_at(pos);
    let painter = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("dock_drag")));
    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    // The edge bands, so the places a pane can go are visible while dragging.
    if let Some(c) = app.ui.dock.center {
        let faint = t.accent.gamma_multiply(0.10);
        for (s, label) in [(Site::Left, "Left"), (Site::Right, "Right"), (Site::Bottom, "Bottom")] {
            if let Some(r) = app.ui.dock.preview_rect(Target { site: s, index: None }).filter(|_| app.ui.dock.rects[s.index()].is_none()) {
                painter.rect_filled(r, 0.0, faint);
                painter.text(r.center(), Align2::CENTER_CENTER, crate::i18n::t(label), semibold(13.0), t.accent.gamma_multiply(0.7));
            }
        }
        let _ = c;
    }
    if let Some(tg) = target
        && let Some(r) = app.ui.dock.preview_rect(tg)
    {
        painter.rect_filled(r, 0.0, t.accent.gamma_multiply(0.28));
        painter.rect_stroke(r.shrink(1.0), 0.0, Stroke::new(2.0, t.accent), egui::StrokeKind::Inside);
        // An insertion mark where a tab dropped on a strip would go.
        if let Some(i) = tg.index {
            let tabs = &app.ui.dock.tab_rects[tg.site.index()];
            if let Some(h) = app.ui.dock.headers[tg.site.index()] {
                let pane = app.ui.dock.group(tg.site).tabs.get(i);
                let x = pane
                    .and_then(|p| tabs.iter().find(|(tab, _)| tab == p))
                    .map(|(_, r)| r.min.x)
                    .or_else(|| tabs.last().map(|(_, r)| r.max.x))
                    .unwrap_or(h.min.x + 4.0);
                painter.line_segment([pos2(x, h.min.y + 4.0), pos2(x, h.max.y - 4.0)], Stroke::new(2.5, Color32::WHITE));
            }
        }
    }
    // The tab under the pointer.
    let galley = painter.layout_no_wrap(crate::i18n::t(pane.title()).to_string(), semibold(13.0), t.text);
    let gr = Rect::from_min_size(pos + vec2(12.0, 8.0), galley.size() + vec2(20.0, 10.0));
    painter.rect_filled(gr, CornerRadius::same(5), t.pane);
    painter.rect_stroke(gr, CornerRadius::same(5), Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
    painter.galley(gr.min + vec2(10.0, 5.0), galley, t.text);
    if released {
        app.ui.dock.drag = None;
        if let Some(tg) = target {
            app.ui.dock.layout.dock(pane, tg.site, tg.index);
            app.ui.dock.revealed = None;
        }
    }
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_contents_on_the_left() {
        let l = Layout::default();
        assert!(l.is_open(Pane::Contents));
        assert_eq!(l.site_of(Pane::Contents), Some(Site::Left));
        assert!(!l.is_docked(Pane::Catalog));
    }

    #[test]
    fn panes_open_where_they_belong_and_share_a_group() {
        let mut l = Layout::default();
        l.open(Pane::Catalog);
        assert_eq!(l.group(Site::Left).tabs, vec![Pane::Contents, Pane::Catalog]);
        assert!(l.is_open(Pane::Catalog) && !l.is_open(Pane::Contents) && l.is_docked(Pane::Contents));
        l.open(Pane::Geoprocessing);
        l.open(Pane::Symbology);
        assert_eq!(l.group(Site::Right).tabs, vec![Pane::Geoprocessing, Pane::Symbology]);
        assert!(l.is_open(Pane::Symbology));
        l.activate(Pane::Geoprocessing);
        assert!(l.is_open(Pane::Geoprocessing));
    }

    #[test]
    fn toggle_closes_the_showing_pane_and_opens_a_hidden_tab() {
        let mut l = Layout::default();
        l.open(Pane::Catalog);
        l.toggle(Pane::Contents);
        assert!(l.is_open(Pane::Contents));
        l.toggle(Pane::Contents);
        assert!(!l.is_docked(Pane::Contents));
        assert_eq!(l.group(Site::Left).active_pane(), Some(Pane::Catalog));
        l.toggle(Pane::Catalog);
        assert!(l.group(Site::Left).tabs.is_empty());
    }

    #[test]
    fn dragging_a_pane_to_another_side_moves_it_and_it_returns_there() {
        let mut l = Layout::default();
        l.open(Pane::Catalog);
        l.dock(Pane::Catalog, Site::Bottom, None);
        assert_eq!(l.group(Site::Left).tabs, vec![Pane::Contents]);
        assert_eq!(l.group(Site::Left).active_pane(), Some(Pane::Contents));
        assert_eq!(l.group(Site::Bottom).tabs, vec![Pane::Catalog]);
        l.close(Pane::Catalog);
        assert!(l.group(Site::Bottom).tabs.is_empty());
        l.open(Pane::Catalog);
        assert_eq!(l.site_of(Pane::Catalog), Some(Site::Bottom));
    }

    #[test]
    fn tabs_reorder_within_a_group() {
        let mut l = Layout::empty();
        for p in [Pane::Geoprocessing, Pane::History, Pane::Details] {
            l.dock(p, Site::Right, None);
        }
        l.dock(Pane::Geoprocessing, Site::Right, Some(3));
        assert_eq!(l.group(Site::Right).tabs, vec![Pane::History, Pane::Details, Pane::Geoprocessing]);
        l.dock(Pane::Geoprocessing, Site::Right, Some(0));
        assert_eq!(l.group(Site::Right).tabs, vec![Pane::Geoprocessing, Pane::History, Pane::Details]);
        assert_eq!(l.group(Site::Right).active_pane(), Some(Pane::Geoprocessing));
    }

    #[test]
    fn opening_an_existing_tab_preserves_its_position() {
        let mut l = Layout::default();
        l.open(Pane::Catalog);
        l.open(Pane::Sources);
        l.open(Pane::Contents);
        assert_eq!(l.group(Site::Left).tabs, vec![Pane::Contents, Pane::Catalog, Pane::Sources]);
        assert!(l.is_open(Pane::Contents));
    }

    #[test]
    fn closing_keeps_the_selection_valid() {
        let mut l = Layout::empty();
        for p in [Pane::History, Pane::Details, Pane::Script] {
            l.dock(p, Site::Right, None);
        }
        l.close(Pane::Script);
        assert_eq!(l.group(Site::Right).active_pane(), Some(Pane::Details));
        l.close(Pane::History);
        assert_eq!(l.group(Site::Right).active_pane(), Some(Pane::Details));
        l.close_site(Site::Right);
        assert_eq!(l.group(Site::Right).active_pane(), None);
    }

    #[test]
    fn auto_hide_needs_a_pane_and_ends_when_the_group_empties() {
        let mut l = Layout::default();
        l.set_auto_hide(Site::Right, true);
        assert!(!l.group(Site::Right).auto_hide);
        l.set_auto_hide(Site::Left, true);
        assert!(l.group(Site::Left).auto_hide);
        l.close(Pane::Contents);
        assert!(!l.group(Site::Left).auto_hide);
    }

    #[test]
    fn the_layout_round_trips_through_json() {
        let mut l = Layout::default();
        l.open(Pane::Catalog);
        l.dock(Pane::Geoprocessing, Site::Bottom, None);
        l.open(Pane::Symbology);
        l.set_auto_hide(Site::Left, true);
        l.close(Pane::Symbology);
        let back = Layout::from_json(&l.to_json()).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.group(Site::Left).tabs, vec![Pane::Contents, Pane::Catalog]);
        assert!(back.group(Site::Left).auto_hide);
    }

    #[test]
    fn preferences_restore_the_layout_in_a_new_app() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let stored = Rc::new(RefCell::new(HashMap::<String, String>::new()));
        let write = stored.clone();
        let mut app = RustGisApp::with_sample().with_services(crate::Services {
            save_pref: Some(Box::new(move |key, value| {
                write.borrow_mut().insert(key.into(), value.into());
            })),
            ..Default::default()
        });
        app.run("ui.dock", json!({"pane": "catalog", "site": "bottom", "autoHide": true})).unwrap();
        persist(&mut app);
        let read = stored.clone();
        let restored = RustGisApp::with_sample()
            .with_services(crate::Services { load_pref: Some(Box::new(move |key| read.borrow().get(key).cloned())), ..Default::default() });
        assert_eq!(restored.ui.dock.layout, app.ui.dock.layout);
        assert!(restored.ui.dock.group(Site::Bottom).auto_hide);
    }

    #[test]
    fn a_damaged_saved_layout_is_survivable() {
        let v = json!({"left": {"tabs": ["contents", "nope", "contents"], "active": 9}, "right": {"tabs": ["contents", "history"], "hide": true}, "home": {"sql": "bottom", "x": "left"}});
        let l = Layout::from_json(&v).unwrap();
        assert_eq!(l.group(Site::Left).tabs, vec![Pane::Contents]);
        assert_eq!(l.group(Site::Left).active, 0);
        assert_eq!(l.group(Site::Right).tabs, vec![Pane::History]);
        let mut l = l;
        l.open(Pane::Sql);
        assert_eq!(l.site_of(Pane::Sql), Some(Site::Bottom));
        assert!(Layout::from_json(&json!(3)).is_none());
    }

    #[test]
    fn keys_are_unique_and_match_the_commands() {
        for p in Pane::ALL {
            assert_eq!(Pane::from_key(p.key()), Some(p));
            assert_eq!(p.command(), format!("ui.pane.{}", p.key()));
        }
    }

    #[test]
    fn drop_targets_follow_the_pointer() {
        let mut d = Dock::default();
        d.open(Pane::Catalog);
        d.center = Some(Rect::from_min_size(pos2(300.0, 100.0), vec2(600.0, 400.0)));
        d.rects[Site::Left.index()] = Some(Rect::from_min_size(pos2(0.0, 100.0), vec2(300.0, 400.0)));
        d.headers[Site::Left.index()] = Some(Rect::from_min_size(pos2(0.0, 100.0), vec2(300.0, 34.0)));
        d.tab_rects[Site::Left.index()] = vec![
            (Pane::Contents, Rect::from_min_size(pos2(4.0, 100.0), vec2(80.0, 34.0))),
            (Pane::Catalog, Rect::from_min_size(pos2(84.0, 100.0), vec2(80.0, 34.0))),
        ];
        // Over the open pane: into its group; on its tab strip, at a position.
        assert_eq!(d.target_at(pos2(100.0, 300.0)), Some(Target { site: Site::Left, index: None }));
        assert_eq!(d.target_at(pos2(20.0, 110.0)), Some(Target { site: Site::Left, index: Some(0) }));
        assert_eq!(d.target_at(pos2(200.0, 110.0)), Some(Target { site: Site::Left, index: Some(2) }));
        // Over the map: the nearest edge band, none in the middle.
        assert_eq!(d.target_at(pos2(880.0, 300.0)), Some(Target { site: Site::Right, index: None }));
        assert_eq!(d.target_at(pos2(600.0, 480.0)), Some(Target { site: Site::Bottom, index: None }));
        assert_eq!(d.target_at(pos2(600.0, 250.0)), None);
        assert_eq!(d.target_at(pos2(10.0, 10.0)), None);
    }

    #[test]
    fn opening_a_hidden_pane_slides_it_out() {
        let mut d = Dock::default();
        d.layout.set_auto_hide(Site::Left, true);
        d.open(Pane::Contents);
        assert_eq!(d.revealed, Some(Site::Left));
        d.toggle(Pane::Contents);
        assert!(!d.is_docked(Pane::Contents));
    }
}
