//! One tool list: the built-in tools that an ArcGIS-verified counterpart replaces.
//!
//! The application's own tools predate the UniGIS suite (`rustgis-unigis`, ids `unigis:*`),
//! whose tools are matched to ArcGIS Pro. When both have a tool (Buffer, Clip, Minimum Bounding
//! Geometry...), only the suite's is listed and run. The built-in ids (saved models, history,
//! scripts) still resolve, to the suite's tool, and their parameters are translated.

use serde_json::{Map, Value};

/// Built-in ids whose ArcGIS-verified counterpart has another id. Built-in ids that are the same
/// as the counterpart's (`buffer`, `clip`, `erase`, `dissolve`) need no entry.
pub const ALIASES: &[(&str, &str)] = &[
    ("convexHull", "minimum_bounding_geometry"),
    ("featureToPoint", "feature_to_point"),
    ("selectByLocation", "select_by_location"),
    ("summaryStatistics", "statistics"),
];

/// The prefix of the ArcGIS-verified suite's tool ids.
pub const SUITE_PREFIX: &str = "unigis:";

/// The suite's id prefix before the rename; ids with it still resolve.
pub const OLD_SUITE_PREFIX: &str = "geoforge:";

/// The suite's id for a built-in tool id (`buffer` is `unigis:buffer`; `convexHull` is
/// `unigis:minimum_bounding_geometry`).
pub fn counterpart_id(id: &str) -> String {
    let id = id.strip_prefix(SUITE_PREFIX).or_else(|| id.strip_prefix(OLD_SUITE_PREFIX)).unwrap_or(id);
    let canonical = ALIASES.iter().find(|(old, _)| old.eq_ignore_ascii_case(id)).map_or(id, |(_, new)| new);
    format!("{SUITE_PREFIX}{canonical}")
}

fn rename(p: &mut Map<String, Value>, old: &str, new: &str) {
    if !p.contains_key(new)
        && let Some(v) = p.remove(old)
    {
        p.insert(new.into(), v);
    }
}

fn upper(p: &mut Map<String, Value>, key: &str) {
    if let Some(Value::String(s)) = p.get_mut(key) {
        *s = s.trim().to_ascii_uppercase();
    }
}

/// Parameters written for a built-in tool, in the names of its ArcGIS-verified counterpart
/// `spec_id` (`unigis:...`). Parameters already in the counterpart's names pass unchanged.
pub fn translate_params(spec_id: &str, params: &Value) -> Value {
    let Some(obj) = params.as_object() else { return params.clone() };
    let mut p = obj.clone();
    match spec_id.strip_prefix(SUITE_PREFIX).unwrap_or("") {
        "buffer" => {
            upper(&mut p, "units");
            upper(&mut p, "dissolve");
        }
        "dissolve" => rename(&mut p, "field", "fields"),
        "minimum_bounding_geometry" => {
            if p.contains_key("group") {
                // The built-in tool made convex hulls only.
                p.entry("geometry_type").or_insert_with(|| "CONVEX_HULL".into());
                rename(&mut p, "group", "group_option");
                upper(&mut p, "group_option");
            }
        }
        "select_by_location" => {
            rename(&mut p, "select", "other");
            if let Some(d) = p.remove("distance") {
                let km = d.as_f64().or_else(|| d.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0);
                if km > 0.0 && !p.contains_key("search_distance") {
                    p.insert("search_distance".into(), km.into());
                    p.entry("units").or_insert_with(|| "KILOMETERS".into());
                }
            }
            if let Some(Value::String(s)) = p.get("invert") {
                let on = s.trim().eq_ignore_ascii_case("yes") || s.trim().eq_ignore_ascii_case("true");
                p.insert("invert".into(), on.into());
            }
        }
        "statistics" => {
            if let Some(field) = p.remove("field").and_then(|v| v.as_str().map(|s| s.trim().to_string()))
                && !p.contains_key("statistics")
            {
                let stats = ["COUNT", "SUM", "MEAN", "MIN", "MAX", "STD"].map(|s| format!("{field} {s}")).join(";");
                p.insert("statistics".into(), stats.into());
            }
            rename(&mut p, "case", "case_fields");
        }
        _ => {}
    }
    Value::Object(p)
}
