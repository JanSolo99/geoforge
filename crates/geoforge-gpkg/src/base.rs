//! The schema ArcGIS Pro 3.7 creates for a new GeoPackage (Create SQLite Database,
//! GEOPACKAGE), in creation order, and its rows. Generated from
//! testdata/gpkg/arcgis_empty.gpkg by scripts/make-gpkg-base.py.

/// (type, name, table, has a b-tree, sql)
pub(crate) const OBJECTS: &[(&str, &str, &str, bool, Option<&str>)] = &[
    (
        "table",
        "gpkg_spatial_ref_sys",
        "gpkg_spatial_ref_sys",
        true,
        Some(
            r#"CREATE TABLE gpkg_spatial_ref_sys (srs_name                  TEXT NOT NULL,srs_id                    INTEGER NOT NULL PRIMARY KEY,organization              TEXT NOT NULL,organization_coordsys_id  INTEGER NOT NULL,definition                TEXT NOT NULL DEFAULT 'undefined',description               TEXT,definition_12_063         TEXT NOT NULL DEFAULT 'undefined')"#,
        ),
    ),
    (
        "table",
        "gpkg_contents",
        "gpkg_contents",
        true,
        Some(
            r#"CREATE TABLE gpkg_contents (table_name   TEXT NOT NULL PRIMARY KEY,data_type    TEXT NOT NULL,identifier   TEXT UNIQUE,description  TEXT DEFAULT '',last_change  DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),min_x        DOUBLE,min_y        DOUBLE,max_x        DOUBLE,max_y        DOUBLE,srs_id       INTEGER,CONSTRAINT fk_gc_r_srs_id FOREIGN KEY (srs_id) REFERENCES gpkg_spatial_ref_sys (srs_id))"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_contents_1",
        "gpkg_contents",
        true,
        None,
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_contents_2",
        "gpkg_contents",
        true,
        None,
    ),
    (
        "table",
        "gpkg_geometry_columns",
        "gpkg_geometry_columns",
        true,
        Some(
            r#"CREATE TABLE gpkg_geometry_columns (table_name  TEXT NOT NULL,column_name TEXT NOT NULL,geometry_type_name          TEXT NOT NULL,srs_id      INTEGER NOT NULL,z           TINYINT NOT NULL,m           TINYINT NOT NULL,CONSTRAINT pk_geom_cols PRIMARY KEY (table_name, column_name),CONSTRAINT uk_gc_table_name UNIQUE (table_name),CONSTRAINT fk_gc_tn FOREIGN KEY (table_name) REFERENCES gpkg_contents (table_name),CONSTRAINT fk_gc_srs FOREIGN KEY (srs_id) REFERENCES gpkg_spatial_ref_sys (srs_id))"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_geometry_columns_1",
        "gpkg_geometry_columns",
        true,
        None,
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_geometry_columns_2",
        "gpkg_geometry_columns",
        true,
        None,
    ),
    (
        "table",
        "gpkg_extensions",
        "gpkg_extensions",
        true,
        Some(
            r#"CREATE TABLE gpkg_extensions (table_name      TEXT,column_name     TEXT,extension_name  TEXT NOT NULL,definition      TEXT NOT NULL,scope           TEXT NOT NULL,CONSTRAINT ge_tce UNIQUE (table_name,column_name,extension_name))"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_extensions_1",
        "gpkg_extensions",
        true,
        None,
    ),
    (
        "table",
        "gpkg_data_columns",
        "gpkg_data_columns",
        true,
        Some(
            r#"CREATE TABLE gpkg_data_columns (table_name      TEXT NOT NULL,column_name     TEXT NOT NULL,name            TEXT,title           TEXT,description     TEXT,mime_type       TEXT,constraint_name TEXT,CONSTRAINT pk_gdc PRIMARY KEY (table_name,column_name),CONSTRAINT fk_gdc_tn FOREIGN KEY (table_name) REFERENCES gpkg_contents (table_name))"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_data_columns_1",
        "gpkg_data_columns",
        true,
        None,
    ),
    (
        "table",
        "gpkg_data_column_constraints",
        "gpkg_data_column_constraints",
        true,
        Some(
            r#"CREATE TABLE gpkg_data_column_constraints (constraint_name   TEXT NOT NULL,constraint_type   TEXT NOT NULL /* 'range' || 'enum' | 'glob' */,value             TEXT,min               NUMERIC,min_is_inclusive  BOOLEAN,max               NUMERIC,max_is_inclusive  BOOLEAN,description       TEXT,CONSTRAINT gdcc_ntv UNIQUE (constraint_name,constraint_type,value))"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_data_column_constraints_1",
        "gpkg_data_column_constraints",
        true,
        None,
    ),
    (
        "table",
        "gpkg_metadata",
        "gpkg_metadata",
        true,
        Some(
            r#"CREATE TABLE gpkg_metadata (id                    INTEGER CONSTRAINT m_pk PRIMARY KEY ASC                      AUTOINCREMENT NOT NULL UNIQUE,md_scope              TEXT NOT NULL DEFAULT 'dataset',md_standard_uri       TEXT NOT NULL,mime_type             TEXT NOT NULL DEFAULT 'text/xml',metadata              TEXT NOT NULL)"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_metadata_1",
        "gpkg_metadata",
        true,
        None,
    ),
    (
        "table",
        "sqlite_sequence",
        "sqlite_sequence",
        true,
        Some(r#"CREATE TABLE sqlite_sequence(name,seq)"#),
    ),
    (
        "table",
        "gpkg_metadata_reference",
        "gpkg_metadata_reference",
        true,
        Some(
            r#"CREATE TABLE gpkg_metadata_reference (reference_scope  TEXT NOT NULL,table_name       TEXT,column_name      TEXT,row_id_value     INTEGER,timestamp        DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),md_file_id       INTEGER NOT NULL,md_parent_id     INTEGER,CONSTRAINT crmr_mfi_fk FOREIGN KEY (md_file_id) REFERENCES gpkg_metadata(id),CONSTRAINT crmr_mpi_fk FOREIGN KEY (md_parent_id) REFERENCES gpkg_metadata(id))"#,
        ),
    ),
    (
        "table",
        "gpkg_tile_matrix",
        "gpkg_tile_matrix",
        true,
        Some(
            r#"CREATE TABLE gpkg_tile_matrix (table_name TEXT NOT NULL,zoom_level INTEGER NOT NULL,matrix_width INTEGER NOT NULL,matrix_height INTEGER NOT NULL,tile_width INTEGER NOT NULL,tile_height INTEGER NOT NULL,pixel_x_size DOUBLE NOT NULL,pixel_y_size DOUBLE NOT NULL,CONSTRAINT pk_ttm PRIMARY KEY (table_name,zoom_level),CONSTRAINT fk_ttm_table_name FOREIGN KEY (table_name) REFERENCES gpkg_contents(table_name))"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_tile_matrix_1",
        "gpkg_tile_matrix",
        true,
        None,
    ),
    (
        "trigger",
        "gpkg_tile_matrix_zoom_level_insert",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_zoom_level_insert' BEFORE INSERT ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'insert on table ''gpkg_tile_matrix'' violates constraint: zoom_level cannot be less than 0') WHERE (NEW.zoom_level < 0);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_zoom_level_update",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_zoom_level_update' BEFORE UPDATE OF zoom_level ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'update on table ''gpkg_tile_matrix'' violates constraint: zoom_level cannot be less than 0') WHERE (NEW.zoom_level < 0);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_matrix_width_insert",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_matrix_width_insert' BEFORE INSERT ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'insert on table ''gpkg_tile_matrix'' violates constraint: matrix_width cannot be less than 1') WHERE (NEW.matrix_width < 1);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_matrix_width_update",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_matrix_width_update' BEFORE UPDATE OF matrix_width ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'update on table ''gpkg_tile_matrix'' violates constraint: matrix_width cannot be less than 1') WHERE (NEW.matrix_width < 1);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_matrix_height_insert",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_matrix_height_insert' BEFORE INSERT ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'insert on table ''gpkg_tile_matrix'' violates constraint: matrix_height cannot be less than 1') WHERE (NEW.matrix_height < 1);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_matrix_height_update",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_matrix_height_update' BEFORE UPDATE OF matrix_height ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'update on table ''gpkg_tile_matrix'' violates constraint: matrix_height cannot be less than 1') WHERE (NEW.matrix_height < 1);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_pixel_x_size_insert",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_pixel_x_size_insert' BEFORE INSERT ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'insert on table ''gpkg_tile_matrix'' violates constraint: pixel_x_size must be greater than 0') WHERE NOT (NEW.pixel_x_size > 0);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_pixel_x_size_update",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_pixel_x_size_update' BEFORE UPDATE OF pixel_x_size ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'update on table ''gpkg_tile_matrix'' violates constraint: pixel_x_size must be greater than 0') WHERE NOT (NEW.pixel_x_size > 0);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_pixel_y_size_insert",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_pixel_y_size_insert' BEFORE INSERT ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'insert on table ''gpkg_tile_matrix'' violates constraint: pixel_y_size must be greater than 0') WHERE NOT (NEW.pixel_y_size > 0);END"#,
        ),
    ),
    (
        "trigger",
        "gpkg_tile_matrix_pixel_y_size_update",
        "gpkg_tile_matrix",
        false,
        Some(
            r#"CREATE TRIGGER 'gpkg_tile_matrix_pixel_y_size_update' BEFORE UPDATE OF pixel_y_size ON 'gpkg_tile_matrix' FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'update on table ''gpkg_tile_matrix'' violates constraint: pixel_y_size must be greater than 0') WHERE NOT (NEW.pixel_y_size > 0);END"#,
        ),
    ),
    (
        "table",
        "gpkg_tile_matrix_set",
        "gpkg_tile_matrix_set",
        true,
        Some(
            r#"CREATE TABLE gpkg_tile_matrix_set (table_name TEXT NOT NULL PRIMARY KEY,srs_id     INTEGER NOT NULL,min_x      DOUBLE NOT NULL,min_y      DOUBLE NOT NULL,max_x      DOUBLE NOT NULL,max_y      DOUBLE NOT NULL,CONSTRAINT fk_gtms_table_name FOREIGN KEY (table_name) REFERENCES gpkg_contents (table_name),CONSTRAINT fk_gtms_srs FOREIGN KEY (srs_id) REFERENCES gpkg_spatial_ref_sys (srs_id))"#,
        ),
    ),
    (
        "index",
        "sqlite_autoindex_gpkg_tile_matrix_set_1",
        "gpkg_tile_matrix_set",
        true,
        None,
    ),
];

/// gpkg_spatial_ref_sys rows: (srs_name, srs_id, organization, organization_coordsys_id, definition, description, definition_12_063)
#[allow(clippy::type_complexity)]
pub(crate) const SPATIAL_REF_SYS: &[(&str, i64, &str, i64, &str, Option<&str>, &str)] = &[
    (
        r#"Undefined Cartesian"#,
        -1,
        "NONE",
        -1,
        r#"undefined"#,
        None,
        r#"undefined"#,
    ),
    (
        r#"Undefined Geographic"#,
        0,
        "NONE",
        0,
        r#"undefined"#,
        None,
        r#"undefined"#,
    ),
    (
        r#"GCS_WGS_1984"#,
        4326,
        "EPSG",
        4326,
        r#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]"#,
        Some(r#"WGS 1984"#),
        r#"GEOGCRS["GCS_WGS_1984",DYNAMIC[FRAMEEPOCH[1990.5],MODEL["AM0-2"]],DATUM["D_WGS_1984",ELLIPSOID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],CS[ellipsoidal,2],AXIS["Latitude (lat)",north,ORDER[1]],AXIS["Longitude (lon)",east,ORDER[2]],ANGLEUNIT["Degree",0.0174532925199433],ID["EPSG","4326"]]"#,
    ),
];

/// gpkg_extensions rows: (table_name, column_name, extension_name, definition, scope)
#[allow(clippy::type_complexity)]
pub(crate) const EXTENSIONS: &[(Option<&str>, Option<&str>, &str, &str, &str)] = &[
    (None, None, "gpkg_metadata", "F.8 Metadata", "read-write"),
    (None, None, "gpkg_schema", "F.9 Schema", "read-write"),
    (
        Some("gpkg_spatial_ref_sys"),
        Some("definition_12_063"),
        "gpkg_crs_wkt",
        "http://www.geopackage.org/spec121/#extension_crs_wkt",
        "read-write",
    ),
];

/// The spatial index ArcGIS creates with a feature table, in schema order:
/// (type, name, sql) with `{t}` for the table name.
pub(crate) const RTREE_OBJECTS: &[(&str, &str, &str)] = &[
    (
        "table",
        r#"rtree_{t}_Shape"#,
        r#"CREATE VIRTUAL TABLE "rtree_{t}_Shape" USING RTREE (id,minx,maxx,miny,maxy)"#,
    ),
    (
        "table",
        r#"rtree_{t}_Shape_rowid"#,
        r#"CREATE TABLE "rtree_{t}_Shape_rowid"(rowid INTEGER PRIMARY KEY,nodeno)"#,
    ),
    (
        "table",
        r#"rtree_{t}_Shape_node"#,
        r#"CREATE TABLE "rtree_{t}_Shape_node"(nodeno INTEGER PRIMARY KEY,data)"#,
    ),
    (
        "table",
        r#"rtree_{t}_Shape_parent"#,
        r#"CREATE TABLE "rtree_{t}_Shape_parent"(nodeno INTEGER PRIMARY KEY,parentnode)"#,
    ),
    (
        "trigger",
        r#"rtree_{t}_Shape_insert"#,
        r#"CREATE TRIGGER "rtree_{t}_Shape_insert" AFTER INSERT ON "{t}" WHEN (new."Shape" NOT NULL AND NOT ST_IsEmpty(NEW."Shape")) BEGIN INSERT OR REPLACE INTO "rtree_{t}_Shape" VALUES (NEW."OBJECTID", ST_MinX(NEW."Shape"), ST_MaxX(NEW."Shape"), ST_MinY(NEW."Shape"), ST_MaxY(NEW."Shape"));END"#,
    ),
    (
        "trigger",
        r#"rtree_{t}_Shape_delete"#,
        r#"CREATE TRIGGER "rtree_{t}_Shape_delete" AFTER DELETE ON "{t}" WHEN old."Shape" NOT NULL BEGIN DELETE FROM "rtree_{t}_Shape" WHERE id = OLD."OBJECTID"; END"#,
    ),
    (
        "trigger",
        r#"rtree_{t}_Shape_update2"#,
        r#"CREATE TRIGGER "rtree_{t}_Shape_update2" AFTER UPDATE OF "Shape" ON "{t}" WHEN OLD."OBJECTID" = NEW."OBJECTID" AND (NEW."Shape" ISNULL OR ST_IsEmpty(NEW."Shape")) BEGIN DELETE FROM "rtree_{t}_Shape" WHERE id = OLD."OBJECTID"; END"#,
    ),
    (
        "trigger",
        r#"rtree_{t}_Shape_update4"#,
        r#"CREATE TRIGGER "rtree_{t}_Shape_update4" AFTER UPDATE ON "{t}" WHEN OLD."OBJECTID" != NEW."OBJECTID" AND (NEW."Shape" ISNULL OR ST_IsEmpty(NEW."Shape")) BEGIN DELETE FROM "rtree_{t}_Shape" WHERE id IN (OLD."OBJECTID", NEW."OBJECTID"); END"#,
    ),
    (
        "trigger",
        r#"rtree_{t}_Shape_update5"#,
        r#"CREATE TRIGGER "rtree_{t}_Shape_update5" AFTER UPDATE ON "{t}" WHEN OLD."OBJECTID" != NEW."OBJECTID" AND (NEW."Shape" NOTNULL AND NOT ST_IsEmpty(NEW."Shape")) BEGIN DELETE FROM "rtree_{t}_Shape" WHERE id = OLD."OBJECTID"; INSERT OR REPLACE INTO "rtree_{t}_Shape" VALUES ( NEW."OBJECTID", ST_MinX(NEW."Shape"), ST_MaxX(NEW."Shape"), ST_MinY(NEW."Shape"), ST_MaxY(NEW."Shape")); END"#,
    ),
    (
        "trigger",
        r#"rtree_{t}_Shape_update6"#,
        r#"CREATE TRIGGER "rtree_{t}_Shape_update6" AFTER UPDATE OF "Shape" ON "{t}" WHEN OLD."OBJECTID" = NEW."OBJECTID" AND (NEW."Shape" NOTNULL AND NOT ST_IsEmpty(NEW."Shape")) AND  (OLD."Shape" NOTNULL AND NOT ST_IsEmpty(OLD."Shape")) BEGIN  UPDATE "rtree_{t}_Shape" SET     minx = ST_MinX(NEW."Shape"),    maxx = ST_MaxX(NEW."Shape"),    miny = ST_MinY(NEW."Shape"),    maxy = ST_MaxY(NEW."Shape")  WHERE id = NEW."OBJECTID"; END"#,
    ),
    (
        "trigger",
        r#"rtree_{t}_Shape_update7"#,
        r#"CREATE TRIGGER "rtree_{t}_Shape_update7" AFTER UPDATE OF "Shape" ON "{t}" WHEN OLD."OBJECTID" = NEW."OBJECTID" AND (NEW."Shape" NOTNULL AND NOT ST_IsEmpty(NEW."Shape")) AND  (OLD."Shape" ISNULL OR ST_IsEmpty(OLD."Shape")) BEGIN INSERT INTO "rtree_{t}_Shape" VALUES ( NEW."OBJECTID", ST_MinX(NEW."Shape"), ST_MaxX(NEW."Shape"), ST_MinY(NEW."Shape"), ST_MaxY(NEW."Shape")); END"#,
    ),
];
