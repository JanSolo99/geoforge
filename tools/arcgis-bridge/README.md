# ArcGIS bridge (MCP)

Lets Claude Code drive the ArcGIS Pro installed on this machine (under Wine/GE-Proton)
so GeoForge can be checked against real ArcGIS behavior: run geoprocessing tools on
test data, `Describe` datasets, run arcpy snippets.

Black-box only: it runs tools and returns their results. It never inspects ArcGIS
internals (see the clean-room rule in `CLAUDE.md`).

- `server.py` — MCP server, runs natively (`uv run --script`, depends on `mcp>=2`).
- `worker.py` — runs inside ArcGIS Pro's own Python (`arcgispro-py3`) under Wine,
  standard library + arcpy only; nothing is installed into the ArcGIS environment.
  It stays running between calls (startup is ~8 s).

Tools: `arcgis_info`, `arcgis_run_tool`, `arcgis_describe`, `arcgis_python`,
`arcgis_restart`. Linux paths in tool parameters are translated to Wine `Z:\` paths and
back in results.

Register (already done for this project, local scope):

```sh
claude mcp add --scope local arcgis -- uv run --script tools/arcgis-bridge/server.py
```

Configuration via environment: `ARCGIS_WINE` (default GE-Proton 11-7's wine),
`ARCGIS_WINEPREFIX` (default `~/.wine`), `ARCGIS_PYTHON`, `ARCGIS_TIMEOUT` (600 s).
