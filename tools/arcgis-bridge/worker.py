"""arcpy worker for the UniGIS ArcGIS bridge.

Runs inside ArcGIS Pro's Python (arcgispro-py3) under Wine. Reads one JSON request
per line on stdin and writes one JSON response per line on stdout. Standard library
and arcpy only: nothing is installed into the ArcGIS environment.

Requests:  {"id": n, "op": "info" | "run_tool" | "python" | "describe", ...}
Responses: {"id": n, "ok": true, ...} or {"id": n, "ok": false, "error": "...", "traceback": "..."}
"""

import contextlib
import io
import json
import sys
import traceback

PROTOCOL_OUT = sys.stdout.buffer  # keep the real stdout for protocol messages only
sys.stdout = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")  # stray prints go nowhere

import arcpy  # noqa: E402  (after redirecting stdout: arcpy may print on import)

def P(path):
    """Linux path -> Wine path, for use in arcgis_python code: P('/mnt/data/x.shp')."""
    return "Z:" + path.replace("/", "\\") if path.startswith("/") else path


NAMESPACE = {"arcpy": arcpy, "P": P}


def send(msg):
    PROTOCOL_OUT.write(json.dumps(msg, default=str).encode("utf-8") + b"\n")
    PROTOCOL_OUT.flush()


def resolve_tool(name):
    """'analysis.Buffer', 'Buffer_analysis' or 'management.CopyFeatures' -> callable."""
    if "." in name:
        obj = arcpy
        for part in name.split("."):
            obj = getattr(obj, part)
        return obj
    return getattr(arcpy, name)


def messages(result=None):
    out = []
    count = result.messageCount if result is not None else arcpy.GetMessageCount()
    for i in range(count):
        if result is not None:
            out.append({"severity": result.getSeverity(i), "text": result.getMessage(i)})
        else:
            out.append({"severity": arcpy.GetSeverity(i), "text": arcpy.GetMessage(i)})
    return out


def op_info(req):
    info = arcpy.GetInstallInfo()
    return {
        "product": info.get("ProductName"),
        "version": info.get("Version"),
        "build": info.get("BuildNumber"),
        "license": arcpy.ProductInfo(),
        "python": sys.version,
        "scratch_folder": arcpy.env.scratchFolder,
    }


def op_run_tool(req):
    tool = resolve_tool(req["tool"])
    args = req.get("args") or []
    kwargs = req.get("kwargs") or {}
    env = req.get("env") or {}
    saved = {k: getattr(arcpy.env, k) for k in env}
    try:
        for k, v in env.items():
            setattr(arcpy.env, k, v)
        try:
            result = tool(*args, **kwargs)
        except arcpy.ExecuteError:
            return {"status": "failed", "messages": messages()}
        outputs = []
        if hasattr(result, "outputCount"):
            outputs = [result.getOutput(i) for i in range(result.outputCount)]
        return {
            "status": "succeeded",
            "outputs": outputs,
            "messages": messages(result) if hasattr(result, "messageCount") else messages(),
        }
    finally:
        for k, v in saved.items():
            setattr(arcpy.env, k, v)


def op_python(req):
    """Executes code in a persistent namespace. Printed output and `result` come back."""
    buf = io.StringIO()
    NAMESPACE.pop("result", None)
    with contextlib.redirect_stdout(buf):
        exec(req["code"], NAMESPACE)
    return {"stdout": buf.getvalue(), "result": NAMESPACE.get("result")}


def op_describe(req):
    d = arcpy.Describe(req["path"])
    props = {}
    for key in ("dataType", "shapeType", "hasZ", "hasM", "OIDFieldName", "shapeFieldName",
                "featureType", "hasSpatialIndex", "workspaceType", "catalogPath"):
        if hasattr(d, key):
            props[key] = getattr(d, key)
    if hasattr(d, "extent") and d.extent is not None:
        e = d.extent
        props["extent"] = [e.XMin, e.YMin, e.XMax, e.YMax, e.ZMin, e.ZMax, e.MMin, e.MMax]
    if hasattr(d, "spatialReference") and d.spatialReference is not None:
        sr = d.spatialReference
        props["spatialReference"] = {
            "name": sr.name, "factoryCode": sr.factoryCode, "type": sr.type,
            "XYResolution": sr.XYResolution, "XYTolerance": sr.XYTolerance,
        }
    if hasattr(d, "fields"):
        props["fields"] = [
            {"name": f.name, "aliasName": f.aliasName, "type": f.type, "length": f.length,
             "precision": f.precision, "scale": f.scale, "isNullable": f.isNullable,
             "domain": f.domain, "required": f.required}
            for f in d.fields
        ]
    if hasattr(d, "children"):
        props["children"] = [{"name": c.name, "dataType": c.dataType} for c in d.children]
    return props


OPS = {"info": op_info, "run_tool": op_run_tool, "python": op_python, "describe": op_describe}


def main():
    send({"id": 0, "ok": True, "ready": True})
    for line in sys.stdin.buffer:
        if not line.strip():
            continue
        req = json.loads(line.decode("utf-8"))
        try:
            send({"id": req.get("id"), "ok": True, **OPS[req["op"]](req)})
        except Exception as e:  # report every failure to the caller
            send({"id": req.get("id"), "ok": False, "error": repr(e), "traceback": traceback.format_exc()})


if __name__ == "__main__":
    main()
