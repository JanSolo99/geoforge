"""ArcGIS bridge: reversed report keys and one-sided aggregation orientation."""
import json
arcpy.env.overwriteOutput=True
arcpy.management.CopyFeatures(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/pairwise_neighbors.shp'),'memory/ppn_order')
with arcpy.da.UpdateCursor('memory/ppn_order',['NAME','GRP']) as c:
 for i,row in enumerate(c):c.updateRow((chr(ord('Z')-i),10-row[1]))
results={}
for fields in ['NAME','GRP','GRP;NAME']:
 for sides in ['BOTH_SIDES','NO_BOTH_SIDES']:
  arcpy.analysis.PairwisePolygonNeighbors('memory/ppn_order','memory/ppn_result',fields,'AREA_OVERLAP',sides)
  names=[f.name for f in arcpy.ListFields('memory/ppn_result') if f.type!='OID']
  results[fields+'_'+sides]={'fields':names,'rows':list(arcpy.da.SearchCursor('memory/ppn_result',names))}
with open(P('/tmp/unigis-ppn-order.json'),'w',newline='\n') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-ppn-order.json')
