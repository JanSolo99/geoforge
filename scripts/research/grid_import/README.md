# Legacy raster imports

ArcGIS Pro 3.7 is the behavior oracle. Public parameter/format references:
- https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/ascii-to-raster.html
- https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/float-to-raster.html

Both legacy tools are exposed through the registry as `ascii_to_raster` and
`float_to_raster`. No GDAL runtime or new dependency is used. Input files live in
testdata/raster/grid_import; `make_inputs.py` recreates the text and explicitly
packed IEEE float32 binary fixtures. `observations.json` and probe scripts preserve
black-box results. Regenerate saved outputs with:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ascimp_*' 'fltimp_*'
```

35 success/error cases establish:
- ASCII defaults to INTEGER, output S32. Decimal values truncate toward zero after
  parsing as doubles, preserving 16777217 and signed 32-bit extrema.
- FLOAT ASCII and binary imports output F32. Both replace input NoData with the
  system marker (S32 -2147483647, F32 minimum). Missing input NoData defaults -9999.
- ASCII NoData equality is checked before output conversion; near values can round
  to the same F32 number without becoming NoData. Binary header NoData is normalized
  to its stored F32 type before comparison.
- Lower-left corner/center origins work; input order is top row to bottom. Binary
  supports LSBFIRST/MSBFIRST, with missing byte order defaulting to little endian.
- Short data fails 010067. Pro 3.7 ignores trailing values (despite documentation's
  count-matching statement). All-NoData rasters succeed without statistics.
- Existing ArcGIS export files with projection sidecars still import with unknown
  CRS. These legacy tools ignore the sidecars; the native implementation does too.
- Pixel conversions/copies are compared exactly, alongside grid/type/NoData/CRS and
  per-band statistics. Other raster arithmetic keeps its existing tolerances.

Limits: TIFF output, finite numeric headers/cells, values fitting the chosen output
representation; INTEGER overflow and non-finite data are explicitly refused pending
probes. Compressed float streams, additional header dialects and nondefault storage
or statistics environments are unsupported. Projection assignment is a separate step;
current root Define Projection implementation only handles feature data.

Local ephemeral logs:
- /tmp/unigis-raster-import-{research,extra-research,empty-research}.log: exploratory probes.
- /tmp/unigis-raster-import-generation.log: saved initial oracle generation.
- /tmp/unigis-raster-import-clippy.log: first compile/lint check.

Malformed-float input preservation: the first standalone short-data probe removed
its .hdr. The generator now copies inputs and sidecars to a temporary directory
for both legacy import tools. A restored short float fails 010067; an explicitly
missing header fails 010062. Native imports are read-only. CLI grid_import.rs tests
input bytes/header preservation and absence of partial output after failure.
Final root gate: /tmp/unigis-raster-import-check.log.

The saved ArcGIS catalog snapshot omits these deprecated callable tools. Dashboard
own-tool and case totals include them; catalog coverage rows remain unchanged.
