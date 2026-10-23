//! Collect Events against ArcGIS Pro 3.7 (testdata/arcgis-reference/spatstat_events.json; probe
//! scripts/research/spatstat/weights/probe_events.py).

use std::path::Path;
use std::process::Command;

use geoforge_core::Value;
use geoforge_shapefile::ShapefileReader;
use serde_json::Value as Json;

#[test]
fn collect_events_equals_arcgis() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/arcgis-reference");
    let r: Json =
        serde_json::from_slice(&std::fs::read(root.join("spatstat_events.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("gf-events-{}.shp", std::process::id()));
    let o = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "collect_events"])
        .arg(format!(
            "input={}",
            root.join("spatstat_weights/ev.shp").display()
        ))
        .arg(format!("output={}", out.display()))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stderr).contains("Max count: 9"));
    let rd = ShapefileReader::open(&out).unwrap();
    let names: Vec<&str> = rd.fields().iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Id", "ICOUNT"]);
    let want = r["rows"].as_array().unwrap();
    let got: Vec<_> = rd.features().map(|f| f.unwrap()).collect();
    assert_eq!(got.len(), want.len());
    for (f, w) in got.iter().zip(want) {
        let p = f.geometry.as_ref().unwrap().xy[0];
        let count = match f.attributes[1] {
            Value::Integer(v) => v as i64,
            Value::BigInteger(v) => v,
            _ => -1,
        };
        assert_eq!(
            (p[0], p[1], count),
            (
                w[0].as_f64().unwrap(),
                w[1].as_f64().unwrap(),
                w[4].as_i64().unwrap()
            ),
            "{w}"
        );
    }
    for ext in ["shp", "shx", "dbf", "prj", "cpg"] {
        std::fs::remove_file(out.with_extension(ext)).ok();
    }
}
