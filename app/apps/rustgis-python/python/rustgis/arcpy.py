"""An arcpy-shaped API over GeoForge's ArcGIS-verified geoprocessing tools.

Modified for GeoForge (not part of upstream GeoForge).

The tools keep ArcGIS Pro's names, positional parameters and keywords, so code written against
``arcpy.analysis``, ``arcpy.management``, ``arcpy.sa`` and ``arcpy.conversion`` carries over:

>>> from rustgis import arcpy
>>> arcpy.use(rustgis.Session(sample=True))
>>> r = arcpy.analysis.Buffer("Cities", "Cities_100km", "100 Kilometers")
>>> r.getOutput(0)
'Cities_100km'

Inputs are layer names of the session, ``Layer`` objects, or paths of files (added as layers on
the way). An output that is a path (``out.shp``, ``city.gdb/roads``, ``dem2.tif``) is written
there as well as added to the session. Tools return a :class:`Result`.

What is not arcpy: the data model is the session's (longitude/latitude layers); distances in a
linear unit are measured on the ground; ``arcpy.sa`` functions take the output name as the keyword
``out_raster`` and return a :class:`Result`, not a ``Raster`` object; ``env`` holds
``workspace``, ``overwriteOutput`` and the raster environments ``cellSize`` (a number), ``extent``
(``"xmin ymin xmax ymax"``, a 4-sequence or an object with ``XMin``..``YMax``), ``snapRaster`` and
``mask`` (a raster or polygon layer), which ``arcpy.sa`` functions honour.
"""

from __future__ import annotations

import os
import re
from typing import Any, Callable, Dict, List, Optional, Sequence, Tuple, Union

from . import Layer, GeoForgeError, Session

__all__ = ["Result", "env", "use", "session", "analysis", "management", "sa", "conversion", "ListTools"]

_PREFIX = "geoforge:"


class _Env:
    """The geoprocessing environment (the settings tools read)."""

    def __init__(self) -> None:
        self.workspace: Optional[str] = None
        self.overwriteOutput: bool = False
        self.cellSize: Any = None
        self.extent: Any = None
        self.snapRaster: Any = None
        self.mask: Any = None


env = _Env()
_session: Optional[Session] = None


def use(s: Session) -> Session:
    """Makes ``s`` the session the tools run in."""
    global _session
    _session = s
    return s


def session() -> Session:
    """The session the tools run in (an empty one is made on first use)."""
    global _session
    if _session is None:
        _session = Session()
    return _session


class Result:
    """What a tool returned: its outputs and messages (arcpy's ``Result``)."""

    def __init__(self, outputs: List[str], messages: List[Tuple[str, bool]], raw: Any) -> None:
        self._outputs = outputs
        self._messages = messages
        self.raw = raw

    @property
    def outputCount(self) -> int:
        return len(self._outputs)

    @property
    def status(self) -> int:
        """4 (succeeded): a failing tool raises instead."""
        return 4

    def getOutput(self, index: int = 0) -> str:
        return self._outputs[index]

    def getMessages(self, severity: int = 0) -> str:
        return "\n".join(t for t, is_error in self._messages if severity != 2 or is_error)

    def __getitem__(self, index: int) -> str:
        return self._outputs[index]

    def __iter__(self):
        return iter(self._outputs)

    def __str__(self) -> str:
        return self._outputs[0] if self._outputs else ""

    def __repr__(self) -> str:
        return f"Result({self._outputs!r})"


# Parameter conversion ---------------------------------------------------------------------------


def _join(value: Any) -> str:
    """A list as ``a;b;c`` (field lists, input lists)."""
    if value is None:
        return ""
    if isinstance(value, str):
        return value
    return ";".join(str(_layer_name(v)) for v in value)


def _statistics(value: Any) -> str:
    """``[["POP", "SUM"], ["AREA", "MEAN"]]`` or ``"POP SUM;AREA MEAN"``."""
    if value is None or isinstance(value, str):
        return value or ""
    return ";".join(" ".join(str(x) for x in pair) for pair in value)


def _sort_fields(value: Any) -> str:
    if value is None or isinstance(value, str):
        return value or ""
    return ";".join(" ".join(x) if not isinstance(x, str) else x for x in value)


_UNITS = {
    "METERS": "METERS", "METER": "METERS", "KILOMETERS": "KILOMETERS", "KILOMETER": "KILOMETERS",
    "MILES": "MILES", "MILE": "MILES", "FEET": "FEET", "FOOT": "FEET", "YARDS": "YARDS", "YARD": "YARDS",
    "NAUTICALMILES": "NAUTICAL_MILES", "NAUTICAL_MILES": "NAUTICAL_MILES",
}


def _distance(value: Any) -> Dict[str, Any]:
    """``"100 Kilometers"`` as the tool's ``distance`` and ``units``; a plain number as it is."""
    if isinstance(value, (int, float)):
        return {"distance": value}
    text = str(value).strip()
    m = re.match(r"^([-+0-9.eE]+)\s*([A-Za-z_]*)$", text)
    if not m:
        return {"distance": text}  # a field name
    number, unit = float(m.group(1)), m.group(2).upper().replace(" ", "")
    if not unit or unit in ("UNKNOWN",):
        return {"distance": number}
    if unit not in _UNITS:
        raise GeoForgeError(f"unknown distance unit '{m.group(2)}'")
    return {"distance": number, "units": _UNITS[unit]}


def _radius(key: str) -> Callable[[Any], Dict[str, Any]]:
    def convert(value: Any) -> Dict[str, Any]:
        if value in (None, ""):
            return {}
        d = _distance(value)
        out = {key: d["distance"]}
        if "units" in d:
            out["units"] = d["units"]
        return out

    return convert


class RadiusVariable:
    """``arcpy.sa.RadiusVariable(numberofpoints=12, maxdistance=None)``: the nearest points."""

    def __init__(self, numberofpoints: int = 12, maxdistance: Any = None) -> None:
        self.numberofpoints = numberofpoints
        self.maxdistance = maxdistance


class RadiusFixed:
    """``arcpy.sa.RadiusFixed(distance=None, minnumberofpoints=0)``: the points within a distance."""

    def __init__(self, distance: Any = None, minnumberofpoints: int = 0) -> None:
        self.distance = distance
        self.minnumberofpoints = minnumberofpoints


def _search_radius(value: Any) -> Dict[str, Any]:
    """A ``RadiusVariable`` or ``RadiusFixed`` (or the word ``VARIABLE`` / ``FIXED``) as parameters."""
    if isinstance(value, RadiusFixed):
        out: Dict[str, Any] = {"search_type": "FIXED", "min_points": value.minnumberofpoints}
        if value.distance is not None:
            out["fixed_distance"] = value.distance
        return out
    if isinstance(value, RadiusVariable):
        out = {"search_type": "VARIABLE", "num_points": value.numberofpoints}
        if value.maxdistance is not None:
            out["max_distance"] = value.maxdistance
        return out
    word = str(value).upper()
    if word in ("VARIABLE", "FIXED"):
        return {"search_type": word}
    raise GeoForgeError(f"search_radius must be RadiusVariable or RadiusFixed, not {value!r}")


class RemapValue:
    """``arcpy.sa.RemapValue([[1, 5], [2, "NODATA"]])``: input value → scale value."""

    def __init__(self, remapTable: Any) -> None:
        self.table = remapTable


class WSTable:
    """``arcpy.sa.WSTable([[raster, "VALUE", weight], ...])``: the Weighted Sum table."""

    def __init__(self, rasterlist: Any) -> None:
        self.rows = rasterlist


class WOTable:
    """``arcpy.sa.WOTable([[raster, influence, "VALUE", RemapValue(...)], ...], [from, to, by])``."""

    def __init__(self, in_remap_table: Any, evaluation_scale: Any = None) -> None:
        self.rows = in_remap_table
        self.scale = evaluation_scale or [1, 9, 1]


class _Fuzzy:
    """A fuzzification function: its name and the parameters it gives the tool."""

    function = ""
    names: Tuple[str, ...] = ()

    def __init__(self, *args: Any) -> None:
        self.values = dict(zip(self.names, args))

    def params(self) -> Dict[str, Any]:
        return {"function": self.function, **self.values}


class FuzzyLinear(_Fuzzy):
    function, names = "LINEAR", ("minimum", "maximum")


class FuzzyGaussian(_Fuzzy):
    function, names = "GAUSSIAN", ("midpoint", "spread")


class FuzzyLarge(_Fuzzy):
    function, names = "LARGE", ("midpoint", "spread")


class FuzzySmall(_Fuzzy):
    function, names = "SMALL", ("midpoint", "spread")


class FuzzyNear(_Fuzzy):
    function, names = "NEAR", ("midpoint", "spread")


class FuzzyMSLarge(_Fuzzy):
    function, names = "MSLARGE", ("mean_multiplier", "std_multiplier")


class FuzzyMSSmall(_Fuzzy):
    function, names = "MSSMALL", ("mean_multiplier", "std_multiplier")


def _raster_list(value: Any) -> str:
    """Rasters as the tool's semicolon list (layer names, or a path as the layer it adds)."""
    names = value if isinstance(value, (list, tuple)) else [value]
    return ";".join(str(_as_layer(session(), v)) for v in names)


def _inputs(value: Any) -> Dict[str, Any]:
    return {"inputs": _raster_list(value)}


def _ws_table(value: Any) -> Dict[str, Any]:
    rows = value.rows if isinstance(value, WSTable) else value
    return {"inputs": _raster_list([r[0] for r in rows]), "weights": ";".join(str(r[2]) for r in rows)}


def _wo_table(value: Any) -> Dict[str, Any]:
    if not isinstance(value, WOTable):
        raise GeoForgeError("the Weighted Overlay table is a WOTable")
    entries = []
    for row in value.rows:
        raster, influence, _, remap = row[0], row[1], row[2], row[3]
        pairs = remap.table if isinstance(remap, RemapValue) else remap
        entries.append(f"{_as_layer(session(), raster)}:{influence}:" + ",".join(f"{a}={b}" for a, b in pairs))
    return {"table": ";".join(entries), "evaluation_scale": " ".join(str(x) for x in value.scale)}


def _fuzzy_function(value: Any) -> Dict[str, Any]:
    if not isinstance(value, _Fuzzy):
        raise GeoForgeError("the fuzzy function is FuzzyLinear, FuzzyGaussian, FuzzyLarge, ...")
    return value.params()


def _flag(true_value: str) -> Callable[[Any], bool]:
    return lambda v: isinstance(v, bool) and v or str(v).upper() == true_value


# Tool table -------------------------------------------------------------------------------------

# (arcpy parameter, our parameter or a converter returning a dict of ours, or None to ignore)
Arg = Tuple[str, Any]


def _t(module: str, name: str, tool: str, *args: Arg, outputs: Sequence[str] = ("output",)) -> Tuple[str, str, str, Tuple[Arg, ...], Tuple[str, ...]]:
    return (module, name, tool, args, tuple(outputs))


def _sel(*names: str) -> Callable[[Any], Dict[str, Any]]:
    return lambda v: {n: v for n in names}


_TOOLS = [
    # analysis
    _t("analysis", "Buffer", "buffer", ("in_features", "input"), ("out_feature_class", "output"), ("buffer_distance_or_field", _distance),
       ("line_side", "line_side"), ("line_end_type", "line_end"), ("dissolve_option", "dissolve"), ("dissolve_field", ("dissolve_fields", _join)), ("method", None)),
    _t("analysis", "Clip", "clip", ("in_features", "input"), ("clip_features", "clip"), ("out_feature_class", "output"), ("cluster_tolerance", None)),
    _t("analysis", "Erase", "erase", ("in_features", "input"), ("erase_features", "erase"), ("out_feature_class", "output"), ("cluster_tolerance", None)),
    _t("analysis", "Identity", "identity", ("in_features", "input"), ("identity_features", "other"), ("out_feature_class", "output"),
       ("join_attributes", "join_attributes"), ("cluster_tolerance", None), ("relationship", ("keep_relationships", _flag("KEEP_RELATIONSHIPS")))),
    _t("analysis", "Update", "update", ("in_features", "input"), ("update_features", "other"), ("out_feature_class", "output"), ("border", None), ("cluster_tolerance", None)),
    _t("analysis", "SymDiff", "symmetrical_difference", ("in_features", "input"), ("update_features", "other"), ("out_feature_class", "output"), ("join_attributes", None), ("cluster_tolerance", None)),
    _t("analysis", "Select", "select", ("in_features", "input"), ("out_feature_class", "output"), ("where_clause", "where")),
    _t("analysis", "Split", "split", ("in_features", "input"), ("split_features", "other"), ("split_field", "split_field"), ("out_workspace", "output"), ("cluster_tolerance", None)),
    _t("analysis", "SpatialJoin", "spatial_join", ("target_features", "input"), ("join_features", "other"), ("out_feature_class", "output"),
       ("join_operation", "join_operation"), ("join_type", ("keep_common", _flag("KEEP_COMMON"))), ("field_mapping", None), ("match_option", "match_option"),
       ("search_radius", _radius("search_radius")), ("distance_field_name", "distance_field_name")),
    _t("analysis", "Near", "near", ("in_features", "input"), ("near_features", "near"), ("search_radius", _radius("search_radius")),
       ("location", ("location", _flag("LOCATION"))), ("angle", ("angle", _flag("ANGLE"))), ("method", None), ("field_names", None), outputs=()),
    _t("analysis", "GenerateNearTable", "generate_near_table", ("in_features", "input"), ("near_features", "near"), ("out_table", "output"),
       ("search_radius", _radius("search_radius")), ("location", None), ("angle", ("angle", _flag("ANGLE"))), ("closest", ("method", lambda v: "CLOSEST" if str(v).upper() == "CLOSEST" else "ALL")),
       ("closest_count", "closest_count")),
    _t("analysis", "Frequency", "frequency", ("in_table", "input"), ("out_table", "output"), ("frequency_fields", ("fields", _join)), ("summary_fields", ("summary_fields", _join))),
    _t("analysis", "Statistics", "statistics", ("in_table", "input"), ("out_table", "output"), ("statistics_fields", ("statistics", _statistics)), ("case_field", ("case_fields", _join))),
    _t("analysis", "Intersect", "intersect", ("in_features", ("@pair", None)), ("out_feature_class", "output"), ("join_attributes", None), ("cluster_tolerance", None), ("output_type", None)),
    _t("analysis", "Union", "union", ("in_features", ("@pair", None)), ("out_feature_class", "output"), ("join_attributes", None), ("cluster_tolerance", None), ("gaps", None)),
    # management
    _t("management", "Dissolve", "dissolve", ("in_features", "input"), ("out_feature_class", "output"), ("dissolve_field", ("fields", _join)), ("statistics_fields", None),
       ("multi_part", ("single_part", lambda v: str(v).upper() == "SINGLE_PART")), ("unsplit_lines", None)),
    _t("management", "MultipartToSinglepart", "multipart_to_singlepart", ("in_features", "input"), ("out_feature_class", "output")),
    _t("management", "FeatureToPoint", "feature_to_point", ("in_features", "input"), ("out_feature_class", "output"), ("point_location", "point_location")),
    _t("management", "FeatureVerticesToPoints", "feature_vertices_to_points", ("in_features", "input"), ("out_feature_class", "output"), ("point_location", "point_location")),
    _t("management", "PointsToLine", "points_to_line", ("Input_Features", "input"), ("Output_Feature_Class", "output"), ("Line_Field", "line_field"), ("Sort_Field", "sort_field"), ("Close_Line", ("close", _flag("CLOSE")))),
    _t("management", "MinimumBoundingGeometry", "minimum_bounding_geometry", ("in_features", "input"), ("out_feature_class", "output"), ("geometry_type", "geometry_type"),
       ("group_option", "group_option"), ("group_field", ("group_fields", _join)), ("mbg_fields_option", ("mbg_fields", _flag("MBG_FIELDS")))),
    _t("management", "Merge", "merge", ("inputs", ("inputs", _join)), ("output", "output"), ("field_mappings", None), ("add_source", ("add_source", _flag("ADD_SOURCE_INFO")))),
    _t("management", "Append", "append", ("inputs", ("inputs", _join)), ("target", "input"), ("schema_type", "schema_type"), ("field_mapping", None), ("subtype", None), outputs=()),
    _t("management", "Sort", "sort", ("in_dataset", "input"), ("out_dataset", "output"), ("sort_field", ("sort_fields", _sort_fields)), ("spatial_sort_method", "spatial_sort_method")),
    _t("management", "AddXY", "add_xy", ("in_features", "input"), outputs=()),
    _t("management", "AddField", "add_field", ("in_table", "input"), ("field_name", "field_name"), ("field_type", "field_type"), ("field_precision", "field_precision"),
       ("field_scale", "field_scale"), ("field_length", "field_length"), ("field_alias", None), ("field_is_nullable", None), ("field_is_required", None), ("field_domain", None), outputs=()),
    _t("management", "DeleteField", "delete_field", ("in_table", "input"), ("drop_field", ("fields", _join)), ("method", "method"), outputs=()),
    _t("management", "CalculateField", "calculate_field", ("in_table", "input"), ("field", "field"), ("expression", "expression"), ("expression_type", None), ("code_block", "code_block"),
       ("field_type", "field_type"), outputs=()),
    _t("management", "DeleteIdentical", "delete_identical", ("in_dataset", "input"), ("fields", ("fields", _join)), ("xy_tolerance", "xy_tolerance"), ("z_tolerance", None), outputs=()),
    _t("management", "FindIdentical", "find_identical", ("in_dataset", "input"), ("out_dataset", "output"), ("fields", ("fields", _join)), ("xy_tolerance", None), ("z_tolerance", None),
       ("output_record_option", ("only_duplicates", _flag("ONLY_DUPLICATES")))),
    _t("management", "AddGeometryAttributes", "add_geometry_attributes", ("Input_Features", "input"), ("Geometry_Properties", ("properties", _join)), ("Length_Unit", "length_unit"),
       ("Area_Unit", "area_unit"), ("Coordinate_System", None), outputs=()),
    _t("management", "CheckGeometry", "check_geometry", ("in_features", "input"), ("out_table", "output"), ("validation_method", "validation_method")),
    _t("management", "RepairGeometry", "repair_geometry", ("in_features", "input"), ("delete_null", "delete_null"), ("validation_method", "validation_method"), outputs=()),
    _t("management", "SelectLayerByLocation", "select_by_location", ("in_layer", "input"), ("overlap_type", "overlap_type"), ("select_features", "other"),
       ("search_distance", _radius("search_distance")), ("selection_type", None), ("invert_spatial_relationship", ("invert", _flag("INVERT"))), outputs=()),
    _t("management", "Resample", "resample", ("in_raster", "input"), ("out_raster", "output"), ("cell_size", "cell_size"), ("resampling_type", "resampling_type")),
    # conversion
    _t("conversion", "RasterToPoint", "raster_to_point", ("in_raster", "input"), ("out_point_features", "output"), ("raster_field", None)),
    _t("conversion", "RasterToPolygon", "raster_to_polygon", ("in_raster", "input"), ("out_polygon_features", "output"), ("simplify", None), ("raster_field", None)),
    _t("conversion", "PolygonToRaster", "polygon_to_raster", ("in_features", "input"), ("value_field", "value_field"), ("out_rasterdataset", "output"),
       ("cell_assignment", "cell_assignment"), ("priority_field", None), ("cellsize", "cell_size")),
    _t("conversion", "PolylineToRaster", "polyline_to_raster", ("in_features", "input"), ("value_field", "value_field"), ("out_rasterdataset", "output"),
       ("cell_assignment", "cell_assignment"), ("priority_field", None), ("cellsize", "cell_size")),
    _t("conversion", "PointToRaster", "point_to_raster", ("in_features", "input"), ("value_field", "value_field"), ("out_rasterdataset", "output"),
       ("cell_assignment", "cell_assignment"), ("priority_field", None), ("cellsize", "cell_size")),
    _t("edit", "Densify", "densify", ("in_features", "input"), ("densification_method", None), ("distance", "distance"), ("max_deviation", None), ("max_angle", None)),
    _t("management", "GeneratePointsAlongLines", "generate_points_along_lines", ("Input_Features", "input"), ("Output_Feature_Class", "output"), ("Point_Placement", "point_placement"),
       ("Distance", "distance"), ("Percentage", "percentage"), ("Include_End_Points", "include_end_points"), ("Add_Chainage_Fields", None)),
    _t("management", "CreateFishnet", "create_fishnet", ("out_feature_class", "output"), ("origin_coord", "origin_coord"), ("y_axis_coord", "y_axis_coord"), ("cell_width", "cell_width"),
       ("cell_height", "cell_height"), ("number_rows", "number_rows"), ("number_columns", "number_columns"), ("corner_coord", "corner_coord"), ("labels", None), ("template", "template"),
       ("geometry_type", "geometry_type")),
    _t("management", "PolygonToLine", "polygon_to_line", ("in_features", "input"), ("out_feature_class", "output"), ("neighbor_option", "neighbor_option")),
    # stats (Spatial Statistics: Measuring Geographic Distributions)
    _t("stats", "MeanCenter", "mean_center", ("Input_Feature_Class", "input"), ("Output_Feature_Class", "output"), ("Weight_Field", "weight_field"), ("Case_Field", "case_field"), ("Dimension_Field", "dimension_field")),
    _t("stats", "MedianCenter", "median_center", ("Input_Feature_Class", "input"), ("Output_Feature_Class", "output"), ("Weight_Field", "weight_field"), ("Case_Field", "case_field"), ("Dimension_Field", "dimension_field")),
    _t("stats", "CentralFeature", "central_feature", ("Input_Feature_Class", "input"), ("Output_Feature_Class", "output"), ("Distance_Method", "distance_method"), ("Weight_Field", "weight_field"),
       ("Self_Potential_Weight_Field", "self_potential_weight_field"), ("Case_Field", "case_field")),
    _t("stats", "StandardDistance", "standard_distance", ("Input_Feature_Class", "input"), ("Output_Standard_Distance_Feature_Class", "output"), ("Circle_Size", "circle_size"), ("Weight_Field", "weight_field"), ("Case_Field", "case_field")),
    _t("stats", "DirectionalDistribution", "directional_distribution", ("Input_Feature_Class", "input"), ("Output_Ellipse_Feature_Class", "output"), ("Ellipse_Size", "ellipse_size"), ("Weight_Field", "weight_field"), ("Case_Field", "case_field")),
    _t("stats", "DirectionalMean", "directional_mean", ("Input_Feature_Class", "input"), ("Output_Feature_Class", "output"), ("Orientation_Only", "orientation_only"), ("Case_Field", "case_field")),
    # sa
    _t("sa", "Slope", "slope", ("in_raster", "input"), ("output_measurement", "output_measurement"), ("z_factor", "z_factor"), ("method", None), ("z_unit", None)),
    _t("sa", "Idw", "idw", ("in_point_features", "input"), ("z_field", "z_field"), ("cell_size", "cell_size"), ("power", "power"),
       ("search_radius", _search_radius), ("in_barrier_polyline_features", None)),
    _t("sa", "CellStatistics", "cell_statistics", ("in_rasters_or_constants", _inputs), ("statistics_type", "statistic"), ("ignore_nodata", "ignore_nodata")),
    _t("sa", "WeightedSum", "weighted_sum", ("in_weighted_sum_table", _ws_table)),
    _t("sa", "HighestPosition", "highest_position", ("in_rasters", _inputs)),
    _t("sa", "LowestPosition", "lowest_position", ("in_rasters", _inputs)),
    _t("sa", "Combine", "combine", ("in_rasters", _inputs)),
    _t("sa", "WeightedOverlay", "weighted_overlay", ("in_weighted_overlay_table", _wo_table)),
    _t("sa", "FuzzyMembership", "fuzzy_membership", ("in_raster", "input"), ("fuzzy_function", _fuzzy_function), ("hedge", "hedge")),
    _t("sa", "FuzzyOverlay", "fuzzy_overlay", ("in_rasters", _inputs), ("overlay_type", "overlay_type"), ("gamma", "gamma")),
    _t("sa", "Aspect", "aspect", ("in_raster", "input"), ("method", None), ("z_unit", None)),
    _t("sa", "Hillshade", "hillshade", ("in_raster", "input"), ("azimuth", "azimuth"), ("altitude", "altitude"), ("model_shadows", ("model_shadows", _flag("SHADOWS"))), ("z_factor", "z_factor")),
    _t("sa", "Curvature", "curvature", ("in_raster", "input"), ("z_factor", "z_factor"), ("out_profile_curve_raster", ("profile", bool)), ("out_plan_curve_raster", ("plan", bool))),
    _t("sa", "Contour", "contour", ("in_raster", "input"), ("out_polyline_features", "output"), ("contour_interval", "contour_interval"), ("base_contour", "base_contour"), ("z_factor", None)),
    _t("sa", "Fill", "fill", ("in_surface_raster", "input"), ("z_limit", "z_limit")),
    _t("sa", "FlowDirection", "flow_direction", ("in_surface_raster", "input"), ("force_flow", "force_flow")),
    _t("sa", "FlowAccumulation", "flow_accumulation", ("in_flow_direction_raster", "input"), ("in_weight_raster", "weight"), ("data_type", "data_type")),
    _t("sa", "Basin", "basin", ("in_flow_direction_raster", "input")),
    _t("sa", "Watershed", "watershed", ("in_flow_direction_raster", "input"), ("in_pour_point_data", "pour_points")),
    _t("sa", "StreamLink", "stream_link", ("in_stream_raster", "streams"), ("in_flow_direction_raster", "input")),
    _t("sa", "StreamOrder", "stream_order", ("in_stream_raster", "streams"), ("in_flow_direction_raster", "input"), ("order_method", "order_method")),
    _t("sa", "FlowLength", "flow_length", ("in_flow_direction_raster", "input"), ("direction_measurement", "direction_measurement")),
    _t("sa", "FocalStatistics", "focal_statistics", ("in_raster", "input"), ("neighborhood", "neighborhood"), ("statistics_type", "statistics_type"), ("ignore_nodata", "ignore_nodata")),
    _t("sa", "Reclassify", "reclassify", ("in_raster", "input"), ("reclass_field", None), ("remap", "remap"), ("missing_values", "missing_values")),
    _t("sa", "ZonalStatistics", "zonal_statistics", ("in_zone_data", "zones"), ("zone_field", None), ("in_value_raster", "input"), ("statistics_type", "statistics_type"), ("ignore_nodata", "ignore_nodata")),
    _t("sa", "ZonalStatisticsAsTable", "zonal_statistics_as_table", ("in_zone_data", "zones"), ("zone_field", None), ("in_value_raster", "input"), ("out_table", "output"),
       ("ignore_nodata", "ignore_nodata"), ("statistics_type", "statistics_type")),
    _t("sa", "EucDistance", "euclidean_distance", ("in_source_data", "input"), ("maximum_distance", "maximum_distance")),
    _t("sa", "ExtractByMask", "extract_by_mask", ("in_raster", "input"), ("in_mask_data", "mask")),
    _t("sa", "Plus", "plus", ("in_raster_or_constant1", "in1"), ("in_raster_or_constant2", "in2")),
    _t("sa", "Minus", "minus", ("in_raster_or_constant1", "in1"), ("in_raster_or_constant2", "in2")),
    _t("sa", "Times", "times", ("in_raster_or_constant1", "in1"), ("in_raster_or_constant2", "in2")),
    _t("sa", "Divide", "divide", ("in_raster_or_constant1", "in1"), ("in_raster_or_constant2", "in2")),
    _t("sa", "Int", "int", ("in_raster_or_constant", "input")),
    _t("sa", "Float", "float", ("in_raster_or_constant", "input")),
    _t("sa", "IsNull", "is_null", ("in_raster", "input")),
    _t("sa", "Con", "con", ("in_conditional_raster", "input"), ("in_true_raster_or_constant", "true_value"), ("in_false_raster_or_constant", "false_value"), ("where_clause", "where_clause")),
    _t("sa", "SetNull", "set_null", ("in_conditional_raster", "input"), ("in_false_raster_or_constant", "false_value"), ("where_clause", "where_clause")),
]

# Names that are rasters when they are inputs (a constant like 5 or "2.5" is left alone).
_RASTER_INPUTS = {"in1", "in2", "true_value", "false_value", "input", "mask", "zones", "streams", "weight", "pour_points"}


def _layer_name(value: Any) -> Any:
    return value.name if isinstance(value, Layer) else value


def _is_number(text: str) -> bool:
    try:
        float(text)
        return True
    except ValueError:
        return False


def _as_layer(s: Session, value: Any) -> Any:
    """A layer name for an input: a name, a ``Layer``, or a file added on the way."""
    value = _layer_name(value)
    if not isinstance(value, (str, os.PathLike)):
        return value
    text = os.fspath(value)
    names = {l.get("name") for l in s.layers()}
    if text in names:
        return text
    path = text
    if env.workspace and not os.path.isabs(path) and os.path.exists(os.path.join(env.workspace, path)):
        path = os.path.join(env.workspace, path)
    if os.path.exists(path) or re.search(r"\.gdb[\\/]", path):
        stem = os.path.splitext(os.path.basename(path.rstrip("/\\")))[0]
        if stem not in names:
            s.add_data(path, name=stem)
        return stem
    return text  # let the tool say it is missing


# Tools that ignore the raster environments (they take their own grid or make vectors).
_NO_ENV = {"contour", "resample", "euclidean_distance", "extract_by_mask", "zonal_statistics", "zonal_statistics_as_table", "raster_to_point", "raster_to_polygon", "polygon_to_raster", "polyline_to_raster", "point_to_raster"}


def _env_params(s: Session, tool: str, module: str) -> Dict[str, Any]:
    """The raster environments that are set, as tool parameters (Spatial Analyst tools only)."""
    if module != "sa" or tool in _NO_ENV:
        return {}
    out: Dict[str, Any] = {}
    if env.cellSize not in (None, ""):
        try:
            out["cell_size"] = float(env.cellSize)
        except (TypeError, ValueError):
            raise GeoForgeError(f"env.cellSize must be a number, not {env.cellSize!r}") from None
    e = env.extent
    if e not in (None, ""):
        if hasattr(e, "XMin"):
            e = [e.XMin, e.YMin, e.XMax, e.YMax]
        out["extent"] = e if isinstance(e, str) else " ".join(repr(float(v)) for v in e)
    if env.snapRaster not in (None, ""):
        out["snap_raster"] = _as_layer(s, env.snapRaster)
    if env.mask not in (None, ""):
        out["mask"] = _as_layer(s, env.mask)
    return out


_PATHY = re.compile(r"(\.[A-Za-z0-9]{2,5}$)|[\\/]")


def _output_target(value: Any) -> Tuple[str, Optional[str]]:
    """(layer name, path to write) for an output argument."""
    text = os.fspath(value)
    if env.workspace and not os.path.isabs(text) and _PATHY.search(text) and ".gdb" not in text and not text.startswith("."):
        text = os.path.join(env.workspace, text)
    if _PATHY.search(text):
        base = os.path.basename(text.rstrip("/\\"))
        return os.path.splitext(base)[0], text
    return text, None


def _make(module: str, name: str, tool: str, args: Tuple[Arg, ...], outputs: Tuple[str, ...]) -> Callable[..., Result]:
    arg_names = [a[0] for a in args]

    def call(*positional: Any, **keywords: Any) -> Result:
        if len(positional) > len(args):
            raise TypeError(f"{name}() takes at most {len(args)} arguments ({len(positional)} given)")
        given: Dict[str, Any] = dict(zip(arg_names, positional))
        for k, v in keywords.items():
            if k not in arg_names and k not in ("out_raster", "output"):
                raise TypeError(f"{name}() got an unexpected keyword argument '{k}'")
            if k in given:
                raise TypeError(f"{name}() got multiple values for argument '{k}'")
            given[k] = v
        s = session()
        params: Dict[str, Any] = {}
        writes: List[Tuple[str, str]] = []
        for arcpy_name, target in args:
            if arcpy_name not in given or given[arcpy_name] in (None, "", "#"):
                continue
            v = given[arcpy_name]
            if target is None:
                continue
            if isinstance(target, tuple):
                key, conv = target
                if key == "@pair":
                    names = [_as_layer(s, x) for x in (v if isinstance(v, (list, tuple)) else [v])]
                    if len(names) != 2:
                        raise GeoForgeError(f"{name} takes two input layers here")
                    params["input"], params["other"] = names
                    continue
                params[key] = conv(v)
                continue
            if callable(target):
                params.update(target(v))
                continue
            if target == "output":
                layer, path = _output_target(v)
                params["output"] = layer
                if path:
                    writes.append((layer, path))
            elif target in _RASTER_INPUTS and isinstance(v, (int, float)) and not isinstance(v, bool):
                params[target] = str(v)  # a constant operand
            elif target in _RASTER_INPUTS and isinstance(_layer_name(v), (str, os.PathLike)):
                text = os.fspath(_layer_name(v))
                params[target] = text if _is_number(text) else _as_layer(s, v)
            elif target in ("input", "other", "clip", "erase", "near", "mask"):
                params[target] = _as_layer(s, v)
            elif isinstance(v, bool):
                params[target] = v
            else:
                params[target] = _layer_name(v)
        # arcpy.sa takes no output name; allow `out_raster=` (or `output=`) as a keyword.
        for k in ("out_raster", "output"):
            if k in keywords and "output" not in params:
                layer, path = _output_target(keywords[k])
                params["output"] = layer
                if path:
                    writes.append((layer, path))
        if "output" in params and "output" in {a for a in params} and not env.overwriteOutput:
            if any(l.get("name") == params["output"] for l in s.layers()):
                raise GeoForgeError(f"ERROR 000258: Output {params['output']} already exists")
        for k, v in _env_params(s, tool, module).items():
            params.setdefault(k, v)
        before = {l.get("id"): l.get("name") for l in s.layers()}
        s.notices()
        raw = s.run("gp.run", tool=_PREFIX + tool, params=params)
        messages = s.notices()
        after = {l.get("id"): l.get("name") for l in s.layers()}
        ids: List[int] = []
        if isinstance(raw, dict):
            if "layer" in raw:
                ids = [raw["layer"]]
            ids += list(raw.get("layers", []))
        out_names = [after[i] for i in ids if i in after] or [after[i] for i in after if i not in before]
        out_names += [str(t) for t in ([raw["table"]] if isinstance(raw, dict) and "table" in raw else [])]
        for layer, path in writes:
            s.run("share.exportLayer", layer=layer, path=path)
            out_names = [path if n == layer else n for n in out_names]
        if not out_names and isinstance(raw, dict):
            out_names = [str(params.get("input", ""))]
        return Result(out_names, messages, raw)

    call.__name__ = name
    call.__qualname__ = f"{module}.{name}"
    call.__doc__ = (
        f"ArcGIS Pro ``{name}`` (GeoForge tool ``{tool}``).\n\nParameters, in order: "
        + ", ".join(arg_names)
        + ". Parameters ArcGIS has and GeoForge does not use are accepted and ignored."
    )
    return call


class _Module:
    """``arcpy.<toolbox>``: its tools as functions."""

    def __init__(self, name: str) -> None:
        self.__name__ = name

    def __repr__(self) -> str:
        return f"<arcpy.{self.__name__}: {', '.join(sorted(k for k in vars(self) if not k.startswith('_')))}>"


analysis = _Module("analysis")
management = _Module("management")
sa = _Module("sa")
conversion = _Module("conversion")
stats = _Module("stats")
edit = _Module("edit")
_modules = {"edit": edit, "analysis": analysis, "management": management, "sa": sa, "conversion": conversion, "stats": stats}

sa.RadiusVariable = RadiusVariable
sa.RadiusFixed = RadiusFixed
for _cls in (RemapValue, WSTable, WOTable, FuzzyLinear, FuzzyGaussian, FuzzyLarge, FuzzySmall, FuzzyNear, FuzzyMSLarge, FuzzyMSSmall):
    setattr(sa, _cls.__name__, _cls)

for _module, _name, _tool, _args, _outs in _TOOLS:
    setattr(_modules[_module], _name, _make(_module, _name, _tool, _args, _outs))


def ListTools(wildcard: str = "*") -> List[str]:
    """Names of the tools, as ``Name_toolbox`` (arcpy's ``ListTools``)."""
    import fnmatch

    names = [f"{n}_{m}" for m, n, *_ in _TOOLS]
    return [n for n in names if fnmatch.fnmatch(n.lower(), wildcard.lower())]
