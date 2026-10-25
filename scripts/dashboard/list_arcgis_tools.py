import json
tools = arcpy.ListTools()
out = {}
for t in tools:
    name, _, tbx = t.rpartition("_")
    out.setdefault(tbx, []).append(name)
print(json.dumps({"count": len(tools), "by_toolbox": {k: sorted(v) for k, v in out.items()}}))
