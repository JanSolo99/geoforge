# rustgis

[![PyPI](https://img.shields.io/pypi/v/rustgis.svg)](https://pypi.org/project/rustgis/)
[![Open in Colab](https://colab.research.google.com/assets/colab-badge.svg)](https://colab.research.google.com/github/opengeos/RustGIS/blob/main/apps/rustgis-python/examples/getting-started.ipynb)

Python bindings to [UniGIS](https://rustgis.app), a fast, free and open-source GIS in pure Rust.
The UniGIS engine runs inside Python, headless: open data and projects, run any of UniGIS's
commands and over 1,000 geoprocessing tools, style layers, render maps and save projects the
desktop app opens as they are.

```sh
pip install rustgis            # add [geopandas] for GeoDataFrame in and out
```

```python
import rustgis

s = rustgis.Session(sample=True)                  # the World Atlas sample
s.run("gp.run", tool="buffer", params={"input": "Cities", "distance": 100})
s.run("layer.symbology", layer="Countries", type="graduated", field="pop_est")
s.to_png("map.png", width=1600, height=900)      # or just `s` in Jupyter
s.save("atlas.rgis")                              # open it in the UniGIS app

gdf = s.layer("Cities").to_geopandas()            # a GeoDataFrame in EPSG:4326
s.add_geopandas(gdf[gdf.pop_max > 5_000_000], name="Big cities")
```

In Jupyter, `rustgis.Map` shows the UniGIS app itself in the cell, in step with its session both
ways (`pip install "rustgis[jupyter]"`):

```python
m = rustgis.Map(sample=True)
m.add_geopandas(gdf, name="Big cities")           # appears in the app
m                                                 # pan, edit, style there...
m.info()["center"]                                # ...and read it back here
```

Every action in UniGIS is a command with an id, the same ids the ribbon, `rustgis-cli` and the
AI Assistant use: `Session.commands()` lists them with their parameters, and `Session.run` runs
any of them. See the [Python guide](https://rustgis.app/python/) and the
[getting-started notebook](https://github.com/opengeos/RustGIS/blob/main/apps/rustgis-python/examples/getting-started.ipynb).

Licensed under Apache-2.0.

## arcpy-shaped API (UniGIS)

`rustgis.arcpy` runs UniGIS's ArcGIS-verified tools with ArcGIS Pro's names, positional parameters and keywords:

```python
import rustgis
from rustgis import arcpy

arcpy.use(rustgis.Session(sample=True))
arcpy.analysis.Buffer("Cities", "Cities_100km", "100 Kilometers")   # geodesic on the ellipsoid
arcpy.analysis.Select("Cities", "Mega", "pop_max > 10000000")
arcpy.management.AddField("Cities", "ratio", "DOUBLE")
arcpy.conversion.PolygonToRaster("Countries", "", "Grid", "CELL_CENTER", cellsize=2)
arcpy.sa.FocalStatistics("Grid", "Rectangle 3 3 CELL", "MAXIMUM", out_raster="GridMax")
arcpy.analysis.Buffer("Cities", "/data/out.gdb/buffers", "10 Kilometers")  # written there, too
```

Toolboxes: `analysis`, `management`, `conversion`, `sa` (about 65 tools; `arcpy.ListTools()`). Inputs are layer names,
`Layer` objects or file paths (added as layers). An output that is a path is written there as well. Tools return a
`Result` (`getOutput`, `getMessages`). Not arcpy: layers are longitude/latitude, `arcpy.sa` functions take the output as
`out_raster=`, and `env` has only `workspace` and `overwriteOutput`.
