"""Public ValidateTableName controls for ordinary folder outputs; no files written."""
import json
names=['.','../path','_abc','-abc','  A ','a.b','a-b','','9abc','a$b','A/B','A\\B','x:y','a,b','a(1)','abc_','a!b']
result={n:arcpy.ValidateTableName(n,P('/tmp/unigis-sba-collision-existing')) for n in names}
with open(P('/tmp/unigis-sba-valid-names.json'),'w',newline='\n') as f:json.dump(result,f,indent=2)
print('Saved /tmp/unigis-sba-valid-names.json')
