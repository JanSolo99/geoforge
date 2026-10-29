# Add Full-Text Index — deferred

https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-full-text-index.html

Official documentation requires an enterprise database, SQLite or mobile
geodatabase; FileGDB is explicitly unsupported. Native format crates currently
write shapefile/FileGDB, with no SQLite/mobile/enterprise full-text backend.
Disposable FileGDB point feature class with TEXT field returned ERROR002764
(operation unsupported). Public recipe is the last block of
../gps_metadata/probe.py; saved result is filegdb_result.json. No runtime stub.
