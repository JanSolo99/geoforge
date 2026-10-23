# Shared instructions for Claude Code and Codex

## Project guidance

Read `CLAUDE.md` for ArcGIS Pro compatibility goals, format rules, and tool workflow.
Read `docs/ROADMAP.md` and `docs/KNOWN_GAPS.md` before choosing follow-up work.
For changes under `app/`, also follow `app/AGENTS.md`.

- ArcGIS Pro is the geoprocessing behavior reference. GDAL is a file format test
  oracle only, never a runtime dependency or geoprocessing behavior reference.
- Keep FileGDB clean-room: public documentation and black-box ArcGIS probes only;
  never use or copy Esri's proprietary SDK.
- Preserve existing edits. Do not commit, reset, stash, or transfer changes between
  worktrees unless the user requests it.
- Avoid overlapping builds. Limit Cargo build, clippy, and test jobs to 3 (`-j3`).
- Update the roadmap as work lands and known gaps when behavior differences are
  found or resolved. Preserve project validation requirements in `CLAUDE.md`.

## Shared workspace

For sequential Claude/Codex work, use `/mnt/media/ArcGIS_Linux` for both CLIs.
The user can launch `./scripts/agents` for the shared coordinator prompt; see
`tools/agent-coordinator/README.md`. Coordinator turns supply the latest request
and prior run logs as well as the handoff. Read those when recovering partial work.
Only one agent should edit or run builds at a time. Stop the previous agent's work
before handing over. Separate Git worktrees do not share uncommitted edits.
If launched elsewhere, report the actual directory and branch; do not assume the
main checkout's handoff describes the current worktree or move changes silently.

## Starting or resuming

1. Read these instructions and `docs/HANDOFF.md`, even in a resumed conversation.
2. Check the working directory, branch, HEAD, Git status, and relevant diffs.
3. Reconcile the handoff with the actual files; Git and files are evidence, while
   the handoff explains intent. Treat old chat context as potentially stale.
4. Continue the recorded objective and next action without requiring the user to
   repeat context. A new user instruction takes precedence. Ask a focused question
   only if the objective is missing, conflicting, or cannot be inferred safely.

## Checkpoints and handoffs

Maintain `docs/HANDOFF.md` automatically after meaningful milestones, findings,
validation runs, or changes of direction, and before stopping or switching agents.
Update during work: usage limits may prevent a final handoff. Keep it concise and
replace stale state rather than accumulating transcripts.

Record:
- Timestamp and last agent; working directory, branch, and HEAD.
- User objective and completion criteria.
- Completed work and unfinished edits with useful file paths. Distinguish edits
  inherited from another session from work performed in the current session.
- Key decisions, evidence, failed approaches, and relevant research artifacts.
- Exact validation commands and outcomes, the revision or edits they cover, and
  checks still pending. Never imply that unrun checks passed.
- Concrete next actions in order, blockers, and any running processes that matter.

The roadmap tracks overall progress; known gaps tracks compatibility differences.
The handoff tracks the current task. Keep durable technical findings in the
appropriate project documentation and link them from the handoff.

Do not launch another agent, resume a second CLI session, or install a coordinator
merely to update the handoff. Those actions need an explicit user task.
