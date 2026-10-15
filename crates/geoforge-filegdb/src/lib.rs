//! Esri File Geodatabase (`.gdb`) reader, implemented from the public
//! reverse-engineered specification (see `docs/formats/filegdb.md`).

mod bytes;
pub mod gdb;
pub mod geometry;
pub mod index;
pub mod schema;
pub mod table;
pub mod writer;

pub use gdb::{CatalogEntry, Geodatabase};
pub use index::IndexDef;
pub use schema::{
    Cardinality, Domain, DomainKind, MergePolicy, RelationshipClass, SplitPolicy, Subtype,
    SubtypeField, Subtypes,
};
pub use table::{GdbField, GdbFieldType, GeometryDef, Table, compare_values};
pub use writer::{GeodatabaseWriter, TableBuilder, rebuild_attribute_indexes};
