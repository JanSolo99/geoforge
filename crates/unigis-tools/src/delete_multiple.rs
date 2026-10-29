//! Delete Multiple value-table parsing and host capability dispatch.
use crate::registry::{Context, Kind, Param, Report, Runner, Tool};
use unigis_core::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteItem {
    pub path: String,
    pub data_type: Option<String>,
}

/// Semicolon-separated value-table rows, with optional type in column two.
/// Single/double quoted paths can contain spaces and semicolons; doubled quotes
/// escape a quote. `#` is ArcGIS's omitted-value placeholder.
pub fn parse(text: &str) -> Result<Vec<DeleteItem>> {
    let mut rows = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut active = false;
    let mut quote = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                if chars.peek() == Some(&q) {
                    word.push(c);
                    chars.next();
                } else {
                    quote = None;
                }
            } else {
                word.push(c);
            }
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            active = true;
        } else if c == ';' || c.is_whitespace() {
            if active {
                words.push(std::mem::take(&mut word));
                active = false;
            }
            if c == ';' && !words.is_empty() {
                rows.push(std::mem::take(&mut words));
            }
        } else {
            word.push(c);
            active = true;
        }
    }
    if quote.is_some() {
        return Err(Error::Unsupported(
            "ERROR 000622: Unterminated quote in Input Data Element".into(),
        ));
    }
    if active {
        words.push(word);
    }
    if !words.is_empty() {
        rows.push(words);
    }
    if rows.is_empty() {
        return Err(Error::Unsupported(
            "ERROR 000735: Input Data Element: Value is required".into(),
        ));
    }
    rows.into_iter()
        .map(|r| {
            if r.len() > 2 || r[0].is_empty() || r[0] == "#" {
                return Err(Error::Unsupported(
                    "ERROR 000622: Input Data Element requires a path and optional data type"
                        .into(),
                ));
            }
            Ok(DeleteItem {
                path: r[0].clone(),
                data_type: r.get(1).filter(|s| !s.is_empty() && *s != "#").cloned(),
            })
        })
        .collect()
}

pub const TOOL: Tool = Tool {
    id: "delete_multiple",
    name: "Delete Multiple",
    toolbox: "Data Management › General",
    summary: "Deletes multiple datasets and their ancillary files, after validating every input.",
    params: &[Param {
        name: "in_data",
        label: "Input Data Element",
        kind: Kind::Text,
        required: true,
        default: "",
        help: "Semicolon-separated paths with optional data types; quote paths containing spaces.",
    }],
    output_suffix: "deleted",
    measures: false,
    run: Runner::Report(run),
};
fn run(ctx: &dyn Context) -> Result<Report> {
    let items = parse(&ctx.get("in_data").unwrap_or_default())?;
    ctx.delete_items(&items)?;
    Ok(Report {
        rows: vec![],
        messages: vec!["out_results: true".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use unigis_core::FeatureClass;
    #[test]
    fn value_tables_keep_literal_path_separators_quotes_and_types() {
        let items =
            parse("'/tmp/a;b/it''s data.shp' FeatureClass; \"/tmp/other file.dbf\" #; plain.gdb")
                .unwrap();
        assert_eq!(
            items[0],
            DeleteItem {
                path: "/tmp/a;b/it's data.shp".into(),
                data_type: Some("FeatureClass".into())
            }
        );
        assert_eq!(items[1].path, "/tmp/other file.dbf");
        assert_eq!(items[1].data_type, None);
        assert!(parse("'unterminated").is_err());
        assert!(parse("x y z").is_err());
    }
    #[test]
    fn read_only_host_refuses_instead_of_claiming_deletion() {
        struct Host;
        impl Context for Host {
            fn get(&self, _: &str) -> Option<String> {
                Some("data.shp".into())
            }
            fn features(&self, _: &str) -> Result<Box<dyn FeatureClass + '_>> {
                unreachable!()
            }
        }
        let Err(e) = crate::registry::run(&TOOL, &Host) else {
            panic!("must refuse")
        };
        assert!(
            e.to_string()
                .contains("dataset deletion is not supported by this host")
        );
    }
}
