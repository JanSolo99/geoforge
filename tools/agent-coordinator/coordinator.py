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
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
AGENTS = ("claude", "codex", "kimi")
# Kimi Code CLI: its installer puts it outside PATH; the model is a config.toml alias.
KIMI_MODEL = os.environ.get("UNIGIS_AGENTS_KIMI_MODEL", "kimi-code/k3.1-pro")
KIMI_FALLBACK = Path.home() / ".kimi-code" / "bin" / "kimi"
LIMIT = re.compile(
    r"rate[_ -]?limit|usage[_ -]?limit|usage limit|quota[_ -]?exceeded|"
    r"you(?:'|’)ve (?:hit|reached) your limit|too many requests|"
    r"insufficient_quota|limit reached|hit your (?:usage )?limit", re.I
)
# Provider outages: hand over like a usage limit, but retry the provider sooner.
TRANSIENT = re.compile(
    r"at capacity|overloaded|temporarily unavailable|service unavailable|"
    r"try a different model|stream disconnected|internal server error", re.I
)
# "try again at 1:49 AM" (Codex), "resets 5:40am (Australia/Sydney)" (Claude).
RESET = re.compile(r"(?:try again at|resets(?: at)?)\s+(\d{1,2})(?::(\d{2}))?\s*([ap]\.?m\.?)"
                   r"(?:\s*\(([\w/+-]+)\))?", re.I)
LIMIT_WAIT = dt.timedelta(minutes=30)      # usage limit without a stated reset time
TRANSIENT_WAIT = dt.timedelta(minutes=10)  # capacity / overload
# A resumed conversation re-sends its whole history on every model call; past this size the
# coordinator starts a fresh one and lets docs/HANDOFF.md carry the context.
MAX_SESSION_MB = float(os.environ.get("UNIGIS_AGENTS_MAX_SESSION_MB", "25"))
# Parallel lanes (scripts/lane): one agent per git worktree, landing on main one at a time.
LANE_TASK = ("Continue: finish the tool your lane has claimed (your lane handoff), or claim the next "
             "one with `scripts/lane claim`; build it following docs/WORK_QUEUE.md's rules, then "
             "`scripts/lane land`.")
SHIM = Path(__file__).resolve().parent / "bin"
DEFAULT_TASK = ("Continue: finish or wrap up the tool in progress (docs/HANDOFF.md), then build the "
                "next tool in docs/WORK_QUEUE.md, following its rules.")
HELP = """Type a task to send it to the selected agent.
/switch claude|codex|kimi  Prefer that agent (clears its recorded limit); context carries over
/session AGENT UUID    Link an existing stopped session (do not run it elsewhere)
/new [AGENT]           Start a fresh conversation on the next turn
/auto on|off           Use the other agent while one is limited (default: on)
/status                Show sessions, last run, and workspace
/handoff               Show the shared handoff
/cli [AGENT]           Open the native CLI with full access and interactive tools
/quit                  Exit; sessions and progress remain on disk
Ctrl-C stops the current turn and its process group; it does not switch agents.
"""


def now():
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def save_json(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def reset_time(errors, current=None):
    """When a reported limit lifts, as an aware datetime, or None if no time is given."""
    for error in errors:
        match = RESET.search(error)
        if not match:
            continue
        hour, minute = int(match[1]) % 12, int(match[2] or 0)
        hour += 12 if match[3].lower().startswith("p") else 0
        zone = None
        if match[4]:
            with contextlib.suppress(Exception):
                from zoneinfo import ZoneInfo
                zone = ZoneInfo(match[4])
        local = (current or dt.datetime.now(dt.timezone.utc)).astimezone(zone)
        moment = local.replace(hour=hour, minute=minute, second=0, microsecond=0)
        if moment <= local:
            moment += dt.timedelta(days=1)
        return moment
    return None


def git(root, *args):
    return subprocess.check_output(
        ["git", *args], cwd=root, text=True, stderr=subprocess.DEVNULL
    ).strip()


class Coordinator:
    def __init__(self, root=ROOT, lane=None):
        self.lane = lane
        if lane:
            root = lane_root(root, lane)
        self.root = root.resolve()
        # Git common-dir also gives separate worktrees the same advisory lock; a lane has its own.
        common = Path(git(self.root, "rev-parse", "--git-common-dir"))
        self.common = (self.root / common).resolve()
        self.directory = self.common / "agent-coordinator"
        if lane:
            self.directory = self.directory / "lanes" / lane
        self.directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        os.chmod(self.directory, 0o700)
        self.state_path = self.directory / "state.json"
        self.state = {}
        self.auto = True
        self.headless = False

    def load(self):
        self.state = json.loads(self.state_path.read_text()) if self.state_path.exists() else {}
        owner = self.state.get("workspace")
        if owner and Path(owner) != self.root:
            raise RuntimeError(f"Coordinator belongs to {owner}; launch it from that checkout.")
        self.state.setdefault("workspace", str(self.root))
        self.state.setdefault("agent", "claude")
        self.state.setdefault("sessions", {})
        self.state.setdefault("limited_until", {})

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
            limited = self.state["limited_until"].get(agent)
            note = f"  (limited until {limited})" if limited and not self.available(agent) else ""
            print(f"{agent}: {self.state['sessions'].get(agent, '(new session)')}{note}")
        print(f"Last run: {json.dumps(self.state.get('last_run', {}))}")
        print(f"Local logs: {self.directory}")

    def available(self, agent):
        until = self.state.get("limited_until", {}).get(agent)
        return not until or dt.datetime.fromisoformat(until) <= dt.datetime.now(dt.timezone.utc)

    def next_available(self):
        times = [dt.datetime.fromisoformat(t) for a, t in self.state["limited_until"].items()
                 if not self.available(a)]
        return min(times) if times else None

    def mark_limited(self, agent, result):
        current = dt.datetime.now(dt.timezone.utc)
        until = reset_time(result["errors"], current)
        if until is None:
            until = current + (TRANSIENT_WAIT if result["status"] == "unavailable" else LIMIT_WAIT)
        self.state["limited_until"][agent] = until.astimezone(dt.timezone.utc).isoformat(timespec="seconds")
        self.save()
        print(f"[{agent} unavailable until {until.astimezone():%a %H:%M}]", flush=True)

    def transcript(self, agent, session):
        """The CLI's stored conversation file for a session, if it can be found."""
        if agent == "claude":
            base = Path(os.environ.get("CLAUDE_CONFIG_DIR", Path.home() / ".claude")) / "projects"
            pattern = f"*/{session}.jsonl"
        elif agent == "kimi":
            base = Path.home() / ".kimi-code" / "sessions"
            pattern = f"**/*{session}*"
        else:
            base = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")) / "sessions"
            pattern = f"**/rollout-*{session}.jsonl"
        return next(base.glob(pattern), None) if base.is_dir() else None

    def rotate_if_large(self, agent):
        session = self.state["sessions"].get(agent)
        path = session and self.transcript(agent, session)
        if not path:
            return
        files = path.rglob("*") if path.is_dir() else [path]
        size = sum(f.stat().st_size for f in files if f.is_file()) / 2**20
        if size > MAX_SESSION_MB:
            print(f"[{agent} session {session[:8]} is {size:.0f} MB; starting a fresh one, "
                  "context comes from docs/HANDOFF.md]", flush=True)
            self.state["sessions"].pop(agent)
            self.save()

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
        if self.lane:
            context += (
                f"\nYou are lane `{self.lane}`, working in the worktree {self.root} on branch "
                f"lane/{self.lane} while other lanes (other agents in other worktrees) work in "
                "parallel. Follow AGENTS.md's 'Parallel lanes' section: claim a tool with "
                "`scripts/lane claim`, use your lane handoff instead of docs/HANDOFF.md, commit on "
                "your branch and land with `scripts/lane land` (never push main directly).\n"
            )
        return (
            "You are working through the shared Claude/Codex/Kimi coordinator. "
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
        if not executable and agent == "kimi" and os.access(KIMI_FALLBACK, os.X_OK):
            executable = str(KIMI_FALLBACK)
        if not executable:
            raise RuntimeError(f"{agent} is not installed or is not on PATH.")
        session = self.state["sessions"].get(agent)
        if agent == "claude":
            command = [executable, "--dangerously-skip-permissions",
                       "--settings", '{"sandbox":{"enabled":false}}']
            if session:
                command += ["--resume", session]
            else:
                session = str(uuid.uuid4())
                command += ["--session-id", session]
            if not native:
                command += ["-p", "--verbose", "--output-format", "stream-json"]
            command += ["--", prompt]
            return command, session
        if agent == "kimi":
            # Prompt mode already runs in Kimi's never-ask mode and rejects --auto;
            # --auto only covers the interactive /cli launch.
            command = [executable, "-m", KIMI_MODEL]
            if native:
                command += ["--auto"]
            if session:
                command += ["-S", session]
            if not native:
                command += ["--output-format", "stream-json", "-p", prompt]
            return command, session
        command = [executable, "-C", str(self.root),
                   "--dangerously-bypass-approvals-and-sandbox"]
        if native:
            if session:
                command += ["resume", session]
        else:
            command += ["exec"]
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
        if agent == "kimi":
            role = event.get("role")
            if role == "meta" and kind == "session.resume_hint":
                # Written only after a turn completes.
                result["session"] = event.get("session_id") or result["session"]
                result["completed"] = True
            elif role == "meta" and kind == "turn.step.retrying":
                print(f"[retrying: {event.get('error_message', '')}]", flush=True)
            elif role == "assistant":
                if event.get("content"):
                    print(event["content"], flush=True)
                for call in event.get("tool_calls") or []:
                    print(f"[tool] {call.get('function', {}).get('name', '')}", flush=True)
            elif kind == "error":
                result["errors"].append(f"{event.get('code', '')}: {event.get('message', '')}")
                result["failed"] = True
        elif agent == "codex":
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

    def child_env(self, agent):
        env = dict(os.environ)
        if agent == "kimi":
            # Lets Kimi load the project's MCP servers (the ArcGIS bridge) without a prompt.
            env["KIMI_CODE_TRUST_WORKSPACE"] = "1"
        if self.lane:
            # The cargo shim shares a few build slots between lanes so they fit in memory.
            env.update(UNIGIS_LANE=self.lane, CARGO_BUILD_JOBS="3",
                       PATH=f"{SHIM}{os.pathsep}{env.get('PATH', '')}")
        return env

    def run(self, agent, task, lock_fd, native=False, previous=None):
        if not native:
            self.rotate_if_large(agent)
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
                # Lets Kimi load the project's MCP servers (the ArcGIS bridge) without a prompt.
                env=self.child_env(agent),
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
                                    # Kimi reports a failed turn ("provider.rate_limit: …") on stderr.
                                    if agent == "kimi" and not text.startswith("Warning"):
                                        result["errors"].append(text)
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
            elif any(TRANSIENT.search(error) for error in result["errors"]):
                result["status"] = "unavailable"
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
        """Run the task on the selected agent, or on the other one while it is limited.

        The selection is a preference: once a limit lifts, turns go back to it.
        """
        preferred = self.state["agent"]
        order = [preferred] + ([a for a in AGENTS if a != preferred] if fallback else [])
        candidates = [agent for agent in order if self.available(agent)]
        if not candidates:
            print(f"[no agent available until {self.next_available().astimezone():%a %H:%M}; "
                  "/switch AGENT retries one now]", flush=True)
            return 1
        if candidates[0] != preferred:
            print(f"[{preferred} is limited; using {candidates[0]}]", flush=True)
        previous = self.state.get("last_run")
        for index, agent in enumerate(candidates):
            result = self.run(agent, task, lock_fd, previous=previous)
            if result["status"] not in ("usage_limit", "unavailable"):
                break
            self.mark_limited(agent, result)
            previous = result
            if index + 1 < len(candidates):
                print(f"{agent} is {result['status'].replace('_', ' ')}; handing the unfinished "
                      f"request to {candidates[index + 1]}.", flush=True)
        return 0 if result["status"] == "completed" else 1

    def handle(self, line, lock_fd):
        """Run one prompt line (a task or a /command). True means quit."""
        if not line.startswith("/"):
            self.turn(line, lock_fd, self.auto)
            return False
        parts = shlex.split(line)
        command, args = parts[0], parts[1:]
        if command == "/quit":
            return True
        if command == "/help":
            print(HELP)
        elif command == "/status":
            self.status()
        elif command == "/handoff":
            print((self.root / "docs/HANDOFF.md").read_text())
        elif command == "/switch" and len(args) == 1 and args[0] in AGENTS:
            self.state["agent"] = args[0]
            self.state["limited_until"].pop(args[0], None)
            self.save()
        elif command == "/session" and len(args) == 2 and args[0] in AGENTS:
            if args[0] != "kimi":
                args[1] = str(uuid.UUID(args[1]))
            elif not re.fullmatch(r"[\w-]{6,}", args[1]):
                raise ValueError("not a Kimi session ID")
            self.state["sessions"][args[0]] = args[1]
            self.save()
        elif command == "/new" and len(args) <= 1 and (not args or args[0] in AGENTS):
            self.state["sessions"].pop(args[0] if args else self.state["agent"], None)
            self.save()
        elif command == "/auto" and args in (["on"], ["off"]):
            self.auto = args == ["on"]
        elif command == "/cli" and self.headless:
            print("/cli needs a terminal; use `agents-remote chat` and /cli there.")
        elif command == "/cli" and len(args) <= 1 and (not args or args[0] in AGENTS):
            agent = args[0] if args else self.state["agent"]
            self.run(agent, "Continue the task in docs/HANDOFF.md.", lock_fd,
                     native=True, previous=self.state.get("last_run"))
            if agent == "codex" and not self.state["sessions"].get(agent):
                print("To resume this native Codex session later, use /session codex UUID "
                      "with the ID shown by Codex when it exits.")
        else:
            print("Unknown command or arguments. /help lists commands.")
        return False

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
                if self.handle(line, lock_fd):
                    return 0
            except (RuntimeError, ValueError, OSError) as error:
                print(f"Error: {error}", file=sys.stderr)

    # --- Headless mode for remote management (see tools/agent-coordinator/REMOTE.md) ---

    def remote_dirs(self):
        remote = self.directory / "remote"
        for name in ("inbox", "done"):
            (remote / name).mkdir(parents=True, exist_ok=True, mode=0o700)
        os.chmod(remote, 0o700)
        return remote

    def write_remote_status(self, remote, state, task="", failures=0, **extra):
        save_json(remote / "status.json", {
            **extra, "time": now(), "state": state, "agent": self.state.get("agent"),
            "limited_until": {a: t for a, t in self.state.get("limited_until", {}).items()
                                              if not self.available(a)},
            "task": task[:300], "failures": failures,
            "paused": (remote / "PAUSE").exists(),
            "last_run": self.state.get("last_run", {}),
            "pid": os.getpid(),
        })

    def daemon(self, lock_fd, keepalive=True, poll=5.0, idle_task=DEFAULT_TASK,
               sleep=time.sleep, max_cycles=None):
        """Run without a terminal: tasks and commands arrive as files in the remote inbox.

        Tasks (*.task) run in file-name order; commands (*.cmd) hold one /command line. With
        `keepalive` and an empty inbox the agent is sent `idle_task` again and again, backing off
        after failures. Files named PAUSE and STOP in the remote directory pause and end the loop.
        """
        remote = self.remote_dirs()
        self.headless = True
        failures = 0
        cycles = 0
        with (remote / "daemon.log").open("a", buffering=1) as log, \
                contextlib.redirect_stdout(_Tee(sys.stdout, log)):
            print(f"[{now()}] daemon started in {self.root}")
            while max_cycles is None or cycles < max_cycles:
                cycles += 1
                if (remote / "STOP").exists():
                    (remote / "STOP").unlink()
                    self.write_remote_status(remote, "stopped")
                    print(f"[{now()}] stop requested")
                    return 0
                paused = (remote / "PAUSE").exists()
                items = sorted(p for p in (remote / "inbox").iterdir()
                               if p.suffix in (".task", ".cmd"))
                item = next((p for p in items if p.suffix == ".cmd" or not paused), None)
                try:
                    if item is not None:
                        text = item.read_text().strip()
                        self.write_remote_status(remote, "running", text, failures, started=now())
                        print(f"[{now()}] {item.name}: {text[:200]}")
                        code = 0
                        if item.suffix == ".cmd":
                            self.handle(text, lock_fd)
                        else:
                            code = self.turn(text, lock_fd, self.auto)
                        item.rename(remote / "done" / f"{item.name}.exit{code}")
                        failures = failures + 1 if code else 0
                    elif keepalive and not paused:
                        self.write_remote_status(remote, "running", idle_task, failures, started=now())
                        print(f"[{now()}] keepalive: {idle_task}")
                        code = self.turn(idle_task, lock_fd, self.auto)
                        failures = failures + 1 if code else 0
                        if code:
                            wait = min(60 * 2 ** (failures - 1), 1800)
                            reset = self.next_available()
                            usable = AGENTS if self.auto else (self.state["agent"],)
                            if reset and not any(self.available(a) for a in usable):
                                # Both limited: sleep until the first reset, not a blind backoff.
                                seconds = (reset - dt.datetime.now(dt.timezone.utc)).total_seconds()
                                wait = max(60, min(seconds + 60, 6 * 3600))
                            print(f"[{now()}] turn did not complete; waiting {wait:.0f}s")
                            until = (dt.datetime.now(dt.timezone.utc)
                                     + dt.timedelta(seconds=wait)).isoformat(timespec="seconds")
                            self.write_remote_status(remote, "backoff", idle_task, failures, until=until)
                            waited = 0.0
                            while waited < wait and not (remote / "STOP").exists():
                                sleep(poll)
                                waited += poll
                            continue
                    else:
                        self.write_remote_status(remote, "paused" if paused else "idle", "", failures)
                        sleep(poll)
                except (RuntimeError, ValueError, OSError) as error:
                    print(f"[{now()}] error: {error}")
                    if item is not None and item.exists():
                        item.rename(remote / "done" / f"{item.name}.error")
                    sleep(poll)
        return 0


class _Tee:
    def __init__(self, *streams):
        self.streams = streams

    def write(self, text):
        for stream in self.streams:
            stream.write(text)
        return len(text)

    def flush(self):
        for stream in self.streams:
            stream.flush()


def lane_root(root, lane):
    """A lane's worktree (created by `scripts/lane setup`)."""
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,30}", lane):
        raise ValueError(f"bad lane name {lane!r}")
    path = Path(root).resolve() / ".worktrees" / f"lane-{lane}"
    if not path.is_dir():
        raise RuntimeError(f"lane {lane} has no worktree; run scripts/lane setup {lane} AGENT")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", choices=AGENTS, help="Initial agent (otherwise reuse selection)")
    parser.add_argument("--status", action="store_true", help="Show local state without starting an agent")
    parser.add_argument("--no-fallback", action="store_true", help="Disable automatic usage-limit fallback")
    parser.add_argument("--task", help="Run one task and exit; otherwise open the interactive coordinator")
    parser.add_argument("--daemon", action="store_true",
                        help="Run headless: take tasks from the remote inbox (see REMOTE.md)")
    parser.add_argument("--cli", nargs="?", const="", choices=("", *AGENTS), metavar="AGENT",
                        help="Open the native CLI on the coordinator's session, then exit")
    parser.add_argument("--lane", help="Run as a parallel lane in .worktrees/lane-NAME (scripts/lane)")
    parser.add_argument("--no-keepalive", action="store_true",
                        help="With --daemon, wait for inbox tasks instead of re-sending the handoff task")
    args = parser.parse_args()
    coordinator = Coordinator(lane=args.lane)
    if args.status:
        coordinator.status()
        return 0
    with coordinator.lock() as lock_fd:
        if args.agent:
            coordinator.state["agent"] = args.agent
        # A lane is one agent; other agents work in their own lanes.
        coordinator.auto = not args.no_fallback and not args.lane
        coordinator.save()
        if args.cli is not None:
            coordinator.handle(f"/cli {args.cli}".strip(), lock_fd)
            return 0
        if args.daemon:
            idle = LANE_TASK if args.lane else DEFAULT_TASK
            return coordinator.daemon(lock_fd, keepalive=not args.no_keepalive, idle_task=idle)
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
