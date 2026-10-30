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

/// What a file geodatabase holds, for a catalog view: its feature datasets, and its tables and
/// feature classes with the feature dataset each is in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeodatabaseContents {
    pub feature_datasets: Vec<String>,
    /// `(feature dataset, name, is a feature class)`.
    pub items: Vec<(Option<String>, String, bool)>,
}

/// The contents of the file geodatabase at `gdb`.
pub fn geodatabase_contents(gdb: &Path) -> Result<GeodatabaseContents> {
    imp::geodatabase_contents(gdb)
}

/// Writes a tool's features as a new dataset on disk: a feature class in a file geodatabase
/// (`city.gdb/name`, `city.gdb/Dataset/name`, which takes the dataset's coordinate system) or a
/// shapefile (`.shp`). Laid out as ArcGIS creates them (`Shape`, Shape_Length / Shape_Area).
pub fn write_features(path: &Path, out: crate::ToolOutput) -> Result<()> {
    imp::write_features(path, out)
}

/// Writes a table as a new geodatabase table (`city.gdb/name`).
pub fn write_table(path: &Path, table: crate::TableOutput) -> Result<()> {
    imp::write_table(path, table)
}

/// Whether `path` names a dataset on disk that [`write_features`] can create: inside a file
/// geodatabase, or a `.shp` in an existing folder.
pub fn writable_path(path: &Path) -> bool {
    if path
        .ancestors()
        .skip(1)
        .any(|p| p.join("a00000001.gdbtable").is_file())
    {
        return true;
    }
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("shp"))
        && path.parent().is_some_and(Path::is_dir)
}

#[cfg(feature = "gdb")]
mod imp {
    use super::*;
    use crate::{TableOutput, ToolOutput};
    use unigis_core::{Feature, Field, FieldType, GeometryKind, Value};
    use unigis_filegdb::{Geodatabase, GeodatabaseWriter};

    pub fn geodatabase_contents(gdb: &Path) -> Result<GeodatabaseContents> {
        let g = Geodatabase::open(gdb)?;
        let mut items = Vec::new();
        for e in g.entries() {
            let spatial = g
                .open_table(&e.name)
                .is_ok_and(|t| t.shape_field().is_some());
            items.push((e.feature_dataset.clone(), e.name.clone(), spatial));
        }
        Ok(GeodatabaseContents {
            feature_datasets: g.feature_datasets().to_vec(),
            items,
        })
    }

    /// The geodatabase, feature dataset and name `path` names.
    fn in_geodatabase(path: &Path) -> Option<(PathBuf, Option<String>, String)> {
        let gdb = path
            .ancestors()
            .skip(1)
            .find(|p| p.join("a00000001.gdbtable").is_file())?;
        let rest: Vec<String> = path
            .strip_prefix(gdb)
            .ok()?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        match rest.as_slice() {
            [name] => Some((gdb.to_path_buf(), None, name.clone())),
            [ds, name] => Some((gdb.to_path_buf(), Some(ds.clone()), name.clone())),
            _ => None,
        }
    }

    pub fn write_features(path: &Path, mut out: ToolOutput) -> Result<()> {
        let Some((gdb, dataset, name)) = in_geodatabase(path) else {
            return write_shapefile(path, out);
        };
        let mut w = GeodatabaseWriter::open_existing(&gdb)?;
        let own_measures = out.fields.iter().any(|f| {
            f.name.eq_ignore_ascii_case("Shape_Length") || f.name.eq_ignore_ascii_case("Shape_Area")
        });
        if !own_measures {
            for (measure, wanted) in [
                (
                    "Shape_Length",
                    matches!(out.kind, GeometryKind::Polyline | GeometryKind::Polygon),
                ),
                ("Shape_Area", out.kind == GeometryKind::Polygon),
            ] {
                if wanted {
                    out.fields.push(Field::new(measure, FieldType::Double));
                    for f in &mut out.features {
                        f.attributes.push(Value::Null);
                    }
                }
            }
            w.use_standard_geometry_fields();
        }
        let mut t = match &dataset {
            Some(ds) => {
                let target = w
                    .feature_dataset_srs(ds)
                    .ok_or_else(|| err(format!("ERROR 000732: Dataset {ds} does not exist")))?;
                if !out.features.is_empty()
                    && target.is_some()
                    && out.spatial_reference_wkt.is_some()
                    && target != out.spatial_reference_wkt
                {
                    return Err(err(
                        "features in another coordinate system than the feature dataset's are not projected here yet",
                    ));
                }
                w.create_feature_class_in(ds, &name, out.kind, out.has_z, out.has_m, &out.fields)?
            }
            None => w.create_table(
                &name,
                Some(out.kind),
                out.has_z,
                out.has_m,
                &out.fields,
                out.spatial_reference_wkt.as_deref(),
            )?,
        };
        for f in &out.features {
            t.write(f)?;
        }
        w.finish_table(t)?;
        w.finish()
    }

    fn write_shapefile(path: &Path, out: ToolOutput) -> Result<()> {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("shp"))
        {
            return Err(err(format!(
                "{}: name a .shp file or a dataset inside a file geodatabase",
                path.display()
            )));
        }
        if path.exists() {
            return Err(err(format!(
                "ERROR 000258: Output {} already exists",
                path.display()
            )));
        }
        let mut fields = out.fields.clone();
        // A .dbf needs a field; ArcGIS's default for a new shapefile is "Id".
        let placeholder = fields.is_empty();
        if placeholder {
            let mut id = Field::new("Id", FieldType::Integer);
            id.precision = 6;
            fields.push(id);
        }
        let mut w = unigis_shapefile::ShapefileWriter::create(
            path,
            out.kind,
            out.has_z,
            out.has_m,
            &fields,
            out.spatial_reference_wkt.as_deref(),
        )?;
        for f in out.features {
            let f = if placeholder {
                Feature {
                    attributes: vec![Value::Integer(0)],
                    ..f
                }
            } else {
                f
            };
            w.write(&f)?;
        }
        w.finish()
    }

    pub fn write_table(path: &Path, table: TableOutput) -> Result<()> {
        let (gdb, None, name) = in_geodatabase(path).ok_or_else(|| {
            err(format!(
                "{}: name a table inside a file geodatabase",
                path.display()
            ))
        })?
        else {
            return Err(err(
                "tables go in the geodatabase, not in a feature dataset",
            ));
        };
        let mut w = GeodatabaseWriter::open_existing(&gdb)?;
        let mut t = w.create_table(&name, None, false, false, &table.fields, None)?;
        for (i, row) in table.rows.iter().enumerate() {
            t.write(&Feature {
                oid: i as i64 + 1,
                geometry: None,
                attributes: row.clone(),
            })?;
        }
        w.finish_table(t)?;
        w.finish()
    }

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

    pub fn geodatabase_contents(_: &Path) -> Result<GeodatabaseContents> {
        Err(unsupported())
    }

    pub fn write_features(_: &Path, _: crate::ToolOutput) -> Result<()> {
        Err(unsupported())
    }

    pub fn write_table(_: &Path, _: crate::TableOutput) -> Result<()> {
        Err(unsupported())
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
    output_suffix: "",
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
    output_suffix: "",
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
    output_suffix: "",
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
