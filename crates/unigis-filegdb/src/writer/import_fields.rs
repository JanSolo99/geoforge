//! Opt-in standard geometry metadata, based on public ArcGIS import controls.
use super::subtype_edit::{Splice, append_child, apply, bad, child};
use crate::GdbField;
use unigis_core::{GeometryKind, Result};

pub(super) fn definition(
    xml: &str,
    kind: Option<GeometryKind>,
    fields: &[GdbField],
) -> Result<String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| bad(e.to_string()))?;
    let root = doc.root_element();
    let mut edits = Vec::new();
    if let Some(node) = child(root, "ShapeFieldName") {
        edits.push(Splice {
            at: node.range(),
            with: "<ShapeFieldName>Shape</ShapeFieldName>".into(),
        });
    }
    for (tag, name, active) in [
        (
            "LengthFieldName",
            "Shape_Length",
            matches!(kind, Some(GeometryKind::Polyline | GeometryKind::Polygon)),
        ),
        (
            "AreaFieldName",
            "Shape_Area",
            kind == Some(GeometryKind::Polygon),
        ),
    ] {
        if active && fields.iter().any(|f| f.name == name) {
            let node = child(root, tag).ok_or_else(|| bad("missing geometry measure metadata"))?;
            edits.push(Splice {
                at: node.range(),
                with: format!("<{tag}>{name}</{tag}>"),
            });
            if let Some(list) = child(root, "GPFieldInfoExs") {
                for node in list.children().filter(|n| n.is_element()) {
                    if child(node, "Name").and_then(|n| n.text()) == Some(name) {
                        edits.push(append_child(
                            xml,
                            node,
                            "<Required>true</Required><Editable>false</Editable>",
                        ));
                    }
                }
            }
        }
    }
    Ok(apply(xml, edits))
}
