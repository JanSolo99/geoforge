"""Fresh typed-table holdout for the inferred GARS grid-input path."""
import csv
import json

root='/mnt/media/ArcGIS_Linux'
results=[]
for fmt in ['GARS','GEOREF']:
    source=root+'/scripts/research/coordinate_notation/grid/gars_boundary_'+fmt.lower()+'.csv'
    table='memory/gars_holdout_'+fmt
    arcpy.management.CreateTable('memory','gars_holdout_'+fmt)
    arcpy.management.AddField(table,'ID','LONG')
    arcpy.management.AddField(table,'COORD','TEXT',field_length=255)
    with open(P(source),newline='') as f, arcpy.da.InsertCursor(table,['ID','COORD']) as cursor:
        for row in csv.DictReader(f): cursor.insertRow((int(row['ID']),row['COORD']))
    fixture=root+'/testdata/geoproc/gars_boundary_'+fmt.lower()+'.dbf'
    arcpy.env.overwriteOutput=True
    arcpy.management.CopyRows(table,P(fixture))
    out='memory/gars_holdout_out_'+fmt
    gp=arcpy.management.ConvertCoordinateNotation(P(fixture),out,'COORD',None,fmt,'GARS','',arcpy.SpatialReference(4326))
    with arcpy.da.SearchCursor(out,['ID','COORD','GARS','SHAPE@XY']) as cursor:
        rows=[{'id':index,'input':code,'output':value or '', 'xy':xy} for index,code,value,xy in cursor]
    results.append({'format':fmt,'rows':rows,'messages':gp.getMessages(1)})
    with open(P('/tmp/unigis-gars-holdout-results.json'),'w') as f: json.dump(results,f)
print('Saved',sum(len(r['rows']) for r in results),'independent grid controls')
