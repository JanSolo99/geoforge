# Shared Claude / Codex coordinator

Run from a normal terminal:

```sh
cd /mnt/media/ArcGIS_Linux
./scripts/agents
```

Type your task at `claude>`. The coordinator streams the agent's response and tool
activity, remembers its session, and returns to the same prompt for follow-up work.
Claude is selected on first launch. Subsequent launches remember your selection.
The coordinator is a local Python program, with no model of its own and no extra
dependencies. It invokes the installed CLIs using their existing authentication
and configuration; it does not set API keys or change authentication providers.

## Switching and existing sessions

```text
/switch codex
Continue the current task.
/switch claude
Review the changes and continue.
```

The selected agent reads the shared instructions, handoff, working tree, and the
previous run's request and event logs. Its own conversation is resumed by ID.
This provides context across providers without importing one provider's private
conversation format into the other.

To bring an existing conversation into the coordinator, stop it in its original
terminal first, then enter:

```text
/session claude YOUR-SESSION-UUID
Continue the current task and checkpoint progress.
```

Use `/session codex UUID` for Codex. Link sessions belonging to this checkout.
The ID is a resume target, not a live connection to the old terminal. The
coordinator does not send messages to or interrupt an already running CLI.

## Usage limits

Automatic fallback is on by default. If a turn ends with a recognized usage-limit
error in the CLI's structured events, the coordinator sends the unfinished request
and log locations to the other agent. It waits for the first process to exit and
switches at most once per user request. The other agent remains selected afterward.
If both are limited, it stops; use `/switch` when you want to retry a provider.

Authentication, permissions, network errors, and ordinary tool failures do not
trigger fallback. Text about rate limits in successful answers or tool output is
not treated as a provider limit. Unknown CLI error formats require `/switch`.
Use `/auto off` to disable automatic switching, or `--no-fallback` on launch.

Agents checkpoint `docs/HANDOFF.md` during work. The coordinator also snapshots it
before and after each turn and records the original request and Git status, so an
abrupt limit still leaves context. A completed turn means the CLI finished its
response; assess the agent's report to determine whether the task is complete.

## Permissions and native CLIs

The shared prompt uses noninteractive CLI modes. Claude uses `acceptEdits` so it
can edit project files; shell and other tool access follow its configured rules.
Codex uses the `workspace-write` sandbox with approvals unavailable. The wrapper
does not add blanket shell approvals or bypass sandboxing. If a needed action
cannot run in that mode, use the native interface to handle its normal prompts:

```text
/cli claude
/cli codex
```

Opening `/cli` does not itself change a resumed session's permission settings.
If native Codex still reports that approvals are unavailable, use `/permissions`
in that CLI to choose Auto (workspace write with on-request approvals). This lets
it request approval for protected operations such as Git metadata writes while
keeping the sandbox enabled.

Exit the native CLI to return to the coordinator. Claude sessions started here
are assigned IDs before launch. A first native Codex session must be linked with
`/session codex UUID` using the ID Codex prints when it exits; streamed Codex turns
capture IDs automatically. Native mode does not record output or automatically
detect limits. Neither mode commits or transfers changes between worktrees.

## State and one active editor

State, request/event logs, and handoff snapshots are private local files under
the Git common directory's `agent-coordinator/` folder (normally
`/mnt/media/ArcGIS_Linux/.git/agent-coordinator/`). They are outside the working
tree and are not staged with project changes. Logs contain prompts and tool output.

A kernel file lock prevents overlapping coordinator instances, including across
worktrees. It is held while the coordinator is open and inherited by its child
CLI. Ctrl-C during a streamed turn stops the CLI and its process group, records an
interruption, and returns to the prompt without automatically switching.

This lock cannot stop independently launched Claude or Codex sessions. Pause or
exit those before starting coding work here. Both providers edit the same checkout;
the coordinator never stashes, resets, commits, or merges it.

## Commands

Enter `/help` for the available commands. Other entry points:

```sh
./scripts/agents --status
./scripts/agents --agent codex
./scripts/agents --agent claude --task "Continue the task in docs/HANDOFF.md"
./scripts/agents --no-fallback --task "Inspect the current changes"
```

`--task` exits with 0 for a completed response and 1 for a failed or interrupted
run. Use `/new claude` or `/new codex` to forget a saved resume target without
deleting its conversation or changing project files.

The implementation uses [Claude Code's programmatic interface](https://code.claude.com/docs/en/headless)
and [Codex's noninteractive JSON events](https://learn.chatgpt.com/docs/non-interactive-mode).
It does not use a continuously running model supervisor or install an MCP service.

## Offline validation

```sh
python3 -m unittest discover -s tools/agent-coordinator -p 'test_*.py' -v
```

Tests run fake CLI processes in temporary Git repositories. They check partial work
and context across a limit switch, session resumption, avoiding spurious switches,
cross-process locking, and stopping descendants on interruption. They do not spend
provider usage.
