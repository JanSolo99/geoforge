#!/usr/bin/env python3
"""Local, sequential Claude Code / Codex coordinator. Python standard library only."""
import argparse
import contextlib
import datetime as dt
import fcntl
import json
import os
from pathlib import Path
import re
import selectors
import shlex
import shutil
import signal
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[2]
AGENTS = ("claude", "codex")
LIMIT = re.compile(
    r"rate[_ -]?limit|usage[_ -]?limit|usage limit|quota[_ -]?exceeded|"
    r"you(?:'|’)ve (?:hit|reached) your limit|too many requests|"
    r"insufficient_quota|limit reached|hit your (?:usage )?limit", re.I
)
HELP = """Type a task to send it to the selected agent.
/switch claude|codex   Select the other agent; context is carried automatically
/session AGENT UUID    Link an existing stopped session (do not run it elsewhere)
/new [AGENT]           Start a fresh conversation on the next turn
/auto on|off           Switch once on a reported usage limit (default: on)
/status                Show sessions, last run, and workspace
/handoff               Show the shared handoff
/cli [AGENT]           Open the native CLI for interactive permissions/tools
/quit                  Exit; sessions and progress remain on disk
Ctrl-C stops the current turn and its process group; it does not switch agents.
"""


def now():
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def save_json(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def git(root, *args):
    return subprocess.check_output(
        ["git", *args], cwd=root, text=True, stderr=subprocess.DEVNULL
    ).strip()


class Coordinator:
    def __init__(self, root=ROOT):
        self.root = root.resolve()
        # Git common-dir also gives separate worktrees the same advisory lock.
        common = Path(git(self.root, "rev-parse", "--git-common-dir"))
        self.common = (self.root / common).resolve()
        self.directory = self.common / "agent-coordinator"
        self.directory.mkdir(mode=0o700, exist_ok=True)
        os.chmod(self.directory, 0o700)
        self.state_path = self.directory / "state.json"
        self.state = {}
        self.auto = True

    def load(self):
        self.state = json.loads(self.state_path.read_text()) if self.state_path.exists() else {}
        owner = self.state.get("workspace")
        if owner and Path(owner) != self.root:
            raise RuntimeError(f"Coordinator belongs to {owner}; launch it from that checkout.")
        self.state.setdefault("workspace", str(self.root))
        self.state.setdefault("agent", "claude")
        self.state.setdefault("sessions", {})

    def save(self):
        save_json(self.state_path, self.state)

    @contextlib.contextmanager
    def lock(self):
        with (self.directory / "lock").open("a+") as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise RuntimeError("Another coordinator owns this repository. Wait for it to exit.")
            self.load()
            yield handle.fileno()

    def status(self):
        self.load()
        print(f"Workspace: {self.root}\nSelected: {self.state['agent']}")
        for agent in AGENTS:
            print(f"{agent}: {self.state['sessions'].get(agent, '(new session)')}")
        print(f"Last run: {json.dumps(self.state.get('last_run', {}))}")
        print(f"Local logs: {self.directory}")

    def prompt(self, task, previous=None):
        context = ""
        if previous:
            context = (
                f"\nThe previous attempt ended: {previous['status']}. "
                f"Its request and complete event logs are in {previous['log']}. "
                "Read request.json and relevant events if the handoff is stale. "
                "Some actions may already have completed; inspect the working tree "
                "and avoid repeating external actions.\n"
            )
        return (
            "You are working through the shared Claude/Codex coordinator. "
            "Read AGENTS.md, CLAUDE.md and docs/HANDOFF.md now, even in a resumed "
            "conversation. Check branch, status and relevant diffs. The latest user "
            "request below takes precedence over older handoff or chat context. "
            "Checkpoint docs/HANDOFF.md during work and before finishing; record "
            "decisions, incomplete edits, validation and concrete next steps. "
            "Do not launch another agent or coordinator. If a tool needs approval "
            "unavailable in this mode, explain the blocked action and suggest /cli. "
            "Do not claim work completed without evidence.\n"
            + context + "\nLatest user request:\n" + task
        )

    def command(self, agent, prompt, native=False):
        executable = shutil.which(agent)
        if not executable:
            raise RuntimeError(f"{agent} is not installed or is not on PATH.")
        session = self.state["sessions"].get(agent)
        if agent == "claude":
            command = [executable]
            if session:
                command += ["--resume", session]
            else:
                session = str(uuid.uuid4())
                command += ["--session-id", session]
            if not native:
                command += ["-p", "--verbose", "--output-format", "stream-json",
                            "--permission-mode", "acceptEdits"]
            command += ["--", prompt]
            return command, session
        command = [executable, "-C", str(self.root)]
        if native:
            if session:
                command += ["resume", session]
        else:
            command += ["-a", "never", "-s", "workspace-write", "exec"]
            if session:
                command += ["resume", session]
            command += ["--json"]
        command += ["--", prompt]
        return command, session

    def event(self, agent, event, result):
        if not isinstance(event, dict):
            return
        kind = event.get("type")
        # Ignore subagent events: their session IDs and failures are not this run's.
        if event.get("parent_tool_use_id"):
            return
        if agent == "codex":
            if kind == "thread.started" and event.get("thread_id"):
                result["session"] = event["thread_id"]
            if kind == "item.completed":
                item = event.get("item", {})
                if item.get("type") == "agent_message":
                    print(item.get("text", ""), flush=True)
                elif item.get("type") == "command_execution":
                    print(f"[command exit {item.get('exit_code')}] {item.get('command', '')}", flush=True)
            if kind == "turn.completed":
                result["completed"] = True
            if kind in ("error", "turn.failed"):
                error = event.get("error", event.get("message", ""))
                result["errors"].append(json.dumps(error))
                if kind == "turn.failed":
                    result["failed"] = True
        else:
            if kind == "system" and event.get("subtype") == "init":
                result["session"] = event.get("session_id", result["session"])
            if kind == "assistant":
                for block in event.get("message", {}).get("content", []):
                    if block.get("type") == "text":
                        print(block.get("text", ""), flush=True)
                    elif block.get("type") == "tool_use":
                        print(f"[tool] {block.get('name', '')}", flush=True)
                if event.get("error"):
                    result["errors"].append(str(event["error"]))
                    result["failed"] = True
            if kind == "result":
                result["completed"] = not event.get("is_error", False)
                result["failed"] = result["failed"] or bool(event.get("is_error", False))
                if result["failed"]:
                    result["errors"].append(str(event.get("result", "")))
                    result["errors"].extend(str(e) for e in event.get("errors", []))
        if result["session"]:
            self.state["sessions"][agent] = result["session"]
            self.save()

    def run(self, agent, task, lock_fd, native=False, previous=None):
        prompt = self.prompt(task, previous)
        command, session = self.command(agent, prompt, native)
        run_id = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S") + "-" + uuid.uuid4().hex[:8]
        log = self.directory / run_id
        log.mkdir(mode=0o700)
        save_json(log / "request.json", {
            "started": now(), "agent": agent, "workspace": str(self.root),
            "task": task, "prompt": prompt, "command": command,
            "head": git(self.root, "rev-parse", "HEAD"),
            "branch": git(self.root, "branch", "--show-current"),
            "status": git(self.root, "status", "--short"),
        })
        if (self.root / "docs/HANDOFF.md").exists():
            shutil.copyfile(self.root / "docs/HANDOFF.md", log / "handoff-before.md")
        result = {"agent": agent, "session": self.state["sessions"].get(agent),
                  "status": "running", "log": str(log),
                  "completed": False, "failed": False, "errors": []}
        self.state["last_run"] = result.copy()
        self.save()
        print(f"\n[{agent}] {log}\n", flush=True)
        # Separate process group lets Ctrl-C stop tools as well as the CLI.
        try:
            child = subprocess.Popen(
                command, cwd=self.root, start_new_session=not native,
                stdin=None if native else subprocess.DEVNULL,
                stdout=None if native else subprocess.PIPE,
                stderr=None if native else subprocess.PIPE,
                pass_fds=(lock_fd,),
            )
        except OSError as error:
            result.update(status="failed", exit_code=None, finished=now(), errors=[str(error)])
            self.state["last_run"] = result
            self.save()
            save_json(log / "result.json", result)
            raise
        interrupted = False
        try:
            if native:
                child.wait()
            else:
                with selectors.DefaultSelector() as selector, \
                        (log / "events.jsonl").open("wb") as events, \
                        (log / "stderr.log").open("wb") as errors:
                    for pipe, output in ((child.stdout, events), (child.stderr, errors)):
                        os.set_blocking(pipe.fileno(), False)
                        selector.register(pipe, selectors.EVENT_READ, [output, b""])
                    while selector.get_map():
                        for key, _ in selector.select(timeout=1):
                            data = os.read(key.fileobj.fileno(), 65536)
                            output, pending = key.data
                            if data:
                                output.write(data)
                                output.flush()
                            pending += data
                            lines = pending.split(b"\n")
                            key.data[1] = lines.pop() if data else b""
                            for line in lines:
                                text = line.decode("utf-8", errors="replace")
                                if key.fileobj is child.stdout:
                                    try:
                                        self.event(agent, json.loads(text), result)
                                    except json.JSONDecodeError:
                                        if text:
                                            print(text, flush=True)
                                elif text:
                                    print(text, file=sys.stderr, flush=True)
                            if not data:
                                selector.unregister(key.fileobj)
                                key.fileobj.close()
                child.wait()
        except KeyboardInterrupt:
            interrupted = True
            print("\nStopping agent and its tools…", flush=True)
        finally:
            if child.poll() is None:
                if native:
                    child.terminate()
                else:
                    os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    if native:
                        child.kill()
                    else:
                        os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
            # Kill descendants left behind by an interrupted CLI before releasing the lock.
            if interrupted and not native:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            for pipe in (child.stdout, child.stderr):
                if pipe is not None:
                    pipe.close()
            if native and child.returncode == 0 and session:
                result["session"] = session
            if interrupted:
                result["status"] = "interrupted"
            elif child.returncode == 0 and (native or result["completed"]) and not result["failed"]:
                result["status"] = "completed"
            elif any(LIMIT.search(error) for error in result["errors"]):
                result["status"] = "usage_limit"
            else:
                result["status"] = "failed"
            result.update(exit_code=child.returncode, finished=now())
            if result["session"]:
                self.state["sessions"][agent] = result["session"]
            self.state["last_run"] = result
            self.save()
            save_json(log / "result.json", result)
            if (self.root / "docs/HANDOFF.md").exists():
                shutil.copyfile(self.root / "docs/HANDOFF.md", log / "handoff-after.md")
            print(f"\n[{agent}: {result['status']}]", flush=True)
        return result

    def turn(self, task, lock_fd, fallback=True):
        agent = self.state["agent"]
        previous = self.state.get("last_run")
        result = self.run(agent, task, lock_fd, previous=previous)
        if fallback and result["status"] == "usage_limit":
            other = "codex" if agent == "claude" else "claude"
            print(f"{agent} reported a limit; handing the unfinished request to {other}.")
            self.state["agent"] = other
            self.save()
            result = self.run(other, task, lock_fd, previous=result)
        return 0 if result["status"] == "completed" else 1

    def repl(self, lock_fd):
        print(f"UniGIS agent coordinator — {self.root}\n{HELP}")
        while True:
            try:
                line = input(f"{self.state['agent']}> ").strip()
            except (EOFError, KeyboardInterrupt):
                print()
                return 0
            if not line:
                continue
            try:
                if not line.startswith("/"):
                    self.turn(line, lock_fd, self.auto)
                    continue
                parts = shlex.split(line)
                command, args = parts[0], parts[1:]
                if command == "/quit":
                    return 0
                if command == "/help":
                    print(HELP)
                elif command == "/status":
                    self.status()
                elif command == "/handoff":
                    print((self.root / "docs/HANDOFF.md").read_text())
                elif command == "/switch" and len(args) == 1 and args[0] in AGENTS:
                    self.state["agent"] = args[0]
                    self.save()
                elif command == "/session" and len(args) == 2 and args[0] in AGENTS:
                    self.state["sessions"][args[0]] = str(uuid.UUID(args[1]))
                    self.save()
                elif command == "/new" and len(args) <= 1 and (not args or args[0] in AGENTS):
                    self.state["sessions"].pop(args[0] if args else self.state["agent"], None)
                    self.save()
                elif command == "/auto" and args in (["on"], ["off"]):
                    self.auto = args == ["on"]
                elif command == "/cli" and len(args) <= 1 and (not args or args[0] in AGENTS):
                    agent = args[0] if args else self.state["agent"]
                    self.run(agent, "Continue the task in docs/HANDOFF.md.", lock_fd,
                             native=True, previous=self.state.get("last_run"))
                    if agent == "codex" and not self.state["sessions"].get(agent):
                        print("To resume this native Codex session later, use /session codex UUID "
                              "with the ID shown by Codex when it exits.")
                else:
                    print("Unknown command or arguments. /help lists commands.")
            except (RuntimeError, ValueError, OSError) as error:
                print(f"Error: {error}", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", choices=AGENTS, help="Initial agent (otherwise reuse selection)")
    parser.add_argument("--status", action="store_true", help="Show local state without starting an agent")
    parser.add_argument("--no-fallback", action="store_true", help="Disable automatic usage-limit fallback")
    parser.add_argument("--task", help="Run one task and exit; otherwise open the interactive coordinator")
    args = parser.parse_args()
    coordinator = Coordinator()
    if args.status:
        coordinator.status()
        return 0
    with coordinator.lock() as lock_fd:
        if args.agent:
            coordinator.state["agent"] = args.agent
        coordinator.auto = not args.no_fallback
        coordinator.save()
        if args.task:
            return coordinator.turn(args.task, lock_fd, coordinator.auto)
        if not sys.stdin.isatty():
            parser.error("Use --task for non-interactive input, or run in a terminal.")
        return coordinator.repl(lock_fd)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Coordinator: {error}", file=sys.stderr)
        sys.exit(1)
