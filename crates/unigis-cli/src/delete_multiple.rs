//! Host implementation for explicitly named Delete Multiple inputs.
use super::{Location, locate, shapefile_files};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use unigis_core::{Error, Result};
use unigis_filegdb::Geodatabase;
use unigis_tools::delete_multiple::DeleteItem;

enum Plan {
    Files(Vec<PathBuf>),
    Item(PathBuf, String),
    Workspace(PathBuf),
}
fn missing(path: &Path) -> Error {
    Error::Unsupported(format!(
        "ERROR 000732: Input Data Element: Dataset {} does not exist or is not supported",
        path.display()
    ))
}
fn locked(path: &Path) -> Result<()> {
    let dir = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(Path::new("."))
    };
    if dir.is_dir()
        && std::fs::read_dir(dir)
            .map_err(|e| Error::io(dir, e))?
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().ends_with(".lock"))
    {
        return Err(Error::Unsupported(format!(
            "ERROR 000464: Cannot acquire exclusive schema lock: {}",
            path.display()
        )));
    }
    Ok(())
}
fn plan(path: &Path) -> Result<(String, Plan)> {
    if let Location::Geodatabase { gdb, item } = locate(path) {
        if !gdb.join("a00000001.gdbtable").is_file() {
            return Err(missing(path));
        }
        if std::fs::symlink_metadata(&gdb)
            .map_err(|e| Error::io(&gdb, e))?
            .file_type()
            .is_symlink()
        {
            return Err(Error::Unsupported(
                "symlink workspaces are not supported".into(),
            ));
        }
        locked(&gdb)?;
        let opened = Geodatabase::open(&gdb)?;
        let canonical = std::fs::canonicalize(&gdb).map_err(|e| Error::io(&gdb, e))?;
        if let Some(item) = item {
            let relative = path.strip_prefix(&gdb).map_err(|_| missing(path))?;
            let parts: Vec<_> = relative.components().collect();
            if parts.iter().any(|c| !matches!(c, Component::Normal(_))) || parts.len() > 2 {
                return Err(missing(path));
            }
            let table = opened
                .entries()
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(&item));
            let exists = if parts.len() == 2 {
                table.is_some_and(|e| {
                    e.feature_dataset.as_ref().is_some_and(|d| {
                        d.eq_ignore_ascii_case(&parts[0].as_os_str().to_string_lossy())
                    })
                })
            } else {
                table.is_some()
                    || opened
                        .feature_datasets()
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&item))
                    || opened
                        .relationship_classes()
                        .iter()
                        .any(|r| r.name.eq_ignore_ascii_case(&item))
            };
            if !exists {
                return Err(missing(path));
            }
            // Type hints disambiguate colliding kinds; the native catalog does not
            // expose a way to select such a collision, so refuse rather than guess.
            let matches = usize::from(table.is_some())
                + usize::from(
                    opened
                        .feature_datasets()
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&item)),
                )
                + usize::from(
                    opened
                        .relationship_classes()
                        .iter()
                        .any(|r| r.name.eq_ignore_ascii_case(&item)),
                );
            if matches > 1 {
                return Err(Error::Unsupported(
                    "ambiguous geodatabase item kinds are not supported".into(),
                ));
            }
            return Ok((
                format!("{}/{}", canonical.display(), item.to_lowercase()),
                Plan::Item(gdb, item),
            ));
        }
        return Ok((canonical.display().to_string(), Plan::Workspace(gdb)));
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| missing(path))?;
    if !metadata.file_type().is_file() {
        return Err(Error::Unsupported("ordinary folders and symlinks are not supported by Delete Multiple (folder recycling is unavailable)".into()));
    }
    let ext = path
        .extension()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let shape = if ext == "dbf" && path.with_extension("shp").is_file() {
        path.with_extension("shp")
    } else {
        path.to_path_buf()
    };
    let canonical = std::fs::canonicalize(&shape).map_err(|e| Error::io(&shape, e))?;
    let mut files = if ext == "shp" || ext == "dbf" {
        shapefile_files(&shape)
    } else {
        vec![shape.clone()]
    };
    if ext == "dbf"
        && !shape
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("shp"))
    {
        files.retain(|p| {
            p.extension().is_some_and(|e| {
                e.eq_ignore_ascii_case("dbf")
                    || e.eq_ignore_ascii_case("cpg")
                    || e.eq_ignore_ascii_case("atx")
            }) || p.to_string_lossy().ends_with(".dbf.xml")
        });
    }
    // Field-specific .atx indexes use <stem>.<field>.atx names.
    if ext == "shp" || ext == "dbf" {
        let dir = shape
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let prefix = format!(
            "{}.",
            shape.file_stem().unwrap().to_string_lossy().to_lowercase()
        );
        let fields: Vec<String> = super::open_feature_class(&shape)
            .map(|(fc, _)| fc.fields().iter().map(|f| f.name.to_lowercase()).collect())
            .unwrap_or_default();
        for entry in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
            let p = entry.map_err(|e| Error::io(dir, e))?.path();
            let n = p.file_name().unwrap().to_string_lossy().to_lowercase();
            if n.strip_prefix(&prefix)
                .and_then(|r| r.strip_suffix(".atx"))
                .is_some_and(|field| fields.iter().any(|f| f == field))
            {
                files.push(p);
            }
        }
        if files.iter().any(|p| p.to_string_lossy().ends_with(".lock")) {
            return Err(Error::Unsupported(
                "ERROR 000464: Cannot acquire exclusive schema lock".into(),
            ));
        }
    }
    if ext == "tif" || ext == "tiff" {
        for suffix in [".aux.xml", ".vat.cpg", ".vat.dbf", ".xml"] {
            let candidate = PathBuf::from(format!("{}{suffix}", path.display()));
            if candidate.exists() {
                files.push(candidate);
            }
        }
        let world = path.with_extension("tfw");
        if world.exists() {
            files.push(world);
        }
    }
    files.sort();
    files.dedup();
    for file in &files {
        if !std::fs::symlink_metadata(file)
            .map_err(|e| Error::io(file, e))?
            .file_type()
            .is_file()
        {
            return Err(Error::Unsupported(
                "non-file ancillary data is not supported".into(),
            ));
        }
    }
    Ok((canonical.display().to_string(), Plan::Files(files)))
}

pub fn run(items: &[DeleteItem]) -> Result<()> {
    let mut plans = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let (key, p) = plan(Path::new(&item.path))?;
        if seen.insert(key) {
            plans.push(p);
        }
    }
    // Every initial input has now been validated; duplicates/previous cascades
    // are successful no-ops. An I/O failure after this point can leave earlier
    // datasets deleted, as this is not a transaction across files/workspaces.
    for p in plans {
        match p {
            Plan::Files(files) => {
                for f in files {
                    if f.exists() {
                        std::fs::remove_file(&f).map_err(|e| Error::io(&f, e))?;
                    }
                }
            }
            Plan::Workspace(gdb) => {
                if gdb.exists() {
                    std::fs::remove_dir_all(&gdb).map_err(|e| Error::io(&gdb, e))?;
                }
            }
            Plan::Item(gdb, item) => {
                if gdb.exists() {
                    let opened = Geodatabase::open(&gdb)?;
                    let exists = opened
                        .entries()
                        .iter()
                        .any(|e| e.name.eq_ignore_ascii_case(&item))
                        || opened
                            .feature_datasets()
                            .iter()
                            .any(|n| n.eq_ignore_ascii_case(&item))
                        || opened
                            .relationship_classes()
                            .iter()
                            .any(|r| r.name.eq_ignore_ascii_case(&item));
                    drop(opened);
                    if exists {
                        unigis_filegdb::delete_item(&gdb, &item)?;
                    }
                }
            }
        }
    }
    Ok(())
}
