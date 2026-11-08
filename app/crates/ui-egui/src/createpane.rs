//! The Create Features pane: feature templates per editable layer, a search box, and the
//! construction tools for the chosen template's geometry type.
//!
//! A layer's templates are its saved feature templates; a layer without any gets automatic ones:
//! one per class of a unique-values symbology (the class value becomes the field's default) or a
//! single template named after the layer. Choosing an automatic template saves it with the layer.

use egui::{CornerRadius, Sense, Stroke, Ui, pos2, vec2};
use rustgis_engine::Tool;
use rustgis_engine::data::GeometryType;
use rustgis_engine::map::{Layer, Renderer, Symbol};
use serde_json::{Value, json};

use crate::RustGisApp;
use crate::theme::{Tokens, regular, semibold};

/// Most automatic templates one layer lists (a unique-values renderer can have thousands of classes).
const MAX_AUTO: usize = 200;

/// One row of the pane.
struct Row {
    name: String,
    /// Field defaults (for an automatic template, to be saved when it is first chosen).
    defaults: Vec<(String, Value)>,
    saved: bool,
    symbol: Symbol,
}

/// The templates of a layer: saved ones, else automatic ones.
fn rows(l: &Layer) -> Vec<Row> {
    let base = l.renderer.base_symbol();
    if !l.design.templates.is_empty() {
        return l
            .design
            .templates
            .iter()
            .map(|t| {
                let symbol = match &l.renderer {
                    Renderer::Unique { field, classes, other, .. } => t
                        .defaults
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(field))
                        .and_then(|(_, v)| classes.iter().find(|c| c.0 == v.to_string()))
                        .map_or(*other, |c| c.1),
                    _ => base,
                };
                Row {
                    name: t.name.clone(),
                    defaults: t.defaults.iter().map(|(k, v)| (k.clone(), v.to_json())).collect(),
                    saved: true,
                    symbol,
                }
            })
            .collect();
    }
    match &l.renderer {
        Renderer::Unique { field, classes, .. } if !classes.is_empty() => classes
            .iter()
            .take(MAX_AUTO)
            .map(|(value, symbol)| Row {
                name: value.clone(),
                defaults: vec![(field.clone(), json!(value))],
                saved: false,
                symbol: *symbol,
            })
            .collect(),
        _ => vec![Row { name: l.name.clone(), defaults: Vec::new(), saved: false, symbol: base }],
    }
}

/// The construction tools of a geometry type: (tool, label).
fn construction_tools(g: GeometryType) -> Vec<(Tool, &'static str)> {
    match g {
        GeometryType::Point => vec![(Tool::CreatePoint, "Point")],
        GeometryType::Polyline => vec![(Tool::CreateLine, "Line")],
        GeometryType::Polygon => vec![
            (Tool::CreatePolygon, "Polygon"),
            (Tool::CreateShape("rectangle"), "Rectangle"),
            (Tool::CreateShape("circle"), "Circle"),
            (Tool::CreateShape("ellipse"), "Ellipse"),
        ],
        _ => Vec::new(),
    }
}

/// Chooses a template: the layer becomes the target, the template is saved if it was automatic,
/// and the construction tool in use (or the geometry's first) starts.
fn choose(app: &mut RustGisApp, layer: u32, geometry: GeometryType, row: &Row) {
    if app.run("layer.select", json!({"layer": layer})).is_err() {
        return;
    }
    if !row.saved {
        let defaults: serde_json::Map<String, Value> = row.defaults.iter().cloned().collect();
        let _ = app.run("edit.template", json!({"layer": layer, "name": row.name, "defaults": defaults}));
    }
    app.forms.template = row.name.clone();
    let tools = construction_tools(geometry);
    let tool = if tools.iter().any(|t| t.0 == app.session.tool) { Some(app.session.tool) } else { tools.first().map(|t| t.0) };
    if let Some(tool) = tool {
        app.map.cancel_sketch();
        let _ = app.run("tool.set", json!({"tool": tool.code()}));
    }
}

pub fn show(app: &mut RustGisApp, ui: &mut Ui, t: &Tokens) {
    let search_id = ui.id().with("create-search");
    let mut search = ui.ctx().data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
    ui.add(egui::TextEdit::singleline(&mut search).hint_text(tl!("Search templates")).desired_width(f32::INFINITY));
    ui.ctx().data_mut(|d| d.insert_temp(search_id, search.clone()));
    let query = search.trim().to_lowercase();
    ui.add_space(4.0);

    let layers: Vec<(u32, String, GeometryType, Vec<Row>)> = app
        .session
        .map()
        .layers
        .iter()
        .filter(|l| !l.is_raster() && !l.graphics && !construction_tools(l.geometry_type()).is_empty())
        .map(|l| (l.id, l.name.clone(), l.geometry_type(), rows(l)))
        .collect();
    if layers.is_empty() {
        ui.label(egui::RichText::new(tl!("No editable layers. Add a point, line or polygon layer to the map.")).weak());
        return;
    }
    let active = app.session.active_layer;
    let mut picked = None;
    for (id, name, geometry, rows) in &layers {
        let layer_match = name.to_lowercase().contains(&query);
        let shown: Vec<&Row> = rows.iter().filter(|r| layer_match || r.name.to_lowercase().contains(&query)).collect();
        if shown.is_empty() {
            continue;
        }
        ui.label(egui::RichText::new(name).font(semibold(12.5)));
        for row in shown {
            let selected = active == Some(*id) && app.forms.template == row.name && app.session.tool.creates();
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, row.name.as_str()));
            if selected || resp.hovered() || resp.has_focus() {
                ui.painter().rect(
                    r,
                    CornerRadius::same(4),
                    if selected { t.checked } else { t.hover },
                    if selected { Stroke::new(1.0, t.accent) } else { Stroke::NONE },
                    egui::StrokeKind::Inside,
                );
            }
            crate::panes::patch(ui.painter(), egui::Rect::from_min_size(r.min + vec2(6.0, 4.0), vec2(30.0, 20.0)), &row.symbol, *geometry);
            let text = ui.painter().layout_no_wrap(row.name.clone(), regular(12.0), t.text);
            ui.painter().galley(pos2(r.min.x + 44.0, r.center().y - text.size().y / 2.0), text, t.text);
            if resp.on_hover_text(tl!("Create features with this template")).clicked() {
                picked = Some((*id, *geometry, row));
            }
        }
        ui.add_space(4.0);
    }
    if let Some((id, geometry, row)) = picked {
        choose(app, id, geometry, row);
    }

    // The construction tools of the layer being edited.
    let target = app.session.active_layer.and_then(|id| app.session.map().layer(id)).map(|l| l.geometry_type());
    if let Some(geometry) = target {
        let tools = construction_tools(geometry);
        if !tools.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new(tl!("Construction Tools")).font(semibold(12.5)));
            ui.horizontal_wrapped(|ui| {
                for (tool, label) in tools {
                    if ui.selectable_label(app.session.tool == tool, tl!(label)).clicked() {
                        app.map.cancel_sketch();
                        let _ = app.run("tool.set", json!({"tool": tool.code()}));
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_templates_follow_the_symbology_classes() {
        let app = RustGisApp::with_sample();
        let map = app.session.map();
        for l in map.layers.iter().filter(|l| !l.is_raster() && !l.graphics) {
            let r = rows(l);
            assert!(!r.is_empty(), "{}", l.name);
            if let Renderer::Unique { classes, .. } = &l.renderer {
                assert_eq!(r.len(), classes.len().clamp(1, MAX_AUTO));
            }
        }
        assert!(construction_tools(GeometryType::Polygon).len() > construction_tools(GeometryType::Point).len());
        assert!(construction_tools(GeometryType::None).is_empty());
    }

    #[test]
    fn choosing_a_template_targets_its_layer_and_starts_a_tool() {
        let mut app = RustGisApp::with_sample();
        let (id, geometry, name) = {
            let l = app
                .session
                .map()
                .layers
                .iter()
                .find(|l| !l.is_raster() && !l.graphics && l.geometry_type() == GeometryType::Polygon)
                .unwrap();
            (l.id, l.geometry_type(), l.name.clone())
        };
        let row = rows(app.session.map().layer(id).unwrap()).into_iter().next().unwrap();
        choose(&mut app, id, geometry, &row);
        assert_eq!(app.session.active_layer, Some(id));
        assert_eq!(app.forms.template, row.name);
        assert_eq!(app.session.tool, Tool::CreatePolygon, "{name}");
        // An automatic template is saved with the layer when first chosen, so new features use it.
        assert!(app.session.map().layer(id).unwrap().design.templates.iter().any(|t| t.name == row.name));
        // The tool in use stays when the next template has it too.
        app.run("tool.set", json!({"tool": "createCircle"})).unwrap();
        choose(&mut app, id, geometry, &row);
        assert_eq!(app.session.tool, Tool::CreateShape("circle"));
    }

    #[test]
    fn dragged_shapes_make_features_with_the_template() {
        let mut app = RustGisApp::with_sample();
        let id = app
            .session
            .map()
            .layers
            .iter()
            .find(|l| !l.is_raster() && !l.graphics && l.geometry_type() == GeometryType::Polygon)
            .unwrap()
            .id;
        let row = rows(app.session.map().layer(id).unwrap()).into_iter().next().unwrap();
        choose(&mut app, id, GeometryType::Polygon, &row);
        let before = app.session.map().layer(id).unwrap().dataset.features.len();
        app.run("edit.addFeature", json!({"shape": "rectangle", "points": [[0, 45], [10, 55]], "template": row.name})).unwrap();
        assert_eq!(app.session.map().layer(id).unwrap().dataset.features.len(), before + 1);
    }
}
