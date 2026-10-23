//! Data Management dataset tools: Create Feature Class, Create Table, Define Projection and Copy.
//!
//! Matched to ArcGIS Pro 3.7 on shapefiles and dBASE tables (cases `cfc_*`, `ct_*`, `defproj_*`,
//! `copy_*`):
//! - Create Feature Class makes an empty shapefile with the template's attribute fields (its
//!   coordinate system is not taken over), or the default `Id` field (Long, 6 digits); Create
//!   Table likewise, with `Field1` (Long) by default.
//! - Define Projection only sets the coordinate system; Copy keeps every field and feature.

use unigis_core::{Error, FeatureClass, Field, FieldType, GeometryKind, Result};

use crate::{TableOutput, ToolOutput};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

/// How Create Feature Class decides Z or M awareness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awareness {
    Disabled,
    Enabled,
    SameAsTemplate,
}

impl Awareness {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "DISABLED" | "" => Self::Disabled,
            "ENABLED" => Self::Enabled,
            "SAME_AS_TEMPLATE" => Self::SameAsTemplate,
            _ => return None,
        })
    }

    fn resolve(self, template: Option<bool>) -> bool {
        match self {
            Self::Disabled => false,
            Self::Enabled => true,
            Self::SameAsTemplate => template.unwrap_or(false),
        }
    }
}

pub fn geometry_kind(name: &str) -> Option<GeometryKind> {
    Some(match name.to_ascii_uppercase().as_str() {
        "POINT" => GeometryKind::Point,
        "MULTIPOINT" => GeometryKind::Multipoint,
        "POLYLINE" => GeometryKind::Polyline,
        "POLYGON" => GeometryKind::Polygon,
        "MULTIPATCH" => GeometryKind::MultiPatch,
        _ => return None,
    })
}

fn attribute_fields(template: Option<&dyn FeatureClass>, default: Field) -> Vec<Field> {
    match template {
        Some(t) => t
            .fields()
            .iter()
            .filter(|f| {
                !matches!(
                    f.field_type,
                    FieldType::ObjectId | FieldType::Geometry | FieldType::GlobalId
                )
            })
            .cloned()
            .collect(),
        None => vec![default],
    }
}

fn long_field(name: &str) -> Field {
    let mut f = Field::new(name, FieldType::Integer);
    f.precision = if name == "Id" { 6 } else { 10 };
    f
}

/// `arcpy.management.CreateFeatureclass(out_path, out_name, geometry_type, template, has_m,
/// has_z, spatial_reference)`.
pub fn create_feature_class(
    geometry_type: &str,
    template: Option<&dyn FeatureClass>,
    has_m: Awareness,
    has_z: Awareness,
    spatial_reference_wkt: Option<String>,
) -> Result<ToolOutput> {
    let kind = geometry_kind(geometry_type).ok_or_else(|| {
        err(format!(
            "ERROR 000067: unknown geometry type {geometry_type}"
        ))
    })?;
    Ok(ToolOutput {
        kind,
        has_z: has_z.resolve(template.map(|t| t.has_z())),
        has_m: has_m.resolve(template.map(|t| t.has_m())),
        fields: attribute_fields(template, long_field("Id")),
        spatial_reference_wkt,
        features: Vec::new(),
        messages: Vec::new(),
    })
}

/// `arcpy.management.CreateTable(out_path, out_name, template)`.
pub fn create_table(template: Option<&dyn FeatureClass>) -> TableOutput {
    TableOutput {
        fields: attribute_fields(template, long_field("Field1")),
        rows: Vec::new(),
    }
}

/// A copy of `input` as it is, with `spatial_reference_wkt` when given (Define Projection).
pub fn copy_features(
    input: &dyn FeatureClass,
    spatial_reference_wkt: Option<String>,
) -> Result<ToolOutput> {
    let kind = input
        .geometry_kind()
        .ok_or_else(|| err("ERROR 000735: Input Dataset: the input has no geometry"))?;
    Ok(ToolOutput {
        kind,
        has_z: input.has_z(),
        has_m: input.has_m(),
        fields: input.fields().to_vec(),
        spatial_reference_wkt: spatial_reference_wkt
            .or_else(|| input.spatial_reference_wkt().map(str::to_owned)),
        features: input.features().collect::<Result<_>>()?,
        messages: Vec::new(),
    })
}

/// The rows of a table as they are (Copy of a table).
pub fn copy_table(input: &dyn FeatureClass) -> Result<TableOutput> {
    let rows = input
        .features()
        .map(|f| f.map(|f| f.attributes))
        .collect::<Result<_>>()?;
    Ok(TableOutput {
        fields: input.fields().to_vec(),
        rows,
    })
}
