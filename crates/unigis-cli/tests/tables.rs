//! Copy Rows, Table To Table, Join Field, Add Fields and Get Count (through `ug run`) against ArcGIS
//! Pro 3.7 (testdata/arcgis-reference/tables.json).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as Json;
use unigis_core::Value;
use unigis_shapefile::ShapefileReader;
use unigis_shapefile::dbf::DbfReader;

fn ogr(dir: &Path, name: &str, features: &str, ext: &str, geometry: bool) -> PathBuf {
    let json = dir.join(format!("{name}.geojson"));
    std::fs::write(
        &json,
        format!(r#"{{"type":"FeatureCollection","features":[{features}]}}"#),
    )
    .unwrap();
    let out = dir.join(format!("{name}.{ext}"));
    let mut cmd = Command::new("ogr2ogr");
    cmd.args(["-f", "ESRI Shapefile"]);
    if geometry {
        cmd.args(["-a_srs", "EPSG:32632"]);
    }
    let r = cmd.arg(&out).arg(&json).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    out
}

fn run(args: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap()
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(v) => Some(*v as f64),
        Value::SmallInteger(v) => Some(*v as f64),
        Value::Double(v) => Some(*v),
        Value::Single(v) => Some(*v as f64),
        _ => None,
    }
}

/// Our rows against an ArcGIS case's attributes (`skip` leading columns of the reference: the
/// ObjectID).
fn same(key: &str, fields: &[String], rows: &[Vec<Value>], want: &Json) -> Vec<String> {
    let mut failures = Vec::new();
    let want_fields: Vec<&str> = want["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f[0].as_str().unwrap())
        .filter(|n| !matches!(*n, "OID" | "FID" | "Shape"))
        .collect();
    if fields.iter().map(String::as_str).collect::<Vec<_>>() != want_fields {
        return vec![format!(
            "{key}: fields {fields:?} but ArcGIS {want_fields:?}"
        )];
    }
    let want_rows = want["rows"].as_array().unwrap();
    if rows.len() != want_rows.len() {
        return vec![format!(
            "{key}: {} rows but ArcGIS {}",
            rows.len(),
            want_rows.len()
        )];
    }
    for (i, (r, w)) in rows.iter().zip(want_rows).enumerate() {
        for (k, name) in want_fields.iter().enumerate() {
            let a = &w[k + 1];
            let ok = match (a, &r[k]) {
                (Json::Number(n), v) => {
                    num(v).is_some_and(|g| (g - n.as_f64().unwrap()).abs() < 1e-9)
                }
                (Json::String(s), Value::String(g)) => s == g.trim_end(),
                (Json::Null, Value::Null) => true,
                _ => false,
            };
            if !ok {
                failures.push(format!("{key} row {i} {name}: {:?} but ArcGIS {a}", r[k]));
            }
        }
    }
    failures
}

fn read_dbf(path: &Path) -> (Vec<String>, Vec<Vec<Value>>) {
    let r = DbfReader::open(path, None).unwrap();
    let fields = r.header().fields.iter().map(|f| f.name.clone()).collect();
    let rows = (0..r.num_records()).map(|i| r.read(i).unwrap().0).collect();
    (fields, rows)
}

fn read_shp(path: &Path) -> (Vec<String>, Vec<Vec<Value>>) {
    let r = ShapefileReader::open(path).unwrap();
    let fields = r.fields().iter().map(|f| f.name.clone()).collect();
    let rows = r.features().map(|f| f.unwrap().attributes).collect();
    (fields, rows)
}

#[test]
fn table_tools_equal_arcgis() {
    if !unigis_testkit::ogrinfo_available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let r: Json = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/tables.json")).unwrap(),
    )
    .unwrap();
    let d = std::env::temp_dir().join(format!("gf-tables-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    // The source: points with Id, NAME, POP, AREA, CODE; the join table: Field1, CODE, LABEL, WEIGHT, RANK.
    let src_features: Vec<String> = r["src"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, row)| {
            format!(
                r#"{{"type":"Feature","properties":{{"Id":0,"NAME":{},"POP":{},"AREA":{},"CODE":{}}},"geometry":{{"type":"Point","coordinates":[{},5000000]}}}}"#,
                row[0],
                row[1],
                row[2],
                row[3],
                500_000 + i * 10
            )
        })
        .collect();
    let src = ogr(&d, "src", &src_features.join(","), "shp", true);
    let jt_features: Vec<String> = r["jt"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            format!(
                r#"{{"type":"Feature","properties":{{"Field1":{},"CODE":{},"LABEL":{},"WEIGHT":{},"RANK":{}}},"geometry":null}}"#,
                row[0], row[1], row[2], row[3], row[4]
            )
        })
        .collect();
    let jt = ogr(&d, "jt", &jt_features.join(","), "dbf", false);
    let (src_s, jt_s) = (
        src.to_str().unwrap().to_string(),
        jt.to_str().unwrap().to_string(),
    );
    let s = |x: &str| x.to_string();
    let mut failures = Vec::new();
    let c = &r["cases"];
    // Tables (written as .dbf).
    for (key, args) in [
        (
            "copyrows",
            vec![s("run"), s("copy_rows"), format!("input={src_s}")],
        ),
        (
            "copyrows_jt",
            vec![s("run"), s("copy_rows"), format!("input={jt_s}")],
        ),
        (
            "t2t",
            vec![s("run"), s("table_to_table"), format!("input={src_s}")],
        ),
        (
            "t2t_where",
            vec![
                s("run"),
                s("table_to_table"),
                format!("input={src_s}"),
                s("where_clause=POP > 15"),
            ],
        ),
    ] {
        let out = d.join(format!("{key}.dbf"));
        let mut a = args;
        a.push(format!("output={}", out.display()));
        let o = run(&a);
        if !o.status.success() {
            failures.push(format!("{key}: {}", String::from_utf8_lossy(&o.stderr)));
            continue;
        }
        let (f, rows) = read_dbf(&out);
        failures.extend(same(key, &f, &rows, &c[key]));
    }
    // Features with joined fields (the reference's join table has the extra Field1).
    for (key, extra) in [
        ("join_all", vec![s("in_field=CODE"), s("join_field=CODE")]),
        (
            "join_some",
            vec![
                s("in_field=CODE"),
                s("join_field=CODE"),
                s("fields=LABEL;RANK"),
            ],
        ),
        (
            "join_dupname",
            vec![s("in_field=NAME"), s("join_field=LABEL"), s("fields=CODE")],
        ),
    ] {
        let out = d.join(format!("{key}.shp"));
        let mut a = vec![
            s("run"),
            s("join_field"),
            format!("input={src_s}"),
            format!("join_table={jt_s}"),
            format!("output={}", out.display()),
        ];
        a.extend(extra);
        let o = run(&a);
        if !o.status.success() {
            failures.push(format!("{key}: {}", String::from_utf8_lossy(&o.stderr)));
            continue;
        }
        let (f, rows) = read_shp(&out);
        failures.extend(same(key, &f, &rows, &c[key]));
    }
    // Add Fields.
    let out = d.join("addfields.shp");
    let o = run(&[
        s("run"),
        s("add_fields"),
        format!("input={src_s}"),
        s("fields=F1 LONG;F2 TEXT 12;F3 DOUBLE;F4 SHORT;F5 FLOAT;F6 DATE"),
        format!("output={}", out.display()),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let (f, rows) = read_shp(&out);
    failures.extend(same("addfields", &f, &rows, &c["addfields"]));
    // Get Count.
    let o = run(&[
        s("run"),
        s("get_count"),
        format!("input={src_s}"),
        s("where_clause=POP > 15"),
    ]);
    assert!(
        String::from_utf8_lossy(&o.stdout).contains("Row Count: 3"),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    // Bad parameters are errors.
    for bad in [
        vec![
            s("run"),
            s("join_field"),
            format!("input={src_s}"),
            format!("join_table={jt_s}"),
            s("in_field=NOPE"),
            s("join_field=CODE"),
            format!("output={}", d.join("x.shp").display()),
        ],
        vec![
            s("run"),
            s("table_to_table"),
            format!("input={src_s}"),
            s("where_clause=NOPE >"),
            format!("output={}", d.join("x.dbf").display()),
        ],
        vec![
            s("run"),
            s("add_fields"),
            format!("input={src_s}"),
            s("fields=ONLYNAME"),
            format!("output={}", d.join("x.shp").display()),
        ],
    ] {
        assert!(!run(&bad).status.success(), "{bad:?}");
    }
    std::fs::remove_dir_all(&d).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
