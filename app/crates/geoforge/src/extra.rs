//! More GeoForge vector tools for the GeoForge registry: geometry attributes, geometry checking and
//! repair, and Split.
//!
//! Modified for GeoForge (not part of upstream GeoForge).

use geoforge_tools as tools;
use rustgis_analysis::{Inputs, Output, OutputLayer, ParamKind as K, ParamSpec as P, ToolResult};
use rustgis_data::{Dataset, FieldType, Value};
use serde_json::Value as Json;

use crate::{
    Def, INPUT, OUTPUT, Work, datasets, err, finish_update, layer, layer_in, list, messages, out_name, table_dataset, text,
    to_dataset_checked,
};

fn length_unit(name: &str) -> f64 {
    match name.to_ascii_uppercase().as_str() {
        "KILOMETERS" => 1000.0,
        "MILES" => 1609.344,
        "FEET" => 0.3048,
        "YARDS" => 0.9144,
        "NAUTICAL_MILES" => 1852.0,
        _ => 1.0,
    }
}

fn area_unit(name: &str) -> f64 {
    match name.to_ascii_uppercase().as_str() {
        "SQUARE_KILOMETERS" => 1e6,
        "HECTARES" => 1e4,
        "ACRES" => 4_046.856_422_4,
        "SQUARE_FEET" => 0.092_903_04,
        "SQUARE_MILES" => 2_589_988.110_336,
        "SQUARE_YARDS" => 0.836_127_36,
        _ => 1.0,
    }
}

/// Add Geometry Attributes, in place. Lengths and areas are measured on the ground in the UTM zone of the
/// layer; coordinates and counts are the layer's own (longitude/latitude).
fn add_geometry_attributes(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    use tools::GeometryProperty as G;
    let props: Vec<G> = list(p, "properties")
        .iter()
        .map(|s| G::parse(s).ok_or_else(|| format!("ERROR 000800: The value is not a member of the geometry properties: {s}")))
        .collect::<Result<_, _>>()?;
    if props.is_empty() {
        return Err("Add Geometry Attributes: choose at least one property".into());
    }
    let measures = |g: &G| matches!(g, G::Area | G::PerimeterLength | G::Length);
    let (by_ground, by_layer): (Vec<G>, Vec<G>) = props.into_iter().partition(measures);
    let all = datasets(inp, p, &["input"])?;
    let ds = all[0];
    let (lu, au) = (length_unit(text(p, "length_unit")), area_unit(text(p, "area_unit")));

    let degrees = Work::degrees();
    let (_, fc) = layer_in(inp, p, "input", &degrees, true)?;
    let mut out = if by_layer.is_empty() {
        None
    } else {
        Some(to_dataset_checked(tools::add_geometry_attributes(&fc, &by_layer, 1.0, 1.0).map_err(err)?, ds.name.clone(), &degrees)?)
    };
    if !by_ground.is_empty() {
        let metres = Work::metres(&all)?;
        let (_, mfc) = layer_in(inp, p, "input", &metres, true)?;
        let tool_out = tools::add_geometry_attributes(&mfc, &by_ground, lu, au).map_err(err)?;
        let measured = to_dataset_checked(tool_out, ds.name.clone(), &metres)?;
        match &mut out {
            None => out = Some(measured),
            Some(o) => {
                for name in by_ground.iter().flat_map(|g| match g {
                    G::Area => ["POLY_AREA"].as_slice(),
                    G::PerimeterLength => ["PERIMETER"].as_slice(),
                    _ => ["LENGTH"].as_slice(),
                }) {
                    let (Some(from), true) = (measured.field_index(name), true) else { continue };
                    let to = o.ensure_field(name, FieldType::Double);
                    for (row, f) in measured.features.iter().enumerate() {
                        if let (Some(v), Some(t)) = (f.attrs.get(from), o.features.get_mut(row)) {
                            if t.attrs.len() <= to {
                                t.attrs.resize(to + 1, Value::Null);
                            }
                            t.attrs[to] = v.clone();
                        }
                    }
                }
            }
        }
    }
    let new = out.ok_or("Add Geometry Attributes: nothing to add")?;
    let m = vec![format!("{}: geometry attributes added to {} features", new.name, new.features.len())];
    Ok(ToolResult { output: Output::Update { layer: ds.name.clone(), dataset: new }, messages: m })
}

fn method(p: &Json) -> Result<geoforge_geometry::ValidationMethod, String> {
    match text(p, "validation_method").to_ascii_uppercase().as_str() {
        "" | "ESRI" => Ok(geoforge_geometry::ValidationMethod::Esri),
        "OGC" => Ok(geoforge_geometry::ValidationMethod::Ogc),
        other => Err(format!("ERROR 000800: The value is not a member of ESRI | OGC: {other}")),
    }
}

fn check_geometry(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer(inp, p, "input")?;
    let input = tools::CheckInput { source: ds.name.clone(), features: &fc, has_spatial_index: true };
    let (rows, warnings) = tools::check_geometry(&[input], method(p)?).map_err(err)?;
    let fields = tools::check_geometry_fields();
    let table = tools::TableOutput {
        fields,
        rows: rows
            .into_iter()
            .map(|r| {
                vec![
                    geoforge_core::Value::String(r.class),
                    geoforge_core::Value::Integer(r.feature_id as i32),
                    geoforge_core::Value::String(r.problem),
                ]
            })
            .collect(),
    };
    let t = table_dataset(table, out_name(p, format!("{}_Check", ds.name)));
    let mut m = vec![format!("{}: {} problems", t.name, t.features.len())];
    m.extend(warnings);
    Ok(ToolResult { output: Output::Table(t), messages: m })
}

fn repair_geometry(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, fc) = layer_in(inp, p, "input", &Work::degrees(), true)?;
    let delete_null = text(p, "delete_null").eq_ignore_ascii_case("DELETE_NULL");
    finish_update(tools::repair_geometry(&fc, delete_null, method(p)?).map_err(err)?, ds)
}

/// Split: one layer per value of the split field of the split features.
fn split(inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let (ds, input) = layer(inp, p, "input")?;
    let (_, splitter) = layer(inp, p, "other")?;
    let parts = tools::split(&input, &splitter, text(p, "split_field")).map_err(err)?;
    let work = Work::degrees();
    let prefix = out_name(p, ds.name.clone());
    let mut layers = Vec::new();
    let mut m = Vec::new();
    for (value, out) in parts {
        let name = format!("{prefix}_{value}");
        let d: Dataset = to_dataset_checked(out.clone(), name, &work)?;
        m.extend(messages(&out, &d, &work));
        layers.push(OutputLayer::Vector(d));
    }
    if layers.is_empty() {
        m.push("Split: nothing was inside the split features".into());
    }
    Ok(ToolResult { output: Output::Layers(layers), messages: m })
}

pub(crate) const DEFS: &[Def] = &[
    Def {
        id: "add_geometry_attributes",
        name: "Add Geometry Attributes",
        toolbox: "GeoForge › Data Management › Features",
        summary: "Adds area, length, coordinates, counts and more as fields, in place (ArcGIS Pro Add Geometry Attributes). Lengths and areas are measured on the ground.",
        params: &[
            INPUT,
            P {
                name: "properties",
                label: "Geometry properties",
                kind: K::Text,
                required: true,
                default: "",
                help: "Separated by semicolons: AREA PERIMETER_LENGTH LENGTH CENTROID CENTROID_INSIDE EXTENT LINE_START_MID_END LINE_BEARING PART_COUNT POINT_COUNT POINT_X_Y_Z_M",
            },
            P {
                name: "length_unit",
                label: "Length unit",
                kind: K::Choice(&["METERS", "KILOMETERS", "MILES", "FEET", "YARDS", "NAUTICAL_MILES"]),
                required: false,
                default: "\"METERS\"",
                help: "",
            },
            P {
                name: "area_unit",
                label: "Area unit",
                kind: K::Choice(&[
                    "SQUARE_METERS",
                    "SQUARE_KILOMETERS",
                    "HECTARES",
                    "ACRES",
                    "SQUARE_FEET",
                    "SQUARE_MILES",
                    "SQUARE_YARDS",
                ]),
                required: false,
                default: "\"SQUARE_METERS\"",
                help: "",
            },
        ],
        run: add_geometry_attributes,
    },
    Def {
        id: "check_geometry",
        name: "Check Geometry",
        toolbox: "GeoForge › Data Management › Features",
        summary: "A table of the features with geometry problems (ArcGIS Pro Check Geometry).",
        params: &[
            INPUT,
            P {
                name: "validation_method",
                label: "Validation method",
                kind: K::Choice(&["ESRI", "OGC"]),
                required: false,
                default: "\"ESRI\"",
                help: "",
            },
            OUTPUT,
        ],
        run: check_geometry,
    },
    Def {
        id: "repair_geometry",
        name: "Repair Geometry",
        toolbox: "GeoForge › Data Management › Features",
        summary: "Repairs the geometry problems of a layer, in place (ArcGIS Pro Repair Geometry).",
        params: &[
            INPUT,
            P {
                name: "delete_null",
                label: "Delete features with null geometry",
                kind: K::Choice(&["DELETE_NULL", "KEEP_NULL"]),
                required: false,
                default: "\"DELETE_NULL\"",
                help: "",
            },
            P {
                name: "validation_method",
                label: "Validation method",
                kind: K::Choice(&["ESRI", "OGC"]),
                required: false,
                default: "\"ESRI\"",
                help: "",
            },
        ],
        run: repair_geometry,
    },
    Def {
        id: "split",
        name: "Split",
        toolbox: "GeoForge › Analysis › Extract",
        summary: "One layer per value of a text field of the split features, each clipped by them (ArcGIS Pro Split).",
        params: &[
            INPUT,
            P { name: "other", label: "Split Features", kind: K::PolygonLayer, required: true, default: "", help: "" },
            P {
                name: "split_field",
                label: "Split Field",
                kind: K::Text,
                required: true,
                default: "",
                help: "A text field of the split features.",
            },
            OUTPUT,
        ],
        run: split,
    },
];
