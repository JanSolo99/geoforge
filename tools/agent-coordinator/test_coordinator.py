"""Offline integration checks; fake CLIs never contact a model provider."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("coordinator", Path(__file__).with_name("coordinator.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

CLAUDE_ID = "11111111-1111-4111-8111-111111111111"
CODEX_ID = "22222222-2222-4222-8222-222222222222"
KIMI_ID = "ses_kimi_333333"


class CoordinatorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", self.root], check=True)
        subprocess.run(["git", "-C", self.root, "-c", "user.name=Test", "-c",
                        "user.email=test@example.invalid", "commit", "-q", "--allow-empty",
                        "-m", "fixture"], check=True)
        (self.root / "docs").mkdir()
        (self.root / "docs/HANDOFF.md").write_text("Existing handoff\n")
        self.coordinator = module.Coordinator(self.root)
        self.bin = self.root / "bin"
        self.bin.mkdir()

    def executable(self, name, body):
        path = self.bin / name
        path.write_text(f"#!{sys.executable}\n" + body)
        path.chmod(0o700)
        return str(path)

    def run_turn(self, task="Continue", fallback=True):
        with self.coordinator.lock() as fd, contextlib.redirect_stdout(io.StringIO()):
            return self.coordinator.turn(task, fd, fallback)

    def test_full_access_for_new_resumed_streamed_and_native_commands(self):
        self.coordinator.load()
        flags = {
            "claude": "--dangerously-skip-permissions",
            "codex": "--dangerously-bypass-approvals-and-sandbox",
        }
        sessions = {"claude": CLAUDE_ID, "codex": CODEX_ID, "kimi": KIMI_ID}
        for agent in module.AGENTS:
            for native in (False, True):
                for resumed in (False, True):
                    with self.subTest(agent=agent, native=native, resumed=resumed):
                        self.coordinator.state["sessions"] = {agent: sessions[agent]} if resumed else {}
                        executable = str(self.bin / agent)
                        with patch.object(module.shutil, "which", return_value=executable):
                            command, session = self.coordinator.command(agent, "Continue", native)
                        self.assertEqual(command[0], executable)
                        if agent == "kimi":
                            # Kimi rejects --prompt with --auto; --auto is native-only.
                            self.assertEqual("--auto" in command, native)
                            self.assertEqual(command[-2:] == ["-p", "Continue"], not native)
                            self.assertEqual(module.KIMI_MODEL, command[command.index("-m") + 1])
                            if resumed:
                                self.assertEqual(command[command.index("-S") + 1], KIMI_ID)
                            continue
                        self.assertIn(flags[agent], command[:command.index("--")])
                        self.assertNotIn("workspace-write", command)
                        self.assertNotIn("acceptEdits", command)
                        self.assertEqual(command[-2:], ["--", "Continue"])
                        if resumed:
                            self.assertEqual(session, sessions[agent])
                            self.assertIn(sessions[agent], command)
                        if agent == "codex":
                            self.assertEqual("exec" in command, not native)
                            self.assertEqual("--json" in command, not native)
                        else:
                            self.assertEqual("-p" in command, not native)
                            settings = json.loads(command[command.index("--settings") + 1])
                            self.assertIs(settings["sandbox"]["enabled"], False)

    def test_partial_work_survives_limit_switch_and_both_sessions_resume(self):
        claude = self.executable("claude", f'''
import json, pathlib, sys
assert '--dangerously-skip-permissions' in sys.argv
assert json.loads(sys.argv[sys.argv.index('--settings') + 1])['sandbox']['enabled'] is False
pathlib.Path('partial.txt').write_text('preserved')
sid = sys.argv[sys.argv.index('--session-id') + 1] if '--session-id' in sys.argv else sys.argv[sys.argv.index('--resume') + 1]
print(json.dumps({{'type': 'system', 'subtype': 'init', 'session_id': sid}}))
print(json.dumps({{'type': 'assistant', 'error': 'rate_limit', 'message': {{'content': []}}}}))
print(json.dumps({{'type': 'result', 'is_error': True, 'result': "You've hit your limit"}}))
sys.exit(1)
''')
        codex = self.executable("codex", f'''
import json, pathlib, sys
assert '--dangerously-bypass-approvals-and-sandbox' in sys.argv
assert pathlib.Path('partial.txt').read_text() == 'preserved'
prompt = sys.argv[-1]
assert 'Latest user request:' in prompt
if 'resume' not in sys.argv:
    assert 'usage_limit' in prompt
    assert 'request.json' in prompt
else:
    assert {CODEX_ID!r} in sys.argv
print(json.dumps({{'type': 'thread.started', 'thread_id': {CODEX_ID!r}}}))
print(json.dumps({{'type': 'turn.completed'}}), end='')
''')
        with patch.object(module.shutil, "which", side_effect=lambda name: {"claude": claude, "codex": codex}[name]):
            self.assertEqual(self.run_turn("Fix the current task"), 0)
            # Claude stays preferred but is skipped while its limit lasts.
            self.assertEqual(self.coordinator.state["agent"], "claude")
            self.assertFalse(self.coordinator.available("claude"))
            self.assertIn("claude", self.coordinator.state["sessions"])
            self.assertEqual(self.coordinator.state["sessions"]["codex"], CODEX_ID)
            self.assertEqual(self.run_turn("Continue with verification"), 0)
            # Without fallback a limited preference waits; /switch retries it now.
            self.assertEqual(self.run_turn(fallback=False), 1)
            with self.coordinator.lock() as fd:
                self.coordinator.handle("/switch claude", fd)
            self.assertEqual(self.run_turn(fallback=False), 1)
        requests = [json.loads(path.read_text()) for path in self.coordinator.directory.glob("*/request.json")]
        self.assertEqual(len(requests), 4)
        self.assertTrue(any("--resume" in request["command"] for request in requests))
        self.assertEqual((self.root / "docs/HANDOFF.md").read_text(), "Existing handoff\n")

    def test_daemon_runs_inbox_tasks_commands_pause_and_keepalive(self):
        codex = self.executable("codex", f'''
import json, pathlib, sys
pathlib.Path('prompts.txt').open('a').write(sys.argv[-1].split('Latest user request:')[-1].strip().splitlines()[0] + '\\n')
print(json.dumps({{'type': 'thread.started', 'thread_id': {CODEX_ID!r}}}))
print(json.dumps({{'type': 'turn.completed'}}))
''')
        with self.coordinator.lock() as fd, patch.object(module.shutil, "which", return_value=codex), \
                contextlib.redirect_stdout(io.StringIO()):
            remote = self.coordinator.remote_dirs()
            (remote / "inbox" / "001.cmd").write_text("/switch codex")
            (remote / "inbox" / "002.task").write_text("first remote task")
            (remote / "inbox" / "003.task").write_text("second remote task")
            self.coordinator.daemon(fd, keepalive=False, poll=0, sleep=lambda _: None, max_cycles=4)
            self.assertEqual(self.coordinator.state["agent"], "codex")
            self.assertEqual(sorted(p.name for p in (remote / "done").iterdir()),
                             ["001.cmd.exit0", "002.task.exit0", "003.task.exit0"])
            status = json.loads((remote / "status.json").read_text())
            self.assertEqual(status["state"], "idle")
            # Paused: tasks wait, commands still run, keepalive does nothing.
            (remote / "PAUSE").write_text("")
            (remote / "inbox" / "004.task").write_text("held task")
            self.coordinator.daemon(fd, keepalive=True, poll=0, sleep=lambda _: None, max_cycles=2)
            self.assertTrue((remote / "inbox" / "004.task").exists())
            # Resumed with an empty inbox: the keepalive sends the handoff task.
            (remote / "PAUSE").unlink()
            (remote / "inbox" / "004.task").unlink()
            self.coordinator.daemon(fd, keepalive=True, poll=0, sleep=lambda _: None, max_cycles=1)
            (remote / "STOP").write_text("")
            self.assertEqual(self.coordinator.daemon(fd, poll=0, sleep=lambda _: None), 0)
        prompts = (self.root / "prompts.txt").read_text().splitlines()
        self.assertEqual(prompts, ["first remote task", "second remote task",
                                   module.DEFAULT_TASK.splitlines()[0]])
        self.assertIn("keepalive", (remote / "daemon.log").read_text())

    def test_successful_reply_about_limits_does_not_trigger_fallback(self):
        codex = self.executable("codex", f'''
import json
print(json.dumps({{'type': 'thread.started', 'thread_id': {CODEX_ID!r}}}))
print(json.dumps({{'type': 'item.completed', 'item': {{'type': 'agent_message', 'text': 'The rate limit is documented.'}}}}))
print(json.dumps({{'type': 'turn.completed'}}))
''')
        with self.coordinator.lock():
            self.coordinator.state["agent"] = "codex"
            self.coordinator.save()
        with patch.object(module.shutil, "which", return_value=codex) as which:
            self.assertEqual(self.run_turn(), 0)
            self.assertEqual(which.call_count, 1)

    def test_authentication_failure_does_not_switch(self):
        claude = self.executable("claude", '''
import json, sys
print(json.dumps({'type': 'result', 'is_error': True, 'result': 'Authentication failed'}))
sys.exit(1)
''')
        with patch.object(module.shutil, "which", return_value=claude) as which:
            self.assertEqual(self.run_turn(), 1)
            self.assertEqual(which.call_count, 1)
            self.assertEqual(self.coordinator.state["agent"], "claude")
            self.assertNotIn("claude", self.coordinator.state["sessions"])

    def test_capacity_error_hands_over_and_reset_time_is_parsed(self):
        codex = self.executable("codex", f'''
import json, sys
print(json.dumps({{'type': 'thread.started', 'thread_id': {CODEX_ID!r}}}))
print(json.dumps({{'type': 'turn.failed', 'error': {{'message': 'Selected model is at capacity. Please try a different model.'}}}}))
sys.exit(1)
''')
        claude = self.executable("claude", '''
import json
print(json.dumps({'type': 'result', 'is_error': False, 'result': 'done'}))
''')
        with self.coordinator.lock():
            self.coordinator.state["agent"] = "codex"
            self.coordinator.save()
        with patch.object(module.shutil, "which", side_effect=lambda name: {"claude": claude, "codex": codex}[name]):
            self.assertEqual(self.run_turn(), 0)
        self.assertEqual(self.coordinator.state["last_run"]["agent"], "claude")
        until = module.dt.datetime.fromisoformat(self.coordinator.state["limited_until"]["codex"])
        self.assertLessEqual(until - module.dt.datetime.now(module.dt.timezone.utc), module.TRANSIENT_WAIT)
        current = module.dt.datetime(2026, 10, 28, 15, 12, tzinfo=module.dt.timezone.utc)
        reset = module.reset_time(["You've hit your session limit · resets 5:40am (Australia/Sydney)"], current)
        self.assertEqual((reset.hour, reset.minute, str(reset.tzinfo)), (5, 40, "Australia/Sydney"))
        self.assertGreater(module.reset_time(["try again at 1:49 AM."], current), current)

    def test_kimi_turn_records_session_and_hands_over_on_rate_limit(self):
        kimi = self.executable("kimi", f'''
import json, os, pathlib, sys
assert os.environ.get('KIMI_CODE_TRUST_WORKSPACE') == '1'
assert sys.argv[sys.argv.index('-m') + 1] == 'kimi-code/k3.1-pro'
calls = pathlib.Path('kimi-calls.txt')
n = int(calls.read_text()) + 1 if calls.exists() else 1
calls.write_text(str(n))
if n == 1:
    assert '-S' not in sys.argv
    print(json.dumps({{'role': 'assistant', 'content': 'Working', 'tool_calls': [{{'type': 'function', 'id': 't1', 'function': {{'name': 'Shell', 'arguments': '{{}}'}}}}]}}))
    print(json.dumps({{'role': 'tool', 'tool_call_id': 't1', 'content': 'ok'}}))
    print(json.dumps({{'role': 'meta', 'type': 'session.resume_hint', 'session_id': {KIMI_ID!r}}}))
else:
    assert sys.argv[sys.argv.index('-S') + 1] == {KIMI_ID!r}
    print('provider.rate_limit: Too many requests, retry later', file=sys.stderr)
    sys.exit(1)
''')
        claude = self.executable("claude", '''
import json
print(json.dumps({'type': 'result', 'is_error': False, 'result': 'done'}))
''')
        with self.coordinator.lock() as fd:
            self.coordinator.handle("/switch kimi", fd)
        with patch.object(module.shutil, "which", side_effect=lambda name: {"claude": claude, "kimi": kimi}.get(name)):
            self.assertEqual(self.run_turn(), 0)
            self.assertEqual(self.coordinator.state["sessions"]["kimi"], KIMI_ID)
            self.assertEqual(self.coordinator.state["last_run"]["agent"], "kimi")
            # Rate limited: the request goes to the next agent (codex is not installed here).
            with patch.object(module.shutil, "which", side_effect=lambda name: {"claude": claude, "kimi": kimi}.get(name)):
                self.run_turn()
        self.assertFalse(self.coordinator.available("kimi"))
        self.assertEqual(self.coordinator.state["agent"], "kimi")

    def test_oversized_session_starts_fresh(self):
        home = self.root / "claude-home"
        transcript = home / "projects" / "-x" / f"{CLAUDE_ID}.jsonl"
        transcript.parent.mkdir(parents=True)
        transcript.write_bytes(b"x" * 2048)
        with self.coordinator.lock(), patch.dict(os.environ, {"CLAUDE_CONFIG_DIR": str(home)}), \
                patch.object(module, "MAX_SESSION_MB", 0.001), contextlib.redirect_stdout(io.StringIO()):
            self.coordinator.state["sessions"]["claude"] = CLAUDE_ID
            self.coordinator.rotate_if_large("claude")
            self.assertNotIn("claude", self.coordinator.state["sessions"])

    def test_headless_daemon_refuses_cli(self):
        with self.coordinator.lock() as fd, patch.object(module.shutil, "which") as which:
            remote = self.coordinator.remote_dirs()
            (remote / "inbox" / "001.cmd").write_text("/cli")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.coordinator.daemon(fd, keepalive=False, poll=0, sleep=lambda _: None, max_cycles=1)
            which.assert_not_called()
        self.assertIn("needs a terminal", out.getvalue())

    def test_lock_blocks_other_coordinators_and_is_inherited_by_children(self):
        with self.coordinator.lock() as fd:
            child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"], pass_fds=(fd,))
            self.addCleanup(lambda: child.poll() is None and child.kill())
            with self.assertRaises(RuntimeError):
                with module.Coordinator(self.root).lock():
                    pass
        try:
            with self.assertRaises(RuntimeError):
                with module.Coordinator(self.root).lock():
                    pass
        finally:
            child.terminate()
            child.wait()
        with module.Coordinator(self.root).lock():
            pass

    def test_interrupt_stops_process_group_before_releasing_lock(self):
        codex = self.executable("codex", '''
import os, pathlib, signal, subprocess, sys, time
child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
pathlib.Path('descendant.pid').write_text(str(child.pid))
def stop(signum, frame):
    child.wait()
    sys.exit(0)
signal.signal(signal.SIGTERM, stop)
print('ready', flush=True)
time.sleep(60)
''')
        with self.coordinator.lock() as fd, patch.object(module.shutil, "which", return_value=codex):
            original = module.selectors.DefaultSelector.select

            def interrupt(selector, *args, **kwargs):
                events = original(selector, *args, **kwargs)
                if events:
                    raise KeyboardInterrupt
                return events

            with patch.object(module.selectors.DefaultSelector, "select", interrupt), \
                    contextlib.redirect_stdout(io.StringIO()):
                result = self.coordinator.run("codex", "Continue", fd)
            self.assertEqual(result["status"], "interrupted")
            descendant = int((self.root / "descendant.pid").read_text())
            with self.assertRaises(ProcessLookupError):
                os.kill(descendant, 0)


if __name__ == "__main__":
    unittest.main()
