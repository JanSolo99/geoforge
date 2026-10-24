"""Prints what each operation of a gdb-reference ops set changed against its base snapshot:
    python3 scripts/research/unsplit/diff_ops.py testdata/gdb-reference/domain_ops
"""
import json
import re
import sys

root = sys.argv[1]
base = json.load(open(f"{root}/base.json"))
manifest = json.load(open(f"{root}/ops.json"))
base_items = dict(((i["name"], i["type"]), i) for i in base["items"])


def short(d):
    d = re.sub(r"<\?xml[^>]*\?>", "", d)
    return re.sub(r'xmlns:\w+="[^"]*"|xsi:type="[^"]*"', "", d)


for op in manifest["ops"]:
    s = json.load(open(f"{root}/{op}.json"))
    print("==", op, manifest["arcgis"][op][:60])
    items = dict(((i["name"], i["type"]), i) for i in s["items"])
    for k, i in items.items():
        if k not in base_items:
            print("  NEW item", k, i["path"], short(i["definition"])[:700])
        elif i != base_items[k]:
            print("  CHANGED", k, short(i["definition"])[:700])
    for k in base_items:
        if k not in items:
            print("  GONE", k)
    if s["tables"] != base["tables"]:
        print("  tables", base["tables"], "->", s["tables"])
    new_rels = [r for r in s["rels"] if r not in base["rels"]]
    gone_rels = [r for r in base["rels"] if r not in s["rels"]]
    if new_rels or gone_rels:
        print("  rels +", new_rels, "-", gone_rels)
    for t in s["rows"]:
        if t not in base["rows"]:
            print("  new table rows", t, repr(s["rows"][t])[:300], s["fields"].get(t))
