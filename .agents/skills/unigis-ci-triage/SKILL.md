---
name: unigis-ci-triage
description: Diagnose UniGIS CI or local validation failures without weakening compatibility checks.
---

# unigis-ci-triage

1. Follow AGENTS.md startup rules. Identify the failing revision, job, exact command, first causal error and environment versions. Retrieve only relevant log sections. Preserve the full log path for later investigation.
2. Classify: dependency/network/resource failure; formatting/lint; compile/type; assertion/behavior; flaky timing. Do not edit code to fix authentication, unavailable fixtures or an absent compiler. Do not repeatedly rebuild after an unchanged environmental failure.
3. Reproduce the smallest failing test with the same fixtures and feature flags, `--locked -j3`. Inspect scripts/check and the workflow before assuming CI equals local defaults. No concurrent builds. If a live service is missing, report it; a skip is not a pass.
4. For format mismatches, compare the native reader and installed GDAL on the exact fixture. Check upstream evidence for version-specific oracle defects; retain direct native assertions and narrowly gate only a proven oracle incompatibility. For geoprocessing mismatches, ArcGIS Pro is the reference, never GDAL. Do not fabricate or casually regenerate golden files.
5. Fix the cause. Run the reproducer, related tests and required project gates. For workflow edits validate YAML/action syntax and failure propagation (especially piped commands). An upload or deployment must depend on successful validation.
6. Record root cause, reproducer, fix, versions, test results and remaining limits. After two failed attempts on the same hypothesis, switch hypotheses using evidence or prepare a focused escalation packet. Never disable checks just to obtain green CI.
