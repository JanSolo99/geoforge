---
name: unigis-delivery
description: Deliver a bounded UniGIS change with targeted context, unchanged correctness gates and a recoverable handoff.
---

# unigis-delivery

1. Read AGENTS.md, CLAUDE.md and the current worktree handoff. Check `pwd`, `git branch --show-current`, `git rev-parse --short HEAD`, `git status --short`. Inspect relevant diffs before editing; preserve inherited changes.
2. Write a brief using docs/AGENT_WORKFLOW.md: user outcome, completion evidence, likely files, validation, constraints. The current user task overrides the tool queue. Finish the requested scope; bounded slices are checkpoints, not excuses to stop early.
3. Use `rg` to find the nearest working implementation and its tests. Read applicable nested AGENTS.md. Search large catalogs by tool or subsystem. Read more only when a dependency or failure warrants it.
4. Make one coherent slice. Reproduce a correctness bug before fixing it when practical. Tests should cover observable behavior, including meaningful failure cases, rather than mirror implementation.
5. Run the targeted test first, then required gates: root `scripts/check`; app changes additionally follow app/AGENTS.md and its `cargo xtask ci`. Never overlap Cargo builds; max `-j3`. Read docs/READINESS.md when changing packaging, browser workflows or release gates. Documentation-only edits need link/path and diff checks; preserve required pre-commit/pre-push gates.
6. Inspect the final diff for accidental scope and generated-file drift. Record commands, outcomes and covered revision/edits in the handoff. Distinguish local, CI, saved-reference and live-ArcGIS evidence. Follow existing authorization rules for commits, publication and deployment.
7. If stuck, use the escalation packet in docs/AGENT_WORKFLOW.md. Infrastructure failures are not a reason to buy a larger model. Report exactly what remains unverified and continue independent work.
