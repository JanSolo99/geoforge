"""Generates crates/geoforge-gpkg/src/base.rs from testdata/gpkg/arcgis_empty.gpkg
(an empty GeoPackage made by ArcGIS Pro's Create SQLite Database): the schema in
creation order and the rows ArcGIS puts in a new GeoPackage.

    python3 scripts/make-gpkg-base.py
"""

import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
con = sqlite3.connect(ROOT / "testdata" / "gpkg" / "arcgis_empty.gpkg")


def lit(s):
    return "None" if s is None else f'Some(r#"{s}"#)'


out = [
    "//! The schema ArcGIS Pro 3.7 creates for a new GeoPackage (Create SQLite Database,",
    "//! GEOPACKAGE), in creation order, and its rows. Generated from",
    "//! testdata/gpkg/arcgis_empty.gpkg by scripts/make-gpkg-base.py.",
    "",
    "/// (type, name, table, has a b-tree, sql)",
    "pub(crate) const OBJECTS: &[(&str, &str, &str, bool, Option<&str>)] = &[",
]
for t, n, tb, root, sql in con.execute("SELECT type,name,tbl_name,rootpage,sql FROM sqlite_master ORDER BY rowid"):
    out.append(f'    ("{t}", "{n}", "{tb}", {"true" if root else "false"}, {lit(sql)}),')
out += ["];", ""]
out.append("/// gpkg_spatial_ref_sys rows: (srs_name, srs_id, organization, organization_coordsys_id, definition, description, definition_12_063)")
out.append("#[allow(clippy::type_complexity)]")
out.append("pub(crate) const SPATIAL_REF_SYS: &[(&str, i64, &str, i64, &str, Option<&str>, &str)] = &[")
for r in con.execute("SELECT srs_name,srs_id,organization,organization_coordsys_id,definition,description,definition_12_063 FROM gpkg_spatial_ref_sys ORDER BY rowid"):
    out.append(f'    (r#"{r[0]}"#, {r[1]}, "{r[2]}", {r[3]}, r#"{r[4]}"#, {lit(r[5])}, r#"{r[6]}"#),')
out += ["];", ""]
out.append("/// gpkg_extensions rows: (table_name, column_name, extension_name, definition, scope)")
out.append("#[allow(clippy::type_complexity)]")
out.append("pub(crate) const EXTENSIONS: &[(Option<&str>, Option<&str>, &str, &str, &str)] = &[")
for r in con.execute("SELECT table_name,column_name,extension_name,definition,scope FROM gpkg_extensions ORDER BY rowid"):
    f = lambda v: "None" if v is None else f'Some("{v}")'
    out.append(f'    ({f(r[0])}, {f(r[1])}, "{r[2]}", "{r[3]}", "{r[4]}"),')
out.append("];")
# The spatial index of a feature table, from an ArcGIS-made GeoPackage holding a
# table named "wells" (testdata/gpkg/arcgis_wells.gpkg).
con = sqlite3.connect(ROOT / "testdata" / "gpkg" / "arcgis_wells.gpkg")
out += ["", "/// The spatial index ArcGIS creates with a feature table, in schema order:",
        "/// (type, name, sql) with `{t}` for the table name.",
        "pub(crate) const RTREE_OBJECTS: &[(&str, &str, &str)] = &["]
for t, n, sql in con.execute("SELECT type, name, sql FROM sqlite_master WHERE (name LIKE 'rtree_wells%' OR (tbl_name='wells' AND type='trigger')) ORDER BY rowid"):
    out.append(f'    ("{t}", r#"{n.replace("wells", "{t}")}"#, r#"{sql.replace("wells", "{t}")}"#),')
out.append("];")
(ROOT / "crates" / "geoforge-gpkg" / "src" / "base.rs").write_text("\n".join(out) + "\n")
