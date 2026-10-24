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
    FieldAlteration, GeodatabaseWriter, TableBuilder, TableEditor, add_field, add_subtype,
    alter_field, assign_default_to_field, assign_domain_to_field, delete_domain, delete_fields,
    delete_item, delete_rows, geodatabase_field_name, rebuild_attribute_indexes, recover,
    remove_domain_from_field, remove_subtype, rename_domain, rename_item, rewrite_table,
    set_default_subtype, set_subtype_field, truncate_table,
};
