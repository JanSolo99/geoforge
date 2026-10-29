"""Public DeleteMultiple controls: delete only tool-created disposable copies."""
import json,os,shutil
root=P('/tmp/unigis-delete-multiple-oracle')
os.makedirs(root,exist_ok=True)
sr=arcpy.SpatialReference(32633)
results={}
def prepare(name):
 folder=os.path.join(root,name);os.makedirs(folder,exist_ok=False)
 fc=arcpy.management.CreateFeatureclass(folder,'points.shp','POINT',spatial_reference=sr).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG')
 with arcpy.da.InsertCursor(fc,['SHAPE@XY','KEY']) as c:c.insertRow(((500000,5000000),7))
 arcpy.management.CreateTable(folder,'table.dbf')
 for file in ['points.txt','points.other.shp','keep.txt']:
  with open(os.path.join(folder,file),'w') as f:f.write('preserve')
 return folder,fc
def snap(folder):return sorted(os.listdir(folder))
for case in ['shape_table','missing','mixed_missing','duplicate','type_fc','wrong_type','invalid_type','dbf_of_shape','folder','gdb_items','whole_gdb','empty','quoted_string','plain_file']:
 folder,fc=prepare(case)
 table=os.path.join(folder,'table.dbf')
 rows={'shape_table':[[fc],[table]],'missing':[[os.path.join(folder,'absent.shp')]],'mixed_missing':[[fc],[os.path.join(folder,'absent.shp')],[table]],'duplicate':[[fc],[fc]],'type_fc':[[fc,'FeatureClass']],'wrong_type':[[fc,'RelationshipClass']],'invalid_type':[[fc,'Banana']],'dbf_of_shape':[[os.path.join(folder,'points.dbf')]],'folder':[[os.path.join(folder,'nested')]],'empty':[], 'quoted_string':"'"+fc+"' #;'"+table+"' #",'plain_file':[[os.path.join(folder,'keep.txt')]]}.get(case)
 if case=='folder':os.makedirs(rows[0][0]);open(os.path.join(rows[0][0],'inside.txt'),'w').write('delete')
 if case in ['gdb_items','whole_gdb']:
  gdb=arcpy.management.CreateFileGDB(folder,'data.gdb').getOutput(0)
  arcpy.management.CopyFeatures(fc,os.path.join(gdb,'points'))
  arcpy.management.CreateTable(gdb,'table')
  rows=[[os.path.join(gdb,'points'),'FeatureClass'],[os.path.join(gdb,'table')]] if case=='gdb_items' else [[gdb]]
 before=snap(folder)
 try:
  r=arcpy.management.DeleteMultiple(rows)
  entry={'success':True,'output':r.getOutput(0),'messages':arcpy.GetMessages()}
 except Exception as e:entry={'success':False,'error':str(e),'messages':arcpy.GetMessages()}
 entry.update(before=before,after=snap(folder))
 if case=='gdb_items':entry['remaining_gdb']=arcpy.ListFeatureClasses(feature_dataset=gdb) if False else sorted(os.listdir(gdb))
 results[case]=entry
with open(P('/tmp/unigis-delete-multiple-probe.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-delete-multiple-probe.json')
