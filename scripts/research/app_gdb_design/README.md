# App geodatabase design completion

Behavior references are the existing ArcGIS Pro 3.7 fixtures and public documentation;
no new format implementation, Esri SDK or geoprocessing algorithm was introduced.

- `testdata/gdb-reference/subtypes.gdb` and `subtype_ops/*.json`: ArcGIS states for
  Assign/RemoveDomainFromField, SetDefaultSubtype, RemoveSubtype and defaults.
  `crates/unigis-filegdb/tests/subtype_edit.rs` compares the existing clean-room
  writer's full definition/default/domain links with these saved states.
  Removing a subtype domain removes that SubtypeFieldInfo (including its default);
  removing all subtype codes retains the subtype field with default code0.
- `relationship_tables.gdb`: ArcGIS's attributed att4 and many-to-many mn between
  Parcels/Owners. Their keys belong to junction tables, not the destination class.
  `crates/unigis-filegdb/tests/relationship_classes.rs` already verifies them against
  the writer/reader; the app regression checks the fixture's concrete metadata.
- [ArcGIS domains](https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/create-modify-and-delete-domains.html):
  once subtypes exist, subtype domains/defaults are used instead of dataset-level
  settings. [Subtype design](https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/create-modify-and-delete-subtypes.html)
  includes a default subtype and per-subtype domains/defaults.

The app uses explicit removal markers so an old project missing design metadata
cannot erase an on-disk assignment. Unknown legacy subtype-domain metadata stays
unknown; imported metadata is authoritative. Shared domain definitions within a
layer are synchronized on edit, and contradictory project definitions are refused
before any row writes. New engine regressions copy the fixtures into temporary
folders, check physical metadata, second-save idempotence, deletion safety and undo.
UI regression changes codes through the actual Create and Identify controls.

Scope: native FileGDB write-back and schema reads (including uploaded reads), plus
in-memory typed attribute controls. Relationship metadata is read and displayed;
this does not create joins or implement junction-table traversal/editing. Subtype
renaming, changing an existing subtype field, explicit default removal, and
cross-layer refresh after editing a shared domain remain outside this item.

Domain conversion is preflighted against an estimated 32 MiB per-class copied-domain
budget, including repeated subtype references. Oversized metadata is refused with
an error rather than silently dropping validation rules or allocating unbounded
copies. A generated fixture with 40 subtypes sharing a large domain covers refusal.

Native verification (2026-11-08): release UniGIS, isolated XDG directories, copies
of edit_fc.gdb and relationship_tables.gdb. The actual default-subtype selector
saved code 2; relationship metadata showed att4/mn and both junction keys. Create
Features produced feature 51 with Int2/Float20.0 through its coded control; Identify
changed it to subtype 1 and offered only Shallow, storing Float10.0. Native Save
Edits followed by a fresh source reopen confirmed 51 features, Int1/Float10.0 on
that feature, default subtype 2 and both per-subtype domain/default assignments.
Local screenshots/projects/logs: `/tmp/gdb-design-native/` (not committed assets).
