//! Coordinated writes of new training/test datasets. Folder outputs are staged
//! before exclusive publication; a FileGDB uses one rollback-journal transaction.
use super::{Location, locate};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use unigis_core::{Error, Feature, Field, Result};
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};
use unigis_tools::registry::WorkspaceOutput;
fn bad(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch(parent: &Path) -> Result<Scratch> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..100 {
        let path = parent.join(format!(
            ".ug-datasets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(Scratch(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(Error::io(path, e)),
        }
    }
    Err(bad("cannot reserve staging directory"))
}
fn fields(data: &WorkspaceOutput) -> &[Field] {
    match data {
        WorkspaceOutput::Features(out) => &out.fields,
        WorkspaceOutput::Table(out) => &out.fields,
    }
}
pub(super) fn write(source: Option<&Path>, outputs: Vec<(String, WorkspaceOutput)>) -> Result<()> {
    let mut workspace = None;
    let mut names = HashSet::new();
    let mut planned = Vec::new();
    let mut is_gdb = None;
    for (raw, data) in outputs {
        let path = PathBuf::from(raw);
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = std::fs::canonicalize(parent).map_err(|e| Error::io(parent, e))?;
        if workspace.as_ref().is_some_and(|p| p != &parent) {
            return Err(bad(
                "training and test outputs must share one folder or FileGDB",
            ));
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| bad("output dataset name is required"))?;
        let path = parent.join(name);
        let gdb = match locate(&path) {
            Location::Geodatabase {
                gdb,
                item: Some(item),
            } => {
                if std::fs::canonicalize(gdb).map_err(|e| Error::io(&parent, e))? != parent
                    || item.contains(['/', '\\'])
                {
                    return Err(bad("feature-dataset output paths are not supported"));
                }
                unigis_filegdb::writer::validate_table_name(&item)?;
                true
            }
            Location::Geodatabase { .. } => {
                return Err(bad("name the output dataset inside the FileGDB"));
            }
            _ => false,
        };
        if is_gdb.is_some_and(|v| v != gdb) {
            return Err(bad("training/test output formats must agree"));
        }
        let stem = if gdb {
            name.to_owned()
        } else {
            let expected = match data {
                WorkspaceOutput::Features(_) => "shp",
                WorkspaceOutput::Table(_) => "dbf",
            };
            if !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(expected))
            {
                return Err(bad(format!("this output must end in .{expected}")));
            }
            path.file_stem()
                .and_then(|n| n.to_str())
                .ok_or_else(|| bad("output dataset name is required"))?
                .to_owned()
        };
        if !names.insert(stem.to_ascii_lowercase()) {
            return Err(bad(
                "ERROR 000258: Training and test outputs must be distinct",
            ));
        }
        workspace = Some(parent);
        is_gdb = Some(gdb);
        planned.push((path, data));
    }
    let Some(workspace) = workspace else {
        return Err(bad("no output datasets requested"));
    };
    if is_gdb == Some(true) {
        return geodatabase(source, &workspace, planned);
    }
    for entry in std::fs::read_dir(&workspace).map_err(|e| Error::io(&workspace, e))? {
        let entry = entry.map_err(|e| Error::io(&workspace, e))?;
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // Also reserve multi-suffix sidecars such as name.shp.xml.
        let file = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if names.contains(&stem) || names.iter().any(|n| file.starts_with(&format!("{n}."))) {
            return Err(bad(format!(
                "ERROR 000258: Output dataset already exists: {}",
                path.display()
            )));
        }
    }
    let staging = scratch(&workspace)?;
    let mut publication = Vec::new();
    for (i, (path, data)) in planned.into_iter().enumerate() {
        let ext = match &data {
            WorkspaceOutput::Features(_) => "shp",
            WorkspaceOutput::Table(_) => "dbf",
        };
        let prefix = format!("part{i}");
        let stage = staging.0.join(format!("{prefix}.{ext}"));
        match data {
            WorkspaceOutput::Features(out) => {
                let expected = out.features.len();
                if super::write_shapefile(out, &stage)? != expected {
                    return Err(bad(
                        "subset output format could not represent every selected record",
                    ));
                }
            }
            WorkspaceOutput::Table(out) => super::write_table(&out, &stage)?,
        }
        for entry in std::fs::read_dir(&staging.0).map_err(|e| Error::io(&staging.0, e))? {
            let entry = entry.map_err(|e| Error::io(&staging.0, e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(suffix) = name.strip_prefix(&prefix).filter(|s| s.starts_with('.')) {
                let target = if suffix == format!(".{ext}") {
                    path.clone()
                } else {
                    path.with_file_name(format!(
                        "{}{}",
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .ok_or_else(|| bad("output name"))?,
                        suffix
                    ))
                };
                publication.push((entry.path(), target));
            }
        }
    }
    let mut created = Vec::new();
    for (stage, target) in publication {
        if let Err(e) = std::fs::hard_link(&stage, &target) {
            for path in created {
                let _ = std::fs::remove_file(path);
            }
            return Err(Error::io(target, e));
        }
        created.push(target);
    }
    Ok(())
}
fn geodatabase(
    source: Option<&Path>,
    path: &Path,
    outputs: Vec<(PathBuf, WorkspaceOutput)>,
) -> Result<()> {
    let target = Geodatabase::open(path)?;
    for (out, _) in &outputs {
        let name = out
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| bad("output name"))?;
        if target
            .entries()
            .iter()
            .any(|e| e.name.eq_ignore_ascii_case(name))
            || target
                .feature_datasets()
                .iter()
                .any(|s| s.eq_ignore_ascii_case(name))
            || target
                .relationship_classes()
                .iter()
                .any(|r| r.name.eq_ignore_ascii_case(name))
        {
            return Err(bad(format!("ERROR 000258: Output {name} already exists")));
        }
    }
    let source_db = source
        .and_then(super::source_gdb)
        .map(Geodatabase::open)
        .transpose()?;
    let mut domains = Vec::new();
    for (_, data) in &outputs {
        for name in fields(data).iter().filter_map(|f| f.domain.as_deref()) {
            let domain = source_db
                .as_ref()
                .and_then(|s| s.domain(name))
                .ok_or_else(|| bad(format!("source domain {name} is unavailable")))?;
            if target
                .domain(name)
                .is_some_and(|existing| existing != domain)
            {
                return Err(bad(format!("incompatible destination domain {name}")));
            }
            if target.domain(name).is_none()
                && !domains
                    .iter()
                    .any(|d: &unigis_filegdb::Domain| d.name.eq_ignore_ascii_case(name))
            {
                domains.push(domain.clone());
            }
        }
    }
    let mut writer = GeodatabaseWriter::open_existing(path)?;
    for domain in domains {
        writer.add_domain(&domain)?;
    }
    for (out, data) in outputs {
        let name = out
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| bad("output name"))?;
        let original = fields(&data);
        let names = super::geodatabase_field_names(original);
        let schema: Vec<Field> = original
            .iter()
            .zip(names)
            .map(|(f, name)| Field { name, ..f.clone() })
            .collect();
        let (kind, z, m, wkt) = match &data {
            WorkspaceOutput::Features(out) => (
                Some(out.kind),
                out.has_z,
                out.has_m,
                out.spatial_reference_wkt.as_deref(),
            ),
            WorkspaceOutput::Table(_) => (None, false, false, None),
        };
        let mut table = writer.create_table(name, kind, z, m, &schema, wkt)?;
        match data {
            WorkspaceOutput::Features(out) => {
                for f in out.features {
                    table.write(&f)?;
                }
            }
            WorkspaceOutput::Table(out) => {
                for row in out.rows {
                    table.write(&Feature {
                        oid: 0,
                        geometry: None,
                        attributes: row,
                    })?;
                }
            }
        }
        writer.finish_table(table)?;
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::{FieldType, Geometry, GeometryKind, Value};
    fn temp(label: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("ug-dataset-write-{}-{label}", std::process::id()));
        std::fs::create_dir(&p).unwrap();
        p
    }
    fn snapshot(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect()
    }
    #[test]
    fn folder_failure_before_publication_leaves_no_partial_datasets() {
        let dir = temp("folder");
        let make = |ty| {
            WorkspaceOutput::Features(unigis_tools::ToolOutput {
                kind: GeometryKind::Point,
                has_z: false,
                has_m: false,
                fields: vec![Field::new("VALUE", ty)],
                spatial_reference_wkt: None,
                features: vec![Feature {
                    oid: 0,
                    geometry: Some(Geometry::point(1.0, 2.0)),
                    attributes: vec![Value::Integer(1)],
                }],
                messages: vec![],
            })
        };
        let result = write(
            None,
            vec![
                (
                    dir.join("first.shp").display().to_string(),
                    make(FieldType::Integer),
                ),
                (
                    dir.join("second.shp").display().to_string(),
                    make(FieldType::BigInteger),
                ),
            ],
        );
        assert!(result.is_err());
        assert!(snapshot(&dir).is_empty());
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn filegdb_mid_write_failure_rolls_back_all_new_tables() {
        let dir = temp("gdb");
        let gdb = dir.join("target.gdb");
        GeodatabaseWriter::create(&gdb).unwrap().finish().unwrap();
        let before = snapshot(&gdb);
        let make = |ty| {
            WorkspaceOutput::Table(unigis_tools::TableOutput {
                fields: vec![Field::new("VALUE", ty)],
                rows: vec![vec![Value::Integer(1)]],
            })
        };
        let result = write(
            None,
            vec![
                (
                    gdb.join("first").display().to_string(),
                    make(FieldType::Integer),
                ),
                (
                    gdb.join("second").display().to_string(),
                    make(FieldType::Geometry),
                ),
            ],
        );
        assert!(result.is_err());
        assert_eq!(snapshot(&gdb), before);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
