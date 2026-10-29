# Delete Multiple

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/delete-multiple.html

Fourteen disposable public ArcGIS Pro3.7 controls establish that the tool
validates all input existence before deletion: a missing later row returns000732
and leaves earlier inputs untouched. Empty input returns000735. Duplicate rows
succeed. Optional type hints are ignored when names are unambiguous, including
wrong/unknown hints. A shapefile's DBF path identifies the whole shapefile.
Standalone DBF deletes its CPG/XML too; shapefile sidecars are deleted while
same-prefix unrelated files survive. Regular files, geodatabase items and whole
FileGDB workspaces are supported. One separate integer TIFF control deletes the
TIFF/TFW/aux.xml/VAT DBF/VAT CPG/XML family, preserving grid.txt.

Implementation: quote-aware value-table parser and Report tool in
`crates/unigis-tools/src/delete_multiple.rs`; optional Context::delete_items
capability explicitly fails on read-only hosts. Native CLI preflights every
input and sidecar before writing, deduplicates canonical paths and reuses native
FileGDB catalog deletion/cascades. The successful report is `out_results: true`.
Existing single Delete behavior is unchanged. No runtime GDAL dependency.

Scope: local shapefile and standalone DBF families, observed TIFF sidecars,
regular files, native FileGDB classes/tables/feature datasets/relationships and
whole FileGDB workspaces. Ordinary folders are refused: ArcGIS documents moving
them to the recycle bin, which this host does not implement. Symlinks, non-file
ancillary paths, detected lock files and ambiguous geodatabase kinds are refused.
ArcGIS lock ownership/staleness, selections/layers, enterprise/mobile workspaces,
other dataset sidecars/formats and malformed datasets remain unverified. TIFF
pyramids (.ovr) are retained; only the saved TIFF family is removed. Data-type
hints cannot disambiguate colliding kinds yet. Filesystem case behavior follows
the host. Input preflight is complete, but later I/O errors can leave earlier
items deleted; this is not a transaction across files or workspaces.

13 dml_* cases are saved in the main ArcGIS manifest:10 successful operations,
3 refusals. The generator copies testdata/geoproc/delete_multiple into temporary
workspaces before calling any destructive tool. Catalog snapshots record the
surviving filesystem and FileGDB items; the native comparator additionally checks
byte-exact refusal, surviving file bytes and surviving tables' schemas/rows.
Tests cannot address paths outside their disposable workspace. The source fixture
includes same-prefix/same-stem sentinels, quoted paths and a tiny integer TIFF.
Native safety tests guard malformed/unsupported later inputs, ancillary directories
and index cleanup beside dotted sibling dataset names. Parser/host tests cover
quoted semicolons, escaped apostrophes, type hints and explicit read-only failure.

Completed probe results/recipes are adjacent. make_fixture.py reuses completed
probe files to create the source once; probe_raster.py creates the tiny raster
and deletes only a disposable copy. First reference generation failed due a
nested-code backslash escaping error; the helper now uses chr(92).

Reproduce only when necessary:

    uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'dml_*'
    cargo test -j3 -p unigis-cli --test arcgis_reference --test delete_multiple_safety
    scripts/check

Validation outcomes are recorded in docs/HANDOFF.md.
