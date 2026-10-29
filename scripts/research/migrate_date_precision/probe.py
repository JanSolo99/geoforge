"""Public date precision migration controls; preserve untouched source once."""
import os,json,datetime
root=P('/tmp/unigis-migrate-date-probe-fixed');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
for name in ['single','multiple','class_dates','empty','already','wrong','missing','mixed','duplicate','case_names','no_fields']:
 fc=(arcpy.management.CreateFeatureclass(gdb,name,'POINT',spatial_reference=sr) if name=='class_dates' else arcpy.management.CreateTable(gdb,name)).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG');arcpy.management.AddField(fc,'D1','DATE',field_alias='First date');arcpy.management.AddField(fc,'D2','DATE')
 if name=='already':arcpy.management.MigrateDateFieldToHighPrecision(fc,'D1')
 if name!='empty':
  fields=['KEY','D1','D2']+(['SHAPE@XY'] if name=='class_dates' else [])
  with arcpy.da.InsertCursor(fc,fields) as c:
   c.insertRow([7,datetime.datetime(2020,1,2,3,4,5,123000 if name=='already' else 0),None]+([(500000,5000000)] if name=='class_dates' else []))
   c.insertRow([8,None,datetime.datetime(2021,2,3,4,5,6)]+([(500001,5000001)] if name=='class_dates' else []))
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/migrate_date_base.gdb'))
args={'single':'D1','multiple':'D1;D2','class_dates':'D1;D2','empty':'D1','already':'D1','wrong':'KEY','missing':'absent','mixed':'D1;KEY','duplicate':'D1;D1','case_names':'d1;d2','no_fields':''}
def snap(fc):
 d=arcpy.Describe(fc);fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
 return dict(fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,precision=f.precision,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))))
result={}
for name,arg in args.items():
 fc=os.path.join(gdb,name);before=snap(fc)
 try:
  arcpy.management.MigrateDateFieldToHighPrecision(fc,arg);messages=arcpy.GetMessages();after=snap(fc)
  arcpy.management.MigrateDateFieldToHighPrecision(fc,arg)
  result[name]=dict(arg=arg,before=before,after=after,repeat=snap(fc),messages=messages,repeat_messages=arcpy.GetMessages())
 except Exception as e:result[name]=dict(arg=arg,before=before,error=str(e),messages=arcpy.GetMessages(),after=snap(fc))
# Unsupported shapefile control uses only disposable data.
shape=arcpy.management.CreateFeatureclass(root,'point.shp','POINT',spatial_reference=sr).getOutput(0);arcpy.management.AddField(shape,'D1','DATE')
try:
 arcpy.management.MigrateDateFieldToHighPrecision(shape,'D1');result['shape']=dict(messages=arcpy.GetMessages())
except Exception as e:result['shape']=dict(error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-migrate-date-probe.json'),'w') as f:json.dump(result,f,indent=2,default=str)
print('Saved /tmp/unigis-migrate-date-probe.json')
