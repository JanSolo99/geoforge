//! Data Management › Workspace: Create Folder, Create File Geodatabase and Create Mobile
//! Geodatabase. Each writes on disk and reports ArcGIS's derived output (`out_folder: <path>`).
//!
//! As ArcGIS Pro 3.7 behaves: the location must be an existing folder (000732) outside a
//! geodatabase (000792); a geodatabase name loses any extension of its own and gets `.gdb` or
//! `.geodatabase`; an existing geodatabase is an error (000258), an existing folder is not;
//! names with characters a file name cannot hold fail (999999).

use std::path::{Path, PathBuf};

use unigis_core::{Error, Result};

use crate::registry::{Context, Kind, Param, Report, Runner, Tool};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

const fn text(name: &'static str, label: &'static str, help: &'static str) -> Param {
    Param {
        name,
        label,
        kind: Kind::Text,
        required: true,
        default: "",
        help,
    }
}

/// The folder a new item goes into, checked as ArcGIS checks it.
fn location(ctx: &dyn Context, label: &str) -> Result<PathBuf> {
    let text = ctx
        .get("out_folder_path")
        .ok_or_else(|| err(format!("ERROR 000735: {label}: Value is required")))?;
    let path = PathBuf::from(&text);
    if !path.is_dir() {
        return Err(err(format!(
            "ERROR 000732: {label}: Dataset {text} does not exist or is not supported"
        )));
    }
    if is_geodatabase(&path) {
        return Err(err(format!(
            "ERROR 000792: Cannot write output to target folder {text}."
        )));
    }
    Ok(path)
}

fn is_geodatabase(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gdb") || e.eq_ignore_ascii_case("geodatabase"))
}

/// The new item's name: required, and a valid file name.
fn name(ctx: &dyn Context, label: &str) -> Result<String> {
    let name = ctx
        .get("out_name")
        .ok_or_else(|| err(format!("ERROR 000735: {label}: Value is required")))?;
    if name
        .chars()
        .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c < ' ')
        || name == "."
        || name == ".."
    {
        return Err(err(
            "ERROR 999999: Something unexpected caused the tool to fail.",
        ));
    }
    Ok(name)
}

/// `name` with its own extension (after the last dot) replaced by `ext`: `g8.txt` → `g8.gdb`.
fn with_extension(name: &str, ext: &str) -> String {
    let stem = match name.rfind('.') {
        Some(k) if k > 0 => &name[..k],
        _ => name,
    };
    format!("{stem}.{ext}")
}

fn derived(key: &str, path: &Path) -> Report {
    Report {
        rows: vec![],
        messages: vec![format!("{key}: {}", path.display())],
    }
}

fn create_folder(ctx: &dyn Context) -> Result<Report> {
    let dir = location(ctx, "Folder Location")?.join(name(ctx, "Folder Name")?);
    if dir.exists() && !dir.is_dir() {
        return Err(err(
            "ERROR 999999: Something unexpected caused the tool to fail.",
        ));
    }
    if !dir.is_dir() {
        std::fs::create_dir(&dir).map_err(|e| Error::io(&dir, e))?;
    }
    Ok(derived("out_folder", &dir))
}

fn create_file_gdb(ctx: &dyn Context) -> Result<Report> {
    let dir = location(ctx, "File Geodatabase Location")?;
    let gdb = dir.join(with_extension(&name(ctx, "File GDB Name")?, "gdb"));
    let version = ctx.get("out_version").unwrap_or_else(|| "CURRENT".into());
    match version.to_ascii_uppercase().as_str() {
        "CURRENT" | "10.0" => {}
        "9.3" | "9.2" => {
            return Err(err(format!(
                "File geodatabase version {version} (the 9.x layout) is not written yet; use CURRENT or 10.0"
            )));
        }
        _ => {
            return Err(err(
                "ERROR 000800: The value is not a member of CURRENT | 10.0 | 9.3 | 9.2.",
            ));
        }
    }
    if gdb.exists() {
        return Err(err(format!(
            "ERROR 000258: Output {} already exists",
            gdb.display()
        )));
    }
    imp::create_file_gdb(&gdb)?;
    Ok(derived("out_file_gdb", &gdb))
}

fn create_mobile_gdb(ctx: &dyn Context) -> Result<Report> {
    let dir = location(ctx, "Mobile Geodatabase Location")?;
    let gdb = dir.join(with_extension(
        &name(ctx, "Mobile Geodatabase Name")?,
        "geodatabase",
    ));
    if gdb.exists() {
        return Err(err(format!(
            "ERROR 000258: Output {} already exists",
            gdb.display()
        )));
    }
    imp::create_mobile_gdb(&gdb)?;
    Ok(derived("out_gdb", &gdb))
}

#[cfg(feature = "gdb")]
mod imp {
    use super::*;

    pub fn create_file_gdb(gdb: &Path) -> Result<()> {
        unigis_filegdb::GeodatabaseWriter::create(gdb)?.finish()
    }

    pub fn create_mobile_gdb(gdb: &Path) -> Result<()> {
        unigis_gpkg::mobile::create(gdb)
    }
}

/// Without the `gdb` feature (the browser build) nothing is written.
#[cfg(not(feature = "gdb"))]
mod imp {
    use super::*;

    fn unsupported() -> Error {
        err("geodatabases can't be created in this build")
    }

    pub fn create_file_gdb(_: &Path) -> Result<()> {
        Err(unsupported())
    }

    pub fn create_mobile_gdb(_: &Path) -> Result<()> {
        Err(unsupported())
    }
}

pub const CREATE_FOLDER: Tool = Tool {
    id: "create_folder",
    name: "Create Folder",
    toolbox: "Data Management › Workspace",
    summary: "A new folder in an existing folder (ArcGIS Pro Create Folder).",
    params: &[
        text(
            "out_folder_path",
            "Folder Location",
            "The folder to create it in.",
        ),
        text("out_name", "Folder Name", ""),
    ],
    output_suffix: "Folder",
    measures: false,
    run: Runner::Report(create_folder),
};

pub const CREATE_FILE_GDB: Tool = Tool {
    id: "create_file_gdb",
    name: "Create File Geodatabase",
    toolbox: "Data Management › Workspace",
    summary: "An empty file geodatabase in a folder (ArcGIS Pro Create File Geodatabase).",
    params: &[
        text(
            "out_folder_path",
            "File Geodatabase Location",
            "The folder to create it in.",
        ),
        text("out_name", "File GDB Name", "`.gdb` is added."),
        Param {
            name: "out_version",
            label: "File Geodatabase Version",
            kind: Kind::Choice(&["CURRENT", "10.0", "9.3", "9.2"]),
            required: false,
            default: "CURRENT",
            help: "The ArcGIS release whose geodatabase layout to write.",
        },
    ],
    output_suffix: "gdb",
    measures: false,
    run: Runner::Report(create_file_gdb),
};

pub const CREATE_MOBILE_GDB: Tool = Tool {
    id: "create_mobile_gdb",
    name: "Create Mobile Geodatabase",
    toolbox: "Data Management › Workspace",
    summary: "An empty mobile geodatabase (.geodatabase, SQLite) in a folder (ArcGIS Pro Create Mobile Geodatabase).",
    params: &[
        text(
            "out_folder_path",
            "Mobile Geodatabase Location",
            "The folder to create it in.",
        ),
        text(
            "out_name",
            "Mobile Geodatabase Name",
            "`.geodatabase` is added.",
        ),
    ],
    output_suffix: "geodatabase",
    measures: false,
    run: Runner::Report(create_mobile_gdb),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_their_own_extension() {
        assert_eq!(with_extension("g1", "gdb"), "g1.gdb");
        assert_eq!(with_extension("g2.gdb", "gdb"), "g2.gdb");
        assert_eq!(with_extension("g7.GDB", "gdb"), "g7.gdb");
        assert_eq!(with_extension("g8.txt", "gdb"), "g8.gdb");
        assert_eq!(with_extension("dot.", "gdb"), "dot.gdb");
        assert_eq!(with_extension("m3.sqlite", "geodatabase"), "m3.geodatabase");
    }
}
