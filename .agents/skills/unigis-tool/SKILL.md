---
name: unigis-tool
description: Implement one queued or user-requested ArcGIS-compatible geoprocessing tool using evidence and existing registries.
---

# unigis-tool

1. Follow AGENTS.md and CLAUDE.md. User requests come first. Otherwise finish the current tool and select according to docs/WORK_QUEUE.md. Do not claim a lane or launch agents unless operating in an explicitly authorized lane.
2. Run `ug tools` using the available local binary, or inspect registries if it is not built. Search aliases before implementing a duplicate. Find a nearby tool with the same input/output types. Read only the matching ROADMAP, KNOWN_GAPS and inventory sections.
3. State the ArcGIS name, parameters, defaults, output schema/geometry/CRS, empty-input behavior and error cases. Consult public documentation and existing saved references. Use the workstation ArcGIS bridge to resolve unknown behavior when available. If unavailable, implement only what evidence supports and record the unresolved behavior; never substitute GDAL geoprocessing behavior.
4. Follow CLAUDE.md's reference workflow: add cases to scripts/make-arcgis-reference.py and RUN, generate/annotate via the bridge when available, inspect the actual results. Preserve the clean-room boundary. Do not manufacture ArcGIS output or change comparison tolerances to hide differences.
5. Implement in the existing vector/table or raster registry; use CLI aliases for renames. Avoid a bespoke CLI command or comparator branch when the manifest path suffices. Cover normal, empty and relevant edge/error behavior; compare saved references when available.
6. Run targeted tests then scripts/check, sequentially at max 3 Cargo jobs. Update roadmap and known gaps with evidence. Queue/dashboard artifacts must use their generators. A registered tool or reference case does not establish full parity or UI availability.
7. Record an actionable handoff. For queue work, follow its deferral rules when blocked; for a user task explain the blocker and continue independent requested work. One tool per landing; do not publish or transfer changes without authorization.
