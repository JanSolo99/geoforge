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
        sessions = {"claude": CLAUDE_ID, "codex": CODEX_ID}
        for agent in module.AGENTS:
            for native in (False, True):
                for resumed in (False, True):
                    with self.subTest(agent=agent, native=native, resumed=resumed):
                        self.coordinator.state["sessions"] = {agent: sessions[agent]} if resumed else {}
                        executable = str(self.bin / agent)
                        with patch.object(module.shutil, "which", return_value=executable):
                            command, session = self.coordinator.command(agent, "Continue", native)
                        self.assertEqual(command[0], executable)
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

    def test_partial_work_survives_limit_switch_and_both_sessions_resume(self):
        claude = self.executable("claude", f'''
import json, pathlib, sys
assert '--dangerously-skip-permissions' in sys.argv
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
            self.assertEqual(self.coordinator.state["agent"], "codex")
            self.assertIn("claude", self.coordinator.state["sessions"])
            self.assertEqual(self.coordinator.state["sessions"]["codex"], CODEX_ID)
            self.assertEqual(self.run_turn("Continue with verification"), 0)
            self.coordinator.state["agent"] = "claude"
            self.coordinator.save()
            self.assertEqual(self.run_turn(fallback=False), 1)
        requests = [json.loads(path.read_text()) for path in self.coordinator.directory.glob("*/request.json")]
        self.assertEqual(len(requests), 4)
        self.assertTrue(any("--resume" in request["command"] for request in requests))
        self.assertEqual((self.root / "docs/HANDOFF.md").read_text(), "Existing handoff\n")

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
