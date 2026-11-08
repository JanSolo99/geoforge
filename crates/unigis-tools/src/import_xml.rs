//! Import XML Workspace Document: plain XML, standalone tables and simple 2D classes.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, Result};

pub const TOOL: Tool = Tool {
    id: "import_xml_workspace_document",
    name: "Import XML Workspace Document",
    toolbox: "Data Management › Workspace",
    summary: "Imports standalone tables and simple 2D feature classes from an XML workspace document into an existing FileGDB.",
    params: &[
        Param {
            name: "target_geodatabase",
            label: "Target Geodatabase",
            kind: Kind::Path,
            required: true,
            default: "",
            help: "Existing file geodatabase.",
        },
        Param {
            name: "in_file",
            label: "Import File",
            kind: Kind::Path,
            required: true,
            default: "",
            help: "Uncompressed XML workspace document.",
        },
        Param {
            name: "import_type",
            label: "Import Options",
            kind: Kind::Choice(&["DATA", "SCHEMA_ONLY"]),
            required: false,
            default: "DATA",
            help: "Import schema and records, or schema only.",
        },
        Param {
            name: "config_keyword",
            label: "Configuration Keyword",
            kind: Kind::Text,
            required: false,
            default: "",
            help: "Only empty or DEFAULTS is supported.",
        },
    ],
    output_suffix: "imported",
    measures: false,
    run: Runner::Report(run),
};

fn err(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}
fn run(ctx: &dyn Context) -> Result<Report> {
    let required = |n| {
        ctx.get(n)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| err(format!("ERROR 000735: {n}: Value is required")))
    };
    let target = required("target_geodatabase")?;
    let file = required("in_file")?;
    let mode = ctx.get("import_type").unwrap_or_else(|| "DATA".into());
    if !mode.eq_ignore_ascii_case("DATA") && !mode.eq_ignore_ascii_case("SCHEMA_ONLY") {
        return Err(err("ERROR 000800: import_type must be DATA or SCHEMA_ONLY"));
    }
    if ctx
        .get("config_keyword")
        .is_some_and(|k| !k.is_empty() && !k.eq_ignore_ascii_case("DEFAULTS"))
    {
        return Err(err("only the DEFAULTS configuration keyword is supported"));
    }
    import(&target, &file, mode.eq_ignore_ascii_case("SCHEMA_ONLY"))
}

#[cfg(not(feature = "gdb"))]
fn import(_: &str, _: &str, _: bool) -> Result<Report> {
    Err(err("XML workspace imports are unavailable in this build"))
}
#[cfg(feature = "gdb")]
use imp::import;

#[cfg(feature = "gdb")]
mod imp {
    use super::*;
    use roxmltree::{Document, Node};
    use std::{collections::HashSet, path::Path};
    use unigis_core::{Envelope, Feature, Field, FieldType, Geometry, GeometryKind, Value};
    use unigis_filegdb::{GeodatabaseWriter, GeometryDef};

    const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
    struct Dataset {
        name: String,
        fields: Vec<Field>,
        kind: Option<GeometryKind>,
        def: Option<GeometryDef>,
        rows: Vec<Feature>,
    }
    fn child<'a>(n: Node<'a, 'a>, tag: &str) -> Option<Node<'a, 'a>> {
        n.children()
            .find(|c| c.is_element() && c.tag_name().name() == tag)
    }
    fn need<'a>(n: Node<'a, 'a>, tag: &str) -> Result<Node<'a, 'a>> {
        child(n, tag).ok_or_else(|| err(format!("XML workspace: missing {tag}")))
    }
    fn text<'a>(n: Node<'a, 'a>, tag: &str) -> &'a str {
        child(n, tag).and_then(|c| c.text()).unwrap_or("")
    }
    fn elements<'a>(n: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
        n.children().filter(Node::is_element)
    }
    fn flag(n: Node, tag: &str) -> bool {
        text(n, tag).trim() == "true"
    }
    fn populated(n: Node, tag: &str) -> bool {
        child(n, tag).is_some_and(|c| {
            elements(c).next().is_some() || !c.text().unwrap_or("").trim().is_empty()
        })
    }
    fn number<T: std::str::FromStr>(raw: &str) -> Result<T> {
        raw.trim()
            .parse()
            .map_err(|_| err(format!("XML workspace: invalid number {raw:?}")))
    }
    fn finite(n: Node, tag: &str) -> Result<f64> {
        let v: f64 = number(text(n, tag))?;
        if !v.is_finite() {
            return Err(err(format!("XML workspace: nonfinite {tag}")));
        }
        Ok(v)
    }
    fn shape_kind(s: &str) -> Result<GeometryKind> {
        match s {
            "esriGeometryPoint" => Ok(GeometryKind::Point),
            "esriGeometryMultipoint" => Ok(GeometryKind::Multipoint),
            "esriGeometryPolyline" => Ok(GeometryKind::Polyline),
            "esriGeometryPolygon" => Ok(GeometryKind::Polygon),
            _ => Err(err(format!("unsupported XML geometry {s}"))),
        }
    }
    fn geometry_def(n: Node) -> Result<GeometryDef> {
        let xy_scale = finite(n, "XYScale")?;
        let xy_tolerance = finite(n, "XYTolerance")?;
        if xy_scale <= 0.0 || xy_tolerance < 0.0 || !flag(n, "HighPrecision") {
            return Err(err(
                "XML workspace: invalid or unsupported coordinate precision",
            ));
        }
        Ok(GeometryDef {
            srs_wkt: Some(text(n, "WKT").to_owned()).filter(|s| !s.is_empty()),
            has_z: false,
            has_m: false,
            x_origin: finite(n, "XOrigin")?,
            y_origin: finite(n, "YOrigin")?,
            xy_scale,
            xy_tolerance,
            z_origin: -100000.0,
            z_scale: 10000.0,
            z_tolerance: 0.001,
            m_origin: -100000.0,
            m_scale: 10000.0,
            m_tolerance: 0.001,
            extent: Envelope::EMPTY,
            grid_sizes: vec![0.0],
            extent_at: None,
            grid_at: None,
        })
    }
    fn field(n: Node) -> Result<Field> {
        let typ = text(n, "Type");
        let t = unigis_filegdb::schema::esri_field_type(typ)
            .ok_or_else(|| err(format!("unsupported XML field {typ}")))?;
        if !matches!(
            t,
            FieldType::SmallInteger
                | FieldType::Integer
                | FieldType::Single
                | FieldType::Double
                | FieldType::String
                | FieldType::Date
                | FieldType::Guid
                | FieldType::ObjectId
                | FieldType::Geometry
        ) {
            return Err(err(format!("unsupported XML field {typ}")));
        }
        let mut f = Field::new(text(n, "Name"), t);
        unigis_filegdb::writer::validate_field_name(&f.name)?;
        f.alias = Some(text(n, "AliasName").to_owned()).filter(|s| !s.is_empty());
        f.nullable = flag(n, "IsNullable");
        f.length = number(text(n, "Length"))?;
        f.precision = number(text(n, "Precision"))?;
        f.scale = number(text(n, "Scale"))?;
        if populated(n, "Domain")
            || flag(n, "HighPrecision")
            || (t == FieldType::Date && f.precision != 0)
        {
            return Err(err(
                "XML field domains and high precision dates are unsupported",
            ));
        }
        if let Some(v) = child(n, "DefaultValue") {
            if !matches!(
                t,
                FieldType::SmallInteger
                    | FieldType::Integer
                    | FieldType::Single
                    | FieldType::Double
                    | FieldType::String
            ) {
                return Err(err("unsupported XML field default type"));
            }
            f.default = Some(value(v, t)?);
        }
        Ok(f)
    }
    fn value(n: Node, t: FieldType) -> Result<Value> {
        if n.attribute((XSI, "nil")) == Some("true") {
            return Ok(Value::Null);
        }
        let raw = n.text().unwrap_or("");
        Ok(match t {
            FieldType::SmallInteger => Value::SmallInteger(number(raw)?),
            FieldType::Integer => Value::Integer(number(raw)?),
            FieldType::Single => {
                let v: f32 = number(raw)?;
                if !v.is_finite() {
                    return Err(err("nonfinite XML field value"));
                }
                Value::Single(v)
            }
            FieldType::Double => {
                let v: f64 = number(raw)?;
                if !v.is_finite() {
                    return Err(err("nonfinite XML field value"));
                }
                Value::Double(v)
            }
            FieldType::String => Value::String(raw.to_owned()),
            FieldType::Guid => {
                let v = raw.trim();
                if v.len() != 38
                    || !v.starts_with('{')
                    || !v.ends_with('}')
                    || !v[1..37].chars().enumerate().all(|(i, c)| {
                        if [8, 13, 18, 23].contains(&i) {
                            c == '-'
                        } else {
                            c.is_ascii_hexdigit()
                        }
                    })
                {
                    return Err(err("invalid XML GUID"));
                }
                Value::Guid(v.to_owned())
            }
            FieldType::Date => Value::Date(
                chrono::NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%dT%H:%M:%S%.f")
                    .map_err(|_| err("invalid XML date"))?,
            ),
            _ => return Err(err("unsupported XML value type")),
        })
    }
    // XML Schema base64 permits whitespace between groups, as in ArcGIS's exports.
    fn base64(raw: &str) -> Result<Vec<u8>> {
        let chars: Vec<u8> = raw.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
        if !chars.len().is_multiple_of(4) {
            return Err(err("invalid XML geometry base64 length"));
        }
        let mut out = Vec::new();
        for (i, block) in chars.as_chunks::<4>().0.iter().enumerate() {
            let mut v = [0u8; 4];
            let mut pads = 0;
            for (j, &c) in block.iter().enumerate() {
                v[j] = match c {
                    b'A'..=b'Z' => c - b'A',
                    b'a'..=b'z' => c - b'a' + 26,
                    b'0'..=b'9' => c - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    b'=' if j >= 2 && i + 1 == chars.len() / 4 => {
                        pads += 1;
                        0
                    }
                    _ => return Err(err("invalid XML geometry base64")),
                };
                if pads > 0 && c != b'=' {
                    return Err(err("invalid XML geometry padding"));
                }
            }
            if (pads == 2 && v[1] & 15 != 0) || (pads == 1 && v[2] & 3 != 0) {
                return Err(err("invalid XML geometry padding bits"));
            }
            out.push((v[0] << 2) | (v[1] >> 4));
            if pads < 2 {
                out.push((v[1] << 4) | (v[2] >> 2));
            }
            if pads == 0 {
                out.push((v[2] << 6) | v[3]);
            }
        }
        Ok(out)
    }
    fn geometry(n: Node, kind: GeometryKind) -> Result<Option<Geometry>> {
        if n.attribute((XSI, "nil")) == Some("true") {
            return Ok(None);
        }
        let (name, code) = match kind {
            GeometryKind::Point => ("Point", 1),
            GeometryKind::Multipoint => ("Multipoint", 8),
            GeometryKind::Polyline => ("Polyline", 3),
            GeometryKind::Polygon => ("Polygon", 5),
            _ => return Err(err("unsupported XML geometry")),
        };
        let typ = n
            .attribute((XSI, "type"))
            .unwrap_or("")
            .rsplit(':')
            .next()
            .unwrap_or("");
        if typ == format!("{name}B") {
            let bytes = base64(text(n, "Bytes"))?;
            let c = bytes
                .get(..4)
                .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| err("truncated XML geometry"))?;
            if c != 0 && c != code {
                return Err(err(
                    "XML shape type differs from schema; Z/M/curves unsupported",
                ));
            }
            let g = unigis_shapefile::shp::decode_record(&bytes)?;
            let mut encoded = Vec::new();
            unigis_shapefile::shp::encode_record(
                unigis_shapefile::ShapeType::from_code(c).unwrap(),
                g.as_ref(),
                &mut encoded,
            );
            if bytes.len() != encoded.len() {
                return Err(err("unexpected XML geometry payload"));
            }
            if g.as_ref()
                .is_some_and(|g| g.xy.iter().flatten().any(|v| !v.is_finite()))
            {
                return Err(err("nonfinite XML geometry"));
            }
            return Ok(g);
        }
        if typ != format!("{name}N") || flag(n, "HasZ") || flag(n, "HasM") || flag(n, "HasID") {
            return Err(err("unsupported XML geometry encoding or dimensions"));
        }
        if n.descendants()
            .any(|c| c.is_element() && c.tag_name().name() == "SegmentArray")
        {
            return Err(err("curved XML geometry is unsupported"));
        }
        let mut g = Geometry {
            kind,
            xy: Vec::new(),
            parts: Vec::new(),
            z: None,
            m: None,
            curves: Vec::new(),
            part_types: None,
        };
        let mut point = |p: Node| -> Result<()> {
            if populated(p, "Z") || populated(p, "M") {
                return Err(err("Z/M XML geometry is unsupported"));
            }
            g.xy.push([finite(p, "X")?, finite(p, "Y")?]);
            Ok(())
        };
        match kind {
            GeometryKind::Point => {
                point(n)?;
                g.parts.push(0);
            }
            GeometryKind::Multipoint => {
                for p in elements(need(n, "PointArray")?) {
                    point(p)?;
                }
            }
            _ => {
                let tag = if kind == GeometryKind::Polyline {
                    "PathArray"
                } else {
                    "RingArray"
                };
                // SegmentArray carries curves and is deliberately not flattened to vertices.
                for part in elements(need(n, tag)?) {
                    g.parts.push(g.xy.len() as u32);
                    for p in elements(need(part, "PointArray")?) {
                        if populated(p, "Z") || populated(p, "M") {
                            return Err(err("Z/M XML geometry is unsupported"));
                        }
                        g.xy.push([finite(p, "X")?, finite(p, "Y")?]);
                    }
                }
            }
        }
        Ok(Some(g))
    }
    fn parse(xml: &str, schema_only: bool) -> Result<Vec<Dataset>> {
        let doc = Document::parse(xml).map_err(|e| err(format!("invalid XML workspace: {e}")))?;
        let root = doc.root_element();
        if root.tag_name().name() != "Workspace" {
            return Err(err("input is not an XML workspace document"));
        }
        let wd = need(root, "WorkspaceDefinition")?;
        if populated(wd, "Domains") || populated(wd, "Sequences") {
            return Err(err("XML workspace domains and sequences are unsupported"));
        }
        let mut datasets = Vec::new();
        let mut names = HashSet::new();
        for n in elements(need(wd, "DatasetDefinitions")?) {
            let name = text(n, "Name").to_owned();
            unigis_filegdb::writer::validate_table_name(&name)?;
            if !names.insert(name.to_lowercase()) {
                return Err(err("duplicate XML dataset definition"));
            }
            let kind = match text(n, "DatasetType") {
                "esriDTTable" => None,
                "esriDTFeatureClass" if text(n, "FeatureType") == "esriFTSimple" => {
                    Some(shape_kind(text(n, "ShapeType"))?)
                }
                _ => {
                    return Err(err(
                        "only standalone XML tables and simple feature classes are supported",
                    ));
                }
            };
            if !flag(n, "HasOID")
                || text(n, "OIDFieldName") != "OBJECTID"
                || (kind.is_some() && text(n, "ShapeFieldName") != "Shape")
            {
                return Err(err(
                    "XML import requires standard OBJECTID and Shape fields",
                ));
            }
            for tag in [
                "Children",
                "Subtypes",
                "SubtypeFieldName",
                "RelationshipClassNames",
                "ControllerMemberships",
                "AttributeRules",
                "ExtensionProperties",
                "EXTCLSID",
            ] {
                if populated(n, tag)
                    && !(tag == "ExtensionProperties"
                        && child(n, tag)
                            .and_then(|v| child(v, "PropertyArray"))
                            .is_some_and(|v| elements(v).next().is_none()))
                {
                    return Err(err(format!("XML {tag} is unsupported")));
                }
            }
            if flag(n, "HasZ")
                || flag(n, "HasM")
                || flag(n, "HasGlobalID")
                || flag(n, "EditorTrackingEnabled")
                || flag(n, "ChangeTracked")
                || flag(n, "ReplicaTracked")
                || flag(n, "Versioned")
                || flag(n, "FieldFilteringEnabled")
            {
                return Err(err(
                    "XML Z/M, GlobalID, tracking and versioned schemas are unsupported",
                ));
            }
            if !text(n, "AliasName").is_empty() && text(n, "AliasName") != name {
                return Err(err("custom XML dataset aliases are unsupported"));
            }
            let all_fields: Vec<Field> = elements(need(need(n, "Fields")?, "FieldArray")?)
                .map(field)
                .collect::<Result<_>>()?;
            let mut field_names = HashSet::new();
            for f in &all_fields {
                if !field_names.insert(f.name.to_lowercase()) {
                    return Err(err("duplicate XML field"));
                }
            }
            if all_fields
                .iter()
                .filter(|f| f.field_type == FieldType::ObjectId)
                .count()
                != 1
                || all_fields
                    .iter()
                    .filter(|f| f.field_type == FieldType::Geometry)
                    .count()
                    != usize::from(kind.is_some())
            {
                return Err(err("XML fields differ from dataset schema"));
            }
            if all_fields.iter().any(|f| {
                (f.field_type == FieldType::ObjectId && f.name != "OBJECTID")
                    || (f.field_type == FieldType::Geometry && f.name != "Shape")
            }) {
                return Err(err(
                    "XML import requires standard OBJECTID and Shape fields",
                ));
            }
            for ix in n
                .descendants()
                .filter(|c| c.is_element() && c.tag_name().name() == "Index")
            {
                if !matches!(text(ix, "Name"), "FDO_OBJECTID" | "FDO_Shape") {
                    return Err(err("custom XML attribute indexes are unsupported"));
                }
            }
            let def = kind
                .map(|_| geometry_def(need(n, "SpatialReference")?))
                .transpose()?;
            let fields: Vec<_> = all_fields
                .iter()
                .filter(|f| !matches!(f.field_type, FieldType::ObjectId | FieldType::Geometry))
                .cloned()
                .collect();
            let mut rows = Vec::new();
            if !schema_only && let Some(data) = child(root, "WorkspaceData") {
                let records: Vec<_> = elements(data)
                    .filter(|d| text(*d, "DatasetName") == name)
                    .collect();
                if records.len() > 1 {
                    return Err(err("duplicate XML dataset records"));
                }
                if let Some(records) = records.first() {
                    let data = need(*records, "Data")?;
                    let data_fields: Vec<Field> =
                        elements(need(need(data, "Fields")?, "FieldArray")?)
                            .map(field)
                            .collect::<Result<_>>()?;
                    if all_fields != data_fields {
                        return Err(err("XML record fields differ from dataset schema"));
                    }
                    for r in elements(need(data, "Records")?) {
                        let vals: Vec<_> = elements(need(r, "Values")?).collect();
                        if vals.len() != all_fields.len() {
                            return Err(err("XML row has the wrong number of values"));
                        }
                        let mut row = Feature {
                            oid: 0,
                            geometry: None,
                            attributes: Vec::new(),
                        };
                        for (f, v) in all_fields.iter().zip(vals) {
                            match f.field_type {
                                FieldType::ObjectId => {
                                    row.oid = number(v.text().unwrap_or(""))?;
                                    if !(1..=16_777_216).contains(&row.oid) {
                                        return Err(err(
                                            "XML OBJECTID outside supported range 1..16777216",
                                        ));
                                    }
                                }
                                FieldType::Geometry => {
                                    row.geometry = geometry(
                                        v,
                                        kind.ok_or_else(|| err("geometry in XML table"))?,
                                    )?
                                }
                                _ => {
                                    let val = value(v, f.field_type)?;
                                    if val.is_null() && !f.nullable {
                                        return Err(err("null in nonnullable XML field"));
                                    }
                                    row.attributes.push(val);
                                }
                            }
                        }
                        if let (Some(g), Some(d)) = (&row.geometry, &def) {
                            for [x, y] in &g.xy {
                                for (v, origin) in [(*x, d.x_origin), (*y, d.y_origin)] {
                                    let scaled = ((v - origin) * d.xy_scale).round();
                                    if !(0.0..9.0e15).contains(&scaled) {
                                        return Err(err(
                                            "XML geometry lies outside its coordinate domain",
                                        ));
                                    }
                                }
                            }
                        }
                        rows.push(row);
                    }
                    rows.sort_by_key(|r| r.oid);
                    if rows.windows(2).any(|r| r[0].oid == r[1].oid) {
                        return Err(err("duplicate XML OBJECTID"));
                    }
                }
            }
            datasets.push(Dataset {
                name,
                fields,
                kind,
                def,
                rows,
            });
        }
        if !schema_only && let Some(data) = child(root, "WorkspaceData") {
            for d in elements(data) {
                if !names.contains(&text(d, "DatasetName").to_lowercase()) {
                    return Err(err("XML data has no supported schema definition"));
                }
            }
        }
        Ok(datasets)
    }
    pub(super) fn import(target: &str, file: &str, schema_only: bool) -> Result<Report> {
        let target = Path::new(target);
        if !target.is_dir()
            || !target
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
            || !target.join("a00000001.gdbtable").is_file()
        {
            return Err(err(
                "ERROR 000732: Target Geodatabase does not exist or is not a supported FileGDB",
            ));
        }
        let file = Path::new(file);
        if !file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
        {
            return Err(err(
                "only uncompressed .xml workspace documents are supported",
            ));
        }
        if std::fs::metadata(file)
            .map_err(|e| Error::io(file, e))?
            .len()
            > 64 * 1024 * 1024
        {
            return Err(Error::Limit("XML workspace exceeds 64 MiB".into()));
        }
        let xml = std::fs::read_to_string(file).map_err(|e| Error::io(file, e))?;
        // Preflight every dataset/record before opening a mutating transaction. The writer
        // also rolls back all changes if any later write or catalog update fails.
        let datasets = parse(&xml, schema_only)?;
        let mut w = GeodatabaseWriter::open_existing(target)?;
        w.use_standard_geometry_fields();
        let mut report = Report::default();
        for d in datasets {
            let mut name = d.name.clone();
            let mut suffix = 1;
            while w.dataset_name_exists(&name) {
                name = format!("{}_{suffix}", d.name);
                suffix += 1;
            }
            let mut t = match (d.kind, d.def) {
                (Some(kind), Some(def)) => {
                    w.create_feature_class_with_definition(&name, kind, &d.fields, def)?
                }
                _ => w.create_table(&name, None, false, false, &d.fields, None)?,
            };
            for row in &d.rows {
                t.write_with_oid(row)?;
            }
            w.finish_table(t)?;
            report.messages.push(format!("Imported {name}"));
        }
        w.finish()?;
        Ok(report)
    }
}
