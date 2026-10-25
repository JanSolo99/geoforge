//! Tools that are an existing command under another ArcGIS name (the Pairwise tools): `ug run
//! pairwise_buffer input=a.shp output=b.shp distance="25 Meters"` is rewritten to the command
//! line of `ug buffer`, so the tool needs no code of its own. The names and parameters are those
//! the ArcGIS reference script gives (`RUN` in `scripts/make-arcgis-reference.py`).

use std::collections::HashMap;

type Params = HashMap<String, String>;

/// The command line `ug` runs for `ug run <alias> name=value ...`, or `args` unchanged when the
/// tool is not an alias.
pub fn rewrite(args: Vec<String>) -> Vec<String> {
    if args.get(1).map(String::as_str) != Some("run") {
        return args;
    }
    let Some(tool) = args.get(2) else {
        return args;
    };
    let params: Params = args[3..]
        .iter()
        .filter_map(|a| a.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let build: fn(&Params) -> Option<Vec<String>> = match tool.as_str() {
        "pairwise_buffer" => buffer,
        "pairwise_dissolve" => dissolve,
        "pairwise_intersect" => intersect,
        "pairwise_clip" => |p| overlay("clip", "clip_features", p),
        "pairwise_erase" => |p| overlay("erase", "erase_features", p),
        _ => return args,
    };
    match build(&params) {
        Some(mut line) => {
            line.insert(0, args[0].clone());
            line
        }
        None => args,
    }
}

fn get<'a>(p: &'a Params, name: &str) -> Option<&'a str> {
    p.get(name).map(|v| v.trim()).filter(|v| !v.is_empty())
}

/// `--flag value` for every `;`-separated value of the parameter.
fn each(line: &mut Vec<String>, flag: &str, p: &Params, name: &str) {
    for v in get(p, name).unwrap_or_default().split(';') {
        if !v.trim().is_empty() {
            line.push(flag.into());
            line.push(v.trim().into());
        }
    }
}

fn option(line: &mut Vec<String>, flag: &str, p: &Params, name: &str) {
    if let Some(v) = get(p, name) {
        line.push(flag.into());
        line.push(v.into());
    }
}

fn buffer(p: &Params) -> Option<Vec<String>> {
    let mut line = vec![
        "buffer".to_string(),
        get(p, "input")?.into(),
        get(p, "output")?.into(),
        get(p, "distance")?.into(),
    ];
    option(&mut line, "--dissolve-option", p, "dissolve_option");
    each(&mut line, "--dissolve-field", p, "dissolve_field");
    option(&mut line, "--line-end-type", p, "line_end_type");
    line.extend(["--max-arc-angle".to_string(), "3.75".to_string()]);
    Some(line)
}

fn dissolve(p: &Params) -> Option<Vec<String>> {
    let mut line = vec![
        "dissolve".to_string(),
        get(p, "input")?.into(),
        get(p, "output")?.into(),
    ];
    each(&mut line, "--dissolve-field", p, "dissolve_field");
    option(&mut line, "--statistics-fields", p, "statistics_fields");
    option(&mut line, "--multi-part", p, "multi_part");
    Some(line)
}

fn intersect(p: &Params) -> Option<Vec<String>> {
    let mut line = vec!["intersect".to_string(), "--merge-pieces".to_string()];
    option(&mut line, "--join-attributes", p, "join_attributes");
    option(&mut line, "--output-type", p, "output_type");
    line.extend(get(p, "inputs")?.split(';').map(|s| s.trim().to_string()));
    line.push(get(p, "output")?.into());
    Some(line)
}

fn overlay(command: &str, second: &str, p: &Params) -> Option<Vec<String>> {
    Some(vec![
        command.to_string(),
        get(p, "input")?.into(),
        get(p, second)?.into(),
        get(p, "output")?.into(),
    ])
}
