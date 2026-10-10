//! Esri Shapefile reader and writer, implemented natively from the
//! *ESRI Shapefile Technical Description* (July 1998).

pub mod dbf;
pub mod encoding;
mod reader;
pub mod shape_type;
pub mod shp;
mod writer;

use std::path::{Path, PathBuf};

pub use reader::ShapefileReader;
pub use shape_type::ShapeType;
pub use writer::{MAX_FILE_BYTES, ShapefileWriter, validate_field_names};

/// Finds the companion file of `path` with extension `ext`, matching the extension
/// case-insensitively (`.SHP`, `.Dbf`, ...).
pub(crate) fn sibling(path: &Path, ext: &str) -> Option<PathBuf> {
    let base = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("shp"))
    {
        path.with_extension("")
    } else {
        path.to_path_buf()
    };
    let exact = base.with_extension(ext);
    if exact.is_file() {
        return Some(exact);
    }
    let dir = base
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stem = base.file_name()?;
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_stem() == Some(stem)
                && p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
        })
}
