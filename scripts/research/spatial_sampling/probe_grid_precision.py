"""Public saved-bin precision and a nonempty COMPLETELY_WITHIN control."""
import os,json
root=P('/tmp/unigis-spatial-sampling-grid');os.makedirs(root,exist_ok=False)
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/ssl_areas.shp');results={}
for name,shape,size,relation in [(s.lower(),s,'100 SquareMeters','HAVE_THEIR_CENTER_IN') for s in ['DIAMOND','HEXAGON','TRANSVERSE_HEXAGON','TRIANGLE']]+[('within_small','SQUARE','25 SquareMeters','COMPLETELY_WITHIN')]:
 out=os.path.join(root,name+'.shp')
 arcpy.management.CreateSpatialSamplingLocations(source,out,sampling_method='SYSTEMATIC',bin_shape=shape,bin_size=size,geometry_type='POLYGON',spatial_relationship=relation)
 d=arcpy.Describe(out)
 results[name]=dict(resolution=d.spatialReference.XYResolution,tolerance=d.spatialReference.XYTolerance,rows=[dict(values=list(r[:-1]),geometry=json.loads(r[-1])) for r in arcpy.da.SearchCursor(out,['Shape_Leng','Shape_Area','GRID_ID','SHAPE@JSON'])])
with open(P('/tmp/unigis-spatial-sampling-grid.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps({k:dict(resolution=v['resolution'],tolerance=v['tolerance'],count=len(v['rows']),first=v['rows'][:1]) for k,v in results.items()},indent=2))
