//! The built-in Map > Basemap gallery. Service names, addresses and credits are
//! catalog data; previews are our own schematic drawings, never provider assets.

use crate::{
    RustGisApp, icons,
    theme::{Tokens, regular, semibold},
};
use egui::{CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use rustgis_engine::map::{Basemap, tiles::TileLayer};
use serde_json::{Value, json};

struct Preset {
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    url: &'static str,
    attribution: &'static str,
    max_zoom: u8,
    subdomains: &'static [&'static str],
    overlay: bool,
}

impl Preset {
    fn params(&self) -> Value {
        json!({"id": self.id, "name": self.name, "kind": self.kind, "url": self.url,
            "attribution": self.attribution, "maxZoom": self.max_zoom,
            "subdomains": self.subdomains, "overlay": self.overlay, "replace": !self.overlay})
    }
    fn matches(&self, tile: &TileLayer) -> bool {
        tile.url == self.url && tile.kind.code() == self.kind && tile.overlay == self.overlay
    }
    fn preview(&self) -> (Basemap, &'static str) {
        if self.overlay {
            (Basemap::Paper, "line_layer")
        } else if self.id.contains("dark") || self.id.contains("eclipse") || self.id.contains("shadow") || self.id.contains("night") {
            (Basemap::Midnight, "basemap")
        } else if self.id.contains("imagery") || self.id.contains("marble") {
            (Basemap::Ocean, "raster_layer")
        } else if self.id.contains("topo") || self.id.contains("relief") || self.id.contains("terrain") || self.id.contains("hillshade") {
            (Basemap::Atlas, "relief")
        } else {
            (Basemap::Paper, "basemap")
        }
    }
}

pub fn default_tiles() -> Value {
    OSM.params()
}

/// Forget the search text once a choice closes the gallery, so it opens unfiltered next time.
fn close(ui: &mut Ui, search_id: egui::Id) {
    ui.ctx().data_mut(|d| d.insert_temp(search_id, String::new()));
    ui.close();
}

pub fn gallery(ui: &mut Ui, app: &mut RustGisApp) {
    let t = Tokens::get(ui.ctx());
    ui.set_width(536.0_f32.min(ui.ctx().content_rect().width() - 32.0).max(260.0));
    let search_id = ui.id().with("basemap-search");
    let mut search = ui.ctx().data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
    let label = ui.label(tl!("Search basemaps"));
    let frame =
        egui::Frame::new().fill(t.input).stroke(Stroke::new(1.0, t.input_border)).corner_radius(CornerRadius::same(4)).inner_margin(4.0);
    ui.add(egui::TextEdit::singleline(&mut search).frame(frame).desired_width(f32::INFINITY)).labelled_by(label.id);
    ui.ctx().data_mut(|d| d.insert_temp(search_id, search.clone()));
    let query = search.trim().to_lowercase();
    let columns = ((ui.available_width() + 8.0) / 136.0).floor().clamp(2.0, 4.0) as usize;
    let height = (ui.ctx().content_rect().height() - 230.0).clamp(160.0, 560.0);
    egui::ScrollArea::vertical().id_salt("basemap-gallery").max_height(height).show(ui, |ui| {
        ui.label(egui::RichText::new(tl!("Offline")).font(semibold(12.5)));
        egui::Grid::new("offline-basemap-grid").spacing(vec2(8.0, 8.0)).show(ui, |ui| {
            for (i, b) in Basemap::ALL.into_iter().filter(|b| tl!(b.label()).to_lowercase().contains(&query)).enumerate() {
                let selected = app.session.map().basemap == b && app.session.map().tiles.iter().all(|t| t.overlay);
                if thumbnail(ui, tl!(b.label()), b, "", selected).clicked() {
                    let _ = app.run("map.basemap", json!({"name": b.code()}));
                    close(ui, search_id);
                }
                if (i + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
        for overlay in [false, true] {
            ui.separator();
            ui.label(egui::RichText::new(if overlay { tl!("Overlays") } else { tl!("Online basemaps") }).font(semibold(12.5)));
            egui::Grid::new(("web-basemap-grid", overlay)).spacing(vec2(8.0, 8.0)).show(ui, |ui| {
                for (i, preset) in PRESETS.iter().filter(|p| p.overlay == overlay && p.name.to_lowercase().contains(&query)).enumerate() {
                    let selected = app.session.map().tiles.iter().any(|tile| preset.matches(tile) && tile.visible && tile.opacity > 0.0)
                        && (overlay || app.session.map().tiles.iter().filter(|t| !t.overlay).count() == 1);
                    let (palette, icon) = preset.preview();
                    if thumbnail(ui, preset.name, palette, icon, selected).on_hover_text(preset.url).clicked() {
                        choose(app, preset);
                        close(ui, search_id);
                    }
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
        }
        crate::ribbon::tile_layers(ui, app, &t);
    });
}

fn choose(app: &mut RustGisApp, preset: &Preset) {
    // Match by address too: projects made by the old plugin keep working, and a
    // caller-supplied id cannot make us remove a different source.
    let existing = app.session.map().tiles.iter().position(|t| preset.matches(t));
    if preset.overlay
        && let Some(index) = existing
    {
        let _ = app.run("map.removeTiles", json!({"index": index}));
    } else {
        let _ = app.run("map.addTiles", preset.params());
    }
}

fn thumbnail(ui: &mut Ui, label: &str, palette: Basemap, icon: &str, selected: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, response) = ui.allocate_exact_size(vec2(128.0, 92.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label));
    let (water, land, outline, _) = palette.palette();
    let thumb = Rect::from_min_size(r.min + vec2(4.0, 4.0), vec2(120.0, 52.0));
    ui.painter().rect_filled(thumb, CornerRadius::same(4), crate::theme::c32(water));
    let points = [(0.12, 0.3), (0.42, 0.18), (0.6, 0.35), (0.55, 0.7), (0.3, 0.82), (0.1, 0.6)];
    let poly = points.iter().map(|(x, y)| pos2(thumb.min.x + x * thumb.width(), thumb.min.y + y * thumb.height())).collect();
    ui.painter().add(egui::Shape::convex_polygon(poly, crate::theme::c32(land), Stroke::new(1.0, crate::theme::c32(outline))));
    if !icon.is_empty() {
        icons::paint(
            ui.painter(),
            Rect::from_center_size(thumb.center() + vec2(27.0, 0.0), vec2(28.0, 28.0)),
            icon,
            crate::theme::c32(outline),
            t.accent,
        );
    }
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            r,
            CornerRadius::same(6),
            Stroke::new(if selected { 2.0 } else { 1.0 }, t.accent),
            egui::StrokeKind::Inside,
        );
    }
    let text = ui.painter().layout(label.to_owned(), regular(11.5), t.text, 120.0);
    ui.painter().galley(pos2(r.center().x - text.size().x / 2.0, r.min.y + 59.0), text, t.text);
    response
}

const OSM: Preset = Preset {
    id: "osm",
    name: "OpenStreetMap",
    kind: "raster",
    url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
    attribution: "© OpenStreetMap contributors",
    max_zoom: 19,
    subdomains: &[],
    overlay: false,
};

const PRESETS: &[Preset] = &[
    OSM,
    Preset {
        id: "osm-hot",
        name: "OSM Humanitarian",
        kind: "raster",
        url: "https://{s}.tile.openstreetmap.fr/hot/{z}/{x}/{y}.png",
        attribution: "© OpenStreetMap contributors, tiles style by Humanitarian OpenStreetMap Team hosted by OpenStreetMap France",
        max_zoom: 19,
        subdomains: &["a", "b"],
        overlay: false,
    },
    Preset {
        id: "osm-de",
        name: "OSM Germany",
        kind: "raster",
        url: "https://tile.openstreetmap.de/{z}/{x}/{y}.png",
        attribution: "© OpenStreetMap contributors",
        max_zoom: 18,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "osm-fr",
        name: "OSM France",
        kind: "raster",
        url: "https://{s}.tile.openstreetmap.fr/osmfr/{z}/{x}/{y}.png",
        attribution: "© OpenStreetMap France | © OpenStreetMap contributors",
        max_zoom: 20,
        subdomains: &["a", "b", "c"],
        overlay: false,
    },
    Preset {
        id: "cyclosm",
        name: "CyclOSM",
        kind: "raster",
        url: "https://{s}.tile-cyclosm.openstreetmap.fr/cyclosm/{z}/{x}/{y}.png",
        attribution: "CyclOSM | Map data: © OpenStreetMap contributors",
        max_zoom: 20,
        subdomains: &["a", "b", "c"],
        overlay: false,
    },
    Preset {
        id: "opentopomap",
        name: "OpenTopoMap",
        kind: "raster",
        url: "https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png",
        attribution: "Map data: © OpenStreetMap contributors, SRTM | Map style: © OpenTopoMap (CC-BY-SA)",
        max_zoom: 17,
        subdomains: &["a", "b", "c"],
        overlay: false,
    },
    Preset {
        id: "esri-streets",
        name: "Esri World Street Map",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: Esri and its data partners",
        max_zoom: 19,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-topo",
        name: "Esri World Topographic",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: Esri and its data partners",
        max_zoom: 19,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-light-gray",
        name: "Esri Light Gray Canvas",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-dark-gray",
        name: "Esri Dark Gray Canvas",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Base/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-natgeo",
        name: "Esri National Geographic",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/NatGeo_World_Map/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — National Geographic, Esri, DeLorme, NAVTEQ, UNEP-WCMC, USGS, NASA, ESA, METI, NRCAN, GEBCO, NOAA, iPC",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-ocean",
        name: "Esri Ocean",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Ocean/World_Ocean_Base/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Sources: GEBCO, NOAA, CHS, OSU, UNH, CSUMB, National Geographic, DeLorme, NAVTEQ, and Esri",
        max_zoom: 13,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-imagery",
        name: "Esri World Imagery",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: Esri, Maxar, Earthstar Geographics, and the GIS User Community",
        max_zoom: 19,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "usgs-imagery",
        name: "USGS Imagery",
        kind: "raster",
        url: "https://basemap.nationalmap.gov/arcgis/rest/services/USGSImageryOnly/MapServer/tile/{z}/{y}/{x}",
        attribution: "USGS The National Map",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "usgs-imagery-topo",
        name: "USGS Imagery Topo",
        kind: "raster",
        url: "https://basemap.nationalmap.gov/arcgis/rest/services/USGSImageryTopo/MapServer/tile/{z}/{y}/{x}",
        attribution: "USGS The National Map",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "nasa-blue-marble",
        name: "NASA Blue Marble",
        kind: "raster",
        url: "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/BlueMarble_ShadedRelief_Bathymetry/default//GoogleMapsCompatible_Level8/{z}/{y}/{x}.jpeg",
        attribution: "Imagery: NASA Global Imagery Browse Services (GIBS), part of EOSDIS",
        max_zoom: 8,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "nasa-night-lights",
        name: "NASA Earth at Night",
        kind: "raster",
        url: "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/VIIRS_CityLights_2012/default//GoogleMapsCompatible_Level8/{z}/{y}/{x}.jpg",
        attribution: "Imagery: NASA Global Imagery Browse Services (GIBS), part of EOSDIS",
        max_zoom: 8,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "usgs-topo",
        name: "USGS Topo",
        kind: "raster",
        url: "https://basemap.nationalmap.gov/arcgis/rest/services/USGSTopo/MapServer/tile/{z}/{y}/{x}",
        attribution: "USGS The National Map",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "usgs-shaded-relief",
        name: "USGS Shaded Relief",
        kind: "raster",
        url: "https://basemap.nationalmap.gov/arcgis/rest/services/USGSShadedReliefOnly/MapServer/tile/{z}/{y}/{x}",
        attribution: "USGS The National Map",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-terrain",
        name: "Esri World Terrain",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Terrain_Base/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: USGS, Esri, TANA, DeLorme, and NPS",
        max_zoom: 13,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-shaded-relief",
        name: "Esri Shaded Relief",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Shaded_Relief/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: Esri",
        max_zoom: 13,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-physical",
        name: "Esri Physical",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Physical_Map/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: US National Park Service",
        max_zoom: 8,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-hillshade",
        name: "Esri World Hillshade",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Elevation/World_Hillshade/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Sources: Esri, Airbus DS, USGS, NGA, NASA, CGIAR, N Robinson, NCEAS, NLS, OS, NMA, Geodatastyrelsen, Rijkswaterstaat, GSA, Geoland, FEMA, Intermap and the GIS user community",
        max_zoom: 16,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "openfreemap-liberty",
        name: "OpenFreeMap Liberty",
        kind: "style",
        url: "https://tiles.openfreemap.org/styles/liberty",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "openfreemap-bright",
        name: "OpenFreeMap Bright",
        kind: "style",
        url: "https://tiles.openfreemap.org/styles/bright",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "openfreemap-positron",
        name: "OpenFreeMap Positron",
        kind: "style",
        url: "https://tiles.openfreemap.org/styles/positron",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "openfreemap-dark",
        name: "OpenFreeMap Dark",
        kind: "style",
        url: "https://tiles.openfreemap.org/styles/dark",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "openfreemap-fiord",
        name: "OpenFreeMap Fiord",
        kind: "style",
        url: "https://tiles.openfreemap.org/styles/fiord",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "openfreemap-tiles",
        name: "OpenFreeMap Tiles (UniGIS Style)",
        kind: "vector",
        url: "https://tiles.openfreemap.org/planet",
        attribution: "OpenFreeMap © OpenMapTiles © OpenStreetMap contributors",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "versatiles-colorful",
        name: "VersaTiles Colorful",
        kind: "style",
        url: "https://tiles.versatiles.org/assets/styles/colorful/style.json",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "versatiles-eclipse",
        name: "VersaTiles Eclipse",
        kind: "style",
        url: "https://tiles.versatiles.org/assets/styles/colorful-dark/style.json",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "versatiles-graybeard",
        name: "VersaTiles Graybeard",
        kind: "style",
        url: "https://tiles.versatiles.org/assets/styles/gray/style.json",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "versatiles-shadow",
        name: "VersaTiles Shadow",
        kind: "style",
        url: "https://tiles.versatiles.org/assets/styles/gray-dark/style.json",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "versatiles-neutrino",
        name: "VersaTiles Neutrino",
        kind: "style",
        url: "https://tiles.versatiles.org/assets/styles/muted/style.json",
        attribution: "",
        max_zoom: 14,
        subdomains: &[],
        overlay: false,
    },
    Preset {
        id: "esri-places",
        name: "Esri Boundaries and Places",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Boundaries_and_Places/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: Esri and its data partners",
        max_zoom: 19,
        subdomains: &[],
        overlay: true,
    },
    Preset {
        id: "esri-gray-reference",
        name: "Esri Light Gray Reference",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Reference/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors",
        max_zoom: 16,
        subdomains: &[],
        overlay: true,
    },
    Preset {
        id: "esri-ocean-reference",
        name: "Esri Ocean Reference",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Ocean/World_Ocean_Reference/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Sources: GEBCO, NOAA, CHS, OSU, UNH, CSUMB, National Geographic, DeLorme, NAVTEQ, and Esri",
        max_zoom: 13,
        subdomains: &[],
        overlay: true,
    },
    Preset {
        id: "esri-transportation",
        name: "Esri Transportation",
        kind: "raster",
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Transportation/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri — Source: Esri and its data partners",
        max_zoom: 19,
        subdomains: &[],
        overlay: true,
    },
    Preset {
        id: "openrailwaymap",
        name: "OpenRailwayMap",
        kind: "raster",
        url: "https://{s}.tiles.openrailwaymap.org/standard/{z}/{x}/{y}.png",
        attribution: "Map data: © OpenStreetMap contributors | Map style: © OpenRailwayMap (CC-BY-SA)",
        max_zoom: 19,
        subdomains: &["a", "b", "c"],
        overlay: true,
    },
    Preset {
        id: "openseamap",
        name: "OpenSeaMap",
        kind: "raster",
        url: "https://tiles.openseamap.org/seamark/{z}/{x}/{y}.png",
        attribution: "Map data: © OpenSeaMap contributors",
        max_zoom: 18,
        subdomains: &[],
        overlay: true,
    },
    Preset {
        id: "waymarked-hiking",
        name: "Hiking Trails",
        kind: "raster",
        url: "https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png",
        attribution: "Map data: © OpenStreetMap contributors | Map style: © waymarkedtrails.org (CC-BY-SA)",
        max_zoom: 18,
        subdomains: &[],
        overlay: true,
    },
    Preset {
        id: "waymarked-cycling",
        name: "Cycling Routes",
        kind: "raster",
        url: "https://tile.waymarkedtrails.org/cycling/{z}/{x}/{y}.png",
        attribution: "Map data: © OpenStreetMap contributors | Map style: © waymarkedtrails.org (CC-BY-SA)",
        max_zoom: 18,
        subdomains: &[],
        overlay: true,
    },
    Preset {
        id: "usgs-hydro",
        name: "USGS Hydrography",
        kind: "raster",
        url: "https://basemap.nationalmap.gov/arcgis/rest/services/USGSHydroCached/MapServer/tile/{z}/{y}/{x}",
        attribution: "USGS The National Map",
        max_zoom: 16,
        subdomains: &[],
        overlay: true,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_presets_replace_basemaps_keep_overlays_and_undo() {
        let mut app = RustGisApp::with_sample();
        assert_eq!(PRESETS.len(), 43);
        let mut ids = std::collections::HashSet::new();
        for preset in PRESETS {
            assert!(ids.insert(preset.id));
            app.run("map.removeTiles", json!({})).unwrap();
            app.run("map.addTiles", json!({"url": "https://example.test/overlay/{z}/{x}/{y}.png", "overlay": true, "id": "custom"}))
                .unwrap();
            app.run("map.addTiles", default_tiles()).unwrap();
            let before = app.session.map().tiles.clone();
            choose(&mut app, preset);
            assert!(app.session.map().tiles.iter().any(|t| preset.matches(t)));
            assert!(app.session.map().tiles.iter().any(|t| t.id == "custom" && t.overlay));
            let tile = app.session.map().tiles.iter().find(|t| preset.matches(t)).unwrap();
            assert_eq!(tile.max_zoom, preset.max_zoom);
            assert_eq!(tile.attribution, preset.attribution);
            if preset.overlay {
                assert_eq!(app.session.map().tiles.len(), 3);
                choose(&mut app, preset);
                assert_eq!(app.session.map().tiles, before);
            } else {
                assert_eq!(app.session.map().tiles.len(), 2);
            }
            app.run("edit.undo", json!({})).unwrap();
            if preset.overlay {
                assert!(app.session.map().tiles.iter().any(|t| preset.matches(t)));
            } else {
                assert_eq!(app.session.map().tiles, before);
            }
        }
    }

    #[test]
    fn startup_does_not_fetch_plugin_packages() {
        use std::sync::{Arc, Mutex};
        let fetched = Arc::new(Mutex::new(Vec::new()));
        let requests = fetched.clone();
        let mut app = RustGisApp::with_sample().with_services(crate::Services {
            fetch: Some(Box::new(move |url, _| requests.lock().unwrap().push(url.to_owned()))),
            ..Default::default()
        });
        app.start(&crate::Start::default()).unwrap();
        assert!(fetched.lock().unwrap().is_empty());
        assert!(app.session.map().tiles.iter().any(|t| OSM.matches(t)));
    }
}
