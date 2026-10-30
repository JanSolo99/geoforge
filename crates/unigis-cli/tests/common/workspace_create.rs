//! Create Folder / File Geodatabase / Mobile Geodatabase, run in a copy of their fixture folder:
//! the folder's listing afterwards, the derived output and the new workspace's (empty) contents.
use serde_json::{Value as Json, json};
use std::path::Path;
use std::process::Command;
use unigis_filegdb::Geodatabase;

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target)
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every entry below `root` (folders end in `/`), not looking inside `.gdb` folders.
fn listing(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            out.push(format!("{rel}/"));
            if !rel.to_lowercase().ends_with(".gdb") {
                listing(root, &p, out);
            }
        } else {
            out.push(rel);
        }
    }
}

/// What ArcGIS's Describe gave for the new workspace: its children and workspace factory.
fn describe(path: &Path) -> Json {
    if path.extension().is_some_and(|e| e == "gdb") {
        let gdb = Geodatabase::open(path).unwrap();
        let mut children: Vec<String> = gdb.entries().iter().map(|e| e.name.clone()).collect();
        children.extend(gdb.feature_datasets().iter().cloned());
        json!({"children": children, "factory": "esriDataSourcesGDB.FileGDBWorkspaceFactory"})
    } else {
        let db = unigis_gpkg::sqlite::Database::open(path).unwrap();
        let items = db.select(db.table("GDB_Items").unwrap()).unwrap();
        // Only the root folder, the workspace and the DEFAULT historical marker.
        let names: Vec<String> = items
            .iter()
            .filter_map(|(_, row)| row.get(3).and_then(|v| v.as_str()).map(str::to_owned))
            .filter(|n| !["", "Workspace", "DEFAULT"].contains(&n.as_str()))
            .collect();
        json!({"children": names, "factory": "esriDataSourcesGDB.SqliteWorkspaceFactory"})
    }
}

pub fn compare(case: &str, info: &Json, root: &Path, out_dir: &Path) -> Vec<String> {
    let reference = &info["workspace_create"];
    let fixture = root.join(reference["source"].as_str().unwrap());
    let temp = out_dir.join(format!("{case}-workspace"));
    if temp.exists() {
        std::fs::remove_dir_all(&temp).unwrap();
    }
    copy(&fixture, &temp);
    let place = |s: &str| s.replace("<workspace>", &temp.display().to_string());
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ug"));
    cmd.args(["run", info["run"]["tool"].as_str().unwrap()]);
    for (name, value) in info["run"]["params"].as_object().unwrap() {
        cmd.arg(format!("{name}={}", place(value.as_str().unwrap())));
    }
    let result = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);
    let mut diffs = Vec::new();
    if info["status"] == "failed" {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let codes: Vec<&str> = info["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m.as_str()?.strip_prefix("ERROR ")?.get(..6))
            .collect();
        if result.status.success() || !codes.iter().any(|c| stderr.contains(c)) {
            diffs.push(format!("expected ArcGIS's ERROR {codes:?}, got {result:?}"));
        }
    } else if !result.status.success() {
        diffs.push(format!("did not succeed: {result:?}"));
    } else if let Some(output) = reference["output"].as_str() {
        let output = place(output);
        if !stdout.lines().any(|l| l.ends_with(&format!(": {output}"))) {
            diffs.push(format!("derived output: ours {stdout:?}; ArcGIS {output}"));
        }
        if let Some(expected) = reference["expected"]["workspace"].as_object() {
            let ours = describe(Path::new(&output));
            if ours != Json::Object(expected.clone()) {
                diffs.push(format!(
                    "workspace differs: ours {ours}; ArcGIS {}",
                    reference["expected"]["workspace"]
                ));
            }
        }
    }
    let mut entries = Vec::new();
    listing(&temp, &temp, &mut entries);
    entries.sort();
    if json!(entries) != reference["expected"]["entries"] {
        diffs.push(format!(
            "folder differs: ours {entries:?}; ArcGIS {}",
            reference["expected"]["entries"]
        ));
    }
    std::fs::remove_dir_all(temp).unwrap();
    diffs
}
