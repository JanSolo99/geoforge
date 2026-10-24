//! Creating File Geodatabases.
//!
//! The on-disk structure mirrors what GDAL's OpenFileGDB driver writes (which
//! ArcGIS opens): system tables `a00000001`–`a00000007`, `gdb` and `timestamps`
//! files, user tables from `a00000009` with an OBJECTID and spatial index.

mod alter;
mod btree;
mod catalog;
mod domain_edit;
mod edit;
mod geometry;
mod keys;
mod rebuild;
mod rewrite;
mod subtype_edit;
mod system_rows;
mod table;
mod txn;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use unigis_core::{Envelope, Error, Feature, Field, FieldType, GeometryKind, Result, Value};

use crate::schema::{
    Cardinality, Domain, DomainKind, MergePolicy, RelationshipClass, SplitPolicy, Subtypes,
};
use crate::table::{GdbField, GdbFieldType, GeometryDef};
use btree::{spatial_entries, write_index};
use table::TableWriter;

pub use alter::{FieldAlteration, alter_field, alter_fields, geodatabase_field_name};
pub use catalog::{delete_domain, delete_item, rename_domain, rename_item};
pub use domain_edit::{
    add_coded_value, alter_coded_value_description, alter_domain, create_domain,
    delete_coded_values, domain_rows, parse_domain_value, set_value_for_range, sort_coded_values,
    table_to_domain,
};
pub use edit::{TableEditor, delete_rows};
pub use rebuild::rebuild_attribute_indexes;
pub use rewrite::{add_field, delete_fields, rewrite_table, truncate_table};
pub use subtype_edit::{
    add_subtype, assign_default_to_field, assign_domain_to_field, remove_domain_from_field,
    remove_subtype, set_default_subtype, set_subtype_field,
};
pub use txn::recover;

const TYPE_FOLDER: &str = "{F3783E6F-65CA-4514-8315-CE3985DAD3B1}";
const TYPE_WORKSPACE: &str = "{C673FE0F-7280-404F-8532-20755DD8FC06}";
const TYPE_FEATURE_CLASS: &str = "{70737809-852C-4A03-9E22-2CECEA5B9BFA}";
const TYPE_TABLE: &str = "{CD06BC3B-789D-4C51-AAFA-A467912B8965}";
const REL_DATASET_IN_FOLDER: &str = "{DC78F1AB-34E4-43AC-BA47-1C4EABD0E7C7}";
const REL_DOMAIN_IN_DATASET: &str = "{17E08ADB-2B31-4DCD-8FDD-DF529E88F843}";
const TYPE_RELATIONSHIP_CLASS: &str = "{B606A7E1-FA5B-439C-849C-6E9C2481537B}";
const REL_RELATED_THROUGH: &str = "{725BADAB-3452-491B-A795-55F32D67229C}";
const TYPE_FEATURE_DATASET: &str = "{74737149-DCB5-4257-8904-B9724E32A530}";
const REL_DATASET_IN_FEATURE_DATASET: &str = "{A1633A59-46BA-4448-8706-D8ABE2B2B02E}";
const TYPE_CODED_DOMAIN: &str = "{8C368B12-A12E-4C7E-9638-C9C64E69E98F}";
const TYPE_RANGE_DOMAIN: &str = "{C29DA988-8C3E-45F7-8B5C-18E51EE7BEB4}";

/// Items registered in GDB_Items (beyond the root and workspace).
struct Item {
    uuid: String,
    name: String,
    kind: Option<GeometryKind>,
    definition: String,
    /// Domains referenced by this table's fields.
    domains: Vec<String>,
    /// The feature dataset it lives in.
    dataset: Option<String>,
    /// Its field names (OBJECTID and the shape field included), for relationship keys.
    field_names: Vec<String>,
}

/// A relationship class being added.
struct RelItem {
    uuid: String,
    dsid: usize,
    /// Fields of the relationship's own table (attributed and many-to-many), if it has one.
    table_fields: Option<[String; 2]>,
    rc: RelationshipClass,
    origin: String,
    destination: String,
}

/// A feature dataset: a folder of feature classes sharing one coordinate system.
struct DatasetItem {
    uuid: String,
    name: String,
    def: GeometryDef,
    dsid: usize,
}

struct DomainItem {
    uuid: String,
    domain: Domain,
}

/// A new geodatabase being written. Call [`GeodatabaseWriter::finish`] at the end.
pub struct GeodatabaseWriter {
    dir: PathBuf,
    next_table: u64,
    tables: Vec<(String, u64)>,
    items: Vec<Item>,
    datasets: Vec<DatasetItem>,
    relationships: Vec<RelItem>,
    spatial_refs: Vec<GeometryDef>,
    domains: Vec<DomainItem>,
    rng: u64,
    /// Set when adding to a geodatabase that already exists.
    existing: Option<Existing>,
    /// The transaction adding to an existing geodatabase: committed by `finish`; a
    /// writer dropped before that (or a crash) leaves the geodatabase as it was.
    txn: Option<txn::Guard>,
}

/// What an existing geodatabase's catalog holds that new items need to refer to.
struct Existing {
    root_uuid: String,
    /// (lower-case name, UUID) of the domains already defined.
    domain_uuids: Vec<(String, String)>,
    /// Spatial reference rows already in `GDB_SpatialRefs`: (text, FalseX, FalseY, XYUnits).
    spatial_refs: Vec<(String, f64, f64, f64)>,
    /// The largest DSID in the existing item definitions.
    max_dsid: i64,
    /// The feature datasets already there: (name, UUID, coordinate system).
    datasets: Vec<(String, String, GeometryDef)>,
    /// The tables and feature classes already there: (name, UUID).
    classes: Vec<(String, String)>,
    /// Names of the relationship classes already there.
    relationship_names: Vec<String>,
}

/// A table or feature class being filled. Hand it back to
/// [`GeodatabaseWriter::finish_table`] when done.
pub struct TableBuilder {
    subtypes: Option<Subtypes>,
    dataset: Option<String>,
    name: String,
    id: u64,
    writer: TableWriter,
    kind: Option<GeometryKind>,
    has_z: bool,
    has_m: bool,
    def: Option<GeometryDef>,
    fields: Vec<GdbField>,
}

impl TableBuilder {
    /// Adds an attribute index on `field` (`arcpy.management.AddIndex`). Call before
    /// writing rows. Text, integer, floating point, date and GUID fields can be indexed.
    pub fn add_attribute_index(&mut self, name: &str, field: &str) -> Result<()> {
        self.writer.add_index(name, field)
    }

    /// Appends a feature (its `oid` is ignored; OBJECTIDs are assigned 1, 2, ...).
    pub fn write(&mut self, feature: &Feature) -> Result<u64> {
        if self.kind.is_none() && feature.geometry.is_some() {
            return Err(Error::Unsupported(
                "geometry in a table without a geometry field".into(),
            ));
        }
        self.writer
            .push(feature.geometry.as_ref(), &feature.attributes)
    }
}

impl GeodatabaseWriter {
    /// Creates a new, empty `.gdb` directory. Fails if `dir` already exists.
    pub fn create(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        if dir.exists() {
            return Err(Error::io(&dir, std::io::ErrorKind::AlreadyExists.into()));
        }
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
            ^ (u64::from(std::process::id()) << 32);
        Ok(Self {
            dir,
            next_table: 9,
            tables: Vec::new(),
            items: Vec::new(),
            datasets: Vec::new(),
            relationships: Vec::new(),
            spatial_refs: Vec::new(),
            domains: Vec::new(),
            rng: seed | 1,
            existing: None,
            txn: None,
        })
    }

    /// Opens an existing `.gdb` to add tables, feature classes and domains to it. The
    /// catalog tables (`GDB_SystemCatalog`, `GDB_SpatialRefs`, `GDB_Items`,
    /// `GDB_ItemRelationships`) get new rows and their indexes are rebuilt by
    /// [`GeodatabaseWriter::finish`]; existing items are left untouched.
    pub fn open_existing(dir: impl AsRef<Path>) -> Result<Self> {
        use crate::table::Table;
        let dir = dir.as_ref().to_path_buf();
        let guard = txn::begin(&dir)?;
        let path = |n: u64| dir.join(format!("a{n:08x}"));
        let catalog = Table::open(path(1))?;
        let mut tables = Vec::new();
        for r in catalog.features() {
            let r = r?;
            if let Some(Value::String(name)) = r.attributes.first()
                && !name.is_empty()
            {
                tables.push((name.clone(), r.oid as u64));
            }
        }
        let next_table = catalog.max_oid() + 1;
        let items = Table::open(path(4))?;
        let col = |name: &str| {
            items
                .fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| Error::Unsupported(format!("GDB_Items has no {name} field")))
        };
        let (c_uuid, c_type, c_name, c_path, c_def) = (
            col("UUID")?,
            col("Type")?,
            col("Name")?,
            col("Path")?,
            col("Definition")?,
        );
        let (mut root_uuid, mut domain_uuids, mut max_dsid) = (None, Vec::new(), 2i64);
        let mut existing_datasets = Vec::new();
        let (mut existing_classes, mut existing_relationships) = (Vec::new(), Vec::new());
        for r in items.features() {
            let r = r?;
            let text = |i: usize| match &r.attributes[i] {
                Value::String(s) | Value::Guid(s) | Value::Xml(s) => s.clone(),
                _ => String::new(),
            };
            let (uuid, ty) = (text(c_uuid), text(c_type));
            if ty.eq_ignore_ascii_case(TYPE_FOLDER) && text(c_path) == "\\" {
                root_uuid = Some(uuid.clone());
            }
            if ty.eq_ignore_ascii_case(TYPE_CODED_DOMAIN)
                || ty.eq_ignore_ascii_case(TYPE_RANGE_DOMAIN)
            {
                domain_uuids.push((text(c_name).to_lowercase(), uuid.clone()));
            }
            let def = text(c_def);
            if ty.len() >= 10
                && (ty[..10].eq_ignore_ascii_case("{70737809-")
                    || ty[..10].eq_ignore_ascii_case("{CD06BC3B-"))
            {
                existing_classes.push((text(c_name), uuid.clone()));
            }
            if ty.len() >= 10 && ty[..10].eq_ignore_ascii_case("{B606A7E1-") {
                existing_relationships.push(text(c_name));
            }
            if ty.len() >= 10
                && ty[..10].eq_ignore_ascii_case("{74737149-")
                && let Some(d) = parse_dataset_def(&def)
            {
                existing_datasets.push((text(c_name), uuid.clone(), d));
            }
            if let Some(a) = def.find("<DSID>")
                && let Some(b) = def[a..].find("</DSID>")
                && let Ok(n) = def[a + 6..a + b].parse::<i64>()
            {
                max_dsid = max_dsid.max(n);
            }
        }
        let sr = Table::open(path(3))?;
        let spatial_refs = sr
            .features()
            .filter_map(|r| r.ok())
            .filter_map(|r| match r.attributes.as_slice() {
                [
                    Value::String(t),
                    Value::Double(x),
                    Value::Double(y),
                    Value::Double(u),
                    ..,
                ] => Some((t.clone(), *x, *y, *u)),
                _ => None,
            })
            .collect();
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
            ^ (u64::from(std::process::id()) << 32);
        Ok(Self {
            dir,
            next_table,
            tables,
            items: Vec::new(),
            datasets: Vec::new(),
            relationships: Vec::new(),
            spatial_refs: Vec::new(),
            domains: Vec::new(),
            rng: seed | 1,
            txn: Some(guard),
            existing: Some(Existing {
                datasets: existing_datasets,
                classes: existing_classes,
                relationship_names: existing_relationships,
                root_uuid: root_uuid.ok_or_else(|| {
                    Error::Unsupported("the geodatabase has no root folder item".into())
                })?,
                domain_uuids,
                spatial_refs,
                max_dsid,
            }),
        })
    }

    /// Adds an attribute domain (`arcpy.management.CreateDomain`). Fields refer to
    /// it through `Field::domain`.
    pub fn add_domain(&mut self, domain: &Domain) -> Result<()> {
        if self
            .domains
            .iter()
            .any(|d| d.domain.name.eq_ignore_ascii_case(&domain.name))
            || self.existing.as_ref().is_some_and(|e| {
                e.domain_uuids
                    .iter()
                    .any(|(n, _)| *n == domain.name.to_lowercase())
            })
        {
            return Err(Error::Unsupported(format!(
                "domain {} already exists",
                domain.name
            )));
        }
        let uuid = self.uuid();
        self.domains.push(DomainItem {
            uuid,
            domain: domain.clone(),
        });
        Ok(())
    }

    /// Whether a domain named `name` exists (added here, or already in the geodatabase).
    pub fn has_domain(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.domains
            .iter()
            .any(|d| d.domain.name.to_lowercase() == lower)
            || self
                .existing
                .as_ref()
                .is_some_and(|e| e.domain_uuids.iter().any(|(n, _)| *n == lower))
    }

    /// Starts a feature class (`kind = Some(..)`) or table (`None`). `fields` are the
    /// attribute fields; OBJECTID and SHAPE are added. Field names must be valid
    /// (see [`validate_field_name`]).
    pub fn create_table(
        &mut self,
        name: &str,
        kind: Option<GeometryKind>,
        has_z: bool,
        has_m: bool,
        fields: &[Field],
        srs_wkt: Option<&str>,
    ) -> Result<TableBuilder> {
        let def = kind.map(|_| default_geometry_def(srs_wkt, has_z, has_m));
        self.create_table_with(name, kind, has_z, has_m, fields, def, None)
    }

    /// Creates a table or feature class with subtypes (`arcpy.management.SetSubtypeField`,
    /// `AddSubtype`, `SetDefaultSubtype`, `AssignDomainToField`/`AssignDefaultToField` by subtype).
    /// The subtype field must be a short or long integer field of `fields`; the field takes the
    /// default subtype's code as its default value, as ArcGIS does. Domains named by subtype
    /// fields must exist (`add_domain`, or already in the geodatabase).
    #[allow(clippy::too_many_arguments)]
    pub fn create_table_with_subtypes(
        &mut self,
        name: &str,
        kind: Option<GeometryKind>,
        has_z: bool,
        has_m: bool,
        fields: &[Field],
        srs_wkt: Option<&str>,
        subtypes: Subtypes,
    ) -> Result<TableBuilder> {
        let bad = |m: String| Error::Unsupported(m);
        let find = |n: &str| fields.iter().find(|f| f.name.eq_ignore_ascii_case(n));
        let key = find(&subtypes.field).ok_or_else(|| {
            bad(format!(
                "ERROR 000728: Field {} does not exist within table",
                subtypes.field
            ))
        })?;
        if !matches!(key.field_type, FieldType::SmallInteger | FieldType::Integer) {
            return Err(bad(format!(
                "the subtype field {} must be a short or long integer",
                key.name
            )));
        }
        let mut codes = std::collections::HashSet::new();
        for s in &subtypes.subtypes {
            if !codes.insert(s.code) {
                return Err(bad(format!("subtype code {} is used twice", s.code)));
            }
            for sf in &s.fields {
                if find(&sf.field).is_none() {
                    return Err(bad(format!(
                        "ERROR 000728: Field {} does not exist within table",
                        sf.field
                    )));
                }
                if let Some(d) = &sf.domain
                    && !self.has_domain(d)
                {
                    return Err(bad(format!("ERROR 000732: Domain {d} does not exist")));
                }
            }
        }
        if !codes.contains(&subtypes.default_code) {
            return Err(bad(format!(
                "the default subtype code {} is not a subtype",
                subtypes.default_code
            )));
        }
        let mut fields = fields.to_vec();
        for f in &mut fields {
            if f.name.eq_ignore_ascii_case(&subtypes.field) {
                f.default = Some(match f.field_type {
                    FieldType::SmallInteger => {
                        Value::SmallInteger(i16::try_from(subtypes.default_code).map_err(|_| {
                            bad("the default subtype code does not fit a short integer".into())
                        })?)
                    }
                    _ => Value::Integer(i32::try_from(subtypes.default_code).map_err(|_| {
                        bad("the default subtype code does not fit a long integer".into())
                    })?),
                });
            }
        }
        let def = kind.map(|_| default_geometry_def(srs_wkt, has_z, has_m));
        let mut t = self.create_table_with(name, kind, has_z, has_m, &fields, def, None)?;
        t.subtypes = Some(subtypes);
        Ok(t)
    }

    /// Adds a relationship class between two tables or feature classes
    /// (`arcpy.management.CreateRelationshipClass`): one to one or one to many, composite or not,
    /// with the origin's primary key and the destination's foreign key. The classes may be made
    /// by this writer or already in the geodatabase. Relationship classes with their own table
    /// (attributed, many to many) are not written yet.
    pub fn add_relationship_class(&mut self, rc: &RelationshipClass) -> Result<()> {
        let bad = |m: String| Error::Unsupported(m);
        validate_table_name(&rc.name)?;
        if self.name_taken(&rc.name) {
            return Err(bad(format!(
                "ERROR 000258: Output {} already exists",
                rc.name
            )));
        }
        // A many-to-many relationship class always has a table of its own; so does an attributed one.
        let attributed = rc.is_attributed || rc.cardinality == Cardinality::ManyToMany;
        let mut rc = rc.clone();
        rc.is_attributed = attributed;
        let rc = &rc;
        let table_keys = if attributed {
            let (Some(dpk), Some(dfk)) = (&rc.destination_primary_key, &rc.destination_foreign_key)
            else {
                return Err(bad(
                    "ERROR 160263: Invalid relationship class specification (name the destination keys)".into(),
                ));
            };
            validate_field_name(&rc.origin_foreign_key)?;
            validate_field_name(dfk)?;
            if rc.origin_foreign_key.eq_ignore_ascii_case(dfk) {
                return Err(bad(
                    "ERROR 160263: Invalid relationship class specification".into(),
                ));
            }
            Some((dpk.clone(), [rc.origin_foreign_key.clone(), dfk.clone()]))
        } else {
            None
        };
        let uuid_of = |w: &Self, name: &str| {
            w.items
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(name))
                .map(|i| i.uuid.clone())
                .or_else(|| {
                    w.existing
                        .as_ref()?
                        .classes
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(name))
                        .map(|(_, u)| u.clone())
                })
                .ok_or_else(|| {
                    bad(format!(
                        "ERROR 000732: Dataset {name} does not exist or is not supported"
                    ))
                })
        };
        let (origin, destination) = (uuid_of(self, &rc.origin)?, uuid_of(self, &rc.destination)?);
        let has_field = |w: &Self, class: &str, field: &str| -> Result<bool> {
            if let Some(i) = w.items.iter().find(|i| i.name.eq_ignore_ascii_case(class)) {
                return Ok(i.field_names.iter().any(|f| f.eq_ignore_ascii_case(field)));
            }
            let id = w
                .tables
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(class))
                .map(|t| t.1)
                .ok_or_else(|| bad(format!("no table {class}")))?;
            let t = crate::table::Table::open(w.table_path(id))?;
            Ok(t.all_fields()
                .iter()
                .any(|f| f.name.eq_ignore_ascii_case(field)))
        };
        // The keys a class must have: the origin's primary key; in a plain relationship the
        // destination's foreign key; with a table of its own, the destination's primary key.
        let mut checks = vec![(&rc.origin, &rc.origin_primary_key)];
        match &table_keys {
            None => checks.push((&rc.destination, &rc.origin_foreign_key)),
            Some((dpk, _)) => checks.push((&rc.destination, dpk)),
        }
        for (class, field) in checks {
            if !has_field(self, class, field)? {
                return Err(bad(format!(
                    "ERROR 000728: Field {field} does not exist within table"
                )));
            }
        }
        let dsid = self.next_dsid();
        let uuid = self.uuid();
        let table_fields = match table_keys {
            Some((_, fields)) => {
                self.write_relationship_table(&rc.name, &fields, dsid)?;
                Some(fields)
            }
            None => None,
        };
        self.relationships.push(RelItem {
            uuid,
            dsid,
            table_fields,
            rc: rc.clone(),
            origin,
            destination,
        });
        Ok(())
    }

    /// Creates a feature dataset (`arcpy.management.CreateFeatureDataset`): a folder of feature
    /// classes that share its coordinate system and XY domain. `srs_wkt` is Esri WKT, `None` for
    /// an unknown system.
    pub fn create_feature_dataset(&mut self, name: &str, srs_wkt: Option<&str>) -> Result<()> {
        validate_table_name(name)?;
        if self.name_taken(name) {
            return Err(Error::Unsupported(format!(
                "ERROR 000258: Output {name} already exists"
            )));
        }
        let def = default_geometry_def(srs_wkt, false, false);
        let dsid = self.next_dsid();
        let uuid = self.uuid();
        // ArcGIS registers the dataset's coordinate system when it creates the dataset.
        if !self.spatial_refs.iter().any(|s| same_sr(s, &def)) {
            self.spatial_refs.push(def.clone());
        }
        self.datasets.push(DatasetItem {
            uuid,
            name: name.to_owned(),
            def,
            dsid,
        });
        Ok(())
    }

    /// The coordinate system (Esri WKT; inner `None` = unknown) of the feature dataset `name`, one
    /// made by this writer or already there; `None` when there is no such dataset.
    pub fn feature_dataset_srs(&self, name: &str) -> Option<Option<String>> {
        self.datasets
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
            .map(|d| d.def.srs_wkt.clone())
            .or_else(|| {
                self.existing
                    .as_ref()?
                    .datasets
                    .iter()
                    .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, _, d)| d.srs_wkt.clone())
            })
    }

    /// Creates a feature class inside the feature dataset `dataset` (one made by this writer or
    /// already in the geodatabase). It takes the dataset's coordinate system and XY domain.
    pub fn create_feature_class_in(
        &mut self,
        dataset: &str,
        name: &str,
        kind: GeometryKind,
        has_z: bool,
        has_m: bool,
        fields: &[Field],
    ) -> Result<TableBuilder> {
        let (dataset, mut def) = self
            .datasets
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(dataset))
            .map(|d| (d.name.clone(), d.def.clone()))
            .or_else(|| {
                self.existing.as_ref().and_then(|e| {
                    e.datasets
                        .iter()
                        .find(|(n, _, _)| n.eq_ignore_ascii_case(dataset))
                        .map(|(n, _, d)| (n.clone(), d.clone()))
                })
            })
            .ok_or_else(|| {
                Error::Unsupported(format!(
                    "ERROR 000732: Dataset {dataset} does not exist or is not supported"
                ))
            })?;
        def.has_z = has_z;
        def.has_m = has_m;
        def.extent = Envelope::EMPTY;
        def.grid_sizes = vec![0.0];
        self.create_table_with(
            name,
            Some(kind),
            has_z,
            has_m,
            fields,
            Some(def),
            Some(dataset),
        )
    }

    /// The table of an attributed or many-to-many relationship class: the two keys as long
    /// integers (indexed, `GDB_<DSID>_<field>`), then the object ID field `RID`.
    fn write_relationship_table(
        &mut self,
        name: &str,
        keys: &[String; 2],
        dsid: usize,
    ) -> Result<()> {
        let id = self.next_table;
        self.next_table += 1;
        let mut fields: Vec<GdbField> = keys
            .iter()
            .map(|k| {
                let mut f = field(k, GdbFieldType::Int32, true, 4);
                f.alias = Some(k.clone());
                f
            })
            .collect();
        fields.push(field("RID", GdbFieldType::ObjectId, false, 4));
        let base = self.table_path(id);
        let mut w = TableWriter::create(&base, fields, None, false, false)?;
        let mut index_names = Vec::new();
        for k in keys {
            let ix = format!("GDB_{dsid}_{k}");
            w.add_index(&ix, k)?;
            index_names.push((ix, k.clone()));
        }
        let table::Finished { indexes, .. } = w.finish()?;
        for ix in &indexes {
            write_index(
                &base.with_extension(format!("{}.atx", ix.name)),
                ix.key_size,
                ix.kind,
                &ix.entries,
            )?;
        }
        write_gdbindexes_for(
            &base.with_extension("gdbindexes"),
            "RID",
            None,
            &index_names,
        )?;
        self.tables.push((name.to_owned(), id));
        Ok(())
    }

    fn name_taken(&self, name: &str) -> bool {
        self.tables
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case(name))
            || self
                .datasets
                .iter()
                .any(|d| d.name.eq_ignore_ascii_case(name))
            || self
                .relationships
                .iter()
                .any(|r| r.rc.name.eq_ignore_ascii_case(name))
            || self.existing.as_ref().is_some_and(|e| {
                e.datasets
                    .iter()
                    .any(|(n, _, _)| n.eq_ignore_ascii_case(name))
                    || e.relationship_names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(name))
            })
    }

    /// The next DSID: one past every item definition so far.
    fn next_dsid(&self) -> usize {
        let base = self
            .existing
            .as_ref()
            .map_or(3, |e| e.max_dsid as usize + 1);
        base + self.items.len() + self.datasets.len() + self.relationships.len()
    }

    #[allow(clippy::too_many_arguments)]
    fn create_table_with(
        &mut self,
        name: &str,
        kind: Option<GeometryKind>,
        has_z: bool,
        has_m: bool,
        fields: &[Field],
        def: Option<GeometryDef>,
        dataset: Option<String>,
    ) -> Result<TableBuilder> {
        validate_table_name(name)?;
        if self.name_taken(name) {
            return Err(Error::Unsupported(format!(
                "a table named {name} already exists"
            )));
        }
        let mut gdb_fields = vec![field("OBJECTID", GdbFieldType::ObjectId, false, 4)];
        if let Some(def) = &def {
            let mut f = field("SHAPE", GdbFieldType::Geometry, true, 0);
            f.geometry = Some(def.clone());
            gdb_fields.push(f);
        }
        for f in fields {
            validate_field_name(&f.name)?;
            if gdb_fields
                .iter()
                .any(|g| g.name.eq_ignore_ascii_case(&f.name))
            {
                return Err(Error::Unsupported(format!(
                    "duplicate field name {}",
                    f.name
                )));
            }
            gdb_fields.push(to_gdb_field(f)?);
        }
        let id = self.next_table;
        self.next_table += 1;
        let writer =
            TableWriter::create(&self.table_path(id), gdb_fields.clone(), kind, has_z, has_m)?;
        Ok(TableBuilder {
            subtypes: None,
            dataset,
            name: name.to_owned(),
            id,
            writer,
            kind,
            has_z,
            has_m,
            def,
            fields: gdb_fields,
        })
    }

    /// Completes a table: writes its index files and registers it in the catalog.
    pub fn finish_table(&mut self, t: TableBuilder) -> Result<()> {
        let base = self.table_path(t.id);
        let table::Finished {
            envelopes,
            extent,
            grid,
            subsecond,
            indexes,
        } = t.writer.finish()?;
        let mut fields = t.fields.clone();
        for f in &mut fields {
            f.high_precision = subsecond.contains(&f.name);
        }
        let mut def = t.def.clone();
        if let Some(d) = &mut def {
            d.extent = extent;
        }
        let mut index_names = Vec::new();
        for ix in &indexes {
            write_index(
                &base.with_extension(format!("{}.atx", ix.name)),
                ix.key_size,
                ix.kind,
                &ix.entries,
            )?;
            index_names.push((ix.name.clone(), ix.field.clone()));
        }
        match (grid, &mut def) {
            (Some(grid), Some(d)) => {
                d.grid_sizes = vec![grid];
                write_index(
                    &base.with_extension("spx"),
                    8,
                    0x40,
                    &spatial_entries(&envelopes, grid),
                )?;
                write_gdbindexes(&base.with_extension("gdbindexes"), true, &index_names)?;
                if !self.spatial_refs.iter().any(|s| same_sr(s, d)) {
                    self.spatial_refs.push(d.clone());
                }
            }
            _ => write_gdbindexes(&base.with_extension("gdbindexes"), false, &index_names)?,
        }

        let uuid = self.uuid();
        let dsid = self.next_dsid();
        let mut definition = item_definition(
            &t.name,
            dsid,
            t.kind,
            t.has_z,
            t.has_m,
            def.as_ref(),
            &fields,
            t.subtypes.as_ref(),
        );
        if let Some(ds) = &t.dataset {
            // A class in a feature dataset has the dataset's name in its path.
            let (from, to) = (
                format!("<CatalogPath>\\{}</CatalogPath>", xml_escape(&t.name)),
                format!(
                    "<CatalogPath>\\{}\\{}</CatalogPath>",
                    xml_escape(ds),
                    xml_escape(&t.name)
                ),
            );
            definition = definition.replacen(&from, &to, 1);
        }
        let mut domains: Vec<String> = fields.iter().filter_map(|f| f.domain.clone()).collect();
        for sf in t
            .subtypes
            .iter()
            .flat_map(|s| &s.subtypes)
            .flat_map(|s| &s.fields)
        {
            if let Some(d) = &sf.domain
                && !domains.iter().any(|n| n.eq_ignore_ascii_case(d))
            {
                domains.push(d.clone());
            }
        }
        self.items.push(Item {
            uuid,
            name: t.name.clone(),
            kind: t.kind,
            definition,
            domains,
            dataset: t.dataset.clone(),
            field_names: fields.iter().map(|f| f.name.clone()).collect(),
        });
        self.tables.push((t.name, t.id));
        Ok(())
    }

    /// Writes the system tables. The geodatabase is complete afterwards.
    pub fn finish(mut self) -> Result<()> {
        if self.existing.is_some() {
            return self.finish_existing();
        }
        let s = |v: &str| Value::String(v.to_owned());
        let g = |v: &str| Value::Guid(v.to_owned());

        // a00000001 GDB_SystemCatalog
        let mut t = self.system(
            1,
            vec![
                field("ID", GdbFieldType::ObjectId, false, 4),
                field("Name", GdbFieldType::String, false, 160),
                field("FileFormat", GdbFieldType::Int32, false, 4),
            ],
        )?;
        let system = [
            "GDB_SystemCatalog",
            "GDB_DBTune",
            "GDB_SpatialRefs",
            "GDB_Items",
            "GDB_ItemTypes",
            "GDB_ItemRelationships",
            "GDB_ItemRelationshipTypes",
        ];
        for name in system {
            t.push(None, &[s(name), Value::Integer(0)])?;
        }
        t.push(None, &[s("GDB_ReplicaLog"), Value::Integer(2)])?;
        for (name, id) in &self.tables {
            while t.row_count() + 1 < *id {
                // Keep OBJECTIDs aligned with file numbers (none skipped today).
                t.push(None, &[s(""), Value::Integer(0)])?;
            }
            t.push(None, &[s(name), Value::Integer(0)])?;
        }
        t.finish()?;

        // a00000002 GDB_DBTune
        let mut t = self.system(
            2,
            vec![
                field("Keyword", GdbFieldType::String, false, 32),
                field("ParameterName", GdbFieldType::String, false, 32),
                field("ConfigString", GdbFieldType::String, true, 2048),
            ],
        )?;
        for (k, p, c) in system_rows::DBTUNE {
            t.push(None, &[s(k), s(p), s(c)])?;
        }
        t.finish()?;

        // a00000003 GDB_SpatialRefs
        let mut fields = vec![
            field("ID", GdbFieldType::ObjectId, false, 4),
            field("SRTEXT", GdbFieldType::String, false, 2048),
        ];
        for n in [
            "FalseX",
            "FalseY",
            "XYUnits",
            "FalseZ",
            "ZUnits",
            "FalseM",
            "MUnits",
            "XYTolerance",
            "ZTolerance",
            "MTolerance",
        ] {
            fields.push(field(n, GdbFieldType::Float64, true, 8));
        }
        let mut t = self.system(3, fields)?;
        for d in std::mem::take(&mut self.spatial_refs) {
            t.push(None, &spatial_ref_values(&d))?;
        }
        t.finish()?;

        // a00000004 GDB_Items
        let mut item_fields = vec![
            field("ObjectID", GdbFieldType::ObjectId, false, 4),
            field("UUID", GdbFieldType::GlobalId, false, 38),
            field("Type", GdbFieldType::Guid, false, 38),
            field("Name", GdbFieldType::String, true, 160),
            field("PhysicalName", GdbFieldType::String, true, 160),
            field("Path", GdbFieldType::String, true, 260),
            field("DatasetSubtype1", GdbFieldType::Int32, true, 4),
            field("DatasetSubtype2", GdbFieldType::Int32, true, 4),
            field("DatasetInfo1", GdbFieldType::String, true, 255),
            field("DatasetInfo2", GdbFieldType::String, true, 255),
            field("URL", GdbFieldType::String, true, 255),
            field("Definition", GdbFieldType::Xml, true, 0),
            field("Documentation", GdbFieldType::Xml, true, 0),
            field("ItemInfo", GdbFieldType::Xml, true, 0),
            field("Properties", GdbFieldType::Int32, true, 4),
            field("Defaults", GdbFieldType::Binary, true, 0),
        ];
        let mut shape = field("Shape", GdbFieldType::Geometry, true, 0);
        shape.geometry = Some(items_shape_def());
        item_fields.push(shape);
        let mut t = TableWriter::create(
            &self.table_path(4),
            item_fields,
            Some(GeometryKind::Polygon),
            true,
            true,
        )?;
        let root_uuid = self.uuid();
        let null = Value::Null;
        t.push(
            None,
            &[
                g(&root_uuid),
                g(TYPE_FOLDER),
                s(""),
                s(""),
                s("\\"),
                null.clone(),
                null.clone(),
                null.clone(),
                null.clone(),
                s(""),
                null.clone(),
                null.clone(),
                null.clone(),
                Value::Integer(1),
                null.clone(),
            ],
        )?;
        let ws_uuid = self.uuid();
        t.push(
            None,
            &[
                g(&ws_uuid),
                g(TYPE_WORKSPACE),
                s("Workspace"),
                s("WORKSPACE"),
                s(""),
                null.clone(),
                null.clone(),
                null.clone(),
                null.clone(),
                s(""),
                Value::Xml(WORKSPACE_XML.into()),
                null.clone(),
                null.clone(),
                Value::Integer(0),
                null.clone(),
            ],
        )?;
        for d in &self.datasets {
            t.push(None, &dataset_values(d))?;
        }
        for item in &self.items {
            t.push(None, &item_values(item))?;
        }
        for r in &self.relationships {
            t.push(None, &relationship_item_values(r))?;
        }
        for d in &self.domains {
            t.push(None, &domain_values(d))?;
        }
        t.finish()?;

        // a00000005 GDB_ItemTypes
        let mut t = self.system(
            5,
            vec![
                field("ObjectID", GdbFieldType::ObjectId, false, 4),
                field("UUID", GdbFieldType::Guid, false, 38),
                field("ParentTypeID", GdbFieldType::Guid, false, 38),
                field("Name", GdbFieldType::String, false, 160),
            ],
        )?;
        for (u, p, n) in system_rows::ITEM_TYPES {
            t.push(None, &[g(u), g(p), s(n)])?;
        }
        t.finish()?;

        // a00000006 GDB_ItemRelationships: each dataset is in the root folder.
        let mut t = self.system(
            6,
            vec![
                field("ObjectID", GdbFieldType::ObjectId, false, 4),
                field("UUID", GdbFieldType::GlobalId, false, 38),
                field("OriginID", GdbFieldType::Guid, false, 38),
                field("DestID", GdbFieldType::Guid, false, 38),
                field("Type", GdbFieldType::Guid, false, 38),
                field("Attributes", GdbFieldType::Xml, true, 0),
                field("Properties", GdbFieldType::Int32, true, 4),
            ],
        )?;
        let mut links: Vec<(String, String, &str)> = self
            .datasets
            .iter()
            .map(|d| (root_uuid.clone(), d.uuid.clone(), REL_DATASET_IN_FOLDER))
            .collect();
        links.extend(self.items.iter().map(|i| match self.dataset_uuid(i, None) {
            Some(d) => (d, i.uuid.clone(), REL_DATASET_IN_FEATURE_DATASET),
            None => (root_uuid.clone(), i.uuid.clone(), REL_DATASET_IN_FOLDER),
        }));
        for r in &self.relationships {
            links.push((root_uuid.clone(), r.uuid.clone(), REL_DATASET_IN_FOLDER));
            links.push((r.origin.clone(), r.uuid.clone(), REL_RELATED_THROUGH));
            links.push((r.destination.clone(), r.uuid.clone(), REL_RELATED_THROUGH));
        }
        for item in &self.items {
            for name in &item.domains {
                if let Some(d) = self
                    .domains
                    .iter()
                    .find(|d| d.domain.name.eq_ignore_ascii_case(name))
                {
                    links.push((item.uuid.clone(), d.uuid.clone(), REL_DOMAIN_IN_DATASET));
                }
            }
        }
        for (origin, dest, ty) in links {
            let uuid = self.uuid();
            t.push(None, &relationship_values(&uuid, &origin, &dest, ty))?;
        }
        t.finish()?;

        // a00000007 GDB_ItemRelationshipTypes
        let mut t = self.system(
            7,
            vec![
                field("ObjectID", GdbFieldType::ObjectId, false, 4),
                field("UUID", GdbFieldType::Guid, false, 38),
                field("OrigItemTypeID", GdbFieldType::Guid, false, 38),
                field("DestItemTypeID", GdbFieldType::Guid, false, 38),
                field("Name", GdbFieldType::String, true, 160),
                field("ForwardLabel", GdbFieldType::String, true, 255),
                field("BackwardLabel", GdbFieldType::String, true, 255),
                field("IsContainment", GdbFieldType::Int16, true, 2),
            ],
        )?;
        for (u, o, d, n, f, b, c) in system_rows::RELATIONSHIP_TYPES {
            t.push(
                None,
                &[g(u), g(o), g(d), s(n), s(f), s(b), Value::SmallInteger(*c)],
            )?;
        }
        t.finish()?;

        let write = |name: &str, bytes: &[u8]| {
            let p = self.dir.join(name);
            std::fs::write(&p, bytes).map_err(|e| Error::io(&p, e))
        };
        write("gdb", &[0x05, 0, 0, 0, 0xDE, 0xAD, 0xBE, 0xEF])?;
        write("timestamps", &[0xFF; 400])?;
        Ok(())
    }

    /// Appends the new items to the catalog tables of an existing geodatabase.
    fn finish_existing(mut self) -> Result<()> {
        use crate::writer::edit::TableEditor;
        let ex = self.existing.take().expect("existing geodatabase");
        let row = |attributes: Vec<Value>| Feature {
            oid: 0,
            geometry: None,
            attributes,
        };
        // GDB_SpatialRefs: spatial references not registered yet.
        let known = |d: &GeometryDef| {
            let wkt = d
                .srs_wkt
                .clone()
                .unwrap_or_else(|| "{B286C06B-0879-11D2-AACA-00C04FA33C20}".into());
            ex.spatial_refs.iter().any(|(t, x, y, u)| {
                *t == wkt && *x == d.x_origin && *y == d.y_origin && *u == d.xy_scale
            })
        };
        let new_refs: Vec<GeometryDef> = std::mem::take(&mut self.spatial_refs)
            .into_iter()
            .filter(|d| !known(d))
            .collect();
        if !new_refs.is_empty() {
            let mut ed = TableEditor::open(self.table_path(3))?;
            for d in &new_refs {
                ed.insert(&row(spatial_ref_values(d)))?;
            }
            ed.finish()?;
        }
        // GDB_Items and GDB_ItemRelationships.
        let mut links: Vec<(String, String, &str)> = self
            .datasets
            .iter()
            .map(|d| (ex.root_uuid.clone(), d.uuid.clone(), REL_DATASET_IN_FOLDER))
            .collect();
        links.extend(
            self.items
                .iter()
                .map(|i| match self.dataset_uuid(i, Some(&ex)) {
                    Some(d) => (d, i.uuid.clone(), REL_DATASET_IN_FEATURE_DATASET),
                    None => (ex.root_uuid.clone(), i.uuid.clone(), REL_DATASET_IN_FOLDER),
                }),
        );
        for r in &self.relationships {
            links.push((ex.root_uuid.clone(), r.uuid.clone(), REL_DATASET_IN_FOLDER));
            links.push((r.origin.clone(), r.uuid.clone(), REL_RELATED_THROUGH));
            links.push((r.destination.clone(), r.uuid.clone(), REL_RELATED_THROUGH));
        }
        for item in &self.items {
            for name in &item.domains {
                let lower = name.to_lowercase();
                let found = self
                    .domains
                    .iter()
                    .find(|d| d.domain.name.to_lowercase() == lower)
                    .map(|d| d.uuid.clone())
                    .or_else(|| {
                        ex.domain_uuids
                            .iter()
                            .find(|(n, _)| *n == lower)
                            .map(|(_, u)| u.clone())
                    });
                if let Some(d) = found {
                    links.push((item.uuid.clone(), d, REL_DOMAIN_IN_DATASET));
                }
            }
        }
        if !self.items.is_empty()
            || !self.domains.is_empty()
            || !self.datasets.is_empty()
            || !self.relationships.is_empty()
        {
            let mut ed = TableEditor::open(self.table_path(4))?;
            for d in &self.datasets {
                ed.insert(&row(dataset_values(d)))?;
            }
            for item in &self.items {
                ed.insert(&row(item_values(item)))?;
            }
            for r in &self.relationships {
                ed.insert(&row(relationship_item_values(r)))?;
            }
            for d in &self.domains {
                ed.insert(&row(domain_values(d)))?;
            }
            ed.finish()?;
            let mut ed = TableEditor::open(self.table_path(6))?;
            for (origin, dest, ty) in links {
                let uuid = self.uuid();
                ed.insert(&row(relationship_values(&uuid, &origin, &dest, ty)))?;
            }
            ed.finish()?;
        }
        // GDB_SystemCatalog: last, so a table is listed only once everything else is in place.
        if !self.tables.is_empty() {
            let mut ed = TableEditor::open(self.table_path(1))?;
            let mut new_tables: Vec<&(String, u64)> = self
                .tables
                .iter()
                .filter(|(_, id)| *id > ed.max_oid())
                .collect();
            new_tables.sort_by_key(|t| t.1);
            for (name, id) in new_tables {
                while ed.max_oid() + 1 < *id {
                    ed.insert(&row(vec![Value::String(String::new()), Value::Integer(0)]))?;
                }
                let at = ed.insert(&row(vec![Value::String(name.clone()), Value::Integer(0)]))?;
                debug_assert_eq!(at, *id);
            }
            ed.finish()?;
        }
        match self.txn.take() {
            Some(t) => t.commit(),
            None => Ok(()),
        }
    }

    /// The UUID of the feature dataset an item lives in.
    fn dataset_uuid(&self, item: &Item, existing: Option<&Existing>) -> Option<String> {
        let name = item.dataset.as_deref()?;
        self.datasets
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
            .map(|d| d.uuid.clone())
            .or_else(|| {
                existing?
                    .datasets
                    .iter()
                    .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, u, _)| u.clone())
            })
    }

    fn table_path(&self, id: u64) -> PathBuf {
        self.dir.join(format!("a{id:08x}"))
    }

    fn system(&self, id: u64, fields: Vec<GdbField>) -> Result<TableWriter> {
        TableWriter::create(&self.table_path(id), fields, None, false, false)
    }

    /// A random (version 4) UUID in `{...}` form.
    fn uuid(&mut self) -> String {
        let mut next = || {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            self.rng
        };
        let (a, b) = (next(), next());
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&a.to_le_bytes());
        bytes[8..].copy_from_slice(&b.to_le_bytes());
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        let h: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
        format!(
            "{{{}-{}-{}-{}-{}}}",
            h[..4].concat(),
            h[4..6].concat(),
            h[6..8].concat(),
            h[8..10].concat(),
            h[10..].concat()
        )
    }
}

/// A row of `GDB_SpatialRefs` for a geometry definition.
fn spatial_ref_values(d: &GeometryDef) -> Vec<Value> {
    let wkt = d
        .srs_wkt
        .clone()
        .unwrap_or_else(|| "{B286C06B-0879-11D2-AACA-00C04FA33C20}".into());
    let v = Value::Double;
    vec![
        Value::String(wkt),
        v(d.x_origin),
        v(d.y_origin),
        v(d.xy_scale),
        v(d.z_origin),
        v(d.z_scale),
        v(d.m_origin),
        v(d.m_scale),
        v(d.xy_tolerance),
        v(d.z_tolerance),
        v(d.m_tolerance),
    ]
}

/// A row of `GDB_Items` (without its OBJECTID).
#[allow(clippy::too_many_arguments)]
fn catalog_item_values(
    uuid: &str,
    ty: &str,
    name: &str,
    physical_name: &str,
    path: &str,
    subtype1: Value,
    subtype2: Value,
    info1: Value,
    definition: Value,
    properties: i32,
) -> Vec<Value> {
    let null = Value::Null;
    vec![
        Value::Guid(uuid.to_owned()),
        Value::Guid(ty.to_owned()),
        Value::String(name.to_owned()),
        Value::String(physical_name.to_owned()),
        Value::String(path.to_owned()),
        subtype1,
        subtype2,
        info1,
        null.clone(),
        Value::String(String::new()),
        definition,
        null.clone(),
        null.clone(),
        Value::Integer(properties),
        null,
    ]
}

fn item_values(item: &Item) -> Vec<Value> {
    let (ty, sub2, info1) = match item.kind {
        Some(k) => (
            TYPE_FEATURE_CLASS,
            Value::Integer(geometry_code(k)),
            Value::String("SHAPE".to_owned()),
        ),
        None => (TYPE_TABLE, Value::Null, Value::Null),
    };
    catalog_item_values(
        &item.uuid,
        ty,
        &item.name,
        &item.name.to_uppercase(),
        &match &item.dataset {
            Some(d) => format!("\\{d}\\{}", item.name),
            None => format!("\\{}", item.name),
        },
        Value::Integer(1),
        sub2,
        info1,
        Value::Xml(item.definition.clone()),
        1,
    )
}

/// A row of `GDB_Items` for a relationship class.
fn relationship_item_values(r: &RelItem) -> Vec<Value> {
    let rc = &r.rc;
    let x = xml_escape;
    let (card, subtype) = match rc.cardinality {
        Cardinality::OneToOne => ("esriRelCardinalityOneToOne", 1),
        Cardinality::OneToMany => ("esriRelCardinalityOneToMany", 2),
        Cardinality::ManyToMany => ("esriRelCardinalityManyToMany", 3),
    };
    let notification = match rc.notification.as_str() {
        "" => "None",
        n => n,
    };
    let key = |name: &str, role: &str| {
        format!(
            "    <RelationshipClassKey xsi:type=\"typens:RelationshipClassKey\">\n      <ObjectKeyName>{}</ObjectKeyName>\n      <ClassKeyName></ClassKeyName>\n      <KeyRole>{role}</KeyRole>\n    </RelationshipClassKey>\n",
            x(name)
        )
    };
    // A relationship class with a table of its own lists its fields and the destination's keys.
    let (table_xml, dest_keys) = match &r.table_fields {
        None => (
            "  <HasOID>false</HasOID>\n  <OIDFieldName></OIDFieldName>\n  <GPFieldInfoExs xsi:type=\"typens:ArrayOfGPFieldInfoEx\">\n  </GPFieldInfoExs>\n".to_string(),
            String::new(),
        ),
        Some(keys) => {
            let mut t = String::from("  <HasOID>true</HasOID>\n  <OIDFieldName>RID</OIDFieldName>\n  <GPFieldInfoExs xsi:type=\"typens:ArrayOfGPFieldInfoEx\">\n");
            for k in keys {
                t += &format!(
                    "    <GPFieldInfoEx xsi:type=\"typens:GPFieldInfoEx\">\n      <Name>{0}</Name>\n      <AliasName>{0}</AliasName>\n      <FieldType>esriFieldTypeInteger</FieldType>\n      <IsNullable>true</IsNullable>\n    </GPFieldInfoEx>\n",
                    x(k)
                );
            }
            t += "    <GPFieldInfoEx xsi:type=\"typens:GPFieldInfoEx\">\n      <Name>RID</Name>\n      <FieldType>esriFieldTypeOID</FieldType>\n      <IsNullable>false</IsNullable>\n      <Required>true</Required>\n      <Editable>false</Editable>\n    </GPFieldInfoEx>\n  </GPFieldInfoExs>\n";
            let dpk = rc.destination_primary_key.as_deref().unwrap_or_default();
            let d = format!(
                "  <DestinationClassKeys xsi:type=\"typens:ArrayOfRelationshipClassKey\">\n{}{}  </DestinationClassKeys>\n",
                key(dpk, "esriRelKeyRoleDestinationPrimary"),
                key(&keys[1], "esriRelKeyRoleDestinationForeign")
            );
            (t, d)
        }
    };
    let definition = format!(
        "<DERelationshipClassInfo xsi:type=\"typens:DERelationshipClassInfo\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:typens=\"http://www.esri.com/schemas/ArcGIS/10.8\">\n  <CatalogPath>\\{n}</CatalogPath>\n  <Name>{n}</Name>\n  <ChildrenExpanded>false</ChildrenExpanded>\n  <DatasetType>esriDTRelationshipClass</DatasetType>\n  <DSID>{dsid}</DSID>\n  <Versioned>false</Versioned>\n  <CanVersion>false</CanVersion>\n{table_xml}  <CLSID></CLSID>\n  <EXTCLSID></EXTCLSID>\n  <Cardinality>{card}</Cardinality>\n  <Notification>esriRelNotification{notification}</Notification>\n  <IsAttributed>{attributed}</IsAttributed>\n  <IsComposite>{composite}</IsComposite>\n  <OriginClassNames xsi:type=\"typens:Names\">\n    <Name>{origin}</Name>\n  </OriginClassNames>\n  <DestinationClassNames xsi:type=\"typens:Names\">\n    <Name>{dest}</Name>\n  </DestinationClassNames>\n  <KeyType>esriRelKeyTypeSingle</KeyType>\n  <ClassKey>esriRelClassKeyUndefined</ClassKey>\n  <ForwardPathLabel>{fwd}</ForwardPathLabel>\n  <BackwardPathLabel>{back}</BackwardPathLabel>\n  <IsReflexive>{reflexive}</IsReflexive>\n  <OriginClassKeys xsi:type=\"typens:ArrayOfRelationshipClassKey\">\n{pk}{fk}  </OriginClassKeys>\n{dest_keys}  <RelationshipRules xsi:type=\"typens:ArrayOfRelationshipRule\">\n  </RelationshipRules>\n  <IsAttachmentRelationship>false</IsAttachmentRelationship>\n</DERelationshipClassInfo>\n",
        n = x(&rc.name),
        dsid = r.dsid,
        composite = rc.is_composite,
        origin = x(&rc.origin),
        dest = x(&rc.destination),
        fwd = x(&rc.forward_label),
        back = x(&rc.backward_label),
        reflexive = rc.origin.eq_ignore_ascii_case(&rc.destination),
        pk = key(&rc.origin_primary_key, "esriRelKeyRoleOriginPrimary"),
        fk = key(&rc.origin_foreign_key, "esriRelKeyRoleOriginForeign"),
        attributed = r.table_fields.is_some(),
        table_xml = table_xml,
        dest_keys = dest_keys,
    );
    catalog_item_values(
        &r.uuid,
        TYPE_RELATIONSHIP_CLASS,
        &rc.name,
        &rc.name.to_uppercase(),
        &format!("\\{}", rc.name),
        Value::Integer(subtype),
        Value::Integer(i32::from(r.table_fields.is_some())),
        Value::Null,
        Value::Xml(definition),
        1,
    )
}

/// A row of `GDB_Items` for a feature dataset.
fn dataset_values(d: &DatasetItem) -> Vec<Value> {
    let sr_type = match &d.def.srs_wkt {
        Some(w) if w.trim_start().starts_with("GEOGCS") => "typens:GeographicCoordinateSystem",
        Some(_) => "typens:ProjectedCoordinateSystem",
        None => "typens:UnknownCoordinateSystem",
    };
    let n = xml_escape(&d.name);
    let definition = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<typens:DEFeatureDataset xmlns:typens=\"http://www.esri.com/schemas/ArcGIS/10.3\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"typens:DEFeatureDataset\">\n  <CatalogPath>\\{n}</CatalogPath>\n  <Name>{n}</Name>\n  <ChildrenExpanded>false</ChildrenExpanded>\n  <DatasetType>esriDTFeatureDataset</DatasetType>\n  <DSID>{}</DSID>\n  <Versioned>false</Versioned>\n  <CanVersion>false</CanVersion>\n  <Extent xsi:nil=\"true\"></Extent>\n{}\n</typens:DEFeatureDataset>\n",
        d.dsid,
        spatial_reference_xml(&d.def, sr_type),
    );
    catalog_item_values(
        &d.uuid,
        TYPE_FEATURE_DATASET,
        &d.name,
        &d.name.to_uppercase(),
        &format!("\\{}", d.name),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Xml(definition),
        1,
    )
}

/// A feature dataset's coordinate system and XY domain from its item definition.
fn parse_dataset_def(xml: &str) -> Option<GeometryDef> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let sr = doc
        .descendants()
        .find(|n| n.has_tag_name("SpatialReference"))?;
    let get = |tag: &str| {
        sr.children()
            .find(|n| n.has_tag_name(tag))
            .and_then(|n| n.text())
            .map(str::to_owned)
    };
    let num = |tag: &str, default: f64| {
        get(tag)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    };
    let mut def = default_geometry_def(get("WKT").as_deref(), false, false);
    def.x_origin = num("XOrigin", def.x_origin);
    def.y_origin = num("YOrigin", def.y_origin);
    def.xy_scale = num("XYScale", def.xy_scale);
    def.z_origin = num("ZOrigin", def.z_origin);
    def.z_scale = num("ZScale", def.z_scale);
    def.m_origin = num("MOrigin", def.m_origin);
    def.m_scale = num("MScale", def.m_scale);
    def.xy_tolerance = num("XYTolerance", def.xy_tolerance);
    def.z_tolerance = num("ZTolerance", def.z_tolerance);
    def.m_tolerance = num("MTolerance", def.m_tolerance);
    Some(def)
}

/// The `SpatialReference` element of a definition.
fn spatial_reference_xml(d: &GeometryDef, sr_type: &str) -> String {
    format!(
        "  <SpatialReference xsi:type=\"{sr_type}\">\n{}    <XOrigin>{}</XOrigin>\n    <YOrigin>{}</YOrigin>\n    <XYScale>{}</XYScale>\n    <ZOrigin>{}</ZOrigin>\n    <ZScale>{}</ZScale>\n    <MOrigin>{}</MOrigin>\n    <MScale>{}</MScale>\n    <XYTolerance>{}</XYTolerance>\n    <ZTolerance>{}</ZTolerance>\n    <MTolerance>{}</MTolerance>\n    <HighPrecision>true</HighPrecision>\n  </SpatialReference>",
        d.srs_wkt
            .as_deref()
            .map(|w| format!("    <WKT>{}</WKT>\n", xml_escape(w)))
            .unwrap_or_default(),
        d.x_origin,
        d.y_origin,
        d.xy_scale,
        d.z_origin,
        d.z_scale,
        d.m_origin,
        d.m_scale,
        d.xy_tolerance,
        d.z_tolerance,
        d.m_tolerance,
    )
}

fn domain_values(d: &DomainItem) -> Vec<Value> {
    let ty = match d.domain.kind {
        DomainKind::Coded(_) => TYPE_CODED_DOMAIN,
        DomainKind::Range { .. } => TYPE_RANGE_DOMAIN,
    };
    catalog_item_values(
        &d.uuid,
        ty,
        &d.domain.name,
        &d.domain.name.to_uppercase(),
        "",
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Xml(domain_xml(&d.domain)),
        1,
    )
}

/// A row of `GDB_ItemRelationships`.
fn relationship_values(uuid: &str, origin: &str, dest: &str, ty: &str) -> Vec<Value> {
    vec![
        Value::Guid(uuid.to_owned()),
        Value::Guid(origin.to_owned()),
        Value::Guid(dest.to_owned()),
        Value::Guid(ty.to_owned()),
        Value::Null,
        Value::Integer(1),
    ]
}

fn field(name: &str, field_type: GdbFieldType, nullable: bool, length: u32) -> GdbField {
    GdbField {
        name: name.to_owned(),
        alias: None,
        field_type,
        nullable,
        length,
        default: None,
        geometry: None,
        raster_type: None,
        high_precision: false,
        domain: None,
    }
}

pub(crate) fn to_gdb_field(f: &Field) -> Result<GdbField> {
    use GdbFieldType as G;
    let (t, len) = match f.field_type {
        FieldType::SmallInteger => (G::Int16, 2),
        FieldType::Integer => (G::Int32, 4),
        FieldType::BigInteger => (G::Int64, 8),
        FieldType::Single => (G::Float32, 4),
        FieldType::Double => (G::Float64, 8),
        FieldType::String => (G::String, if f.length == 0 { 255 } else { f.length }),
        FieldType::Date => (G::DateTime, 8),
        FieldType::DateOnly => (G::DateOnly, 8),
        FieldType::TimeOnly => (G::TimeOnly, 8),
        FieldType::TimestampOffset => (G::DateTimeOffset, 10),
        FieldType::Blob => (G::Binary, 0),
        FieldType::Guid => (G::Guid, 38),
        FieldType::GlobalId => (G::GlobalId, 38),
        FieldType::Xml => (G::Xml, 0),
        other => {
            return Err(Error::Unsupported(format!(
                "field {} of type {other:?}",
                f.name
            )));
        }
    };
    let mut g = field(&f.name, t, f.nullable, len);
    g.alias = f.alias.clone().filter(|a| *a != f.name);
    g.domain = f.domain.clone();
    g.default = f
        .default
        .as_ref()
        .and_then(|v| crate::table::encode_default(t, v));
    Ok(g)
}

/// ArcGIS rules: starts with a letter, letters/digits/underscore, at most 64 chars.
pub fn validate_field_name(name: &str) -> Result<()> {
    let ok = name.chars().next().is_some_and(char::is_alphabetic)
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && name.chars().count() <= 64;
    if ok {
        Ok(())
    } else {
        Err(Error::Unsupported(format!(
            "invalid geodatabase field name `{name}`"
        )))
    }
}

/// ArcGIS rules: starts with a letter, letters/digits/underscore, at most 160 chars,
/// and not a reserved `GDB_` name.
pub fn validate_table_name(name: &str) -> Result<()> {
    let ok = name.chars().next().is_some_and(char::is_alphabetic)
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && name.chars().count() <= 160
        && !name.to_ascii_uppercase().starts_with("GDB_");
    if ok {
        Ok(())
    } else {
        Err(Error::Unsupported(format!(
            "invalid geodatabase table name `{name}`"
        )))
    }
}

/// ArcGIS's default precision: 1e-9 degrees for geographic systems, 0.0001 units
/// (with origin far below any coordinate) for projected or unknown ones.
fn default_geometry_def(srs_wkt: Option<&str>, has_z: bool, has_m: bool) -> GeometryDef {
    let geographic = srs_wkt.is_some_and(|w| w.trim_start().starts_with("GEOGCS"));
    // Unknown coordinate system: ArcGIS Pro 3.7 uses origin -450359962737.0495.
    // Systems ArcGIS knows get its own domain (`xy_domains`); others a generous fixed origin.
    let (origin, scale, tol) = if geographic {
        (-400.0, 1e9, 8.983152841195215e-9)
    } else if srs_wkt.is_none() {
        (-450359962737.0495, 10000.0, 0.001)
    } else {
        (-2147483647.0, 10000.0, 0.001)
    };
    let mut def = GeometryDef {
        srs_wkt: srs_wkt.map(str::to_owned),
        has_z,
        has_m,
        x_origin: origin,
        y_origin: origin,
        xy_scale: scale,
        m_origin: -100000.0,
        m_scale: 10000.0,
        z_origin: -100000.0,
        z_scale: 10000.0,
        xy_tolerance: tol,
        m_tolerance: 0.001,
        z_tolerance: 0.001,
        extent: Envelope::EMPTY,
        grid_sizes: vec![0.0],
        extent_at: None,
        grid_at: None,
    };
    // ArcGIS's own default for the system, when it is one it knows.
    if let Some(d) = srs_wkt.and_then(crate::xy_domains::for_wkt) {
        def.x_origin = d.x_origin;
        def.y_origin = d.y_origin;
        def.xy_scale = d.scale;
        def.xy_tolerance = d.tolerance;
    }
    def
}

/// GDB_Items.Shape storage, as ArcGIS defines it (WGS84, 1e-6 degree resolution).
fn items_shape_def() -> GeometryDef {
    GeometryDef {
        srs_wkt: Some(WGS84.into()),
        has_z: true,
        has_m: true,
        x_origin: -180.0,
        y_origin: -90.0,
        xy_scale: 1e6,
        m_origin: -100000.0,
        m_scale: 10000.0,
        z_origin: -100000.0,
        z_scale: 10000.0,
        xy_tolerance: 2e-6,
        m_tolerance: 0.001,
        z_tolerance: 0.001,
        extent: Envelope::EMPTY,
        grid_sizes: vec![0.012, 0.4, 12.0],
        extent_at: None,
        grid_at: None,
    }
}

fn same_sr(a: &GeometryDef, b: &GeometryDef) -> bool {
    a.srs_wkt == b.srs_wkt
        && a.x_origin == b.x_origin
        && a.xy_scale == b.xy_scale
        && a.z_tolerance == b.z_tolerance
}

fn geometry_code(k: GeometryKind) -> i32 {
    match k {
        GeometryKind::Point => 1,
        GeometryKind::Multipoint => 2,
        GeometryKind::Polyline => 3,
        GeometryKind::Polygon => 4,
        GeometryKind::MultiPatch => 9,
    }
}

pub(crate) fn write_gdbindexes(
    path: &Path,
    spatial: bool,
    attribute: &[(String, String)],
) -> Result<()> {
    write_gdbindexes_for(path, "OBJECTID", spatial.then_some("SHAPE"), attribute)
}

/// `.gdbindexes` of a table whose object ID field is called `oid` and whose geometry
/// field (if it has a spatial index) `shape`.
pub(crate) fn write_gdbindexes_for(
    path: &Path,
    oid: &str,
    shape: Option<&str>,
    attribute: &[(String, String)],
) -> Result<()> {
    let spatial = shape.is_some();
    let mut out = Vec::new();
    let entry = |out: &mut Vec<u8>, name: &str, magic2: i32, magic3: i16, expr: &str| {
        out.extend_from_slice(&(name.encode_utf16().count() as u32).to_le_bytes());
        name.encode_utf16()
            .for_each(|u| out.extend_from_slice(&u.to_le_bytes()));
        out.extend_from_slice(&0i16.to_le_bytes());
        out.extend_from_slice(&magic2.to_le_bytes());
        out.extend_from_slice(&magic3.to_le_bytes());
        out.extend_from_slice(&1i32.to_le_bytes());
        out.extend_from_slice(&(expr.encode_utf16().count() as u32).to_le_bytes());
        expr.encode_utf16()
            .for_each(|u| out.extend_from_slice(&u.to_le_bytes()));
        out.extend_from_slice(&0i16.to_le_bytes());
    };
    out.extend_from_slice(&(1 + u32::from(spatial) + attribute.len() as u32).to_le_bytes());
    entry(&mut out, &format!("FDO_{oid}"), 16, -1, oid);
    if let Some(shape) = shape {
        entry(&mut out, &format!("FDO_{shape}"), 4, 0, shape);
    }
    for (name, field) in attribute {
        entry(&mut out, name, 2, 0, field);
    }
    txn::write_file(path, &out)
}

const WGS84: &str = r#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]"#;

const WORKSPACE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DEWorkspace xmlns:typens="http://www.esri.com/schemas/ArcGIS/10.3" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xs="http://www.w3.org/2001/XMLSchema" xsi:type="typens:DEWorkspace">
  <CatalogPath>\</CatalogPath>
  <Name/>
  <ChildrenExpanded>false</ChildrenExpanded>
  <WorkspaceType>esriLocalDatabaseWorkspace</WorkspaceType>
  <WorkspaceFactoryProgID/>
  <ConnectionString/>
  <ConnectionInfo xsi:nil="true"/>
  <Domains xsi:type="typens:ArrayOfDomain"/>
  <MajorVersion>3</MajorVersion>
  <MinorVersion>0</MinorVersion>
  <BugfixVersion>0</BugfixVersion>
</DEWorkspace>"#;

pub(super) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn esri_type_name(t: GdbFieldType) -> &'static str {
    use GdbFieldType as G;
    match t {
        G::Int16 => "esriFieldTypeSmallInteger",
        G::Int32 => "esriFieldTypeInteger",
        G::Int64 => "esriFieldTypeBigInteger",
        G::Float32 => "esriFieldTypeSingle",
        G::Float64 => "esriFieldTypeDouble",
        G::String => "esriFieldTypeString",
        G::DateTime => "esriFieldTypeDate",
        G::DateOnly => "esriFieldTypeDateOnly",
        G::TimeOnly => "esriFieldTypeTimeOnly",
        G::DateTimeOffset => "esriFieldTypeTimestampOffset",
        G::ObjectId => "esriFieldTypeOID",
        G::Geometry => "esriFieldTypeGeometry",
        G::Binary => "esriFieldTypeBlob",
        G::Raster => "esriFieldTypeRaster",
        G::Guid => "esriFieldTypeGUID",
        G::GlobalId => "esriFieldTypeGlobalID",
        G::Xml => "esriFieldTypeXML",
    }
}

/// `DEFeatureClassInfo` / `DETableInfo` XML for GDB_Items.Definition, in the shape
/// GDAL and ArcGIS write.
#[allow(clippy::too_many_arguments)]
fn item_definition(
    name: &str,
    dsid: usize,
    kind: Option<GeometryKind>,
    has_z: bool,
    has_m: bool,
    def: Option<&GeometryDef>,
    fields: &[GdbField],
    subtypes: Option<&Subtypes>,
) -> String {
    let root = if kind.is_some() {
        "DEFeatureClassInfo"
    } else {
        "DETableInfo"
    };
    let mut x = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<typens:{root} xmlns:typens=\"http://www.esri.com/schemas/ArcGIS/10.3\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"typens:{root}\">\n  <CatalogPath>\\{n}</CatalogPath>\n  <Name>{n}</Name>\n  <ChildrenExpanded>false</ChildrenExpanded>\n  <DatasetType>{dt}</DatasetType>\n  <DSID>{dsid}</DSID>\n  <Versioned>false</Versioned>\n  <CanVersion>false</CanVersion>\n  <HasOID>true</HasOID>\n  <OIDFieldName>OBJECTID</OIDFieldName>\n  <GPFieldInfoExs xsi:type=\"typens:ArrayOfGPFieldInfoEx\">\n",
        n = xml_escape(name),
        dt = if kind.is_some() {
            "esriDTFeatureClass"
        } else {
            "esriDTTable"
        },
    );
    for f in fields {
        x += &format!(
            "    <GPFieldInfoEx xsi:type=\"typens:GPFieldInfoEx\">\n      <Name>{}</Name>\n{}{}      <FieldType>{}</FieldType>\n{}      <IsNullable>{}</IsNullable>\n      <Length>{}</Length>\n      <Precision>0</Precision>\n      <Scale>0</Scale>\n{}    </GPFieldInfoEx>\n",
            xml_escape(&f.name),
            f.alias
                .as_deref()
                .map(|a| format!("      <AliasName>{}</AliasName>\n", xml_escape(a)))
                .unwrap_or_default(),
            f.domain
                .as_deref()
                .map(|d| format!("      <DomainName>{}</DomainName>\n", xml_escape(d)))
                .unwrap_or_default(),
            esri_type_name(f.field_type),
            default_xml(f),
            f.nullable,
            f.length,
            match f.field_type {
                GdbFieldType::ObjectId | GdbFieldType::Geometry =>
                    "      <Required>true</Required>\n",
                // Pro 3.2+: millisecond dates. Only set when the data needs it, so older
                // ArcGIS versions can still open the table otherwise.
                GdbFieldType::DateTime if f.high_precision =>
                    "      <HighPrecision>true</HighPrecision>\n",
                _ => "",
            },
        );
    }
    x += "  </GPFieldInfoExs>\n";
    match (kind, def) {
        (Some(k), Some(d)) => {
            let shape = match k {
                GeometryKind::Point => "esriGeometryPoint",
                GeometryKind::Multipoint => "esriGeometryMultipoint",
                GeometryKind::Polyline => "esriGeometryPolyline",
                GeometryKind::Polygon => "esriGeometryPolygon",
                GeometryKind::MultiPatch => "esriGeometryMultiPatch",
            };
            let sr_type = match &d.srs_wkt {
                Some(w) if w.trim_start().starts_with("GEOGCS") => {
                    "typens:GeographicCoordinateSystem"
                }
                Some(_) => "typens:ProjectedCoordinateSystem",
                None => "typens:UnknownCoordinateSystem",
            };
            x += &format!(
                "  <CLSID>{{52353152-891A-11D0-BEC6-00805F7C4268}}</CLSID>\n  <EXTCLSID></EXTCLSID>\n  <IsTimeInUTC>true</IsTimeInUTC>\n  <FeatureType>{}</FeatureType>\n  <ShapeType>{shape}</ShapeType>\n  <ShapeFieldName>SHAPE</ShapeFieldName>\n  <HasM>{has_m}</HasM>\n  <HasZ>{has_z}</HasZ>\n  <HasSpatialIndex>true</HasSpatialIndex>\n  <AreaFieldName></AreaFieldName>\n  <LengthFieldName></LengthFieldName>\n  <Extent xsi:nil=\"true\"></Extent>\n  <SpatialReference xsi:type=\"{sr_type}\">\n{}    <XOrigin>{}</XOrigin>\n    <YOrigin>{}</YOrigin>\n    <XYScale>{}</XYScale>\n    <ZOrigin>{}</ZOrigin>\n    <ZScale>{}</ZScale>\n    <MOrigin>{}</MOrigin>\n    <MScale>{}</MScale>\n    <XYTolerance>{}</XYTolerance>\n    <ZTolerance>{}</ZTolerance>\n    <MTolerance>{}</MTolerance>\n    <HighPrecision>true</HighPrecision>\n  </SpatialReference>\n",
                "esriFTSimple",
                d.srs_wkt
                    .as_deref()
                    .map(|w| format!("    <WKT>{}</WKT>\n", xml_escape(w)))
                    .unwrap_or_default(),
                d.x_origin,
                d.y_origin,
                d.xy_scale,
                d.z_origin,
                d.z_scale,
                d.m_origin,
                d.m_scale,
                d.xy_tolerance,
                d.z_tolerance,
                d.m_tolerance,
            );
        }
        _ => {
            x +=
                "  <CLSID>{7A566981-C114-11D2-8A28-006097AFF44E}</CLSID>\n  <EXTCLSID></EXTCLSID>\n"
        }
    }
    if let Some(st) = subtypes {
        x += &subtypes_xml(st);
    }
    x += &format!("</typens:{root}>\n");
    x
}

/// A field's default value as the definition gives it.
fn default_xml(f: &GdbField) -> String {
    let Some(v) = f
        .default
        .as_deref()
        .and_then(|d| crate::table::decode_default(f.field_type, d))
    else {
        return String::new();
    };
    match v {
        Value::String(s) => format!(
            "      <DefaultValueString>{}</DefaultValueString>\n",
            xml_escape(&s)
        ),
        Value::SmallInteger(n) => format!("      <DefaultValueNumeric>{n}</DefaultValueNumeric>\n"),
        Value::Integer(n) => format!("      <DefaultValueNumeric>{n}</DefaultValueNumeric>\n"),
        Value::BigInteger(n) => format!("      <DefaultValueNumeric>{n}</DefaultValueNumeric>\n"),
        Value::Single(n) => format!("      <DefaultValueNumeric>{n}</DefaultValueNumeric>\n"),
        Value::Double(n) => format!("      <DefaultValueNumeric>{n}</DefaultValueNumeric>\n"),
        _ => String::new(),
    }
}

/// The subtype elements of a table definition.
fn subtypes_xml(st: &Subtypes) -> String {
    let mut x = format!(
        "  <SubtypeFieldName>{}</SubtypeFieldName>\n  <DefaultSubtypeCode>{}</DefaultSubtypeCode>\n  <Subtypes xsi:type=\"typens:ArrayOfSubtype\">\n",
        xml_escape(&st.field),
        st.default_code
    );
    for s in &st.subtypes {
        x += &format!(
            "    <Subtype xsi:type=\"typens:Subtype\">\n      <SubtypeName>{}</SubtypeName>\n      <SubtypeCode>{}</SubtypeCode>\n      <FieldInfos xsi:type=\"typens:ArrayOfSubtypeFieldInfo\">\n",
            xml_escape(&s.name),
            s.code
        );
        for f in &s.fields {
            x += &format!(
                "        <SubtypeFieldInfo xsi:type=\"typens:SubtypeFieldInfo\">\n          <FieldName>{}</FieldName>\n",
                xml_escape(&f.field)
            );
            if let Some(d) = &f.domain {
                x += &format!("          <DomainName>{}</DomainName>\n", xml_escape(d));
            }
            if let Some(v) = &f.default {
                let (ty, text) = match v {
                    Value::SmallInteger(n) => ("xs:short", n.to_string()),
                    Value::Integer(n) => ("xs:int", n.to_string()),
                    Value::BigInteger(n) => ("xs:long", n.to_string()),
                    Value::Single(n) => ("xs:float", n.to_string()),
                    Value::Double(n) => ("xs:double", n.to_string()),
                    Value::String(s) => ("xs:string", xml_escape(s)),
                    _ => ("xs:string", String::new()),
                };
                x += &format!("          <DefaultValue xsi:type=\"{ty}\">{text}</DefaultValue>\n");
            }
            x += "        </SubtypeFieldInfo>\n";
        }
        x += "      </FieldInfos>\n    </Subtype>\n";
    }
    x += "  </Subtypes>\n";
    x
}

/// `GPCodedValueDomain2` / `GPRangeDomain2` XML, as ArcGIS writes it.
pub(super) fn domain_xml(d: &Domain) -> String {
    let (xs, esri) = match d.field_type {
        FieldType::SmallInteger => ("xs:short", "esriFieldTypeSmallInteger"),
        FieldType::Integer => ("xs:int", "esriFieldTypeInteger"),
        FieldType::BigInteger => ("xs:long", "esriFieldTypeBigInteger"),
        FieldType::Single => ("xs:float", "esriFieldTypeSingle"),
        FieldType::Double => ("xs:double", "esriFieldTypeDouble"),
        FieldType::Date => ("xs:dateTime", "esriFieldTypeDate"),
        FieldType::DateOnly => ("xs:date", "esriFieldTypeDateOnly"),
        FieldType::TimeOnly => ("xs:time", "esriFieldTypeTimeOnly"),
        _ => ("xs:string", "esriFieldTypeString"),
    };
    let text = |v: &Value| match v {
        Value::SmallInteger(n) => n.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::BigInteger(n) => n.to_string(),
        Value::Single(n) => n.to_string(),
        Value::Double(n) => n.to_string(),
        Value::Date(t) => t.format("%Y-%m-%dT%H:%M:%S").to_string(),
        Value::String(s) => s.clone(),
        other => format!("{other:?}"),
    };
    let split = match d.split_policy {
        SplitPolicy::DefaultValue => "esriSPTDefaultValue",
        SplitPolicy::Duplicate => "esriSPTDuplicate",
        SplitPolicy::GeometryRatio => "esriSPTGeometryRatio",
    };
    let merge = match d.merge_policy {
        MergePolicy::DefaultValue => "esriMPTDefaultValue",
        MergePolicy::SumValues => "esriMPTSumValues",
        MergePolicy::AreaWeighted => "esriMPTAreaWeighted",
    };
    let root = match d.kind {
        DomainKind::Coded(_) => "GPCodedValueDomain2",
        DomainKind::Range { .. } => "GPRangeDomain2",
    };
    // ArcGIS Pro 3.7's form: no declaration, a plain root element, single-quoted attributes.
    let mut x = format!(
        "<{root} xsi:type='typens:{root}' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' xmlns:xs='http://www.w3.org/2001/XMLSchema' xmlns:typens='http://www.esri.com/schemas/ArcGIS/10.8'><DomainName>{}</DomainName><FieldType>{esri}</FieldType><MergePolicy>{merge}</MergePolicy><SplitPolicy>{split}</SplitPolicy><Description>{}</Description><Owner></Owner>",
        xml_escape(&d.name),
        xml_escape(&d.description),
    );
    match &d.kind {
        DomainKind::Coded(values) => {
            x += "<CodedValues xsi:type='typens:ArrayOfCodedValue'>";
            for (code, name) in values {
                x += &format!(
                    "<CodedValue xsi:type='typens:CodedValue'><Name>{}</Name><Code xsi:type='{xs}'>{}</Code></CodedValue>",
                    xml_escape(name),
                    xml_escape(&text(code))
                );
            }
            x += "</CodedValues>";
        }
        DomainKind::Range { min, max } => {
            x += &format!(
                "<MaxValue xsi:type='{xs}'>{}</MaxValue><MinValue xsi:type='{xs}'>{}</MinValue>",
                xml_escape(&text(max)),
                xml_escape(&text(min))
            );
        }
    }
    x += &format!("</{root}>");
    x
}
