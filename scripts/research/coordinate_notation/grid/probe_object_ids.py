"""Black-box source HasOID / ORIG_OID policy for coordinate conversion."""
import csv
import json
import os

root = '/tmp/unigis-carry-object-ids'
os.makedirs(P(root), exist_ok=True)
gdb = root + '/sources.gdb'
if not arcpy.Exists(P(gdb)): arcpy.management.CreateFileGDB(P(root), 'sources.gdb')
paths = [('dbf', '/mnt/media/ArcGIS_Linux/testdata/geoproc/grid_carry_precision.dbf', 'COORD', None, 'GEOREF')]
for kind in ['shape', 'gdb']:
    path = root + '/points.shp' if kind == 'shape' else gdb + '/rows'
    if arcpy.Exists(P(path)): arcpy.management.Delete(P(path))
    if kind == 'shape': arcpy.management.CreateFeatureclass(P(root), 'points.shp', 'POINT', spatial_reference=4326)
    else: arcpy.management.CreateTable(P(gdb), 'rows')
    for field in ['LON', 'LAT']: arcpy.management.AddField(P(path), field, 'DOUBLE')
    fields = ['LON', 'LAT'] + (['SHAPE@XY'] if kind == 'shape' else [])
    with arcpy.da.InsertCursor(P(path), fields) as cursor:
        for x, y in [(15.123456,45.678901),(-73.9857,40.7484),(151.2093,-33.8688)]:
            cursor.insertRow([x, y] + ([(x,y)] if kind == 'shape' else []))
    paths.append((kind, path, 'LON', 'LAT', 'DD_2'))
path = root + '/rows.csv'
with open(P(path),'w',newline='') as f:
    writer=csv.writer(f);writer.writerow(['LON','LAT']);writer.writerows([(15.123456,45.678901),(-73.9857,40.7484)])
paths.append(('csv',path,'LON','LAT','DD_2'))
for kind, path, x, y, fmt in paths:
    description = arcpy.Describe(P(path))
    out = 'memory/oid_probe_' + kind
    arcpy.management.ConvertCoordinateNotation(P(path),out,x,y,fmt,'DD_NUMERIC','',arcpy.SpatialReference(4326))
    names=[f.name for f in arcpy.ListFields(out)]
    fields=[n for n in ['DDLon','DDLat','ORIG_OID'] if n in names]
    with arcpy.da.SearchCursor(out, fields) as cursor:
        values=list(cursor)
    print(json.dumps({'kind':kind,'hasOID':getattr(description,'hasOID',None),
                      'oidField':getattr(description,'OIDFieldName',None),'fields':names,
                      'values':values[:3]}))
