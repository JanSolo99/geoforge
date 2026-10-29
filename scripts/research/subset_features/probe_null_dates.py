"""Public dBASE NULL-date export behavior, separate from record selection."""
import os,json,datetime
root=P('/tmp/unigis-subset-null-dates');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
fc=arcpy.management.CreateTable(gdb,'dates').getOutput(0)
arcpy.management.AddField(fc,'KEY','LONG');arcpy.management.AddField(fc,'STAMP','DATE')
with arcpy.da.InsertCursor(fc,['KEY','STAMP'],explicit=True) as c:
 for row in [(1,None),(2,datetime.datetime(2020,1,2)),(3,None),(4,datetime.datetime(2021,2,3)),(5,None)]:c.insertRow(row)
out=os.path.join(root,'all.dbf');arcpy.management.SubsetFeatures(fc,out,'',100,'PERCENTAGE_OF_INPUT')
result=dict(source=list(arcpy.da.SearchCursor(fc,['KEY','STAMP'])),output=list(arcpy.da.SearchCursor(out,['KEY','STAMP'])))
with open(P('/tmp/unigis-subset-null-dates.json'),'w') as f:json.dump(result,f,indent=2,default=str)
print(json.dumps(result,indent=2,default=str))
