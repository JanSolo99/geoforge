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

## Agents

Three CLIs can take turns: Claude Code (`claude`), Codex (`codex`) and Kimi Code (`kimi`,
found in `~/.kimi-code/bin` if it is not on PATH). Kimi runs `kimi -m kimi-code/k3.1-pro`
(`UNIGIS_AGENTS_KIMI_MODEL` picks another alias from `~/.kimi-code/config.toml`) with its
stream-json prompt mode, which already runs in never-ask mode; `--auto` is added for the
interactive `/cli` launch only, since Kimi rejects `--prompt` together with `--auto`. Kimi
resumes with `-S ID` and loads MCP servers from `~/.kimi-code/mcp.json`
(the ArcGIS bridge is configured there). When the selected agent is limited, the request goes
to the next available one in the order claude, codex, kimi.

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

## Usage limits, outages and long sessions

Automatic fallback is on by default. If a turn ends with a recognized usage-limit
error ("hit your limit") or a provider outage ("model is at capacity", "overloaded")
in the CLI's structured events, the coordinator sends the unfinished request and
log locations to the other agent. It records when the limited agent is usable
again: the reset time the CLI states ("resets 5:40am", "try again at 1:49 AM"),
otherwise 30 minutes for a limit and 10 for an outage. Until then turns go straight
to the other agent; afterwards they return to the selected one. The selection is
a preference and fallback never changes it. If both are limited, the daemon sleeps
until the first reset. `/switch AGENT` clears that agent's recorded limit to retry
it now.

Authentication, permissions, network errors, and ordinary tool failures do not
trigger fallback. Text about rate limits in successful answers or tool output is
not treated as a provider limit. Unknown CLI error formats require `/switch`.
Use `/auto off` to disable automatic switching, or `--no-fallback` on launch.

A resumed conversation re-sends its whole history on every model call, so a
session that runs for days makes every turn slow and expensive. When a session's
stored transcript (`~/.claude/projects/*/ID.jsonl`, `~/.codex/sessions/**/rollout-*ID.jsonl`)
passes 25 MB (`UNIGIS_AGENTS_MAX_SESSION_MB`), the next turn starts a fresh
session; `docs/HANDOFF.md` and the previous run's logs carry the context. `/new`
does the same by hand.

Agents checkpoint `docs/HANDOFF.md` during work. The coordinator also snapshots it
before and after each turn and records the original request and Git status, so an
abrupt limit still leaves context. A completed turn means the CLI finished its
response; assess the agent's report to determine whether the task is complete.

## Permissions and native CLIs

All coordinator launches explicitly use full access, as requested for this
workspace:

- Claude: `--dangerously-skip-permissions --settings '{"sandbox":{"enabled":false}}'`.
- Codex: `--dangerously-bypass-approvals-and-sandbox`.

These flags apply to fresh and resumed sessions, both streamed turns and native
`/cli` sessions. The coordinator no longer forces Codex's `workspace-write`
sandbox or Claude's `acceptEdits` mode. CLI permission prompts and both providers'
sandboxes are disabled. Claude's separate sandbox setting is explicitly overridden
for the launch ([Claude sandbox documentation](https://code.claude.com/docs/en/sandboxing)).
Authentication and provider configuration still
come from the installed CLIs.

Native interfaces remain available for their interactive tools and UI:

```text
/cli claude
/cli codex
```

After updating the coordinator code, restart the coordinator once so its running
Python process loads the new launch policy. If inside `/cli`, exit the native
CLI, enter `/quit` at the coordinator prompt, and run `./scripts/agents` again.
Changing files does not replace code already loaded by a running coordinator.

Exit the native CLI to return to the coordinator. Claude sessions started here
are assigned IDs before launch. A first native Codex session must be linked with
`/session codex UUID` using the ID Codex prints when it exits; streamed Codex turns
capture IDs automatically. Native mode does not record output or automatically
detect limits. The coordinator itself never stages, commits or transfers changes
between worktrees.

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
./scripts/agents --cli claude   # native CLI on the coordinator's session
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

## Remote management

`./scripts/agents --daemon` runs headless and `./scripts/agents-remote` steers it over SSH or a
tailnet-only web page; see [REMOTE.md](REMOTE.md).

## Offline validation

```sh
python3 -m unittest discover -s tools/agent-coordinator -p 'test_*.py' -v
```

Tests run fake CLI processes in temporary Git repositories. They check partial work
and context across a limit switch, session resumption, avoiding spurious switches,
cross-process locking, stopping descendants on interruption, and full-access
launch flags for both providers across new/resumed and streamed/native modes.
They do not spend provider usage.
