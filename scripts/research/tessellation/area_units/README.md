# Generate Tessellation area units and invalid-parameter safety

ArcGIS Pro3.7 is the GP oracle.31 `tess_area_*` goldens cover18 canonical area
units, defaults/unknownSR, projected meters/US feet, four isolated conversion
factors and hectare equivalents for all other supported tessellation shapes.
OGR reads output files and can recreate the empty projected template; it does
not define geoprocessing behavior and is never a runtime dependency.

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'tess_area_*'
python scripts/research/tessellation/area_units/compare_native.py
python scripts/research/tessellation/area_units/recreate_template.py /tmp/new-area-template
```

The three original private probe scripts recreate27 oracle controls. Full parameters
for the31 saved cases are in cases.json and the generator. observations.json records
all27 private results. The empty polygon feet_template fixture names EPSG2272;
its original five component files remained byte-identical through generation
(source_verification.json). Its geometry is not used; only the coordinate system. The recipe builds storage
with OGR then restores the captured canonical ArcGIS spatial_reference.prj, retaining
its full numeric precision.

Bare numeric sizes are in coordinate units squared. Explicit physical units are
converted into square meters, then divided by the projected CRS's meters-per-unit
squared. When no spatial reference is supplied, ArcGIS uses meter coordinates
while retaining an unknown output CRS. Thus10000sqm and1hectare agree on a meter
grid, whereas a US-foot grid converts10000sqm to107638.673611ft2. Bare10000 in the
same feet CRS remains10000ft2. The earlier parser discarded all unit suffixes.

Supported canonical units: SquareMeters/Kilometers/Centimeters/Millimeters/
Decimeters, Hectares/Ares/Acres, SquareFeet/Yards/Inches/Miles/NauticalMiles, plus
SquareFeetUS/YardsUS/InchesUS/MilesUS and AcresUS. US survey foot is1200/3937m;
international foot is0.3048m. A1e8SquareFeet request on the US-foot grid has side
9999.98ft, while1e8SquareFeetUS has side10000ft; the large test makes this distinction
observable beyond XY tolerance.

Origin-centered one-cell probes isolate conversion constants without cancellation
from large coordinates. ArcGIS uses2589988.11sqm per international square mile,
rather than the exact square of1609.344m. US mile2589998.4703195216, acre4046.8564224,
US acre4046.8726098742522. Existing coordinate areas and other shape formulas remain
unchanged after conversion.

All31 initial native cases matched semantic CRS, counts, attributes/order and ordered ring vertices
at0.001 coordinate tolerance. The first full gate caught four exact PRJ mismatches:
PROJ re-export renamed Foot_US and shortened parameters to15digits. Provided Esri
Foot_US definitions now retain their original precision, while the existing degree
normalization remains. Unconditional source-WKT preservation was rejected because
meter inputs still require that angular normalization. The comparator now checks
exact PRJ bytes too; final results are in native_after.txt. Full reference validation
also checks schemas/geometries. Numeric extents are now parsed strictly; nonfinite or
overflowing spans and tiny/excessive square grids return errors before output.
A CLI regression starts with a valid shapefile and checks that every component
remains byte-identical for five bad requests, with structured failure instead of
panic. Excessive-grid cases are native safety checks, not claims about ArcGIS's
exact resource-limit error codes; they are intentionally not run unbounded in ArcGIS.

Geographic/compound CRS physical-unit conversion, WKT2/custom engineering CRS,
nondefault environments and H3 tessellation remain unsupported/unverified.
