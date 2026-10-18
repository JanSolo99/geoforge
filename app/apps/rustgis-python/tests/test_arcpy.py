"""Tests for the arcpy-shaped API (run after `maturin develop`)."""

import os

import pytest

import rustgis
from rustgis import arcpy


@pytest.fixture
def s():
    """A session with the World Atlas sample open, the tools' session."""
    session = rustgis.Session(sample=True)
    arcpy.use(session)
    arcpy.env.overwriteOutput = False
    arcpy.env.workspace = None
    arcpy.env.cellSize = arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
    return session


def fields(session, name):
    """Field names of a layer (its info lists them as `name (Type)`)."""
    return {f.split(" (")[0] for f in session.layer(name).info["fields"]}


def count(session, name):
    return session.layer(name).info["features"]


def test_buffer_takes_arcpy_arguments(s):
    r = arcpy.analysis.Buffer("Cities", "Cities_100km", "100 Kilometers")
    assert r.getOutput(0) == "Cities_100km" and str(r) == "Cities_100km"
    assert count(s, "Cities_100km") == count(s, "Cities")
    assert "Cities_100km" in r.getMessages()
    # Keywords work as in arcpy, and a plain number is the data's unit.
    r = arcpy.analysis.Buffer(in_features="Cities", out_feature_class="Cities_5deg", buffer_distance_or_field=5, dissolve_option="ALL")
    assert count(s, "Cities_5deg") == 1


def test_distances_are_on_the_ground(s):
    arcpy.analysis.Buffer("Cities", "B1", "10 Kilometers")
    arcpy.analysis.Buffer("Cities", "B2", "10 Miles")
    # Ten miles cover more ground than ten kilometres: the area of the first feature grows.
    a1 = s.layer("B1").to_geojson()["features"][0]["geometry"]["coordinates"][0]
    a2 = s.layer("B2").to_geojson()["features"][0]["geometry"]["coordinates"][0]
    spread = lambda ring: max(p[1] for p in ring) - min(p[1] for p in ring)
    assert spread(a2) > spread(a1) * 1.5


def test_outputs_that_exist_need_overwrite(s):
    arcpy.analysis.Buffer("Cities", "Twice", "10 Kilometers")
    with pytest.raises(rustgis.GeoForgeError, match="already exists"):
        arcpy.analysis.Buffer("Cities", "Twice", "10 Kilometers")
    arcpy.env.overwriteOutput = True
    arcpy.analysis.Buffer("Cities", "Twice", "10 Kilometers")


def test_wrong_arguments_are_python_errors(s):
    with pytest.raises(TypeError, match="unexpected keyword"):
        arcpy.analysis.Buffer("Cities", "X", "1 Kilometers", nonsense=1)
    with pytest.raises(TypeError, match="at most"):
        arcpy.analysis.Clip(*["a"] * 9)
    with pytest.raises(rustgis.GeoForgeError, match="unknown distance unit"):
        arcpy.analysis.Buffer("Cities", "X", "1 Parsecs")
    with pytest.raises(rustgis.GeoForgeError):
        arcpy.analysis.Buffer("No such layer", "X", "1 Kilometers")


def test_overlay_select_and_tables(s):
    arcpy.analysis.Buffer("Cities", "Big", "300 Kilometers")
    r = arcpy.analysis.Clip("Countries", "Big", "Near_cities")
    assert count(s, "Near_cities") > 0
    r = arcpy.analysis.Select("Cities", "Megacities", "pop_max > 10000000")
    assert 0 < count(s, "Megacities") < count(s, "Cities")
    r = arcpy.analysis.Frequency("Cities", "ByCountry", ["adm0name"])
    assert r.getOutput(0) == "ByCountry"
    r = arcpy.analysis.Statistics("Cities", "Pop", [["pop_max", "SUM"], ["pop_max", "MEAN"]])
    assert r.outputCount == 1


def test_management_in_place_tools(s):
    arcpy.management.AddField("Cities", "ratio", "DOUBLE")
    arcpy.management.CalculateField("Cities", "ratio", "!pop_max! / 1000.0", "PYTHON3")
    assert "ratio" in fields(s, "Cities")
    arcpy.management.AddXY("Cities")
    arcpy.management.DeleteField("Cities", ["ratio"])
    assert "ratio" not in fields(s, "Cities")


def test_select_layer_by_location_selects(s):
    arcpy.analysis.Buffer("Cities", "Hub", "50 Kilometers")
    r = arcpy.management.SelectLayerByLocation("Cities", "WITHIN_A_DISTANCE", "Hub", "10 Kilometers")
    assert r.raw["selected"] > 0


def test_files_are_added_and_written(s, tmp_path):
    out = str(tmp_path / "buffers.shp")
    r = arcpy.analysis.Buffer("Cities", out, "20 Kilometers")
    assert os.path.exists(out) and r.getOutput(0) == out
    # A file is a valid input: it is added as a layer named after it.
    r = arcpy.analysis.Buffer(out, "Again", "1 Kilometers")
    assert count(s, "Again") > 0 and "buffers" in {l["name"] for l in s.layers()}
    gdb = str(tmp_path / "city.gdb" / "cities")
    arcpy.management.Merge(["Cities"], gdb)
    assert os.path.isdir(tmp_path / "city.gdb")


def test_rasters_flow_through_spatial_analyst(s):
    r = arcpy.conversion.PolygonToRaster("Countries", "", "Grid", "CELL_CENTER", cellsize=2)
    assert r.getOutput(0) == "Grid"
    # Spatial Analyst functions take the output as `out_raster`, as keyword.
    arcpy.sa.FocalStatistics("Grid", "Rectangle 3 3 CELL", "MAXIMUM", out_raster="GridMax")
    arcpy.sa.Plus("Grid", 10, out_raster="GridPlus")
    arcpy.sa.Slope("Grid", "DEGREE", 1, out_raster="GridSlope")
    names = {l["name"] for l in s.layers()}
    assert {"Grid", "GridMax", "GridPlus", "GridSlope"} <= names
    r = arcpy.conversion.RasterToPoint("Grid", "GridPoints")
    assert count(s, "GridPoints") > 100


def test_list_tools():
    names = arcpy.ListTools("Buffer*")
    assert names == ["Buffer_analysis"]
    assert len(arcpy.ListTools()) > 60


def test_raster_environments_shape_spatial_analyst_output(s):
    arcpy.conversion.PolygonToRaster("Countries", "", "Grid", "CELL_CENTER", cellsize=2)
    arcpy.sa.Plus("Grid", 1, out_raster="Plain")
    plain = s.layer("Plain").info
    arcpy.env.cellSize = 4
    arcpy.sa.Plus("Grid", 1, out_raster="Coarse")
    arcpy.env.cellSize = None
    assert s.layer("Coarse").info != plain
    arcpy.env.cellSize = "not a size"
    with pytest.raises(rustgis.GeoForgeError, match="cellSize"):
        arcpy.sa.Plus("Grid", 1, out_raster="Bad")


def test_idw_takes_arcpy_search_radii(s):
    arcpy.sa.Idw("Cities", "pop_max", 2, 2, arcpy.sa.RadiusVariable(8), out_raster="CitiesIdw")
    arcpy.sa.Idw("Cities", "pop_max", 2, search_radius=arcpy.sa.RadiusFixed(10, 2), out_raster="CitiesFixed")
    names = {l["name"] for l in s.layers()}
    assert {"CitiesIdw", "CitiesFixed"} <= names
    with pytest.raises(rustgis.GeoForgeError):
        arcpy.sa.Idw("Cities", "pop_max", 2, 2, "nonsense")


def test_overlay_and_fuzzy_tools(s):
    arcpy.conversion.PolygonToRaster("Countries", "", "Grid", "CELL_CENTER", cellsize=4)
    arcpy.sa.Plus("Grid", 3, out_raster="Grid3")
    arcpy.sa.CellStatistics(["Grid", "Grid3"], "MAXIMUM", "DATA", out_raster="Max")
    arcpy.sa.WeightedSum(arcpy.sa.WSTable([["Grid", "VALUE", 0.5], ["Grid3", "VALUE", 0.5]]), out_raster="WSum")
    arcpy.sa.HighestPosition(["Grid", "Grid3"], out_raster="HP")
    arcpy.sa.Combine(["Grid", "Grid3"], out_raster="Comb")
    arcpy.sa.FuzzyMembership("Grid", arcpy.sa.FuzzyLinear(0, 200), "SOMEWHAT", out_raster="Fz")
    arcpy.sa.FuzzyOverlay(["Fz", "Fz"], "GAMMA", 0.5, out_raster="Fo")
    table = arcpy.sa.WOTable([["Grid", 100, "VALUE", arcpy.sa.RemapValue([[1, 5]])]], [1, 9, 1])
    arcpy.sa.WeightedOverlay(table, out_raster="Wo")
    names = {l["name"] for l in s.layers()}
    assert {"Max", "WSum", "HP", "Comb", "Fz", "Fo", "Wo"} <= names
    with pytest.raises(rustgis.GeoForgeError):
        arcpy.sa.FuzzyMembership("Grid", "not a function", out_raster="Bad")


def test_spatial_statistics_tools(s):
    # Planar statistics run in the data's UTM zone, so use one region.
    arcpy.analysis.Select("Cities", "Italian", "adm0name = 'India'")
    arcpy.stats.MeanCenter("Italian", "CityCenter", "pop_max")
    arcpy.stats.StandardDistance("Italian", "CitySD", "2_STANDARD_DEVIATIONS", "pop_max")
    arcpy.stats.DirectionalDistribution("Italian", "CityEllipse", "1_STANDARD_DEVIATION")
    arcpy.stats.CentralFeature("Italian", "CityHub", "MANHATTAN_DISTANCE")
    arcpy.stats.MedianCenter("Italian", "CityMedian")
    names = {l["name"] for l in s.layers()}
    assert {"CityCenter", "CitySD", "CityEllipse", "CityHub", "CityMedian"} <= names
    assert count(s, "CityCenter") == 1
    with pytest.raises(rustgis.GeoForgeError):
        arcpy.stats.StandardDistance("Italian", "Bad", "HUGE")


def test_construction_tools(s):
    arcpy.analysis.Select("Cities", "Indian", "adm0name = 'India'")
    arcpy.management.CreateFishnet("Net", "70 8", "70 18", 1, 1, 4, 5, "#", "NO_LABELS", "#", "POLYGON")
    assert count(s, "Net") == 20
    arcpy.management.PolygonToLine("Net", "NetLines", "IGNORE_NEIGHBORS")
    assert count(s, "NetLines") == 20
    arcpy.edit.Densify("NetLines", "DISTANCE", 0.25, out_raster="Dense")
    assert count(s, "Dense") == 20
    arcpy.management.GeneratePointsAlongLines("NetLines", "Along", "DISTANCE", "0.5", None, "END_POINTS")
    assert count(s, "Along") > 20
    with pytest.raises(rustgis.GeoForgeError):
        arcpy.management.PolygonToLine("Net", "Bad", "IDENTIFY_NEIGHBORS")
