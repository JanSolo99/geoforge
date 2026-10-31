//! `ug`: UniGIS geoprocessing tools from the command line. Tool names and
//! behavior follow their ArcGIS Pro counterparts.
//!
//! Datasets are addressed the way ArcGIS does: `roads.shp`, `city.gdb`,
//! `city.gdb/roads` or `city.gdb/Transport/roads` (feature dataset optional).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use unigis_core::{Error, Feature, FeatureClass, Field, FieldType, GeometryKind, Result, Value};
use unigis_filegdb::Geodatabase;
use unigis_shapefile::{ShapefileReader, ShapefileWriter};
use unigis_tools::{Dissolve, ToolOutput};

mod aliases;
mod dataset_outputs;
mod delete_multiple;
mod feature_class_to_geodatabase;

#[derive(Parser)]
#[command(name = "ug", version, about = "UniGIS geoprocessing tools")]
struct Cli {
    #[command(subcommand)]
    tool: Tool,
}

#[derive(Subcommand)]
enum Tool {
    /// Print dataset properties, like arcpy.Describe.
    Describe { dataset: PathBuf },
    /// Copy features to a new feature class (ArcGIS "Copy Features").
    CopyFeatures {
        in_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Copy the features a where clause selects, with an optional field mapping
    /// (ArcGIS "Feature Class To Feature Class").
    FeatureClassToFeatureClass {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        #[arg(long)]
        where_clause: Option<String>,
        /// A field mapping string as ArcGIS's `field_mapping` parameter.
        #[arg(long)]
        field_mapping: Option<String>,
    },
    /// Extract the parts of input features inside clip polygons (ArcGIS "Clip").
    Clip {
        in_features: PathBuf,
        clip_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Geometric intersection of inputs (ArcGIS "Intersect"): `ug intersect a b ... out`.
    Intersect {
        /// Two or more inputs followed by the output.
        #[arg(num_args = 3.., required = true)]
        paths: Vec<PathBuf>,
        /// ALL, NO_FID or ONLY_FID.
        #[arg(long, default_value = "ALL")]
        join_attributes: String,
        /// INPUT (lowest input dimension), LINE or POINT.
        #[arg(long, default_value = "INPUT")]
        output_type: String,
        /// One feature per combination of input features, its pieces merged (Pairwise Intersect).
        #[arg(long, hide = true)]
        merge_pieces: bool,
    },
    /// Polygon boundaries as lines (ArcGIS "Polygon To Line"): with IDENTIFY_NEIGHBORS (the
    /// default) shared boundaries are split and get LEFT_FID / RIGHT_FID.
    PolygonToLine {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// IDENTIFY_NEIGHBORS or IGNORE_NEIGHBORS.
        #[arg(long, default_value = "IDENTIFY_NEIGHBORS")]
        neighbor_option: String,
    },
    /// Polygons from the areas lines or polygon outlines enclose (ArcGIS "Feature To
    /// Polygon"): `ug feature-to-polygon a [b ...] out`.
    FeatureToPolygon {
        /// One or more inputs followed by the output.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// XY tolerance, e.g. `0.01 Meters` (default: the coordinate system's).
        #[arg(long)]
        cluster_tolerance: Option<String>,
        /// ATTRIBUTES or NO_ATTRIBUTES.
        #[arg(long, default_value = "ATTRIBUTES")]
        attributes: String,
        /// Points whose attributes the polygons containing them take.
        #[arg(long)]
        label_features: Option<PathBuf>,
    },
    /// Lines and polygon outlines split where they cross or meet (ArcGIS "Feature To
    /// Line"): `ug feature-to-line a [b ...] out`.
    FeatureToLine {
        /// One or more inputs followed by the output.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// XY tolerance, e.g. `0.01 Meters` (default: the coordinate system's).
        #[arg(long)]
        cluster_tolerance: Option<String>,
        /// ATTRIBUTES or NO_ATTRIBUTES.
        #[arg(long, default_value = "ATTRIBUTES")]
        attributes: String,
    },
    /// Planar overlay of polygon inputs (ArcGIS "Union"): `ug union a b ... out`.
    Union {
        /// One or more inputs followed by the output.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// ALL, NO_FID or ONLY_FID.
        #[arg(long, default_value = "ALL")]
        join_attributes: String,
        /// GAPS, or NO_GAPS to add the enclosed areas no input covers.
        #[arg(long, default_value = "GAPS")]
        gaps: String,
    },
    /// The parts of two polygon inputs the other does not cover (ArcGIS "Symmetrical
    /// Difference"): `ug symmetrical-difference in update out`.
    SymmetricalDifference {
        in_features: PathBuf,
        update_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Replace the area update polygons cover (ArcGIS "Update"): `ug update in update out`.
    Update {
        in_features: PathBuf,
        update_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Remove the parts of input features inside erase polygons (ArcGIS "Erase").
    Erase {
        in_features: PathBuf,
        erase_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Split input features by identity polygons, joining their attributes
    /// (ArcGIS "Identity").
    Identity {
        in_features: PathBuf,
        identity_features: PathBuf,
        out_feature_class: PathBuf,
        /// ALL, NO_FID or ONLY_FID (for the identity features' columns).
        #[arg(long, default_value = "ALL")]
        join_attributes: String,
        /// NO_RELATIONSHIPS, or KEEP_RELATIONSHIPS for LEFT_/RIGHT_ columns (lines).
        #[arg(long, default_value = "NO_RELATIONSHIPS")]
        relationship: String,
    },
    /// Combine inputs of the same geometry type into one (ArcGIS "Merge"):
    /// `ug merge a b ... out`.
    Merge {
        /// One or more inputs followed by the output.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// NO_SOURCE_INFO (default) or ADD_SOURCE_INFO (adds MERGE_SRC).
        #[arg(long, default_value = "NO_SOURCE_INFO")]
        add_source: String,
        /// A field mapping string as ArcGIS's `field_mappings` parameter.
        #[arg(long)]
        field_mapping: Option<String>,
    },
    /// Add the distance to the closest near feature (ArcGIS "Near", planar).
    /// Updates the input shapefile in place, as ArcGIS does, unless an output is
    /// given.
    Near {
        in_features: PathBuf,
        /// One near feature class, or several joined with ';' (adds NEAR_FC).
        near_features: String,
        /// Write the result here instead of updating the input.
        out_feature_class: Option<PathBuf>,
        /// Only consider near features within this distance, e.g. "100 Meters".
        #[arg(long)]
        search_radius: Option<String>,
        /// LOCATION: add NEAR_X and NEAR_Y.
        #[arg(long, default_value = "NO_LOCATION")]
        location: String,
        /// ANGLE: add NEAR_ANGLE.
        #[arg(long, default_value = "NO_ANGLE")]
        angle: String,
        /// PLANAR or GEODESIC (longitude/latitude data: metres on the ellipsoid, azimuth angles).
        #[arg(long, default_value = "PLANAR")]
        method: String,
    },
    /// Join attributes by spatial relationship (ArcGIS "Spatial Join").
    SpatialJoin {
        target_features: PathBuf,
        join_features: PathBuf,
        out_feature_class: PathBuf,
        /// JOIN_ONE_TO_ONE or JOIN_ONE_TO_MANY.
        #[arg(long, default_value = "JOIN_ONE_TO_ONE")]
        join_operation: String,
        /// KEEP_ALL or KEEP_COMMON.
        #[arg(long, default_value = "KEEP_ALL")]
        join_type: String,
        /// INTERSECT, WITHIN_A_DISTANCE, CONTAINS, COMPLETELY_CONTAINS, WITHIN,
        /// COMPLETELY_WITHIN or CLOSEST.
        #[arg(long, default_value = "INTERSECT")]
        match_option: String,
        #[arg(long)]
        search_radius: Option<String>,
        /// Add a field with the distance to the joined feature.
        #[arg(long)]
        distance_field_name: Option<String>,
        /// A join field's merge rule, FIELD=RULE[:DELIMITER] (repeatable): First,
        /// Last, Join, Sum, Mean, Median, Mode, Min, Max, StdDev, Count, Range.
        #[arg(long = "merge-rule")]
        merge_rules: Vec<String>,
        /// A full field mapping string as ArcGIS's `field_mapping` parameter: output
        /// names, types, lengths, merge rules and source fields.
        #[arg(long)]
        field_mapping: Option<String>,
    },
    /// Write the input features that relate to selecting features (ArcGIS "Select
    /// Layer By Location" with a new selection, then "Copy Features").
    SelectByLocation {
        in_features: PathBuf,
        /// INTERSECT, WITHIN_A_DISTANCE, CONTAINS, COMPLETELY_CONTAINS,
        /// CONTAINS_CLEMENTINI, WITHIN, COMPLETELY_WITHIN, WITHIN_CLEMENTINI,
        /// ARE_IDENTICAL_TO, BOUNDARY_TOUCHES, SHARE_A_LINE_SEGMENT_WITH,
        /// CROSSED_BY_THE_OUTLINE_OF or HAVE_THEIR_CENTER_IN.
        overlap_type: String,
        select_features: PathBuf,
        out_feature_class: PathBuf,
        /// For INTERSECT and WITHIN_A_DISTANCE, e.g. "30 Meters".
        #[arg(long)]
        search_distance: Option<String>,
        /// INVERT: select the features that do not relate.
        #[arg(long, default_value = "NOT_INVERT")]
        invert_spatial_relationship: String,
    },
    /// Add the features of inputs to an existing target (ArcGIS "Append"):
    /// `ug append a b ... target`. Updates the target shapefile in place.
    Append {
        /// One or more inputs followed by the target.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// TEST (schemas must match) or NO_TEST (fields matched by name).
        #[arg(long, default_value = "TEST")]
        schema_type: String,
        /// Write the result here instead of updating the target.
        #[arg(long)]
        output: Option<PathBuf>,
        /// A field mapping string as ArcGIS's `field_mapping` parameter (NO_TEST).
        #[arg(long)]
        field_mapping: Option<String>,
    },
    /// Clip the input by each group of split polygons sharing a text value, one
    /// output per value named after it (ArcGIS "Split").
    Split {
        in_features: PathBuf,
        split_features: PathBuf,
        split_field: String,
        /// A folder; outputs are `<value>.shp`.
        out_workspace: PathBuf,
    },
    /// Reorder features by field values and/or location (ArcGIS "Sort").
    Sort {
        in_dataset: PathBuf,
        out_dataset: PathBuf,
        /// Keys as ArcGIS writes them, e.g. "ZONE DESCENDING;VALUE ASCENDING"; the
        /// field `Shape` sorts spatially. A key alone means ascending.
        sort_field: String,
        /// UR (default), UL, LR or LL: where a `Shape` key starts.
        #[arg(long, default_value = "UR")]
        spatial_sort_method: String,
    },
    /// Append a field (ArcGIS "Add Field"). Updates the input shapefile in place unless
    /// an output is given.
    AddField {
        in_table: PathBuf,
        field_name: String,
        /// TEXT, FLOAT, DOUBLE, SHORT, LONG or DATE.
        field_type: String,
        #[arg(long)]
        precision: Option<u32>,
        #[arg(long)]
        scale: Option<u32>,
        #[arg(long)]
        length: Option<u32>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Remove fields (ArcGIS "Delete Field"). Updates the input shapefile in place unless
    /// an output is given.
    DeleteField {
        in_table: PathBuf,
        /// Field names, `;`-separated.
        drop_field: String,
        /// DELETE_FIELDS (default) or KEEP_FIELDS (delete all others).
        #[arg(long, default_value = "DELETE_FIELDS")]
        method: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Delete rows (ArcGIS "Delete Rows"), in place; with `--where`, only the matching
    /// rows (as on a table view with that definition query). OBJECTIDs are not reused.
    DeleteRows {
        in_rows: PathBuf,
        /// SQL where clause; the OBJECTID field can be used.
        #[arg(long = "where")]
        where_clause: Option<String>,
    },
    /// Delete features (ArcGIS "Delete Features"); as `delete-rows`.
    DeleteFeatures {
        in_features: PathBuf,
        #[arg(long = "where")]
        where_clause: Option<String>,
    },
    /// Delete every row of a geodatabase table, restarting its OBJECTIDs at 1
    /// (ArcGIS "Truncate Table").
    TruncateTable { in_table: PathBuf },
    /// Fill a field with a Python expression (ArcGIS "Calculate Field", PYTHON3).
    /// Updates the input shapefile in place unless an output is given.
    CalculateField {
        in_table: PathBuf,
        field: String,
        /// e.g. "!VALUE! * 2" (`!FIELD!` stands for the row's value).
        expression: String,
        /// Python code defining functions the expression uses.
        #[arg(long, default_value = "")]
        code_block: String,
        /// Creates the field with this type (TEXT, LONG, DOUBLE, ...) if it is missing.
        #[arg(long)]
        field_type: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Delete records that repeat the values (or shape) of an earlier one (ArcGIS "Delete
    /// Identical"). Updates the input shapefile in place unless an output is given.
    DeleteIdentical {
        in_dataset: PathBuf,
        /// Fields to compare, `;`-separated; `Shape` compares geometries.
        fields: String,
        /// Vertices closer than this count as equal (default 0.001, the XY tolerance).
        #[arg(long, default_value_t = 0.001)]
        xy_tolerance: f64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Create a feature dataset in a file geodatabase (ArcGIS "Create Feature Dataset"); the
    /// geodatabase is created too if it does not exist. Feature classes written to
    /// `city.gdb/Dataset/name` join it and take its coordinate system.
    CreateFeatureDataset {
        /// `city.gdb`.
        out_dataset_path: PathBuf,
        out_name: String,
        /// An EPSG/Esri code, a `.prj` file or WKT; none = unknown coordinate system.
        spatial_reference: Option<String>,
    },
    /// Make a short or long integer field the subtype field of a geodatabase table (ArcGIS "Set
    /// Subtype Field").
    SetSubtypeField {
        in_table: PathBuf,
        subtype_field: String,
    },
    /// Add a subtype to a geodatabase table (ArcGIS "Add Subtype"); it starts with the table's
    /// field defaults.
    AddSubtype {
        in_table: PathBuf,
        subtype_code: i64,
        subtype_description: String,
    },
    /// Remove subtypes (ArcGIS "Remove Subtype"); codes separated by `;`.
    RemoveSubtype {
        in_table: PathBuf,
        subtype_code: String,
    },
    /// Set the default subtype (ArcGIS "Set Default Subtype").
    SetDefaultSubtype {
        in_table: PathBuf,
        subtype_code: i64,
    },
    /// Assign a domain to a field, or to it within subtypes (ArcGIS "Assign Domain To Field").
    AssignDomainToField {
        in_table: PathBuf,
        field_name: String,
        domain_name: String,
        /// Subtype codes separated by `;`.
        #[arg(long, default_value = "")]
        subtype_code: String,
    },
    /// Remove a field's domain, or its domain within subtypes (ArcGIS "Remove Domain From Field").
    RemoveDomainFromField {
        in_table: PathBuf,
        field_name: String,
        #[arg(long, default_value = "")]
        subtype_code: String,
    },
    /// Add an attribute index on a field of a file geodatabase table (ArcGIS "Add Attribute Index"; UNIQUE and
    /// ASCENDING make no difference there, one field only).
    AddIndex {
        in_table: PathBuf,
        field: String,
        index_name: String,
    },
    /// Remove an attribute index of a file geodatabase table by name (ArcGIS "Remove Attribute Index").
    RemoveIndex {
        in_table: PathBuf,
        index_name: String,
    },
    /// Rename a field, change its alias, type, length or nullability, in place on a table of a
    /// file geodatabase (ArcGIS "Alter Field"). A type change needs an empty table.
    AlterField {
        in_table: PathBuf,
        field: String,
        #[arg(long)]
        new_field_name: Option<String>,
        #[arg(long)]
        new_field_alias: Option<String>,
        /// TEXT, FLOAT, DOUBLE, SHORT, LONG, BIGINTEGER, DATE, DATEONLY, TIMEONLY, BLOB or GUID.
        #[arg(long)]
        field_type: Option<String>,
        #[arg(long)]
        field_length: Option<u32>,
        /// NULLABLE or NON_NULLABLE.
        #[arg(long)]
        field_is_nullable: Option<String>,
        /// CLEAR_ALIAS or NOT_CLEAR_ALIAS.
        #[arg(long)]
        clear_field_alias: Option<String>,
    },
    /// Alter several fields in order (ArcGIS "Alter Fields"): each `--field` is
    /// `name|new name|new alias|type|length|nullable|clear alias`; empty or `#` skips a part.
    AlterFields {
        in_table: PathBuf,
        #[arg(long = "field", required = true)]
        fields: Vec<String>,
    },
    /// Assign a default value to a field, or to it within subtypes; no value clears it (ArcGIS
    /// "Assign Default To Field").
    AssignDefaultToField {
        in_table: PathBuf,
        field_name: String,
        default_value: Option<String>,
        #[arg(long, default_value = "")]
        subtype_code: String,
    },
    /// Create a relationship class between two tables or feature classes of a file geodatabase
    /// (ArcGIS "Create Relationship Class"). Attributed and many-to-many classes need all four keys.
    CreateRelationshipClass {
        origin_table: PathBuf,
        destination_table: PathBuf,
        /// `city.gdb/name`; in the same geodatabase as the tables.
        out_relationship_class: PathBuf,
        /// SIMPLE or COMPOSITE.
        relationship_type: String,
        forward_label: String,
        backward_label: String,
        /// NONE, FORWARD, BACKWARD or BOTH.
        message_direction: String,
        /// ONE_TO_ONE, ONE_TO_MANY or MANY_TO_MANY.
        cardinality: String,
        /// NONE or ATTRIBUTED.
        attributed: String,
        origin_primary_key: String,
        origin_foreign_key: String,
        destination_primary_key: Option<String>,
        destination_foreign_key: Option<String>,
    },
    /// Delete a shapefile (with its sidecar files) or an item of a file geodatabase: a table,
    /// feature class, relationship class (also the relationship classes that use a deleted
    /// table) or feature dataset with its feature classes (ArcGIS "Delete").
    Delete {
        /// `roads.shp` or `city.gdb/roads`.
        in_data: PathBuf,
    },
    /// Rename a shapefile or a geodatabase item; a feature dataset's children move with it
    /// (ArcGIS "Rename"). The new name is the last component of `out_data`.
    Rename {
        in_data: PathBuf,
        /// A new name, or a path ending in it (`city.gdb/roads2`).
        out_data: PathBuf,
    },
    /// Delete a domain from a file geodatabase; one fields use is refused (ArcGIS "Delete Domain").
    DeleteDomain {
        in_workspace: PathBuf,
        domain_name: String,
    },
    /// Rename a domain of a file geodatabase (also in the fields that use it), or change its
    /// description, split or merge policy or owner (ArcGIS "Alter Domain").
    AlterDomain {
        in_workspace: PathBuf,
        domain_name: String,
        #[arg(long)]
        new_domain_name: Option<String>,
        #[arg(long)]
        new_domain_description: Option<String>,
        /// DEFAULT, DUPLICATE or GEOMETRY_RATIO.
        #[arg(long)]
        split_policy: Option<String>,
        /// DEFAULT, SUM_VALUES or AREA_WEIGHTED.
        #[arg(long)]
        merge_policy: Option<String>,
        #[arg(long)]
        new_domain_owner: Option<String>,
    },
    /// Create an empty coded domain or a range domain in a file geodatabase (ArcGIS "Create Domain").
    CreateDomain {
        in_workspace: PathBuf,
        domain_name: String,
        #[arg(long, default_value = "")]
        domain_description: String,
        /// TEXT, FLOAT, DOUBLE, SHORT, LONG, DATE, BIGINTEGER, DATEONLY or TIMEONLY.
        field_type: String,
        /// CODED or RANGE.
        #[arg(long, default_value = "CODED")]
        domain_type: String,
        /// DEFAULT, DUPLICATE or GEOMETRY_RATIO.
        #[arg(long, default_value = "DEFAULT")]
        split_policy: String,
        /// DEFAULT, SUM_VALUES or AREA_WEIGHTED.
        #[arg(long, default_value = "DEFAULT")]
        merge_policy: String,
    },
    /// Add a code and its description to a coded domain; an existing code gets the description (ArcGIS "Add Coded Value To Domain").
    AddCodedValueToDomain {
        in_workspace: PathBuf,
        domain_name: String,
        code: String,
        code_description: String,
    },
    /// Delete codes (separated by `;`) from a coded domain (ArcGIS "Delete Coded Value From Domain").
    DeleteCodedValueFromDomain {
        in_workspace: PathBuf,
        domain_name: String,
        code: String,
    },
    /// Change the description of a code of a coded domain (ArcGIS "Alter Coded Value Description").
    AlterCodedValueDescription {
        in_workspace: PathBuf,
        domain_name: String,
        code: String,
        new_description: String,
    },
    /// Set the bounds of a range domain (ArcGIS "Set Value For Range Domain").
    SetValueForRangeDomain {
        in_workspace: PathBuf,
        domain_name: String,
        min_value: String,
        max_value: String,
    },
    /// Sort the values of a coded domain (ArcGIS "Sort Coded Value Domain").
    SortCodedValueDomain {
        in_workspace: PathBuf,
        domain_name: String,
        /// CODE or DESCRIPTION.
        #[arg(default_value = "CODE")]
        sort_by: String,
        /// ASCENDING or DESCENDING.
        #[arg(default_value = "ASCENDING")]
        sort_order: String,
    },
    /// Write the values of a domain to a table (ArcGIS "Domain To Table"); a range domain gives
    /// the rows `Min value` and `Max value`.
    DomainToTable {
        in_workspace: PathBuf,
        domain_name: String,
        out_table: PathBuf,
        code_field: String,
        description_field: String,
    },
    /// Make or update a coded domain from the code and description fields of a table (ArcGIS
    /// "Table To Domain").
    TableToDomain {
        in_table: PathBuf,
        code_field: String,
        description_field: String,
        in_workspace: PathBuf,
        domain_name: String,
        #[arg(long, default_value = "")]
        domain_description: String,
        /// APPEND or REPLACE.
        #[arg(long, default_value = "APPEND")]
        update_option: String,
    },
    /// Features matching an SQL expression (ArcGIS "Select").
    Select {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// e.g. "ZONE IN (1, 3) AND NAME LIKE 'Lot 1%'".
        #[arg(long = "where")]
        where_clause: Option<String>,
    },
    /// Summary statistics into a .dbf table (ArcGIS "Summary Statistics").
    Statistics {
        in_table: PathBuf,
        out_table: PathBuf,
        /// "FIELD STAT;FIELD STAT": SUM, MEAN, MIN, MAX, RANGE, STD, COUNT, FIRST,
        /// LAST, MEDIAN, VARIANCE, UNIQUE.
        statistics_fields: String,
        /// Fields to summarize by (repeatable).
        #[arg(long = "case-field")]
        case_fields: Vec<String>,
    },
    /// Counts per combination of field values into a .dbf table (ArcGIS "Frequency").
    Frequency {
        in_table: PathBuf,
        out_table: PathBuf,
        /// Fields separated by ';'.
        frequency_fields: String,
        /// Fields to sum, separated by ';'.
        #[arg(long)]
        summary_fields: Option<String>,
    },
    /// Add geometry measures as fields (ArcGIS "Add Geometry Attributes"). Updates
    /// the input in place unless an output is given.
    AddGeometryAttributes {
        in_features: PathBuf,
        /// "AREA;PERIMETER_LENGTH;LENGTH;CENTROID;CENTROID_INSIDE;EXTENT;
        /// LINE_START_MID_END;LINE_BEARING;PART_COUNT;POINT_COUNT;POINT_X_Y_Z_M".
        geometry_properties: String,
        /// METERS, KILOMETERS, FEET_INT, FEET_US, MILES_INT, YARDS, NAUTICAL_MILES.
        #[arg(long)]
        length_unit: Option<String>,
        /// SQUARE_METERS, SQUARE_KILOMETERS, HECTARES, ACRES, SQUARE_FEET, SQUARE_MILES.
        #[arg(long)]
        area_unit: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Lines connecting points (ArcGIS "Points To Line").
    PointsToLine {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        #[arg(long)]
        line_field: Option<String>,
        #[arg(long)]
        sort_field: Option<String>,
        /// NO_CLOSE or CLOSE.
        #[arg(long, default_value = "NO_CLOSE")]
        close_line: String,
    },
    /// Distances to nearby features as a .dbf table (ArcGIS "Generate Near Table").
    GenerateNearTable {
        in_features: PathBuf,
        near_features: PathBuf,
        out_table: PathBuf,
        #[arg(long)]
        search_radius: Option<String>,
        /// NO_LOCATION or LOCATION.
        #[arg(long, default_value = "NO_LOCATION")]
        location: String,
        /// NO_ANGLE or ANGLE.
        #[arg(long, default_value = "NO_ANGLE")]
        angle: String,
        /// CLOSEST or ALL.
        #[arg(long, default_value = "CLOSEST")]
        closest: String,
        #[arg(long, default_value_t = 0)]
        closest_count: usize,
        /// PLANAR or GEODESIC.
        #[arg(long, default_value = "PLANAR")]
        method: String,
    },
    /// Groups of records with identical values or shapes (ArcGIS "Find Identical").
    FindIdentical {
        in_dataset: PathBuf,
        out_table: PathBuf,
        /// Fields separated by ';' ("Shape" compares geometries).
        fields: String,
        /// ALL or ONLY_DUPLICATES.
        #[arg(long, default_value = "ALL")]
        output_record_option: String,
    },
    /// One feature per part (ArcGIS "Multipart To Singlepart").
    MultipartToSinglepart {
        in_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Run a tool of the shared registry: `ug run mean_center input=a.shp output=out.shp weight_field=W`.
    Run {
        /// The tool's id (see `ug tools`).
        tool: String,
        /// `name=value` pairs; layers are paths, `output` is the result's path.
        args: Vec<String>,
    },
    /// List the tools of the shared registry with their parameters.
    Tools,
    /// Spatial Statistics "Mean Center".
    MeanCenter {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        #[arg(long)]
        weight_field: Option<String>,
        #[arg(long)]
        case_field: Option<String>,
        /// A numeric field whose (weighted) mean is added to the output.
        #[arg(long)]
        dimension_field: Option<String>,
    },
    /// Spatial Statistics "Median Center" (approximate: ArcGIS's stopping rule is not reproduced).
    MedianCenter {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        #[arg(long)]
        weight_field: Option<String>,
        #[arg(long)]
        case_field: Option<String>,
        #[arg(long)]
        dimension_field: Option<String>,
    },
    /// Spatial Statistics "Central Feature".
    CentralFeature {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// EUCLIDEAN_DISTANCE or MANHATTAN_DISTANCE.
        #[arg(long, default_value = "EUCLIDEAN_DISTANCE")]
        distance_method: String,
        #[arg(long)]
        weight_field: Option<String>,
        #[arg(long)]
        self_potential_weight_field: Option<String>,
        #[arg(long)]
        case_field: Option<String>,
    },
    /// Spatial Statistics "Standard Distance": circles around the mean center.
    StandardDistance {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// 1_STANDARD_DEVIATION, 2_STANDARD_DEVIATIONS or 3_STANDARD_DEVIATIONS.
        #[arg(long, default_value = "1_STANDARD_DEVIATION")]
        circle_size: String,
        #[arg(long)]
        weight_field: Option<String>,
        #[arg(long)]
        case_field: Option<String>,
    },
    /// Spatial Statistics "Directional Distribution": standard deviational ellipses.
    DirectionalDistribution {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// 1_STANDARD_DEVIATION, 2_STANDARD_DEVIATIONS or 3_STANDARD_DEVIATIONS.
        #[arg(long, default_value = "1_STANDARD_DEVIATION")]
        ellipse_size: String,
        #[arg(long)]
        weight_field: Option<String>,
        #[arg(long)]
        case_field: Option<String>,
    },
    /// Spatial Statistics "Linear Directional Mean" of lines.
    DirectionalMean {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// Fold directions onto 0-180 degrees (ORIENTATION_ONLY).
        #[arg(long)]
        orientation_only: bool,
        #[arg(long)]
        case_field: Option<String>,
    },
    /// A point per feature (ArcGIS "Feature To Point").
    FeatureToPoint {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// CENTROID or INSIDE.
        #[arg(long, default_value = "CENTROID")]
        point_location: String,
    },
    /// Points at vertices of lines or polygons (ArcGIS "Feature Vertices To Points").
    FeatureVerticesToPoints {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// ALL, START, END, BOTH_ENDS, MID or DANGLE.
        #[arg(long, default_value = "ALL")]
        point_location: String,
    },
    /// Bounding polygons (ArcGIS "Minimum Bounding Geometry").
    MinimumBoundingGeometry {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// RECTANGLE_BY_AREA, RECTANGLE_BY_WIDTH, CONVEX_HULL, CIRCLE or ENVELOPE.
        #[arg(long, default_value = "RECTANGLE_BY_AREA")]
        geometry_type: String,
        /// NONE, ALL or LIST (with --group-field).
        #[arg(long, default_value = "NONE")]
        group_option: String,
        #[arg(long = "group-field")]
        group_fields: Vec<String>,
        /// NO_MBG_FIELDS or MBG_FIELDS.
        #[arg(long, default_value = "NO_MBG_FIELDS")]
        mbg_fields_option: String,
    },
    /// Add POINT_X/POINT_Y (and Z/M) to point features (ArcGIS "Add XY
    /// Coordinates"). Updates the input in place unless an output is given.
    AddXy {
        in_features: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Write features as Esri JSON or GeoJSON (ArcGIS "Features To JSON").
    FeaturesToJson {
        in_features: PathBuf,
        out_json_file: PathBuf,
        /// NOT_FORMATTED or FORMATTED.
        #[arg(long, default_value = "NOT_FORMATTED")]
        format_json: String,
        /// NO_Z_VALUES or Z_VALUES.
        #[arg(long, default_value = "NO_Z_VALUES")]
        include_z_values: String,
        /// NO_M_VALUES or M_VALUES.
        #[arg(long, default_value = "NO_M_VALUES")]
        include_m_values: String,
        /// NO_GEOJSON or GEOJSON.
        #[arg(long, default_value = "NO_GEOJSON")]
        geojson: String,
    },
    /// Features from an Esri JSON or GeoJSON file (ArcGIS "JSON To Features").
    JsonToFeatures {
        in_json_file: PathBuf,
        out_features: PathBuf,
        /// For GeoJSON: POINT, MULTIPOINT, POLYLINE or POLYGON.
        #[arg(long)]
        geometry_type: Option<String>,
    },
    /// Write a table's attributes to CSV (ArcGIS "Export Table" to .csv).
    ExportTable {
        in_table: PathBuf,
        out_table: PathBuf,
    },
    /// Points from X/Y (and Z) columns of a table or CSV (ArcGIS "XY Table To Point").
    XyTableToPoint {
        in_table: PathBuf,
        out_feature_class: PathBuf,
        x_field: String,
        y_field: String,
        #[arg(long)]
        z_field: Option<String>,
        /// A factory code, .prj file or WKT (default WGS 84, as ArcGIS).
        #[arg(long, default_value = "4326")]
        coordinate_system: String,
    },
    /// Transform features to another coordinate system (ArcGIS "Project").
    Project {
        in_dataset: PathBuf,
        out_dataset: PathBuf,
        /// A factory code (4326, 32633, 3857, ...), a .prj file or WKT.
        out_coor_system: String,
        /// Geographic transformation(s) between the datums, e.g.
        /// "ED_1950_To_WGS_1984_1" or "A + B" for a chain.
        #[arg(long)]
        transform_method: Option<String>,
    },
    /// Write the `.sbn`/`.sbx` spatial index of a shapefile (ArcGIS "Add Spatial
    /// Index"); ug writes one with every shapefile anyway.
    AddSpatialIndex { in_features: PathBuf },
    /// Spatial Analyst "Slope" (planar) of a GeoTIFF elevation raster.
    Slope {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// DEGREE or PERCENT_RISE.
        #[arg(long, default_value = "DEGREE")]
        output_measurement: String,
        #[arg(long, default_value_t = 1.0)]
        z_factor: f64,
    },
    /// Spatial Analyst "Focal Statistics".
    FocalStatistics {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// "Rectangle 3 3 CELL", "Circle 2 CELL" or "Annulus 1 3 CELL".
        #[arg(long, default_value = "Rectangle 3 3 CELL")]
        neighborhood: String,
        /// MEAN, MAJORITY, MAXIMUM, MEDIAN, MINIMUM, MINORITY, RANGE, STD, SUM, VARIETY.
        #[arg(long, default_value = "MEAN")]
        statistics_type: String,
        /// DATA (ignore NoData) or NODATA.
        #[arg(long, default_value = "DATA")]
        ignore_nodata: String,
    },
    /// Spatial Analyst "Aggregate": combine blocks into coarser raster cells.
    Aggregate {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// Whole number of input cells per output cell edge (at least 2).
        #[arg(value_parser = clap::value_parser!(u32).range(2..=i32::MAX as i64))]
        cell_factor: u32,
        /// SUM, MAXIMUM, MEAN, MEDIAN or MINIMUM.
        #[arg(long, default_value = "SUM")]
        aggregation_type: String,
        /// EXPAND keeps partial edge blocks; TRUNCATE drops them.
        #[arg(long, default_value = "EXPAND")]
        extent_handling: String,
        /// DATA (ignore NoData) or NODATA.
        #[arg(long, default_value = "DATA")]
        ignore_nodata: String,
    },
    /// Spatial Analyst "Block Statistics": assign each block's statistic to all its cells.
    BlockStatistics {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// Rectangle W H CELL|MAP, Circle R CELL|MAP or Annulus R1 R2 CELL|MAP.
        #[arg(long, default_value = "Rectangle 3 3 CELL")]
        neighborhood: String,
        /// MEAN, MAJORITY, MAXIMUM, MEDIAN, MINIMUM, MINORITY, RANGE, STD, SUM, VARIETY.
        #[arg(long, default_value = "MEAN")]
        statistics_type: String,
        /// DATA (ignore NoData) or NODATA.
        #[arg(long, default_value = "DATA")]
        ignore_nodata: String,
    },
    /// Spatial Analyst "Fill": remove sinks from an elevation raster.
    Fill {
        in_surface_raster: PathBuf,
        out_surface_raster: PathBuf,
        #[arg(long)]
        z_limit: Option<f64>,
    },
    /// Spatial Analyst "Flow Direction" (D8).
    FlowDirection {
        in_surface_raster: PathBuf,
        out_flow_direction_raster: PathBuf,
        /// NORMAL or FORCE (all edge cells flow outward).
        #[arg(long, default_value = "NORMAL")]
        force_flow: String,
    },
    /// Spatial Analyst "Flow Accumulation" (D8).
    FlowAccumulation {
        in_flow_direction_raster: PathBuf,
        out_accumulation_raster: PathBuf,
        #[arg(long)]
        in_weight_raster: Option<PathBuf>,
        /// FLOAT or INTEGER.
        #[arg(long, default_value = "FLOAT")]
        data_type: String,
    },
    /// Spatial Analyst "Reclassify" on cell values.
    Reclassify {
        in_raster: PathBuf,
        /// "old new;…" or "from to new;…" (NODATA allowed as new).
        remap: String,
        out_raster: PathBuf,
        /// DATA (keep unmatched values) or NODATA.
        #[arg(long, default_value = "DATA")]
        missing_values: String,
    },
    /// Spatial Analyst "Reclass by Table": the remap read from the from, to and output fields of a table.
    ReclassByTable {
        in_raster: PathBuf,
        in_remap_table: PathBuf,
        out_raster: PathBuf,
        #[arg(long)]
        from_value_field: String,
        #[arg(long)]
        to_value_field: String,
        #[arg(long)]
        output_value_field: String,
        /// DATA (keep unmatched values) or NODATA.
        #[arg(long, default_value = "DATA")]
        missing_values: String,
    },
    /// Spatial Analyst "Zonal Statistics".
    ZonalStatistics {
        in_zone_data: PathBuf,
        in_value_raster: PathBuf,
        out_raster: PathBuf,
        /// MEAN, MAJORITY, MAXIMUM, MEDIAN, MINIMUM, MINORITY, RANGE, STD, SUM, VARIETY.
        #[arg(long, default_value = "MEAN")]
        statistics_type: String,
        /// DATA (ignore NoData) or NODATA.
        #[arg(long, default_value = "DATA")]
        ignore_nodata: String,
    },
    /// Spatial Analyst "Curvature".
    Curvature {
        in_raster: PathBuf,
        out_curvature_raster: PathBuf,
        #[arg(long, default_value_t = 1.0)]
        z_factor: f64,
        #[arg(long)]
        out_profile_curve_raster: Option<PathBuf>,
        #[arg(long)]
        out_plan_curve_raster: Option<PathBuf>,
    },
    /// Spatial Analyst "Plus": raster or constant operands.
    Plus {
        #[arg(allow_negative_numbers = true)]
        in1: String,
        #[arg(allow_negative_numbers = true)]
        in2: String,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Minus".
    Minus {
        #[arg(allow_negative_numbers = true)]
        in1: String,
        #[arg(allow_negative_numbers = true)]
        in2: String,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Times".
    Times {
        #[arg(allow_negative_numbers = true)]
        in1: String,
        #[arg(allow_negative_numbers = true)]
        in2: String,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Divide" (integer inputs divide to a truncated integer).
    Divide {
        #[arg(allow_negative_numbers = true)]
        in1: String,
        #[arg(allow_negative_numbers = true)]
        in2: String,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Int": truncate to integer.
    Int {
        in_raster: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Float".
    Float {
        in_raster: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Is Null".
    IsNull {
        in_raster: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Con": conditional choice per cell.
    Con {
        in_conditional_raster: PathBuf,
        #[arg(allow_negative_numbers = true)]
        in_true_raster_or_constant: String,
        out_raster: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        in_false_raster_or_constant: Option<String>,
        /// e.g. "Value > 3".
        #[arg(long = "where")]
        where_clause: Option<String>,
    },
    /// Spatial Analyst "Set Null".
    SetNull {
        in_conditional_raster: PathBuf,
        #[arg(allow_negative_numbers = true)]
        in_false_raster_or_constant: String,
        out_raster: PathBuf,
        #[arg(long = "where")]
        where_clause: Option<String>,
    },
    /// Spatial Analyst "Contour": polylines with fields Id and Contour.
    Contour {
        in_raster: PathBuf,
        out_polyline_features: PathBuf,
        contour_interval: f64,
        #[arg(long, default_value_t = 0.0)]
        base_contour: f64,
    },
    /// Spatial Analyst "Basin".
    Basin {
        in_flow_direction_raster: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Watershed" (raster pour points).
    Watershed {
        in_flow_direction_raster: PathBuf,
        in_pour_point_data: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Stream Link".
    StreamLink {
        in_stream_raster: PathBuf,
        in_flow_direction_raster: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Stream Order".
    StreamOrder {
        in_stream_raster: PathBuf,
        in_flow_direction_raster: PathBuf,
        out_raster: PathBuf,
        /// STRAHLER or SHREVE.
        #[arg(long, default_value = "STRAHLER")]
        order_method: String,
    },
    /// Spatial Analyst "Flow Length" (unweighted).
    FlowLength {
        in_flow_direction_raster: PathBuf,
        out_raster: PathBuf,
        /// DOWNSTREAM or UPSTREAM.
        #[arg(long, default_value = "DOWNSTREAM")]
        direction_measurement: String,
    },
    /// Conversion "Raster To Point": cell centers with pointid and grid_code.
    RasterToPoint {
        in_raster: PathBuf,
        out_point_features: PathBuf,
    },
    /// Conversion "Raster To Polygon" (NO_SIMPLIFY): Id and gridcode.
    RasterToPolygon {
        in_raster: PathBuf,
        out_polygon_features: PathBuf,
        /// Only NO_SIMPLIFY is supported.
        #[arg(long, default_value = "NO_SIMPLIFY")]
        simplify: String,
    },
    /// Conversion "Feature To Raster": polygons by cell center, lines by the cells
    /// they cross, points by the most frequent value.
    FeatureToRaster {
        in_features: PathBuf,
        field: String,
        out_raster: PathBuf,
        cell_size: f64,
    },
    /// Conversion "Polygon To Raster".
    PolygonToRaster {
        in_features: PathBuf,
        value_field: String,
        out_raster: PathBuf,
        /// CELL_CENTER, MAXIMUM_AREA or MAXIMUM_COMBINED_AREA.
        #[arg(long, default_value = "CELL_CENTER")]
        cell_assignment: String,
        #[arg(long)]
        cellsize: f64,
    },
    /// Conversion "Polyline To Raster".
    PolylineToRaster {
        in_features: PathBuf,
        value_field: String,
        out_raster: PathBuf,
        /// MAXIMUM_LENGTH or MAXIMUM_COMBINED_LENGTH.
        #[arg(long, default_value = "MAXIMUM_LENGTH")]
        cell_assignment: String,
        #[arg(long)]
        cellsize: f64,
    },
    /// Spatial Analyst "IDW": inverse distance weighted interpolation of a point field.
    Idw {
        in_features: PathBuf,
        z_field: String,
        out_raster: PathBuf,
        /// Default: the shorter side of the points' extent / 250.
        #[arg(long)]
        cellsize: Option<f64>,
        #[arg(long, default_value_t = 2.0)]
        power: f64,
        /// VARIABLE or FIXED.
        #[arg(long, default_value = "VARIABLE")]
        search_radius: String,
        /// Variable: the nearest points used.
        #[arg(long, default_value_t = 12)]
        num_points: usize,
        /// Variable: no point farther than this is used.
        #[arg(long)]
        max_distance: Option<f64>,
        /// Fixed: the radius (default five cells).
        #[arg(long)]
        distance: Option<f64>,
        /// Fixed: a cell with fewer points inside widens the radius a cell at a time.
        #[arg(long, default_value_t = 0)]
        min_points: usize,
        /// The extent environment: xmin ymin xmax ymax.
        #[arg(long, num_args = 4, value_names = ["XMIN", "YMIN", "XMAX", "YMAX"])]
        extent: Option<Vec<f64>>,
    },
    /// Spatial Analyst math function (Abs, Exp, Ln, Sin, ..., Power, Mod, EqualTo, BooleanAnd, Diff,
    /// BitwiseAnd, ...): `ug math-fn Power dem.tif 2 out.tif`; operands are rasters or numbers.
    MathFn {
        function: String,
        /// One or two operands, then the output raster.
        #[arg(required = true, num_args = 2..=3)]
        paths: Vec<String>,
    },
    /// Spatial Analyst "Cell Statistics" over several rasters.
    CellStatistics {
        /// The input rasters, then the output raster.
        #[arg(required = true, num_args = 2..)]
        paths: Vec<PathBuf>,
        /// MEAN, MAJORITY, MAXIMUM, MEDIAN, MINIMUM, MINORITY, RANGE, STD, SUM or VARIETY.
        #[arg(long, default_value = "MEAN")]
        statistics_type: String,
        /// DATA or NODATA.
        #[arg(long, default_value = "DATA")]
        ignore_nodata: String,
    },
    /// Spatial Analyst "Weighted Sum": raster=weight pairs, then the output raster.
    WeightedSum {
        /// `raster.tif=0.5` entries, then the output raster.
        #[arg(required = true, num_args = 2..)]
        paths: Vec<String>,
    },
    /// Spatial Analyst "Highest Position" (or Lowest with --lowest).
    HighestPosition {
        #[arg(required = true, num_args = 2..)]
        paths: Vec<PathBuf>,
        #[arg(long)]
        lowest: bool,
    },
    /// Spatial Analyst "Combine": a value per distinct combination.
    Combine {
        #[arg(required = true, num_args = 2..)]
        paths: Vec<PathBuf>,
    },
    /// Spatial Analyst "Fuzzy Membership".
    FuzzyMembership {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// LINEAR, GAUSSIAN, LARGE, SMALL, NEAR, MSLARGE or MSSMALL.
        function: String,
        /// The function's numbers: LINEAR min max; GAUSSIAN/LARGE/SMALL/NEAR midpoint spread;
        /// MSLARGE/MSSMALL mean-multiplier std-multiplier.
        #[arg(num_args = 0..=2)]
        values: Vec<f64>,
        /// NONE, SOMEWHAT or VERY.
        #[arg(long, default_value = "NONE")]
        hedge: String,
    },
    /// Spatial Analyst "Fuzzy Overlay" of membership rasters.
    FuzzyOverlay {
        #[arg(required = true, num_args = 2..)]
        paths: Vec<PathBuf>,
        /// AND, OR, PRODUCT, SUM or GAMMA.
        #[arg(long, default_value = "AND")]
        overlay_type: String,
        #[arg(long)]
        gamma: Option<f64>,
    },
    /// Conversion "Point To Raster".
    PointToRaster {
        in_features: PathBuf,
        value_field: String,
        out_raster: PathBuf,
        /// MOST_FREQUENT, SUM, MEAN, STANDARD_DEVIATION, MAXIMUM, MINIMUM, RANGE or COUNT.
        #[arg(long, default_value = "MOST_FREQUENT")]
        cell_assignment: String,
        #[arg(long)]
        cellsize: f64,
    },
    /// Data Management "Resample" to square cells.
    Resample {
        in_raster: PathBuf,
        out_raster: PathBuf,
        cell_size: f64,
        /// NEAREST, BILINEAR, CUBIC or MAJORITY.
        #[arg(long, default_value = "NEAREST")]
        resampling_type: String,
    },
    /// Spatial Analyst "Euclidean Distance" from a raster source.
    EucDistance {
        in_source_data: PathBuf,
        out_distance_raster: PathBuf,
        #[arg(long)]
        maximum_distance: Option<f64>,
    },
    /// Spatial Analyst "Euclidean Direction" (azimuth to the nearest source cell).
    EucDirection {
        in_source_data: PathBuf,
        out_direction_raster: PathBuf,
        #[arg(long)]
        maximum_distance: Option<f64>,
    },
    /// Spatial Analyst "Expand": grow zones by a number of cells.
    Expand {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// Cells to expand by (rounded half to even, at least 1).
        number_cells: f64,
        /// Zone values, separated by ';'.
        #[arg(allow_hyphen_values = true)]
        zone_values: String,
        /// MORPHOLOGICAL or DISTANCE.
        #[arg(long, default_value = "MORPHOLOGICAL")]
        expand_method: String,
    },
    /// Spatial Analyst "Shrink": shrink zones by a number of cells.
    Shrink {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// Cells to shrink by (rounded half to even, at least 1).
        number_cells: f64,
        /// Zone values, separated by ';'.
        #[arg(allow_hyphen_values = true)]
        zone_values: String,
        /// MORPHOLOGICAL or DISTANCE.
        #[arg(long, default_value = "MORPHOLOGICAL")]
        shrink_method: String,
    },
    /// Spatial Analyst "Majority Filter".
    MajorityFilter {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// FOUR or EIGHT.
        #[arg(long, default_value = "FOUR")]
        number_neighbors: String,
        /// MAJORITY or HALF.
        #[arg(long, default_value = "MAJORITY")]
        majority_definition: String,
    },
    /// Spatial Analyst "Region Group".
    RegionGroup {
        in_raster: PathBuf,
        out_raster: PathBuf,
        /// FOUR or EIGHT.
        #[arg(long, default_value = "FOUR")]
        number_neighbors: String,
        /// WITHIN or CROSS.
        #[arg(long, default_value = "WITHIN")]
        zone_connectivity: String,
        /// ADD_LINK or NO_LINK (the attribute table is not written).
        #[arg(long, default_value = "ADD_LINK")]
        add_link: String,
        /// Value that joins no region (0 in the output).
        #[arg(long)]
        excluded_value: Option<f64>,
    },
    /// Spatial Analyst "Euclidean Allocation" (value of the nearest source cell).
    EucAllocation {
        in_source_data: PathBuf,
        out_allocation_raster: PathBuf,
        #[arg(long)]
        maximum_distance: Option<f64>,
    },
    /// Spatial Analyst "Kernel Density" of points or lines.
    KernelDensity {
        in_features: PathBuf,
        /// A numeric field, or NONE.
        population_field: String,
        out_raster: PathBuf,
        cell_size: f64,
        #[arg(long)]
        search_radius: Option<f64>,
        #[arg(long, default_value = "SQUARE_KILOMETERS")]
        area_unit_scale_factor: String,
        /// DENSITIES or EXPECTED_COUNTS.
        #[arg(long, default_value = "DENSITIES")]
        out_cell_values: String,
    },
    /// Spatial Analyst "Point Density".
    PointDensity {
        in_point_features: PathBuf,
        /// A numeric field, or NONE.
        population_field: String,
        out_raster: PathBuf,
        cell_size: f64,
        /// For example "Circle 100 MAP", "Rectangle 80 120 CELL", "Annulus 2 5 CELL",
        /// "Wedge 120 90 180 MAP".
        #[arg(long, default_value = "Circle 5 CELL")]
        neighborhood: String,
        #[arg(long, default_value = "SQUARE_KILOMETERS")]
        area_unit_scale_factor: String,
    },
    /// Spatial Analyst "Line Density".
    LineDensity {
        in_polyline_features: PathBuf,
        /// A numeric field, or NONE.
        population_field: String,
        out_raster: PathBuf,
        cell_size: f64,
        search_radius: f64,
        #[arg(long, default_value = "SQUARE_KILOMETERS")]
        area_unit_scale_factor: String,
    },
    /// Spatial Analyst "Extract Values to Points" (RASTERVALU).
    ExtractValuesToPoints {
        in_point_features: PathBuf,
        in_raster: PathBuf,
        out_point_features: PathBuf,
        /// NONE or INTERPOLATE (bilinear).
        #[arg(long, default_value = "NONE")]
        interpolate_values: String,
        /// VALUE_ONLY or ALL (the raster's attribute table is not joined).
        #[arg(long, default_value = "VALUE_ONLY")]
        add_attributes: String,
    },
    /// Spatial Analyst "Extract by Mask".
    ExtractByMask {
        in_raster: PathBuf,
        in_mask_data: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Zonal Statistics as Table" (.dbf).
    ZonalStatisticsAsTable {
        in_zone_data: PathBuf,
        in_value_raster: PathBuf,
        out_table: PathBuf,
        /// DATA (ignore NoData) or NODATA.
        #[arg(long, default_value = "DATA")]
        ignore_nodata: String,
        /// ALL, MEAN, MIN_MAX_MEAN, …
        #[arg(long, default_value = "ALL")]
        statistics_type: String,
    },
    /// Spatial Analyst "Cost Distance": the least accumulated cost from the source cells over a cost raster.
    CostDistance {
        in_source_data: PathBuf,
        in_cost_raster: PathBuf,
        out_distance_raster: PathBuf,
        #[arg(long)]
        maximum_distance: Option<f64>,
        /// Also write the back link raster here.
        #[arg(long)]
        out_backlink_raster: Option<PathBuf>,
    },
    /// Spatial Analyst "Cost Back Link": the direction (1 east, clockwise to 8) to the cheapest path's previous cell.
    CostBackLink {
        in_source_data: PathBuf,
        in_cost_raster: PathBuf,
        out_backlink_raster: PathBuf,
        #[arg(long)]
        maximum_distance: Option<f64>,
    },
    /// Spatial Analyst "Cost Allocation": the source each cell is cheapest to reach from.
    CostAllocation {
        in_source_data: PathBuf,
        in_cost_raster: PathBuf,
        out_allocation_raster: PathBuf,
        #[arg(long)]
        maximum_distance: Option<f64>,
    },
    /// Spatial Analyst "Corridor": the sum of two accumulated cost rasters.
    Corridor {
        in_distance_raster1: PathBuf,
        in_distance_raster2: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Zonal Histogram": a row per value, a column per zone (.dbf or geodatabase table).
    ZonalHistogram {
        in_zone_data: PathBuf,
        in_value_raster: PathBuf,
        out_table: PathBuf,
        /// The zone field (its name starts the column names).
        #[arg(long, default_value = "Value")]
        zone_field: String,
    },
    /// Spatial Analyst "Tabulate Area": the area of each class in each zone.
    TabulateArea {
        in_zone_data: PathBuf,
        in_class_data: PathBuf,
        out_table: PathBuf,
        #[arg(long, default_value = "Value")]
        zone_field: String,
        #[arg(long, default_value = "Value")]
        class_field: String,
        #[arg(long)]
        processing_cell_size: Option<f64>,
        /// One row per zone and class (CLASSES_AS_ROWS).
        #[arg(long)]
        classes_as_rows: bool,
    },
    /// Spatial Analyst "Viewshed" (approximate): how many observer points see
    /// each cell; observers' OFFSETA field (default 1) is their height.
    Viewshed {
        in_raster: PathBuf,
        in_observer_features: PathBuf,
        out_raster: PathBuf,
        #[arg(long, default_value_t = 1.0)]
        z_factor: f64,
    },
    /// Spatial Analyst "Aspect" (planar): downslope direction, −1 on flat cells.
    Aspect {
        in_raster: PathBuf,
        out_raster: PathBuf,
    },
    /// Spatial Analyst "Hillshade" without shadows.
    Hillshade {
        in_raster: PathBuf,
        out_raster: PathBuf,
        #[arg(long, default_value_t = 315.0)]
        azimuth: f64,
        #[arg(long, default_value_t = 45.0)]
        altitude: f64,
        /// NO_SHADOWS or SHADOWS.
        #[arg(long, default_value = "NO_SHADOWS")]
        model_shadows: String,
        #[arg(long, default_value_t = 1.0)]
        z_factor: f64,
    },
    /// List geometry problems (ArcGIS "Check Geometry") into a .dbf table:
    /// CLASS, FEATURE_ID, PROBLEM.
    CheckGeometry {
        /// One feature class, or several joined with ';'.
        in_features: String,
        out_table: PathBuf,
        /// ESRI or OGC.
        #[arg(long, default_value = "ESRI")]
        validation_method: String,
    },
    /// Repair geometry problems (ArcGIS "Repair Geometry"). Updates the input
    /// shapefile in place, as ArcGIS does, unless an output is given.
    RepairGeometry {
        in_features: PathBuf,
        /// DELETE_NULL or KEEP_NULL.
        #[arg(long, default_value = "DELETE_NULL")]
        delete_null: String,
        /// ESRI or OGC.
        #[arg(long, default_value = "ESRI")]
        validation_method: String,
        /// Write the result here instead of updating the input.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Merge features by attribute values (ArcGIS "Dissolve").
    Dissolve {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// Field(s) to group by (repeatable). None: dissolve everything into one.
        #[arg(long = "dissolve-field")]
        dissolve_fields: Vec<String>,
        /// Statistics as ArcGIS writes them: "VALUE SUM;PARCEL_ID COUNT".
        #[arg(long)]
        statistics_fields: Option<String>,
        /// MULTI_PART (default) or SINGLE_PART.
        #[arg(long, default_value = "MULTI_PART")]
        multi_part: String,
    },
    /// Buffer features by a distance (ArcGIS "Buffer", planar).
    Buffer {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// Linear distance, e.g. "25 Meters" (negative shrinks polygons), or the
        /// name of a numeric field holding each feature's distance.
        #[arg(allow_hyphen_values = true)]
        buffer_distance: String,
        /// FULL, LEFT or RIGHT (lines), OUTSIDE_ONLY (polygons).
        #[arg(long, default_value = "FULL")]
        line_side: String,
        /// ROUND or FLAT ends for lines.
        #[arg(long, default_value = "ROUND")]
        line_end_type: String,
        /// NONE (one buffer per feature), ALL (dissolve into one) or LIST (one per
        /// combination of the --dissolve-field values).
        #[arg(long, default_value = "NONE")]
        dissolve_option: String,
        /// Fields for dissolve_option LIST (repeatable).
        #[arg(long = "dissolve-field")]
        dissolve_fields: Vec<String>,
        /// Largest angle of an arc segment in degrees for line and polygon input (Pairwise
        /// Buffer uses 3.75).
        #[arg(long, hide = true)]
        max_arc_angle: Option<f64>,
    },
}

fn main() -> ExitCode {
    let tool = Cli::parse_from(aliases::rewrite(std::env::args().collect())).tool;
    if let Err(e) = check_64bit_oids(&tool) {
        eprintln!("ERROR: {e}");
        return ExitCode::FAILURE;
    }
    let result = match tool {
        Tool::Describe { dataset } => describe(&dataset),
        Tool::CopyFeatures {
            in_features,
            out_feature_class,
        } => copy_features(&in_features, &out_feature_class),
        Tool::FeatureClassToFeatureClass {
            in_features,
            out_feature_class,
            where_clause,
            field_mapping,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let field_map = field_mapping
                .as_deref()
                .map(unigis_tools::parse_field_mapping)
                .transpose()?;
            let out = unigis_tools::feature_class_to_feature_class(
                fc.as_ref(),
                &catalog_path(&in_features),
                where_clause.as_deref(),
                field_map.as_deref(),
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::Clip {
            in_features,
            clip_features,
            out_feature_class,
        } => clip_tool(&in_features, &clip_features, &out_feature_class),
        Tool::Intersect {
            mut paths,
            join_attributes,
            output_type,
            merge_pieces,
        } => (|| {
            let out = paths.pop().unwrap();
            let opts = unigis_tools::IntersectOptions {
                join_attributes: join_attributes_arg(&join_attributes)?,
                output_type: unigis_tools::OutputType::parse(&output_type)
                    .ok_or_else(|| Error::Unsupported(format!("output_type {output_type}")))?,
            };
            overlay_tool(&paths, &out, |i| {
                let mut out = unigis_tools::intersect_with(i, &opts)?;
                if merge_pieces {
                    merge_pieces_with_equal_attributes(&mut out)?;
                    // Pairwise Intersect with POINT output finds no points where lines cross polygons
                    // (observed).
                    let polygons = paths.iter().any(|p| {
                        open_feature_class(p).is_ok_and(|(fc, _)| {
                            fc.geometry_kind() == Some(unigis_core::GeometryKind::Polygon)
                        })
                    });
                    if polygons
                        && output_type.eq_ignore_ascii_case("POINT")
                        && matches!(
                            out.kind,
                            unigis_core::GeometryKind::Point
                                | unigis_core::GeometryKind::Multipoint
                        )
                    {
                        out.features.clear();
                    }
                }
                Ok(out)
            })
        })(),
        Tool::PolygonToLine {
            in_features,
            out_feature_class,
            neighbor_option,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = match neighbor_option.to_ascii_uppercase().as_str() {
                "IDENTIFY_NEIGHBORS" => {
                    unigis_tools::planar::polygon_to_line_neighbors(fc.as_ref())?
                }
                "IGNORE_NEIGHBORS" => unigis_tools::construct::polygon_to_line(fc.as_ref())?,
                other => return Err(Error::Unsupported(format!("neighbor_option {other}"))),
            };
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::FeatureToPolygon {
            mut paths,
            cluster_tolerance,
            attributes,
            label_features,
        } => (|| {
            let out = paths.pop().unwrap();
            let attributes = attributes_arg(&attributes)?;
            let labels = label_features
                .as_deref()
                .map(open_feature_class)
                .transpose()?
                .map(|(f, _)| f);
            overlay_tool(&paths, &out, |i| {
                let tol = cluster_tolerance_arg(cluster_tolerance.as_deref(), i)?;
                unigis_tools::planar::feature_to_polygon(i, tol, attributes, labels.as_deref())
            })
        })(),
        Tool::FeatureToLine {
            mut paths,
            cluster_tolerance,
            attributes,
        } => (|| {
            let out = paths.pop().unwrap();
            let attributes = attributes_arg(&attributes)?;
            overlay_tool(&paths, &out, |i| {
                let tol = cluster_tolerance_arg(cluster_tolerance.as_deref(), i)?;
                unigis_tools::planar::feature_to_line(i, tol, attributes)
            })
        })(),
        Tool::Union {
            mut paths,
            join_attributes,
            gaps,
        } => (|| {
            let out = paths.pop().unwrap();
            let join = join_attributes_arg(&join_attributes)?;
            let gaps = match gaps.to_ascii_uppercase().as_str() {
                "GAPS" => true,
                "NO_GAPS" => false,
                other => return Err(Error::Unsupported(format!("gaps {other}"))),
            };
            overlay_tool(&paths, &out, |i| unigis_tools::union_with(i, join, gaps))
        })(),
        Tool::SymmetricalDifference {
            in_features,
            update_features,
            out_feature_class,
        } => overlay_tool(
            &[in_features, update_features],
            &out_feature_class,
            unigis_tools::symmetrical_difference,
        ),
        Tool::Update {
            in_features,
            update_features,
            out_feature_class,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let (up, _) = open_feature_class(&update_features)?;
            write_output(
                unigis_tools::update(fc.as_ref(), up.as_ref())?,
                &out_feature_class,
                source_gdb(&in_features).as_deref(),
            )
        })(),
        Tool::Erase {
            in_features,
            erase_features,
            out_feature_class,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let (er, _) = open_feature_class(&erase_features)?;
            write_output(
                unigis_tools::erase(fc.as_ref(), er.as_ref())?,
                &out_feature_class,
                source_gdb(&in_features).as_deref(),
            )
        })(),
        Tool::Identity {
            in_features,
            identity_features,
            out_feature_class,
            join_attributes,
            relationship,
        } => (|| {
            let opts = unigis_tools::IdentityOptions {
                join_attributes: join_attributes_arg(&join_attributes)?,
                keep_relationships: match relationship.to_ascii_uppercase().as_str() {
                    "NO_RELATIONSHIPS" => false,
                    "KEEP_RELATIONSHIPS" => true,
                    other => return Err(Error::Unsupported(format!("relationship {other}"))),
                },
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let (id, _) = open_feature_class(&identity_features)?;
            let out = unigis_tools::identity_with(
                unigis_tools::NamedInput {
                    name: dataset_name(&in_features),
                    features: fc.as_ref(),
                },
                unigis_tools::NamedInput {
                    name: dataset_name(&identity_features),
                    features: id.as_ref(),
                },
                &opts,
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::Merge {
            mut paths,
            add_source,
            field_mapping,
        } => {
            let out = paths.pop().unwrap();
            merge_tool(&paths, &out, &add_source, field_mapping.as_deref())
        }
        Tool::Near {
            in_features,
            near_features,
            out_feature_class,
            search_radius,
            location,
            angle,
            method,
        } => near_tool(
            &in_features,
            &near_features,
            out_feature_class.as_deref(),
            search_radius.as_deref(),
            &location,
            &angle,
            &method,
        ),
        Tool::SpatialJoin {
            target_features,
            join_features,
            out_feature_class,
            join_operation,
            join_type,
            match_option,
            search_radius,
            distance_field_name,
            merge_rules,
            field_mapping,
        } => (|| {
            let field_map = field_mapping
                .as_deref()
                .map(unigis_tools::parse_field_mapping)
                .transpose()?;
            let merge_rules = merge_rules
                .iter()
                .map(|m| {
                    let (field, rule) = m.split_once('=').ok_or_else(|| {
                        Error::Unsupported(format!("merge rule `{m}`: expected FIELD=RULE"))
                    })?;
                    let (rule, delim) = match rule.split_once(':') {
                        Some((r, d)) => (r, Some(d)),
                        None => (rule, None),
                    };
                    let rule = unigis_tools::MergeRule::parse(rule, delim)
                        .ok_or_else(|| Error::Unsupported(format!("merge rule {rule}")))?;
                    Ok((field.to_owned(), rule))
                })
                .collect::<Result<Vec<_>>>()?;
            let (target, _) = open_feature_class(&target_features)?;
            let (join, _) = open_feature_class(&join_features)?;
            let opts = unigis_tools::SpatialJoinOptions {
                one_to_many: match join_operation.to_ascii_uppercase().as_str() {
                    "JOIN_ONE_TO_ONE" => false,
                    "JOIN_ONE_TO_MANY" => true,
                    other => return Err(Error::Unsupported(format!("join_operation {other}"))),
                },
                keep_all: match join_type.to_ascii_uppercase().as_str() {
                    "KEEP_ALL" => true,
                    "KEEP_COMMON" => false,
                    other => return Err(Error::Unsupported(format!("join_type {other}"))),
                },
                match_option: unigis_tools::MatchOption::parse(&match_option)
                    .ok_or_else(|| Error::Unsupported(format!("match_option {match_option}")))?,
                search_radius: search_radius
                    .as_deref()
                    .map(|r| parse_distance(r, target.spatial_reference_wkt()))
                    .transpose()?,
                distance_field_name,
                merge_rules,
                field_map,
                target_name: unigis_tools::table_stem(&target_features.to_string_lossy()),
                join_name: unigis_tools::table_stem(&join_features.to_string_lossy()),
            };
            let out = unigis_tools::spatial_join(target.as_ref(), join.as_ref(), &opts)?;
            write_output(
                out,
                &out_feature_class,
                source_gdb(&target_features).as_deref(),
            )
        })(),
        Tool::SelectByLocation {
            in_features,
            overlap_type,
            select_features,
            out_feature_class,
            search_distance,
            invert_spatial_relationship,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let (sel, _) = open_feature_class(&select_features)?;
            let overlap = unigis_tools::Overlap::parse(&overlap_type)
                .ok_or_else(|| Error::Unsupported(format!("overlap_type {overlap_type}")))?;
            let invert = match invert_spatial_relationship.to_ascii_uppercase().as_str() {
                "INVERT" => true,
                "NOT_INVERT" => false,
                other => return Err(Error::Unsupported(format!("invert {other}"))),
            };
            let distance = search_distance
                .as_deref()
                .map(|d| parse_distance(d, fc.spatial_reference_wkt()))
                .transpose()?;
            let out = unigis_tools::select_by_location(
                fc.as_ref(),
                overlap,
                sel.as_ref(),
                distance,
                invert,
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::Append {
            mut paths,
            schema_type,
            output,
            field_mapping,
        } => (|| {
            let target_path = paths.pop().unwrap();
            let test = match schema_type.to_ascii_uppercase().as_str() {
                "TEST" => true,
                "NO_TEST" => false,
                other => return Err(Error::Unsupported(format!("schema_type {other}"))),
            };
            let (target, _) = open_feature_class(&target_path)?;
            let inputs: Vec<Box<dyn FeatureClass>> = paths
                .iter()
                .map(|p| open_feature_class(p).map(|(f, _)| f))
                .collect::<Result<_>>()?;
            let field_map = field_mapping
                .as_deref()
                .map(unigis_tools::parse_field_mapping)
                .transpose()?;
            let named: Vec<unigis_tools::PathInput> = paths
                .iter()
                .zip(&inputs)
                .map(|(p, f)| unigis_tools::PathInput {
                    source: catalog_path(p),
                    features: f.as_ref(),
                })
                .collect();
            let out =
                unigis_tools::append_with(&named, target.as_ref(), test, field_map.as_deref())?;
            drop(target);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&target_path).as_deref()),
                None => update_in_place(out, &target_path),
            }
        })(),
        Tool::Split {
            in_features,
            split_features,
            split_field,
            out_workspace,
        } => split_tool(&in_features, &split_features, &split_field, &out_workspace),
        Tool::AddField {
            in_table,
            field_name,
            field_type,
            precision,
            scale,
            length,
            output,
        } => (|| {
            let (fc, _) = open_feature_class(&in_table)?;
            let out = unigis_tools::add_field(
                fc.as_ref(),
                &field_name,
                &field_type,
                precision,
                scale,
                length,
            )?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_table).as_deref()),
                None => update_in_place(out, &in_table),
            }
        })(),
        Tool::DeleteField {
            in_table,
            drop_field,
            method,
            output,
        } => (|| {
            let keep = match method.to_ascii_uppercase().as_str() {
                "DELETE_FIELDS" => false,
                "KEEP_FIELDS" => true,
                other => return Err(Error::Unsupported(format!("method {other}"))),
            };
            let names: Vec<String> = drop_field
                .split(';')
                .map(|f| f.trim().to_owned())
                .filter(|f| !f.is_empty())
                .collect();
            let (fc, _) = open_feature_class(&in_table)?;
            let out = unigis_tools::delete_field(fc.as_ref(), &names, keep)?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_table).as_deref()),
                None => update_in_place(out, &in_table),
            }
        })(),
        Tool::DeleteRows {
            in_rows,
            where_clause,
        } => delete_rows_tool(&in_rows, where_clause.as_deref(), false),
        Tool::DeleteFeatures {
            in_features,
            where_clause,
        } => delete_rows_tool(&in_features, where_clause.as_deref(), false),
        Tool::TruncateTable { in_table } => delete_rows_tool(&in_table, None, true),
        Tool::CalculateField {
            in_table,
            field,
            expression,
            code_block,
            field_type,
            output,
        } => (|| {
            let (fc, _) = open_feature_class(&in_table)?;
            let out = unigis_tools::calculate_field(
                fc.as_ref(),
                &field,
                &expression,
                &code_block,
                field_type.as_deref(),
            )?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_table).as_deref()),
                None => update_in_place(out, &in_table),
            }
        })(),
        Tool::CreateFeatureDataset {
            out_dataset_path,
            out_name,
            spatial_reference,
        } => (|| {
            let wkt = match spatial_reference
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(s) => Some(unigis_proj::Crs::parse(s)?.arcgis_wkt()),
                None => None,
            };
            let mut w = open_geodatabase_writer(&out_dataset_path)?;
            w.create_feature_dataset(&out_name, wkt.as_deref())?;
            w.finish()?;
            println!(
                "Created feature dataset {}",
                out_dataset_path.join(&out_name).display()
            );
            Ok(())
        })(),
        Tool::SetSubtypeField {
            in_table,
            subtype_field,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::set_subtype_field(&gdb, &table, &subtype_field)
        })(),
        Tool::AddSubtype {
            in_table,
            subtype_code,
            subtype_description,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::add_subtype(&gdb, &table, subtype_code, &subtype_description)
        })(),
        Tool::RemoveSubtype {
            in_table,
            subtype_code,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::remove_subtype(&gdb, &table, &codes(&subtype_code)?)
        })(),
        Tool::SetDefaultSubtype {
            in_table,
            subtype_code,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::set_default_subtype(&gdb, &table, subtype_code)
        })(),
        Tool::AssignDomainToField {
            in_table,
            field_name,
            domain_name,
            subtype_code,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::assign_domain_to_field(
                &gdb,
                &table,
                &field_name,
                &domain_name,
                &codes(&subtype_code)?,
            )
        })(),
        Tool::RemoveDomainFromField {
            in_table,
            field_name,
            subtype_code,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::remove_domain_from_field(
                &gdb,
                &table,
                &field_name,
                &codes(&subtype_code)?,
            )
        })(),
        Tool::AddIndex {
            in_table,
            field,
            index_name,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            if field.contains(';') {
                return Err(Error::Unsupported(
                    "indexes on several fields are not done".into(),
                ));
            }
            unigis_filegdb::add_attribute_index(&gdb, &table, &field, &index_name)
        })(),
        Tool::RemoveIndex {
            in_table,
            index_name,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::remove_attribute_index(&gdb, &table, &index_name)
        })(),
        Tool::AlterField {
            in_table,
            field,
            new_field_name,
            new_field_alias,
            field_type,
            field_length,
            field_is_nullable,
            clear_field_alias,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            unigis_filegdb::alter_field(
                &gdb,
                &table,
                &field,
                &unigis_filegdb::FieldAlteration {
                    new_name: new_field_name.as_deref(),
                    alias: new_field_alias.as_deref().filter(|a| *a != "#"),
                    clear_alias: clear_field_alias
                        .as_deref()
                        .is_some_and(|c| c.eq_ignore_ascii_case("CLEAR_ALIAS")),
                    new_type: alter_field_type(field_type.as_deref())?,
                    length: field_length,
                    nullable: alter_nullable(field_is_nullable.as_deref())?,
                },
            )
        })(),
        Tool::AlterFields { in_table, fields } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            // `field|new name|new alias|type|length|nullable|clear alias`, empty or `#` to skip.
            let specs: Vec<Vec<Option<&str>>> = fields
                .iter()
                .map(|f| {
                    f.split('|')
                        .map(str::trim)
                        .map(|p| Some(p).filter(|p| !p.is_empty() && *p != "#"))
                        .collect()
                })
                .collect();
            let mut rows = Vec::new();
            for spec in &specs {
                let part = |i: usize| spec.get(i).copied().flatten();
                let name = part(0).ok_or_else(|| {
                    Error::Unsupported("ERROR 000735: Field Name: Value is required".into())
                })?;
                rows.push((
                    name,
                    unigis_filegdb::FieldAlteration {
                        new_name: part(1),
                        alias: part(2),
                        clear_alias: part(6).is_some_and(|c| c.eq_ignore_ascii_case("CLEAR_ALIAS")),
                        new_type: alter_field_type(part(3))?,
                        length: part(4).and_then(|l| l.parse().ok()),
                        nullable: alter_nullable(part(5))?,
                    },
                ));
            }
            unigis_filegdb::alter_fields(&gdb, &table, &rows)
        })(),
        Tool::AssignDefaultToField {
            in_table,
            field_name,
            default_value,
            subtype_code,
        } => (|| {
            let (gdb, table) = gdb_table(&in_table)?;
            // Numbers by their look; the field's type decides what fits.
            let value = default_value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| {
                    v.parse::<i64>()
                        .map(Value::BigInteger)
                        .or_else(|_| v.parse::<f64>().map(Value::Double))
                        .unwrap_or_else(|_| Value::String(v.to_string()))
                });
            unigis_filegdb::assign_default_to_field(
                &gdb,
                &table,
                &field_name,
                value.as_ref(),
                &codes(&subtype_code)?,
            )
        })(),
        Tool::CreateRelationshipClass {
            origin_table,
            destination_table,
            out_relationship_class,
            relationship_type,
            forward_label,
            backward_label,
            message_direction,
            cardinality,
            attributed,
            origin_primary_key,
            origin_foreign_key,
            destination_primary_key,
            destination_foreign_key,
        } => (|| {
            let (gdb, origin) = gdb_table(&origin_table)?;
            let (_, destination) = gdb_table(&destination_table)?;
            let (_, name) = gdb_table(&out_relationship_class)?;
            let cardinality = match cardinality.to_ascii_uppercase().as_str() {
                "ONE_TO_ONE" => unigis_filegdb::Cardinality::OneToOne,
                "ONE_TO_MANY" => unigis_filegdb::Cardinality::OneToMany,
                "MANY_TO_MANY" => unigis_filegdb::Cardinality::ManyToMany,
                other => {
                    return Err(Error::Unsupported(format!(
                        "ERROR 000800: The value is not a member of ONE_TO_ONE | ONE_TO_MANY | MANY_TO_MANY: {other}"
                    )));
                }
            };
            let notification = match message_direction.to_ascii_uppercase().as_str() {
                "NONE" => "None",
                "FORWARD" => "Forward",
                "BACKWARD" => "Backward",
                "BOTH" => "Both",
                other => {
                    return Err(Error::Unsupported(format!(
                        "ERROR 000800: The value is not a member of NONE | FORWARD | BACKWARD | BOTH: {other}"
                    )));
                }
            };
            let rc = unigis_filegdb::RelationshipClass {
                name,
                cardinality,
                is_composite: relationship_type.eq_ignore_ascii_case("COMPOSITE"),
                is_attributed: attributed.eq_ignore_ascii_case("ATTRIBUTED"),
                notification: notification.into(),
                origin,
                destination,
                origin_primary_key,
                origin_foreign_key,
                destination_primary_key,
                destination_foreign_key,
                forward_label,
                backward_label,
            };
            let mut w = unigis_filegdb::GeodatabaseWriter::open_existing(&gdb)?;
            w.add_relationship_class(&rc)?;
            w.finish()
        })(),
        Tool::Delete { in_data } => delete_data(&in_data),
        Tool::Rename { in_data, out_data } => rename_data(&in_data, &out_data),
        Tool::DeleteDomain {
            in_workspace,
            domain_name,
        } => unigis_filegdb::delete_domain(&in_workspace, &domain_name),
        Tool::AlterDomain {
            in_workspace,
            domain_name,
            new_domain_name,
            new_domain_description,
            split_policy,
            merge_policy,
            new_domain_owner,
        } => (|| {
            unigis_filegdb::alter_domain(
                &in_workspace,
                &domain_name,
                new_domain_name.as_deref(),
                new_domain_description.as_deref(),
                domain_split_policy(split_policy.as_deref())?,
                domain_merge_policy(merge_policy.as_deref())?,
                new_domain_owner.as_deref(),
            )
        })(),
        Tool::CreateDomain {
            in_workspace,
            domain_name,
            domain_description,
            field_type,
            domain_type,
            split_policy,
            merge_policy,
        } => (|| {
            let ty = alter_field_type(Some(&field_type))?.ok_or_else(|| {
                Error::Unsupported("ERROR 000735: Field Type: Value is required".into())
            })?;
            let range = match domain_type.to_ascii_uppercase().as_str() {
                "CODED" => false,
                "RANGE" => true,
                other => return Err(Error::Unsupported(format!("domain type {other}"))),
            };
            unigis_filegdb::create_domain(
                &in_workspace,
                &domain_name,
                &domain_description,
                ty,
                range,
                domain_split_policy(Some(&split_policy))?
                    .unwrap_or(unigis_filegdb::SplitPolicy::DefaultValue),
                domain_merge_policy(Some(&merge_policy))?
                    .unwrap_or(unigis_filegdb::MergePolicy::DefaultValue),
            )
        })(),
        Tool::AddCodedValueToDomain {
            in_workspace,
            domain_name,
            code,
            code_description,
        } => unigis_filegdb::add_coded_value(&in_workspace, &domain_name, &code, &code_description),
        Tool::DeleteCodedValueFromDomain {
            in_workspace,
            domain_name,
            code,
        } => unigis_filegdb::delete_coded_values(&in_workspace, &domain_name, &code),
        Tool::AlterCodedValueDescription {
            in_workspace,
            domain_name,
            code,
            new_description,
        } => unigis_filegdb::alter_coded_value_description(
            &in_workspace,
            &domain_name,
            &code,
            &new_description,
        ),
        Tool::SetValueForRangeDomain {
            in_workspace,
            domain_name,
            min_value,
            max_value,
        } => {
            unigis_filegdb::set_value_for_range(&in_workspace, &domain_name, &min_value, &max_value)
        }
        Tool::SortCodedValueDomain {
            in_workspace,
            domain_name,
            sort_by,
            sort_order,
        } => unigis_filegdb::sort_coded_values(
            &in_workspace,
            &domain_name,
            !sort_by.eq_ignore_ascii_case("DESCRIPTION"),
            !sort_order.eq_ignore_ascii_case("DESCENDING"),
        ),
        Tool::DomainToTable {
            in_workspace,
            domain_name,
            out_table,
            code_field,
            description_field,
        } => (|| {
            let (field_type, rows) = unigis_filegdb::domain_rows(&in_workspace, &domain_name)?;
            let mut description = Field::new(&description_field, FieldType::String);
            description.length = 255;
            write_table(
                &unigis_tools::TableOutput {
                    fields: vec![Field::new(&code_field, field_type), description],
                    rows: rows
                        .into_iter()
                        .map(|(code, d)| vec![code, Value::String(d)])
                        .collect(),
                },
                &out_table,
            )
        })(),
        Tool::TableToDomain {
            in_table,
            code_field,
            description_field,
            in_workspace,
            domain_name,
            domain_description,
            update_option,
        } => (|| {
            let (table, _) = open_feature_class(&in_table)?;
            let find = |name: &str| {
                table
                    .fields()
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| {
                        Error::Unsupported(format!(
                            "ERROR 000728: Field {name} does not exist within table"
                        ))
                    })
            };
            let (c, d) = (find(&code_field)?, find(&description_field)?);
            let field_type = table.fields()[c].field_type;
            let mut values = Vec::new();
            for f in table.features() {
                let f = f?;
                let desc = match &f.attributes[d] {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => format!("{other:?}"),
                };
                values.push((f.attributes[c].clone(), desc));
            }
            unigis_filegdb::table_to_domain(
                &in_workspace,
                &domain_name,
                &domain_description,
                field_type,
                &values,
                update_option.eq_ignore_ascii_case("REPLACE"),
            )
        })(),
        Tool::DeleteIdentical {
            in_dataset,
            fields,
            xy_tolerance,
            output,
        } => (|| {
            let names: Vec<String> = fields
                .split(';')
                .map(|f| f.trim().to_owned())
                .filter(|f| !f.is_empty())
                .collect();
            let (fc, _) = open_feature_class(&in_dataset)?;
            let out = unigis_tools::delete_identical(fc.as_ref(), &names, xy_tolerance)?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_dataset).as_deref()),
                None => update_in_place(out, &in_dataset),
            }
        })(),
        Tool::Sort {
            in_dataset,
            out_dataset,
            sort_field,
            spatial_sort_method,
        } => (|| {
            let keys = sort_field
                .split(';')
                .filter(|k| !k.trim().is_empty())
                .map(|k| {
                    let mut parts = k.split_whitespace();
                    let name = parts.next().unwrap_or_default().to_owned();
                    match parts.next().map(str::to_ascii_uppercase).as_deref() {
                        None | Some("ASCENDING") => Ok((name, true)),
                        Some("DESCENDING") => Ok((name, false)),
                        Some(other) => Err(Error::Unsupported(format!("sort order {other}"))),
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            let spatial =
                unigis_tools::SpatialSort::parse(&spatial_sort_method).ok_or_else(|| {
                    Error::Unsupported(format!("spatial_sort_method {spatial_sort_method}"))
                })?;
            let (fc, _) = open_feature_class(&in_dataset)?;
            let out = unigis_tools::sort(fc.as_ref(), &keys, spatial)?;
            write_output(out, &out_dataset, source_gdb(&in_dataset).as_deref())
        })(),
        Tool::Select {
            in_features,
            out_feature_class,
            where_clause,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::select(fc.as_ref(), where_clause.as_deref())?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::Statistics {
            in_table,
            out_table,
            statistics_fields,
            case_fields,
        } => (|| {
            let mut stats = Vec::new();
            for item in statistics_fields
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let mut w = item.split_whitespace();
                let (Some(field), Some(stat)) = (w.next(), w.next()) else {
                    return Err(Error::Unsupported(format!(
                        "statistic `{item}`: expected \"FIELD STATISTIC\""
                    )));
                };
                let stat = unigis_tools::Statistic::parse(stat)
                    .ok_or_else(|| Error::Unsupported(format!("statistic type {stat}")))?;
                stats.push((field.to_owned(), stat));
            }
            let (fc, _) = open_feature_class(&in_table)?;
            let out = unigis_tools::statistics(fc.as_ref(), &stats, &case_fields)?;
            write_table(&out, &out_table)
        })(),
        Tool::Frequency {
            in_table,
            out_table,
            frequency_fields,
            summary_fields,
        } => (|| {
            let split = |s: &str| -> Vec<String> {
                s.split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            };
            let (fc, _) = open_feature_class(&in_table)?;
            let out = unigis_tools::frequency(
                fc.as_ref(),
                &split(&frequency_fields),
                &split(summary_fields.as_deref().unwrap_or("")),
            )?;
            write_table(&out, &out_table)
        })(),
        Tool::AddGeometryAttributes {
            in_features,
            geometry_properties,
            length_unit,
            area_unit,
            output,
        } => (|| {
            let props = geometry_properties
                .split(';')
                .filter(|p| !p.trim().is_empty())
                .map(|p| {
                    unigis_tools::GeometryProperty::parse(p)
                        .ok_or_else(|| Error::Unsupported(format!("geometry property {p}")))
                })
                .collect::<Result<Vec<_>>>()?;
            let length = match length_unit
                .as_deref()
                .map(str::to_ascii_uppercase)
                .as_deref()
            {
                None | Some("") | Some("METERS") => 1.0,
                Some("KILOMETERS") => 1000.0,
                Some("FEET_INT") => 0.3048,
                Some("FEET_US") => 1200.0 / 3937.0,
                Some("MILES_INT") => 1609.344,
                Some("YARDS") => 0.9144,
                Some("NAUTICAL_MILES") => 1852.0,
                Some(other) => return Err(Error::Unsupported(format!("length unit {other}"))),
            };
            let area = match area_unit.as_deref().map(str::to_ascii_uppercase).as_deref() {
                None | Some("") | Some("SQUARE_METERS") => 1.0,
                Some("SQUARE_KILOMETERS") => 1e6,
                Some("HECTARES") => 1e4,
                Some("ACRES") => 4046.8564224,
                Some("SQUARE_FEET") => 0.09290304,
                Some("SQUARE_MILES") => 2589988.110336,
                Some(other) => return Err(Error::Unsupported(format!("area unit {other}"))),
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::add_geometry_attributes(fc.as_ref(), &props, length, area)?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_features).as_deref()),
                None => update_in_place(out, &in_features),
            }
        })(),
        Tool::PointsToLine {
            in_features,
            out_feature_class,
            line_field,
            sort_field,
            close_line,
        } => (|| {
            let close = match close_line.to_ascii_uppercase().as_str() {
                "NO_CLOSE" => false,
                "CLOSE" => true,
                other => return Err(Error::Unsupported(format!("close_line {other}"))),
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::points_to_line(
                fc.as_ref(),
                line_field.as_deref(),
                sort_field.as_deref(),
                close,
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::GenerateNearTable {
            in_features,
            near_features,
            out_table,
            search_radius,
            location,
            angle,
            closest,
            closest_count,
            method,
        } => (|| {
            let method = near_method(&method)?;
            let flag = |v: &str, yes: &str, no: &str| match v.to_ascii_uppercase() {
                x if x == yes => Ok(true),
                x if x == no => Ok(false),
                x => Err(Error::Unsupported(format!(
                    "expected {yes} or {no}, got {x}"
                ))),
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let (near, _) = open_feature_class(&near_features)?;
            let radius = search_radius
                .as_deref()
                .map(|r| near_radius(r, method, fc.spatial_reference_wkt()))
                .transpose()?;
            let out = unigis_tools::generate_near_table_with(
                fc.as_ref(),
                near.as_ref(),
                radius,
                flag(&location, "LOCATION", "NO_LOCATION")?,
                flag(&angle, "ANGLE", "NO_ANGLE")?,
                flag(&closest, "ALL", "CLOSEST")?,
                closest_count,
                method,
            )?;
            write_table(&out, &out_table)
        })(),
        Tool::FindIdentical {
            in_dataset,
            out_table,
            fields,
            output_record_option,
        } => (|| {
            let only = match output_record_option.to_ascii_uppercase().as_str() {
                "ALL" => false,
                "ONLY_DUPLICATES" => true,
                other => return Err(Error::Unsupported(format!("output_record_option {other}"))),
            };
            let names: Vec<String> = fields
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            let (fc, _) = open_feature_class(&in_dataset)?;
            let out = unigis_tools::find_identical(fc.as_ref(), &names, only)?;
            write_table(&out, &out_table)
        })(),
        Tool::MultipartToSinglepart {
            in_features,
            out_feature_class,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::multipart_to_singlepart(fc.as_ref())?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::Tools => {
            use std::io::{self, Write};
            let result = (|| -> io::Result<()> {
                let stdout = io::stdout();
                let mut stdout = stdout.lock();
                for t in unigis_raster::registry::TOOLS {
                    writeln!(stdout, "{}  {}  [Spatial Analyst]", t.id, t.name)?;
                    for p in t.params {
                        writeln!(
                            stdout,
                            "    {}{}  {}{}",
                            p.name,
                            if p.required { " *" } else { "" },
                            p.label,
                            if p.default.is_empty() {
                                String::new()
                            } else {
                                format!(" (default {})", p.default)
                            }
                        )?;
                    }
                    writeln!(stdout, "    output *  Output path")?;
                }
                for t in unigis_tools::registry::TOOLS {
                    writeln!(stdout, "{}  {}  [{}]", t.id, t.name, t.toolbox)?;
                    for p in t.params {
                        writeln!(
                            stdout,
                            "    {}{}  {}{}",
                            p.name,
                            if p.required { " *" } else { "" },
                            p.label,
                            if p.default.is_empty() {
                                String::new()
                            } else {
                                format!(" (default {})", p.default)
                            }
                        )?;
                    }
                    writeln!(stdout, "    output *  Output path")?;
                }
                Ok(())
            })();
            match result {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                Err(error) => Err(Error::io("<stdout>", error)),
            }
        }
        Tool::Run { tool, args } => (|| {
            use unigis_tools::registry;
            if let Some(spec) = unigis_raster::registry::find(&tool) {
                return run_raster_tool(spec, &args);
            }
            let spec = registry::find(&tool)
                .ok_or_else(|| Error::Unsupported(format!("no tool '{tool}' (see `ug tools`)")))?;
            let mut map = std::collections::HashMap::new();
            for a in &args {
                let (k, v) = a
                    .split_once('=')
                    .ok_or_else(|| Error::Unsupported(format!("'{a}': expected name=value")))?;
                if k != "output" && !spec.params.iter().any(|p| p.name == k) {
                    return Err(Error::Unsupported(format!(
                        "{}: no parameter '{k}'",
                        spec.id
                    )));
                }
                map.insert(k.to_string(), v.to_string());
            }
            let reports = matches!(spec.run, unigis_tools::registry::Runner::Report(_));
            let output = match map.get("output") {
                Some(o) => o.clone(),
                None if reports => String::new(),
                None => return Err(Error::Unsupported("output=<path> is required".into())),
            };
            if spec.id == "define_projection"
                && map.get("input").is_some_and(|p| {
                    Path::new(p).extension().is_some_and(|e| {
                        e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff")
                    })
                })
            {
                let input = Path::new(&map["input"]);
                let coordinate_system = map.get("coordinate_system").ok_or_else(|| {
                    Error::Unsupported("ERROR 000735: Coordinate System: Value is required".into())
                })?;
                return define_raster_projection(input, Path::new(&output), coordinate_system);
            }
            struct Args(std::collections::HashMap<String, String>);
            impl registry::Context for Args {
                fn import_feature_classes(
                    &self,
                    inputs: &[String],
                    target: &str,
                ) -> Result<Vec<String>> {
                    feature_class_to_geodatabase::run(inputs, Path::new(target))
                }
                fn import_tables(&self, inputs: &[String], target: &str) -> Result<Vec<String>> {
                    feature_class_to_geodatabase::run_tables(inputs, Path::new(target))
                }
                fn export_shapefiles(
                    &self,
                    inputs: &[String],
                    folder: &str,
                ) -> Result<Vec<String>> {
                    export_shapefiles(inputs, Path::new(folder))
                }
                fn export_dbase(&self, inputs: &[String], folder: &str) -> Result<Vec<String>> {
                    export_dbase(inputs, Path::new(folder))
                }
                fn export_excel(
                    &self,
                    inputs: &[String],
                    output: &str,
                    use_aliases: bool,
                    use_descriptions: bool,
                ) -> Result<Vec<String>> {
                    export_excel(inputs, Path::new(output), use_aliases, use_descriptions)
                }
                fn get(&self, name: &str) -> Option<String> {
                    self.0
                        .get(name)
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                }
                fn write_workspace(
                    &self,
                    workspace: &str,
                    outputs: Vec<(String, registry::WorkspaceOutput)>,
                ) -> Result<()> {
                    write_workspace_outputs(Path::new(workspace), outputs)
                }
                fn write_datasets(
                    &self,
                    source: &str,
                    outputs: Vec<(String, registry::WorkspaceOutput)>,
                ) -> Result<()> {
                    let source = self.get(source).map(PathBuf::from);
                    dataset_outputs::write(source.as_deref(), outputs)
                }
                fn extract_data(
                    &self,
                    request: &unigis_tools::extract_data::Request,
                ) -> Result<()> {
                    let mut source = None;
                    let mut names = Vec::new();
                    for input in &request.inputs {
                        let Location::Geodatabase {
                            gdb,
                            item: Some(name),
                        } = locate(Path::new(input))
                        else {
                            return Err(Error::Unsupported(
                                "extraction requires FileGDB table or feature class paths".into(),
                            ));
                        };
                        let actual = std::fs::canonicalize(&gdb).map_err(|e| Error::io(&gdb, e))?;
                        if source.as_ref().is_some_and(|s| s != &actual) {
                            return Err(Error::Unsupported(
                                "extraction from multiple source geodatabases is not supported"
                                    .into(),
                            ));
                        }
                        source = Some(actual);
                        names.push(name);
                    }
                    let source = source.ok_or_else(|| {
                        Error::Unsupported(
                            "ERROR 000735: Datasets to Extract: Value is required".into(),
                        )
                    })?;
                    let destination = Path::new(&request.destination);
                    if !destination
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
                    {
                        return Err(Error::Unsupported(
                            "extraction destination must be a FileGDB".into(),
                        ));
                    }
                    if request.create_new && !destination.parent().is_some_and(Path::is_dir) {
                        return Err(Error::Unsupported(
                            "geodatabase output folder must already exist".into(),
                        ));
                    }
                    use unigis_filegdb::{Expand, ExtractOptions};
                    let expand = match request.expand.as_str() {
                        "USE_DEFAULTS" => Expand::Defaults,
                        "ADD_WITH_SCHEMA_ONLY" => Expand::SchemaOnly,
                        "ALL_ROWS" => Expand::AllRows,
                        "DO_NOT_ADD" => Expand::None,
                        _ => {
                            return Err(Error::Unsupported(
                                "invalid extraction expansion option".into(),
                            ));
                        }
                    };
                    unigis_filegdb::extract_data(
                        &source,
                        &names,
                        destination,
                        request.create_new,
                        ExtractOptions {
                            schema_only: request.schema_only,
                            expand,
                            related: request.related,
                            all_table_rows: request.all_table_rows,
                        },
                    )
                }
                fn add_schema_fields(
                    &self,
                    input: &str,
                    fields: &[Field],
                    domains: &[registry::SchemaDomain],
                ) -> Result<()> {
                    let path = self
                        .get(input)
                        .ok_or_else(|| Error::Unsupported(format!("{input} is required")))?;
                    let Location::Geodatabase {
                        gdb,
                        item: Some(item),
                    } = locate(Path::new(&path))
                    else {
                        return Err(Error::Unsupported("ERROR 000656: A Shapefile is not supported by this tool; use a FileGDB feature class".into()));
                    };
                    let domains: Vec<_> = domains
                        .iter()
                        .map(|d| unigis_filegdb::Domain {
                            name: d.name.into(),
                            description: d.description.into(),
                            field_type: d.field_type,
                            split_policy: unigis_filegdb::SplitPolicy::DefaultValue,
                            merge_policy: unigis_filegdb::MergePolicy::DefaultValue,
                            kind: match &d.kind {
                                registry::SchemaDomainKind::Coded(values) => {
                                    unigis_filegdb::DomainKind::Coded(values.clone())
                                }
                                registry::SchemaDomainKind::Range(lo, hi) => {
                                    unigis_filegdb::DomainKind::Range {
                                        min: lo.clone(),
                                        max: hi.clone(),
                                    }
                                }
                            },
                        })
                        .collect();
                    unigis_filegdb::add_schema_fields(&gdb, &item, fields, &domains)
                }
                fn delete_items(
                    &self,
                    items: &[unigis_tools::delete_multiple::DeleteItem],
                ) -> Result<()> {
                    delete_multiple::run(items)
                }
                fn migrate_date_fields(&self, input: &str, fields: &[String]) -> Result<()> {
                    let path = self
                        .get(input)
                        .ok_or_else(|| Error::Unsupported(format!("{input} is required")))?;
                    match locate(Path::new(&path)) {
                        Location::Geodatabase{gdb,item:Some(item)}=>unigis_filegdb::migrate_date_fields_to_high_precision(gdb,&item,fields),
                        _=>Err(Error::Unsupported("ERROR 000214: Cannot edit class schema; use a FileGDB table or feature class".into())),
                    }
                }
                fn enable_editor_tracking(
                    &self,
                    input: &str,
                    fields: [String; 4],
                    add_fields: bool,
                    utc: bool,
                ) -> Result<()> {
                    let path = self
                        .get(input)
                        .ok_or_else(|| Error::Unsupported(format!("{input} is required")))?;
                    match locate(Path::new(&path)) {
                        Location::Geodatabase {
                            gdb,
                            item: Some(item),
                        } => unigis_filegdb::enable_editor_tracking(
                            gdb,
                            &item,
                            fields.each_ref().map(String::as_str),
                            add_fields,
                            utc,
                        ),
                        _ => Err(Error::Unsupported(
                            "ERROR 001216: Editor Tracking not supported for dataset".into(),
                        )),
                    }
                }
                fn disable_editor_tracking(&self, input: &str, disable: [bool; 4]) -> Result<()> {
                    let path = self
                        .get(input)
                        .ok_or_else(|| Error::Unsupported(format!("{input} is required")))?;
                    match locate(Path::new(&path)) {
                        Location::Geodatabase { gdb, item: Some(item) } => unigis_filegdb::disable_editor_tracking(&gdb, &item, disable),
                        _ => Err(Error::Unsupported("ERROR 001216: Editor Tracking not supported for dataset; use a FileGDB feature class, table or feature dataset".into())),
                    }
                }
                fn add_global_ids(&self, input: &str) -> Result<()> {
                    let paths = self.get(input).unwrap_or_default();
                    let mut workspace = None;
                    let mut items = Vec::new();
                    for path in paths.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                        match locate(Path::new(path.trim_matches('\''))) {
                            Location::Geodatabase {
                                gdb,
                                item: Some(item),
                            } => {
                                let canonical =
                                    std::fs::canonicalize(&gdb).map_err(|e| Error::io(&gdb, e))?;
                                if workspace.as_ref().is_some_and(|old| old != &canonical) {
                                    return Err(Error::Unsupported(
                                        "GlobalID input datasets must share one FileGDB".into(),
                                    ));
                                }
                                workspace = Some(canonical);
                                items.push(item);
                            }
                            _ => {
                                return Err(Error::Unsupported(
                                    "ERROR 000242: The input dataset, file or layer is invalid"
                                        .into(),
                                ));
                            }
                        }
                    }
                    let workspace = workspace.ok_or_else(|| {
                        Error::Unsupported(
                            "ERROR 000242: The input dataset, file or layer is invalid".into(),
                        )
                    })?;
                    unigis_filegdb::add_global_ids(workspace, &items)
                }
                fn enable_cogo(&self, input: &str) -> Result<()> {
                    let path = self
                        .get(input)
                        .ok_or_else(|| Error::Unsupported(format!("{input} is required")))?;
                    match locate(Path::new(&path)) {
                        Location::Geodatabase {
                            gdb,
                            item: Some(item),
                        } => unigis_filegdb::enable_cogo(&gdb, &item),
                        _ => Err(Error::Unsupported(
                            "ERROR 000156: Input must be a feature class".into(),
                        )),
                    }
                }
                fn disable_cogo(&self, input: &str) -> Result<()> {
                    let path = self
                        .get(input)
                        .ok_or_else(|| Error::Unsupported(format!("{input} is required")))?;
                    match locate(Path::new(&path)) {
                        Location::Geodatabase {
                            gdb,
                            item: Some(item),
                        } => unigis_filegdb::disable_cogo(&gdb, &item),
                        Location::Shapefile(p)
                            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("shp")) =>
                        {
                            Ok(())
                        }
                        _ => Err(Error::Unsupported(
                            "COGO edits require a FileGDB feature class or shapefile".into(),
                        )),
                    }
                }
                fn features(&self, name: &str) -> Result<Box<dyn FeatureClass + '_>> {
                    let path = self
                        .get(name)
                        .ok_or_else(|| Error::Unsupported(format!("{name} is required")))?;
                    open_feature_class(Path::new(&path)).map(|(fc, _)| fc)
                }
            }
            let ctx = Args(map);
            match registry::run(spec, &ctx)? {
                registry::Outcome::Features(out) => {
                    let source = registry::Context::get(&ctx, "input")
                        .and_then(|i| source_gdb(Path::new(&i)));
                    let own_wkt =
                        !matches!(spec.id, "copy" | "define_projection" | "gpx_to_features");
                    write_output_as(out, Path::new(&output), source.as_deref(), own_wkt)
                }
                registry::Outcome::Table(t) => write_table(&t, Path::new(&output)),
                registry::Outcome::FeaturesAndTables(out, tables) => {
                    let source = registry::Context::get(&ctx, "input")
                        .and_then(|i| source_gdb(Path::new(&i)));
                    write_output(out, Path::new(&output), source.as_deref())?;
                    for (param, t) in tables {
                        if let Some(p) = registry::Context::get(&ctx, param) {
                            write_table(&t, Path::new(&p))?;
                        }
                    }
                    Ok(())
                }
                registry::Outcome::Report(r) => {
                    for (name, v) in &r.rows {
                        println!("{name}: {v}");
                    }
                    for m in &r.messages {
                        println!("{m}");
                    }
                    Ok(())
                }
            }
        })(),
        Tool::MeanCenter {
            in_features,
            out_feature_class,
            weight_field,
            case_field,
            dimension_field,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::spatstat::mean_center(
                fc.as_ref(),
                weight_field.as_deref(),
                case_field.as_deref(),
                dimension_field.as_deref(),
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::MedianCenter {
            in_features,
            out_feature_class,
            weight_field,
            case_field,
            dimension_field,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::spatstat::median_center(
                fc.as_ref(),
                weight_field.as_deref(),
                case_field.as_deref(),
                dimension_field.as_deref(),
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::CentralFeature {
            in_features,
            out_feature_class,
            distance_method,
            weight_field,
            self_potential_weight_field,
            case_field,
        } => (|| {
            let manhattan = match distance_method.to_ascii_uppercase().as_str() {
                "EUCLIDEAN_DISTANCE" => false,
                "MANHATTAN_DISTANCE" => true,
                other => return Err(Error::Unsupported(format!("distance method {other}"))),
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::spatstat::central_feature(
                fc.as_ref(),
                manhattan,
                weight_field.as_deref(),
                self_potential_weight_field.as_deref(),
                case_field.as_deref(),
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::StandardDistance {
            in_features,
            out_feature_class,
            circle_size,
            weight_field,
            case_field,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::spatstat::standard_distance(
                fc.as_ref(),
                &circle_size,
                weight_field.as_deref(),
                case_field.as_deref(),
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::DirectionalDistribution {
            in_features,
            out_feature_class,
            ellipse_size,
            weight_field,
            case_field,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::spatstat::directional_distribution(
                fc.as_ref(),
                &ellipse_size,
                weight_field.as_deref(),
                case_field.as_deref(),
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::DirectionalMean {
            in_features,
            out_feature_class,
            orientation_only,
            case_field,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::spatstat::linear_directional_mean(
                fc.as_ref(),
                orientation_only,
                case_field.as_deref(),
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::FeatureToPoint {
            in_features,
            out_feature_class,
            point_location,
        } => (|| {
            let inside = match point_location.to_ascii_uppercase().as_str() {
                "CENTROID" => false,
                "INSIDE" => true,
                other => return Err(Error::Unsupported(format!("point_location {other}"))),
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::feature_to_point(fc.as_ref(), inside)?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::FeatureVerticesToPoints {
            in_features,
            out_feature_class,
            point_location,
        } => (|| {
            let location = unigis_tools::VertexLocation::parse(&point_location)
                .ok_or_else(|| Error::Unsupported(format!("point_location {point_location}")))?;
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::feature_vertices_to_points(fc.as_ref(), location)?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::MinimumBoundingGeometry {
            in_features,
            out_feature_class,
            geometry_type,
            group_option,
            group_fields,
            mbg_fields_option,
        } => (|| {
            use unigis_geometry::BoundingKind as K;
            let kind = match geometry_type.to_ascii_uppercase().as_str() {
                "RECTANGLE_BY_AREA" => K::RectangleByArea,
                "RECTANGLE_BY_WIDTH" => K::RectangleByWidth,
                "CONVEX_HULL" => K::ConvexHull,
                "CIRCLE" => K::Circle,
                "ENVELOPE" => K::Envelope,
                other => return Err(Error::Unsupported(format!("geometry_type {other}"))),
            };
            let group = match group_option.to_ascii_uppercase().as_str() {
                "NONE" => unigis_tools::BoundingGroup::None,
                "ALL" => unigis_tools::BoundingGroup::All,
                "LIST" if !group_fields.is_empty() => {
                    unigis_tools::BoundingGroup::List(group_fields)
                }
                other => return Err(Error::Unsupported(format!("group_option {other}"))),
            };
            let mbg = match mbg_fields_option.to_ascii_uppercase().as_str() {
                "NO_MBG_FIELDS" => false,
                "MBG_FIELDS" => true,
                other => return Err(Error::Unsupported(format!("mbg_fields_option {other}"))),
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::minimum_bounding_geometry(fc.as_ref(), kind, &group, mbg)?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::AddXy {
            in_features,
            output,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::add_xy(fc.as_ref())?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_features).as_deref()),
                None => update_in_place(out, &in_features),
            }
        })(),
        Tool::FeaturesToJson {
            in_features,
            out_json_file,
            format_json,
            include_z_values,
            include_m_values,
            geojson,
        } => (|| {
            let flag = |v: &str, yes: &str, no: &str| match v.to_ascii_uppercase() {
                x if x == yes => Ok(true),
                x if x == no => Ok(false),
                x => Err(Error::Unsupported(format!(
                    "expected {yes} or {no}, got {x}"
                ))),
            };
            let formatted = flag(&format_json, "FORMATTED", "NOT_FORMATTED")?;
            let geojson = flag(&geojson, "GEOJSON", "NO_GEOJSON")?;
            let (fc, _) = open_feature_class(&in_features)?;
            let kind = fc
                .geometry_kind()
                .ok_or_else(|| Error::Unsupported("input has no geometry".into()))?;
            let features: Vec<Feature> = fc.features().collect::<Result<_>>()?;
            let wkid = fc
                .spatial_reference_wkt()
                .and_then(|w| unigis_proj::Crs::from_wkt(w).ok())
                .and_then(|c| c.wkid());
            let d = unigis_json::Dataset {
                kind,
                oid_name: fc.oid_field_name(),
                fields: fc.fields(),
                features: &features,
                wkid,
                z: flag(&include_z_values, "Z_VALUES", "NO_Z_VALUES")? && fc.has_z(),
                m: flag(&include_m_values, "M_VALUES", "NO_M_VALUES")? && fc.has_m(),
            };
            let text = if geojson {
                unigis_json::geojson(&d, formatted)
            } else {
                unigis_json::esri_json(&d, formatted)
            };
            std::fs::write(&out_json_file, text).map_err(|e| Error::io(&out_json_file, e))?;
            println!(
                "Wrote {} ({} features)",
                out_json_file.display(),
                features.len()
            );
            Ok(())
        })(),
        Tool::JsonToFeatures {
            in_json_file,
            out_features,
            geometry_type,
        } => (|| {
            let kind = match geometry_type
                .as_deref()
                .map(str::to_ascii_uppercase)
                .as_deref()
            {
                None => None,
                Some("POINT") => Some(unigis_core::GeometryKind::Point),
                Some("MULTIPOINT") => Some(unigis_core::GeometryKind::Multipoint),
                Some("POLYLINE") => Some(unigis_core::GeometryKind::Polyline),
                Some("POLYGON") => Some(unigis_core::GeometryKind::Polygon),
                Some(other) => return Err(Error::Unsupported(format!("geometry_type {other}"))),
            };
            let text =
                std::fs::read_to_string(&in_json_file).map_err(|e| Error::io(&in_json_file, e))?;
            let imported = unigis_json::read(&text, kind)?;
            let wkt = match (&imported.wkt, imported.wkid) {
                (Some(w), _) => Some(w.clone()),
                (None, Some(code)) => unigis_proj::Crs::parse(&code.to_string())
                    .ok()
                    .map(|c| c.esri_wkt().to_owned()),
                _ => None,
            };
            let out = ToolOutput {
                kind: imported.kind,
                has_z: imported.has_z,
                has_m: imported.has_m,
                fields: imported.fields,
                spatial_reference_wkt: wkt,
                features: imported.features,
                messages: Vec::new(),
            };
            write_output(out, &out_features, None)
        })(),
        Tool::ExportTable {
            in_table,
            out_table,
        } => (|| {
            let (fc, _) = open_feature_class(&in_table)?;
            if let Some((gpkg, item)) = geopackage_item(&out_table) {
                let name = item.ok_or_else(|| {
                    Error::Unsupported("name the output table inside the .gpkg".into())
                })?;
                let name = name.strip_prefix("main.").unwrap_or(&name).to_string();
                let features = fc
                    .features()
                    .map(|f| {
                        f.map(|mut f| {
                            f.geometry = None;
                            f
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                unigis_gpkg::write::write_table(
                    &gpkg,
                    &unigis_gpkg::write::NewTable {
                        name: &name,
                        geometry: None,
                        fields: fc.fields(),
                        spatial_ref: None,
                        xy_domain: None,
                        features: &features,
                    },
                )?;
                println!("Wrote {} rows to {}", features.len(), out_table.display());
                return Ok(());
            }
            if out_table
                .extension()
                .is_none_or(|e| !e.eq_ignore_ascii_case("csv"))
            {
                return Err(Error::Unsupported(
                    "Export Table writes .csv or a GeoPackage table here".into(),
                ));
            }
            let rows = fc
                .features()
                .map(|f| f.map(|f| f.attributes))
                .collect::<Result<Vec<_>>>()?;
            let n = rows.len();
            let text = unigis_csv::write(fc.fields(), rows);
            std::fs::write(&out_table, text).map_err(|e| Error::io(&out_table, e))?;
            println!("Wrote {} ({n} rows)", out_table.display());
            Ok(())
        })(),
        Tool::XyTableToPoint {
            in_table,
            out_feature_class,
            x_field,
            y_field,
            z_field,
            coordinate_system,
        } => (|| {
            let crs = unigis_proj::Crs::parse(&coordinate_system)?;
            let (table, _) = open_feature_class(&in_table)?;
            let out = unigis_tools::xy_table_to_point(
                table.as_ref(),
                &x_field,
                &y_field,
                z_field.as_deref(),
                Some(crs.esri_wkt().to_owned()),
            )?;
            write_output(out, &out_feature_class, None)
        })(),
        Tool::Project {
            in_dataset,
            out_dataset,
            out_coor_system,
            transform_method,
        } => (|| {
            let target = unigis_proj::Crs::parse(&out_coor_system)?;
            let (fc, _) = open_feature_class(&in_dataset)?;
            let out = unigis_tools::project(fc.as_ref(), &target, transform_method.as_deref())?;
            write_output(out, &out_dataset, source_gdb(&in_dataset).as_deref())
        })(),
        Tool::AddSpatialIndex { in_features } => (|| {
            let Location::Shapefile(shp) = locate(&in_features) else {
                return Err(Error::Unsupported(
                    "geodatabase feature classes always have a spatial index".into(),
                ));
            };
            let bytes = std::fs::read(&shp).map_err(|e| Error::io(&shp, e))?;
            let (records, extent, entries) = unigis_shapefile::sbn::entries_from_shp(&bytes)?;
            unigis_shapefile::sbn::write(&shp, records, extent, &entries)?;
            println!("Indexed {} ({} shapes)", shp.display(), entries.len());
            Ok(())
        })(),
        Tool::Slope {
            in_raster,
            out_raster,
            output_measurement,
            z_factor,
        } => (|| {
            let unit = match output_measurement.to_ascii_uppercase().as_str() {
                "DEGREE" => unigis_raster::surface::SlopeUnit::Degree,
                "PERCENT_RISE" => unigis_raster::surface::SlopeUnit::PercentRise,
                other => return Err(Error::Unsupported(format!("output measurement {other}"))),
            };
            let dem = unigis_raster::read_geotiff(&in_raster)?;
            let out = unigis_raster::surface::slope(&dem, unit, z_factor);
            write_raster(&out_raster, &out)
        })(),
        Tool::FocalStatistics {
            in_raster,
            out_raster,
            neighborhood,
            statistics_type,
            ignore_nodata,
        } => (|| {
            use unigis_raster::focal::{FocalStatistic, Neighborhood};
            let nbr = Neighborhood::parse(&neighborhood)
                .ok_or_else(|| Error::Unsupported(format!("neighborhood {neighborhood}")))?;
            let stat = FocalStatistic::parse(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistic {statistics_type}")))?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            if nbr.cell_count() > 4_000_000 {
                return Err(Error::Unsupported(format!(
                    "the neighborhood spans {} cells",
                    nbr.cell_count()
                )));
            }
            let input = unigis_raster::read_geotiff(&in_raster)?;
            let out = unigis_raster::focal::focal_statistics(&input, nbr, stat, ignore);
            write_raster(&out_raster, &out)
        })(),
        Tool::Aggregate {
            in_raster,
            out_raster,
            cell_factor,
            aggregation_type,
            extent_handling,
            ignore_nodata,
        } => (|| {
            use unigis_raster::blockops::{AggregateStat, aggregate};
            let stat = AggregateStat::parse(&aggregation_type).ok_or_else(|| {
                Error::Unsupported(format!("aggregation type {aggregation_type}"))
            })?;
            let expand = match extent_handling.to_ascii_uppercase().as_str() {
                "EXPAND" => true,
                "TRUNCATE" => false,
                other => return Err(Error::Unsupported(format!("extent handling {other}"))),
            };
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            let input = unigis_raster::read_geotiff(&in_raster)?;
            if input.bands.len() != 1 {
                return Err(Error::Unsupported(
                    "Aggregate currently requires a single-band raster".into(),
                ));
            }
            let out =
                aggregate(&input, cell_factor as usize, stat, expand, ignore).ok_or_else(|| {
                    Error::Unsupported(
                        "Aggregate produces no output cells; reduce cell_factor or use EXPAND"
                            .into(),
                    )
                })?;
            write_raster(&out_raster, &out)
        })(),
        Tool::BlockStatistics {
            in_raster,
            out_raster,
            neighborhood,
            statistics_type,
            ignore_nodata,
        } => (|| {
            use unigis_raster::blockops::{block_check, block_neighborhood, block_statistics};
            use unigis_raster::focal::FocalStatistic;
            let input = unigis_raster::read_geotiff(&in_raster)?;
            let nbr = block_neighborhood(&neighborhood, input.cell_width(), input.cell_height())
                .ok_or_else(|| {
                    Error::Unsupported(format!(
                        "ERROR 000628: Cannot set input into parameter neighborhood: {neighborhood}"
                    ))
                })?;
            let stat = FocalStatistic::parse(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistic {statistics_type}")))?;
            block_check(stat, input.data_type).map_err(Error::Unsupported)?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            let out = block_statistics(&input, nbr, stat, ignore)
                .ok_or_else(|| Error::Unsupported("Block Statistics neighborhood".into()))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::Fill {
            in_surface_raster,
            out_surface_raster,
            z_limit,
        } => (|| {
            let dem = unigis_raster::read_geotiff(&in_surface_raster)?;
            write_raster_plain(
                &out_surface_raster,
                &unigis_raster::hydro::fill(&dem, z_limit),
            )
        })(),
        Tool::FlowDirection {
            in_surface_raster,
            out_flow_direction_raster,
            force_flow,
        } => (|| {
            let force = match force_flow.to_ascii_uppercase().as_str() {
                "NORMAL" => false,
                "FORCE" => true,
                other => return Err(Error::Unsupported(format!("force_flow {other}"))),
            };
            let dem = unigis_raster::read_geotiff(&in_surface_raster)?;
            write_raster_plain(
                &out_flow_direction_raster,
                &unigis_raster::hydro::flow_direction(&dem, force),
            )
        })(),
        Tool::FlowAccumulation {
            in_flow_direction_raster,
            out_accumulation_raster,
            in_weight_raster,
            data_type,
        } => (|| {
            let integer = match data_type.to_ascii_uppercase().as_str() {
                "FLOAT" => false,
                "INTEGER" => true,
                other => return Err(Error::Unsupported(format!("data type {other}"))),
            };
            let dirs = unigis_raster::read_geotiff(&in_flow_direction_raster)?;
            let weight = in_weight_raster
                .as_deref()
                .map(unigis_raster::read_geotiff)
                .transpose()?;
            write_raster(
                &out_accumulation_raster,
                &unigis_raster::hydro::flow_accumulation(&dirs, weight.as_ref(), integer),
            )
        })(),
        Tool::Reclassify {
            in_raster,
            remap,
            out_raster,
            missing_values,
        } => (|| {
            let entries = unigis_raster::zonal::parse_remap(&remap)
                .ok_or_else(|| Error::Unsupported(format!("remap {remap}")))?;
            let keep = match missing_values.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("missing_values {other}"))),
            };
            let input = unigis_raster::read_geotiff(&in_raster)?;
            write_raster_plain(
                &out_raster,
                &unigis_raster::zonal::reclassify(&input, &entries, keep),
            )
        })(),
        Tool::ReclassByTable {
            in_raster,
            in_remap_table,
            out_raster,
            from_value_field,
            to_value_field,
            output_value_field,
            missing_values,
        } => (|| {
            let (table, _) = open_feature_class(&in_remap_table)?;
            let find = |name: &str| {
                table
                    .fields()
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| {
                        Error::Unsupported(format!(
                            "ERROR 000728: Field {name} does not exist within table"
                        ))
                    })
            };
            let (a, b, c) = (
                find(&from_value_field)?,
                find(&to_value_field)?,
                find(&output_value_field)?,
            );
            if !matches!(
                table.fields()[c].field_type,
                FieldType::SmallInteger | FieldType::Integer
            ) {
                return Err(Error::Unsupported(
                    "ERROR 000864: Output value field: The input is not within the defined domain.\nERROR 003911: The field is not of type SHORT | LONG".into(),
                ));
            }
            let number = |v: &Value| match v {
                Value::SmallInteger(n) => Some(f64::from(*n)),
                Value::Integer(n) => Some(f64::from(*n)),
                Value::Single(n) => Some(f64::from(*n)),
                Value::Double(n) => Some(*n),
                _ => None,
            };
            let mut entries = Vec::new();
            for f in table.features() {
                let f = f?;
                if let (Some(from), Some(to)) = (number(&f.attributes[a]), number(&f.attributes[b]))
                {
                    entries.push(unigis_raster::zonal::Remap {
                        from,
                        to,
                        new: number(&f.attributes[c]),
                    });
                }
            }
            let keep = match missing_values.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("missing_values {other}"))),
            };
            let input = unigis_raster::read_geotiff(&in_raster)?;
            write_raster_plain(
                &out_raster,
                &unigis_raster::zonal::reclassify_table(&input, &entries, keep),
            )
        })(),
        Tool::ZonalStatistics {
            in_zone_data,
            in_value_raster,
            out_raster,
            statistics_type,
            ignore_nodata,
        } => (|| {
            let stat = unigis_raster::zonal::ZonalStatistic::parse(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistic {statistics_type}")))?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            let zones = unigis_raster::read_geotiff(&in_zone_data)?;
            let values = unigis_raster::read_geotiff(&in_value_raster)?;
            let (zones, values) = unigis_raster::resample::zones_on_value_grid(&zones, &values)
                .ok_or_else(|| {
                    Error::Unsupported("zone and value rasters do not overlap".into())
                })?;
            write_raster_plain(
                &out_raster,
                &unigis_raster::zonal::zonal_statistics(&zones, &values, stat, ignore),
            )
        })(),
        Tool::Curvature {
            in_raster,
            out_curvature_raster,
            z_factor,
            out_profile_curve_raster,
            out_plan_curve_raster,
        } => (|| {
            let dem = unigis_raster::read_geotiff(&in_raster)?;
            let (curv, profile, plan) = unigis_raster::surface::curvature(&dem, z_factor);
            write_raster(&out_curvature_raster, &curv)?;
            if let Some(p) = out_profile_curve_raster {
                write_raster(&p, &profile)?;
            }
            if let Some(p) = out_plan_curve_raster {
                write_raster(&p, &plan)?;
            }
            Ok(())
        })(),
        Tool::Plus {
            in1,
            in2,
            out_raster,
        } => math_tool(unigis_raster::local::MathOp::Plus, &in1, &in2, &out_raster),
        Tool::Minus {
            in1,
            in2,
            out_raster,
        } => math_tool(unigis_raster::local::MathOp::Minus, &in1, &in2, &out_raster),
        Tool::Times {
            in1,
            in2,
            out_raster,
        } => math_tool(unigis_raster::local::MathOp::Times, &in1, &in2, &out_raster),
        Tool::Divide {
            in1,
            in2,
            out_raster,
        } => math_tool(
            unigis_raster::local::MathOp::Divide,
            &in1,
            &in2,
            &out_raster,
        ),
        Tool::Int {
            in_raster,
            out_raster,
        } => unigis_raster::read_geotiff(&in_raster)
            .and_then(|r| write_raster(&out_raster, &unigis_raster::local::int(&r))),
        Tool::Float {
            in_raster,
            out_raster,
        } => unigis_raster::read_geotiff(&in_raster)
            .and_then(|r| write_raster(&out_raster, &unigis_raster::local::float(&r))),
        Tool::IsNull {
            in_raster,
            out_raster,
        } => unigis_raster::read_geotiff(&in_raster)
            .and_then(|r| write_raster(&out_raster, &unigis_raster::local::is_null(&r))),
        Tool::Con {
            in_conditional_raster,
            in_true_raster_or_constant,
            out_raster,
            in_false_raster_or_constant,
            where_clause,
        } => (|| {
            let cond = unigis_raster::read_geotiff(&in_conditional_raster)?;
            let t = operand(&in_true_raster_or_constant)?;
            let f = in_false_raster_or_constant
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(operand)
                .transpose()?;
            let clause = value_clause(where_clause.as_deref())?;
            let out = unigis_raster::local::con(&cond, &t, f.as_ref(), clause.as_ref())
                .ok_or_else(|| Error::Unsupported("input rasters do not overlap".into()))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::SetNull {
            in_conditional_raster,
            in_false_raster_or_constant,
            out_raster,
            where_clause,
        } => (|| {
            let cond = unigis_raster::read_geotiff(&in_conditional_raster)?;
            let f = operand(&in_false_raster_or_constant)?;
            let clause = value_clause(where_clause.as_deref())?;
            let out = unigis_raster::local::set_null(&cond, &f, clause.as_ref())
                .ok_or_else(|| Error::Unsupported("input rasters do not overlap".into()))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::Contour {
            in_raster,
            out_polyline_features,
            contour_interval,
            base_contour,
        } => (|| {
            let r = unigis_raster::read_geotiff(&in_raster)?;
            if unigis_raster::contour::level_count(&r, contour_interval, base_contour)
                > unigis_raster::contour::MAX_LEVELS
            {
                return Err(Error::Unsupported(format!(
                    "the contour interval {contour_interval} gives more than {} levels",
                    unigis_raster::contour::MAX_LEVELS
                )));
            }
            let lines = unigis_raster::contour::contour(&r, contour_interval, base_contour);
            write_contours(lines, &r, &out_polyline_features)
        })(),
        Tool::Basin {
            in_flow_direction_raster,
            out_raster,
        } => unigis_raster::read_geotiff(&in_flow_direction_raster)
            .and_then(|d| write_raster_basin(&out_raster, &unigis_raster::hydro::basin(&d))),
        Tool::Watershed {
            in_flow_direction_raster,
            in_pour_point_data,
            out_raster,
        } => (|| {
            let d = unigis_raster::read_geotiff(&in_flow_direction_raster)?;
            let p = unigis_raster::read_geotiff(&in_pour_point_data)?;
            write_raster_plain(&out_raster, &unigis_raster::hydro::watershed(&d, &p))
        })(),
        Tool::StreamLink {
            in_stream_raster,
            in_flow_direction_raster,
            out_raster,
        } => (|| {
            let s = unigis_raster::read_geotiff(&in_stream_raster)?;
            let d = unigis_raster::read_geotiff(&in_flow_direction_raster)?;
            write_raster_plain(&out_raster, &unigis_raster::hydro::stream_link(&s, &d))
        })(),
        Tool::StreamOrder {
            in_stream_raster,
            in_flow_direction_raster,
            out_raster,
            order_method,
        } => (|| {
            use unigis_raster::hydro::StreamOrder;
            let method = match order_method.to_ascii_uppercase().as_str() {
                "STRAHLER" => StreamOrder::Strahler,
                "SHREVE" => StreamOrder::Shreve,
                other => return Err(Error::Unsupported(format!("order method {other}"))),
            };
            let s = unigis_raster::read_geotiff(&in_stream_raster)?;
            let d = unigis_raster::read_geotiff(&in_flow_direction_raster)?;
            write_raster(
                &out_raster,
                &unigis_raster::hydro::stream_order(&s, &d, method),
            )
        })(),
        Tool::FlowLength {
            in_flow_direction_raster,
            out_raster,
            direction_measurement,
        } => (|| {
            let upstream = match direction_measurement.to_ascii_uppercase().as_str() {
                "DOWNSTREAM" => false,
                "UPSTREAM" => true,
                other => return Err(Error::Unsupported(format!("direction {other}"))),
            };
            let d = unigis_raster::read_geotiff(&in_flow_direction_raster)?;
            write_raster(
                &out_raster,
                &unigis_raster::hydro::flow_length(&d, upstream),
            )
        })(),
        Tool::RasterToPoint {
            in_raster,
            out_point_features,
        } => (|| {
            let r = unigis_raster::read_geotiff(&in_raster)?;
            let float = r.data_type.is_float();
            let features = unigis_raster::conversion::raster_to_points(&r)
                .into_iter()
                .enumerate()
                .map(|(i, (p, v))| Feature {
                    oid: i as i64,
                    geometry: Some(unigis_core::Geometry::point(p[0], p[1])),
                    attributes: vec![
                        Value::Integer(i as i32 + 1),
                        if float {
                            Value::Single(v as f32)
                        } else {
                            Value::Integer(v as i32)
                        },
                    ],
                })
                .collect();
            let out = ToolOutput {
                kind: unigis_core::GeometryKind::Point,
                has_z: false,
                has_m: false,
                fields: vec![
                    Field::new("pointid", FieldType::Integer),
                    Field::new(
                        "grid_code",
                        if float {
                            FieldType::Single
                        } else {
                            FieldType::Integer
                        },
                    ),
                ],
                spatial_reference_wkt: raster_wkt(&r)?,
                features,
                messages: Vec::new(),
            };
            write_output(out, &out_point_features, None)
        })(),
        Tool::RasterToPolygon {
            in_raster,
            out_polygon_features,
            simplify,
        } => (|| {
            if !simplify.eq_ignore_ascii_case("NO_SIMPLIFY") {
                return Err(Error::Unsupported(format!("simplify {simplify}")));
            }
            let r = unigis_raster::read_geotiff(&in_raster)?;
            if r.data_type.is_float() {
                return Err(Error::Unsupported(
                    "ERROR 010247: The input raster must be integer".into(),
                ));
            }
            let features = unigis_raster::conversion::raster_to_polygons(&r)
                .into_iter()
                .enumerate()
                .map(|(i, poly)| {
                    let mut g = unigis_core::Geometry::point(0.0, 0.0);
                    g.kind = unigis_core::GeometryKind::Polygon;
                    g.xy.clear();
                    g.parts.clear();
                    for ring in poly.rings {
                        g.parts.push(g.xy.len() as u32);
                        g.xy.extend(ring);
                    }
                    Feature {
                        oid: i as i64,
                        geometry: Some(g),
                        attributes: vec![
                            Value::Integer(i as i32 + 1),
                            Value::Integer(poly.value as i32),
                        ],
                    }
                })
                .collect();
            let out = ToolOutput {
                kind: unigis_core::GeometryKind::Polygon,
                has_z: false,
                has_m: false,
                fields: vec![
                    Field::new("Id", FieldType::Integer),
                    Field::new("gridcode", FieldType::Integer),
                ],
                spatial_reference_wkt: raster_wkt(&r)?,
                features,
                messages: Vec::new(),
            };
            write_output(out, &out_polygon_features, None)
        })(),
        Tool::FeatureToRaster {
            in_features,
            field,
            out_raster,
            cell_size,
        } => (|| {
            use unigis_raster::rasterize::*;
            let (features, kind, epsg, geometry) = rasterize_input(&in_features, &field)?;
            let r = match geometry {
                Some(unigis_core::GeometryKind::Polygon) => {
                    polygons_to_raster(&features, cell_size, PolygonAssignment::CellCenter, kind)
                }
                Some(unigis_core::GeometryKind::Polyline) => {
                    lines_to_raster(&features, cell_size, kind)
                }
                _ => points_to_raster(&features, cell_size, PointAssignment::MostFrequent, kind),
            };
            let mut r = r.ok_or_else(|| Error::Unsupported("no features to rasterize".into()))?;
            r.epsg = epsg;
            write_raster(&out_raster, &r)
        })(),
        Tool::PolylineToRaster {
            in_features,
            value_field,
            out_raster,
            cell_assignment,
            cellsize,
        } => (|| {
            use unigis_raster::rasterize::*;
            let assignment = match cell_assignment.to_ascii_uppercase().as_str() {
                "MAXIMUM_LENGTH" => LineAssignment::MaximumLength,
                "MAXIMUM_COMBINED_LENGTH" => LineAssignment::MaximumCombinedLength,
                other => return Err(Error::Unsupported(format!("cell assignment {other}"))),
            };
            let (features, kind, epsg, _) = rasterize_input(&in_features, &value_field)?;
            let mut r = polyline_to_raster(&features, cellsize, assignment, kind)
                .ok_or_else(|| Error::Unsupported("no features to rasterize".into()))?;
            r.epsg = epsg;
            write_raster(&out_raster, &r)
        })(),
        Tool::PolygonToRaster {
            in_features,
            value_field,
            out_raster,
            cell_assignment,
            cellsize,
        } => (|| {
            use unigis_raster::rasterize::*;
            let assignment = match cell_assignment.to_ascii_uppercase().as_str() {
                "CELL_CENTER" => PolygonAssignment::CellCenter,
                "MAXIMUM_AREA" => PolygonAssignment::MaximumArea,
                "MAXIMUM_COMBINED_AREA" => PolygonAssignment::MaximumCombinedArea,
                other => return Err(Error::Unsupported(format!("cell assignment {other}"))),
            };
            let (features, kind, epsg, _) = rasterize_input(&in_features, &value_field)?;
            let mut r = polygons_to_raster(&features, cellsize, assignment, kind)
                .ok_or_else(|| Error::Unsupported("no features to rasterize".into()))?;
            r.epsg = epsg;
            write_raster(&out_raster, &r)
        })(),
        Tool::Idw {
            in_features,
            z_field,
            out_raster,
            cellsize,
            power,
            search_radius,
            num_points,
            max_distance,
            distance,
            min_points,
            extent,
        } => (|| {
            use unigis_raster::interpolate::{Sample, SearchRadius};
            let (features, _, epsg, _) = rasterize_input(&in_features, &z_field)?;
            let mut samples = Vec::new();
            for (g, z) in &features {
                if matches!(
                    g.kind,
                    unigis_core::GeometryKind::Point | unigis_core::GeometryKind::Multipoint
                ) {
                    samples.extend(g.xy.iter().map(|c| Sample {
                        x: c[0],
                        y: c[1],
                        z: *z,
                    }));
                }
            }
            if !(power.is_finite() && power > 0.0)
                || cellsize.is_some_and(|c| c.is_nan() || c <= 0.0)
            {
                return Err(Error::Unsupported(
                    "power and cell size must be positive".into(),
                ));
            }
            let search = match search_radius.to_ascii_uppercase().as_str() {
                "VARIABLE" => SearchRadius::Variable {
                    points: num_points,
                    max_distance,
                },
                "FIXED" => SearchRadius::Fixed {
                    distance,
                    min_points,
                },
                other => return Err(Error::Unsupported(format!("search radius {other}"))),
            };
            let env = unigis_raster::env::Environment {
                cell_size: cellsize,
                extent: extent.map(|v| [v[0], v[1], v[2], v[3]]),
                ..Default::default()
            };
            let r = unigis_raster::interpolate::idw(&samples, power, search, &env, epsg)
                .ok_or_else(|| {
                    Error::Unsupported(
                        "no output grid (no points with values, or a bad cell size)".into(),
                    )
                })?;
            write_raster(&out_raster, &r)
        })(),
        Tool::MathFn { function, paths } => (|| {
            use unigis_raster::local::Operand;
            use unigis_raster::mathfn::{Binary, Unary, binary, unary};
            let (out, ins) = paths
                .split_last()
                .ok_or_else(|| Error::Unsupported("no operands".into()))?;
            let operand = |t: &String| -> Result<Operand> {
                match Operand::constant(t) {
                    Some(c) => Ok(c),
                    None => Ok(Operand::Raster(unigis_raster::read_geotiff(Path::new(t))?)),
                }
            };
            let result = match (Unary::parse(&function), Binary::parse(&function), ins) {
                (Some(op), _, [a]) => match operand(a)? {
                    Operand::Raster(r) => Some(unary(op, &r)),
                    Operand::Constant(_) => {
                        return Err(Error::Unsupported(format!("{function} needs a raster")));
                    }
                },
                (_, Some(op), [a, b]) => binary(op, &operand(a)?, &operand(b)?),
                (None, None, _) => {
                    return Err(Error::Unsupported(format!("math function {function}")));
                }
                _ => {
                    return Err(Error::Unsupported(format!(
                        "{function}: wrong number of operands"
                    )));
                }
            };
            let r = result.ok_or_else(|| {
                Error::Unsupported(
                    "the rasters do not overlap, or a bitwise operator met a float".into(),
                )
            })?;
            write_raster(Path::new(out), &r)
        })(),
        Tool::CellStatistics {
            paths,
            statistics_type,
            ignore_nodata,
        } => (|| {
            use unigis_raster::overlay::*;
            let (out, ins) = paths
                .split_last()
                .ok_or_else(|| Error::Unsupported("no rasters".into()))?;
            let stat = CellStat::parse(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistics type {statistics_type}")))?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore NoData {other}"))),
            };
            let rs: Vec<_> = ins
                .iter()
                .map(|p| unigis_raster::read_geotiff(p))
                .collect::<Result<_, _>>()?;
            let refs: Vec<&unigis_raster::Raster> = rs.iter().collect();
            let r = cell_statistics(&refs, stat, ignore)
                .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
            write_raster(out, &r)
        })(),
        Tool::WeightedSum { paths } => (|| {
            let (out, ins) = paths
                .split_last()
                .ok_or_else(|| Error::Unsupported("no rasters".into()))?;
            let mut rs = Vec::new();
            let mut weights = Vec::new();
            for e in ins {
                let (p, w) = e
                    .rsplit_once('=')
                    .ok_or_else(|| Error::Unsupported(format!("'{e}': expected raster=weight")))?;
                weights.push(
                    w.parse::<f64>()
                        .map_err(|_| Error::Unsupported(format!("weight '{w}'")))?,
                );
                rs.push(unigis_raster::read_geotiff(std::path::Path::new(p))?);
            }
            let refs: Vec<&unigis_raster::Raster> = rs.iter().collect();
            let r = unigis_raster::overlay::weighted_sum(&refs, &weights)
                .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
            write_raster(std::path::Path::new(out), &r)
        })(),
        Tool::HighestPosition { paths, lowest } => (|| {
            let (out, ins) = paths
                .split_last()
                .ok_or_else(|| Error::Unsupported("no rasters".into()))?;
            let rs: Vec<_> = ins
                .iter()
                .map(|p| unigis_raster::read_geotiff(p))
                .collect::<Result<_, _>>()?;
            let refs: Vec<&unigis_raster::Raster> = rs.iter().collect();
            let r = unigis_raster::overlay::position(&refs, !lowest)
                .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
            write_raster(out, &r)
        })(),
        Tool::Combine { paths } => (|| {
            let (out, ins) = paths
                .split_last()
                .ok_or_else(|| Error::Unsupported("no rasters".into()))?;
            let rs: Vec<_> = ins
                .iter()
                .map(|p| unigis_raster::read_geotiff(p))
                .collect::<Result<_, _>>()?;
            let refs: Vec<&unigis_raster::Raster> = rs.iter().collect();
            let r = unigis_raster::overlay::combine(&refs)
                .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
            write_raster(out, &r)
        })(),
        Tool::FuzzyMembership {
            in_raster,
            out_raster,
            function,
            values,
            hedge,
        } => (|| {
            use unigis_raster::overlay::*;
            let v = |i: usize, default: Option<f64>| {
                values
                    .get(i)
                    .copied()
                    .or(default)
                    .ok_or_else(|| Error::Unsupported(format!("{function} needs more numbers")))
            };
            let class = match function.to_ascii_uppercase().as_str() {
                "LINEAR" => Fuzzy::Linear {
                    min: v(0, None)?,
                    max: v(1, None)?,
                },
                "GAUSSIAN" => Fuzzy::Gaussian {
                    midpoint: v(0, None)?,
                    spread: v(1, Some(0.1))?,
                },
                "LARGE" => Fuzzy::Large {
                    midpoint: v(0, None)?,
                    spread: v(1, Some(5.0))?,
                },
                "SMALL" => Fuzzy::Small {
                    midpoint: v(0, None)?,
                    spread: v(1, Some(5.0))?,
                },
                "NEAR" => Fuzzy::Near {
                    midpoint: v(0, None)?,
                    spread: v(1, Some(0.1))?,
                },
                "MSLARGE" => Fuzzy::MsLarge {
                    mean_multiplier: v(0, Some(1.0))?,
                    std_multiplier: v(1, Some(1.0))?,
                },
                "MSSMALL" => Fuzzy::MsSmall {
                    mean_multiplier: v(0, Some(1.0))?,
                    std_multiplier: v(1, Some(1.0))?,
                },
                other => return Err(Error::Unsupported(format!("function {other}"))),
            };
            let hedge = match hedge.to_ascii_uppercase().as_str() {
                "NONE" => Hedge::None,
                "SOMEWHAT" => Hedge::Somewhat,
                "VERY" => Hedge::Very,
                other => return Err(Error::Unsupported(format!("hedge {other}"))),
            };
            let r = unigis_raster::read_geotiff(&in_raster)?;
            write_raster(&out_raster, &fuzzy_membership(&r, class, hedge))
        })(),
        Tool::FuzzyOverlay {
            paths,
            overlay_type,
            gamma,
        } => (|| {
            use unigis_raster::overlay::*;
            let (out, ins) = paths
                .split_last()
                .ok_or_else(|| Error::Unsupported("no rasters".into()))?;
            let kind = match overlay_type.to_ascii_uppercase().as_str() {
                "AND" => FuzzyOverlayType::And,
                "OR" => FuzzyOverlayType::Or,
                "PRODUCT" => FuzzyOverlayType::Product,
                "SUM" => FuzzyOverlayType::Sum,
                "GAMMA" => {
                    FuzzyOverlayType::Gamma(gamma.filter(|g| (0.0..=1.0).contains(g)).ok_or_else(
                        || Error::Unsupported("--gamma is a number from 0 to 1".into()),
                    )?)
                }
                other => return Err(Error::Unsupported(format!("overlay type {other}"))),
            };
            let rs: Vec<_> = ins
                .iter()
                .map(|p| unigis_raster::read_geotiff(p))
                .collect::<Result<_, _>>()?;
            let refs: Vec<&unigis_raster::Raster> = rs.iter().collect();
            let r = fuzzy_overlay(&refs, kind)
                .ok_or_else(|| Error::Unsupported("the rasters do not overlap".into()))?;
            write_raster(out, &r)
        })(),
        Tool::PointToRaster {
            in_features,
            value_field,
            out_raster,
            cell_assignment,
            cellsize,
        } => (|| {
            use unigis_raster::rasterize::*;
            let assignment = match cell_assignment.to_ascii_uppercase().as_str() {
                "MOST_FREQUENT" => PointAssignment::MostFrequent,
                "SUM" => PointAssignment::Sum,
                "MEAN" => PointAssignment::Mean,
                "MAXIMUM" => PointAssignment::Maximum,
                "MINIMUM" => PointAssignment::Minimum,
                "COUNT" => PointAssignment::Count,
                "RANGE" => PointAssignment::Range,
                "STANDARD_DEVIATION" => PointAssignment::StandardDeviation,
                other => return Err(Error::Unsupported(format!("cell assignment {other}"))),
            };
            let (features, kind, epsg, _) = rasterize_input(&in_features, &value_field)?;
            let mut r = points_to_raster(&features, cellsize, assignment, kind)
                .ok_or_else(|| Error::Unsupported("no features to rasterize".into()))?;
            r.epsg = epsg;
            write_raster(&out_raster, &r)
        })(),
        Tool::Resample {
            in_raster,
            out_raster,
            cell_size,
            resampling_type,
        } => (|| {
            use unigis_raster::resample::Technique;
            let t = match resampling_type.to_ascii_uppercase().as_str() {
                "NEAREST" => Technique::Nearest,
                "BILINEAR" => Technique::Bilinear,
                "CUBIC" => Technique::Cubic,
                "MAJORITY" => Technique::Majority,
                other => return Err(Error::Unsupported(format!("resampling type {other}"))),
            };
            let r = unigis_raster::read_geotiff(&in_raster)?;
            let (cols, rows) = unigis_raster::resample::output_size(&r, cell_size);
            if cols.saturating_mul(rows) > unigis_raster::MAX_CELLS {
                return Err(Error::Unsupported(format!(
                    "a cell size of {cell_size} would give {cols} x {rows} cells"
                )));
            }
            write_raster(
                &out_raster,
                &unigis_raster::resample::resample(&r, cell_size, t),
            )
        })(),
        Tool::EucDistance {
            in_source_data,
            out_distance_raster,
            maximum_distance,
        } => (|| {
            let r = unigis_raster::read_geotiff(&in_source_data)?;
            write_raster_plain(
                &out_distance_raster,
                &unigis_raster::resample::euclidean_distance(&r, maximum_distance),
            )
        })(),
        Tool::EucDirection {
            in_source_data,
            out_direction_raster,
            maximum_distance,
        } => (|| {
            let r = unigis_raster::read_geotiff(&in_source_data)?;
            write_raster_plain(
                &out_direction_raster,
                &unigis_raster::resample::euclidean_direction(&r, maximum_distance),
            )
        })(),
        Tool::Expand {
            in_raster,
            out_raster,
            number_cells,
            zone_values,
            expand_method,
        } => expand_shrink(
            true,
            &in_raster,
            &out_raster,
            number_cells,
            &zone_values,
            &expand_method,
        ),
        Tool::Shrink {
            in_raster,
            out_raster,
            number_cells,
            zone_values,
            shrink_method,
        } => expand_shrink(
            false,
            &in_raster,
            &out_raster,
            number_cells,
            &zone_values,
            &shrink_method,
        ),
        Tool::MajorityFilter {
            in_raster,
            out_raster,
            number_neighbors,
            majority_definition,
        } => (|| {
            let eight = eight_neighbors(&number_neighbors)?;
            let half = match majority_definition.to_ascii_uppercase().as_str() {
                "MAJORITY" => false,
                "HALF" => true,
                other => return Err(Error::Unsupported(format!("majority definition {other}"))),
            };
            let r = unigis_raster::read_geotiff(&in_raster)?;
            let out = unigis_raster::generalize::majority_filter(&r, eight, half)
                .map_err(|e| Error::Unsupported(e.0))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::RegionGroup {
            in_raster,
            out_raster,
            number_neighbors,
            zone_connectivity,
            add_link,
            excluded_value,
        } => (|| {
            let eight = eight_neighbors(&number_neighbors)?;
            let cross = match zone_connectivity.to_ascii_uppercase().as_str() {
                "WITHIN" => false,
                "CROSS" => true,
                other => return Err(Error::Unsupported(format!("zone connectivity {other}"))),
            };
            if !matches!(
                add_link.to_ascii_uppercase().as_str(),
                "ADD_LINK" | "NO_LINK"
            ) {
                return Err(Error::Unsupported(format!("add_link {add_link}")));
            }
            let r = unigis_raster::read_geotiff(&in_raster)?;
            write_raster_plain(
                &out_raster,
                &unigis_raster::generalize::region_group(&r, eight, cross, excluded_value),
            )
        })(),
        Tool::EucAllocation {
            in_source_data,
            out_allocation_raster,
            maximum_distance,
        } => (|| {
            let r = unigis_raster::read_geotiff(&in_source_data)?;
            write_raster_plain(
                &out_allocation_raster,
                &unigis_raster::resample::euclidean_allocation(&r, maximum_distance),
            )
        })(),
        Tool::KernelDensity {
            in_features,
            population_field,
            out_raster,
            cell_size,
            search_radius,
            area_unit_scale_factor,
            out_cell_values,
        } => (|| {
            use unigis_raster::density;
            let (features, epsg, kind) = density_input(&in_features, &population_field)?;
            let area = density::area_unit(&area_unit_scale_factor).map_err(Error::Unsupported)?;
            let expected = match out_cell_values.to_ascii_uppercase().as_str() {
                "DENSITIES" => false,
                "EXPECTED_COUNTS" => true,
                other => return Err(Error::Unsupported(format!("output cell values {other}"))),
            };
            let weighted = !population_field.eq_ignore_ascii_case("NONE");
            let mut r = match kind {
                Some(unigis_core::GeometryKind::Polyline | unigis_core::GeometryKind::Polygon) => {
                    density::kernel_density_lines(
                        &features,
                        cell_size,
                        search_radius,
                        area,
                        weighted,
                    )
                }
                _ => density::kernel_density_points(
                    &features,
                    cell_size,
                    search_radius,
                    area,
                    expected,
                ),
            }
            .map_err(Error::Unsupported)?;
            r.epsg = epsg;
            write_raster(&out_raster, &r)
        })(),
        Tool::PointDensity {
            in_point_features,
            population_field,
            out_raster,
            cell_size,
            neighborhood,
            area_unit_scale_factor,
        } => (|| {
            use unigis_raster::density;
            let (features, epsg, _) = density_input(&in_point_features, &population_field)?;
            let area = density::area_unit(&area_unit_scale_factor).map_err(Error::Unsupported)?;
            let n = density::parse_neighborhood(&neighborhood, cell_size)
                .map_err(Error::Unsupported)?;
            let mut r = density::point_density(&features, cell_size, n, area)
                .map_err(Error::Unsupported)?;
            r.epsg = epsg;
            write_raster(&out_raster, &r)
        })(),
        Tool::LineDensity {
            in_polyline_features,
            population_field,
            out_raster,
            cell_size,
            search_radius,
            area_unit_scale_factor,
        } => (|| {
            use unigis_raster::density;
            let (features, epsg, _) = density_input(&in_polyline_features, &population_field)?;
            let area = density::area_unit(&area_unit_scale_factor).map_err(Error::Unsupported)?;
            let weighted = !population_field.eq_ignore_ascii_case("NONE");
            let mut r = density::line_density(&features, cell_size, search_radius, area, weighted)
                .map_err(Error::Unsupported)?;
            r.epsg = epsg;
            write_raster(&out_raster, &r)
        })(),
        Tool::ExtractValuesToPoints {
            in_point_features,
            in_raster,
            out_point_features,
            interpolate_values,
            add_attributes,
        } => (|| {
            let interpolate = match interpolate_values.to_ascii_uppercase().as_str() {
                "NONE" => false,
                "INTERPOLATE" => true,
                other => return Err(Error::Unsupported(format!("interpolate_values {other}"))),
            };
            if !matches!(
                add_attributes.to_ascii_uppercase().as_str(),
                "VALUE_ONLY" | "ALL"
            ) {
                return Err(Error::Unsupported(format!(
                    "add_attributes {add_attributes}"
                )));
            }
            let raster = unigis_raster::read_geotiff(&in_raster)?;
            let (fc, _) = open_feature_class(&in_point_features)?;
            if fc.geometry_kind() != Some(unigis_core::GeometryKind::Point) {
                return Err(Error::Unsupported(
                    "ERROR 000187: Only supports Geometry type Point.".into(),
                ));
            }
            let mut fields = fc.fields().to_vec();
            let mut name = "RASTERVALU".to_string();
            let mut n = 1;
            while fields.iter().any(|f| f.name.eq_ignore_ascii_case(&name)) {
                name = format!("RASTERVAL{n}");
                n += 1;
            }
            use unigis_core::FieldType as Ft;
            let ty = if raster.data_type.is_float() {
                if matches!(raster.data_type, unigis_raster::DataType::F64) {
                    Ft::Double
                } else {
                    Ft::Single
                }
            } else if interpolate {
                Ft::Single
            } else {
                Ft::Integer
            };
            fields.push(unigis_core::Field::new(name, ty));
            let mut features = Vec::new();
            let mut points = Vec::new();
            for f in fc.features() {
                let f = f?;
                points.push(f.geometry.as_ref().and_then(|g| g.xy.first().copied()));
                features.push(f);
            }
            let shp = out_point_features
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("shp"));
            let values = unigis_raster::sample::extract_values(
                &raster,
                &points
                    .iter()
                    .map(|p| p.unwrap_or([f64::NAN; 2]))
                    .collect::<Vec<_>>(),
                interpolate,
            );
            // A shapefile has no null: NoData is -9999 when the raster declares a NoData
            // value, else 0.
            let fill = if raster.nodata.is_some() {
                -9999.0
            } else {
                0.0
            };
            for (f, v) in features.iter_mut().zip(values) {
                let v = v.or(shp.then_some(fill));
                f.attributes
                    .resize(fields.len() - 1, unigis_core::Value::Null);
                f.attributes.push(match (v, ty) {
                    (None, _) => unigis_core::Value::Null,
                    (Some(v), Ft::Integer) => unigis_core::Value::Integer(v as i32),
                    (Some(v), Ft::Single) => unigis_core::Value::Single(v as f32),
                    (Some(v), _) => unigis_core::Value::Double(v),
                });
            }
            let out = unigis_tools::ToolOutput {
                kind: unigis_core::GeometryKind::Point,
                has_z: fc.has_z(),
                has_m: fc.has_m(),
                fields,
                spatial_reference_wkt: fc.spatial_reference_wkt().map(String::from),
                features,
                messages: Vec::new(),
            };
            drop(fc);
            write_output(
                out,
                &out_point_features,
                source_gdb(&in_point_features).as_deref(),
            )
        })(),
        Tool::ExtractByMask {
            in_raster,
            in_mask_data,
            out_raster,
        } => (|| {
            let r = unigis_raster::read_geotiff(&in_raster)?;
            let m = unigis_raster::read_geotiff(&in_mask_data)?;
            let out = unigis_raster::resample::extract_by_mask(&r, &m)
                .ok_or_else(|| Error::Unsupported("raster and mask do not overlap".into()))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::ZonalStatisticsAsTable {
            in_zone_data,
            in_value_raster,
            out_table,
            ignore_nodata,
            statistics_type,
        } => (|| {
            let stats = unigis_raster::zonal::table_statistics(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistic {statistics_type}")))?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            let zones = unigis_raster::read_geotiff(&in_zone_data)?;
            let values = unigis_raster::read_geotiff(&in_value_raster)?;
            let (zones, values) = unigis_raster::resample::zones_on_value_grid(&zones, &values)
                .ok_or_else(|| {
                    Error::Unsupported("zone and value rasters do not overlap".into())
                })?;
            let (fields, rows) = unigis_raster::zonal::zonal_table(&zones, &values, &stats, ignore);
            write_table(&unigis_tools::TableOutput { fields, rows }, &out_table)
        })(),
        Tool::CostDistance {
            in_source_data,
            in_cost_raster,
            out_distance_raster,
            maximum_distance,
            out_backlink_raster,
        } => (|| {
            let (source, cost, result) =
                run_cost(&in_source_data, &in_cost_raster, maximum_distance)?;
            write_raster_plain(
                &out_distance_raster,
                &unigis_raster::cost::distance_raster(&cost, &result),
            )?;
            if let Some(path) = out_backlink_raster {
                write_raster_plain(
                    &path,
                    &unigis_raster::cost::back_link_raster(&source, &result),
                )?;
            }
            Ok(())
        })(),
        Tool::CostBackLink {
            in_source_data,
            in_cost_raster,
            out_backlink_raster,
            maximum_distance,
        } => (|| {
            let (source, _, result) = run_cost(&in_source_data, &in_cost_raster, maximum_distance)?;
            write_raster_plain(
                &out_backlink_raster,
                &unigis_raster::cost::back_link_raster(&source, &result),
            )
        })(),
        Tool::CostAllocation {
            in_source_data,
            in_cost_raster,
            out_allocation_raster,
            maximum_distance,
        } => (|| {
            let (source, cost, result) =
                run_cost(&in_source_data, &in_cost_raster, maximum_distance)?;
            write_raster_plain(
                &out_allocation_raster,
                &unigis_raster::cost::allocation_raster(&cost, &source, &result),
            )
        })(),
        Tool::Corridor {
            in_distance_raster1,
            in_distance_raster2,
            out_raster,
        } => (|| {
            let a = unigis_raster::read_geotiff(&in_distance_raster1)?;
            let b = unigis_raster::read_geotiff(&in_distance_raster2)?;
            let out = unigis_raster::cost::corridor(&a, &b)
                .ok_or_else(|| Error::Unsupported("the rasters must share a grid".into()))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::ZonalHistogram {
            in_zone_data,
            in_value_raster,
            out_table,
            zone_field,
        } => (|| {
            let zones = unigis_raster::read_geotiff(&in_zone_data)?;
            let values = unigis_raster::read_geotiff(&in_value_raster)?;
            let prefix = raster_field_name(&in_zone_data, &zone_field);
            let (fields, rows) =
                unigis_raster::zonal_tables::zonal_histogram(&zones, &values, &prefix)
                    .map_err(Error::Unsupported)?;
            write_table(&unigis_tools::TableOutput { fields, rows }, &out_table)
        })(),
        Tool::TabulateArea {
            in_zone_data,
            in_class_data,
            out_table,
            zone_field,
            class_field,
            processing_cell_size,
            classes_as_rows,
        } => (|| {
            let zones = unigis_raster::read_geotiff(&in_zone_data)?;
            let classes = unigis_raster::read_geotiff(&in_class_data)?;
            let (fields, rows) = unigis_raster::zonal_tables::tabulate_area(
                &zones,
                &classes,
                processing_cell_size,
                classes_as_rows,
                &raster_field_name(&in_zone_data, &zone_field),
                &raster_field_name(&in_class_data, &class_field),
            )
            .map_err(Error::Unsupported)?;
            write_table(&unigis_tools::TableOutput { fields, rows }, &out_table)
        })(),
        Tool::Viewshed {
            in_raster,
            in_observer_features,
            out_raster,
            z_factor,
        } => (|| {
            let dem = unigis_raster::read_geotiff(&in_raster)?;
            let (fc, _) = open_feature_class(&in_observer_features)?;
            let offset_field = fc
                .fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case("OFFSETA"));
            let mut observers = Vec::new();
            for f in fc.features() {
                let f = f?;
                let offset = match offset_field.map(|i| &f.attributes[i]) {
                    Some(Value::Double(v)) => *v,
                    Some(Value::Single(v)) => *v as f64,
                    Some(Value::Integer(v)) => *v as f64,
                    Some(Value::SmallInteger(v)) => *v as f64,
                    _ => 1.0,
                };
                for p in f.geometry.iter().flat_map(|g| g.xy.iter()) {
                    observers.push(unigis_raster::surface::Observer {
                        x: p[0],
                        y: p[1],
                        offset,
                    });
                }
            }
            write_raster(
                &out_raster,
                &unigis_raster::surface::viewshed(&dem, &observers, z_factor),
            )
        })(),
        Tool::Aspect {
            in_raster,
            out_raster,
        } => (|| {
            let dem = unigis_raster::read_geotiff(&in_raster)?;
            write_raster(&out_raster, &unigis_raster::surface::aspect(&dem))
        })(),
        Tool::Hillshade {
            in_raster,
            out_raster,
            azimuth,
            altitude,
            model_shadows,
            z_factor,
        } => (|| {
            let shadows = match model_shadows.to_ascii_uppercase().as_str() {
                "NO_SHADOWS" => false,
                "SHADOWS" => true,
                other => return Err(Error::Unsupported(format!("model_shadows {other}"))),
            };
            let dem = unigis_raster::read_geotiff(&in_raster)?;
            let out =
                unigis_raster::surface::hillshade_with(&dem, azimuth, altitude, z_factor, shadows);
            write_raster(&out_raster, &out)
        })(),
        Tool::CheckGeometry {
            in_features,
            out_table,
            validation_method,
        } => check_geometry_tool(&in_features, &out_table, &validation_method),
        Tool::RepairGeometry {
            in_features,
            delete_null,
            validation_method,
            output,
        } => (|| {
            let delete_null = match delete_null.to_ascii_uppercase().as_str() {
                "DELETE_NULL" => true,
                "KEEP_NULL" => false,
                other => return Err(Error::Unsupported(format!("delete_null {other}"))),
            };
            let method = validation_method_arg(&validation_method)?;
            let (fc, _) = open_feature_class(&in_features)?;
            let out = unigis_tools::repair_geometry(fc.as_ref(), delete_null, method)?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_features).as_deref()),
                None => update_in_place(out, &in_features),
            }
        })(),
        Tool::Dissolve {
            in_features,
            out_feature_class,
            dissolve_fields,
            statistics_fields,
            multi_part,
        } => dissolve_tool(
            &in_features,
            &out_feature_class,
            &dissolve_fields,
            statistics_fields.as_deref(),
            &multi_part,
        ),
        Tool::Buffer {
            in_features,
            out_feature_class,
            buffer_distance,
            line_side,
            line_end_type,
            dissolve_option,
            dissolve_fields,
            max_arc_angle,
        } => {
            if let Some(degrees) = max_arc_angle {
                let points = open_feature_class(&in_features).is_ok_and(|(fc, _)| {
                    matches!(
                        fc.geometry_kind(),
                        Some(
                            unigis_core::GeometryKind::Point
                                | unigis_core::GeometryKind::Multipoint
                        )
                    )
                });
                if !points {
                    unigis_core::curves::set_arc_angle_step(degrees.to_radians());
                }
            }
            buffer_tool(
                &in_features,
                &out_feature_class,
                &buffer_distance,
                &line_side,
                &line_end_type,
                &dissolve_option,
                dissolve_fields,
            )
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A path split into its geodatabase and the item inside it, if any.
enum Location {
    Shapefile(PathBuf),
    Geodatabase { gdb: PathBuf, item: Option<String> },
}

fn locate(path: &Path) -> Location {
    let comps: Vec<_> = path.components().collect();
    let is_gdb_name = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gdb"));
    // Prefer the directory that really is a geodatabase; fall back to a `.gdb` name
    // (e.g. an output that does not exist yet).
    let mut prefix = PathBuf::new();
    let mut found = None;
    for (i, c) in comps.iter().enumerate() {
        prefix.push(c);
        if prefix.join("a00000001.gdbtable").is_file() {
            found = Some(i);
            break;
        }
        if found.is_none() && is_gdb_name(&prefix) && !prefix.is_dir() {
            found = Some(i);
        }
    }
    match found {
        Some(i) => {
            let gdb: PathBuf = comps[..=i].iter().collect();
            // The last component names the item; a feature dataset before it is optional.
            let item = comps[i + 1..]
                .last()
                .map(|c| c.as_os_str().to_string_lossy().into_owned());
            Location::Geodatabase { gdb, item }
        }
        None => Location::Shapefile(path.to_path_buf()),
    }
}

/// Tools that ArcGIS refuses to run from a table with 64-bit ObjectIDs (every
/// GeoPackage table) into a shapefile (observed in ArcGIS Pro 3.7; Select, Copy
/// Features and Clip on points run).
fn check_64bit_oids(tool: &Tool) -> Result<()> {
    let (inputs, output, code): (Vec<&PathBuf>, &PathBuf, &str) = match tool {
        Tool::Buffer {
            in_features,
            out_feature_class,
            ..
        }
        | Tool::Dissolve {
            in_features,
            out_feature_class,
            ..
        }
        | Tool::Erase {
            in_features,
            out_feature_class,
            ..
        }
        | Tool::FeatureToPoint {
            in_features,
            out_feature_class,
            ..
        }
        | Tool::MultipartToSinglepart {
            in_features,
            out_feature_class,
        } => (vec![in_features], out_feature_class, "010672"),
        // Clip runs for points but not for lines and polygons (observed).
        Tool::Clip {
            in_features,
            out_feature_class,
            ..
        } if open_feature_class(in_features)
            .is_ok_and(|(fc, _)| fc.geometry_kind() != Some(unigis_core::GeometryKind::Point)) =>
        {
            (vec![in_features], out_feature_class, "010672")
        }
        Tool::Intersect { paths, .. } => {
            let (out, inputs) = paths.split_last().expect("clap requires paths");
            (inputs.iter().collect(), out, "010672")
        }
        Tool::SpatialJoin {
            target_features,
            out_feature_class,
            ..
        } => (vec![target_features], out_feature_class, "002809"),
        _ => return Ok(()),
    };
    let to_shapefile =
        geopackage_item(output).is_none() && matches!(locate(output), Location::Shapefile(_));
    if !to_shapefile || !inputs.iter().any(|p| geopackage_item(p).is_some()) {
        return Ok(());
    }
    Err(Error::Unsupported(if code == "010672" {
        "ERROR 010672: The output workspace does not support database fields containing 64 \
         bit integer values."
            .into()
    } else {
        format!(
            "ERROR 002809: Field TARGET_FID in {} is of an unsupported type for the output \
             workspace.",
            output.display()
        )
    }))
}

/// The gpkg_spatial_ref_sys row ArcGIS writes for a coordinate system: the EPSG
/// code as srs_id, its Esri WKT and name, and a description made from the name.
fn gpkg_spatial_ref(crs: &unigis_proj::Crs) -> Option<unigis_gpkg::write::SpatialRef> {
    let (_, latest) = crs.wkid()?;
    let definition = crs.database_wkt();
    let name = definition.split('"').nth(1)?.to_string();
    // ArcGIS takes descriptions from its own projection database; this matches
    // it for names like WGS_1984_UTM_Zone_33N and GCS_WGS_1984 only.
    let description = name.strip_prefix("GCS_").unwrap_or(&name).replace('_', " ");
    // Esri's own codes (World_Robinson 54030, USA_Contiguous_Albers 102003).
    let organization = if (53000..55000).contains(&latest) || latest >= 100000 {
        "Esri"
    } else {
        "EPSG"
    };
    Some(unigis_gpkg::write::SpatialRef {
        srs_id: latest as i64,
        srs_name: name,
        organization: organization.into(),
        organization_coordsys_id: latest as i64,
        definition_12_063: crs
            .esri_wkt2(organization, latest)
            .unwrap_or_else(|| "undefined".into()),
        definition,
        description: Some(description),
    })
}

/// A path inside a GeoPackage file (`data.gpkg/main.roads`): the file and the
/// table name, if any.
fn geopackage_item(path: &Path) -> Option<(PathBuf, Option<String>)> {
    let mut prefix = PathBuf::new();
    let comps: Vec<_> = path.components().collect();
    for (i, c) in comps.iter().enumerate() {
        prefix.push(c);
        if prefix
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("gpkg"))
            && (prefix.is_file() || !prefix.exists())
        {
            let item = comps[i + 1..]
                .last()
                .map(|c| c.as_os_str().to_string_lossy().into_owned());
            return Some((prefix, item));
        }
    }
    None
}

fn open_feature_class(path: &Path) -> Result<(Box<dyn FeatureClass>, &'static str)> {
    if let Some((gpkg, item)) = geopackage_item(path) {
        let item = item.ok_or_else(|| {
            Error::Unsupported(format!(
                "{} is a workspace; name a table inside it",
                gpkg.display()
            ))
        })?;
        let table = unigis_gpkg::GeoPackage::open(&gpkg)?.open_table(&item)?;
        let kind = if table.geometry_kind().is_some() {
            "FeatureClass"
        } else {
            "Table"
        };
        return Ok((Box::new(table), kind));
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"))
    {
        return Ok((Box::new(unigis_csv::CsvTable::open(path)?), "TextFile"));
    }
    match locate(path) {
        Location::Shapefile(p)
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dbf"))
                && !p.with_extension("shp").exists() =>
        {
            Ok((Box::new(unigis_shapefile::DbfTable::open(p)?), "Table"))
        }
        Location::Shapefile(p) => Ok((Box::new(ShapefileReader::open(p)?), "ShapeFile")),
        Location::Geodatabase {
            gdb,
            item: Some(item),
        } => {
            let table = Geodatabase::open(&gdb)?.open_table(&item)?;
            let kind = if table.geometry_kind().is_some() {
                "FeatureClass"
            } else {
                "Table"
            };
            Ok((Box::new(table), kind))
        }
        Location::Geodatabase { gdb, item: None } => Err(Error::Unsupported(format!(
            "{} is a workspace; name a feature class inside it",
            gdb.display()
        ))),
    }
}

fn describe(path: &Path) -> Result<()> {
    if let Location::Geodatabase { gdb, item: None } = locate(path) {
        return describe_workspace(&gdb);
    }
    let (fc, data_type) = open_feature_class(path)?;
    let py = |b: bool| if b { "True" } else { "False" };
    println!("dataType:         {data_type}");
    if let Some(kind) = fc.geometry_kind() {
        println!("shapeType:        {kind:?}");
        println!("hasZ:             {}", py(fc.has_z()));
        println!("hasM:             {}", py(fc.has_m()));
        println!("shapeFieldName:   {}", fc.shape_field_name());
    }
    println!("OIDFieldName:     {}", fc.oid_field_name());
    println!("featureCount:     {}", fc.features().count());
    if let Some(e) = fc.stored_extent() {
        println!(
            "extent:           {} {} {} {}",
            e.xmin, e.ymin, e.xmax, e.ymax
        );
        if let Some((lo, hi)) = e.z {
            println!("extent Z:         {lo} {hi}");
        }
        if let Some((lo, hi)) = e.m {
            println!("extent M:         {lo} {hi}");
        }
    }
    if fc.geometry_kind().is_some() {
        let sr = fc
            .spatial_reference_wkt()
            .and_then(|wkt| wkt.split('"').nth(1))
            .unwrap_or("Unknown");
        println!("spatialReference: {sr}");
    }
    println!("fields:");
    println!("  {:<16} OID", fc.oid_field_name());
    if fc.geometry_kind().is_some() {
        println!("  {:<16} Geometry", fc.shape_field_name());
    }
    for f in fc.fields() {
        let detail = match f.field_type {
            FieldType::String => format!("length {}", f.length),
            FieldType::SmallInteger | FieldType::Integer | FieldType::Double if f.precision > 0 => {
                format!("precision {}, scale {}", f.precision, f.scale)
            }
            _ => String::new(),
        };
        let alias = f
            .alias
            .as_deref()
            .map(|a| format!(" alias \"{a}\""))
            .unwrap_or_default();
        let domain = f
            .domain
            .as_deref()
            .map(|d| format!(" domain \"{d}\""))
            .unwrap_or_default();
        println!(
            "  {:<16} {:<15} {detail}{alias}{domain}",
            f.name,
            esri_type_name(f.field_type)
        );
    }
    describe_subtypes(path)
}

fn describe_subtypes(path: &Path) -> Result<()> {
    let Location::Geodatabase {
        gdb,
        item: Some(item),
    } = locate(path)
    else {
        return Ok(());
    };
    let table = Geodatabase::open(&gdb)?.open_table(&item)?;
    if let Some(st) = table.subtypes() {
        println!(
            "subtypeFieldName: {} (default {})",
            st.field, st.default_code
        );
        for s in &st.subtypes {
            println!("  {:<6} {}", s.code, s.name);
            for f in &s.fields {
                let domain = f
                    .domain
                    .as_deref()
                    .map(|d| format!(" domain \"{d}\""))
                    .unwrap_or_default();
                let default = f
                    .default
                    .as_ref()
                    .map(|v| format!(" default {v:?}"))
                    .unwrap_or_default();
                println!("         {}{domain}{default}", f.field);
            }
        }
    }
    Ok(())
}

fn describe_workspace(gdb: &Path) -> Result<()> {
    let ws = Geodatabase::open(gdb)?;
    println!("dataType:         Workspace");
    println!("workspaceType:    LocalDatabase");
    println!("workspaceFactoryProgID: esriDataSourcesGDB.FileGDBWorkspaceFactory");
    if !ws.feature_datasets().is_empty() {
        println!("feature datasets: {}", ws.feature_datasets().join(", "));
    }
    if !ws.domains().is_empty() {
        println!("domains:");
        for d in ws.domains() {
            let kind = match &d.kind {
                unigis_filegdb::DomainKind::Coded(v) => format!("coded, {} values", v.len()),
                unigis_filegdb::DomainKind::Range { min, max } => {
                    format!("range {min:?} .. {max:?}")
                }
            };
            println!(
                "  {:<40} {:<13} {kind}",
                d.name,
                esri_type_name(d.field_type)
            );
        }
    }
    if !ws.relationship_classes().is_empty() {
        println!("relationship classes:");
        for r in ws.relationship_classes() {
            println!(
                "  {:<32} {:?}{} {} -> {} ({} = {})",
                r.name,
                r.cardinality,
                if r.is_composite { " composite" } else { "" },
                r.origin,
                r.destination,
                r.origin_primary_key,
                r.origin_foreign_key
            );
        }
    }
    println!("children:");
    for e in ws.entries() {
        let path = match &e.feature_dataset {
            Some(fd) => format!("{fd}/{}", e.name),
            None => e.name.clone(),
        };
        let kind = match ws.open_table(&e.name) {
            Ok(t) => match t.geometry_kind() {
                Some(k) => format!(
                    "FeatureClass ({k:?}{}{})",
                    if t.has_z() { " Z" } else { "" },
                    if t.has_m() { " M" } else { "" }
                ),
                None => "Table".into(),
            },
            Err(err) => format!("unreadable: {err}"),
        };
        println!("  {path:<40} {kind}");
    }
    Ok(())
}

/// The field type names ArcGIS Pro shows (`Field.type`).
fn esri_type_name(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "SmallInteger",
        FieldType::Integer => "Integer",
        FieldType::BigInteger => "BigInteger",
        FieldType::Single => "Single",
        FieldType::Double => "Double",
        FieldType::String => "String",
        FieldType::Date => "Date",
        FieldType::DateOnly => "DateOnly",
        FieldType::TimeOnly => "TimeOnly",
        FieldType::TimestampOffset => "TimestampOffset",
        FieldType::ObjectId => "OID",
        FieldType::Geometry => "Geometry",
        FieldType::Blob => "Blob",
        FieldType::Raster => "Raster",
        FieldType::Guid => "Guid",
        FieldType::GlobalId => "GlobalID",
        FieldType::Xml => "XML",
    }
}

/// What ArcGIS Copy Features does with a field when writing a shapefile
/// (observed in ArcGIS Pro 3.7): store it, drop it with WARNING 000592, or fail the
/// tool with ERROR 000308.
enum ShapefileField {
    Store,
    Drop,
    Fail,
}

fn shapefile_field(t: FieldType) -> ShapefileField {
    match t {
        FieldType::Blob | FieldType::Raster => ShapefileField::Drop,
        FieldType::BigInteger
        | FieldType::Xml
        | FieldType::DateOnly
        | FieldType::TimeOnly
        | FieldType::TimestampOffset => ShapefileField::Fail,
        _ => ShapefileField::Store,
    }
}

/// ArcGIS `ValidateFieldName` for file geodatabases: invalid characters become
/// `_`, names must start with a letter, at most 64 characters, unique.
fn geodatabase_field_names(fields: &[Field]) -> Vec<String> {
    let mut seen: Vec<String> = vec!["OBJECTID".into(), "SHAPE".into()];
    fields
        .iter()
        .map(|f| {
            let mut n: String = f
                .name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if !n.chars().next().is_some_and(char::is_alphabetic) {
                n.insert(0, 'F');
            }
            n = n.chars().take(64).collect();
            let base = n.clone();
            let mut i = 1;
            while seen.iter().any(|s| s.eq_ignore_ascii_case(&n)) {
                n = format!("{base}_{i}");
                i += 1;
            }
            seen.push(n.clone());
            n
        })
        .collect()
}

/// Writes a tool's output to a shapefile or `path.gdb/name`, applying the target
/// format's ArcGIS rules.
fn write_output(out: ToolOutput, output: &Path, source_gdb: Option<&Path>) -> Result<()> {
    write_output_as(out, output, source_gdb, true)
}

/// `write_output`; `own_wkt` false keeps the coordinate system's text as it is (Copy and Define
/// Projection, where ArcGIS copies the text it was given).
fn write_output_as(
    mut out: ToolOutput,
    output: &Path,
    source_gdb: Option<&Path>,
    own_wkt: bool,
) -> Result<()> {
    for m in &out.messages {
        eprintln!("{m}");
    }
    // ArcGIS writes its own WKT of the coordinate system, not the input's text.
    if own_wkt {
        out.spatial_reference_wkt = out
            .spatial_reference_wkt
            .map(|w| unigis_proj::Crs::from_wkt(&w).map_or(w, |crs| crs.arcgis_wkt()));
    }
    if let Some((gpkg, item)) = geopackage_item(output) {
        let name = item
            .ok_or_else(|| Error::Unsupported("name the output table inside the .gpkg".into()))?;
        let name = name.strip_prefix("main.").unwrap_or(&name).to_string();
        let crs = out
            .spatial_reference_wkt
            .as_deref()
            .and_then(|w| unigis_proj::Crs::from_wkt(w).ok());
        let spatial_ref = crs.as_ref().and_then(gpkg_spatial_ref);
        let xy_domain = crs.as_ref().and_then(arcgis_xy_domain);
        unigis_gpkg::write::write_table(
            &gpkg,
            &unigis_gpkg::write::NewTable {
                name: &name,
                geometry: Some((out.kind, out.has_z, out.has_m)),
                fields: &out.fields,
                spatial_ref: spatial_ref.as_ref(),
                xy_domain,
                features: &out.features,
            },
        )?;
        println!(
            "Wrote {} features to {}",
            out.features.len(),
            output.display()
        );
        return Ok(());
    }
    match locate(output) {
        Location::Geodatabase { gdb, item } => {
            let item = item.ok_or_else(|| {
                Error::Unsupported("name the output feature class inside the .gdb".into())
            })?;
            write_geodatabase(
                out,
                &gdb,
                &item,
                source_gdb,
                output_dataset(output, &gdb).as_deref(),
            )
        }
        Location::Shapefile(path) => {
            let n = write_shapefile(out, &path)?;
            println!("Wrote {n} features to {}", path.display());
            Ok(())
        }
    }
}

/// A writer for a new geodatabase, or one that adds to the geodatabase at `gdb` if it exists.
fn open_geodatabase_writer(gdb: &Path) -> Result<unigis_filegdb::GeodatabaseWriter> {
    if gdb.join("a00000001.gdbtable").is_file() {
        unigis_filegdb::GeodatabaseWriter::open_existing(gdb)
    } else if gdb.exists() {
        Err(Error::Unsupported(format!(
            "{} exists but is not a file geodatabase",
            gdb.display()
        )))
    } else {
        unigis_filegdb::GeodatabaseWriter::create(gdb)
    }
}

/// The feature dataset an output path names: `city.gdb/Dataset/name`.
fn output_dataset(output: &Path, gdb: &Path) -> Option<String> {
    let rest: Vec<_> = output.strip_prefix(gdb).ok()?.components().collect();
    (rest.len() == 2).then(|| rest[0].as_os_str().to_string_lossy().into_owned())
}

fn write_geodatabase(
    out: ToolOutput,
    gdb: &Path,
    name: &str,
    source_gdb: Option<&Path>,
    dataset: Option<&str>,
) -> Result<()> {
    let mut out = out;
    // Data that brings its own Shape_Length / Shape_Area (a copy) keeps those values; otherwise
    // the class is laid out as ArcGIS creates one: `Shape`, and for lines and polygons measure
    // fields the writer fills in.
    let own_measures = out.fields.iter().any(|f| {
        f.name.eq_ignore_ascii_case("Shape_Length") || f.name.eq_ignore_ascii_case("Shape_Area")
    });
    for (measure, wanted) in [
        (
            "Shape_Length",
            matches!(out.kind, GeometryKind::Polyline | GeometryKind::Polygon),
        ),
        ("Shape_Area", out.kind == GeometryKind::Polygon),
    ] {
        if wanted && !own_measures {
            out.fields.push(Field::new(measure, FieldType::Double));
            for f in &mut out.features {
                f.attributes.push(Value::Null);
            }
        }
    }
    let names = geodatabase_field_names(&out.fields);
    let fields: Vec<Field> = out
        .fields
        .iter()
        .zip(&names)
        .map(|(f, n)| Field {
            name: n.clone(),
            ..f.clone()
        })
        .collect();
    let mut w = open_geodatabase_writer(gdb)?;
    if !own_measures {
        w.use_standard_geometry_fields();
    }
    // Like ArcGIS, carry over the attribute domains the copied fields use.
    if let Some(source) = source_gdb {
        let src = Geodatabase::open(source)?;
        for name in fields.iter().filter_map(|f| f.domain.as_deref()) {
            if let Some(d) = src.domain(name).filter(|d| !w.has_domain(&d.name)) {
                w.add_domain(d)?;
            }
        }
    }
    // In a feature dataset the class takes the dataset's coordinate system: the output is
    // projected to it (as ArcGIS does), unless it has none or says the same.
    if let Some(ds) = dataset
        && let Some(Some(target)) = w.feature_dataset_srs(ds)
    {
        out = project_to(out, &target)?;
    }
    let mut t = match dataset {
        Some(ds) => w.create_feature_class_in(ds, name, out.kind, out.has_z, out.has_m, &fields)?,
        None => w.create_table(
            name,
            Some(out.kind),
            out.has_z,
            out.has_m,
            &fields,
            out.spatial_reference_wkt.as_deref(),
        )?,
    };
    let n = out.features.len();
    for f in &out.features {
        t.write(f)?;
    }
    w.finish_table(t)?;
    w.finish()?;
    println!(
        "Wrote {n} features to {}",
        gdb.join(dataset.unwrap_or("")).join(name).display()
    );
    Ok(())
}

/// Writes a shapefile; returns the number of features written.
fn write_shapefile(out: ToolOutput, path: &Path) -> Result<usize> {
    let mut keep = Vec::new();
    for (i, f) in out.fields.iter().enumerate() {
        match shapefile_field(f.field_type) {
            ShapefileField::Store => keep.push(i),
            ShapefileField::Drop => eprintln!(
                "WARNING 000592: Output format does not support BLOB, Raster, or GUID fields. Field(s) {} will not be converted to output.",
                f.name
            ),
            ShapefileField::Fail => {
                return Err(Error::Unsupported(format!(
                    "ERROR 000308: Invalid field type: field {} ({}) cannot be stored in a shapefile",
                    f.name,
                    esri_type_name(f.field_type)
                )));
            }
        }
    }
    let mut fields: Vec<Field> = keep.iter().map(|&i| out.fields[i].clone()).collect();
    // A .dbf needs a field; ArcGIS's default for a new shapefile is "Id" N(6).
    let default_id = fields.is_empty();
    if default_id {
        let mut id = Field::new("Id", FieldType::Integer);
        id.precision = 6;
        fields.push(id);
    }
    let mut w = ShapefileWriter::create(
        path,
        out.kind,
        out.has_z,
        out.has_m,
        &fields,
        out.spatial_reference_wkt.as_deref(),
    )?;
    let (mut n, mut skipped) = (0, Vec::new());
    for f in out.features {
        let attributes = if default_id {
            vec![Value::Integer(0)]
        } else {
            keep.iter().map(|&i| f.attributes[i].clone()).collect()
        };
        match w.write(&Feature {
            oid: f.oid,
            geometry: f.geometry,
            attributes,
        }) {
            Ok(()) => n += 1,
            // ArcGIS skips rows whose values cannot be stored and carries on.
            Err(Error::BadValue(why)) => {
                eprintln!(
                    "WARNING 160277: The row contains a bad value. (OID {}: {why})",
                    f.oid
                );
                skipped.push(f.oid);
            }
            Err(e) => return Err(e),
        }
    }
    if !skipped.is_empty() {
        eprintln!(
            "WARNING 000595: {} records could not be processed: {skipped:?}",
            skipped.len()
        );
    }
    w.finish()?;
    Ok(n)
}

fn source_gdb(input: &Path) -> Option<PathBuf> {
    match locate(input) {
        Location::Geodatabase { gdb, .. } => Some(gdb),
        Location::Shapefile(_) => None,
    }
}

/// `out` with its coordinates in the system `target_wkt`, when it has another one.
fn project_to(mut out: ToolOutput, target_wkt: &str) -> Result<ToolOutput> {
    let Some(source_wkt) = out.spatial_reference_wkt.clone() else {
        return Ok(out);
    };
    let (source, target) = (
        unigis_proj::Crs::from_wkt(&source_wkt)?,
        unigis_proj::Crs::from_wkt(target_wkt)?,
    );
    if source.arcgis_wkt() == target.arcgis_wkt() {
        return Ok(out);
    }
    let transform = unigis_proj::Transform::with_method(&source, &target, None)?;
    let deviation = unigis_core::curves::arcgis_densify_deviation(source.is_geographic());
    for f in &mut out.features {
        if let Some(g) = &mut f.geometry {
            if g.has_curves() {
                *g = g.densify_curves_arcgis(deviation)?;
            }
            transform.apply(&mut g.xy)?;
        }
    }
    out.spatial_reference_wkt = Some(target.arcgis_wkt());
    eprintln!("The output was projected to the feature dataset's coordinate system.");
    Ok(out)
}

/// The XY domain ArcGIS gives a coordinate system: its own table (`unigis_filegdb::xy_domains`), else
/// what could be derived from the projection.
fn arcgis_xy_domain(crs: &unigis_proj::Crs) -> Option<(f64, f64, f64)> {
    unigis_filegdb::xy_domains::for_wkt(crs.arcgis_wkt().as_str())
        .map(|d| (d.x_origin, d.y_origin, 1.0 / d.scale))
        .or_else(|| crs.xy_domain())
}

/// `city.gdb/table` as the geodatabase and the table's name.
fn gdb_table(path: &Path) -> Result<(PathBuf, String)> {
    match locate(path) {
        Location::Geodatabase {
            gdb,
            item: Some(item),
        } => Ok((gdb, item)),
        _ => Err(Error::Unsupported(
            "name a table inside a file geodatabase (city.gdb/table)".into(),
        )),
    }
}

/// Subtype codes separated by `;`.
fn codes(text: &str) -> Result<Vec<i64>> {
    text.split(';')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| {
            c.parse().map_err(|_| {
                Error::Unsupported(format!("ERROR 000192: Invalid value for Subtype Code: {c}"))
            })
        })
        .collect()
}

/// The files of a shapefile: `<stem>.<ext>` for each extension a shapefile (or ArcGIS, beside it) uses.
fn shapefile_files(shp: &Path) -> Vec<PathBuf> {
    const EXTENSIONS: &[&str] = &[
        "shp", "shx", "dbf", "prj", "cpg", "sbn", "sbx", "shp.xml", "qix", "idx", "atx", "ixs",
        "mxs", "fbn", "fbx", "ain", "aih", "dbf.xml", "dbf.lock",
    ];
    let (Some(dir), Some(stem)) = (shp.parent(), shp.file_stem()) else {
        return Vec::new();
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let stem = stem.to_string_lossy().into_owned();
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_ascii_lowercase())
                        .unwrap_or_default();
                    let prefix = format!("{}.", stem.to_ascii_lowercase());
                    name.strip_prefix(&prefix).is_some_and(|ext| {
                        EXTENSIONS.contains(&ext)
                            || (ext.ends_with(".lock") && ext.starts_with("shp"))
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Delete: a shapefile with its sidecar files, or a geodatabase item.
fn delete_data(path: &Path) -> Result<()> {
    match locate(path) {
        Location::Shapefile(shp) => {
            let files = shapefile_files(&shp);
            if files.is_empty() {
                return Err(Error::Unsupported(format!(
                    "ERROR 000732: Dataset {} does not exist or is not supported",
                    shp.display()
                )));
            }
            for f in files {
                std::fs::remove_file(&f).map_err(|e| Error::io(&f, e))?;
            }
            Ok(())
        }
        Location::Geodatabase { gdb, item: Some(item) } => {
            let gone = unigis_filegdb::delete_item(&gdb, &item)?;
            println!("Deleted: {}", gone.join(", "));
            Ok(())
        }
        Location::Geodatabase { item: None, .. } => Err(Error::Unsupported(
            "name the item to delete inside the .gdb (deleting a whole geodatabase: remove the folder)".into(),
        )),
    }
}

/// Rename: a shapefile's files, or a geodatabase item.
fn rename_data(path: &Path, to: &Path) -> Result<()> {
    let new = to
        .file_stem()
        .or_else(|| to.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| Error::Unsupported("give the new name".into()))?;
    match locate(path) {
        Location::Shapefile(shp) => {
            let files = shapefile_files(&shp);
            if files.is_empty() {
                return Err(Error::Unsupported(format!(
                    "ERROR 000732: Dataset {} does not exist or is not supported",
                    shp.display()
                )));
            }
            let dir = shp.parent().unwrap_or(Path::new("."));
            let old_stem = shp
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if shapefile_files(&dir.join(format!("{new}.shp")))
                .iter()
                .any(|p| !files.contains(p))
            {
                return Err(Error::Unsupported(format!(
                    "ERROR 000725: Output Dataset or Feature Class: Dataset {new} already exists."
                )));
            }
            for f in files {
                let name = f
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let rest = name.get(old_stem.len()..).unwrap_or("");
                let dest = dir.join(format!("{new}{rest}"));
                std::fs::rename(&f, &dest).map_err(|e| Error::io(&f, e))?;
            }
            Ok(())
        }
        Location::Geodatabase {
            gdb,
            item: Some(item),
        } => unigis_filegdb::rename_item(&gdb, &item, &new),
        Location::Geodatabase { item: None, .. } => Err(Error::Unsupported(
            "name the item to rename inside the .gdb".into(),
        )),
    }
}

/// Copy Features: the input unchanged, written to the output format.
fn copy_features(input: &Path, output: &Path) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let kind = fc.geometry_kind().ok_or_else(|| {
        Error::Unsupported("input has no geometry; use Copy Rows for tables".into())
    })?;
    let out = ToolOutput {
        kind,
        has_z: fc.has_z(),
        has_m: fc.has_m(),
        fields: fc.fields().to_vec(),
        spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
        features: fc.features().collect::<Result<_>>()?,
        messages: Vec::new(),
    };
    write_output(out, output, source_gdb(input).as_deref())
}

fn clip_tool(input: &Path, clip_features: &Path, output: &Path) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let (clip, _) = open_feature_class(clip_features)?;
    let out = unigis_tools::clip(fc.as_ref(), clip.as_ref())?;
    write_output(out, output, source_gdb(input).as_deref())
}

/// The name ArcGIS uses for a dataset in output field names (`FID_<name>`).
fn dataset_name(path: &Path) -> String {
    match locate(path) {
        Location::Geodatabase {
            item: Some(item), ..
        } => item,
        _ => path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

fn attributes_arg(a: &str) -> Result<bool> {
    match a.to_ascii_uppercase().as_str() {
        "ATTRIBUTES" => Ok(true),
        "NO_ATTRIBUTES" => Ok(false),
        other => Err(Error::Unsupported(format!(
            "attributes {other}: ATTRIBUTES or NO_ATTRIBUTES"
        ))),
    }
}

/// A cluster tolerance in the first input's coordinate units.
fn cluster_tolerance_arg(
    text: Option<&str>,
    inputs: &[unigis_tools::NamedInput],
) -> Result<Option<f64>> {
    text.map(|t| {
        unigis_tools::registry::distance_in_units(
            t,
            inputs
                .first()
                .and_then(|i| i.features.spatial_reference_wkt()),
        )
    })
    .transpose()
}

fn overlay_tool(
    inputs: &[PathBuf],
    output: &Path,
    tool: impl Fn(&[unigis_tools::NamedInput]) -> Result<ToolOutput>,
) -> Result<()> {
    let opened: Vec<Box<dyn FeatureClass>> = inputs
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let named: Vec<unigis_tools::NamedInput> = inputs
        .iter()
        .zip(&opened)
        .map(|(p, f)| unigis_tools::NamedInput {
            name: dataset_name(p),
            features: f.as_ref(),
        })
        .collect();
    write_output(tool(&named)?, output, source_gdb(&inputs[0]).as_deref())
}

fn near_tool(
    input: &Path,
    near_features: &str,
    output: Option<&Path>,
    search_radius: Option<&str>,
    location: &str,
    angle: &str,
    method: &str,
) -> Result<()> {
    let method = near_method(method)?;
    let flag = |value: &str, yes: &str, no: &str| match value.to_ascii_uppercase() {
        v if v == yes => Ok(true),
        v if v == no => Ok(false),
        v => Err(Error::Unsupported(format!(
            "expected {yes} or {no}, got {v}"
        ))),
    };
    let location = flag(location, "LOCATION", "NO_LOCATION")?;
    let angle = flag(angle, "ANGLE", "NO_ANGLE")?;
    let (fc, _) = open_feature_class(input)?;
    let near_paths: Vec<PathBuf> = near_features
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    let opened: Vec<Box<dyn FeatureClass>> = near_paths
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let near_inputs: Vec<unigis_tools::PathInput> = near_paths
        .iter()
        .zip(&opened)
        .map(|(p, f)| unigis_tools::PathInput {
            source: catalog_path(p),
            features: f.as_ref(),
        })
        .collect();
    let radius = search_radius
        .map(|r| near_radius(r, method, fc.spatial_reference_wkt()))
        .transpose()?;
    let out = unigis_tools::near_with(fc.as_ref(), &near_inputs, radius, location, angle, method)?;
    drop(fc);
    if let Some(output) = output {
        return write_output(out, output, source_gdb(input).as_deref());
    }
    update_in_place(out, input)
}

/// Replaces a shapefile with a tool's output (tools that edit their input, like
/// Near and Append): written next to it, then moved over its files.
fn update_in_place(out: ToolOutput, dataset: &Path) -> Result<()> {
    let shp = match locate(dataset) {
        Location::Shapefile(shp) => shp,
        Location::Geodatabase {
            gdb,
            item: Some(item),
        } => return update_gdb_in_place(out, &gdb, &item),
        Location::Geodatabase { .. } => {
            return Err(Error::Unsupported(format!(
                "{} is a workspace; name a table inside it",
                dataset.display()
            )));
        }
    };
    let stem = shp
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = shp.with_file_name(format!("{stem}.gf-tmp.shp"));
    for m in &out.messages {
        eprintln!("{m}");
    }
    let n = write_shapefile(out, &tmp)?;
    // The new spatial index replaces the old one with the rest.
    for ext in ["shp", "shx", "dbf", "prj", "cpg", "sbn", "sbx"] {
        let from = tmp.with_extension(ext);
        if from.exists() {
            std::fs::rename(&from, shp.with_extension(ext)).map_err(|e| Error::io(&from, e))?;
        }
    }
    println!("Updated {} ({n} features)", shp.display());
    Ok(())
}

/// Replaces the rows of a geodatabase table with a tool's output, keeping the table's
/// identity, geometry definition and indexes. Rows keep their OBJECTID when the tool
/// carried it through; rows without a usable one are numbered after the largest used.
fn update_gdb_in_place(out: ToolOutput, gdb: &Path, item: &str) -> Result<()> {
    for m in &out.messages {
        eprintln!("{m}");
    }
    let base = Geodatabase::open(gdb)?.table_base(item)?;
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<(Option<u64>, unigis_core::Feature)> = out
        .features
        .into_iter()
        .map(|f| {
            let oid = u64::try_from(f.oid)
                .ok()
                .filter(|o| *o > 0 && seen.insert(*o));
            (oid, f)
        })
        .collect();
    let n = unigis_filegdb::rewrite_table(&base, &out.fields, &rows)?;
    println!("Updated {}/{item} ({n} features)", gdb.display());
    Ok(())
}

/// A dataset's full path, as ArcGIS records it in fields like `MERGE_SRC`.
fn catalog_path(p: &Path) -> String {
    std::path::absolute(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .display()
        .to_string()
}

fn join_attributes_arg(s: &str) -> Result<unigis_tools::JoinAttributes> {
    unigis_tools::JoinAttributes::parse(s)
        .ok_or_else(|| Error::Unsupported(format!("join_attributes {s}")))
}

fn validation_method_arg(s: &str) -> Result<unigis_geometry::ValidationMethod> {
    match s.to_ascii_uppercase().as_str() {
        "ESRI" => Ok(unigis_geometry::ValidationMethod::Esri),
        "OGC" => Ok(unigis_geometry::ValidationMethod::Ogc),
        other => Err(Error::Unsupported(format!("validation_method {other}"))),
    }
}

fn check_geometry_tool(in_features: &str, out_table: &Path, method: &str) -> Result<()> {
    let method = validation_method_arg(method)?;
    let paths: Vec<PathBuf> = in_features
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    let opened: Vec<Box<dyn FeatureClass>> = paths
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let inputs: Vec<unigis_tools::CheckInput> = paths
        .iter()
        .zip(&opened)
        .map(|(p, f)| unigis_tools::CheckInput {
            source: catalog_path(p),
            features: f.as_ref(),
            has_spatial_index: match locate(p) {
                Location::Shapefile(shp) => shp.with_extension("sbn").exists(),
                Location::Geodatabase { .. } => true,
            },
        })
        .collect();
    let (rows, messages) = unigis_tools::check_geometry(&inputs, method)?;
    for m in &messages {
        eprintln!("{m}");
    }
    if out_table
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("dbf"))
    {
        return Err(Error::Unsupported(
            "Check Geometry writes a .dbf table; geodatabase tables are not implemented yet".into(),
        ));
    }
    let fields = unigis_tools::check_geometry_fields();
    let dbf_fields = fields
        .iter()
        .map(|f| unigis_shapefile::dbf::DbfField::from_field(f.name.clone(), f))
        .collect::<Result<Vec<_>>>()?;
    let mut w = unigis_shapefile::dbf::DbfWriter::create(
        out_table,
        dbf_fields,
        chrono::Local::now().date_naive(),
    )?;
    for r in &rows {
        w.push(&[
            Value::String(r.class.clone()),
            Value::Integer(r.feature_id as i32),
            Value::String(r.problem.clone()),
        ])?;
    }
    w.finish()?;
    let cpg = out_table.with_extension("cpg");
    std::fs::write(&cpg, "UTF-8").map_err(|e| Error::io(&cpg, e))?;
    println!("Wrote {} ({} problems)", out_table.display(), rows.len());
    Ok(())
}

/// Writes a table as a `.dbf` (with `.cpg`), field names by the shapefile rules.
fn write_table(table: &unigis_tools::TableOutput, path: &Path) -> Result<()> {
    if let Some((gpkg, item)) = geopackage_item(path) {
        let name = item
            .ok_or_else(|| Error::Unsupported("name the output table inside the .gpkg".into()))?;
        let name = name.strip_prefix("main.").unwrap_or(&name).to_string();
        let features: Vec<unigis_core::Feature> = table
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| unigis_core::Feature {
                oid: i as i64 + 1,
                geometry: None,
                attributes: row.clone(),
            })
            .collect();
        unigis_gpkg::write::write_table(
            &gpkg,
            &unigis_gpkg::write::NewTable {
                name: &name,
                geometry: None,
                fields: &table.fields,
                spatial_ref: None,
                xy_domain: None,
                features: &features,
            },
        )?;
        return Ok(());
    }
    if let Location::Geodatabase { gdb, item } = locate(path) {
        let name =
            item.ok_or_else(|| Error::Unsupported("name the output table inside the .gdb".into()))?;
        let names = geodatabase_field_names(&table.fields);
        let fields: Vec<Field> = table
            .fields
            .iter()
            .zip(&names)
            .map(|(f, n)| Field {
                name: n.clone(),
                ..f.clone()
            })
            .collect();
        let mut w = open_geodatabase_writer(&gdb)?;
        let mut t = w.create_table(&name, None, false, false, &fields, None)?;
        for (i, row) in table.rows.iter().enumerate() {
            t.write(&Feature {
                oid: i as i64 + 1,
                geometry: None,
                attributes: row.clone(),
            })?;
        }
        w.finish_table(t)?;
        w.finish()?;
        return Ok(());
    }
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("dbf"))
    {
        return Err(Error::Unsupported(
            "tables are written as .dbf, in a GeoPackage or in a new geodatabase".into(),
        ));
    }
    let names =
        unigis_shapefile::validate_field_names(table.fields.iter().map(|f| f.name.as_str()));
    let dbf_fields = table
        .fields
        .iter()
        .zip(names)
        .map(|(f, n)| unigis_shapefile::dbf::DbfField::from_field(n, f))
        .collect::<Result<Vec<_>>>()?;
    let mut w = unigis_shapefile::dbf::DbfWriter::create(
        path,
        dbf_fields,
        chrono::Local::now().date_naive(),
    )?;
    for row in &table.rows {
        w.push(row)?;
    }
    w.finish()?;
    let cpg = path.with_extension("cpg");
    std::fs::write(&cpg, "UTF-8").map_err(|e| Error::io(&cpg, e))?;
    println!("Wrote {} ({} rows)", path.display(), table.rows.len());
    Ok(())
}

/// Allocate all names before writing. Existing files (including sidecars) are
/// reserved case-insensitively, matching ArcGIS's unique-name suffix starting at0.
fn write_workspace_outputs(
    workspace: &Path,
    outputs: Vec<(String, unigis_tools::registry::WorkspaceOutput)>,
) -> Result<()> {
    use unigis_tools::registry::WorkspaceOutput;
    if !workspace.is_dir() {
        return Err(Error::Unsupported(
            "ERROR 000732: Target Workspace does not exist or is not a folder".into(),
        ));
    }
    if matches!(locate(workspace), Location::Geodatabase { .. }) {
        return Err(Error::Unsupported(
            "geodatabase split workspaces are not yet supported; use a folder".into(),
        ));
    }
    let mut reserved = std::collections::HashSet::new();
    for entry in std::fs::read_dir(workspace).map_err(|e| Error::io(workspace, e))? {
        let entry = entry.map_err(|e| Error::io(workspace, e))?;
        let file = entry.file_name().to_string_lossy().to_string();
        reserved.insert(file.split('.').next().unwrap_or(&file).to_ascii_lowercase());
    }
    let mut planned = Vec::new();
    for (name, data) in outputs {
        // A malicious custom host/tool result cannot escape the target workspace.
        if name.is_empty()
            || !name.is_ascii()
            || name.len() > 128
            || name.contains(['/', '\\', '.', ':', '*', '?', '"', '<', '>', '|'])
            || name.chars().any(|c| c.is_ascii_control())
        {
            return Err(Error::Unsupported("invalid workspace output name".into()));
        }
        let mut candidate = name.clone();
        let mut suffix = 0;
        while reserved.contains(&candidate.to_ascii_lowercase()) {
            candidate = format!("{name}{suffix}");
            suffix += 1;
        }
        reserved.insert(candidate.to_ascii_lowercase());
        let extension = match &data {
            WorkspaceOutput::Features(_) => "shp",
            WorkspaceOutput::Table(_) => "dbf",
        };
        planned.push((workspace.join(format!("{candidate}.{extension}")), data));
    }
    for (path, data) in planned {
        match data {
            WorkspaceOutput::Features(out) => write_output_as(out, &path, None, false)?,
            WorkspaceOutput::Table(out) => write_table(&out, &path)?,
        }
    }
    Ok(())
}

fn split_tool(
    input: &Path,
    split_features: &Path,
    split_field: &str,
    out_workspace: &Path,
) -> Result<()> {
    if matches!(locate(out_workspace), Location::Geodatabase { .. }) {
        return Err(Error::Unsupported(
            "splitting into a geodatabase needs adding to an existing one (not implemented yet); use a folder".into(),
        ));
    }
    if !out_workspace.is_dir() {
        return Err(Error::Unsupported(format!(
            "ERROR 000732: Target Workspace: Dataset {} does not exist or is not supported",
            out_workspace.display()
        )));
    }
    let (fc, _) = open_feature_class(input)?;
    let (sf, _) = open_feature_class(split_features)?;
    for (value, out) in unigis_tools::split(fc.as_ref(), sf.as_ref(), split_field)? {
        let name: String = value
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
            .collect();
        write_output(out, &out_workspace.join(format!("{name}.shp")), None)?;
    }
    Ok(())
}

fn merge_tool(
    inputs: &[PathBuf],
    output: &Path,
    add_source: &str,
    field_mapping: Option<&str>,
) -> Result<()> {
    let add = match add_source.to_ascii_uppercase().as_str() {
        "NO_SOURCE_INFO" => false,
        "ADD_SOURCE_INFO" => true,
        other => return Err(Error::Unsupported(format!("add_source {other}"))),
    };
    let opened: Vec<Box<dyn FeatureClass>> = inputs
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let merge_inputs: Vec<unigis_tools::PathInput> = inputs
        .iter()
        .zip(&opened)
        .map(|(p, f)| unigis_tools::PathInput {
            source: catalog_path(p),
            features: f.as_ref(),
        })
        .collect();
    let field_map = field_mapping
        .map(unigis_tools::parse_field_mapping)
        .transpose()?;
    let out = unigis_tools::merge_with(&merge_inputs, add, field_map.as_deref())?;
    write_output(out, output, source_gdb(&inputs[0]).as_deref())
}

/// Reads the source and cost rasters of a cost distance tool and runs it.
fn run_cost(
    source: &Path,
    cost: &Path,
    max_distance: Option<f64>,
) -> Result<
    (
        unigis_raster::Raster,
        unigis_raster::Raster,
        unigis_raster::cost::CostResult,
    ),
    Error,
> {
    let source = unigis_raster::read_geotiff(source)?;
    let cost = unigis_raster::read_geotiff(cost)?;
    let result =
        unigis_raster::cost::cost_distance(&source, &cost, max_distance).ok_or_else(|| {
            Error::Unsupported("the source and cost rasters must share a grid".into())
        })?;
    if result.masked_sources > 0 {
        eprintln!(
            "WARNING 010780: Some sources are located on masked cells and will not be processed."
        );
    }
    Ok((source, cost, result))
}

/// A raster's value field as its attribute table (`<raster>.vat.dbf`) spells it, else as given.
fn raster_field_name(raster: &Path, field: &str) -> String {
    let vat = PathBuf::from(format!("{}.vat.dbf", raster.display()));
    open_feature_class(&vat)
        .ok()
        .and_then(|(fc, _)| {
            fc.fields()
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(field))
                .map(|f| f.name.clone())
        })
        .unwrap_or_else(|| field.to_owned())
}

/// The split policy keyword of Create Domain / Alter Domain.
fn domain_split_policy(
    keyword: Option<&str>,
) -> Result<Option<unigis_filegdb::SplitPolicy>, Error> {
    Ok(match keyword.map(str::to_ascii_uppercase).as_deref() {
        None | Some("") | Some("#") => None,
        Some("DEFAULT") => Some(unigis_filegdb::SplitPolicy::DefaultValue),
        Some("DUPLICATE") => Some(unigis_filegdb::SplitPolicy::Duplicate),
        Some("GEOMETRY_RATIO") => Some(unigis_filegdb::SplitPolicy::GeometryRatio),
        Some(other) => return Err(Error::Unsupported(format!("split policy {other}"))),
    })
}

/// The merge policy keyword of Create Domain / Alter Domain.
fn domain_merge_policy(
    keyword: Option<&str>,
) -> Result<Option<unigis_filegdb::MergePolicy>, Error> {
    Ok(match keyword.map(str::to_ascii_uppercase).as_deref() {
        None | Some("") | Some("#") => None,
        Some("DEFAULT") => Some(unigis_filegdb::MergePolicy::DefaultValue),
        Some("SUM_VALUES") => Some(unigis_filegdb::MergePolicy::SumValues),
        Some("AREA_WEIGHTED") => Some(unigis_filegdb::MergePolicy::AreaWeighted),
        Some(other) => return Err(Error::Unsupported(format!("merge policy {other}"))),
    })
}

/// The field type keyword of Alter Field.
fn alter_field_type(keyword: Option<&str>) -> Result<Option<FieldType>, Error> {
    Ok(
        match keyword
            .map(str::trim)
            .filter(|t| !t.is_empty() && *t != "#")
            .map(str::to_ascii_uppercase)
            .as_deref()
        {
            None => None,
            Some("TEXT") => Some(FieldType::String),
            Some("FLOAT") => Some(FieldType::Single),
            Some("DOUBLE") => Some(FieldType::Double),
            Some("SHORT") => Some(FieldType::SmallInteger),
            Some("LONG") => Some(FieldType::Integer),
            Some("BIGINTEGER") => Some(FieldType::BigInteger),
            Some("DATE") => Some(FieldType::Date),
            Some("DATEONLY") => Some(FieldType::DateOnly),
            Some("TIMEONLY") => Some(FieldType::TimeOnly),
            Some("TIMESTAMPOFFSET") => Some(FieldType::TimestampOffset),
            Some("BLOB") => Some(FieldType::Blob),
            Some("GUID") => Some(FieldType::Guid),
            Some(other) => return Err(Error::Unsupported(format!("field type {other}"))),
        },
    )
}

/// `NULLABLE` / `NON_NULLABLE` of Alter Field.
fn alter_nullable(keyword: Option<&str>) -> Result<Option<bool>, Error> {
    Ok(match keyword.map(str::to_ascii_uppercase).as_deref() {
        None | Some("") | Some("#") => None,
        Some("NULLABLE") => Some(true),
        Some("NON_NULLABLE") => Some(false),
        Some(other) => return Err(Error::Unsupported(format!("nullable {other}"))),
    })
}

fn dissolve_tool(
    input: &Path,
    output: &Path,
    fields: &[String],
    statistics: Option<&str>,
    multi_part: &str,
) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let mut stats = Vec::new();
    for item in statistics
        .unwrap_or("")
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let mut words = item.split_whitespace();
        let (Some(field), Some(stat)) = (words.next(), words.next()) else {
            return Err(Error::Unsupported(format!(
                "statistic `{item}`: expected \"FIELD STATISTIC\""
            )));
        };
        let stat = unigis_tools::Statistic::parse(stat)
            .ok_or_else(|| Error::Unsupported(format!("statistic type {stat}")))?;
        stats.push((field.to_owned(), stat));
    }
    let multi = match multi_part.to_ascii_uppercase().as_str() {
        "MULTI_PART" => true,
        "SINGLE_PART" => false,
        other => return Err(Error::Unsupported(format!("multi_part {other}"))),
    };
    let out = unigis_tools::dissolve(fc.as_ref(), fields, &stats, multi)?;
    write_output(out, output, source_gdb(input).as_deref())
}

/// Parses an ArcGIS linear distance ("25", "25 Meters", "0.5 Kilometers") into the
/// units of `wkt` (the last `UNIT[...]` of a projected system).
fn near_method(text: &str) -> Result<unigis_tools::NearMethod> {
    unigis_tools::NearMethod::parse(text)
        .ok_or_else(|| Error::Unsupported(format!("method `{text}`: expected PLANAR or GEODESIC")))
}

/// A Near search radius: in the data's unit when planar, in metres (from a linear unit) when geodesic.
fn near_radius(text: &str, method: unigis_tools::NearMethod, wkt: Option<&str>) -> Result<f64> {
    match method {
        unigis_tools::NearMethod::Planar => parse_distance(text, wkt),
        unigis_tools::NearMethod::Geodesic => parse_distance(text, None),
    }
}

fn parse_distance(text: &str, wkt: Option<&str>) -> Result<f64> {
    let mut words = text.split_whitespace();
    let value: f64 = words
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Error::Unsupported(format!("invalid distance `{text}`")))?;
    let Some(unit) = words.next() else {
        return Ok(value);
    };
    let meters_per = |u: &str| -> Option<f64> {
        Some(match u.to_ascii_lowercase().trim_end_matches('s') {
            "meter" | "metre" => 1.0,
            "kilometer" | "kilometre" => 1000.0,
            "centimeter" => 0.01,
            "millimeter" => 0.001,
            "feet" | "foot" | "feetint" => 0.3048,
            "ussurveyfeet" | "foot_us" => 1200.0 / 3937.0,
            "yard" => 0.9144,
            "mile" | "milesint" => 1609.344,
            "nauticalmile" => 1852.0,
            _ => return None,
        })
    };
    let from = meters_per(unit)
        .ok_or_else(|| Error::Unsupported(format!("unknown distance unit `{unit}`")))?;
    // The data's linear unit: UNIT["Meter",1.0] at the end of a PROJCS.
    let data = wkt
        .and_then(|w| w.rsplit("UNIT[").next())
        .and_then(|u| u.split(',').nth(1))
        .and_then(|f| f.trim_end_matches([']', ' ']).parse::<f64>().ok())
        .unwrap_or(1.0);
    Ok(value * from / data)
}

/// A buffer distance. On geographic data a distance in a linear unit is measured on the ellipsoid
/// (geodesic); a plain number, or one in degrees, is planar in degrees, as ArcGIS does.
fn buffer_distance(text: &str, wkt: Option<&str>) -> Result<unigis_tools::BufferDistance> {
    let geographic = wkt.is_some_and(|w| w.trim_start().starts_with("GEOGCS"));
    let mut words = text.split_whitespace();
    let value = words.next().and_then(|v| v.parse::<f64>().ok());
    let unit = words.next().map(str::to_ascii_lowercase);
    match (geographic, value, unit.as_deref()) {
        (true, Some(v), Some("decimaldegrees" | "degrees" | "degree")) => {
            Ok(unigis_tools::BufferDistance::Linear(v))
        }
        (true, Some(_), Some(_)) => {
            // Metres from the unit, with a data unit of 1.
            Ok(unigis_tools::BufferDistance::Geodesic(parse_distance(
                text, None,
            )?))
        }
        _ => Ok(unigis_tools::BufferDistance::Linear(parse_distance(
            text, wkt,
        )?)),
    }
}

/// Merges the features that have the same attribute values (the pieces of one pair of input
/// features) into one, in the order of their first piece.
fn merge_pieces_with_equal_attributes(out: &mut ToolOutput) -> Result<()> {
    if !matches!(
        out.kind,
        unigis_core::GeometryKind::Polygon | unigis_core::GeometryKind::Polyline
    ) {
        return Ok(());
    }
    let mut groups: Vec<(String, Vec<Feature>)> = Vec::new();
    for f in std::mem::take(&mut out.features) {
        let key = format!("{:?}", f.attributes);
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.1.push(f),
            None => groups.push((key, vec![f])),
        }
    }
    for (_, pieces) in groups {
        if pieces.len() == 1 && out.kind == unigis_core::GeometryKind::Polygon {
            out.features.extend(pieces);
            continue;
        }
        let geometry =
            unigis_geometry::dissolve(pieces.iter().filter_map(|f| f.geometry.as_ref()))?;
        let mut first = pieces.into_iter().next().expect("a group has a piece");
        first.geometry = geometry.or(first.geometry);
        out.features.push(first);
    }
    Ok(())
}

fn buffer_tool(
    input: &Path,
    output: &Path,
    distance: &str,
    line_side: &str,
    line_end: &str,
    dissolve: &str,
    dissolve_fields: Vec<String>,
) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    // A number (with or without a unit) is a distance; anything else a field name.
    let distance = if distance
        .split_whitespace()
        .next()
        .is_some_and(|v| v.parse::<f64>().is_ok())
    {
        buffer_distance(distance, fc.spatial_reference_wkt())?
    } else {
        unigis_tools::BufferDistance::Field(distance.trim().to_owned())
    };
    let side = match line_side.to_ascii_uppercase().as_str() {
        "FULL" => unigis_geometry::LineSide::Full,
        "LEFT" => unigis_geometry::LineSide::Left,
        "RIGHT" => unigis_geometry::LineSide::Right,
        "OUTSIDE_ONLY" => unigis_geometry::LineSide::OutsideOnly,
        other => return Err(Error::Unsupported(format!("line_side {other}"))),
    };
    let line_end = match line_end.to_ascii_uppercase().as_str() {
        "ROUND" => unigis_geometry::LineEnd::Round,
        "FLAT" => unigis_geometry::LineEnd::Flat,
        other => return Err(Error::Unsupported(format!("line_end_type {other}"))),
    };
    let dissolve = match dissolve.to_ascii_uppercase().as_str() {
        "NONE" => Dissolve::None,
        "ALL" => Dissolve::All,
        "LIST" if !dissolve_fields.is_empty() => Dissolve::List(dissolve_fields),
        "LIST" => {
            return Err(Error::Unsupported(
                "dissolve_option LIST needs --dissolve-field".into(),
            ));
        }
        other => return Err(Error::Unsupported(format!("dissolve_option {other}"))),
    };
    // A geodatabase keeps the round parts as true arcs, as ArcGIS does.
    let out = if matches!(locate(output), Location::Geodatabase { .. }) {
        unigis_tools::buffer_arcs(fc.as_ref(), distance, line_end, side, dissolve)?
    } else {
        unigis_tools::buffer(fc.as_ref(), distance, line_end, side, dissolve)?
    };
    write_output(out, output, source_gdb(input).as_deref())
}

/// Writes a raster result as GeoTIFF the way ArcGIS writes Spatial Analyst
/// output: 128×128 tiles, LZW for integer cells, uncompressed floats.
/// Spatial Analyst Expand (`grow`) or Shrink.
fn expand_shrink(
    grow: bool,
    in_raster: &Path,
    out_raster: &Path,
    number_cells: f64,
    zone_values: &str,
    method: &str,
) -> Result<()> {
    use unigis_raster::generalize::{Method, expand, number_of_cells, shrink};
    let method =
        Method::parse(method).ok_or_else(|| Error::Unsupported(format!("method {method}")))?;
    let n = number_of_cells(number_cells).map_err(|e| Error::Unsupported(e.0))?;
    let zones = zone_values
        .split(';')
        .map(str::trim)
        .filter(|z| !z.is_empty())
        .map(|z| {
            z.parse::<f64>()
                .map_err(|_| Error::Unsupported(format!("zone value {z}")))
        })
        .collect::<Result<Vec<f64>>>()?;
    let r = unigis_raster::read_geotiff(in_raster)?;
    let out = if grow {
        expand(&r, n, &zones, method)
    } else {
        shrink(&r, n, &zones, method)
    }
    .map_err(|e| Error::Unsupported(e.0))?;
    if method == Method::Distance && grow {
        // ArcGIS's statistics include the workspace extended n cells to the
        // right and bottom, before cropping pixels to the original extent.
        let width = r
            .width
            .checked_add(n)
            .ok_or_else(|| Error::Unsupported("Expand statistics grid is too large".into()))?;
        let height = r
            .height
            .checked_add(n)
            .ok_or_else(|| Error::Unsupported("Expand statistics grid is too large".into()))?;
        if width
            .checked_mul(height)
            .is_none_or(|v| v as u64 > unigis_raster::MAX_CELLS)
        {
            return Err(Error::Unsupported(
                "Expand statistics grid is too large".into(),
            ));
        }
        let mut padded = unigis_raster::Raster::new(width, height, r.data_type, r.transform);
        padded.epsg = r.epsg;
        padded.nodata = Some(r.nodata.unwrap_or(f64::NAN));
        padded.bands[0].fill(padded.nodata.unwrap_or(f64::NAN));
        for row in 0..r.height {
            let start = row * width;
            padded.bands[0][start..start + r.width]
                .copy_from_slice(&r.bands[0][row * r.width..(row + 1) * r.width]);
        }
        let expanded = expand(&padded, n, &zones, method).map_err(|e| Error::Unsupported(e.0))?;
        write_raster_with_metadata(
            out_raster,
            &out,
            false,
            unigis_raster::stats_metadata::for_raster(&expanded),
        )
    } else if method == Method::Distance {
        write_raster_plain(out_raster, &out)
    } else {
        write_raster(out_raster, &out)
    }
}

/// FOUR or EIGHT neighbors.
fn eight_neighbors(s: &str) -> Result<bool> {
    match s.to_ascii_uppercase().as_str() {
        "FOUR" => Ok(false),
        "EIGHT" => Ok(true),
        other => Err(Error::Unsupported(format!("number of neighbors {other}"))),
    }
}

/// `ug run` for a tool of the raster registry: `name=value` pairs, rasters as GeoTIFF paths.
fn run_raster_tool(spec: &unigis_raster::registry::Tool, args: &[String]) -> Result<()> {
    use unigis_raster::registry;
    use unigis_raster::registry::Context as _;
    let mut map = std::collections::HashMap::new();
    for a in args {
        let (k, v) = a
            .split_once('=')
            .ok_or_else(|| Error::Unsupported(format!("'{a}': expected name=value")))?;
        if k != "output" && !spec.params.iter().any(|p| p.name == k) {
            return Err(Error::Unsupported(format!(
                "{}: no parameter '{k}'",
                spec.id
            )));
        }
        map.insert(k.to_string(), v.to_string());
    }
    let output = map.get("output").cloned();
    struct Args(std::collections::HashMap<String, String>);
    impl registry::Context for Args {
        fn get(&self, name: &str) -> Option<String> {
            self.0
                .get(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        }
        fn raster(&self, name: &str) -> Result<unigis_raster::Raster> {
            let path = self
                .get(name)
                .ok_or_else(|| Error::Unsupported(format!("{name} is required")))?;
            unigis_raster::read_geotiff(Path::new(&path))
        }
        fn raster_at(&self, path: &str) -> Result<unigis_raster::Raster> {
            unigis_raster::read_geotiff(Path::new(path))
        }
        fn line_values(
            &self,
            name: &str,
            field: &str,
        ) -> Result<(Vec<(Vec<Vec<[f64; 2]>>, f64)>, bool, Option<u32>)> {
            use unigis_raster::rasterize::ValueKind;
            let path = self
                .get(name)
                .ok_or_else(|| Error::Unsupported(format!("{name} is required")))?;
            let (features, kind, epsg, _) = rasterize_input(Path::new(&path), field)?;
            let lines = features
                .iter()
                .filter(|(g, _)| g.kind == unigis_core::GeometryKind::Polyline)
                .map(|(g, v)| {
                    let parts = g.part_ranges().map(|r| g.xy[r].to_vec()).collect();
                    (parts, *v)
                })
                .collect();
            Ok((lines, matches!(kind, ValueKind::Integer), epsg))
        }
        fn point_samples(
            &self,
            name: &str,
            field: &str,
        ) -> Result<(Vec<unigis_raster::interpolate::Sample>, Option<u32>)> {
            let path = self
                .get(name)
                .ok_or_else(|| Error::Unsupported(format!("{name} is required")))?;
            let (features, _, epsg, _) = rasterize_input(Path::new(&path), field)?;
            let mut samples = Vec::new();
            for (g, z) in &features {
                if matches!(
                    g.kind,
                    unigis_core::GeometryKind::Point | unigis_core::GeometryKind::Multipoint
                ) {
                    samples.extend(g.xy.iter().map(|c| unigis_raster::interpolate::Sample {
                        x: c[0],
                        y: c[1],
                        z: *z,
                    }));
                }
            }
            Ok((samples, epsg))
        }
        fn multipatches(&self, name: &str) -> Result<(Vec<unigis_core::Geometry>, Option<u32>)> {
            let path = self
                .get(name)
                .ok_or_else(|| Error::Unsupported(format!("{name} is required")))?;
            let (fc, _) = open_feature_class(Path::new(&path))?;
            let mut geometries = Vec::new();
            for feature in fc.features() {
                if let Some(g) = feature?.geometry
                    && g.kind == unigis_core::GeometryKind::MultiPatch
                {
                    geometries.push(g);
                }
            }
            let epsg = fc
                .spatial_reference_wkt()
                .and_then(|w| unigis_proj::Crs::from_wkt(w).ok())
                .and_then(|c| c.wkid())
                .map(|(_, latest)| latest);
            Ok((geometries, epsg))
        }
        fn attribute_table(&self, name: &str) -> Result<Vec<(String, Vec<f64>)>> {
            let path = self
                .get(name)
                .ok_or_else(|| Error::Unsupported(format!("{name} is required")))?;
            let (table, _) = open_feature_class(Path::new(&format!("{path}.vat.dbf")))?;
            let mut columns: Vec<(String, Vec<f64>)> = table
                .fields()
                .iter()
                .map(|f| (f.name.clone(), Vec::new()))
                .collect();
            for f in table.features() {
                let f = f?;
                for (column, value) in columns.iter_mut().zip(&f.attributes) {
                    column.1.push(match value {
                        Value::SmallInteger(n) => f64::from(*n),
                        Value::Integer(n) => f64::from(*n),
                        Value::Single(n) => f64::from(*n),
                        Value::Double(n) => *n,
                        _ => f64::NAN,
                    });
                }
            }
            Ok(columns)
        }
    }
    if spec.id == "raster_to_other_format" {
        return run_raster_to_other_format(&Args(map));
    }
    let output = output.ok_or_else(|| Error::Unsupported("output=<path> is required".into()))?;
    if spec.id == "raster_to_polyline" {
        return run_raster_to_polyline(&Args(map), &output);
    }
    if spec.id == "sample" {
        return run_sample(&Args(map), &output);
    }
    if spec.id == "extract_multi_values_to_points" {
        return run_extract_multi_values(&Args(map), &output);
    }
    if spec.id == "interpolate_shape" {
        return run_interpolate_shape(&Args(map), &output);
    }
    if spec.id == "locate_depressions" {
        return run_locate_depressions(&Args(map), &output);
    }
    if spec.id == "add_surface_information" {
        return run_add_surface_information(&Args(map), &output);
    }
    if spec.id == "contour_list" {
        let r = unigis_raster::read_geotiff(Path::new(
            map.get("input")
                .ok_or_else(|| Error::Unsupported("input is required".into()))?,
        ))?;
        let levels = map
            .get("contour_values")
            .map(String::as_str)
            .unwrap_or_default()
            .split([';', ',', ' '])
            .filter(|v| !v.trim().is_empty())
            .map(|v| {
                v.trim().parse::<f64>().map_err(|_| {
                    Error::Unsupported(format!("ERROR 000800: contour value {v} is not a number"))
                })
            })
            .collect::<Result<Vec<f64>>>()?;
        if levels.is_empty() {
            return Err(Error::Unsupported(
                "ERROR 000735: Contour values: value is required".into(),
            ));
        }
        let lines = unigis_raster::contour::contour_list(&r, &levels);
        return write_contours(lines, &r, Path::new(&output));
    }
    if spec.id == "validate_flow_direction" {
        if map
            .get("flow_direction_type")
            .is_some_and(|t| !t.eq_ignore_ascii_case("D8"))
        {
            return Err(Error::Unsupported(
                "flow direction type MFD is not supported yet".into(),
            ));
        }
        let directions = unigis_raster::read_geotiff(Path::new(
            map.get("input")
                .ok_or_else(|| Error::Unsupported("input is required".into()))?,
        ))?;
        if directions.data_type.is_float() {
            return Err(Error::Unsupported(
                "ERROR 000969: The input raster is not integer type.".into(),
            ));
        }
        let cells = unigis_raster::hydro::flow_direction_loops(&directions);
        if cells.is_empty() {
            // ArcGIS writes no feature class in this case.
            println!("Flow Direction is valid. No loops found.");
            return Ok(());
        }
        let t = directions.transform;
        let loops = cells.last().map_or(0, |c| c.0);
        let features = cells
            .into_iter()
            .enumerate()
            .map(|(i, (id, code, col, row))| Feature {
                oid: i as i64,
                geometry: Some(unigis_core::Geometry::point(
                    t[0] + (col as f64 + 0.5) * t[1],
                    t[3] + (row as f64 + 0.5) * t[5],
                )),
                attributes: vec![
                    Value::Integer(id as i32),
                    Value::Integer(code as i32),
                    Value::Integer(col as i32),
                    Value::Integer(row as i32),
                ],
            })
            .collect();
        let field = |name: &str| {
            let mut f = Field::new(name, FieldType::Integer);
            f.length = 10;
            f
        };
        let out = ToolOutput {
            kind: unigis_core::GeometryKind::Point,
            has_z: false,
            has_m: false,
            fields: vec![
                field("LoopID"),
                field("FlowDir"),
                field("Col"),
                field("Row"),
            ],
            spatial_reference_wkt: raster_wkt(&directions)?,
            features,
            messages: vec![format!("{loops} loops found.")],
        };
        return write_output(out, Path::new(&output), None);
    }
    if spec.id == "stream_to_feature" {
        let read = |k: &str| -> Result<unigis_raster::Raster> {
            unigis_raster::read_geotiff(Path::new(
                map.get(k)
                    .ok_or_else(|| Error::Unsupported(format!("{k} is required")))?,
            ))
        };
        let (streams, directions) = (read("in_stream_raster")?, read("in_flow_direction_raster")?);
        let simplify = !map.get("simplify").is_some_and(|v| {
            v.eq_ignore_ascii_case("NO_SIMPLIFY") || v.eq_ignore_ascii_case("false")
        });
        let arcs = unigis_raster::hydro::stream_to_feature(&streams, &directions, simplify);
        let features = arcs
            .into_iter()
            .enumerate()
            .map(|(i, a)| {
                let mut g = unigis_core::Geometry::point(0.0, 0.0);
                g.kind = unigis_core::GeometryKind::Polyline;
                g.parts = vec![0];
                g.xy = a.points;
                Feature {
                    oid: i as i64,
                    geometry: Some(g),
                    attributes: vec![
                        Value::Integer(i as i32 + 1),
                        Value::Integer(a.grid_code as i32),
                        Value::Integer(a.from_node as i32),
                        Value::Integer(a.to_node as i32),
                    ],
                }
            })
            .collect();
        let field = |name: &str| {
            let mut f = Field::new(name, FieldType::Integer);
            f.length = 10;
            f
        };
        let out = ToolOutput {
            kind: unigis_core::GeometryKind::Polyline,
            has_z: false,
            has_m: false,
            fields: vec![
                field("arcid"),
                field("grid_code"),
                field("from_node"),
                field("to_node"),
            ],
            spatial_reference_wkt: raster_wkt(&directions)?,
            features,
            messages: Vec::new(),
        };
        return write_output(out, Path::new(&output), None);
    }
    if spec.id == "storage_capacity" {
        if map.contains_key("in_zone_data") {
            return Err(Error::Unsupported(
                "Storage Capacity zones are not supported yet".into(),
            ));
        }
        let surface = unigis_raster::read_geotiff(Path::new(
            map.get("input")
                .ok_or_else(|| Error::Unsupported("input is required".into()))?,
        ))?;
        let number = |k: &str| map.get(k).and_then(|v| v.trim().parse::<f64>().ok());
        let analysis = map
            .get("analysis_type")
            .map_or("AREA_VOLUME".into(), |v| v.to_ascii_uppercase());
        let by_value = map
            .get("increment_type")
            .is_some_and(|v| v.eq_ignore_ascii_case("VALUE_OF_INCREMENT"));
        let options = unigis_raster::zonal_tables::StorageCapacity {
            area: analysis != "VOLUME",
            volume: analysis != "AREA",
            min: number("min_elevation"),
            max: number("max_elevation"),
            increments: if by_value {
                Err(number("increment").ok_or_else(|| {
                    Error::Unsupported("ERROR 000735: Increment: value is required".into())
                })?)
            } else {
                Ok(number("increment").map_or(10, |n| n as usize))
            },
            z_metres: unigis_raster::registry::z_unit_metres(
                map.get("z_unit").map_or("METER", String::as_str),
            )?,
        };
        let (fields, rows) =
            unigis_raster::zonal_tables::storage_capacity_table(&surface, &options)?;
        return write_table(
            &unigis_tools::TableOutput { fields, rows },
            Path::new(&output),
        );
    }
    if spec.id == "zonal_geometry_as_table" {
        if let Some(field) = map.get("zone_field")
            && !field.eq_ignore_ascii_case("Value")
        {
            return Err(Error::Unsupported(format!(
                "zone field {field}: only Value is supported yet"
            )));
        }
        let zones = unigis_raster::read_geotiff(Path::new(
            map.get("input")
                .ok_or_else(|| Error::Unsupported("input is required".into()))?,
        ))?;
        let (fields, rows) = unigis_raster::zonal::zonal_geometry_table(&zones)?;
        return write_table(
            &unigis_tools::TableOutput { fields, rows },
            Path::new(&output),
        );
    }
    if spec.id == "zonal_characterization" {
        return run_zonal_characterization(&Args(map), &output);
    }
    let parameters = Args(map);
    let out = registry::run(spec, &parameters)?;
    for (name, raster) in &out.extra {
        if let Some(path) = registry::Context::get(&parameters, name) {
            write_raster_with(Path::new(&path), raster, true)?;
        }
    }
    if spec.id == "raster_to_float" {
        let path = Path::new(&output);
        std::fs::write(path, unigis_raster::conversion::to_float(&out.raster))
            .map_err(|e| Error::io(path, e))?;
        let hdr = path.with_extension("hdr");
        std::fs::write(&hdr, unigis_raster::conversion::float_header(&out.raster))
            .map_err(|e| Error::io(&hdr, e))?;
        println!("Wrote {output}");
        return Ok(());
    }
    if spec.id == "raster_to_ascii" {
        std::fs::write(&output, unigis_raster::conversion::to_ascii(&out.raster))
            .map_err(|e| Error::io(Path::new(&output), e))?;
        println!("Wrote {output}");
        return Ok(());
    }
    if spec.id == "create_binary_mask" {
        // ArcGIS embeds an "empty" statistics cache (count 0, max 255, stddev -1) in the TIFF.
        let empty: unigis_raster::stats_metadata::BandStatistics = [
            ("COUNT", "0.000000"),
            ("MINIMUM", "0"),
            ("MAXIMUM", "255"),
            ("MEAN", "0"),
            ("STDDEV", "-1"),
            ("MEDIAN", "nan"),
            ("SKIPFACTORX", "1"),
            ("SKIPFACTORY", "1"),
        ]
        .into_iter()
        .map(|(k, v)| (format!("STATISTICS_{k}"), v.to_string()))
        .collect();
        return write_raster_with_metadata(
            Path::new(&output),
            &out.raster,
            false,
            unigis_raster::stats_metadata::encode(&[empty]),
        );
    }
    if out.covariance_statistics {
        write_raster_with(Path::new(&output), &out.raster, false)?;
        unigis_raster::stats::write_aux_statistics_with_covariance(Path::new(&output), &out.raster)
    } else if out.empty_band_statistics {
        write_raster_with(Path::new(&output), &out.raster, false)?;
        unigis_raster::stats::write_aux_statistics_with_empty(Path::new(&output), &out.raster)
    } else if out.zero_count {
        write_raster_basin(Path::new(&output), &out.raster)
    } else if !out.statistics {
        let metadata = if spec.id == "copy_raster" {
            if let Some(input) = parameters.get("input") {
                let mut stored = unigis_raster::stats_metadata::read(Path::new(&input))?;
                for band in &mut stored {
                    if !band.is_empty() {
                        // ArcGIS exports zero for a missing cached count; keep
                        // the supplied moments instead of recalculating them.
                        band.entry("STATISTICS_COUNT".into())
                            .or_insert_with(|| "0.000000".into());
                        // ArcGIS treats the moments as a pair: either missing
                        // field resets both, even when min/max remain cached.
                        if !band.contains_key("STATISTICS_MEAN")
                            || !band.contains_key("STATISTICS_STDDEV")
                        {
                            band.insert("STATISTICS_MEAN".into(), "0".into());
                            band.insert("STATISTICS_STDDEV".into(), "-1".into());
                        }
                        for (key, value) in [
                            ("STATISTICS_MEDIAN", "nan"),
                            ("STATISTICS_EXCLUDEDVALUES", ""),
                            ("STATISTICS_SKIPFACTORX", "1"),
                            ("STATISTICS_SKIPFACTORY", "1"),
                        ] {
                            band.entry(key.into()).or_insert_with(|| value.into());
                        }
                    }
                }
                unigis_raster::stats_metadata::encode(&stored)
            } else {
                None
            }
        } else {
            unigis_raster::stats_metadata::for_raster(&out.raster)
        };
        write_raster_with_tags(
            Path::new(&output),
            &out.raster,
            false,
            metadata,
            out.nodata_tag,
        )
    } else {
        write_raster_with_tags(Path::new(&output), &out.raster, true, None, out.nodata_tag)
    }
}

fn run_raster_to_polyline(ctx: &dyn unigis_raster::registry::Context, output: &str) -> Result<()> {
    let r = ctx.raster("input")?;
    if r.data_type.is_float() {
        return Err(Error::Unsupported(
            "ERROR 000864: Input raster: The input is not within the defined domain.\r\nERROR 000969: The input raster is not integer type.\nERROR 000864: Field: The input is not within the defined domain.\r\nERROR 000889: Invalid field type."
                .into(),
        ));
    }
    let include_zero = match ctx.get("background_value").as_deref() {
        None | Some("ZERO") => false,
        Some("NODATA") => true,
        Some(other) => return Err(Error::Unsupported(format!("background value {other}"))),
    };
    if let Some(length) = ctx.get("minimum_dangle_length") {
        length.parse::<f64>().map_err(|_| {
            Error::Unsupported(format!("minimum dangle length {length} is not numeric"))
        })?;
    }
    if ctx
        .get("simplify")
        .is_some_and(|s| !["SIMPLIFY", "NO_SIMPLIFY"].contains(&s.as_str()))
    {
        return Err(Error::Unsupported(
            "simplify must be SIMPLIFY or NO_SIMPLIFY".into(),
        ));
    }
    if ctx
        .get("raster_field")
        .is_some_and(|f| !f.eq_ignore_ascii_case("Value"))
    {
        return Err(Error::Unsupported(
            "only the Value raster field is supported".into(),
        ));
    }
    let features = unigis_raster::conversion::raster_to_polylines(&r, include_zero)
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let mut g = unigis_core::Geometry::point(0.0, 0.0);
            g.kind = unigis_core::GeometryKind::Polyline;
            g.parts = vec![0];
            g.xy = line.points;
            Feature {
                oid: i as i64,
                geometry: Some(g),
                attributes: vec![
                    Value::Integer(i as i32 + 1),
                    Value::Integer(line.value as i32),
                    Value::Integer(line.from_node as i32),
                    Value::Integer(line.to_node as i32),
                ],
            }
        })
        .collect();
    let out = ToolOutput {
        kind: unigis_core::GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: vec![
            Field::new("arcid", FieldType::Integer),
            Field::new("grid_code", FieldType::Integer),
            Field::new("from_node", FieldType::Integer),
            Field::new("to_node", FieldType::Integer),
        ],
        spatial_reference_wkt: raster_wkt(&r)?,
        features,
        messages: Vec::new(),
    };
    write_output(out, Path::new(output), None)
}

fn run_raster_to_other_format(ctx: &dyn unigis_raster::registry::Context) -> Result<()> {
    let inputs = ctx.get("input_rasters").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Input Rasters: value is required".into())
    })?;
    let workspace = ctx.get("output_workspace").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Output Workspace: value is required".into())
    })?;
    if ctx
        .get("raster_format")
        .is_some_and(|format| !format.eq_ignore_ascii_case("TIFF"))
    {
        return Err(Error::Unsupported(
            "ERROR 000067: Raster Format: only TIFF is supported".into(),
        ));
    }
    let workspace = Path::new(&workspace);
    if !workspace.is_dir() {
        return Err(Error::Unsupported(format!(
            "ERROR 000732: Output Workspace: {} is not a folder",
            workspace.display()
        )));
    }
    for input in inputs
        .split(';')
        .map(|path| path.trim().trim_matches('\''))
        .filter(|path| !path.is_empty())
    {
        let source = Path::new(input);
        let mut raster = unigis_raster::read_geotiff(source)?;
        if raster.nodata.is_none() {
            raster.nodata = match raster.data_type {
                unigis_raster::DataType::F32 => Some(f64::from(3.4e38_f32)),
                unigis_raster::DataType::F64 => Some(1.79e308),
                _ => None,
            };
        }
        let stem = source
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| Error::Unsupported(format!("invalid raster name: {input}")))?;
        let mut suffix = 0;
        let output = loop {
            let name = if suffix == 0 {
                format!("{stem}.tif")
            } else {
                format!("{stem}_{suffix}.tif")
            };
            let candidate = workspace.join(name);
            if !candidate.exists() {
                break candidate;
            }
            suffix += 1;
        };
        write_raster_with(&output, &raster, true)?;
        println!("Wrote {}", output.display());
    }
    Ok(())
}

/// Sample: one row per point with its id, X, Y and the value of each raster. Values are rounded
/// to three decimals; a point outside a raster (or on NoData) gets the raster's NoData value, or
/// 0 for a raster without one (-9999 for an integer raster with one). A point on a cell
/// edge takes the cell to its left / above.
fn run_sample(ctx: &dyn unigis_raster::registry::Context, output: &str) -> Result<()> {
    use unigis_core::{Field, FieldType};
    let rasters: Vec<(String, unigis_raster::Raster)> = ctx
        .get("input")
        .unwrap_or_default()
        .split(';')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let path = p.trim();
            let stem = Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok((stem, unigis_raster::read_geotiff(Path::new(path))?))
        })
        .collect::<Result<_>>()?;
    let locations = ctx
        .get("locations")
        .ok_or_else(|| Error::Unsupported("locations is required".into()))?;
    let (fc, _) = open_feature_class(Path::new(&locations))?;
    let bilinear = ctx
        .get("resampling")
        .is_some_and(|v| v.eq_ignore_ascii_case("BILINEAR"));
    let id_field = ctx.get("unique_id_field");
    let id_index = id_field.as_ref().and_then(|name| {
        fc.fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
    });
    let truncate = |name: String| name.chars().take(10).collect::<String>();
    let stem = Path::new(&locations)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut fields = vec![
        {
            let mut f = Field::new(truncate(stem), FieldType::Integer);
            f.length = 10;
            f.precision = 10;
            f
        },
        {
            let mut f = Field::new("X", FieldType::Double);
            f.precision = 18;
            f.scale = 11;
            f
        },
        {
            let mut f = Field::new("Y", FieldType::Double);
            f.precision = 18;
            f.scale = 11;
            f
        },
    ];
    for (name, r) in &rasters {
        let (column, ty) = match r.data_type {
            unigis_raster::DataType::F32 => (format!("{name}_Band_1"), FieldType::Single),
            unigis_raster::DataType::F64 => (format!("{name}_Band_1"), FieldType::Double),
            _ => (name.clone(), FieldType::Integer),
        };
        let mut f = Field::new(truncate(column), ty);
        match ty {
            FieldType::Double => {
                f.precision = 18;
                f.scale = 11;
            }
            FieldType::Integer => {
                f.length = 10;
                f.precision = 10;
            }
            _ => {}
        }
        fields.push(f);
    }
    let mut rows = Vec::new();
    for f in fc.features() {
        let f = f?;
        let Some(g) = &f.geometry else { continue };
        for p in &g.xy {
            let id = match id_index.map(|i| &f.attributes[i]) {
                Some(Value::Integer(n)) => *n,
                Some(Value::SmallInteger(n)) => i32::from(*n),
                Some(Value::BigInteger(n)) => *n as i32,
                _ => f.oid as i32,
            };
            let mut row = vec![Value::Integer(id), Value::Double(p[0]), Value::Double(p[1])];
            for (_, r) in &rasters {
                row.push(sample_value(r, p[0], p[1], bilinear, false));
            }
            rows.push(row);
        }
    }
    write_table(
        &unigis_tools::TableOutput { fields, rows },
        Path::new(output),
    )
}

/// The value of `r` at (x, y) as a table cell (see [`run_sample`] for the rules).
fn sample_value(
    r: &unigis_raster::Raster,
    x: f64,
    y: f64,
    bilinear: bool,
    float_out: bool,
) -> Value {
    // Cell index of a coordinate: edges belong to the cell to the left / above.
    let index = |v: f64| {
        if v.fract() == 0.0 {
            v as i64 - 1
        } else {
            v.floor() as i64
        }
    };
    let t = r.transform;
    let (cx, cy) = ((x - t[0]) / t[1], (t[3] - y) / -t[5]);
    let inside = cx >= 0.0 && cy >= 0.0 && cx < r.width as f64 && cy < r.height as f64;
    let value = if !inside {
        None
    } else if bilinear {
        // Beside the outermost cell centres: a raster without NoData counts the
        // missing neighbours as 0 (0.8 of the edge value 0.2 of a cell from the
        // edge); one with NoData drops missing and NoData neighbours and
        // renormalises the weights (observed).
        let (fx, fy) = (cx - 0.5, cy - 0.5);
        let (c0, r0) = (fx.floor() as i64, fy.floor() as i64);
        let (dx, dy) = (fx - c0 as f64, fy - r0 as f64);
        let has_nodata = r.nodata.is_some();
        let at = |c: i64, rr: i64| {
            if c < 0 || rr < 0 || c >= r.width as i64 || rr >= r.height as i64 {
                (!has_nodata).then_some(0.0)
            } else {
                r.get(c as usize, rr as usize)
            }
        };
        let corners = [
            (at(c0, r0), (1.0 - dx) * (1.0 - dy)),
            (at(c0 + 1, r0), dx * (1.0 - dy)),
            (at(c0, r0 + 1), (1.0 - dx) * dy),
            (at(c0 + 1, r0 + 1), dx * dy),
        ];
        let weight: f64 = corners
            .iter()
            .filter(|(v, _)| v.is_some())
            .map(|c| c.1)
            .sum();
        (weight > 0.0).then(|| {
            corners
                .iter()
                .filter_map(|(v, w)| v.map(|v| v * w))
                .sum::<f64>()
                / weight
        })
    } else {
        let (c, rr) = (index(cx), index(cy));
        r.get(c as usize, rr as usize)
    };
    let float = r.data_type.is_float();
    // Without a NoData value a raster gives 0; with one, floats give it and integers
    // -9999 (observed).
    let missing = match (r.nodata, float) {
        (None, _) => 0.0,
        (Some(n), true) => n,
        (Some(_), false) => -9999.0,
    };
    let v = value.unwrap_or(missing);
    // A 32-bit float column keeps six significant digits (its 13-wide field holds
    // d.ddddde+xx).
    let v = if r.data_type == unigis_raster::DataType::F32 && v != 0.0 {
        format!("{v:.5e}").parse().unwrap_or(v)
    } else {
        v
    };
    match r.data_type {
        unigis_raster::DataType::F32 => Value::Single(v as f32),
        unigis_raster::DataType::F64 => Value::Double(v),
        _ if float_out => Value::Single(v as f32),
        _ => Value::Integer(v as i32),
    }
}

#[derive(Clone, Copy)]
struct SurfaceVertex {
    xy: [f64; 2],
    m: f64,
}

fn surface_vertex(g: &unigis_core::Geometry, i: usize) -> SurfaceVertex {
    SurfaceVertex {
        xy: g.xy[i],
        m: g.m.as_ref().map_or(f64::NAN, |m| m[i]),
    }
}

fn surface_lerp(a: SurfaceVertex, b: SurfaceVertex, t: f64, interpolate_m: bool) -> SurfaceVertex {
    if t == 0.0 {
        return a;
    }
    if t == 1.0 {
        return b;
    }
    SurfaceVertex {
        xy: [
            a.xy[0] + (b.xy[0] - a.xy[0]) * t,
            a.xy[1] + (b.xy[1] - a.xy[1]) * t,
        ],
        m: if interpolate_m && a.m.is_finite() && b.m.is_finite() {
            a.m + (b.m - a.m) * t
        } else {
            f64::NAN
        },
    }
}

fn clip_surface_segment(a: [f64; 2], b: [f64; 2], extent: [f64; 4]) -> Option<(f64, f64)> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let p = [-d[0], d[0], -d[1], d[1]];
    let q = [
        a[0] - extent[0],
        extent[1] - a[0],
        a[1] - extent[2],
        extent[3] - a[1],
    ];
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (&p, &q) in p.iter().zip(&q) {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

fn clip_surface_line(
    g: &unigis_core::Geometry,
    extent: [f64; 4],
    interpolate_m: bool,
) -> Option<unigis_core::Geometry> {
    let mut runs: Vec<Vec<SurfaceVertex>> = Vec::new();
    for range in g.part_ranges() {
        let mut run = Vec::new();
        for i in range.clone().take(range.len().saturating_sub(1)) {
            let a = surface_vertex(g, i);
            let b = surface_vertex(g, i + 1);
            let Some((t0, t1)) = clip_surface_segment(a.xy, b.xy, extent) else {
                if run.len() > 1 {
                    runs.push(std::mem::take(&mut run));
                } else {
                    run.clear();
                }
                continue;
            };
            let ca = surface_lerp(a, b, t0, interpolate_m);
            let cb = surface_lerp(a, b, t1, interpolate_m);
            if run.is_empty() {
                run.push(ca);
            } else if run.last().is_some_and(|v: &SurfaceVertex| v.xy != ca.xy) {
                if run.len() > 1 {
                    runs.push(std::mem::take(&mut run));
                } else {
                    run.clear();
                }
                run.push(ca);
            }
            if run.last().is_some_and(|v| v.xy != cb.xy) {
                run.push(cb);
            }
            if t1 < 1.0 && run.len() > 1 {
                runs.push(std::mem::take(&mut run));
            }
        }
        if run.len() > 1 {
            runs.push(run);
        }
    }
    if runs.is_empty() {
        return None;
    }
    let has_m = g.m.is_some();
    let mut out = unigis_core::Geometry::point(0.0, 0.0);
    out.kind = g.kind;
    out.parts = Vec::new();
    out.xy = Vec::new();
    let mut m = Vec::new();
    for run in runs {
        out.parts.push(out.xy.len() as u32);
        for v in run {
            out.xy.push(v.xy);
            m.push(v.m);
        }
    }
    out.m = has_m.then_some(m);
    Some(out)
}

fn densify_surface_geometry(
    g: &unigis_core::Geometry,
    distance: f64,
    interpolate_m: bool,
) -> unigis_core::Geometry {
    if matches!(
        g.kind,
        unigis_core::GeometryKind::Point | unigis_core::GeometryKind::Multipoint
    ) {
        return g.clone();
    }
    let mut out = unigis_core::Geometry::point(0.0, 0.0);
    out.kind = g.kind;
    out.parts = Vec::new();
    out.xy = Vec::new();
    let mut m = Vec::new();
    for range in g.part_ranges() {
        out.parts.push(out.xy.len() as u32);
        for i in range.clone() {
            let a = surface_vertex(g, i);
            out.xy.push(a.xy);
            m.push(a.m);
            if i + 1 >= range.end {
                continue;
            }
            let b = surface_vertex(g, i + 1);
            let length = (b.xy[0] - a.xy[0]).hypot(b.xy[1] - a.xy[1]);
            let n = (length / distance).ceil().max(1.0) as usize;
            for k in 1..n {
                let v = surface_lerp(a, b, k as f64 / n as f64, interpolate_m);
                out.xy.push(v.xy);
                m.push(v.m);
            }
        }
    }
    out.m = g.m.as_ref().map(|_| m);
    out
}

fn interpolate_surface_z(r: &unigis_raster::Raster, x: f64, y: f64, bilinear: bool) -> Option<f64> {
    let t = r.transform;
    let cx = (x - t[0]) / t[1];
    let cy = (t[3] - y) / -t[5];
    if !cx.is_finite() || !cy.is_finite() {
        return None;
    }
    let (w, h) = (r.width as f64, r.height as f64);
    if !bilinear {
        if !(cx >= 0.0 && cx < w && cy > 0.0 && cy <= h) {
            return None;
        }
        let index = |v: f64| (v.ceil() as i64 - 1).max(0);
        return r.get(index(cx) as usize, index(cy) as usize);
    }
    if !(cx >= 0.0 && cx <= w && cy >= 0.0 && cy <= h) {
        return None;
    }
    let (fx, fy) = (cx - 0.5, cy - 0.5);
    let (c0, r0) = (fx.floor() as i64, fy.floor() as i64);
    let (dx, dy) = (fx - c0 as f64, fy - r0 as f64);
    let at = |c: i64, rr: i64| {
        if c < 0 || rr < 0 || c >= r.width as i64 || rr >= r.height as i64 {
            None
        } else {
            r.get(c as usize, rr as usize)
        }
    };
    let corners = [
        (at(c0, r0), (1.0 - dx) * (1.0 - dy)),
        (at(c0 + 1, r0), dx * (1.0 - dy)),
        (at(c0, r0 + 1), (1.0 - dx) * dy),
        (at(c0 + 1, r0 + 1), dx * dy),
    ];
    let weight: f64 = corners
        .iter()
        .filter(|(v, wt)| v.is_some() && *wt > 0.0)
        .map(|(_, wt)| *wt)
        .sum();
    (weight > 0.0).then(|| {
        corners
            .iter()
            .filter_map(|(v, wt)| v.filter(|_| *wt > 0.0).map(|v| v * wt))
            .sum::<f64>()
            / weight
    })
}

fn zonal_value_table_tokens(text: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    for c in text.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => token.push(c),
            (None, '\'' | '"') => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            (None, c) => token.push(c),
        }
    }
    if quote.is_some() {
        return Err(Error::Unsupported(
            "Input Value Rasters: unmatched quote".into(),
        ));
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn run_zonal_characterization(
    ctx: &dyn unigis_raster::registry::Context,
    output: &str,
) -> Result<()> {
    use unigis_raster::zonal::{CharacterizationRequest, CharacterizationStatistic};

    if ctx.get("out_statistics_features").is_some() {
        return Err(Error::Unsupported(
            "Zonal Characterization feature output is not implemented".into(),
        ));
    }
    if ctx
        .get("process_as_multidimensional")
        .is_some_and(|v| v.eq_ignore_ascii_case("ALL_SLICES"))
    {
        return Err(Error::Unsupported(
            "Zonal Characterization ALL_SLICES is not implemented".into(),
        ));
    }
    if ctx
        .get("zone_field")
        .is_some_and(|v| !v.eq_ignore_ascii_case("VALUE"))
    {
        return Err(Error::Unsupported(
            "Zonal Characterization currently supports raster VALUE zones only".into(),
        ));
    }
    let zone_path = ctx
        .get("in_zone_data")
        .ok_or_else(|| Error::Unsupported("in_zone_data is required".into()))?;
    let zone_source = unigis_raster::read_geotiff(Path::new(&zone_path))?;
    if zone_source.data_type.is_float() || zone_source.bands.len() != 1 {
        return Err(Error::Unsupported(
            "Zonal Characterization zones must be a single-band integer raster".into(),
        ));
    }
    let value_table = ctx
        .get("in_value_rasters_statistics")
        .ok_or_else(|| Error::Unsupported("in_value_rasters_statistics is required".into()))?;
    let mut requests = Vec::new();
    let mut analysis_zones = None;
    for entry in value_table.split(';').filter(|s| !s.trim().is_empty()) {
        let tokens = zonal_value_table_tokens(entry)?;
        let (path, statistic, field_name) = match tokens.as_slice() {
            [path, statistic] => (path, statistic, None),
            [path, statistic, field_name] => (path, statistic, Some(field_name.clone())),
            _ => {
                return Err(Error::Unsupported(
                    "Input Value Rasters entries must be: raster statistic [field_name]".into(),
                ));
            }
        };
        let statistic = CharacterizationStatistic::parse(statistic).ok_or_else(|| {
            Error::Unsupported(format!("Zonal Characterization statistic {statistic}"))
        })?;
        let values = unigis_raster::read_geotiff(Path::new(path))?;
        if values.bands.len() != 1 {
            return Err(Error::Unsupported(
                "Zonal Characterization currently supports single-band value rasters".into(),
            ));
        }
        let (zones, values) = unigis_raster::resample::zones_on_value_grid(&zone_source, &values)
            .ok_or_else(|| {
            Error::Unsupported("zone and value rasters do not overlap".into())
        })?;
        if let Some(base) = &analysis_zones {
            let base: &unigis_raster::Raster = base;
            if base.width != zones.width
                || base.height != zones.height
                || base.transform != zones.transform
            {
                return Err(Error::Unsupported(
                    "Zonal Characterization currently requires value rasters on one analysis grid"
                        .into(),
                ));
            }
        } else {
            analysis_zones = Some(zones);
        }
        let name = Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        requests.push(CharacterizationRequest {
            name,
            values,
            statistic,
            field_name,
        });
    }
    if requests.is_empty() {
        return Err(Error::Unsupported(
            "Input Value Rasters: at least one entry is required".into(),
        ));
    }
    let percentiles = ctx.get("percentile_values").unwrap_or_else(|| "90".into());
    let percentiles: Vec<f64> = percentiles
        .trim_matches(|c| c == '[' || c == ']')
        .split(|c: char| c == ';' || c == ',' || c.is_whitespace())
        .filter(|v| !v.is_empty())
        .map(|v| {
            v.parse::<f64>()
                .ok()
                .filter(|p| (0.0..=100.0).contains(p))
                .ok_or_else(|| Error::Unsupported(format!("Percentile Values: {v}")))
        })
        .collect::<Result<_>>()?;
    if percentiles.is_empty() {
        return Err(Error::Unsupported(
            "Percentile Values: at least one value is required".into(),
        ));
    }
    let percentile_linear = match ctx
        .get("percentile_interpolation_type")
        .unwrap_or_else(|| "AUTO_DETECT".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "AUTO_DETECT" => None,
        "NEAREST" => Some(false),
        "LINEAR" => Some(true),
        other => {
            return Err(Error::Unsupported(format!(
                "percentile interpolation {other}"
            )));
        }
    };
    let ignore_nodata = match ctx
        .get("ignore_nodata")
        .unwrap_or_else(|| "DATA".into())
        .to_ascii_uppercase()
        .as_str()
    {
        "DATA" => true,
        "NODATA" => false,
        other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
    };
    let circular = ctx
        .get("circular_calculation")
        .is_some_and(|v| v.eq_ignore_ascii_case("CIRCULAR"));
    let circular_wrap = if circular {
        let wrap = ctx
            .get("circular_wrap_value")
            .unwrap_or_else(|| "360".into())
            .parse::<f64>()
            .map_err(|_| Error::Unsupported("Circular Wrap Value is not a number".into()))?;
        if !wrap.is_finite() || wrap <= 0.0 {
            return Err(Error::Unsupported(
                "Circular Wrap Value must be positive".into(),
            ));
        }
        Some(wrap)
    } else {
        None
    };
    let (fields, rows) = unigis_raster::zonal::zonal_characterization(
        analysis_zones.as_ref().unwrap(),
        &requests,
        ignore_nodata,
        &percentiles,
        percentile_linear,
        circular_wrap,
    )?;
    write_table(
        &unigis_tools::TableOutput { fields, rows },
        Path::new(output),
    )
}

fn run_locate_depressions(ctx: &dyn unigis_raster::registry::Context, output: &str) -> Result<()> {
    if ctx.get("in_road_data").is_some() || ctx.get("max_road_distance").is_some() {
        return Err(Error::Unsupported(
            "Locate Depressions road input is not implemented".into(),
        ));
    }
    let input = ctx
        .get("in_surface_raster")
        .ok_or_else(|| Error::Unsupported("in_surface_raster is required".into()))?;
    let raster = unigis_raster::read_geotiff(Path::new(&input))?;
    let wkt = raster_wkt(&raster)?;
    let z_unit = ctx.get("z_unit").unwrap_or_else(|| "METER".into());
    let meters_per_z = match z_unit.to_ascii_uppercase().as_str() {
        "INCH" => 0.0254,
        "FOOT" => 0.3048,
        "FOOT_US" => 1200.0 / 3937.0,
        "YARD" => 0.9144,
        "MILE_US" => 6_336_000.0 / 3937.0,
        "NAUTICAL_MILE" => 1852.0,
        "MILLIMETER" => 0.001,
        "CENTIMETER" => 0.01,
        "METER" => 1.0,
        "KILOMETER" => 1000.0,
        "DECIMETER" => 0.1,
        other => return Err(Error::Unsupported(format!("z unit {other}"))),
    };
    let min_depth = ctx
        .get("min_depth")
        .map_or(Ok(0.05), |v| parse_distance(&v, None))?
        / meters_per_z;
    let cell_area = (raster.transform[1] * raster.transform[5]
        - raster.transform[2] * raster.transform[4])
        .abs();
    let min_area = match ctx.get("min_area") {
        None => 3.0 * cell_area,
        Some(text) => {
            let mut words = text.split_whitespace();
            let value = words
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .ok_or_else(|| Error::Unsupported(format!("invalid area `{text}`")))?;
            let Some(unit) = words.next() else {
                return write_locate_depressions(
                    ctx,
                    output,
                    raster,
                    min_depth,
                    value,
                    meters_per_z,
                    wkt,
                );
            };
            let square_meters = match unit.to_ascii_lowercase().replace([' ', '_'], "").as_str() {
                "squaremeters" | "squaremetres" => 1.0,
                "squarekilometers" | "squarekilometres" => 1_000_000.0,
                "squarecentimeters" => 0.0001,
                "squaremillimeters" => 0.000001,
                "squarefeet" => 0.3048_f64.powi(2),
                "squareusfeet" | "squarefootus" => (1200.0_f64 / 3937.0).powi(2),
                "squareyards" => 0.9144_f64.powi(2),
                "squaremiles" => 1609.344_f64.powi(2),
                "acres" => 4046.8564224,
                "hectares" => 10_000.0,
                other => return Err(Error::Unsupported(format!("unknown area unit `{other}`"))),
            };
            let map_meters = wkt
                .as_deref()
                .and_then(|w| w.rsplit("UNIT[").next())
                .and_then(|u| u.split(',').nth(1))
                .and_then(|f| f.trim_end_matches([']', ' ']).parse::<f64>().ok())
                .unwrap_or(1.0);
            value * square_meters / map_meters.powi(2)
        }
    };
    write_locate_depressions(ctx, output, raster, min_depth, min_area, meters_per_z, wkt)
}

fn write_locate_depressions(
    ctx: &dyn unigis_raster::registry::Context,
    output: &str,
    raster: unigis_raster::Raster,
    min_depth: f64,
    min_area: f64,
    meters_per_z: f64,
    wkt: Option<String>,
) -> Result<()> {
    use unigis_core::{Feature, Field, FieldType, Geometry, GeometryKind, Value};

    let max_connection = ctx
        .get("max_connection_distance")
        .map(|v| parse_distance(&v, wkt.as_deref()))
        .transpose()?;
    let depressions = unigis_raster::hydro::locate_depressions(
        &raster,
        min_depth,
        min_area,
        max_connection,
        meters_per_z,
    );
    let output = Path::new(output);
    let folder = output.extension().is_none();
    if folder {
        std::fs::create_dir_all(output).map_err(|e| Error::io(output, e))?;
    }
    let polygon_path = if folder {
        output.join("depressions.shp")
    } else {
        output.to_path_buf()
    };
    let point_path = ctx
        .get("out_connection_points")
        .map(PathBuf::from)
        .or_else(|| folder.then(|| output.join("points.shp")));
    let connected = max_connection.is_some();
    let mut polygon_fields = vec![
        Field::new("DEP_ID", FieldType::Integer),
        Field::new("COUNT", FieldType::Double),
        Field::new("MIN", FieldType::Double),
        Field::new("MAX", FieldType::Double),
        Field::new("RANGE", FieldType::Double),
        Field::new("MEAN", FieldType::Double),
        Field::new("SUM", FieldType::Double),
        Field::new("AREA", FieldType::Double),
        Field::new("VOLUME", FieldType::Double),
    ];
    if connected {
        polygon_fields.push(Field::new("DIST_CON", FieldType::Double));
    }
    let mut polygons = Vec::new();
    let mut points = Vec::new();
    for (id, depression) in depressions.into_iter().enumerate() {
        let mut geometry = Geometry::point(0.0, 0.0);
        geometry.kind = GeometryKind::Polygon;
        geometry.xy.clear();
        geometry.parts.clear();
        for ring in depression.polygon.rings {
            geometry.parts.push(geometry.xy.len() as u32);
            geometry.xy.extend(ring);
        }
        let mut attributes = vec![
            Value::Integer(id as i32),
            Value::Double(depression.count as f64),
            Value::Double(depression.min),
            Value::Double(depression.max),
            Value::Double(depression.max - depression.min),
            Value::Double(depression.mean),
            Value::Double(depression.sum),
            Value::Double(depression.area),
            Value::Double(depression.volume),
        ];
        if connected {
            attributes.push(Value::Double(
                depression
                    .connection_distance
                    .expect("connected depression"),
            ));
        }
        polygons.push(Feature {
            oid: id as i64,
            geometry: Some(geometry),
            attributes,
        });
        for point in depression.points {
            let mut attributes = vec![
                Value::Integer(id as i32),
                Value::Integer(point.point_type),
                Value::Double(point.elevation),
                Value::Double(point.edge_distance),
            ];
            if connected {
                attributes.push(Value::Double(
                    point.point_distance.expect("connected point"),
                ));
            }
            points.push(Feature {
                oid: points.len() as i64,
                geometry: Some(Geometry::point(point.xy[0], point.xy[1])),
                attributes,
            });
        }
    }
    write_output(
        ToolOutput {
            kind: GeometryKind::Polygon,
            has_z: false,
            has_m: false,
            fields: polygon_fields,
            spatial_reference_wkt: wkt.clone(),
            features: polygons,
            messages: Vec::new(),
        },
        &polygon_path,
        None,
    )?;
    if let Some(path) = point_path {
        let mut fields = vec![
            Field::new("DEP_ID", FieldType::Integer),
            Field::new("POINT_TYPE", FieldType::Integer),
            Field::new("POINT_ELEV", FieldType::Double),
            Field::new("DIST_EDGE", FieldType::Double),
        ];
        if connected {
            fields.push(Field::new("DIST_POINT", FieldType::Double));
        }
        write_output(
            ToolOutput {
                kind: GeometryKind::Point,
                has_z: false,
                has_m: false,
                fields,
                spatial_reference_wkt: wkt,
                features: points,
                messages: Vec::new(),
            },
            &path,
            None,
        )?;
    }
    Ok(())
}

fn run_interpolate_shape(ctx: &dyn unigis_raster::registry::Context, output: &str) -> Result<()> {
    use unigis_core::{Geometry, GeometryKind};
    let input = ctx
        .get("input")
        .ok_or_else(|| Error::Unsupported("input is required".into()))?;
    let features = ctx
        .get("features")
        .ok_or_else(|| Error::Unsupported("features is required".into()))?;
    let raster = unigis_raster::read_geotiff(Path::new(&input))?;
    let (fc, _) = open_feature_class(Path::new(&features))?;
    let kind = fc
        .geometry_kind()
        .ok_or_else(|| Error::Unsupported("input features have no geometry".into()))?;
    if kind == GeometryKind::MultiPatch {
        return Err(Error::Unsupported(
            "Interpolate Shape does not support multipatch input".into(),
        ));
    }
    let densify = !ctx
        .get("vertices_only")
        .is_some_and(|v| v.eq_ignore_ascii_case("VERTICES_ONLY"));
    let method = ctx.get("method").unwrap_or_else(|| "BILINEAR".into());
    if !method.eq_ignore_ascii_case("BILINEAR") && !method.eq_ignore_ascii_case("NEAREST") {
        return Err(Error::Unsupported(format!(
            "unsupported interpolation method: {method}"
        )));
    }
    let bilinear = densify || method.eq_ignore_ascii_case("BILINEAR");
    let interpolate_m = ctx
        .get("interpolate_m_value")
        .is_some_and(|v| v.eq_ignore_ascii_case("INTERPOLATE_M_VALUE"));
    if ctx
        .get("preserve_features")
        .is_some_and(|v| v.eq_ignore_ascii_case("PRESERVE"))
    {
        return Err(Error::Unsupported(
            "Interpolate Shape PRESERVE is not implemented".into(),
        ));
    }
    let pyramid = ctx
        .get("pyramid_level_resolution")
        .filter(|v| !v.is_empty())
        .map_or(Ok(0.0), |v| {
            v.parse::<f64>()
                .map_err(|_| Error::Unsupported(format!("invalid pyramid level resolution: {v}")))
        })?;
    if pyramid != 0.0 {
        return Err(Error::Unsupported(
            "Interpolate Shape raster input only supports pyramid level resolution 0".into(),
        ));
    }
    let z_factor = ctx
        .get("z_factor")
        .filter(|v| !v.is_empty())
        .map_or(Ok(1.0), |v| {
            v.parse::<f64>()
                .map_err(|_| Error::Unsupported(format!("invalid z factor: {v}")))
        })?;
    if !z_factor.is_finite() {
        return Err(Error::Unsupported("z factor must be finite".into()));
    }
    let cell_size = raster.transform[1].abs();
    let distance = ctx
        .get("sample_distance")
        .filter(|v| !v.is_empty())
        .map_or(Ok(cell_size), |v| {
            v.parse::<f64>()
                .map_err(|_| Error::Unsupported(format!("invalid sample distance: {v}")))
        })?;
    if densify && !(distance.is_finite() && distance > 0.0) {
        return Err(Error::Unsupported(
            "sample distance must be greater than zero".into(),
        ));
    }
    let extent = [
        raster.transform[0],
        raster.transform[0] + raster.transform[1] * raster.width as f64,
        raster.transform[3] + raster.transform[5] * raster.height as f64,
        raster.transform[3],
    ];
    let extent_polygon = Geometry {
        kind: GeometryKind::Polygon,
        parts: vec![0],
        part_types: None,
        xy: vec![
            [extent[0], extent[2]],
            [extent[0], extent[3]],
            [extent[1], extent[3]],
            [extent[1], extent[2]],
            [extent[0], extent[2]],
        ],
        z: None,
        m: None,
        curves: Vec::new(),
    };
    let clip_area = unigis_geometry::clip::ClipArea::new([&extent_polygon])?;
    let has_m = fc.has_m();
    let mut output_features = Vec::new();
    for feature in fc.features() {
        let mut feature = feature?;
        let Some(g) = &feature.geometry else {
            continue;
        };
        if g.has_curves() {
            return Err(Error::Unsupported(
                "Interpolate Shape does not yet support true curves".into(),
            ));
        }
        let mut g = if densify {
            match kind {
                GeometryKind::Polyline => {
                    let Some(g) = clip_surface_line(g, extent, interpolate_m) else {
                        continue;
                    };
                    densify_surface_geometry(&g, distance, interpolate_m)
                }
                GeometryKind::Polygon => {
                    let outside = g.xy.iter().any(|p| {
                        p[0] < extent[0] || p[0] > extent[1] || p[1] < extent[2] || p[1] > extent[3]
                    });
                    let clipped = if outside {
                        unigis_geometry::clip::clip(g, &clip_area)?
                    } else {
                        Some(g.clone())
                    };
                    let Some(g) = clipped else {
                        continue;
                    };
                    densify_surface_geometry(&g, distance, interpolate_m)
                }
                _ => g.clone(),
            }
        } else {
            g.clone()
        };
        let mut z = Vec::with_capacity(g.xy.len());
        let mut valid = true;
        for p in &g.xy {
            let Some(value) = interpolate_surface_z(&raster, p[0], p[1], bilinear) else {
                valid = false;
                break;
            };
            z.push(value * z_factor);
        }
        if !valid {
            continue;
        }
        g.z = Some(z);
        if has_m && g.m.is_none() {
            g.m = Some(vec![f64::NAN; g.xy.len()]);
        }
        feature.geometry = Some(g);
        output_features.push(feature);
    }
    let spatial_reference_wkt = fc.spatial_reference_wkt().map(|wkt| {
        let normalized = unigis_proj::Crs::from_wkt(wkt)
            .map(|crs| crs.arcgis_wkt())
            .unwrap_or_else(|_| wkt.to_owned());
        if z_factor == 1.0 || normalized.contains(",VERTCS[") {
            normalized
        } else {
            format!(
                "{normalized},VERTCS[\"Unknown VCS from ArcInfo Workstation\",VDATUM[\"Unknown\"],PARAMETER[\"Vertical_Shift\",0.0],PARAMETER[\"Direction\",1.0],UNIT[\"User_Defined_Unit\",{}]]",
                1.0 / z_factor
            )
        }
    });
    write_output_as(
        ToolOutput {
            kind,
            has_z: true,
            has_m,
            fields: fc.fields().to_vec(),
            spatial_reference_wkt,
            features: output_features,
            messages: Vec::new(),
        },
        Path::new(output),
        None,
        false,
    )
}

/// Extract Multi Values To Points: the points with one added field per raster (named as given, or
/// by the raster's file stem, cut to 10 characters), holding the raster's value at the point as
/// in Sample. Integer rasters give integers, except with BILINEAR (single floats).
/// The surface height at `(x, y)`: bilinear between the four cell centres around it, positions
/// beyond the outermost centres taking the edge (ArcGIS's surface interpolation); `None` outside
/// the raster or where all four neighbours are NoData.
fn surface_z(r: &unigis_raster::Raster, x: f64, y: f64) -> Option<f64> {
    let t = r.transform;
    let (cx, cy) = ((x - t[0]) / t[1], (t[3] - y) / -t[5]);
    if !(cx >= 0.0 && cy >= 0.0 && cx <= r.width as f64 && cy <= r.height as f64) {
        return None;
    }
    let fx = (cx - 0.5).clamp(0.0, (r.width - 1) as f64);
    let fy = (cy - 0.5).clamp(0.0, (r.height - 1) as f64);
    let (c0, r0) = (fx.floor() as usize, fy.floor() as usize);
    let (c1, r1) = ((c0 + 1).min(r.width - 1), (r0 + 1).min(r.height - 1));
    let (dx, dy) = (fx - c0 as f64, fy - r0 as f64);
    let mut sum = 0.0;
    let mut weight = 0.0;
    for (c, rr, w) in [
        (c0, r0, (1.0 - dx) * (1.0 - dy)),
        (c1, r0, dx * (1.0 - dy)),
        (c0, r1, (1.0 - dx) * dy),
        (c1, r1, dx * dy),
    ] {
        if w == 0.0 {
            continue;
        }
        if let Some(v) = r.get(c, rr) {
            sum += v * w;
            weight += w;
        }
    }
    // NoData neighbours drop out and the weights renormalize; NoData only when all are.
    (weight > 0.0).then(|| sum / weight)
}

/// The part of the segment `a`..`b` inside the rectangle (Liang-Barsky), if any.
fn clip_segment(
    a: [f64; 2],
    b: [f64; 2],
    (x0, y0, x1, y1): (f64, f64, f64, f64),
) -> Option<([f64; 2], [f64; 2])> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a[0] - x0),
        (dx, x1 - a[0]),
        (-dy, a[1] - y0),
        (dy, y1 - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then(|| {
        (
            [a[0] + t0 * dx, a[1] + t0 * dy],
            [a[0] + t1 * dx, a[1] + t1 * dy],
        )
    })
}

/// `arcpy.sa.AddSurfaceInformation(in_feature_class, in_surface, out_property, method,
/// sample_distance, z_factor)` for points (Z) and lines (Z_MIN, Z_MAX, Z_MEAN, SURFACE_LENGTH,
/// MIN_SLOPE, MAX_SLOPE, AVG_SLOPE). A line is cut to the raster extent and each segment split into
/// equal steps no longer than the sampling distance (the cell size by default); its profile gives
/// the 3D length, the 3D-length-weighted mean height and percent slope, and the extreme heights and
/// segment slopes (observed, ArcGIS Pro 3.7).
/// A step of a surface profile: 2D length, 3D length, start and end height.
type Step = (f64, f64, f64, f64);

fn run_add_surface_information(
    ctx: &dyn unigis_raster::registry::Context,
    output: &str,
) -> Result<()> {
    use unigis_core::{Field, FieldType, GeometryKind};
    let input = ctx.get("input").ok_or_else(|| {
        Error::Unsupported("ERROR 000735: Input Features: value is required".into())
    })?;
    let (fc, _) = open_feature_class(Path::new(&input))?;
    let surface = ctx.raster("in_surface")?;
    let z_factor: f64 = ctx
        .get("z_factor")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1.0);
    let wanted: Vec<String> = ctx
        .get("out_property")
        .unwrap_or_default()
        .split(';')
        .map(|p| p.trim().to_ascii_uppercase())
        .filter(|p| !p.is_empty())
        .collect();
    let kind = fc.geometry_kind();
    let allowed: &[(&str, &str)] = match kind {
        Some(GeometryKind::Point) => &[("Z", "Z")],
        Some(GeometryKind::Polyline) => &[
            ("Z_MIN", "Z_Min"),
            ("Z_MAX", "Z_Max"),
            ("Z_MEAN", "Z_Mean"),
            ("SURFACE_LENGTH", "SLength"),
            ("MIN_SLOPE", "Min_Slope"),
            ("MAX_SLOPE", "Max_Slope"),
            ("AVG_SLOPE", "Avg_Slope"),
        ],
        _ => {
            return Err(Error::Unsupported(
                "Add Surface Information on polygons and multipoints is not supported yet".into(),
            ));
        }
    };
    for w in &wanted {
        if !allowed.iter().any(|(k, _)| k == w) {
            let names: Vec<&str> = allowed.iter().map(|(k, _)| *k).collect();
            return Err(Error::Unsupported(format!(
                "ERROR 000800: The value is not a member of {}.",
                names.join(" | ")
            )));
        }
    }
    if wanted.is_empty() {
        return Err(Error::Unsupported(
            "ERROR 000735: Output Property: value is required".into(),
        ));
    }
    // Fields in the order the properties are given.
    let mut chosen: Vec<(&str, &str)> = Vec::new();
    for w in &wanted {
        if let Some(&p) = allowed.iter().find(|(k, _)| k == w)
            && !chosen.contains(&p)
        {
            chosen.push(p);
        }
    }
    let t = surface.transform;
    let extent = (
        t[0],
        t[3] + t[5] * surface.height as f64,
        t[0] + t[1] * surface.width as f64,
        t[3],
    );
    let step = ctx
        .get("sample_distance")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(t[1]);
    let mut fields = fc.fields().to_vec();
    for (_, name) in &chosen {
        fields.push(Field::new(*name, FieldType::Double));
    }
    let mut features = Vec::new();
    for f in fc.features() {
        let mut f = f?;
        let mut values: Vec<Option<f64>> = vec![None; chosen.len()];
        if let Some(g) = &f.geometry {
            if kind == Some(GeometryKind::Point) {
                let p = g.xy[0];
                values[0] = surface_z(&surface, p[0], p[1]).map(|z| z * z_factor);
            } else {
                // The profile, part by part: (2D length, 3D length, start z, end z) of each step.
                // NoData samples are skipped; the profile joins the valid samples around them.
                let mut steps: Vec<Step> = Vec::new();
                let mut zs: Vec<f64> = Vec::new();
                let starts: Vec<usize> = g.parts.iter().map(|&p| p as usize).collect();
                let ends: Vec<usize> = starts.iter().skip(1).copied().chain([g.xy.len()]).collect();
                for (k, &start) in starts.iter().enumerate() {
                    let ring = &g.xy[start..ends[k]];
                    let mut last: Option<(f64, f64, f64)> = None;
                    for w in ring.windows(2) {
                        let Some((a, b)) = clip_segment(w[0], w[1], extent) else {
                            continue;
                        };
                        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                        let n = ((len / step).ceil() as usize).max(1);
                        for i in 0..=n {
                            let s = i as f64 / n as f64;
                            let (x, y) = (a[0] + s * (b[0] - a[0]), a[1] + s * (b[1] - a[1]));
                            let Some(z) = surface_z(&surface, x, y).map(|z| z * z_factor) else {
                                continue;
                            };
                            if let Some((x0, y0, z0)) = last {
                                let d = (x - x0).hypot(y - y0);
                                if d > 0.0 {
                                    steps.push((d, d.hypot(z - z0), z0, z));
                                }
                            }
                            zs.push(z);
                            last = Some((x, y, z));
                        }
                    }
                }
                if !zs.is_empty() {
                    let length3: f64 = steps.iter().map(|s| s.1).sum();
                    let slopes: Vec<f64> = steps
                        .iter()
                        .filter(|s| s.0 > 0.0)
                        .map(|s| (s.3 - s.2).abs() / s.0 * 100.0)
                        .collect();
                    let weighted = |v: &dyn Fn(&Step) -> f64| {
                        (length3 > 0.0)
                            .then(|| steps.iter().map(|s| v(s) * s.1).sum::<f64>() / length3)
                    };
                    for (slot, (key, _)) in values.iter_mut().zip(&chosen) {
                        *slot = match *key {
                            "Z_MIN" => zs.iter().copied().reduce(f64::min),
                            "Z_MAX" => zs.iter().copied().reduce(f64::max),
                            "Z_MEAN" => weighted(&|s| (s.2 + s.3) / 2.0),
                            "SURFACE_LENGTH" => Some(length3),
                            "MIN_SLOPE" => slopes.iter().copied().reduce(f64::min),
                            "MAX_SLOPE" => slopes.iter().copied().reduce(f64::max),
                            _ => weighted(&|s| {
                                if s.0 > 0.0 {
                                    (s.3 - s.2).abs() / s.0 * 100.0
                                } else {
                                    0.0
                                }
                            }),
                        };
                    }
                }
            }
        }
        f.attributes.extend(
            values
                .into_iter()
                .map(|v| v.map_or(Value::Double(0.0), Value::Double)),
        );
        features.push(f);
    }
    write_output_as(
        ToolOutput {
            kind: kind.unwrap_or(GeometryKind::Point),
            has_z: fc.has_z(),
            has_m: fc.has_m(),
            fields,
            spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
            features,
            messages: Vec::new(),
        },
        Path::new(output),
        None,
        true,
    )
}

/// Contour lines as ArcGIS writes them: `Id` 1.. and the `Contour` value.
fn write_contours(
    lines: Vec<unigis_raster::contour::ContourLine>,
    r: &unigis_raster::Raster,
    output: &Path,
) -> Result<()> {
    let features = lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let mut g = unigis_core::Geometry::point(0.0, 0.0);
            g.kind = unigis_core::GeometryKind::Polyline;
            g.parts = vec![0];
            g.xy = l.points;
            Feature {
                oid: i as i64,
                geometry: Some(g),
                attributes: vec![Value::Integer(i as i32 + 1), Value::Double(l.value)],
            }
        })
        .collect();
    let out = ToolOutput {
        kind: unigis_core::GeometryKind::Polyline,
        has_z: false,
        has_m: false,
        fields: vec![
            Field::new("Id", FieldType::Integer),
            Field::new("Contour", FieldType::Double),
        ],
        spatial_reference_wkt: raster_wkt(r)?,
        features,
        messages: Vec::new(),
    };
    write_output(out, output, None)
}

fn run_extract_multi_values(
    ctx: &dyn unigis_raster::registry::Context,
    output: &str,
) -> Result<()> {
    use unigis_core::{Field, FieldType};
    let input = ctx
        .get("input")
        .ok_or_else(|| Error::Unsupported("input is required".into()))?;
    let (fc, _) = open_feature_class(Path::new(&input))?;
    let bilinear = ctx
        .get("interpolate")
        .is_some_and(|v| v.eq_ignore_ascii_case("BILINEAR"));
    let mut rasters = Vec::new();
    for item in ctx.get("rasters").unwrap_or_default().split(';') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        // "path" or "path name" (the name is the last word when the path itself has no space).
        let (path, name) = match item.rsplit_once(' ') {
            Some((p, n)) if !Path::new(item).exists() => (p.trim(), Some(n.trim())),
            _ => (item, None),
        };
        let stem = Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name: String = name.map_or(stem, str::to_owned).chars().take(10).collect();
        rasters.push((name, unigis_raster::read_geotiff(Path::new(path))?));
    }
    let mut fields = fc.fields().to_vec();
    for (name, r) in &rasters {
        let ty = match r.data_type {
            unigis_raster::DataType::F32 => FieldType::Single,
            unigis_raster::DataType::F64 => FieldType::Double,
            _ if bilinear => FieldType::Single,
            _ => FieldType::Integer,
        };
        let mut f = Field::new(name.clone(), ty);
        match ty {
            FieldType::Double => {
                f.precision = 18;
                f.scale = 11;
            }
            FieldType::Integer => {
                f.length = 10;
                f.precision = 10;
            }
            _ => {}
        }
        fields.push(f);
    }
    let mut features = Vec::new();
    for f in fc.features() {
        let mut f = f?;
        if let Some(g) = &f.geometry {
            let p = g.xy[0];
            for (_, r) in &rasters {
                f.attributes
                    .push(sample_value(r, p[0], p[1], bilinear, bilinear));
            }
        }
        features.push(f);
    }
    write_output_as(
        ToolOutput {
            kind: fc
                .geometry_kind()
                .unwrap_or(unigis_core::GeometryKind::Point),
            has_z: fc.has_z(),
            has_m: fc.has_m(),
            fields,
            spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
            features,
            messages: Vec::new(),
        },
        Path::new(output),
        None,
        true,
    )
}

fn write_raster(path: &Path, raster: &unigis_raster::Raster) -> Result<()> {
    write_raster_with(path, raster, true)
}

/// For tools whose ArcGIS statistics are embedded in TIFF rather than a PAM sidecar
/// (Euclidean Distance, Fill, Flow Direction, Reclassify, Stream Link, Watershed, Zonal Statistics).
fn write_raster_plain(path: &Path, raster: &unigis_raster::Raster) -> Result<()> {
    write_raster_with_metadata(
        path,
        raster,
        false,
        unigis_raster::stats_metadata::for_raster(raster),
    )
}

/// ArcGIS's Basin writes statistics whose cell count is 0.
fn write_raster_basin(path: &Path, raster: &unigis_raster::Raster) -> Result<()> {
    write_raster_with(path, raster, false)?;
    unigis_raster::stats::write_aux_statistics_count(path, raster, Some(0))
}

fn define_raster_projection(input: &Path, output: &Path, coordinate_system: &str) -> Result<()> {
    let normalized = coordinate_system.trim().to_ascii_uppercase();
    let code: u32 = normalized
        .strip_prefix("EPSG:")
        .unwrap_or(&normalized)
        .parse()
        .map_err(|_| {
            Error::Unsupported(
                "Raster Define Projection currently requires a numeric EPSG code".into(),
            )
        })?;
    if code == 0 || code >= u32::from(u16::MAX) {
        return Err(Error::Unsupported(
            "EPSG code cannot be represented by this GeoTIFF writer".into(),
        ));
    }
    let crs = unigis_proj::Crs::parse(&format!("EPSG:{code}"))?;
    if !crs.esri_wkt().starts_with("GEOGCS[") && !crs.esri_wkt().starts_with("PROJCS[") {
        return Err(Error::Unsupported(
            "Raster Define Projection requires a geographic or projected CRS".into(),
        ));
    }
    for path in [input, output] {
        let aux = std::path::PathBuf::from(format!("{}.aux.xml", path.display()));
        match std::fs::read_to_string(&aux) {
            Ok(xml) if xml.contains("<SRS") => {
                return Err(Error::Unsupported(
                    "Raster Define Projection does not yet update auxiliary SRS overrides".into(),
                ));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(Error::io(&aux, e)),
            _ => {}
        }
    }
    let mut raster = unigis_raster::read_geotiff(input)?;
    let statistics = raster.statistics.is_some();
    raster.epsg = Some(code);
    write_raster_with(output, &raster, statistics)
}

fn write_raster_with(path: &Path, raster: &unigis_raster::Raster, statistics: bool) -> Result<()> {
    write_raster_with_metadata(path, raster, statistics, None)
}

fn write_raster_with_metadata(
    path: &Path,
    raster: &unigis_raster::Raster,
    statistics: bool,
    gdal_metadata: Option<String>,
) -> Result<()> {
    write_raster_with_tags(path, raster, statistics, gdal_metadata, None)
}

fn write_raster_with_tags(
    path: &Path,
    raster: &unigis_raster::Raster,
    statistics: bool,
    gdal_metadata: Option<String>,
    nodata_tag: Option<f64>,
) -> Result<()> {
    let compression = if raster.data_type.is_float() {
        unigis_raster::Compression::None
    } else {
        unigis_raster::Compression::Lzw
    };
    unigis_raster::write_geotiff_with(
        path,
        raster,
        &unigis_raster::WriteOptions {
            compression,
            tile: Some((128, 128)),
            zero_out_of_range_nodata: true,
            gdal_metadata,
            nodata_tag,
            ..Default::default()
        },
    )?;
    // A completed replacement must not inherit old PAM overrides, including
    // when an all-NoData output has no new statistics to write.
    let aux = std::path::PathBuf::from(format!("{}.aux.xml", path.display()));
    if let Err(error) = std::fs::remove_file(&aux)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(Error::io(&aux, error));
    }
    if statistics {
        unigis_raster::stats::write_aux_statistics(path, raster)?;
    }
    println!(
        "Wrote {} ({}x{})",
        path.display(),
        raster.width,
        raster.height
    );
    Ok(())
}

/// A raster path or a numeric constant.
fn operand(text: &str) -> Result<unigis_raster::local::Operand> {
    match unigis_raster::local::Operand::constant(text) {
        Some(c) => Ok(c),
        None => Ok(unigis_raster::local::Operand::Raster(
            unigis_raster::read_geotiff(Path::new(text))?,
        )),
    }
}

fn value_clause(text: Option<&str>) -> Result<Option<unigis_raster::local::ValueClause>> {
    match text.filter(|t| !t.trim().is_empty()) {
        None => Ok(None),
        Some(t) => unigis_raster::local::ValueClause::parse(t)
            .map(Some)
            .ok_or_else(|| Error::Unsupported(format!("where clause {t}"))),
    }
}

fn math_tool(op: unigis_raster::local::MathOp, a: &str, b: &str, out: &Path) -> Result<()> {
    let (a, b) = (operand(a)?, operand(b)?);
    let result = unigis_raster::local::math(op, &a, &b)
        .ok_or_else(|| Error::Unsupported("at least one input must be a raster".into()))?;
    write_raster(out, &result)
}

/// The ArcGIS WKT of a raster's EPSG coordinate system, if it has one.
fn raster_wkt(r: &unigis_raster::Raster) -> Result<Option<String>> {
    r.epsg
        .map(|code| unigis_proj::Crs::parse(&code.to_string()).map(|c| c.arcgis_wkt()))
        .transpose()
}

type RasterizeInput = (
    Vec<(unigis_core::Geometry, f64)>,
    unigis_raster::rasterize::ValueKind,
    Option<u32>,
    Option<unigis_core::GeometryKind>,
);

/// Shapes with the numeric value of `field` (the OID field included), the kind
/// of that value, and the EPSG code of the features' coordinate system.
/// Features with their weights for the density tools: the population field's value,
/// or 1 for NONE.
type DensityInput = (
    Vec<(unigis_core::Geometry, f64)>,
    Option<u32>,
    Option<unigis_core::GeometryKind>,
);

fn density_input(path: &Path, field: &str) -> Result<DensityInput> {
    if !field.eq_ignore_ascii_case("NONE") {
        let (features, _, epsg, kind) = rasterize_input(path, field)?;
        return Ok((features, epsg, kind));
    }
    let (fc, _) = open_feature_class(path)?;
    let mut features = Vec::new();
    for f in fc.features() {
        if let Some(g) = f?.geometry {
            features.push((g, 1.0));
        }
    }
    let epsg = fc
        .spatial_reference_wkt()
        .and_then(|w| unigis_proj::Crs::from_wkt(w).ok())
        .and_then(|c| c.wkid())
        .map(|(_, latest)| latest);
    Ok((features, epsg, fc.geometry_kind()))
}

/// Feature Class To Shapefile: every input converted (as Feature Class To Feature Class) before
/// any is written, then written as `<name>.shp` in `folder`, `<name>_1.shp`, ... when taken.
fn export_shapefiles(inputs: &[String], folder: &Path) -> Result<Vec<String>> {
    if !folder.is_dir() {
        return Err(Error::Unsupported(format!(
            "ERROR 000732: Output Folder: Dataset {} does not exist or is not supported",
            folder.display()
        )));
    }
    let mut planned = Vec::new();
    for input in inputs {
        let path = Path::new(input);
        let (fc, _) = open_feature_class(path).map_err(|_| {
            Error::Unsupported(format!(
                "ERROR 000732: Input Features: Dataset {input} does not exist or is not supported"
            ))
        })?;
        let out = unigis_tools::feature_class_to_feature_class(
            fc.as_ref(),
            &catalog_path(path),
            None,
            None,
        )?;
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".into());
        planned.push((stem, out, source_gdb(path)));
    }
    let mut written: Vec<String> = Vec::new();
    for (stem, out, gdb) in planned {
        let taken = |n: &str| {
            written.iter().any(|w| w.eq_ignore_ascii_case(n))
                || folder.join(format!("{n}.shp")).exists()
                || folder.join(format!("{n}.dbf")).exists()
        };
        let mut name = stem.clone();
        let mut k = 0;
        while taken(&name) {
            k += 1;
            name = format!("{stem}_{k}");
        }
        write_output(out, &folder.join(format!("{name}.shp")), gdb.as_deref())?;
        written.push(name);
    }
    Ok(written)
}

fn export_dbase(inputs: &[String], folder: &Path) -> Result<Vec<String>> {
    if !folder.is_dir() {
        return Err(Error::Unsupported(format!(
            "ERROR 000732: Output Folder: Dataset {} does not exist or is not supported",
            folder.display()
        )));
    }
    let mut planned: Vec<(String, unigis_tools::TableOutput)> = Vec::new();
    for input in inputs {
        let path = Path::new(input);
        let (fc, _) = open_feature_class(path).map_err(|_| {
            Error::Unsupported(format!(
                "ERROR 000732: Input Tables: Dataset {input} does not exist or is not supported"
            ))
        })?;
        let mut table = unigis_tools::tables::copy_rows(fc.as_ref(), None)?;
        for field in &mut table.fields {
            if field.field_type == FieldType::Double && field.precision > 0 {
                field.precision = field.precision.min(19 - u32::from(field.scale > 0));
            }
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".into());
        if let Some((_, previous)) = planned
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(&stem))
        {
            *previous = table;
        } else {
            planned.push((stem, table));
        }
    }
    let mut names = Vec::new();
    for (name, table) in planned {
        write_table(&table, &folder.join(format!("{name}.dbf")))?;
        names.push(name);
    }
    Ok(names)
}

fn export_excel(
    inputs: &[String],
    output: &Path,
    use_aliases: bool,
    use_descriptions: bool,
) -> Result<Vec<String>> {
    if !output
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
    {
        return Err(Error::Unsupported(
            "Table To Excel currently supports .xlsx output only; legacy .xls is not supported"
                .into(),
        ));
    }
    let mut sheets = Vec::new();
    for input in inputs {
        let path = Path::new(input);
        let missing = || {
            Error::Unsupported(format!(
                "ERROR 000732: Input Table: Dataset {input} does not exist or is not supported"
            ))
        };
        let mut sheet = match locate(path) {
            Location::Geodatabase {
                gdb,
                item: Some(item),
            } => {
                let geodatabase = Geodatabase::open(&gdb).map_err(|_| missing())?;
                let table = geodatabase.open_table(&item).map_err(|_| missing())?;
                excel_sheet(
                    path,
                    &table,
                    Some(&geodatabase),
                    table.subtypes(),
                    use_aliases,
                    use_descriptions,
                )?
            }
            _ => {
                let (table, _) = open_feature_class(path).map_err(|_| missing())?;
                excel_sheet(
                    path,
                    table.as_ref(),
                    None,
                    None,
                    use_aliases,
                    use_descriptions,
                )?
            }
        };
        let base = sheet.name.clone();
        let mut suffix = 1;
        while sheets
            .iter()
            .any(|s: &unigis_tools::table_to_excel::ExcelSheet| {
                s.name.eq_ignore_ascii_case(&sheet.name)
            })
        {
            let ending = format!("_{suffix}");
            let keep = 31usize.saturating_sub(ending.len());
            sheet.name = format!("{}{ending}", base.chars().take(keep).collect::<String>());
            suffix += 1;
        }
        sheets.push(sheet);
    }
    unigis_tools::table_to_excel::write_workbook(output, &sheets)?;
    Ok(sheets.into_iter().map(|s| s.name).collect())
}

fn excel_sheet(
    path: &Path,
    table: &dyn FeatureClass,
    gdb: Option<&Geodatabase>,
    subtypes: Option<&unigis_filegdb::Subtypes>,
    use_aliases: bool,
    use_descriptions: bool,
) -> Result<unigis_tools::table_to_excel::ExcelSheet> {
    let fields: Vec<(usize, &Field)> = table
        .fields()
        .iter()
        .enumerate()
        .filter(|(_, f)| f.field_type != FieldType::Geometry)
        .collect();
    if fields
        .iter()
        .any(|(_, f)| matches!(f.field_type, FieldType::Blob | FieldType::Raster))
    {
        return Err(Error::Unsupported(
            "Table To Excel does not support BLOB or raster fields".into(),
        ));
    }
    let mut headers = Vec::new();
    if table.has_object_id() {
        headers.push(table.oid_field_name().to_owned());
    }
    headers.extend(fields.iter().map(|(_, f)| {
        if use_aliases {
            f.alias
                .as_deref()
                .filter(|a| !a.is_empty())
                .unwrap_or(&f.name)
                .to_owned()
        } else {
            f.name.clone()
        }
    }));
    let subtype_field = subtypes.and_then(|st| {
        table
            .fields()
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(&st.field))
    });
    let mut rows = Vec::new();
    for feature in table.features() {
        let feature = feature?;
        let subtype = subtypes.and_then(|st| {
            let code = match feature.attributes.get(subtype_field?)? {
                Value::SmallInteger(v) => i64::from(*v),
                Value::Integer(v) => i64::from(*v),
                Value::BigInteger(v) => *v,
                _ => return None,
            };
            st.subtypes.iter().find(|s| s.code == code)
        });
        let mut row = Vec::new();
        if table.has_object_id() {
            row.push(
                i32::try_from(feature.oid)
                    .map_or_else(|_| Value::BigInteger(feature.oid), Value::Integer),
            );
        }
        for &(index, field) in &fields {
            let mut value = feature.attributes[index].clone();
            if use_descriptions && !value.is_null() {
                if subtype_field == Some(index) {
                    if let Some(subtype) = subtype {
                        value = Value::String(subtype.name.clone());
                    }
                } else {
                    let domain = subtype
                        .and_then(|st| {
                            st.fields
                                .iter()
                                .find(|f| f.field.eq_ignore_ascii_case(&field.name))
                        })
                        .and_then(|f| f.domain.as_ref())
                        .or(field.domain.as_ref());
                    if let Some(description) =
                        domain.and_then(|name| coded_domain_description(gdb?, name, &value))
                    {
                        value = Value::String(description);
                    }
                }
            }
            row.push(value);
        }
        rows.push(row);
    }
    let input_name = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("dbf"))
        && path.with_extension("shp").is_file()
    {
        path.with_extension("shp")
    } else {
        path.to_path_buf()
    };
    let raw_name = input_name
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Sheet".into());
    let name = raw_name.replace(['[', ']', ':', '*', '?', '/', '\\'], "_");
    let name = name.trim_matches('\'');
    let name = if name.is_empty() { "Sheet" } else { name };
    Ok(unigis_tools::table_to_excel::ExcelSheet {
        name: name.chars().take(31).collect(),
        headers,
        rows,
    })
}

fn coded_domain_description(gdb: &Geodatabase, name: &str, value: &Value) -> Option<String> {
    let unigis_filegdb::DomainKind::Coded(codes) = &gdb.domain(name)?.kind else {
        return None;
    };
    codes
        .iter()
        .find(|(code, _)| {
            unigis_filegdb::compare_values(code, value).is_some_and(std::cmp::Ordering::is_eq)
        })
        .map(|(_, description)| description.clone())
}

fn rasterize_input(path: &Path, field: &str) -> Result<RasterizeInput> {
    use unigis_raster::rasterize::ValueKind;
    let (fc, _) = open_feature_class(path)?;
    let oid = fc.oid_field_name().eq_ignore_ascii_case(field);
    let index = fc
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(field));
    let kind = if oid {
        ValueKind::Integer
    } else {
        match index.map(|i| fc.fields()[i].field_type) {
            Some(FieldType::Double) => ValueKind::Double,
            Some(FieldType::Single) => ValueKind::Single,
            Some(
                FieldType::SmallInteger
                | FieldType::Integer
                | FieldType::BigInteger
                | FieldType::ObjectId,
            ) => ValueKind::Integer,
            _ => {
                return Err(Error::Unsupported(format!(
                    "field {field} is missing or not numeric"
                )));
            }
        }
    };
    let mut features = Vec::new();
    for f in fc.features() {
        let f = f?;
        let value = if oid {
            Some(f.oid as f64)
        } else {
            match &f.attributes[index.unwrap_or(0)] {
                Value::SmallInteger(v) => Some(*v as f64),
                Value::Integer(v) => Some(*v as f64),
                Value::BigInteger(v) => Some(*v as f64),
                Value::Single(v) => Some(*v as f64),
                Value::Double(v) => Some(*v),
                _ => None,
            }
        };
        if let (Some(g), Some(v)) = (f.geometry, value) {
            features.push((g, v));
        }
    }
    let epsg = fc
        .spatial_reference_wkt()
        .and_then(|w| unigis_proj::Crs::from_wkt(w).ok())
        .and_then(|c| c.wkid())
        .map(|(_, latest)| latest);
    Ok((features, kind, epsg, fc.geometry_kind()))
}

/// Delete Rows / Delete Features / Truncate Table. Geodatabase tables are edited in place
/// (one transaction; OBJECTIDs stay, except that Truncate Table restarts them); a
/// shapefile is rewritten without the rows.
fn delete_rows_tool(input: &Path, where_clause: Option<&str>, truncate: bool) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    // The where clause sees the attribute fields and the OBJECTID.
    let mut fields = fc.fields().to_vec();
    fields.push(unigis_core::Field::new(
        fc.oid_field_name(),
        unigis_core::FieldType::BigInteger,
    ));
    let clause = where_clause
        .map(|w| unigis_tools::WhereClause::parse(w, &fields))
        .transpose()?;
    let doomed = |f: &unigis_core::Feature| {
        clause.as_ref().is_none_or(|c| {
            let mut a = f.attributes.clone();
            a.push(unigis_core::Value::BigInteger(f.oid));
            c.matches(&a)
        })
    };
    if let Location::Geodatabase {
        gdb,
        item: Some(item),
    } = locate(input)
    {
        drop(fc);
        let base = Geodatabase::open(&gdb)?.table_base(&item)?;
        if truncate {
            unigis_filegdb::truncate_table(&base)?;
            println!("Truncated {}/{item}", gdb.display());
        } else {
            let n = unigis_filegdb::delete_rows(&base, doomed)?;
            println!("Deleted {n} rows from {}/{item}", gdb.display());
        }
        return Ok(());
    }
    if truncate {
        return Err(Error::Unsupported(
            "Truncate Table works on geodatabase tables only".into(),
        ));
    }
    let mut kept = Vec::new();
    let mut n = 0;
    for f in fc.features() {
        let f = f?;
        if doomed(&f) {
            n += 1;
        } else {
            kept.push(f);
        }
    }
    let out = unigis_tools::ToolOutput {
        kind: fc
            .geometry_kind()
            .unwrap_or(unigis_core::GeometryKind::Point),
        has_z: fc.has_z(),
        has_m: fc.has_m(),
        fields: fc.fields().to_vec(),
        spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
        features: kept,
        messages: Vec::new(),
    };
    drop(fc);
    eprintln!("Deleting {n} rows");
    update_in_place(out, input)
}
