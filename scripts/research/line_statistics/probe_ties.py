import json,os
arcpy.CheckOutExtension('Spatial')
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
fc=arcpy.management.CreateFeatureclass('memory','line_ties','POLYLINE',spatial_reference=arcpy.SpatialReference(32633))[0]
arcpy.management.AddField(fc,'IVAL','LONG')
arcpy.management.AddField(fc,'DVAL','DOUBLE')
with arcpy.da.InsertCursor(fc,['SHAPE@','IVAL','DVAL']) as c:
 for i in [1,2]:
  c.insertRow((arcpy.Polyline(arcpy.Array([arcpy.Point(0,0),arcpy.Point(20,20)]),arcpy.SpatialReference(32633)),i,i+0.25))
for field in ['IVAL','DVAL']:
 for stat in ['LENGTH','MEAN','MAXIMUM','MINIMUM','RANGE','VARIETY','MAJORITY','MINORITY']:
  try:
   r=arcpy.sa.LineStatistics(fc,field,10,6,stat)
   out='/tmp/unigis_line_ties_'+field+'_'+stat+'.tif'
   r.save(P(out))
   r=arcpy.Raster(P(out))
   print(json.dumps({'field':field,'stat':stat,'type':r.pixelType,'nodata':r.noDataValue,'shape':[r.width,r.height],'cells':arcpy.RasterToNumPyArray(r).tolist()}))
  except Exception as e:print(json.dumps({'field':field,'stat':stat,'error':str(e)}))
