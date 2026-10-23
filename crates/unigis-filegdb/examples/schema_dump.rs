//! Dumps every table's raw field definitions and rows (for comparing writers).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut ids: Vec<_> = std::fs::read_dir(&a[1])
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "gdbtable"))
        .collect();
    ids.sort();
    for p in ids {
        let t = unigis_filegdb::Table::open(&p).unwrap();
        println!(
            "== {} rows={} kind={:?}",
            p.file_name().unwrap().to_string_lossy(),
            t.row_count(),
            t.geometry_kind()
        );
        for f in t.all_fields() {
            println!(
                "   {:<22} {:?} nullable={} len={} default={:?}",
                f.name,
                f.field_type,
                f.nullable,
                f.length,
                f.default.as_ref().map(|d| d.len())
            );
        }
        if a.get(2).is_some() {
            for r in t.features().take(60) {
                let r = r.unwrap();
                println!(
                    "   row {} {:?}",
                    r.oid,
                    r.attributes
                        .iter()
                        .map(|v| {
                            let s = format!("{v:?}");
                            s.chars().take(90).collect::<String>()
                        })
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
