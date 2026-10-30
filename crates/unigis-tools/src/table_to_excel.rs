//! Table To Excel: native XLSX workbook output, with schema observed in ArcGIS Pro.
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};
use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use unigis_core::{Error, Result, Value};
use zip::write::SimpleFileOptions;

use crate::registry::{Context, Kind, Param, Report, Runner, Tool};

pub const TOOL: Tool = Tool {
    id: "table_to_excel",
    name: "Table To Excel",
    toolbox: "Conversion › Excel",
    summary: "Writes one or more tables as worksheets in an Excel workbook.",
    params: &[
        Param {
            name: "input_table",
            label: "Input Table",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "Semicolon-separated table, feature class or CSV paths; optional single quotes around each path.",
        },
        Param {
            name: "output_excel_file",
            label: "Output Excel File",
            kind: Kind::Text,
            required: true,
            default: "",
            help: "The output .xlsx workbook.",
        },
        Param {
            name: "use_field_alias_as_column_header",
            label: "Use Field Alias as Column Header",
            kind: Kind::Bool,
            required: false,
            default: "false",
            help: "Use field aliases instead of field names for worksheet headers.",
        },
        Param {
            name: "use_domain_and_subtype_description",
            label: "Use Domain and Subtype Description",
            kind: Kind::Bool,
            required: false,
            default: "false",
            help: "Write coded-domain and subtype descriptions instead of their codes.",
        },
    ],
    output_suffix: "TableToExcel",
    measures: false,
    run: Runner::Report(run),
};

#[derive(Debug, Clone)]
pub struct ExcelSheet {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

fn run(ctx: &dyn Context) -> Result<Report> {
    let input = ctx.get("input_table").unwrap_or_default();
    let inputs: Vec<String> = input
        .split(';')
        .map(|p| p.trim().trim_matches('\'').to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    if inputs.is_empty() {
        return Err(Error::Unsupported(
            "ERROR 000735: Input Table: Value is required".into(),
        ));
    }
    let output = ctx.get("output_excel_file").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Output Excel File: Value is required".into())
    })?;
    let flag = |name: &str| -> Result<bool> {
        match ctx
            .get(name)
            .unwrap_or_else(|| "false".into())
            .to_ascii_uppercase()
            .as_str()
        {
            "TRUE" | "1" => Ok(true),
            "FALSE" | "0" => Ok(false),
            _ => Err(Error::Unsupported(format!(
                "ERROR 000800: {name} must be true or false"
            ))),
        }
    };
    let names = ctx.export_excel(
        &inputs,
        &output,
        flag("use_field_alias_as_column_header")?,
        flag("use_domain_and_subtype_description")?,
    )?;
    Ok(Report {
        rows: vec![("Worksheets written".into(), names.len() as f64)],
        messages: names,
    })
}

pub fn write_workbook(path: &Path, sheets: &[ExcelSheet]) -> Result<()> {
    if sheets.is_empty() {
        return Err(Error::Unsupported(
            "an Excel workbook needs a worksheet".into(),
        ));
    }
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut part = |name: &str, xml: &str| -> Result<()> {
            zip.start_file(name, options)
                .map_err(|e| Error::Unsupported(format!("cannot write XLSX part {name}: {e}")))?;
            zip.write_all(xml.as_bytes())
                .map_err(|e| Error::io(path, e))
        };
        part("[Content_Types].xml", &content_types(sheets))?;
        part("_rels/.rels", ROOT_RELS)?;
        part("xl/workbook.xml", &workbook_xml(sheets)?)?;
        part("xl/_rels/workbook.xml.rels", &workbook_rels(sheets))?;
        part("xl/styles.xml", STYLES)?;
        for (i, sheet) in sheets.iter().enumerate() {
            part(
                &format!("xl/worksheets/sheet{}.xml", i + 1),
                &worksheet_xml(sheet)?,
            )?;
        }
        zip.finish()
            .map_err(|e| Error::Unsupported(format!("cannot finish XLSX workbook: {e}")))?;
    }
    std::fs::write(path, bytes.into_inner()).map_err(|e| Error::io(path, e))
}

fn content_types(sheets: &[ExcelSheet]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    for i in 1..=sheets.len() {
        write!(
            xml,
            r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        )
        .unwrap();
    }
    xml.push_str("</Types>");
    xml
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

fn workbook_xml(sheets: &[ExcelSheet]) -> Result<String> {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>"#,
    );
    for (i, sheet) in sheets.iter().enumerate() {
        write!(xml, r#"<sheet name=""#).unwrap();
        escape(&mut xml, &sheet.name)?;
        write!(xml, r#"" sheetId="{}" r:id="rId{}"/>"#, i + 1, i + 1).unwrap();
    }
    xml.push_str("</sheets></workbook>");
    Ok(xml)
}

fn workbook_rels(sheets: &[ExcelSheet]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for i in 1..=sheets.len() {
        write!(
            xml,
            r#"<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>"#
        )
        .unwrap();
    }
    write!(
        xml,
        r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
        sheets.len() + 1
    )
    .unwrap();
    xml
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="3"><numFmt numFmtId="164" formatCode="yyyy-mm-dd h:mm:ss"/><numFmt numFmtId="165" formatCode="yyyy-mm-dd"/><numFmt numFmtId="166" formatCode="h:mm:ss"/></numFmts><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="5"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="165" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="166" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#;

fn worksheet_xml(sheet: &ExcelSheet) -> Result<String> {
    if sheet.headers.len() > 16_384 || sheet.rows.len() >= 1_048_576 {
        return Err(Error::Unsupported(
            "table exceeds the Excel worksheet row or column limit".into(),
        ));
    }
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1">"#,
    );
    for (col, header) in sheet.headers.iter().enumerate() {
        start_cell(&mut xml, 1, col, Some(1), "inlineStr");
        xml.push_str("<is><t xml:space=\"preserve\">");
        escape(&mut xml, header)?;
        xml.push_str("</t></is></c>");
    }
    xml.push_str("</row>");
    for (row, values) in sheet.rows.iter().enumerate() {
        if values.len() != sheet.headers.len() {
            return Err(Error::Unsupported(format!(
                "worksheet {} has a row with the wrong field count",
                sheet.name
            )));
        }
        write!(xml, r#"<row r="{}">"#, row + 2).unwrap();
        for (col, value) in values.iter().enumerate() {
            write_value(&mut xml, row + 2, col, value)?;
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    Ok(xml)
}

fn start_cell(xml: &mut String, row: usize, col: usize, style: Option<u8>, kind: &str) {
    write!(xml, r#"<c r="{}{row}""#, column_name(col)).unwrap();
    if let Some(style) = style {
        write!(xml, r#" s="{style}""#).unwrap();
    }
    write!(xml, r#" t="{kind}">"#).unwrap();
}

fn write_value(xml: &mut String, row: usize, col: usize, value: &Value) -> Result<()> {
    let number = |xml: &mut String, value: f64, style: Option<u8>| -> Result<()> {
        if !value.is_finite() {
            return Err(Error::Unsupported(
                "nonfinite numbers cannot be written to Excel".into(),
            ));
        }
        start_cell(xml, row, col, style, "n");
        write!(xml, "<v>{value}</v></c>").unwrap();
        Ok(())
    };
    match value {
        Value::Null => {}
        Value::SmallInteger(v) => number(xml, f64::from(*v), None)?,
        Value::Integer(v) => number(xml, f64::from(*v), None)?,
        Value::BigInteger(v) => number(xml, *v as f64, None)?,
        Value::Single(v) => number(xml, f64::from(*v), None)?,
        Value::Double(v) => number(xml, *v, None)?,
        Value::Date(v) => number(xml, datetime_serial(*v), Some(2))?,
        Value::DateOnly(v) => number(xml, date_serial(*v), Some(3))?,
        Value::TimeOnly(v) => number(xml, time_serial(*v), Some(4))?,
        Value::String(v) | Value::Guid(v) | Value::Xml(v) => {
            start_cell(xml, row, col, None, "inlineStr");
            xml.push_str("<is><t xml:space=\"preserve\">");
            escape(xml, v)?;
            xml.push_str("</t></is></c>");
        }
        Value::TimestampOffset(v) => {
            start_cell(xml, row, col, None, "inlineStr");
            xml.push_str("<is><t xml:space=\"preserve\">");
            escape(xml, &v.to_rfc3339())?;
            xml.push_str("</t></is></c>");
        }
        Value::Blob(_) => {
            return Err(Error::Unsupported(
                "BLOB fields cannot be written to Excel".into(),
            ));
        }
    }
    Ok(())
}

fn column_name(mut col: usize) -> String {
    let mut out = Vec::new();
    col += 1;
    while col > 0 {
        col -= 1;
        out.push(b'A' + (col % 26) as u8);
        col /= 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

fn datetime_serial(value: NaiveDateTime) -> f64 {
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let elapsed = value.signed_duration_since(epoch);
    let seconds = elapsed.num_seconds();
    let fraction = (elapsed - chrono::Duration::seconds(seconds))
        .num_nanoseconds()
        .unwrap_or(0) as f64
        / 1_000_000_000.0;
    (seconds as f64 + fraction) / 86_400.0
}

fn date_serial(value: NaiveDate) -> f64 {
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
    value.signed_duration_since(epoch).num_days() as f64
}

fn time_serial(value: NaiveTime) -> f64 {
    (f64::from(value.num_seconds_from_midnight()) + f64::from(value.nanosecond()) / 1e9) / 86_400.0
}

fn escape(xml: &mut String, text: &str) -> Result<()> {
    for c in text.chars() {
        match c {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '"' => xml.push_str("&quot;"),
            '\'' => xml.push_str("&apos;"),
            '\t' | '\n' | '\r' => xml.push(c),
            c if c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}' => xml.push(c),
            _ => {
                return Err(Error::Unsupported(
                    "text contains a character XML 1.0 cannot represent".into(),
                ));
            }
        }
    }
    Ok(())
}
