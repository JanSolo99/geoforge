//! Existing ArcGIS Esri foot definitions retain names and parameter precision.
use std::path::Path;
use unigis_proj::Crs;

#[test]
fn provided_arcgis_foot_definition_matches_tessellation_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        std::fs::read_to_string(root.join("testdata/tessellation/feet_template.prj")).unwrap();
    let expected =
        std::fs::read_to_string(root.join("testdata/arcgis-reference/tess_area_feet_sqm.prj"))
            .unwrap();
    assert_eq!(Crs::from_wkt(&source).unwrap().arcgis_wkt(), expected);
}
