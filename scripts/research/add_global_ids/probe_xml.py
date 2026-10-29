import json
root=P('/tmp/unigis-globalids-probe')
arcpy.management.ExportXMLWorkspaceDocument(root+'/control.gdb',root+'/schema.xml','SCHEMA_ONLY')
print('saved schema.xml')
