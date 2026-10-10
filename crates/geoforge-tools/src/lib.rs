//! Geoprocessing tools. Each follows its ArcGIS Pro counterpart (parameters,
//! output schema, feature order, messages) as observed through the ArcGIS bridge
//! and recorded in `testdata/arcgis-reference/manifest.json`.

mod zm;

use geoforge_core::{Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value};
use geoforge_geometry::{BufferOptions, ClipArea, LineEnd};

/// A tool's result, ready to be written to any output format.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub kind: GeometryKind,
    pub has_z: bool,
    pub has_m: bool,
    pub fields: Vec<Field>,
    pub spatial_reference_wkt: Option<String>,
    pub features: Vec<Feature>,
    /// Warnings in ArcGIS style (`WARNING nnnnnn: ...`).
    pub messages: Vec<String>,
}

/// `arcpy.analysis.Clip(in_features, clip_features, out_feature_class)`.
///
/// Output keeps the input's schema, order and Z/M awareness; features entirely
/// outside the (dissolved) clip polygons are dropped; split features stay
/// multipart.
pub fn clip(input: &dyn FeatureClass, clip_features: &dyn FeatureClass) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    if clip_features.geometry_kind() != Some(GeometryKind::Polygon) {
        return Err(Error::Unsupported(
            "ERROR 000365: clip features must be polygons".into(),
        ));
    }
    let clip_geoms: Vec<_> = clip_features
        .features()
        .filter_map(|f| f.map(|f| f.geometry).transpose())
        .collect::<Result<_>>()?;
    let area = ClipArea::new(clip_geoms.iter())?;
    let mut features = Vec::new();
    for f in input.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        if let Some(mut out) = geoforge_geometry::clip(g, &area)? {
            if kind != GeometryKind::Point && kind != GeometryKind::Multipoint {
                zm::restore(&mut out, g, input.has_z(), input.has_m());
            }
            features.push(Feature {
                oid: f.oid,
                geometry: Some(out),
                attributes: f.attributes,
            });
        }
    }
    let mut messages = Vec::new();
    if features.is_empty() {
        messages.push("WARNING 000117: Warning empty output generated.".into());
    }
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
        features,
        messages,
    })
}

/// ArcGIS `dissolve_option`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dissolve {
    None,
    All,
}

/// `arcpy.analysis.Buffer(in, out, distance, "FULL", line_end, dissolve)`, planar.
///
/// `distance` is in the input's linear unit. Without dissolving, the output has the
/// input fields plus `BUFF_DIST` and `ORIG_FID`, one feature per input feature
/// (in input order); with `Dissolve::All` it is a single feature with no fields.
pub fn buffer(
    input: &dyn FeatureClass,
    distance: f64,
    line_end: LineEnd,
    dissolve: Dissolve,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
    let sr = input.spatial_reference_wkt();
    if sr.is_some_and(|w| w.trim_start().starts_with("GEOGCS")) {
        return Err(Error::Unsupported(
            "buffering geographic data needs geodesic buffers (coordinate systems are Phase 4)"
                .into(),
        ));
    }
    let opts = BufferOptions {
        distance,
        line_end,
        max_deviation: geoforge_core::curves::arcgis_densify_deviation(false),
    };
    let mut out = ToolOutput {
        kind: GeometryKind::Polygon,
        has_z: false,
        has_m: false,
        fields: Vec::new(),
        spatial_reference_wkt: sr.map(str::to_owned),
        features: Vec::new(),
        messages: Vec::new(),
    };
    match dissolve {
        Dissolve::All => {
            let geoms: Vec<_> = input
                .features()
                .filter_map(|f| f.map(|f| f.geometry).transpose())
                .collect::<Result<_>>()?;
            if let Some(g) = geoforge_geometry::buffer_union(geoms.iter(), &opts)? {
                out.features.push(Feature {
                    oid: 1,
                    geometry: Some(g),
                    attributes: Vec::new(),
                });
            }
        }
        Dissolve::None => {
            out.fields = input.fields().to_vec();
            out.fields.push(Field::new("BUFF_DIST", FieldType::Double));
            out.fields.push(Field::new("ORIG_FID", FieldType::Integer));
            for f in input.features() {
                let f = f?;
                let Some(g) = &f.geometry else { continue };
                let Some(b) = geoforge_geometry::buffer(g, &opts)? else {
                    continue;
                };
                let mut attributes = f.attributes;
                attributes.push(Value::Double(distance));
                attributes.push(Value::Integer(f.oid as i32));
                out.features.push(Feature {
                    oid: f.oid,
                    geometry: Some(b),
                    attributes,
                });
            }
        }
    }
    let _ = kind;
    Ok(out)
}
