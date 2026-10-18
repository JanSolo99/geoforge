//! The tools of `geoforge_tools::registry` as GeoForge tools: their parameters become tool
//! parameters and a run reads them through [`Context`]. A tool added to the registry needs no
//! wrapper here.
//!
//! Modified for GeoForge (not part of upstream GeoForge).

use geoforge_tools::registry::{self, Context, Kind, Tool};
use rustgis_analysis::{Inputs, ParamKind as K, ParamSpec as P, ToolResult};
use serde_json::Value as Json;

use crate::{OUTPUT, Work, finish_in, layer_in, out_name, text};

/// The tool's parameters as the application shows them (leaked once per tool).
pub(crate) fn params_of(t: &'static Tool) -> &'static [P] {
    static CACHE: std::sync::Mutex<Vec<(&'static str, &'static [P])>> = std::sync::Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, p)) = cache.iter().find(|(id, _)| *id == t.id) {
        return p;
    }
    let mut params: Vec<P> = t
        .params
        .iter()
        .map(|p| P {
            name: p.name,
            label: p.label,
            kind: match p.kind {
                Kind::Features => K::Layer,
                Kind::Field { numeric: true, .. } => K::NumericField,
                Kind::Field { numeric: false, .. } => K::Field,
                Kind::Choice(c) => K::Choice(c),
                Kind::Number => K::Number,
                Kind::Text => K::Text,
                Kind::Bool => K::Bool,
            },
            required: p.required,
            // The application keeps defaults as JSON text.
            default: if p.default.is_empty() {
                ""
            } else if matches!(p.kind, Kind::Number | Kind::Bool) {
                p.default
            } else {
                Box::leak(serde_json::Value::String(p.default.into()).to_string().into_boxed_str())
            },
            help: p.help,
        })
        .collect();
    params.push(OUTPUT);
    let all: &'static [P] = Box::leak(params.into_boxed_slice());
    cache.push((t.id, all));
    all
}

struct Ctx<'a> {
    inp: &'a dyn Inputs,
    p: &'a Json,
    work: &'a Work,
}

impl Context for Ctx<'_> {
    fn get(&self, name: &str) -> Option<String> {
        let v = match self.p.get(name)? {
            Json::String(s) => s.trim().to_string(),
            Json::Null => return None,
            other => other.to_string(),
        };
        (!v.is_empty()).then_some(v)
    }

    fn features(&self, name: &str) -> geoforge_core::Result<Box<dyn geoforge_core::FeatureClass + '_>> {
        let (_, fc) = layer_in(self.inp, self.p, name, self.work, false).map_err(geoforge_core::Error::Unsupported)?;
        Ok(Box::new(fc))
    }

    fn default_spatial_reference_wkt(&self) -> Option<String> {
        Some(self.work.wkt.clone())
    }
}

/// Runs a registry tool.
pub(crate) fn run(t: &'static Tool, inp: &dyn Inputs, p: &Json) -> Result<ToolResult, String> {
    let work = if t.measures {
        let names: Vec<&str> = t.params.iter().filter(|q| q.kind == Kind::Features).map(|q| q.name).collect();
        let sets: Vec<&rustgis_data::Dataset> = names.iter().filter_map(|n| inp.dataset(text(p, n))).collect();
        Work::metres(&sets)?
    } else {
        Work::degrees()
    };
    let ctx = Ctx { inp, p, work: &work };
    let out = registry::run(t, &ctx).map_err(|e| e.to_string())?;
    let first = t.params.iter().filter(|q| q.kind == Kind::Features).map(|q| text(p, q.name)).find(|n| !n.is_empty());
    let default = first.map_or_else(|| t.output_suffix.to_string(), |n| format!("{n}_{}", t.output_suffix));
    finish_in(out, out_name(p, default), &work)
}
