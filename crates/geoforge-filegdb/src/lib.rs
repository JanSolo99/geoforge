//! Esri File Geodatabase (`.gdb`) reader, implemented from the public
//! reverse-engineered specification (see `docs/formats/filegdb.md`).

mod bytes;
pub mod gdb;
pub mod geometry;
pub mod table;

pub use gdb::{CatalogEntry, Geodatabase};
pub use table::{GdbField, GdbFieldType, GeometryDef, Table};
