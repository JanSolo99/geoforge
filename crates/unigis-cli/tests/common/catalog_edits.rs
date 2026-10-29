//! Destructive catalog tools are tested only on copies of the saved source fixture.
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use unigis_core::{Feature, Field};
use unigis_filegdb::Geodatabase;

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target)
        } else if !entry.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
fn bytes(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            bytes(root, &p, out)
        } else {
            out.insert(
                p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                std::fs::read(p).unwrap(),
            );
        }
    }
}
fn snapshot(root: &Path) -> Json {
    let mut files = Vec::new();
    let mut catalogs = BTreeMap::new();
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            let gdb = Geodatabase::open(&p).unwrap();
            files.push(format!("{name}/"));
            let mut names: Vec<_> = gdb.entries().iter().map(|e| e.name.clone()).collect();
            names.extend(gdb.feature_datasets().iter().cloned());
            names.extend(gdb.relationship_classes().iter().map(|r| r.name.clone()));
            names.sort();
            catalogs.insert(name, names);
        } else {
            files.push(name);
        }
    }
    files.sort();
    json!({"files":files,"catalogs":catalogs})
}
type Rows = BTreeMap<String, (Vec<Field>, Vec<Feature>)>;
fn rows(root: &Path) -> Rows {
    let mut out = Rows::new();
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if !p.is_dir() {
            continue;
        }
        let gdb = Geodatabase::open(&p).unwrap();
        for e in gdb.entries() {
            let t = gdb.open_table(&e.name).unwrap();
            out.insert(
                format!("{}/{}", p.file_name().unwrap().to_string_lossy(), e.name),
                (
                    t.fields().to_vec(),
                    t.features().collect::<Result<Vec<_>, _>>().unwrap(),
                ),
            );
        }
    }
    out
}
pub fn compare(case: &str, info: &Json, root: &Path, out_dir: &Path) -> Vec<String> {
    let fixture = root.join(info["catalog_edit"]["source"].as_str().unwrap());
    let temp = out_dir.join(format!("{case}-workspace"));
    if temp.exists() {
        std::fs::remove_dir_all(&temp).unwrap();
    }
    copy(&fixture, &temp);
    let mut before = BTreeMap::new();
    bytes(&temp, &temp, &mut before);
    let old_rows = rows(&temp);
    let template = info["run"]["params"]
        .get("in_data")
        .and_then(Json::as_str)
        .unwrap_or("");
    assert!(
        template.is_empty() || template.contains("<workspace>"),
        "unisolated destructive reference input"
    );
    let actual_input = template.replace("<workspace>", &temp.display().to_string());
    if !actual_input.is_empty() {
        for item in unigis_tools::delete_multiple::parse(&actual_input).unwrap() {
            let path = Path::new(&item.path);
            assert!(
                path.starts_with(&temp)
                    && !path
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir)),
                "reference attempted to escape disposable workspace"
            );
        }
    }
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "delete_multiple"])
        .arg(format!(
            "in_data={}",
            template.replace("<workspace>", &temp.display().to_string())
        ))
        .output()
        .unwrap();
    let mut diffs = Vec::new();
    if info["status"] == "failed" {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let matches = info["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m.as_str()?.strip_prefix("ERROR ")?.get(..6))
            .any(|c| stderr.contains(c));
        if result.status.success() || !matches {
            diffs.push(format!("expected ArcGIS refusal, got {result:?}"));
        }
        let mut after = BTreeMap::new();
        bytes(&temp, &temp, &mut after);
        if before != after {
            diffs.push("invalid batch changed dataset bytes".into());
        }
    } else {
        if !result.status.success()
            || !String::from_utf8_lossy(&result.stdout).contains("out_results: true")
        {
            diffs.push(format!("delete did not succeed: {result:?}"));
        }
        let mut after = BTreeMap::new();
        bytes(&temp, &temp, &mut after);
        for (name, bytes) in &after {
            if !name.contains(".gdb/") && before.get(name) != Some(bytes) {
                diffs.push(format!("surviving file changed: {name}"));
            }
        }
        for (name, data) in rows(&temp) {
            if old_rows.get(&name) != Some(&data) {
                diffs.push(format!("surviving table changed: {name}"));
            }
        }
    }
    let actual = snapshot(&temp);
    if actual != info["catalog_edit"]["expected"] {
        diffs.push(format!(
            "catalog differs: ours {actual}; ArcGIS {}",
            info["catalog_edit"]["expected"]
        ));
    }
    std::fs::remove_dir_all(temp).unwrap();
    diffs
}
