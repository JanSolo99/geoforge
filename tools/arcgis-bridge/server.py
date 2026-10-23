# /// script
# requires-python = ">=3.10"
# dependencies = ["mcp>=2"]
# ///
"""MCP server exposing ArcGIS Pro (arcpy) running under Wine on this machine.

UniGIS uses it as a black-box oracle: run ArcGIS geoprocessing tools on test data
and compare their outputs with ours. It never inspects ArcGIS internals.

Configuration (environment variables):
  ARCGIS_WINE        wine binary (default: GE-Proton 11-7 from Steam)
  ARCGIS_WINEPREFIX  prefix with ArcGIS Pro installed (default: ~/.wine)
  ARCGIS_PYTHON      Windows path of ArcGIS Pro's python.exe
  ARCGIS_TIMEOUT     default per-call timeout in seconds (default 600)
"""

import json
import os
import re
import subprocess
import threading
from pathlib import Path

from mcp.server.mcpserver import MCPServer

HOME = Path.home()
WINE = os.environ.get(
    "ARCGIS_WINE",
    str(HOME / ".local/share/Steam/compatibilitytools.d/GE-Proton11-7-x86_64/files/bin/wine"),
)
PREFIX = os.environ.get("ARCGIS_WINEPREFIX", str(HOME / ".wine"))
PYTHON = os.environ.get(
    "ARCGIS_PYTHON",
    r"C:\Program Files\ArcGIS\Pro\bin\Python\envs\arcgispro-py3\python.exe",
)
TIMEOUT = float(os.environ.get("ARCGIS_TIMEOUT", "600"))
WORKER = Path(__file__).with_name("worker.py")

mcp = MCPServer("arcgis")


# --- path translation -------------------------------------------------------------

def to_windows(value):
    """Linux absolute paths (in strings, lists, dicts) -> Wine Z: paths."""
    if isinstance(value, str) and value.startswith("/"):
        return "Z:" + value.replace("/", "\\")
    if isinstance(value, list):
        return [to_windows(v) for v in value]
    if isinstance(value, dict):
        return {k: to_windows(v) for k, v in value.items()}
    return value


_ZPATH = re.compile(r"\b[Zz]:\\[^\s\"';,]*")


def to_linux(value):
    """Wine Z: paths inside results -> Linux paths."""
    if isinstance(value, str):
        return _ZPATH.sub(lambda m: m.group(0)[2:].replace("\\", "/"), value)
    if isinstance(value, list):
        return [to_linux(v) for v in value]
    if isinstance(value, dict):
        return {k: to_linux(v) for k, v in value.items()}
    return value


# --- worker process ---------------------------------------------------------------

class Worker:
    """One persistent arcpy process; calls are serialized."""

    def __init__(self):
        self.proc = None
        self.next_id = 1
        self.lock = threading.Lock()

    def _start(self):
        env = dict(os.environ, WINEPREFIX=PREFIX, WINEDEBUG="-all",
                   PYTHONIOENCODING="utf-8", PYTHONUNBUFFERED="1")
        self.proc = subprocess.Popen(
            [WINE, PYTHON, "-u", to_windows(str(WORKER))],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env,
        )
        ready = None
        while ready is None:
            ready = self._read(timeout=300)
        if not ready.get("ready"):
            raise RuntimeError(f"arcpy worker failed to start: {ready}")

    def _read(self, timeout):
        result = {}

        def reader():
            line = self.proc.stdout.readline()
            result["line"] = line

        t = threading.Thread(target=reader, daemon=True)
        t.start()
        t.join(timeout)
        if t.is_alive():
            self.stop()
            raise TimeoutError(f"ArcGIS did not answer within {timeout:.0f} s; worker restarted")
        line = result.get("line", b"")
        if not line:
            self.stop()
            raise RuntimeError("arcpy worker exited unexpectedly")
        try:
            return json.loads(line.decode("utf-8", "replace"))
        except json.JSONDecodeError:
            # Some tools print straight to the process's stdout; skip such lines.
            return None

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.kill()
        self.proc = None

    def call(self, op, timeout=None, **payload):
        with self.lock:
            if self.proc is None or self.proc.poll() is not None:
                self._start()
            req = {"id": self.next_id, "op": op, **to_windows(payload)}
            self.next_id += 1
            self.proc.stdin.write(json.dumps(req).encode("utf-8") + b"\n")
            self.proc.stdin.flush()
            while True:
                resp = self._read(timeout or TIMEOUT)
                if isinstance(resp, dict) and resp.get("id") == req["id"]:
                    return to_linux(resp)


worker = Worker()


def _format(resp):
    return json.dumps(resp, indent=2, ensure_ascii=False, default=str)


# --- tools ------------------------------------------------------------------------

@mcp.tool()
def arcgis_info() -> str:
    """ArcGIS Pro version, license level and Python version of the arcpy worker.
    The first call starts the worker (about 10 seconds)."""
    return _format(worker.call("info"))


@mcp.tool()
def arcgis_run_tool(tool: str, args: list | None = None, kwargs: dict | None = None,
                    env: dict | None = None, timeout_s: float | None = None) -> str:
    """Run an ArcGIS geoprocessing tool and return its status, outputs and messages.

    tool: arcpy name, e.g. "analysis.Buffer", "management.CopyFeatures" or
          "Buffer_analysis".
    args/kwargs: tool parameters as arcpy expects them. Linux paths (starting with
          "/") are translated to Wine paths automatically, and back in the results.
    env: arcpy.env settings for this call only, e.g. {"overwriteOutput": true,
          "XYResolution": "0.0001 Meters"}.
    """
    return _format(worker.call("run_tool", timeout=timeout_s, tool=tool,
                               args=args or [], kwargs=kwargs or {}, env=env or {}))


@mcp.tool()
def arcgis_describe(path: str) -> str:
    """arcpy.Describe of a dataset or workspace: data type, shape type, Z/M, extent,
    spatial reference (name, WKID, resolution, tolerance), fields (type, length,
    precision, scale, nullability, domain) and children."""
    return _format(worker.call("describe", path=path))


@mcp.tool()
def arcgis_python(code: str, timeout_s: float | None = None) -> str:
    """Execute Python in the persistent arcpy session (arcpy is imported; variables
    persist between calls). Printed output is returned, plus the value of a variable
    named `result` if the code sets one. Linux paths inside code are not translated
    automatically: wrap them with the helper `P('/mnt/data/x.shp')`. Paths in the
    returned output are translated back to Linux form.
    """
    return _format(worker.call("python", timeout=timeout_s, code=code))


@mcp.tool()
def arcgis_restart() -> str:
    """Kill and restart the arcpy worker (clears the Python session)."""
    worker.stop()
    return _format(worker.call("info"))


if __name__ == "__main__":
    mcp.run()
