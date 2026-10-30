//! Prevalidated feature-class batch import in one FileGDB transaction.
use super::{Location, ToolOutput, locate, open_feature_class};
use std::{collections::HashSet, path::Path};
use unigis_core::{Error, Field, FieldType, GeometryKind, Result, Value};
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};
fn bad(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}
fn name(raw: &str, reserved: &mut HashSet<String>, writer: &GeodatabaseWriter) -> Result<String> {
    if raw.is_empty()
        || !raw.is_ascii()
        || raw.chars().any(|c| c.is_ascii_control())
        || raw.len() > 150
    {
        return Err(bad(
            "Import names currently require1..150 printable ASCII characters",
        ));
    }
    let mut base: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !base.starts_with(|c: char| c.is_ascii_alphabetic()) {
        base.insert(0, 'T');
    }
    unigis_filegdb::writer::validate_table_name(&base)?;
    let mut candidate = base.clone();
    let mut suffix = 1;
    while writer.dataset_name_exists(&candidate) || !reserved.insert(candidate.to_ascii_lowercase())
    {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    Ok(candidate)
}
pub(super) fn run(inputs: &[String], target: &Path) -> Result<Vec<String>> {
    import(inputs, target, false)
}

/// Table To Geodatabase: tables (dBASE, CSV, geodatabase tables) and feature classes, into the
/// geodatabase itself (not a feature dataset).
pub(super) fn run_tables(inputs: &[String], target: &Path) -> Result<Vec<String>> {
    import(inputs, target, true)
}

fn import(inputs: &[String], target: &Path, tables: bool) -> Result<Vec<String>> {
    let invalid_output = || {
        bad(format!(
            "ERROR 000732: Output Geodatabase: Dataset {} does not exist or is not supported",
            target.display()
        ))
    };
    let Location::Geodatabase { gdb, item: dataset } = locate(target) else {
        return Err(invalid_output());
    };
    if !gdb.join("a00000001.gdbtable").is_file() {
        return Err(invalid_output());
    }
    // Reject arbitrary nesting that locate() would otherwise reduce to a basename.
    if target
        .strip_prefix(&gdb)
        .map_err(|_| invalid_output())?
        .components()
        .count()
        > 1
    {
        return Err(invalid_output());
    }
    let existing = Geodatabase::open(&gdb)?;
    let mut writer = GeodatabaseWriter::open_existing(&gdb)?;
    writer.use_standard_geometry_fields();
    if let Some(ds) = &dataset
        && (tables || writer.feature_dataset_srs(ds).is_none())
    {
        return Err(invalid_output());
    }
    let mut reserved: HashSet<String> = existing
        .entries()
        .iter()
        .map(|e| e.name.to_ascii_lowercase())
        .chain(
            existing
                .feature_datasets()
                .iter()
                .map(|n| n.to_ascii_lowercase()),
        )
        .chain(
            existing
                .relationship_classes()
                .iter()
                .map(|r| r.name.to_ascii_lowercase()),
        )
        .collect();
    let mut planned = Vec::new();
    for input in inputs {
        let path = Path::new(input);
        let source_gdb = matches!(locate(path), Location::Geodatabase { .. });
        let ext = |e: &str| path.extension().is_some_and(|x| x.eq_ignore_ascii_case(e));
        let label = if tables {
            "Input Table"
        } else {
            "Input Features"
        };
        let missing = || {
            bad(format!(
                "ERROR 000732: {label}: Dataset {input} does not exist or is not supported"
            ))
        };
        let csv = ext("csv") || ext("txt");
        if !source_gdb && !ext("shp") && !(tables && (ext("dbf") || csv)) {
            return Err(missing());
        }
        let (fc, _) = open_feature_class(path).map_err(|_| missing())?;
        // Table To Geodatabase writes every input as a table, feature classes too.
        let kind = if tables { None } else { fc.geometry_kind() };
        if kind.is_none() && !tables {
            return Err(bad("ERROR 000732: Input Features is not a feature class"));
        }
        if kind == Some(GeometryKind::MultiPatch) {
            return Err(bad("Multipatch feature imports are not yet supported"));
        }
        let raw = if source_gdb {
            path.file_name()
        } else {
            path.file_stem()
        }
        .and_then(|n| n.to_str())
        .ok_or_else(|| bad("input dataset name"))?;
        let output_name = name(raw, &mut reserved, &writer)?;
        let mut fields = fc.fields().to_vec();
        // These names can be ordinary user attributes on other geometry types;
        // do not let the import writer reinterpret and recompute their values.
        if kind.is_some()
            && fields.iter().any(|f| {
                (f.name.eq_ignore_ascii_case("Shape_Length")
                    && !matches!(kind, Some(GeometryKind::Polyline | GeometryKind::Polygon)))
                    || (f.name.eq_ignore_ascii_case("Shape_Area")
                        && kind != Some(GeometryKind::Polygon))
            })
        {
            return Err(bad(
                "Nonstandard geometry-measure field names are not yet supported",
            ));
        }

        for field in &mut fields {
            if matches!(
                field.field_type,
                FieldType::GlobalId | FieldType::Raster | FieldType::Blob | FieldType::Xml
            ) {
                return Err(bad(
                    "GlobalID, raster, blob and XML import fields are not yet supported",
                ));
            }
            if !source_gdb {
                field.nullable = true;
            }
            // ArcGIS reads CSV text as 8000 characters.
            if csv && field.field_type == FieldType::String {
                field.length = 8000;
            }
            field.domain = None;
            field.precision = 0;
            field.scale = 0;
            if field.alias.is_none() {
                field.alias = Some(field.name.clone());
            }
        }
        let names = super::geodatabase_field_names(&fields);
        for (field, name) in fields.iter_mut().zip(names) {
            field.name = name;
        }
        let mut features = fc.features().collect::<Result<Vec<_>>>()?;
        if kind.is_none() {
            for f in &mut features {
                f.geometry = None;
            }
        }
        for (field_name, active) in [
            (
                "Shape_Length",
                matches!(kind, Some(GeometryKind::Polyline | GeometryKind::Polygon)),
            ),
            ("Shape_Area", kind == Some(GeometryKind::Polygon)),
        ] {
            if !active {
                continue;
            }
            if let Some(f) = fields
                .iter_mut()
                .find(|f| f.name.eq_ignore_ascii_case(field_name))
            {
                if f.field_type != FieldType::Double {
                    return Err(bad("Incompatible derived geometry field"));
                }
                f.name = field_name.into();
                f.nullable = true;
            } else {
                let mut f = Field::new(field_name, FieldType::Double);
                f.alias = Some(field_name.into());
                fields.push(f);
                for row in &mut features {
                    row.attributes.push(Value::Null);
                }
            }
        }
        planned.push((
            output_name,
            kind.is_some(),
            ToolOutput {
                kind: kind.unwrap_or(GeometryKind::Point),
                has_z: fc.has_z(),
                has_m: fc.has_m(),
                fields,
                // ArcGIS writes its own WKT of the coordinate system (with its WKID), not
                // the input's text, as the other geodatabase outputs do.
                spatial_reference_wkt: fc.spatial_reference_wkt().map(|w| {
                    unigis_proj::Crs::from_wkt(w).map_or_else(|_| w.to_owned(), |c| c.arcgis_wkt())
                }),
                features,
                messages: vec![],
            },
        ));
    }
    // Writer's transaction rolls back all new classes if projection/encoding fails.
    let target_wkt = dataset
        .as_deref()
        .and_then(|d| writer.feature_dataset_srs(d))
        .flatten();
    let mut created = Vec::new();
    for (name, spatial, mut out) in planned {
        if let Some(wkt) = target_wkt.as_ref().filter(|_| spatial) {
            out = super::project_to(out, wkt)?;
        }
        let mut table = match &dataset {
            Some(ds) => writer.create_feature_class_in(
                ds,
                &name,
                out.kind,
                out.has_z,
                out.has_m,
                &out.fields,
            )?,
            None => writer.create_table(
                &name,
                spatial.then_some(out.kind),
                out.has_z,
                out.has_m,
                &out.fields,
                out.spatial_reference_wkt.as_deref(),
            )?,
        };
        for row in &out.features {
            table.write(row)?;
        }
        writer.finish_table(table)?;
        created.push(name);
    }
    writer.finish()?;
    Ok(created)
}
