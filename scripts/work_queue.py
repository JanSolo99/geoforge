#!/usr/bin/env python3
"""Builds docs/WORK_QUEUE.md: the ranked list of ArcGIS tools UniGIS does not have yet.

    python3 scripts/work_queue.py           # rewrite docs/WORK_QUEUE.md
    python3 scripts/work_queue.py --next    # print the next tool to build

ArcGIS's tool list is docs/dashboard/arcgis_tools.json (arcpy.ListTools); a tool counts as done once
a registry, command or alias exposes it (the same test as the dashboard). Priority, difficulty and
whether ArcGIS output can be reproduced exactly come from docs/arcgis-inventory/*.csv. Tools that
were tried and set aside are listed, with the reason, in docs/WORK_QUEUE_DEFERRED.txt.
"""
import csv
import re
import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts" / "dashboard"))
from build import implemented, norm  # noqa: E402

OUT = ROOT / "docs" / "WORK_QUEUE.md"
DEFERRED = ROOT / "docs" / "WORK_QUEUE_DEFERRED.txt"
SHOWN = 40

# Most-used general toolboxes first; domain toolboxes (aviation, utility network, ...) last.
TOOLBOXES = ["analysis", "management", "conversion", "sa", "stats", "3d", "edit", "cartography",
             "lr", "ga", "ra", "ia", "gapro", "geoanalytics", "sfa", "stpm", "md", "na", "td"]
NAMES = {"analysis": "Analysis", "management": "Data Management", "conversion": "Conversion",
         "sa": "Spatial Analyst", "stats": "Spatial Statistics", "3d": "3D Analyst", "edit": "Editing",
         "cartography": "Cartography", "lr": "Linear Referencing", "ga": "Geostatistical Analyst",
         "ra": "Raster Analysis", "ia": "Image Analyst", "gapro": "GeoAnalytics Desktop",
         "geoanalytics": "GeoAnalytics Server", "sfa": "Standard Feature Analysis",
         "stpm": "Space Time Pattern Mining", "md": "Multidimension", "na": "Network Analyst",
         "td": "Territory Design"}
DIFFICULTY = {"S": 0, "M": 1, "L": 2, "XL": 3}
# Pushed to the back: tools that need a server, an enterprise database or a cloud service, and
# tools that only manage map layers/views (they wait for the layer model of the tool framework).
SERVER = re.compile(r"enterprise|oracle|sap hana|sql server|postgresql|db2|teradata|\bservice\b|"
                    r"services\b|portal|arcgis online|cloud|big data|replica|version|archiving|"
                    r"\bbim\b|wcs|wms|image server|scene layer package|vector tile package|\.slpk", re.I)
LAYER = re.compile(r"^(make\w*(layer|view)|(add|remove)(join|relate)|selectlayer\w*|savetolayerfile|"
                   r"applysymbology\w*|setlayer\w*|getcount|\w*layer\w*)$", re.I)


def tier(item):
    """0 = build now, 1 = waits for the layer model, 2 = needs a server or enterprise database."""
    if SERVER.search(item["title"] + " " + item["summary"].split(". ")[0]):
        return 2
    return 1 if LAYER.match(item["name"]) else 0

RULES = """\
## How agents use this queue

1. **Finish or wrap up the current tool first.** If the work in progress is polish on a tool that
   already landed (byte-level differences, rare parameter combinations), commit what is validated,
   record the rest in `docs/KNOWN_GAPS.md`, and move on.
2. **Then take the first tool in "Up next"** that is not deferred. Skip only for a concrete reason.
3. **One tool = one landing.** Implement the default and commonly used parameters, add ArcGIS
   reference cases, pass `scripts/check`, then commit and push. Options not covered go to the roadmap
   and known gaps, not to the next action.
4. **Time box: about two coordinator turns per tool.** If a tool is not converging (untestable,
   random output, needs a licence/ArcSDE/enterprise geodatabase, or a format we cannot write yet),
   add a line to `docs/WORK_QUEUE_DEFERRED.txt` with the reason and take the next one.
5. After each landing run `python3 scripts/work_queue.py` (the post-commit hook also does) and make
   the handoff's next action "next queue item: <tool>".
6. A user request always comes first. Polish of landed tools happens only when asked.
"""


def inventory():
    """(toolbox alias, normalized function) -> inventory row."""
    rows = {}
    for path in (ROOT / "docs" / "arcgis-inventory").glob("*.csv"):
        with path.open(newline="") as f:
            for row in csv.DictReader(f):
                arcpy = row.get("arcpy_name") or ""
                if not arcpy:
                    continue
                parts = arcpy.split(".")
                rows.setdefault((parts[-2] if len(parts) > 2 else "", norm(parts[-1])), row)
                rows.setdefault(("", norm(parts[-1])), row)
    return rows


def deferred():
    """alias.Name -> reason, from lines like `management.AddIncrementingIDField  ArcSDE only`."""
    found = {}
    if DEFERRED.exists():
        for line in DEFERRED.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                key, _, reason = line.partition(" ")
                found[key] = reason.strip(" -—")
    return found


def queue():
    arc = json.loads((ROOT / "docs/dashboard/arcgis_tools.json").read_text())
    have = implemented()
    meta = inventory()
    skip = deferred()
    missing, done, held = [], Counter(), []
    totals = Counter()
    for alias, names in arc["by_toolbox"].items():
        for name in names:
            totals[alias] += 1
            row = meta.get((alias, norm(name))) or meta.get(("", norm(name))) or {}
            if norm(name) in have or norm(row.get("tool") or "") in have:
                done[alias] += 1
                continue
            item = {"alias": alias, "name": name, "title": row.get("tool") or name,
                    "priority": row.get("priority") or "P5", "difficulty": row.get("difficulty") or "?",
                    "exact": row.get("exact_repro") or "?", "license": row.get("license") or "",
                    "url": row.get("url") or "", "summary": row.get("description") or ""}
            if f"{alias}.{name}" in skip:
                held.append((item, skip[f"{alias}.{name}"]))
            else:
                missing.append(item)
    rank = {a: i for i, a in enumerate(TOOLBOXES)}
    missing.sort(key=lambda t: (tier(t), t["priority"], rank.get(t["alias"], len(rank)), t["exact"] != "Y",
                                DIFFICULTY.get(t["difficulty"], 4), t["name"]))
    return missing, held, done, totals


def main():
    missing, held, done, totals = queue()
    if "--next" in sys.argv:
        t = missing[0]
        print(f"{t['title']} ({NAMES.get(t['alias'], t['alias'])}, arcpy {t['alias']}.{t['name']}) {t['url']}")
        return
    lines = ["# Work queue", "",
             "Generated by `scripts/work_queue.py`; do not edit by hand (defer tools in "
             "`docs/WORK_QUEUE_DEFERRED.txt`). Order: inventory priority (P0 first), then toolbox "
             "(most-used first), then tools whose output ArcGIS lets us reproduce exactly, then "
             "easier ones. Tools that need a server or enterprise database, and tools that only "
             "manage map layers, come after all others.", "", RULES,
             f"## Up next ({len(missing)} tools missing; first {SHOWN} shown)", "",
             "| # | Tool | Toolbox | arcpy | Priority | Difficulty | Exact | Summary |",
             "|---|---|---|---|---|---|---|---|"]
    for i, t in enumerate(missing[:SHOWN], 1):
        title = f"[{t['title']}]({t['url']})" if t["url"] else t["title"]
        summary = t["summary"].replace("|", "/")[:110]
        lines.append(f"| {i} | {title} | {NAMES.get(t['alias'], t['alias'])} | `{t['alias']}.{t['name']}` "
                     f"| {t['priority']} | {t['difficulty']} | {t['exact']} | {summary} |")
    lines += ["", "## Progress by toolbox", "", "| Toolbox | Done | Total | |", "|---|---|---|---|"]
    for alias in sorted(totals, key=lambda a: (TOOLBOXES.index(a) if a in TOOLBOXES else 99, a)):
        n, d = totals[alias], done[alias]
        bar = "█" * round(20 * d / n) + "░" * (20 - round(20 * d / n))
        lines.append(f"| {NAMES.get(alias, alias)} | {d} | {n} | `{bar}` |")
    lines += ["", f"Overall: {sum(done.values())} of {sum(totals.values())} ArcGIS tools.", ""]
    if held:
        lines += ["## Deferred", "", "| Tool | Reason |", "|---|---|"]
        lines += [f"| `{t['alias']}.{t['name']}` | {reason} |" for t, reason in held]
        lines.append("")
    OUT.write_text("\n".join(lines))
    print(f"{OUT.relative_to(ROOT)}: {len(missing)} missing, next: {missing[0]['title']}")


if __name__ == "__main__":
    main()
