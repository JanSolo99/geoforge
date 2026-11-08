# Delete Raster Attribute Table (Data Management) — ArcGIS Pro 3.7

Probed 2026-11-08 on copies of `testdata/raster/Combina_patches1.tif` (with `.tif.aux.xml`,
`.tfw`, `.tif.xml`), `patches_i16.tif` (only the table) and `classes_u8.tif` (no table).

Observed:
- The tool edits the raster in place: `<raster>.vat.dbf` and `<raster>.vat.cpg` are deleted. The
  `.tif`, `.tif.aux.xml` and `.tfw` are byte-identical afterwards.
- `<raster>.tif.xml` (UTF-16 ArcGIS metadata) gains a lineage `Process` entry naming the tool with
  the run's date and time (created when absent); not reproduced.
- A raster without a table fails: `ERROR 000236: Input raster does not have an attribute table`.

Implemented as `ug run delete_raster_attribute_table input=… output=…`: the raster and its sidecars
are copied to `output` (or edited in place when equal) and the table files are not carried over.
Reference cases `drat_aux`, `drat_vat_only`, `drat_none` (the test also compares whether the
`.vat.*` files exist beside the output).
