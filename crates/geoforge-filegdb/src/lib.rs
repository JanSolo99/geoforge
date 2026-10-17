//! Esri File Geodatabase (`.gdb`) reader, implemented from the public
//! reverse-engineered specification (see `docs/formats/filegdb.md`).

mod bytes;
pub mod gdb;
pub mod geometry;
pub mod index;
pub mod schema;
pub mod table;
pub mod writer;
pub mod xy_domains;

pub use gdb::{CatalogEntry, Geodatabase};
pub use index::IndexDef;
pub use schema::{
    Cardinality, Domain, DomainKind, MergePolicy, RelationshipClass, SplitPolicy, Subtype,
    SubtypeField, Subtypes,
};
pub use table::{GdbField, GdbFieldType, GeometryDef, Table, compare_values};
pub use writer::{
    GeodatabaseWriter, TableBuilder, TableEditor, add_subtype, assign_default_to_field,
    assign_domain_to_field, delete_domain, delete_item, rebuild_attribute_indexes,
    remove_domain_from_field, remove_subtype, rename_domain, rename_item, rewrite_table,
    set_default_subtype, set_subtype_field,
};
