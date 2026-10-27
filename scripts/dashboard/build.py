#!/usr/bin/env python3
"""Builds docs/dashboard.html: how far UniGIS is from having every ArcGIS Pro tool.

    python3 scripts/dashboard/build.py            # offline: files and git only
    python3 scripts/dashboard/build.py --ci       # also asks GitHub for the latest CI runs (gh)

Open docs/dashboard.html in a browser. The `.githooks/post-commit` hook rebuilds it after every
commit. ArcGIS's tool list (docs/dashboard/arcgis_tools.json) comes from arcpy.ListTools() through
the bridge (scripts/dashboard/list_arcgis_tools.py); without it the page lists only our tools.
"""
import datetime
import html
import json
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
OUT = ROOT / "docs" / "dashboard.html"


def norm(name: str) -> str:
    return re.sub(r"[^a-z0-9]", "", name.lower())


def read(path: Path) -> str:
    try:
        return path.read_text()
    except OSError:
        return ""


def tool_names(text: str) -> list[str]:
    """Literal Tool headers only: Param.name is not a tool declaration."""
    names = re.findall(r'\bTool\s*\{\s*id:\s*"[^"]+",\s*name:\s*"([^"]+)"', text)
    names += re.findall(r'_tool!\(\s*\w+,\s*"[^"]+",\s*"([^"]+)",?\s*\)', text)
    return names


def implemented() -> dict[str, str]:
    """Normalized names of tools exposed by registries, commands or aliases."""
    found: dict[str, str] = {}
    for crate, kind in (("unigis-raster", "raster registry"), ("unigis-tools", "tools registry")):
        folder = ROOT / "crates" / crate / "src"
        text = read(folder / "registry.rs")
        for name in tool_names(text):
            found.setdefault(norm(name), kind)
        # Constants registered from other modules, including newly added modules.
        for module, constant in re.findall(r'crate::(\w+)::([A-Z][A-Z_0-9]+)\s*,', text):
            source = read(folder / f"{module}.rs")
            decl = re.search(rf'\bconst\s+{re.escape(constant)}\s*:\s*Tool\s*=\s*(Tool\s*\{{.*?^\}};)', source, re.M | re.S)
            if decl:
                for name in tool_names(decl.group(1)):
                    found.setdefault(norm(name), kind)
    cli = read(ROOT / "crates/unigis-cli/src/main.rs")
    block = cli[cli.find("enum Tool {"):]
    block = block[: block.find("\n}\n")]
    for m in re.finditer(r"^    ([A-Z][A-Za-z0-9]+)\s*[{,]", block, re.M):
        found.setdefault(norm(m.group(1)), "ug command")
    aliases = read(ROOT / "crates/unigis-cli/src/aliases.rs")
    for m in re.finditer(r'"([a-z0-9_]+)" =>', aliases):
        found.setdefault(norm(m.group(1)), "alias")
    return found


def verified(manifest: dict) -> dict[str, int]:
    """normalized tool name -> number of ArcGIS reference cases."""
    count: Counter = Counter()
    for case in manifest.get("cases", {}).values():
        if case.get("status") != "succeeded":
            continue
        tool = case.get("tool", "")
        name = tool.split(".", 1)[-1]
        name = re.sub(r"_(sa|3d|ga|stats|na|md|ba)$", "", name)
        count[norm(name)] += 1
    return dict(count)


def coverage_status(name: str, have: dict, references: dict) -> str:
    key = norm(name)
    if key not in have:
        return "missing"
    return "verified" if key in references else "implemented"


def git(*args: str) -> str:
    try:
        return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def ci_runs() -> list[dict]:
    try:
        out = subprocess.run(
            ["gh", "run", "list", "--workflow", "CI", "--limit", "8", "--json",
             "conclusion,status,displayTitle,createdAt,url"],
            cwd=ROOT, capture_output=True, text=True, timeout=30,
        ).stdout
        return json.loads(out)
    except (OSError, subprocess.SubprocessError, ValueError):
        return []


def roadmap_counts() -> tuple[int, int]:
    text = read(ROOT / "docs/ROADMAP.md")
    return len(re.findall(r"^- \[x\]", text, re.M)), len(re.findall(r"^- \[ \]", text, re.M))


def section(path: str, heading: str) -> str:
    text = read(ROOT / path)
    m = re.search(rf"^##+ {re.escape(heading)}.*?\n(.*?)(?=^##+ |\Z)", text, re.M | re.S)
    return m.group(1).strip() if m else ""


def main() -> None:
    arc_path = ROOT / "docs/dashboard/arcgis_tools.json"
    arc = json.loads(read(arc_path) or "{}")
    manifest = json.loads(read(ROOT / "testdata/arcgis-reference/manifest.json") or "{}")
    have = implemented()
    ref = verified(manifest)

    rows = []  # (toolbox, name, status)
    for tbx, names in sorted(arc.get("by_toolbox", {}).items()):
        for name in names:
            status = coverage_status(name, have, ref)
            rows.append((tbx, name, status))
    total = len(rows)
    by_status = Counter(r[2] for r in rows)
    per_toolbox: dict[str, Counter] = defaultdict(Counter)
    for tbx, _, status in rows:
        per_toolbox[tbx][status] += 1

    done, todo = roadmap_counts()
    commits = [line.split("\t", 2) for line in git("log", "-n", "25", "--format=%h\t%ad\t%s", "--date=short").splitlines()]
    day = Counter(git("log", "--since=30 days ago", "--format=%ad", "--date=short").split())
    runs = ci_runs() if "--ci" in sys.argv else []
    next_actions = section("docs/HANDOFF.md", "Next actions")
    gaps = section("docs/KNOWN_GAPS.md", "Raster registry and Pairwise tools (2026-10-25)")

    data = {
        "generated": datetime.datetime.now().isoformat(timespec="minutes"),
        "arcgis_total": total,
        "have_total": len(have),
        "status": by_status,
        "rows": rows,
        "toolbox": {k: dict(v) for k, v in per_toolbox.items()},
        "cases": sum(ref.values()),
        "case_tools": len(ref),
        "roadmap": [done, todo],
        "commits": commits,
        "perday": sorted(day.items()),
        "ci": runs,
        "next": next_actions,
        "gaps": gaps,
        "ours_only": sorted(have),
        "arc_version": arc.get("version", ""),
    }
    OUT.write_text(TEMPLATE.replace("/*DATA*/", json.dumps(data, default=list).replace("</", "<\\/")))
    print(f"wrote {OUT.relative_to(ROOT)}: {len(have)} tools of ours, {total} ArcGIS tools listed, "
          f"{by_status.get('verified', 0)} verified, {by_status.get('implemented', 0)} implemented")


TEMPLATE = r"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>UniGIS Progress</title>
<style>
:root{--bg:#f6f7f9;--card:#fff;--ink:#1b1f24;--mute:#667085;--line:#e3e6eb;--ok:#2f9e5b;--warn:#d98e04;--bad:#cf3f3f;--blue:#2f6fdc}
@media (prefers-color-scheme:dark){:root{--bg:#14171c;--card:#1c2027;--ink:#e8eaed;--mute:#98a2b3;--line:#2b313a;--ok:#4cc27f;--warn:#e5a93a;--bad:#e5645f;--blue:#6b9cf0}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:14px/1.45 system-ui,sans-serif}
header{padding:20px 24px 8px}h1{margin:0;font-size:22px}header p{margin:4px 0 0;color:var(--mute)}
main{padding:12px 24px 40px;display:grid;gap:16px;grid-template-columns:repeat(auto-fit,minmax(340px,1fr))}
.card{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:16px;min-width:0}
.card h2{margin:0 0 10px;font-size:14px;color:var(--mute);text-transform:uppercase;letter-spacing:.04em}
.wide{grid-column:1/-1}.big{font-size:34px;font-weight:650}.row{display:flex;gap:24px;flex-wrap:wrap}
.stat small{display:block;color:var(--mute)}
.bar{display:flex;height:14px;border-radius:7px;overflow:hidden;background:var(--line)}
.bar i{display:block}.v{background:var(--ok)}.i{background:var(--blue)}.m{background:var(--line)}
.tb{display:grid;grid-template-columns:150px 1fr 90px;gap:10px;align-items:center;margin:6px 0;font-size:13px}
.tb span:last-child{color:var(--mute);text-align:right}
table{width:100%;border-collapse:collapse;font-size:13px}th,td{text-align:left;padding:5px 8px;border-bottom:1px solid var(--line)}
.pill{padding:1px 8px;border-radius:9px;font-size:12px;color:#fff}.pill.verified{background:var(--ok)}.pill.implemented{background:var(--blue)}.pill.missing{background:var(--mute)}
input,select{padding:6px 8px;border:1px solid var(--line);border-radius:6px;background:var(--card);color:var(--ink);margin-right:6px}
pre{white-space:pre-wrap;margin:0;font:12.5px/1.45 ui-monospace,monospace}
.spark{display:flex;align-items:flex-end;gap:2px;height:60px}.spark i{flex:1;background:var(--blue);min-height:2px;border-radius:2px}
.ok{color:var(--ok)}.bad{color:var(--bad)}.warn{color:var(--warn)}a{color:var(--blue)}
.scroll{max-height:420px;overflow:auto}
</style></head><body>
<header><h1>UniGIS: toward every ArcGIS Pro tool</h1>
<p id="meta"></p></header>
<main id="main"></main>
<script>
const D=/*DATA*/;
const $=(h)=>{const d=document.createElement('div');d.innerHTML=h;return d.firstElementChild};
const esc=s=>String(s).replace(/[&<>]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]));
const S=D.status, T=D.arcgis_total||0, vv=S.verified||0, ii=S.implemented||0, mm=S.missing||0;
const pct=n=>T?(100*n/T).toFixed(1)+'%':'-';
document.getElementById('meta').textContent='Generated '+D.generated+' - rebuild: python3 scripts/dashboard/build.py';
const main=document.getElementById('main');
const add=(h)=>main.appendChild($(h));
add(`<section class="card"><h2>Coverage of ArcGIS tools</h2><div class="row">
 <div class="stat"><div class="big">${T?pct(vv+ii):'-'}</div><small>${vv+ii} of ${T} ArcGIS tools</small></div>
 <div class="stat"><div class="big ok">${vv}</div><small>implemented with ArcGIS reference cases</small></div>
 <div class="stat"><div class="big" style="color:var(--blue)">${ii}</div><small>implemented, no reference case</small></div>
 <div class="stat"><div class="big">${mm}</div><small>missing</small></div></div>
 <div class="bar" style="margin-top:14px"><i class="v" style="width:${T?100*vv/T:0}%"></i><i class="i" style="width:${T?100*ii/T:0}%"></i></div>
 <p>Reference coverage records saved successful ArcGIS cases for implemented tools; it is not a claim of full compatibility. See known gaps and CI results below.</p>
 ${T?'':'<p class="warn">No ArcGIS tool list yet: run scripts/dashboard/list_arcgis_tools.py through the bridge and save docs/dashboard/arcgis_tools.json.</p>'}</section>`);
add(`<section class="card"><h2>Work so far</h2><div class="row">
 <div class="stat"><div class="big">${D.have_total}</div><small>tools of ours</small></div>
 <div class="stat"><div class="big">${D.cases}</div><small>ArcGIS reference cases (${D.case_tools} tools)</small></div>
 <div class="stat"><div class="big">${D.roadmap[0]}</div><small>roadmap items done (${D.roadmap[1]} open)</small></div></div>
 <h2 style="margin-top:16px">Commits per day (30 days)</h2><div class="spark">${(()=>{const m=Math.max(1,...D.perday.map(x=>x[1]));return D.perday.map(x=>`<i title="${x[0]}: ${x[1]}" style="height:${100*x[1]/m}%"></i>`).join('')})()}</div></section>`);
if(D.ci.length)add(`<section class="card"><h2>CI</h2>${D.ci.map(r=>`<div><span class="${r.conclusion==='success'?'ok':r.status==='completed'?'bad':'warn'}">${r.conclusion||r.status}</span> <a href="${r.url}">${esc(r.displayTitle)}</a> <small>${r.createdAt.slice(0,16)}</small></div>`).join('')}</section>`);
add(`<section class="card"><h2>By toolbox</h2>${Object.entries(D.toolbox).sort((a,b)=>Object.values(b[1]).reduce((x,y)=>x+y,0)-Object.values(a[1]).reduce((x,y)=>x+y,0)).map(([k,c])=>{const n=(c.verified||0)+(c.implemented||0)+(c.missing||0);return `<div class="tb"><span>${esc(k)}</span><div class="bar"><i class="v" style="width:${100*(c.verified||0)/n}%"></i><i class="i" style="width:${100*(c.implemented||0)/n}%"></i></div><span>${(c.verified||0)+(c.implemented||0)} / ${n}</span></div>`}).join('')||'<p>No data.</p>'}</section>`);
add(`<section class="card"><h2>Next actions</h2><pre>${esc(D.next||'-')}</pre></section>`);
add(`<section class="card"><h2>Recent commits</h2><div class="scroll"><table>${D.commits.map(c=>`<tr><td>${c[0]}</td><td>${c[1]}</td><td>${esc(c[2])}</td></tr>`).join('')}</table></div></section>`);
add(`<section class="card"><h2>Known gaps (latest)</h2><pre>${esc(D.gaps||'-')}</pre></section>`);
add(`<section class="card wide"><h2>All ArcGIS tools</h2>
 <input id="q" placeholder="search"><select id="st"><option value="">any status</option><option>verified</option><option>implemented</option><option>missing</option></select><select id="tbx"><option value="">any toolbox</option>${Object.keys(D.toolbox).sort().map(k=>`<option>${esc(k)}</option>`).join('')}</select><span id="count"></span>
 <div class="scroll" style="margin-top:8px"><table><thead><tr><th>Tool</th><th>Toolbox</th><th>Status</th></tr></thead><tbody id="rows"></tbody></table></div></section>`);
const rows=document.getElementById('rows');
function draw(){const q=document.getElementById('q').value.toLowerCase(),s=document.getElementById('st').value,t=document.getElementById('tbx').value;
 const r=D.rows.filter(x=>(!q||x[1].toLowerCase().includes(q))&&(!s||x[2]===s)&&(!t||x[0]===t));
 document.getElementById('count').textContent=r.length+' tools';
 rows.innerHTML=r.slice(0,600).map(x=>`<tr><td>${esc(x[1])}</td><td>${esc(x[0])}</td><td><span class="pill ${x[2]}">${x[2]}</span></td></tr>`).join('');}
['q','st','tbx'].forEach(i=>document.getElementById(i).addEventListener('input',draw));draw();
</script></body></html>
"""

if __name__ == "__main__":
    main()
