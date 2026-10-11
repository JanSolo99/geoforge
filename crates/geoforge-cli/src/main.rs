//! `gf`: GeoForge geoprocessing tools from the command line. Tool names and
//! behavior follow their ArcGIS Pro counterparts.
//!
//! Datasets are addressed the way ArcGIS does: `roads.shp`, `city.gdb`,
//! `city.gdb/roads` or `city.gdb/Transport/roads` (feature dataset optional).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use geoforge_core::{Error, Feature, FeatureClass, Field, FieldType, Result, Value};
use geoforge_filegdb::Geodatabase;
use geoforge_shapefile::{ShapefileReader, ShapefileWriter};
use geoforge_tools::{Dissolve, ToolOutput};

#[derive(Parser)]
#[command(name = "gf", version, about = "GeoForge geoprocessing tools")]
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
    /// Extract the parts of input features inside clip polygons (ArcGIS "Clip").
    Clip {
        in_features: PathBuf,
        clip_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Geometric intersection of inputs (ArcGIS "Intersect"): `gf intersect a b ... out`.
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
    },
    /// Planar overlay of polygon inputs (ArcGIS "Union"): `gf union a b ... out`.
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
    /// `gf merge a b ... out`.
    Merge {
        /// One or more inputs followed by the output.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// NO_SOURCE_INFO (default) or ADD_SOURCE_INFO (adds MERGE_SRC).
        #[arg(long, default_value = "NO_SOURCE_INFO")]
        add_source: String,
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
    /// `gf append a b ... target`. Updates the target shapefile in place.
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
    },
    /// Write the `.sbn`/`.sbx` spatial index of a shapefile (ArcGIS "Add Spatial
    /// Index"); gf writes one with every shapefile anyway.
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
    },
}

fn main() -> ExitCode {
    let tool = Cli::parse().tool;
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
        Tool::Clip {
            in_features,
            clip_features,
            out_feature_class,
        } => clip_tool(&in_features, &clip_features, &out_feature_class),
        Tool::Intersect {
            mut paths,
            join_attributes,
            output_type,
        } => (|| {
            let out = paths.pop().unwrap();
            let opts = geoforge_tools::IntersectOptions {
                join_attributes: join_attributes_arg(&join_attributes)?,
                output_type: geoforge_tools::OutputType::parse(&output_type)
                    .ok_or_else(|| Error::Unsupported(format!("output_type {output_type}")))?,
            };
            overlay_tool(&paths, &out, |i| geoforge_tools::intersect_with(i, &opts))
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
            overlay_tool(&paths, &out, |i| geoforge_tools::union_with(i, join, gaps))
        })(),
        Tool::Erase {
            in_features,
            erase_features,
            out_feature_class,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let (er, _) = open_feature_class(&erase_features)?;
            write_output(
                geoforge_tools::erase(fc.as_ref(), er.as_ref())?,
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
            let opts = geoforge_tools::IdentityOptions {
                join_attributes: join_attributes_arg(&join_attributes)?,
                keep_relationships: match relationship.to_ascii_uppercase().as_str() {
                    "NO_RELATIONSHIPS" => false,
                    "KEEP_RELATIONSHIPS" => true,
                    other => return Err(Error::Unsupported(format!("relationship {other}"))),
                },
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let (id, _) = open_feature_class(&identity_features)?;
            let out = geoforge_tools::identity_with(
                geoforge_tools::NamedInput {
                    name: dataset_name(&in_features),
                    features: fc.as_ref(),
                },
                geoforge_tools::NamedInput {
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
        } => {
            let out = paths.pop().unwrap();
            merge_tool(&paths, &out, &add_source)
        }
        Tool::Near {
            in_features,
            near_features,
            out_feature_class,
            search_radius,
            location,
            angle,
        } => near_tool(
            &in_features,
            &near_features,
            out_feature_class.as_deref(),
            search_radius.as_deref(),
            &location,
            &angle,
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
        } => (|| {
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
                    let rule = geoforge_tools::MergeRule::parse(rule, delim)
                        .ok_or_else(|| Error::Unsupported(format!("merge rule {rule}")))?;
                    Ok((field.to_owned(), rule))
                })
                .collect::<Result<Vec<_>>>()?;
            let (target, _) = open_feature_class(&target_features)?;
            let (join, _) = open_feature_class(&join_features)?;
            let opts = geoforge_tools::SpatialJoinOptions {
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
                match_option: geoforge_tools::MatchOption::parse(&match_option)
                    .ok_or_else(|| Error::Unsupported(format!("match_option {match_option}")))?,
                search_radius: search_radius
                    .as_deref()
                    .map(|r| parse_distance(r, target.spatial_reference_wkt()))
                    .transpose()?,
                distance_field_name,
                merge_rules,
            };
            let out = geoforge_tools::spatial_join(target.as_ref(), join.as_ref(), &opts)?;
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
            let overlap = geoforge_tools::Overlap::parse(&overlap_type)
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
            let out = geoforge_tools::select_by_location(
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
            let refs: Vec<&dyn FeatureClass> = inputs.iter().map(|b| b.as_ref()).collect();
            let out = geoforge_tools::append(&refs, target.as_ref(), test)?;
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
        Tool::Select {
            in_features,
            out_feature_class,
            where_clause,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = geoforge_tools::select(fc.as_ref(), where_clause.as_deref())?;
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
                let stat = geoforge_tools::Statistic::parse(stat)
                    .ok_or_else(|| Error::Unsupported(format!("statistic type {stat}")))?;
                stats.push((field.to_owned(), stat));
            }
            let (fc, _) = open_feature_class(&in_table)?;
            let out = geoforge_tools::statistics(fc.as_ref(), &stats, &case_fields)?;
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
            let out = geoforge_tools::frequency(
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
                    geoforge_tools::GeometryProperty::parse(p)
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
            let out = geoforge_tools::add_geometry_attributes(fc.as_ref(), &props, length, area)?;
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
            let out = geoforge_tools::points_to_line(
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
        } => (|| {
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
                .map(|r| parse_distance(r, fc.spatial_reference_wkt()))
                .transpose()?;
            let out = geoforge_tools::generate_near_table(
                fc.as_ref(),
                near.as_ref(),
                radius,
                flag(&location, "LOCATION", "NO_LOCATION")?,
                flag(&angle, "ANGLE", "NO_ANGLE")?,
                flag(&closest, "ALL", "CLOSEST")?,
                closest_count,
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
            let out = geoforge_tools::find_identical(fc.as_ref(), &names, only)?;
            write_table(&out, &out_table)
        })(),
        Tool::MultipartToSinglepart {
            in_features,
            out_feature_class,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = geoforge_tools::multipart_to_singlepart(fc.as_ref())?;
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
            let out = geoforge_tools::feature_to_point(fc.as_ref(), inside)?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::FeatureVerticesToPoints {
            in_features,
            out_feature_class,
            point_location,
        } => (|| {
            let location = geoforge_tools::VertexLocation::parse(&point_location)
                .ok_or_else(|| Error::Unsupported(format!("point_location {point_location}")))?;
            let (fc, _) = open_feature_class(&in_features)?;
            let out = geoforge_tools::feature_vertices_to_points(fc.as_ref(), location)?;
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
            use geoforge_geometry::BoundingKind as K;
            let kind = match geometry_type.to_ascii_uppercase().as_str() {
                "RECTANGLE_BY_AREA" => K::RectangleByArea,
                "RECTANGLE_BY_WIDTH" => K::RectangleByWidth,
                "CONVEX_HULL" => K::ConvexHull,
                "CIRCLE" => K::Circle,
                "ENVELOPE" => K::Envelope,
                other => return Err(Error::Unsupported(format!("geometry_type {other}"))),
            };
            let group = match group_option.to_ascii_uppercase().as_str() {
                "NONE" => geoforge_tools::BoundingGroup::None,
                "ALL" => geoforge_tools::BoundingGroup::All,
                "LIST" if !group_fields.is_empty() => {
                    geoforge_tools::BoundingGroup::List(group_fields)
                }
                other => return Err(Error::Unsupported(format!("group_option {other}"))),
            };
            let mbg = match mbg_fields_option.to_ascii_uppercase().as_str() {
                "NO_MBG_FIELDS" => false,
                "MBG_FIELDS" => true,
                other => return Err(Error::Unsupported(format!("mbg_fields_option {other}"))),
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let out = geoforge_tools::minimum_bounding_geometry(fc.as_ref(), kind, &group, mbg)?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::AddXy {
            in_features,
            output,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let out = geoforge_tools::add_xy(fc.as_ref())?;
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
                .and_then(|w| geoforge_proj::Crs::from_wkt(w).ok())
                .and_then(|c| c.wkid());
            let d = geoforge_json::Dataset {
                kind,
                oid_name: fc.oid_field_name(),
                fields: fc.fields(),
                features: &features,
                wkid,
                z: flag(&include_z_values, "Z_VALUES", "NO_Z_VALUES")? && fc.has_z(),
                m: flag(&include_m_values, "M_VALUES", "NO_M_VALUES")? && fc.has_m(),
            };
            let text = if geojson {
                geoforge_json::geojson(&d, formatted)
            } else {
                geoforge_json::esri_json(&d, formatted)
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
                Some("POINT") => Some(geoforge_core::GeometryKind::Point),
                Some("MULTIPOINT") => Some(geoforge_core::GeometryKind::Multipoint),
                Some("POLYLINE") => Some(geoforge_core::GeometryKind::Polyline),
                Some("POLYGON") => Some(geoforge_core::GeometryKind::Polygon),
                Some(other) => return Err(Error::Unsupported(format!("geometry_type {other}"))),
            };
            let text =
                std::fs::read_to_string(&in_json_file).map_err(|e| Error::io(&in_json_file, e))?;
            let imported = geoforge_json::read(&text, kind)?;
            let wkt = match (&imported.wkt, imported.wkid) {
                (Some(w), _) => Some(w.clone()),
                (None, Some(code)) => geoforge_proj::Crs::parse(&code.to_string())
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
            if out_table
                .extension()
                .is_none_or(|e| !e.eq_ignore_ascii_case("csv"))
            {
                return Err(Error::Unsupported("Export Table writes .csv here".into()));
            }
            let (fc, _) = open_feature_class(&in_table)?;
            let rows = fc
                .features()
                .map(|f| f.map(|f| f.attributes))
                .collect::<Result<Vec<_>>>()?;
            let n = rows.len();
            let text = geoforge_csv::write(fc.fields(), rows);
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
            let crs = geoforge_proj::Crs::parse(&coordinate_system)?;
            let (table, _) = open_feature_class(&in_table)?;
            let out = geoforge_tools::xy_table_to_point(
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
        } => (|| {
            let target = geoforge_proj::Crs::parse(&out_coor_system)?;
            let (fc, _) = open_feature_class(&in_dataset)?;
            let out = geoforge_tools::project(fc.as_ref(), &target)?;
            write_output(out, &out_dataset, source_gdb(&in_dataset).as_deref())
        })(),
        Tool::AddSpatialIndex { in_features } => (|| {
            let Location::Shapefile(shp) = locate(&in_features) else {
                return Err(Error::Unsupported(
                    "geodatabase feature classes always have a spatial index".into(),
                ));
            };
            let bytes = std::fs::read(&shp).map_err(|e| Error::io(&shp, e))?;
            let (records, extent, entries) = geoforge_shapefile::sbn::entries_from_shp(&bytes)?;
            geoforge_shapefile::sbn::write(&shp, records, extent, &entries)?;
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
                "DEGREE" => geoforge_raster::surface::SlopeUnit::Degree,
                "PERCENT_RISE" => geoforge_raster::surface::SlopeUnit::PercentRise,
                other => return Err(Error::Unsupported(format!("output measurement {other}"))),
            };
            let dem = geoforge_raster::read_geotiff(&in_raster)?;
            let out = geoforge_raster::surface::slope(&dem, unit, z_factor);
            write_raster(&out_raster, &out)
        })(),
        Tool::FocalStatistics {
            in_raster,
            out_raster,
            neighborhood,
            statistics_type,
            ignore_nodata,
        } => (|| {
            use geoforge_raster::focal::{FocalStatistic, Neighborhood};
            let nbr = Neighborhood::parse(&neighborhood)
                .ok_or_else(|| Error::Unsupported(format!("neighborhood {neighborhood}")))?;
            let stat = FocalStatistic::parse(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistic {statistics_type}")))?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            let input = geoforge_raster::read_geotiff(&in_raster)?;
            let out = geoforge_raster::focal::focal_statistics(&input, nbr, stat, ignore);
            write_raster(&out_raster, &out)
        })(),
        Tool::Fill {
            in_surface_raster,
            out_surface_raster,
            z_limit,
        } => (|| {
            let dem = geoforge_raster::read_geotiff(&in_surface_raster)?;
            write_raster(
                &out_surface_raster,
                &geoforge_raster::hydro::fill(&dem, z_limit),
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
            let dem = geoforge_raster::read_geotiff(&in_surface_raster)?;
            write_raster(
                &out_flow_direction_raster,
                &geoforge_raster::hydro::flow_direction(&dem, force),
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
            let dirs = geoforge_raster::read_geotiff(&in_flow_direction_raster)?;
            let weight = in_weight_raster
                .as_deref()
                .map(geoforge_raster::read_geotiff)
                .transpose()?;
            write_raster(
                &out_accumulation_raster,
                &geoforge_raster::hydro::flow_accumulation(&dirs, weight.as_ref(), integer),
            )
        })(),
        Tool::Reclassify {
            in_raster,
            remap,
            out_raster,
            missing_values,
        } => (|| {
            let entries = geoforge_raster::zonal::parse_remap(&remap)
                .ok_or_else(|| Error::Unsupported(format!("remap {remap}")))?;
            let keep = match missing_values.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("missing_values {other}"))),
            };
            let input = geoforge_raster::read_geotiff(&in_raster)?;
            write_raster(
                &out_raster,
                &geoforge_raster::zonal::reclassify(&input, &entries, keep),
            )
        })(),
        Tool::ZonalStatistics {
            in_zone_data,
            in_value_raster,
            out_raster,
            statistics_type,
            ignore_nodata,
        } => (|| {
            let stat = geoforge_raster::zonal::ZonalStatistic::parse(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistic {statistics_type}")))?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            let zones = geoforge_raster::read_geotiff(&in_zone_data)?;
            let values = geoforge_raster::read_geotiff(&in_value_raster)?;
            let (zones, values) = geoforge_raster::resample::zones_on_value_grid(&zones, &values)
                .ok_or_else(|| {
                Error::Unsupported("zone and value rasters do not overlap".into())
            })?;
            write_raster(
                &out_raster,
                &geoforge_raster::zonal::zonal_statistics(&zones, &values, stat, ignore),
            )
        })(),
        Tool::Curvature {
            in_raster,
            out_curvature_raster,
            z_factor,
            out_profile_curve_raster,
            out_plan_curve_raster,
        } => (|| {
            let dem = geoforge_raster::read_geotiff(&in_raster)?;
            let (curv, profile, plan) = geoforge_raster::surface::curvature(&dem, z_factor);
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
        } => math_tool(
            geoforge_raster::local::MathOp::Plus,
            &in1,
            &in2,
            &out_raster,
        ),
        Tool::Minus {
            in1,
            in2,
            out_raster,
        } => math_tool(
            geoforge_raster::local::MathOp::Minus,
            &in1,
            &in2,
            &out_raster,
        ),
        Tool::Times {
            in1,
            in2,
            out_raster,
        } => math_tool(
            geoforge_raster::local::MathOp::Times,
            &in1,
            &in2,
            &out_raster,
        ),
        Tool::Divide {
            in1,
            in2,
            out_raster,
        } => math_tool(
            geoforge_raster::local::MathOp::Divide,
            &in1,
            &in2,
            &out_raster,
        ),
        Tool::Int {
            in_raster,
            out_raster,
        } => geoforge_raster::read_geotiff(&in_raster)
            .and_then(|r| write_raster(&out_raster, &geoforge_raster::local::int(&r))),
        Tool::Float {
            in_raster,
            out_raster,
        } => geoforge_raster::read_geotiff(&in_raster)
            .and_then(|r| write_raster(&out_raster, &geoforge_raster::local::float(&r))),
        Tool::IsNull {
            in_raster,
            out_raster,
        } => geoforge_raster::read_geotiff(&in_raster)
            .and_then(|r| write_raster(&out_raster, &geoforge_raster::local::is_null(&r))),
        Tool::Con {
            in_conditional_raster,
            in_true_raster_or_constant,
            out_raster,
            in_false_raster_or_constant,
            where_clause,
        } => (|| {
            let cond = geoforge_raster::read_geotiff(&in_conditional_raster)?;
            let t = operand(&in_true_raster_or_constant)?;
            let f = in_false_raster_or_constant
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(operand)
                .transpose()?;
            let clause = value_clause(where_clause.as_deref())?;
            let out = geoforge_raster::local::con(&cond, &t, f.as_ref(), clause.as_ref())
                .ok_or_else(|| Error::Unsupported("input rasters do not overlap".into()))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::SetNull {
            in_conditional_raster,
            in_false_raster_or_constant,
            out_raster,
            where_clause,
        } => (|| {
            let cond = geoforge_raster::read_geotiff(&in_conditional_raster)?;
            let f = operand(&in_false_raster_or_constant)?;
            let clause = value_clause(where_clause.as_deref())?;
            let out = geoforge_raster::local::set_null(&cond, &f, clause.as_ref())
                .ok_or_else(|| Error::Unsupported("input rasters do not overlap".into()))?;
            write_raster(&out_raster, &out)
        })(),
        Tool::Contour {
            in_raster,
            out_polyline_features,
            contour_interval,
            base_contour,
        } => (|| {
            let r = geoforge_raster::read_geotiff(&in_raster)?;
            let lines = geoforge_raster::contour::contour(&r, contour_interval, base_contour);
            let features = lines
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    let mut g = geoforge_core::Geometry::point(0.0, 0.0);
                    g.kind = geoforge_core::GeometryKind::Polyline;
                    g.parts = vec![0];
                    g.xy = l.points;
                    Feature {
                        oid: i as i64,
                        geometry: Some(g),
                        attributes: vec![Value::Integer(i as i32 + 1), Value::Double(l.value)],
                    }
                })
                .collect();
            let wkt = raster_wkt(&r)?;
            let out = ToolOutput {
                kind: geoforge_core::GeometryKind::Polyline,
                has_z: false,
                has_m: false,
                fields: vec![
                    Field::new("Id", FieldType::Integer),
                    Field::new("Contour", FieldType::Double),
                ],
                spatial_reference_wkt: wkt,
                features,
                messages: Vec::new(),
            };
            write_output(out, &out_polyline_features, None)
        })(),
        Tool::Basin {
            in_flow_direction_raster,
            out_raster,
        } => geoforge_raster::read_geotiff(&in_flow_direction_raster)
            .and_then(|d| write_raster(&out_raster, &geoforge_raster::hydro::basin(&d))),
        Tool::Watershed {
            in_flow_direction_raster,
            in_pour_point_data,
            out_raster,
        } => (|| {
            let d = geoforge_raster::read_geotiff(&in_flow_direction_raster)?;
            let p = geoforge_raster::read_geotiff(&in_pour_point_data)?;
            write_raster(&out_raster, &geoforge_raster::hydro::watershed(&d, &p))
        })(),
        Tool::StreamLink {
            in_stream_raster,
            in_flow_direction_raster,
            out_raster,
        } => (|| {
            let s = geoforge_raster::read_geotiff(&in_stream_raster)?;
            let d = geoforge_raster::read_geotiff(&in_flow_direction_raster)?;
            write_raster(&out_raster, &geoforge_raster::hydro::stream_link(&s, &d))
        })(),
        Tool::StreamOrder {
            in_stream_raster,
            in_flow_direction_raster,
            out_raster,
            order_method,
        } => (|| {
            use geoforge_raster::hydro::StreamOrder;
            let method = match order_method.to_ascii_uppercase().as_str() {
                "STRAHLER" => StreamOrder::Strahler,
                "SHREVE" => StreamOrder::Shreve,
                other => return Err(Error::Unsupported(format!("order method {other}"))),
            };
            let s = geoforge_raster::read_geotiff(&in_stream_raster)?;
            let d = geoforge_raster::read_geotiff(&in_flow_direction_raster)?;
            write_raster(
                &out_raster,
                &geoforge_raster::hydro::stream_order(&s, &d, method),
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
            let d = geoforge_raster::read_geotiff(&in_flow_direction_raster)?;
            write_raster(
                &out_raster,
                &geoforge_raster::hydro::flow_length(&d, upstream),
            )
        })(),
        Tool::RasterToPoint {
            in_raster,
            out_point_features,
        } => (|| {
            let r = geoforge_raster::read_geotiff(&in_raster)?;
            let float = r.data_type.is_float();
            let features = geoforge_raster::conversion::raster_to_points(&r)
                .into_iter()
                .enumerate()
                .map(|(i, (p, v))| Feature {
                    oid: i as i64,
                    geometry: Some(geoforge_core::Geometry::point(p[0], p[1])),
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
                kind: geoforge_core::GeometryKind::Point,
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
            let r = geoforge_raster::read_geotiff(&in_raster)?;
            if r.data_type.is_float() {
                return Err(Error::Unsupported(
                    "ERROR 010247: The input raster must be integer".into(),
                ));
            }
            let features = geoforge_raster::conversion::raster_to_polygons(&r)
                .into_iter()
                .enumerate()
                .map(|(i, poly)| {
                    let mut g = geoforge_core::Geometry::point(0.0, 0.0);
                    g.kind = geoforge_core::GeometryKind::Polygon;
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
                kind: geoforge_core::GeometryKind::Polygon,
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
            use geoforge_raster::rasterize::*;
            let (features, kind, epsg, geometry) = rasterize_input(&in_features, &field)?;
            let r = match geometry {
                Some(geoforge_core::GeometryKind::Polygon) => {
                    polygons_to_raster(&features, cell_size, PolygonAssignment::CellCenter, kind)
                }
                Some(geoforge_core::GeometryKind::Polyline) => {
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
            use geoforge_raster::rasterize::*;
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
            use geoforge_raster::rasterize::*;
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
        Tool::PointToRaster {
            in_features,
            value_field,
            out_raster,
            cell_assignment,
            cellsize,
        } => (|| {
            use geoforge_raster::rasterize::*;
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
            use geoforge_raster::resample::Technique;
            let t = match resampling_type.to_ascii_uppercase().as_str() {
                "NEAREST" => Technique::Nearest,
                "BILINEAR" => Technique::Bilinear,
                "CUBIC" => Technique::Cubic,
                "MAJORITY" => Technique::Majority,
                other => return Err(Error::Unsupported(format!("resampling type {other}"))),
            };
            let r = geoforge_raster::read_geotiff(&in_raster)?;
            write_raster(
                &out_raster,
                &geoforge_raster::resample::resample(&r, cell_size, t),
            )
        })(),
        Tool::EucDistance {
            in_source_data,
            out_distance_raster,
            maximum_distance,
        } => (|| {
            let r = geoforge_raster::read_geotiff(&in_source_data)?;
            write_raster(
                &out_distance_raster,
                &geoforge_raster::resample::euclidean_distance(&r, maximum_distance),
            )
        })(),
        Tool::ExtractByMask {
            in_raster,
            in_mask_data,
            out_raster,
        } => (|| {
            let r = geoforge_raster::read_geotiff(&in_raster)?;
            let m = geoforge_raster::read_geotiff(&in_mask_data)?;
            let out = geoforge_raster::resample::extract_by_mask(&r, &m)
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
            let stats = geoforge_raster::zonal::table_statistics(&statistics_type)
                .ok_or_else(|| Error::Unsupported(format!("statistic {statistics_type}")))?;
            let ignore = match ignore_nodata.to_ascii_uppercase().as_str() {
                "DATA" => true,
                "NODATA" => false,
                other => return Err(Error::Unsupported(format!("ignore_nodata {other}"))),
            };
            let zones = geoforge_raster::read_geotiff(&in_zone_data)?;
            let values = geoforge_raster::read_geotiff(&in_value_raster)?;
            let (zones, values) = geoforge_raster::resample::zones_on_value_grid(&zones, &values)
                .ok_or_else(|| {
                Error::Unsupported("zone and value rasters do not overlap".into())
            })?;
            let (fields, rows) =
                geoforge_raster::zonal::zonal_table(&zones, &values, &stats, ignore);
            write_table(&geoforge_tools::TableOutput { fields, rows }, &out_table)
        })(),
        Tool::Viewshed {
            in_raster,
            in_observer_features,
            out_raster,
            z_factor,
        } => (|| {
            let dem = geoforge_raster::read_geotiff(&in_raster)?;
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
                    observers.push(geoforge_raster::surface::Observer {
                        x: p[0],
                        y: p[1],
                        offset,
                    });
                }
            }
            write_raster(
                &out_raster,
                &geoforge_raster::surface::viewshed(&dem, &observers, z_factor),
            )
        })(),
        Tool::Aspect {
            in_raster,
            out_raster,
        } => (|| {
            let dem = geoforge_raster::read_geotiff(&in_raster)?;
            write_raster(&out_raster, &geoforge_raster::surface::aspect(&dem))
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
            let dem = geoforge_raster::read_geotiff(&in_raster)?;
            let out = geoforge_raster::surface::hillshade_with(
                &dem, azimuth, altitude, z_factor, shadows,
            );
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
            let out = geoforge_tools::repair_geometry(fc.as_ref(), delete_null, method)?;
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
        } => buffer_tool(
            &in_features,
            &out_feature_class,
            &buffer_distance,
            &line_side,
            &line_end_type,
            &dissolve_option,
            dissolve_fields,
        ),
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
        } if open_feature_class(in_features).is_ok_and(|(fc, _)| {
            fc.geometry_kind() != Some(geoforge_core::GeometryKind::Point)
        }) =>
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
        let table = geoforge_gpkg::GeoPackage::open(&gpkg)?.open_table(&item)?;
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
        return Ok((Box::new(geoforge_csv::CsvTable::open(path)?), "TextFile"));
    }
    match locate(path) {
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
                geoforge_filegdb::DomainKind::Coded(v) => format!("coded, {} values", v.len()),
                geoforge_filegdb::DomainKind::Range { min, max } => {
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
fn write_output(mut out: ToolOutput, output: &Path, source_gdb: Option<&Path>) -> Result<()> {
    for m in &out.messages {
        eprintln!("{m}");
    }
    // ArcGIS writes its own WKT of the coordinate system, not the input's text.
    out.spatial_reference_wkt = out
        .spatial_reference_wkt
        .map(|w| geoforge_proj::Crs::from_wkt(&w).map_or(w, |crs| crs.arcgis_wkt()));
    if geopackage_item(output).is_some() {
        return Err(Error::Unsupported(
            "writing to a GeoPackage (read only so far)".into(),
        ));
    }
    match locate(output) {
        Location::Geodatabase { gdb, item } => {
            let item = item.ok_or_else(|| {
                Error::Unsupported("name the output feature class inside the .gdb".into())
            })?;
            write_geodatabase(out, &gdb, &item, source_gdb)
        }
        Location::Shapefile(path) => {
            let n = write_shapefile(out, &path)?;
            println!("Wrote {n} features to {}", path.display());
            Ok(())
        }
    }
}

fn write_geodatabase(
    out: ToolOutput,
    gdb: &Path,
    name: &str,
    source_gdb: Option<&Path>,
) -> Result<()> {
    if gdb.exists() {
        return Err(Error::Unsupported(format!(
            "{} exists; adding to an existing geodatabase is not implemented yet",
            gdb.display()
        )));
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
    let mut w = geoforge_filegdb::GeodatabaseWriter::create(gdb)?;
    // Like ArcGIS, carry over the attribute domains the copied fields use.
    if let Some(source) = source_gdb {
        let src = Geodatabase::open(source)?;
        let mut added: Vec<&str> = Vec::new();
        for name in fields.iter().filter_map(|f| f.domain.as_deref()) {
            if let Some(d) = src.domain(name).filter(|_| !added.contains(&name)) {
                w.add_domain(d)?;
                added.push(name);
            }
        }
    }
    let mut t = w.create_table(
        name,
        Some(out.kind),
        out.has_z,
        out.has_m,
        &fields,
        out.spatial_reference_wkt.as_deref(),
    )?;
    let n = out.features.len();
    for f in &out.features {
        t.write(f)?;
    }
    w.finish_table(t)?;
    w.finish()?;
    println!("Wrote {n} features to {}", gdb.join(name).display());
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
    let out = geoforge_tools::clip(fc.as_ref(), clip.as_ref())?;
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

fn overlay_tool(
    inputs: &[PathBuf],
    output: &Path,
    tool: impl Fn(&[geoforge_tools::NamedInput]) -> Result<ToolOutput>,
) -> Result<()> {
    let opened: Vec<Box<dyn FeatureClass>> = inputs
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let named: Vec<geoforge_tools::NamedInput> = inputs
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::NamedInput {
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
) -> Result<()> {
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
    let near_inputs: Vec<geoforge_tools::PathInput> = near_paths
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::PathInput {
            source: catalog_path(p),
            features: f.as_ref(),
        })
        .collect();
    let radius = search_radius
        .map(|r| parse_distance(r, fc.spatial_reference_wkt()))
        .transpose()?;
    let out = geoforge_tools::near(fc.as_ref(), &near_inputs, radius, location, angle)?;
    drop(fc);
    if let Some(output) = output {
        return write_output(out, output, source_gdb(input).as_deref());
    }
    update_in_place(out, input)
}

/// Replaces a shapefile with a tool's output (tools that edit their input, like
/// Near and Append): written next to it, then moved over its files.
fn update_in_place(out: ToolOutput, dataset: &Path) -> Result<()> {
    let Location::Shapefile(shp) = locate(dataset) else {
        return Err(Error::Unsupported(
            "updating a geodatabase feature class in place is not implemented yet; give an output"
                .into(),
        ));
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

/// A dataset's full path, as ArcGIS records it in fields like `MERGE_SRC`.
fn catalog_path(p: &Path) -> String {
    std::path::absolute(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .display()
        .to_string()
}

fn join_attributes_arg(s: &str) -> Result<geoforge_tools::JoinAttributes> {
    geoforge_tools::JoinAttributes::parse(s)
        .ok_or_else(|| Error::Unsupported(format!("join_attributes {s}")))
}

fn validation_method_arg(s: &str) -> Result<geoforge_geometry::ValidationMethod> {
    match s.to_ascii_uppercase().as_str() {
        "ESRI" => Ok(geoforge_geometry::ValidationMethod::Esri),
        "OGC" => Ok(geoforge_geometry::ValidationMethod::Ogc),
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
    let inputs: Vec<geoforge_tools::CheckInput> = paths
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::CheckInput {
            source: catalog_path(p),
            features: f.as_ref(),
            has_spatial_index: match locate(p) {
                Location::Shapefile(shp) => shp.with_extension("sbn").exists(),
                Location::Geodatabase { .. } => true,
            },
        })
        .collect();
    let (rows, messages) = geoforge_tools::check_geometry(&inputs, method)?;
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
    let fields = geoforge_tools::check_geometry_fields();
    let dbf_fields = fields
        .iter()
        .map(|f| geoforge_shapefile::dbf::DbfField::from_field(f.name.clone(), f))
        .collect::<Result<Vec<_>>>()?;
    let mut w = geoforge_shapefile::dbf::DbfWriter::create(
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
fn write_table(table: &geoforge_tools::TableOutput, path: &Path) -> Result<()> {
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("dbf"))
    {
        return Err(Error::Unsupported(
            "tables are written as .dbf; geodatabase tables are not implemented yet".into(),
        ));
    }
    let names =
        geoforge_shapefile::validate_field_names(table.fields.iter().map(|f| f.name.as_str()));
    let dbf_fields = table
        .fields
        .iter()
        .zip(names)
        .map(|(f, n)| geoforge_shapefile::dbf::DbfField::from_field(n, f))
        .collect::<Result<Vec<_>>>()?;
    let mut w = geoforge_shapefile::dbf::DbfWriter::create(
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
    for (value, out) in geoforge_tools::split(fc.as_ref(), sf.as_ref(), split_field)? {
        let name: String = value
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
            .collect();
        write_output(out, &out_workspace.join(format!("{name}.shp")), None)?;
    }
    Ok(())
}

fn merge_tool(inputs: &[PathBuf], output: &Path, add_source: &str) -> Result<()> {
    let add = match add_source.to_ascii_uppercase().as_str() {
        "NO_SOURCE_INFO" => false,
        "ADD_SOURCE_INFO" => true,
        other => return Err(Error::Unsupported(format!("add_source {other}"))),
    };
    let opened: Vec<Box<dyn FeatureClass>> = inputs
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let merge_inputs: Vec<geoforge_tools::PathInput> = inputs
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::PathInput {
            source: catalog_path(p),
            features: f.as_ref(),
        })
        .collect();
    let out = geoforge_tools::merge(&merge_inputs, add)?;
    write_output(out, output, source_gdb(&inputs[0]).as_deref())
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
        let stat = geoforge_tools::Statistic::parse(stat)
            .ok_or_else(|| Error::Unsupported(format!("statistic type {stat}")))?;
        stats.push((field.to_owned(), stat));
    }
    let multi = match multi_part.to_ascii_uppercase().as_str() {
        "MULTI_PART" => true,
        "SINGLE_PART" => false,
        other => return Err(Error::Unsupported(format!("multi_part {other}"))),
    };
    let out = geoforge_tools::dissolve(fc.as_ref(), fields, &stats, multi)?;
    write_output(out, output, source_gdb(input).as_deref())
}

/// Parses an ArcGIS linear distance ("25", "25 Meters", "0.5 Kilometers") into the
/// units of `wkt` (the last `UNIT[...]` of a projected system).
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
        geoforge_tools::BufferDistance::Linear(parse_distance(
            distance,
            fc.spatial_reference_wkt(),
        )?)
    } else {
        geoforge_tools::BufferDistance::Field(distance.trim().to_owned())
    };
    let side = match line_side.to_ascii_uppercase().as_str() {
        "FULL" => geoforge_geometry::LineSide::Full,
        "LEFT" => geoforge_geometry::LineSide::Left,
        "RIGHT" => geoforge_geometry::LineSide::Right,
        "OUTSIDE_ONLY" => geoforge_geometry::LineSide::OutsideOnly,
        other => return Err(Error::Unsupported(format!("line_side {other}"))),
    };
    let line_end = match line_end.to_ascii_uppercase().as_str() {
        "ROUND" => geoforge_geometry::LineEnd::Round,
        "FLAT" => geoforge_geometry::LineEnd::Flat,
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
    let out = geoforge_tools::buffer(fc.as_ref(), distance, line_end, side, dissolve)?;
    write_output(out, output, source_gdb(input).as_deref())
}

/// Writes a raster result as GeoTIFF the way ArcGIS writes Spatial Analyst
/// output: 128×128 tiles, LZW for integer cells, uncompressed floats.
fn write_raster(path: &Path, raster: &geoforge_raster::Raster) -> Result<()> {
    let compression = if raster.data_type.is_float() {
        geoforge_raster::Compression::None
    } else {
        geoforge_raster::Compression::Lzw
    };
    geoforge_raster::write_geotiff_with(
        path,
        raster,
        &geoforge_raster::WriteOptions {
            compression,
            tile: Some((128, 128)),
            ..Default::default()
        },
    )?;
    println!(
        "Wrote {} ({}x{})",
        path.display(),
        raster.width,
        raster.height
    );
    Ok(())
}

/// A raster path or a numeric constant.
fn operand(text: &str) -> Result<geoforge_raster::local::Operand> {
    match geoforge_raster::local::Operand::constant(text) {
        Some(c) => Ok(c),
        None => Ok(geoforge_raster::local::Operand::Raster(
            geoforge_raster::read_geotiff(Path::new(text))?,
        )),
    }
}

fn value_clause(text: Option<&str>) -> Result<Option<geoforge_raster::local::ValueClause>> {
    match text.filter(|t| !t.trim().is_empty()) {
        None => Ok(None),
        Some(t) => geoforge_raster::local::ValueClause::parse(t)
            .map(Some)
            .ok_or_else(|| Error::Unsupported(format!("where clause {t}"))),
    }
}

fn math_tool(op: geoforge_raster::local::MathOp, a: &str, b: &str, out: &Path) -> Result<()> {
    let (a, b) = (operand(a)?, operand(b)?);
    let result = geoforge_raster::local::math(op, &a, &b)
        .ok_or_else(|| Error::Unsupported("at least one input must be a raster".into()))?;
    write_raster(out, &result)
}

/// The ArcGIS WKT of a raster's EPSG coordinate system, if it has one.
fn raster_wkt(r: &geoforge_raster::Raster) -> Result<Option<String>> {
    r.epsg
        .map(|code| geoforge_proj::Crs::parse(&code.to_string()).map(|c| c.arcgis_wkt()))
        .transpose()
}

type RasterizeInput = (
    Vec<(geoforge_core::Geometry, f64)>,
    geoforge_raster::rasterize::ValueKind,
    Option<u32>,
    Option<geoforge_core::GeometryKind>,
);

/// Shapes with the numeric value of `field` (the OID field included), the kind
/// of that value, and the EPSG code of the features' coordinate system.
fn rasterize_input(path: &Path, field: &str) -> Result<RasterizeInput> {
    use geoforge_raster::rasterize::ValueKind;
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
        .and_then(|w| geoforge_proj::Crs::from_wkt(w).ok())
        .and_then(|c| c.wkid())
        .map(|(_, latest)| latest);
    Ok((features, kind, epsg, fc.geometry_kind()))
}
