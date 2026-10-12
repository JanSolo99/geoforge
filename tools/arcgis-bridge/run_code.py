"""Runs arcpy code from a file through a fresh ArcGIS worker, without the MCP
server (useful when the MCP session is unavailable or out of step):

    uv run -q --with "mcp>=2" python tools/arcgis-bridge/run_code.py code.py

Inside the code, wrap Linux paths with P('/path') as with arcgis_python.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import server  # noqa: E402

code = open(sys.argv[1]).read()
r = server.worker.call("python", timeout=1200, code=code)
print(r.get("stdout", ""), r.get("error", ""), r.get("traceback", ""))
server.worker.stop()
