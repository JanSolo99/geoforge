//! Generates the small projected datasets in `testdata/geoproc/` used to compare
//! geoprocessing tools with ArcGIS:
//! `cargo run -p geoforge-cli --example make_geoproc_testdata`

use geoforge_core::{Feature, Field, FieldType, Geometry, GeometryKind, Value};
use geoforge_shapefile::ShapefileWriter;

const UTM33N: &str = r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;
const X0: f64 = 500_000.0;
const Y0: f64 = 5_000_000.0;

/// Deterministic xorshift so the data never changes between runs.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn geom(kind: GeometryKind, rings: Vec<Vec<[f64; 2]>>) -> Geometry {
    let mut parts = Vec::new();
    let mut xy = Vec::new();
    for r in rings {
        parts.push(xy.len() as u32);
        xy.extend(r);
    }
    let mut g = Geometry {
        kind,
        parts,
        part_types: None,
        xy,
        z: None,
        m: None,
        curves: Vec::new(),
    };
    g.orient_rings_esri();
    g
}

fn ring(cx: f64, cy: f64, r: f64, n: usize, rng: &mut Rng) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            let rr = r * (0.7 + 0.3 * rng.next());
            [cx + rr * a.cos(), cy + rr * a.sin()]
        })
        .collect();
    pts.push(pts[0]);
    pts
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
    vec![[x0, y0], [x0, y1], [x1, y1], [x1, y0], [x0, y0]]
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc");
    std::fs::create_dir_all(&dir).unwrap();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    // Parcels: a 6 x 6 grid of 100 m lots, some with holes, one multipart.
    let mut fields = vec![
        Field::new("PARCEL_ID", FieldType::Integer),
        Field::new("NAME", FieldType::String),
        Field::new("VALUE", FieldType::Double),
        Field::new("ZONE", FieldType::SmallInteger),
    ];
    fields[1].length = 20;
    let mut w = ShapefileWriter::create(
        dir.join("parcels.shp"),
        GeometryKind::Polygon,
        false,
        false,
        &fields,
        Some(UTM33N),
    )
    .unwrap();
    let mut id = 0;
    for row in 0..6 {
        for col in 0..6 {
            id += 1;
            let (x, y) = (X0 + col as f64 * 100.0, Y0 + row as f64 * 100.0);
            let mut rings = vec![rect(x + 2.0, y + 2.0, x + 98.0, y + 98.0)];
            if id % 5 == 0 {
                rings.push(rect(x + 30.0, y + 30.0, x + 60.0, y + 60.0)); // courtyard hole
            }
            if id == 8 {
                rings = vec![
                    rect(x + 2.0, y + 2.0, x + 45.0, y + 98.0),
                    rect(x + 55.0, y + 2.0, x + 98.0, y + 98.0),
                ];
            }
            let attrs = vec![
                Value::Integer(id),
                Value::String(format!("Lot {id}")),
                Value::Double((rng.next() * 1e6).round() / 100.0),
                Value::SmallInteger((id % 4) as i16),
            ];
            w.write(&Feature {
                oid: 0,
                geometry: Some(geom(GeometryKind::Polygon, rings)),
                attributes: attrs,
            })
            .unwrap();
        }
    }
    w.finish().unwrap();

    // Roads: polylines across the grid, one multipart, one wiggly.
    let fields = vec![
        Field::new("ROAD_ID", FieldType::Integer),
        Field::new("CLASS", FieldType::SmallInteger),
    ];
    let mut w = ShapefileWriter::create(
        dir.join("roads.shp"),
        GeometryKind::Polyline,
        false,
        false,
        &fields,
        Some(UTM33N),
    )
    .unwrap();
    let roads: Vec<Vec<Vec<[f64; 2]>>> = vec![
        vec![vec![[X0 - 50.0, Y0 + 250.0], [X0 + 650.0, Y0 + 260.0]]],
        vec![vec![
            [X0 + 310.0, Y0 - 40.0],
            [X0 + 300.0, Y0 + 300.0],
            [X0 + 330.0, Y0 + 640.0],
        ]],
        vec![
            vec![[X0 + 20.0, Y0 + 20.0], [X0 + 580.0, Y0 + 580.0]],
            vec![[X0 + 20.0, Y0 + 580.0], [X0 + 200.0, Y0 + 400.0]],
        ],
        vec![
            (0..40)
                .map(|i| {
                    [
                        X0 + 100.0 + i as f64 * 10.0,
                        Y0 + 120.0 + 25.0 * (i as f64 * 0.6).sin(),
                    ]
                })
                .collect(),
        ],
    ];
    for (i, parts) in roads.into_iter().enumerate() {
        w.write(&Feature {
            oid: 0,
            geometry: Some(geom(GeometryKind::Polyline, parts)),
            attributes: vec![
                Value::Integer(i as i32 + 1),
                Value::SmallInteger((i % 3) as i16 + 1),
            ],
        })
        .unwrap();
    }
    w.finish().unwrap();

    // Wells: 50 points scattered over and around the grid.
    let fields = vec![
        Field::new("WELL_ID", FieldType::Integer),
        Field::new("DEPTH", FieldType::Double),
    ];
    let mut w = ShapefileWriter::create(
        dir.join("wells.shp"),
        GeometryKind::Point,
        false,
        false,
        &fields,
        Some(UTM33N),
    )
    .unwrap();
    for i in 0..50 {
        let (x, y) = (
            X0 - 100.0 + rng.next() * 800.0,
            Y0 - 100.0 + rng.next() * 800.0,
        );
        w.write(&Feature {
            oid: 0,
            geometry: Some(Geometry::point(x.round(), y.round())),
            attributes: vec![
                Value::Integer(i + 1),
                Value::Double((rng.next() * 2000.0).round() / 10.0),
            ],
        })
        .unwrap();
    }
    w.finish().unwrap();

    // Clip areas: two overlapping polygons, one with a hole (ArcGIS dissolves them).
    let fields = vec![Field::new("AREA_ID", FieldType::Integer)];
    let mut w = ShapefileWriter::create(
        dir.join("clip_areas.shp"),
        GeometryKind::Polygon,
        false,
        false,
        &fields,
        Some(UTM33N),
    )
    .unwrap();
    let blob = ring(X0 + 220.0, Y0 + 240.0, 190.0, 24, &mut rng);
    let with_hole = vec![
        rect(X0 + 300.0, Y0 + 150.0, X0 + 520.0, Y0 + 470.0),
        rect(X0 + 380.0, Y0 + 260.0, X0 + 440.0, Y0 + 330.0),
    ];
    for (i, rings) in [vec![blob], with_hole].into_iter().enumerate() {
        w.write(&Feature {
            oid: 0,
            geometry: Some(geom(GeometryKind::Polygon, rings)),
            attributes: vec![Value::Integer(i as i32 + 1)],
        })
        .unwrap();
    }
    w.finish().unwrap();
    println!("wrote {}", dir.display());
}
