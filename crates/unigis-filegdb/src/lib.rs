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
    Expand, ExtractOptions, FieldAlteration, GeodatabaseWriter, TableBuilder, TableEditor,
    add_attribute_index, add_coded_value, add_field, add_schema_fields, add_subtype,
    alter_coded_value_description, alter_domain, alter_field, alter_fields,
    assign_default_to_field, assign_domain_to_field, create_domain, delete_coded_values,
    delete_domain, delete_fields, delete_item, delete_rows, disable_cogo, disable_editor_tracking,
    domain_rows, enable_cogo, enable_editor_tracking, extract_data, geodatabase_field_name,
    migrate_date_fields_to_high_precision, parse_domain_value, rebuild_attribute_indexes, recover,
    remove_attribute_index, remove_domain_from_field, remove_subtype, rename_domain, rename_item,
    rewrite_table, set_default_subtype, set_subtype_field, set_value_for_range, sort_coded_values,
    table_to_domain, truncate_table,
};
