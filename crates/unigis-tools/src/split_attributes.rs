//! Split By Attributes: pure grouping with workspace writes supplied by the host.
use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::registry::{Context, Kind, Param, Report, Runner, Tool, WorkspaceOutput};
use crate::{TableOutput, ToolOutput};
use unigis_core::{Error, Feature, FeatureClass, FieldType, Result, Value};

fn err(message: &str) -> Error {
    Error::Unsupported(message.into())
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Integer(i32),
    Text(String),
}
impl Key {
    fn from_value(value: &Value) -> Result<Self> {
        match value {
            Value::Integer(v) => Ok(Self::Integer(*v)),
            Value::SmallInteger(v) => Ok(Self::Integer(i32::from(*v))),
            Value::String(v) => Ok(Self::Text(v.clone())),
            _ => Err(err(
                "null, floating-point, date and 64-bit split values are not yet supported",
            )),
        }
    }
    fn text(&self) -> String {
        match self {
            Self::Integer(v) => v.to_string(),
            Self::Text(v) => v.clone(),
        }
    }
    fn order(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()),
            _ => self.cmp(other),
        }
    }
}

/// Observed folder naming. Keep punctuation Windows permits, replace separators,
/// periods and whitespace, and preserve leading underscores/hyphens/digits.
pub(crate) fn dataset_name(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if !raw.is_ascii() || raw.chars().any(|c| c.is_ascii_control()) || raw.len() > 128 {
        return Err(err(
            "split output names currently require at most128 printable ASCII characters",
        ));
    }
    let mut name: String = raw
        .chars()
        .map(|c| if "\\/:*?\"<>|. ".contains(c) { '_' } else { c })
        .collect();
    if raw.starts_with('.') || name.is_empty() {
        name.insert(0, 'T');
    }
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3",
    ]
    .contains(&name.to_ascii_uppercase().as_str())
    {
        return Err(err(
            "reserved device names are not supported for split outputs",
        ));
    }
    Ok(name)
}

pub fn partition(
    input: &dyn FeatureClass,
    split_fields: &str,
) -> Result<Vec<(String, WorkspaceOutput)>> {
    let names: Vec<&str> = split_fields
        .split(';')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        return Err(err("ERROR 000735: Split Fields: Value is required"));
    }
    let mut positions = Vec::new();
    for name in names {
        if input.has_object_id() && name.eq_ignore_ascii_case(input.oid_field_name()) {
            return Err(err(
                "ERROR 003911: The field is not of type SHORT | LONG | TEXT",
            ));
        }
        let index = crate::field_positions(input, &[name.to_owned()])?[0];
        if !matches!(
            input.fields()[index].field_type,
            FieldType::SmallInteger | FieldType::Integer | FieldType::String
        ) {
            return Err(err(
                "floating-point, date and 64-bit split fields are not yet supported",
            ));
        }
        positions.push(index);
    }
    let mut lookup = BTreeMap::new();
    let mut groups: Vec<(Vec<Key>, Vec<Feature>)> = Vec::new();
    for feature in input.features() {
        let feature = feature?;
        let key = positions
            .iter()
            .map(|i| {
                feature
                    .attributes
                    .get(*i)
                    .ok_or_else(|| err("input row does not match its schema"))
                    .and_then(Key::from_value)
            })
            .collect::<Result<Vec<_>>>()?;
        let index = match lookup.get(&key) {
            Some(&i) => i,
            None => {
                let i = groups.len();
                lookup.insert(key.clone(), i);
                groups.push((key, Vec::new()));
                i
            }
        };
        groups[index].1.push(feature);
    }
    // Stable, case-insensitive ordering explains both reversed normalization
    // collisions and equal-case keys (which retain first-input order).
    groups.sort_by(|(a, _), (b, _)| {
        a.iter()
            .zip(b)
            .map(|(a, b)| a.order(b))
            .find(|v| *v != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    });
    let mut outputs = Vec::new();
    for (key, features) in groups {
        let name = dataset_name(&key.iter().map(Key::text).collect::<Vec<_>>().join("_"))?;
        let data = match input.geometry_kind() {
            Some(kind) => WorkspaceOutput::Features(ToolOutput {
                kind,
                has_z: input.has_z(),
                has_m: input.has_m(),
                fields: input.fields().to_vec(),
                spatial_reference_wkt: input.spatial_reference_wkt().map(str::to_owned),
                features,
                messages: vec![],
            }),
            None => WorkspaceOutput::Table(TableOutput {
                fields: input.fields().to_vec(),
                rows: features.into_iter().map(|f| f.attributes).collect(),
            }),
        };
        outputs.push((name, data));
    }
    Ok(outputs)
}

fn run(ctx: &dyn Context) -> Result<Report> {
    let input = ctx.features("input")?;
    let fields = ctx
        .get("split_fields")
        .ok_or_else(|| err("ERROR 000735: Split Fields: Value is required"))?;
    let workspace = ctx
        .get("target_workspace")
        .ok_or_else(|| err("ERROR 000735: Target Workspace: Value is required"))?;
    let outputs = partition(input.as_ref(), &fields)?;
    let count = outputs.len();
    ctx.write_workspace(&workspace, outputs)?;
    Ok(Report {
        rows: vec![("Datasets created".into(), count as f64)],
        messages: vec![],
    })
}

pub const TOOL: Tool = Tool {
    id: "split_by_attributes",
    name: "Split By Attributes",
    toolbox: "Analysis › Extract",
    summary: "Split features or table rows by unique text/integer attribute combinations into an existing folder.",
    params: &[
        Param {
            name: "input",
            label: "Input Table or Features",
            kind: Kind::Features,
            required: true,
            default: "",
            help: "",
        },
        Param {
            name: "target_workspace",
            label: "Target Workspace",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Existing folder; outputs receive unique names and existing datasets are preserved.",
        },
        Param {
            name: "split_fields",
            label: "Split Fields",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated text or32-bit integer fields.",
        },
    ],
    output_suffix: "Split",
    measures: false,
    run: Runner::Report(run),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Mem;
    use unigis_core::{Field, Geometry, GeometryKind};

    #[test]
    fn folder_names_match_arcgis_controls() {
        for (raw, expected) in [
            (".", "T_"),
            ("../path", "T___path"),
            ("_abc", "_abc"),
            ("-abc", "-abc"),
            ("  A ", "A"),
            ("a.b", "a_b"),
            ("a-b", "a-b"),
            ("", "T"),
            ("9abc", "9abc"),
            ("a$b", "a$b"),
            ("A/B", "A_B"),
            ("A\\B", "A_B"),
            ("x:y", "x_y"),
            ("a,b", "a,b"),
            ("a(1)", "a(1)"),
        ] {
            assert_eq!(dataset_name(raw).unwrap(), expected, "{raw}");
        }
        assert!(dataset_name(&"x".repeat(129)).is_err());
        assert!(dataset_name("NUL").is_err());
    }

    #[test]
    fn grouping_keeps_source_order_attributes_and_geometry() {
        let mut region = Field::new("REGION", FieldType::String);
        region.length = 32;
        let input = Mem(ToolOutput {
            kind: GeometryKind::Point,
            has_z: false,
            has_m: false,
            fields: vec![region],
            spatial_reference_wkt: Some("source wkt".into()),
            features: ["West", "East", "West", "East"]
                .iter()
                .enumerate()
                .map(|(i, v)| Feature {
                    oid: i as i64,
                    geometry: Some(Geometry::point(i as f64, 10.0)),
                    attributes: vec![Value::String((*v).into())],
                })
                .collect(),
            messages: vec![],
        });
        let parts = partition(&input, "REGION").unwrap();
        assert_eq!(
            parts.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["East", "West"]
        );
        let WorkspaceOutput::Features(east) = &parts[0].1 else {
            panic!("expected features")
        };
        assert_eq!(
            east.features.iter().map(|f| f.oid).collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(
            east.features[1].geometry.as_ref().unwrap().xy,
            vec![[3.0, 10.0]]
        );
        assert_eq!(east.fields[0].length, 32);
        assert_eq!(east.spatial_reference_wkt.as_deref(), Some("source wkt"));
    }

    #[test]
    fn host_without_workspace_support_fails_explicitly() {
        struct ReadOnly;
        impl Context for ReadOnly {
            fn get(&self, _: &str) -> Option<String> {
                None
            }
            fn features(&self, _: &str) -> Result<Box<dyn FeatureClass + '_>> {
                Err(err("no input"))
            }
        }
        let result = ReadOnly.write_workspace("ignored", vec![]);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("workspace outputs are not supported by this host")
        );
    }
}
