//! Geodatabase schema tools (Data Management › Feature Dataset, Domains, Subtypes, Fields,
//! Indexes, Relationship Classes) as registry entries, so every front end runs them. They edit a
//! file geodatabase in place through `unigis-filegdb`, with ArcGIS Pro's parameter names and
//! defaults (`arcpy.GetParameterInfo`, ArcGIS Pro 3.7), and report ArcGIS's derived output
//! (`out_table: <path>`).
//!
//! Datasets are named by path: `city.gdb/parcels`, or `city.gdb/Dataset/parcels` inside a
//! feature dataset. Mobile geodatabases and shapefiles are not edited by these tools yet.

use std::path::{Path, PathBuf};

use unigis_core::{Error, FieldType, Result};

use crate::registry::{Context, Kind, Outcome, Param, Report, Runner, Tool};

fn err(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

const fn path(name: &'static str, label: &'static str, help: &'static str) -> Param {
    Param {
        name,
        label,
        kind: Kind::Path,
        required: true,
        default: "",
        help,
    }
}

const fn text(name: &'static str, label: &'static str, required: bool) -> Param {
    Param {
        name,
        label,
        kind: Kind::Text,
        required,
        default: "",
        help: "",
    }
}

const fn choice(
    name: &'static str,
    label: &'static str,
    options: &'static [&'static str],
    default: &'static str,
) -> Param {
    Param {
        name,
        label,
        kind: Kind::Choice(options),
        required: false,
        default,
        help: "",
    }
}

const TABLE: Param = path(
    "in_table",
    "Input Table",
    "A geodatabase table or feature class: city.gdb/parcels.",
);
const WORKSPACE: Param = path(
    "in_workspace",
    "Input Workspace",
    "The file geodatabase (.gdb).",
);
const DOMAIN: Param = text("domain_name", "Domain Name", true);
const SUBTYPES: Param = Param {
    name: "subtype_code",
    label: "Subtype",
    kind: Kind::Text,
    required: false,
    default: "",
    help: "Subtype codes separated by `;` (none: the field itself).",
};
const FIELD_TYPES: &[&str] = &[
    "SHORT",
    "LONG",
    "BIGINTEGER",
    "FLOAT",
    "DOUBLE",
    "TEXT",
    "DATE",
    "DATEHIGHPRECISION",
    "DATEONLY",
    "TIMEONLY",
    "TIMESTAMPOFFSET",
    "BLOB",
    "GUID",
    "RASTER",
];
const DOMAIN_FIELD_TYPES: &[&str] = &[
    "SHORT",
    "LONG",
    "BIGINTEGER",
    "FLOAT",
    "DOUBLE",
    "TEXT",
    "DATE",
    "DATEONLY",
    "TIMEONLY",
];
const SPLIT: &[&str] = &["DEFAULT", "DUPLICATE", "GEOMETRY_RATIO"];
const MERGE: &[&str] = &["DEFAULT", "SUM_VALUES", "AREA_WEIGHTED"];

fn need(ctx: &dyn Context, name: &str, label: &str) -> Result<String> {
    ctx.get(name)
        .ok_or_else(|| err(format!("ERROR 000735: {label}: Value is required")))
}

/// The file geodatabase that `path` is or lies in.
fn geodatabase_of(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| p.join("a00000001.gdbtable").is_file())
        .map(Path::to_path_buf)
}

fn not_supported(text: &str, label: &str) -> Error {
    if Path::new(text).ancestors().any(|p| {
        p.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("geodatabase"))
    }) {
        err(format!(
            "{label}: {text}: mobile geodatabases are not edited by this tool yet"
        ))
    } else {
        err(format!(
            "ERROR 000732: {label}: Dataset {text} does not exist or is not supported"
        ))
    }
}

/// `city.gdb[/Dataset]/table` as the geodatabase and the table's name.
fn table_in(ctx: &dyn Context, name: &str, label: &str) -> Result<(PathBuf, String, String)> {
    let text = need(ctx, name, label)?;
    let path = PathBuf::from(&text);
    let gdb = geodatabase_of(&path).ok_or_else(|| not_supported(&text, label))?;
    if gdb == path {
        return Err(not_supported(&text, label));
    }
    let item = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| not_supported(&text, label))?;
    if !imp::has_table(&gdb, &item) {
        return Err(not_supported(&text, label));
    }
    Ok((gdb, item, text))
}

/// The file geodatabase named by a workspace parameter.
fn workspace_in(ctx: &dyn Context) -> Result<(PathBuf, String)> {
    let text = need(ctx, "in_workspace", "Input Workspace")?;
    let path = PathBuf::from(&text);
    if !path.join("a00000001.gdbtable").is_file() {
        return Err(not_supported(&text, "Input Workspace"));
    }
    Ok((path, text))
}

fn derived(key: &str, value: &str) -> Report {
    Report {
        rows: vec![],
        messages: vec![format!("{key}: {value}")],
    }
}

/// Subtype codes separated by `;`.
fn codes(ctx: &dyn Context) -> Result<Vec<i64>> {
    ctx.get("subtype_code")
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| {
            c.parse()
                .map_err(|_| err(format!("ERROR 000192: Invalid value for Subtype Code: {c}")))
        })
        .collect()
}

/// A field type keyword of Add Field, Alter Field and Create Domain.
fn field_type(keyword: &str) -> Result<FieldType> {
    Ok(match keyword.trim().to_ascii_uppercase().as_str() {
        "TEXT" => FieldType::String,
        "FLOAT" => FieldType::Single,
        "DOUBLE" => FieldType::Double,
        "SHORT" => FieldType::SmallInteger,
        "LONG" => FieldType::Integer,
        "BIGINTEGER" => FieldType::BigInteger,
        "DATE" => FieldType::Date,
        "DATEONLY" => FieldType::DateOnly,
        "TIMEONLY" => FieldType::TimeOnly,
        "TIMESTAMPOFFSET" => FieldType::TimestampOffset,
        "BLOB" => FieldType::Blob,
        "GUID" => FieldType::Guid,
        other => return Err(err(format!("Field Type {other} is not supported yet"))),
    })
}

/// `NAME_A` / `NAME_B` keyword pairs: whether the value is the first.
fn flag(ctx: &dyn Context, name: &str, yes: &str, default: bool) -> bool {
    ctx.get(name)
        .map_or(default, |v| v.eq_ignore_ascii_case(yes) || v == "true")
}

#[cfg(feature = "gdb")]
mod imp {
    use super::*;
    use crate::TableOutput;
    use unigis_core::{Field, Value};
    use unigis_filegdb as fgdb;

    fn split(ctx: &dyn Context) -> Result<Option<fgdb::SplitPolicy>> {
        Ok(
            match ctx
                .get("split_policy")
                .map(|s| s.to_ascii_uppercase())
                .as_deref()
            {
                None => None,
                Some("DEFAULT") => Some(fgdb::SplitPolicy::DefaultValue),
                Some("DUPLICATE") => Some(fgdb::SplitPolicy::Duplicate),
                Some("GEOMETRY_RATIO") => Some(fgdb::SplitPolicy::GeometryRatio),
                Some(other) => return Err(err(format!("split policy {other}"))),
            },
        )
    }

    fn merge(ctx: &dyn Context) -> Result<Option<fgdb::MergePolicy>> {
        Ok(
            match ctx
                .get("merge_policy")
                .map(|s| s.to_ascii_uppercase())
                .as_deref()
            {
                None => None,
                Some("DEFAULT") => Some(fgdb::MergePolicy::DefaultValue),
                Some("SUM_VALUES") => Some(fgdb::MergePolicy::SumValues),
                Some("AREA_WEIGHTED") => Some(fgdb::MergePolicy::AreaWeighted),
                Some(other) => return Err(err(format!("merge policy {other}"))),
            },
        )
    }

    pub fn has_table(gdb: &Path, item: &str) -> bool {
        fgdb::Geodatabase::open(gdb).is_ok_and(|g| {
            g.entries()
                .iter()
                .any(|e| e.name.eq_ignore_ascii_case(item))
        })
    }

    #[cfg(feature = "proj")]
    fn spatial_reference(spec: &str) -> Result<String> {
        Ok(unigis_proj::Crs::parse(spec)?.arcgis_wkt())
    }

    #[cfg(not(feature = "proj"))]
    fn spatial_reference(_: &str) -> Result<String> {
        Err(err("a coordinate system parameter needs the proj feature"))
    }

    pub fn create_feature_dataset(ctx: &dyn Context) -> Result<Report> {
        let text = need(ctx, "out_dataset_path", "Output Geodatabase")?;
        let gdb = PathBuf::from(&text);
        if !gdb.join("a00000001.gdbtable").is_file() {
            return Err(not_supported(&text, "Output Geodatabase"));
        }
        let name = need(ctx, "out_name", "Feature Dataset Name")?;
        let wkt = ctx
            .get("spatial_reference")
            .map(|s| spatial_reference(&s))
            .transpose()?;
        let mut w = fgdb::GeodatabaseWriter::open_existing(&gdb)?;
        w.create_feature_dataset(&name, wkt.as_deref())?;
        w.finish()?;
        Ok(derived(
            "out_dataset",
            &gdb.join(&name).display().to_string(),
        ))
    }

    pub fn create_domain(ctx: &dyn Context) -> Result<Report> {
        let (gdb, text) = workspace_in(ctx)?;
        let range = match ctx
            .get("domain_type")
            .unwrap_or_else(|| "CODED".into())
            .to_ascii_uppercase()
            .as_str()
        {
            "CODED" => false,
            "RANGE" => true,
            other => return Err(err(format!("domain type {other}"))),
        };
        fgdb::create_domain(
            &gdb,
            &need(ctx, "domain_name", "Domain Name")?,
            &ctx.get("domain_description").unwrap_or_default(),
            field_type(&ctx.get("field_type").unwrap_or_else(|| "SHORT".into()))?,
            range,
            split(ctx)?.unwrap_or(fgdb::SplitPolicy::DefaultValue),
            merge(ctx)?.unwrap_or(fgdb::MergePolicy::DefaultValue),
        )?;
        Ok(derived("out_workspace", &text))
    }

    pub fn delete_domain(ctx: &dyn Context) -> Result<Report> {
        let (gdb, text) = workspace_in(ctx)?;
        fgdb::delete_domain(&gdb, &need(ctx, "domain_name", "Domain Name")?)?;
        Ok(derived("out_workspace", &text))
    }

    pub fn alter_domain(ctx: &dyn Context) -> Result<Report> {
        let (gdb, text) = workspace_in(ctx)?;
        fgdb::alter_domain(
            &gdb,
            &need(ctx, "domain_name", "Domain Name")?,
            ctx.get("new_domain_name").as_deref(),
            ctx.get("new_domain_description").as_deref(),
            split(ctx)?,
            merge(ctx)?,
            ctx.get("new_domain_owner").as_deref(),
        )?;
        Ok(derived("out_workspace", &text))
    }

    pub fn add_coded_value_to_domain(ctx: &dyn Context) -> Result<Report> {
        let (gdb, text) = workspace_in(ctx)?;
        fgdb::add_coded_value(
            &gdb,
            &need(ctx, "domain_name", "Domain Name")?,
            &need(ctx, "code", "Code Value")?,
            &need(ctx, "code_description", "Code Description")?,
        )?;
        Ok(derived("out_workspace", &text))
    }

    pub fn delete_coded_value_from_domain(ctx: &dyn Context) -> Result<Report> {
        let (gdb, text) = workspace_in(ctx)?;
        fgdb::delete_coded_values(
            &gdb,
            &need(ctx, "domain_name", "Domain Name")?,
            &need(ctx, "code", "Code Value")?,
        )?;
        Ok(derived("out_workspace", &text))
    }

    pub fn set_value_for_range_domain(ctx: &dyn Context) -> Result<Report> {
        let (gdb, text) = workspace_in(ctx)?;
        fgdb::set_value_for_range(
            &gdb,
            &need(ctx, "domain_name", "Domain Name")?,
            &need(ctx, "min_value", "Minimum Value")?,
            &need(ctx, "max_value", "Maximum Value")?,
        )?;
        Ok(derived("out_workspace", &text))
    }

    pub fn sort_coded_value_domain(ctx: &dyn Context) -> Result<Report> {
        let (gdb, text) = workspace_in(ctx)?;
        fgdb::sort_coded_values(
            &gdb,
            &need(ctx, "domain_name", "Domain Name")?,
            !flag(ctx, "sort_by", "DESCRIPTION", false),
            !flag(ctx, "sort_order", "DESCENDING", false),
        )?;
        Ok(derived("out_workspace", &text))
    }

    pub fn domain_to_table(ctx: &dyn Context) -> Result<Outcome> {
        let (gdb, _) = workspace_in(ctx)?;
        let (ty, rows) = fgdb::domain_rows(&gdb, &need(ctx, "domain_name", "Domain Name")?)?;
        let mut description = Field::new(
            need(ctx, "description_field", "Field Description")?,
            FieldType::String,
        );
        description.length = 255;
        Ok(Outcome::Table(TableOutput {
            fields: vec![
                Field::new(need(ctx, "code_field", "Code Field")?, ty),
                description,
            ],
            rows: rows
                .into_iter()
                .map(|(code, d)| vec![code, Value::String(d)])
                .collect(),
        }))
    }

    pub fn table_to_domain(ctx: &dyn Context) -> Result<Report> {
        let table = ctx.features("in_table")?;
        let find = |name: &str| {
            table
                .fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    err(format!(
                        "ERROR 000728: Field {name} does not exist within table"
                    ))
                })
        };
        let c = find(&need(ctx, "code_field", "Code Field")?)?;
        let d = find(&need(ctx, "description_field", "Description Field")?)?;
        let ty = table.fields()[c].field_type;
        let mut values = Vec::new();
        for f in table.features() {
            let f = f?;
            let desc = match &f.attributes[d] {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => format!("{other:?}"),
            };
            values.push((f.attributes[c].clone(), desc));
        }
        let (gdb, text) = workspace_in(ctx)?;
        fgdb::table_to_domain(
            &gdb,
            &need(ctx, "domain_name", "Domain Name")?,
            &ctx.get("domain_description").unwrap_or_default(),
            ty,
            &values,
            flag(ctx, "update_option", "REPLACE", false),
        )?;
        Ok(derived("out_workspace", &text))
    }

    pub fn assign_domain_to_field(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        fgdb::assign_domain_to_field(
            &gdb,
            &table,
            &need(ctx, "field_name", "Field Name")?,
            &need(ctx, "domain_name", "Domain Name")?,
            &codes(ctx)?,
        )?;
        Ok(derived("out_table", &text))
    }

    pub fn remove_domain_from_field(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        fgdb::remove_domain_from_field(
            &gdb,
            &table,
            &need(ctx, "field_name", "Field Name")?,
            &codes(ctx)?,
        )?;
        Ok(derived("out_table", &text))
    }

    pub fn assign_default_to_field(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        let clear = flag(ctx, "clear_value", "CLEAR_VALUE", false);
        // Numbers by their look; the field's type decides what fits.
        let value = ctx.get("default_value").filter(|_| !clear).map(|v| {
            v.parse::<i64>()
                .map(Value::BigInteger)
                .or_else(|_| v.parse::<f64>().map(Value::Double))
                .unwrap_or(Value::String(v))
        });
        if value.is_none() && !clear {
            return Err(err("ERROR 000735: Default Value: Value is required"));
        }
        fgdb::assign_default_to_field(
            &gdb,
            &table,
            &need(ctx, "field_name", "Field Name")?,
            value.as_ref(),
            &codes(ctx)?,
        )?;
        Ok(derived("out_table", &text))
    }

    pub fn set_subtype_field(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        if flag(ctx, "clear_value", "CLEAR_SUBTYPE_FIELD", false) {
            return Err(err("clearing the subtype field is not supported yet"));
        }
        fgdb::set_subtype_field(&gdb, &table, &need(ctx, "field", "Field Name")?)?;
        Ok(derived("out_table", &text))
    }

    fn code(ctx: &dyn Context) -> Result<i64> {
        let c = need(ctx, "subtype_code", "Subtype Code")?;
        c.trim()
            .parse()
            .map_err(|_| err(format!("ERROR 000192: Invalid value for Subtype Code: {c}")))
    }

    pub fn add_subtype(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        fgdb::add_subtype(
            &gdb,
            &table,
            code(ctx)?,
            &need(ctx, "subtype_description", "Subtype Name")?,
        )?;
        Ok(derived("out_table", &text))
    }

    pub fn remove_subtype(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        need(ctx, "subtype_code", "Subtype Code")?;
        fgdb::remove_subtype(&gdb, &table, &codes(ctx)?)?;
        Ok(derived("out_table", &text))
    }

    pub fn set_default_subtype(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        fgdb::set_default_subtype(&gdb, &table, code(ctx)?)?;
        Ok(derived("out_table", &text))
    }

    pub fn add_index(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        let fields = need(ctx, "fields", "Fields to Index")?;
        if fields.contains(';') {
            return Err(err("indexes on several fields are not supported yet"));
        }
        if flag(ctx, "unique", "UNIQUE", false) {
            return Err(err("unique indexes are not supported yet"));
        }
        let name = ctx
            .get("index_name")
            .unwrap_or_else(|| format!("{}_idx", fields.trim()));
        fgdb::add_attribute_index(&gdb, &table, fields.trim(), &name)?;
        Ok(derived("out_table", &text))
    }

    pub fn remove_index(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        fgdb::remove_attribute_index(
            &gdb,
            &table,
            &need(ctx, "index_name", "Index Name or Indexed Item")?,
        )?;
        Ok(derived("out_table", &text))
    }

    pub fn add_field(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        let keyword = ctx.get("field_type").unwrap_or_else(|| "LONG".into());
        let name = need(ctx, "field_name", "Field Name")?;
        if name.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(err(
                "ERROR 000310: The Field name must not start with a number",
            ));
        }
        let mut field = Field::new(name, field_type(&keyword)?);
        // An existing field of the same type is left as it is, with a warning.
        let geodatabase = fgdb::Geodatabase::open(&gdb)?;
        let existing = geodatabase
            .open_table(&table)?
            .fields()
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(&field.name))
            .map(|f| f.field_type);
        match existing {
            Some(t) if t == field.field_type => {
                return Ok(Report {
                    rows: vec![],
                    messages: vec![
                        format!("WARNING 000012: {} already exists", field.name),
                        format!("out_table: {text}"),
                    ],
                });
            }
            Some(_) => {
                return Err(err(format!("ERROR 000012: {} already exists", field.name)));
            }
            None => {}
        }
        let number = |name: &str| -> Result<Option<u32>> {
            ctx.get(name)
                .map(|v| {
                    v.trim()
                        .parse()
                        .map_err(|_| err(format!("ERROR 000094: {name}: {v} is not a number")))
                })
                .transpose()
        };
        field.precision = number("field_precision")?.unwrap_or(0);
        field.scale = number("field_scale")?.unwrap_or(0);
        if field.field_type == FieldType::String {
            field.length = number("field_length")?.unwrap_or(255);
        }
        field.alias = ctx.get("field_alias");
        field.nullable = !flag(ctx, "field_is_nullable", "NON_NULLABLE", false);
        if flag(ctx, "field_is_required", "REQUIRED", false) {
            return Err(err("required fields are not supported yet"));
        }
        let base = geodatabase.table_base(&table)?;
        drop(geodatabase);
        fgdb::add_field(&base, &field)?;
        if let Some(domain) = ctx.get("field_domain") {
            fgdb::assign_domain_to_field(&gdb, &table, &field.name, &domain, &[])?;
        }
        Ok(derived("out_table", &text))
    }

    pub fn delete_field(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        let named: Vec<String> = need(ctx, "drop_field", "Fields")?
            .split(';')
            .map(|f| f.trim().to_owned())
            .filter(|f| !f.is_empty())
            .collect();
        let geodatabase = fgdb::Geodatabase::open(&gdb)?;
        let base = geodatabase.table_base(&table)?;
        let opened = geodatabase.open_table(&table)?;
        let names: Vec<String> = if flag(ctx, "method", "KEEP_FIELDS", false) {
            let shape = opened.shape_field().map(|f| f.name.to_ascii_lowercase());
            opened
                .fields()
                .iter()
                .map(|f| f.name.clone())
                .filter(|n| !named.iter().any(|k| k.eq_ignore_ascii_case(n)))
                .filter(|n| {
                    let n = n.to_ascii_lowercase();
                    shape
                        .as_ref()
                        .is_none_or(|s| n != format!("{s}_length") && n != format!("{s}_area"))
                })
                .collect()
        } else {
            named
        };
        drop(opened);
        drop(geodatabase);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        fgdb::delete_fields(&base, &refs)?;
        Ok(derived("out_table", &text))
    }

    pub fn alter_field(ctx: &dyn Context) -> Result<Report> {
        let (gdb, table, text) = table_in(ctx, "in_table", "Input Table")?;
        let new_type = ctx.get("field_type").map(|t| field_type(&t)).transpose()?;
        let nullable = ctx
            .get("field_is_nullable")
            .map(|v| !v.eq_ignore_ascii_case("NON_NULLABLE"));
        let length = ctx
            .get("field_length")
            .map(|v| {
                v.trim()
                    .parse()
                    .map_err(|_| err(format!("ERROR 000094: New Field Length: {v}")))
            })
            .transpose()?;
        let new_name = ctx.get("new_field_name");
        let alias = ctx.get("new_field_alias");
        fgdb::alter_field(
            &gdb,
            &table,
            &need(ctx, "field", "Field Name")?,
            &fgdb::FieldAlteration {
                new_name: new_name.as_deref(),
                alias: alias.as_deref(),
                clear_alias: flag(ctx, "clear_field_alias", "CLEAR_ALIAS", false),
                new_type,
                length,
                nullable,
            },
        )?;
        Ok(derived("out_table", &text))
    }

    pub fn create_relationship_class(ctx: &dyn Context) -> Result<Report> {
        let (gdb, origin, _) = table_in(ctx, "origin_table", "Origin Table")?;
        let (dest_gdb, destination, _) = table_in(ctx, "destination_table", "Destination Table")?;
        let out = need(ctx, "out_relationship_class", "Output Relationship Class")?;
        let out_gdb = geodatabase_of(Path::new(&out));
        if dest_gdb != gdb || out_gdb.as_ref() != Some(&gdb) {
            return Err(err(
                "the tables and the relationship class must be in the same geodatabase",
            ));
        }
        let name = Path::new(&out)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| err("ERROR 000735: Output Relationship Class: Value is required"))?;
        let upper = |n: &str, d: &str| ctx.get(n).unwrap_or_else(|| d.into()).to_ascii_uppercase();
        let cardinality = match upper("cardinality", "ONE_TO_ONE").as_str() {
            "ONE_TO_ONE" => fgdb::Cardinality::OneToOne,
            "ONE_TO_MANY" => fgdb::Cardinality::OneToMany,
            "MANY_TO_MANY" => fgdb::Cardinality::ManyToMany,
            _ => {
                return Err(err(
                    "ERROR 000800: The value is not a member of ONE_TO_ONE | ONE_TO_MANY | MANY_TO_MANY.",
                ));
            }
        };
        let notification = match upper("message_direction", "NONE").as_str() {
            "NONE" => "None",
            "FORWARD" => "Forward",
            "BACKWARD" => "Backward",
            "BOTH" => "Both",
            _ => {
                return Err(err(
                    "ERROR 000800: The value is not a member of FORWARD | BACKWARD | BOTH | NONE.",
                ));
            }
        };
        let rc = fgdb::RelationshipClass {
            name,
            cardinality,
            is_composite: upper("relationship_type", "SIMPLE") == "COMPOSITE",
            is_attributed: upper("attributed", "NONE") == "ATTRIBUTED",
            notification: notification.into(),
            origin,
            destination,
            origin_primary_key: need(ctx, "origin_primary_key", "Origin Primary Key")?,
            origin_foreign_key: need(ctx, "origin_foreign_key", "Origin Foreign Key")?,
            destination_primary_key: ctx.get("destination_primary_key"),
            destination_foreign_key: ctx.get("destination_foreign_key"),
            forward_label: need(ctx, "forward_label", "Forward Path Label")?,
            backward_label: need(ctx, "backward_label", "Backward Path label")?,
        };
        let mut w = fgdb::GeodatabaseWriter::open_existing(&gdb)?;
        w.add_relationship_class(&rc)?;
        w.finish()?;
        Ok(derived("out_relationship_class", &out))
    }
}

/// Without the `gdb` feature (the browser build) the tools are listed but refuse to run.
macro_rules! unavailable {
    ($($report:ident),* ; $($dynamic:ident),*) => {
        #[cfg(not(feature = "gdb"))]
        mod imp {
            use super::*;
            fn none() -> Error {
                err("geodatabases can't be edited in this build")
            }
            pub fn has_table(_: &Path, _: &str) -> bool {
                false
            }
            $(pub fn $report(_: &dyn Context) -> Result<Report> {
                let _ = (table_in, workspace_in, codes, field_type, flag, derived);
                Err(none())
            })*
            $(pub fn $dynamic(_: &dyn Context) -> Result<Outcome> {
                Err(none())
            })*
        }
    };
}
unavailable!(create_feature_dataset, create_domain, delete_domain, alter_domain,
    add_coded_value_to_domain, delete_coded_value_from_domain, set_value_for_range_domain,
    sort_coded_value_domain, table_to_domain, assign_domain_to_field, remove_domain_from_field,
    assign_default_to_field, set_subtype_field, add_subtype, remove_subtype, set_default_subtype,
    add_index, remove_index, add_field, delete_field, alter_field, create_relationship_class;
    domain_to_table);

macro_rules! tool {
    ($konst:ident, $id:literal, $name:literal, $toolset:literal, $summary:literal, $run:expr, [$($p:expr),* $(,)?]) => {
        pub const $konst: Tool = Tool {
            id: $id,
            name: $name,
            toolbox: concat!("Data Management › ", $toolset),
            summary: $summary,
            params: &[$($p),*],
            output_suffix: "",
            measures: false,
            run: $run,
        };
    };
}

tool!(
    CREATE_FEATURE_DATASET,
    "create_feature_dataset",
    "Create Feature Dataset",
    "Feature Dataset",
    "A new feature dataset in a file geodatabase (ArcGIS Pro Create Feature Dataset).",
    Runner::Report(imp::create_feature_dataset),
    [
        path(
            "out_dataset_path",
            "Output Geodatabase",
            "The file geodatabase (.gdb)."
        ),
        text("out_name", "Feature Dataset Name", true),
        Param {
            name: "spatial_reference",
            label: "Coordinate System",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "A factory code, a .prj file or WKT."
        },
    ]
);

tool!(
    CREATE_DOMAIN,
    "create_domain",
    "Create Domain",
    "Domains",
    "A new attribute domain in a geodatabase (ArcGIS Pro Create Domain).",
    Runner::Report(imp::create_domain),
    [
        WORKSPACE,
        DOMAIN,
        text("domain_description", "Domain Description", false),
        choice("field_type", "Field Type", DOMAIN_FIELD_TYPES, "SHORT"),
        choice("domain_type", "Domain Type", &["CODED", "RANGE"], "CODED"),
        choice("split_policy", "Split Policy", SPLIT, "DEFAULT"),
        choice("merge_policy", "Merge Policy", MERGE, "DEFAULT"),
    ]
);

tool!(
    DELETE_DOMAIN,
    "delete_domain",
    "Delete Domain",
    "Domains",
    "Deletes an attribute domain no field uses (ArcGIS Pro Delete Domain).",
    Runner::Report(imp::delete_domain),
    [WORKSPACE, DOMAIN]
);

tool!(
    ALTER_DOMAIN,
    "alter_domain",
    "Alter Domain",
    "Domains",
    "Renames an attribute domain or changes its description and policies (ArcGIS Pro Alter Domain).",
    Runner::Report(imp::alter_domain),
    [
        WORKSPACE,
        DOMAIN,
        text("new_domain_name", "New Domain Name", false),
        text("new_domain_description", "New Domain Description", false),
        choice("split_policy", "New Split Policy", SPLIT, ""),
        choice("merge_policy", "New Merge Policy", MERGE, ""),
        text("new_domain_owner", "New Domain Owner", false),
    ]
);

tool!(
    ADD_CODED_VALUE_TO_DOMAIN,
    "add_coded_value_to_domain",
    "Add Coded Value To Domain",
    "Domains",
    "Adds a code and its description to a coded value domain (ArcGIS Pro Add Coded Value To Domain).",
    Runner::Report(imp::add_coded_value_to_domain),
    [
        WORKSPACE,
        DOMAIN,
        text("code", "Code Value", true),
        text("code_description", "Code Description", true),
    ]
);

tool!(
    DELETE_CODED_VALUE_FROM_DOMAIN,
    "delete_coded_value_from_domain",
    "Delete Coded Value From Domain",
    "Domains",
    "Removes codes (separated by `;`) from a coded value domain (ArcGIS Pro Delete Coded Value From Domain).",
    Runner::Report(imp::delete_coded_value_from_domain),
    [WORKSPACE, DOMAIN, text("code", "Code Value", true)]
);

tool!(
    SET_VALUE_FOR_RANGE_DOMAIN,
    "set_value_for_range_domain",
    "Set Value For Range Domain",
    "Domains",
    "Sets the minimum and maximum of a range domain (ArcGIS Pro Set Value For Range Domain).",
    Runner::Report(imp::set_value_for_range_domain),
    [
        WORKSPACE,
        DOMAIN,
        text("min_value", "Minimum Value", true),
        text("max_value", "Maximum Value", true),
    ]
);

tool!(
    SORT_CODED_VALUE_DOMAIN,
    "sort_coded_value_domain",
    "Sort Coded Value Domain",
    "Domains",
    "Orders a coded value domain by code or description (ArcGIS Pro Sort Coded Value Domain).",
    Runner::Report(imp::sort_coded_value_domain),
    [
        WORKSPACE,
        DOMAIN,
        choice("sort_by", "Sort By", &["CODE", "DESCRIPTION"], "CODE"),
        choice(
            "sort_order",
            "Sort Order",
            &["ASCENDING", "DESCENDING"],
            "ASCENDING"
        ),
    ]
);

tool!(
    DOMAIN_TO_TABLE,
    "domain_to_table",
    "Domain To Table",
    "Domains",
    "A table of a domain's codes and descriptions (ArcGIS Pro Domain To Table).",
    Runner::Dynamic(imp::domain_to_table),
    [
        WORKSPACE,
        DOMAIN,
        text("code_field", "Code Field", true),
        text("description_field", "Field Description", true),
    ]
);

tool!(
    TABLE_TO_DOMAIN,
    "table_to_domain",
    "Table To Domain",
    "Domains",
    "A coded value domain from a table's code and description fields (ArcGIS Pro Table To Domain).",
    Runner::Report(imp::table_to_domain),
    [
        Param {
            name: "in_table",
            label: "Input Table",
            kind: Kind::Features,
            required: true,
            default: "",
            help: ""
        },
        Param {
            name: "code_field",
            label: "Code Field",
            kind: Kind::Field {
                of: "in_table",
                numeric: false
            },
            required: true,
            default: "",
            help: ""
        },
        Param {
            name: "description_field",
            label: "Description Field",
            kind: Kind::Field {
                of: "in_table",
                numeric: false
            },
            required: true,
            default: "",
            help: ""
        },
        WORKSPACE,
        DOMAIN,
        text("domain_description", "Domain Description", false),
        choice(
            "update_option",
            "Update Option",
            &["APPEND", "REPLACE"],
            "APPEND"
        ),
    ]
);

tool!(
    ASSIGN_DOMAIN_TO_FIELD,
    "assign_domain_to_field",
    "Assign Domain To Field",
    "Domains",
    "Sets the attribute domain of a field, or of a field in some subtypes (ArcGIS Pro Assign Domain To Field).",
    Runner::Report(imp::assign_domain_to_field),
    [
        TABLE,
        text("field_name", "Field Name", true),
        DOMAIN,
        SUBTYPES,
    ]
);

tool!(
    REMOVE_DOMAIN_FROM_FIELD,
    "remove_domain_from_field",
    "Remove Domain From Field",
    "Domains",
    "Removes a field's attribute domain (ArcGIS Pro Remove Domain From Field).",
    Runner::Report(imp::remove_domain_from_field),
    [TABLE, text("field_name", "Field Name", true), SUBTYPES]
);

tool!(
    ASSIGN_DEFAULT_TO_FIELD,
    "assign_default_to_field",
    "Assign Default To Field",
    "Fields",
    "Sets or clears the default value of a field, or of a field in some subtypes (ArcGIS Pro Assign Default To Field).",
    Runner::Report(imp::assign_default_to_field),
    [
        TABLE,
        text("field_name", "Field Name", true),
        text("default_value", "Default Value", false),
        SUBTYPES,
        choice(
            "clear_value",
            "Clear Value",
            &["DO_NOT_CLEAR", "CLEAR_VALUE"],
            "DO_NOT_CLEAR"
        ),
    ]
);

tool!(
    SET_SUBTYPE_FIELD,
    "set_subtype_field",
    "Set Subtype Field",
    "Subtypes",
    "Makes a SHORT or LONG field the subtype field (ArcGIS Pro Set Subtype Field).",
    Runner::Report(imp::set_subtype_field),
    [
        TABLE,
        text("field", "Field Name", true),
        choice(
            "clear_value",
            "Clear Value",
            &["DO_NOT_CLEAR", "CLEAR_SUBTYPE_FIELD"],
            "DO_NOT_CLEAR"
        ),
    ]
);

tool!(
    ADD_SUBTYPE,
    "add_subtype",
    "Add Subtype",
    "Subtypes",
    "Adds a subtype code and name (ArcGIS Pro Add Subtype).",
    Runner::Report(imp::add_subtype),
    [
        TABLE,
        Param {
            name: "subtype_code",
            label: "Subtype Code",
            kind: Kind::Number,
            required: true,
            default: "",
            help: ""
        },
        text("subtype_description", "Subtype Name", true),
    ]
);

tool!(
    REMOVE_SUBTYPE,
    "remove_subtype",
    "Remove Subtype",
    "Subtypes",
    "Removes subtypes by code, separated by `;` (ArcGIS Pro Remove Subtype).",
    Runner::Report(imp::remove_subtype),
    [
        TABLE,
        Param {
            name: "subtype_code",
            label: "Subtype Code",
            kind: Kind::Text,
            required: true,
            default: "",
            help: ""
        },
    ]
);

tool!(
    SET_DEFAULT_SUBTYPE,
    "set_default_subtype",
    "Set Default Subtype",
    "Subtypes",
    "Sets the subtype new rows get (ArcGIS Pro Set Default Subtype).",
    Runner::Report(imp::set_default_subtype),
    [
        TABLE,
        Param {
            name: "subtype_code",
            label: "Subtype Code",
            kind: Kind::Number,
            required: true,
            default: "",
            help: ""
        },
    ]
);

tool!(
    ADD_INDEX,
    "add_index",
    "Add Attribute Index",
    "Indexes",
    "An attribute index on a field (ArcGIS Pro Add Attribute Index).",
    Runner::Report(imp::add_index),
    [
        TABLE,
        text("fields", "Fields to Index", true),
        text("index_name", "Index Name", false),
        choice("unique", "Unique", &["NON_UNIQUE", "UNIQUE"], "NON_UNIQUE"),
        choice(
            "ascending",
            "Ascending",
            &["NON_ASCENDING", "ASCENDING"],
            "NON_ASCENDING"
        ),
    ]
);

tool!(
    REMOVE_INDEX,
    "remove_index",
    "Remove Attribute Index",
    "Indexes",
    "Deletes an attribute index (ArcGIS Pro Remove Attribute Index).",
    Runner::Report(imp::remove_index),
    [
        TABLE,
        text("index_name", "Index Name or Indexed Item", true)
    ]
);

tool!(
    ADD_FIELD,
    "add_field",
    "Add Field",
    "Fields",
    "Adds a field to a geodatabase table or feature class (ArcGIS Pro Add Field).",
    Runner::Report(imp::add_field),
    [
        TABLE,
        text("field_name", "Field Name", true),
        Param {
            name: "field_type",
            label: "Field Type",
            kind: Kind::Choice(FIELD_TYPES),
            required: true,
            default: "LONG",
            help: ""
        },
        Param {
            name: "field_precision",
            label: "Field Precision",
            kind: Kind::Number,
            required: false,
            default: "",
            help: ""
        },
        Param {
            name: "field_scale",
            label: "Field Scale",
            kind: Kind::Number,
            required: false,
            default: "",
            help: ""
        },
        Param {
            name: "field_length",
            label: "Field Length",
            kind: Kind::Number,
            required: false,
            default: "",
            help: "Text fields: 255 when not given."
        },
        text("field_alias", "Field Alias", false),
        choice(
            "field_is_nullable",
            "Field supports null values",
            &["NULLABLE", "NON_NULLABLE"],
            "NULLABLE"
        ),
        choice(
            "field_is_required",
            "Field is required",
            &["NON_REQUIRED", "REQUIRED"],
            "NON_REQUIRED"
        ),
        text("field_domain", "Field Domain", false),
    ]
);

tool!(
    DELETE_FIELD,
    "delete_field",
    "Delete Field",
    "Fields",
    "Deletes fields (separated by `;`) from a geodatabase table, or keeps only them (ArcGIS Pro Delete Field).",
    Runner::Report(imp::delete_field),
    [
        TABLE,
        text("drop_field", "Fields", true),
        choice(
            "method",
            "Method",
            &["DELETE_FIELDS", "KEEP_FIELDS"],
            "DELETE_FIELDS"
        ),
    ]
);

tool!(
    ALTER_FIELD,
    "alter_field",
    "Alter Field",
    "Fields",
    "Renames a field or changes its alias, type, length or nullability (ArcGIS Pro Alter Field).",
    Runner::Report(imp::alter_field),
    [
        TABLE,
        text("field", "Field Name", true),
        text("new_field_name", "New Field Name", false),
        text("new_field_alias", "New Field Alias", false),
        choice("field_type", "New Field Type", FIELD_TYPES, ""),
        Param {
            name: "field_length",
            label: "New Field Length",
            kind: Kind::Number,
            required: false,
            default: "",
            help: ""
        },
        choice(
            "field_is_nullable",
            "New Field IsNullable",
            &["NULLABLE", "NON_NULLABLE"],
            ""
        ),
        choice(
            "clear_field_alias",
            "Clear Alias",
            &["DO_NOT_CLEAR", "CLEAR_ALIAS"],
            "DO_NOT_CLEAR"
        ),
    ]
);

tool!(
    CREATE_RELATIONSHIP_CLASS,
    "create_relationship_class",
    "Create Relationship Class",
    "Relationship Classes",
    "A relationship class between two tables of a geodatabase (ArcGIS Pro Create Relationship Class).",
    Runner::Report(imp::create_relationship_class),
    [
        path("origin_table", "Origin Table", ""),
        path("destination_table", "Destination Table", ""),
        path(
            "out_relationship_class",
            "Output Relationship Class",
            "city.gdb/name"
        ),
        choice(
            "relationship_type",
            "Relationship Type",
            &["SIMPLE", "COMPOSITE"],
            "SIMPLE"
        ),
        text("forward_label", "Forward Path Label", true),
        text("backward_label", "Backward Path label", true),
        choice(
            "message_direction",
            "Message Direction",
            &["NONE", "FORWARD", "BACKWARD", "BOTH"],
            "NONE"
        ),
        choice(
            "cardinality",
            "Cardinality",
            &["ONE_TO_ONE", "ONE_TO_MANY", "MANY_TO_MANY"],
            "ONE_TO_ONE"
        ),
        choice(
            "attributed",
            "Relationship class is attributed",
            &["NONE", "ATTRIBUTED"],
            "NONE"
        ),
        text("origin_primary_key", "Origin Primary Key", true),
        text("origin_foreign_key", "Origin Foreign Key", true),
        text("destination_primary_key", "Destination Primary Key", false),
        text("destination_foreign_key", "Destination Foreign Key", false),
    ]
);
