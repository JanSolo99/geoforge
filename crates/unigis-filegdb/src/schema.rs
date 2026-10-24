//! Geodatabase schema stored as XML in `GDB_Items.Definition`: attribute domains,
//! subtypes, relationship classes and per-field properties.

use roxmltree::{Document, Node};
use unigis_core::{FieldType, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPolicy {
    DefaultValue,
    Duplicate,
    GeometryRatio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePolicy {
    DefaultValue,
    SumValues,
    AreaWeighted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DomainKind {
    /// (code, description) pairs, in definition order.
    Coded(Vec<(Value, String)>),
    Range {
        min: Value,
        max: Value,
    },
}

/// An attribute domain (`arcpy.da.ListDomains`).
#[derive(Debug, Clone, PartialEq)]
pub struct Domain {
    pub name: String,
    pub description: String,
    pub field_type: FieldType,
    pub split_policy: SplitPolicy,
    pub merge_policy: MergePolicy,
    pub kind: DomainKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtypeField {
    pub field: String,
    pub domain: Option<String>,
    pub default: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subtype {
    pub code: i64,
    pub name: String,
    pub fields: Vec<SubtypeField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subtypes {
    pub field: String,
    pub default_code: i64,
    pub subtypes: Vec<Subtype>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    OneToOne,
    OneToMany,
    ManyToMany,
}

/// A relationship class (`arcpy.Describe(...).cardinality`, `.originClassNames`, ...).
#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipClass {
    pub name: String,
    pub cardinality: Cardinality,
    pub is_composite: bool,
    pub is_attributed: bool,
    pub notification: String,
    pub origin: String,
    pub destination: String,
    /// Primary key in the origin class.
    pub origin_primary_key: String,
    /// Foreign key referencing the origin: in the destination class, or in the
    /// relationship table for many-to-many / attributed relationships.
    pub origin_foreign_key: String,
    /// Many-to-many / attributed only: primary key in the destination class.
    pub destination_primary_key: Option<String>,
    /// Many-to-many / attributed only: foreign key in the relationship table.
    pub destination_foreign_key: Option<String>,
    pub forward_label: String,
    pub backward_label: String,
}

/// Field-level properties from a table's definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FieldInfo {
    pub name: String,
    pub domain: Option<String>,
    pub high_precision: bool,
}

/// What a table definition contributes when the table is opened.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableDefinition {
    pub fields: Vec<FieldInfo>,
    pub subtypes: Option<Subtypes>,
}

/// One parsed `GDB_Items.Definition`.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemDefinition {
    Domain(Domain),
    Relationship(RelationshipClass),
    Table(TableDefinition),
    Other,
}

pub fn parse_definition(xml: &str) -> ItemDefinition {
    let Ok(doc) = Document::parse(xml) else {
        return ItemDefinition::Other;
    };
    let root = doc.root_element();
    match root.tag_name().name() {
        "GPCodedValueDomain2" | "GPRangeDomain2" => {
            parse_domain(root).map_or(ItemDefinition::Other, ItemDefinition::Domain)
        }
        "DERelationshipClassInfo" => {
            parse_relationship(root).map_or(ItemDefinition::Other, ItemDefinition::Relationship)
        }
        "DEFeatureClassInfo" | "DETableInfo" => ItemDefinition::Table(parse_table(root)),
        _ => ItemDefinition::Other,
    }
}

fn child<'a>(n: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    n.children().find(|c| c.tag_name().name() == name)
}

/// Element text, verbatim: ArcGIS preserves leading/trailing spaces in names and
/// descriptions (callers trim where the value is numeric or an identifier).
fn text(n: Node, name: &str) -> Option<String> {
    child(n, name).map(|c| c.text().unwrap_or_default().to_owned())
}

fn flag(n: Node, name: &str) -> bool {
    text(n, name).is_some_and(|t| t.trim().eq_ignore_ascii_case("true"))
}

pub fn esri_field_type(name: &str) -> Option<FieldType> {
    Some(match name {
        "esriFieldTypeSmallInteger" => FieldType::SmallInteger,
        "esriFieldTypeInteger" => FieldType::Integer,
        "esriFieldTypeBigInteger" => FieldType::BigInteger,
        "esriFieldTypeSingle" => FieldType::Single,
        "esriFieldTypeDouble" => FieldType::Double,
        "esriFieldTypeString" => FieldType::String,
        "esriFieldTypeDate" => FieldType::Date,
        "esriFieldTypeDateOnly" => FieldType::DateOnly,
        "esriFieldTypeTimeOnly" => FieldType::TimeOnly,
        "esriFieldTypeTimestampOffset" => FieldType::TimestampOffset,
        "esriFieldTypeOID" => FieldType::ObjectId,
        "esriFieldTypeGeometry" => FieldType::Geometry,
        "esriFieldTypeBlob" => FieldType::Blob,
        "esriFieldTypeRaster" => FieldType::Raster,
        "esriFieldTypeGUID" => FieldType::Guid,
        "esriFieldTypeGlobalID" => FieldType::GlobalId,
        "esriFieldTypeXML" => FieldType::Xml,
        _ => return None,
    })
}

/// Parses a domain code / range bound / default as the domain's field type.
pub(crate) fn typed_value(raw: &str, t: FieldType) -> Value {
    let raw = raw.trim();
    let parsed = match t {
        FieldType::SmallInteger => raw.parse().ok().map(Value::SmallInteger),
        FieldType::Integer => raw.parse().ok().map(Value::Integer),
        FieldType::BigInteger => raw.parse().ok().map(Value::BigInteger),
        FieldType::Single => raw.parse().ok().map(Value::Single),
        FieldType::Double => raw.parse().ok().map(Value::Double),
        FieldType::Date => chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f")
            .ok()
            .map(Value::Date),
        _ => None,
    };
    parsed.unwrap_or_else(|| Value::String(raw.to_owned()))
}

/// A domain from its item definition XML.
pub(crate) fn parse_domain_xml(xml: &str) -> Option<Domain> {
    let doc = Document::parse(xml).ok()?;
    parse_domain(doc.root_element())
}

fn parse_domain(root: Node) -> Option<Domain> {
    let field_type = esri_field_type(text(root, "FieldType")?.trim())?;
    let split_policy = match text(root, "SplitPolicy").as_deref() {
        Some("esriSPTDuplicate") => SplitPolicy::Duplicate,
        Some("esriSPTGeometryRatio") => SplitPolicy::GeometryRatio,
        _ => SplitPolicy::DefaultValue,
    };
    let merge_policy = match text(root, "MergePolicy").as_deref() {
        Some("esriMPTSumValues") => MergePolicy::SumValues,
        Some("esriMPTAreaWeighted") => MergePolicy::AreaWeighted,
        _ => MergePolicy::DefaultValue,
    };
    let kind = if root.tag_name().name() == "GPRangeDomain2" {
        DomainKind::Range {
            min: typed_value(&text(root, "MinValue")?, field_type),
            max: typed_value(&text(root, "MaxValue")?, field_type),
        }
    } else {
        let values = child(root, "CodedValues")?
            .children()
            .filter(|c| c.tag_name().name() == "CodedValue")
            .filter_map(|cv| {
                Some((
                    typed_value(&text(cv, "Code")?, field_type),
                    text(cv, "Name")?,
                ))
            })
            .collect();
        DomainKind::Coded(values)
    };
    Some(Domain {
        name: text(root, "DomainName")?,
        description: text(root, "Description").unwrap_or_default(),
        field_type,
        split_policy,
        merge_policy,
        kind,
    })
}

fn names(n: Node, list: &str) -> Vec<String> {
    child(n, list)
        .map(|l| {
            l.children()
                .filter(|c| c.tag_name().name() == "Name")
                .filter_map(|c| c.text().map(|t| t.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// `KeyRole → ObjectKeyName` from an `...ClassKeys` list.
fn keys(n: Node, list: &str) -> Vec<(String, String)> {
    child(n, list)
        .map(|l| {
            l.children()
                .filter(|c| c.tag_name().name() == "RelationshipClassKey")
                .filter_map(|k| Some((text(k, "KeyRole")?, text(k, "ObjectKeyName")?)))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_relationship(root: Node) -> Option<RelationshipClass> {
    let cardinality = match text(root, "Cardinality")?.trim() {
        "esriRelCardinalityOneToOne" => Cardinality::OneToOne,
        "esriRelCardinalityOneToMany" => Cardinality::OneToMany,
        "esriRelCardinalityManyToMany" => Cardinality::ManyToMany,
        _ => return None,
    };
    let mut all = keys(root, "OriginClassKeys");
    all.extend(keys(root, "DestinationClassKeys"));
    let key = |role: &str| all.iter().find(|(r, _)| r == role).map(|(_, k)| k.clone());
    Some(RelationshipClass {
        name: text(root, "Name")?,
        cardinality,
        is_composite: flag(root, "IsComposite"),
        is_attributed: flag(root, "IsAttributed"),
        notification: text(root, "Notification")
            .unwrap_or_default()
            .trim_start_matches("esriRelNotification")
            .to_owned(),
        origin: names(root, "OriginClassNames").into_iter().next()?,
        destination: names(root, "DestinationClassNames").into_iter().next()?,
        origin_primary_key: key("esriRelKeyRoleOriginPrimary")?,
        origin_foreign_key: key("esriRelKeyRoleOriginForeign")?,
        destination_primary_key: key("esriRelKeyRoleDestinationPrimary"),
        destination_foreign_key: key("esriRelKeyRoleDestinationForeign"),
        forward_label: text(root, "ForwardPathLabel").unwrap_or_default(),
        backward_label: text(root, "BackwardPathLabel").unwrap_or_default(),
    })
}

fn parse_table(root: Node) -> TableDefinition {
    let fields: Vec<FieldInfo> = child(root, "GPFieldInfoExs")
        .map(|l| {
            l.children()
                .filter(|c| c.tag_name().name() == "GPFieldInfoEx")
                .filter_map(|f| {
                    Some(FieldInfo {
                        name: text(f, "Name")?,
                        domain: text(f, "DomainName").filter(|d| !d.is_empty()),
                        high_precision: flag(f, "HighPrecision"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let field_type = |name: &str| {
        child(root, "GPFieldInfoExs").and_then(|l| {
            l.children()
                .filter(|c| c.tag_name().name() == "GPFieldInfoEx")
                .find(|f| text(*f, "Name").is_some_and(|n| n.eq_ignore_ascii_case(name)))
                .and_then(|f| esri_field_type(text(f, "FieldType")?.trim()))
        })
    };

    let subtypes = text(root, "SubtypeFieldName")
        .filter(|s| !s.is_empty())
        .map(|field| Subtypes {
            default_code: text(root, "DefaultSubtypeCode")
                .and_then(|c| c.parse().ok())
                .unwrap_or(0),
            subtypes: child(root, "Subtypes")
                .map(|l| {
                    l.children()
                        .filter(|c| c.tag_name().name() == "Subtype")
                        .filter_map(|s| {
                            let fields = child(s, "FieldInfos")
                                .map(|fi| {
                                    fi.children()
                                        .filter(|c| c.tag_name().name() == "SubtypeFieldInfo")
                                        .filter_map(|i| {
                                            let name = text(i, "FieldName")?;
                                            let t = field_type(&name).unwrap_or(FieldType::String);
                                            Some(SubtypeField {
                                                domain: text(i, "DomainName")
                                                    .filter(|d| !d.is_empty()),
                                                default: text(i, "DefaultValue")
                                                    .map(|v| typed_value(&v, t)),
                                                field: name,
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            Some(Subtype {
                                code: text(s, "SubtypeCode")?.parse().ok()?,
                                name: text(s, "SubtypeName")?,
                                fields,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            field,
        });
    TableDefinition { fields, subtypes }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Subtype XML as ArcGIS writes it in `DEFeatureClassInfo` (no sample in the
    /// GDAL test data, so this checks the documented structure).
    #[test]
    fn parses_subtypes() {
        let xml = r#"<DEFeatureClassInfo xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:typens="http://www.esri.com/schemas/ArcGIS/10.1" xsi:type="typens:DEFeatureClassInfo">
          <GPFieldInfoExs><GPFieldInfoEx><Name>RoadClass</Name><FieldType>esriFieldTypeInteger</FieldType></GPFieldInfoEx>
          <GPFieldInfoEx><Name>Speed</Name><FieldType>esriFieldTypeSmallInteger</FieldType><DomainName>SpeedLimit</DomainName></GPFieldInfoEx></GPFieldInfoExs>
          <SubtypeFieldName>RoadClass</SubtypeFieldName><DefaultSubtypeCode>1</DefaultSubtypeCode>
          <Subtypes><Subtype><SubtypeName>Highway</SubtypeName><SubtypeCode>1</SubtypeCode>
            <FieldInfos><SubtypeFieldInfo><FieldName>Speed</FieldName><DomainName>SpeedLimit</DomainName><DefaultValue xsi:type="xs:short">100</DefaultValue></SubtypeFieldInfo></FieldInfos></Subtype>
          <Subtype><SubtypeName>Local</SubtypeName><SubtypeCode>2</SubtypeCode></Subtype></Subtypes>
        </DEFeatureClassInfo>"#;
        let ItemDefinition::Table(t) = parse_definition(xml) else {
            panic!("not a table")
        };
        let s = t.subtypes.unwrap();
        assert_eq!(
            (s.field.as_str(), s.default_code, s.subtypes.len()),
            ("RoadClass", 1, 2)
        );
        assert_eq!(
            s.subtypes[0].fields[0].default,
            Some(Value::SmallInteger(100))
        );
        assert_eq!(t.fields[1].domain.as_deref(), Some("SpeedLimit"));
    }
}
