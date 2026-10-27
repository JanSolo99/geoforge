"""Coverage must distinguish implemented tools from saved oracle experiments."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("dashboard", Path(__file__).with_name("build.py"))
dashboard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dashboard)


class CoverageTests(unittest.TestCase):
    def test_parameters_are_not_tools(self):
        source = '''Tool {
            id: "pick", name: "Pick",
            params: &[Param { name: "items" }, Param { name: "input" }],
        }
        unary_tool!(abs, "abs", "Abs"),
        binary_tool!(power, "power", "Power"),
        '''
        self.assertEqual(dashboard.tool_names(source), ["Pick", "Abs", "Power"])

    def test_registered_external_constants_are_discovered(self):
        with tempfile.TemporaryDirectory() as root:
            src = Path(root) / "crates/unigis-tools/src"
            src.mkdir(parents=True)
            (src / "registry.rs").write_text("pub static TOOLS: &[Tool] = &[crate::new_module::PUBLIC_TOOL,];")
            (src / "new_module.rs").write_text('''
pub const PUBLIC_TOOL: Tool = Tool {
    id: "public", name: "Public Tool", params: &[],
};
pub const PRIVATE_TOOL: Tool = Tool {
    id: "private", name: "Private Tool", params: &[],
};
''')
            with patch.object(dashboard, "ROOT", Path(root)):
                self.assertEqual(dashboard.implemented(), {"publictool": "tools registry"})

    def test_oracle_only_tool_is_not_claimed_implemented(self):
        self.assertEqual(dashboard.coverage_status("Unfinished", {}, {"unfinished": 3}), "missing")
        self.assertEqual(dashboard.coverage_status("Finished", {"finished": "registry"}, {}), "implemented")
        self.assertEqual(dashboard.coverage_status("Finished", {"finished": "registry"}, {"finished": 2}), "verified")

    def test_only_successful_reference_cases_count(self):
        manifest = {"cases": {
            "ok": {"tool": "gp.Abs_sa", "status": "succeeded"},
            "bad": {"tool": "gp.Abs_sa", "status": "failed"},
            "unavailable": {"tool": "gp.Unknown_sa", "status": "failed"},
        }}
        self.assertEqual(dashboard.verified(manifest), {"abs": 1})


class SnapshotTests(unittest.TestCase):
    RUNS = [{"status": "completed", "conclusion": "success", "displayTitle": "A checkpoint",
             "createdAt": "2026-10-27T13:00:00Z", "url": "https://github.com/example/actions/runs/1"}]

    def test_offline_rebuild_keeps_the_last_ci_fetch(self):
        with tempfile.TemporaryDirectory() as root, patch.object(dashboard, "ROOT", Path(root)):
            with patch.object(dashboard, "ci_runs", return_value=self.RUNS):
                fresh = dashboard.ci_data(True)
            with patch.object(dashboard, "ci_runs", side_effect=AssertionError("offline must not fetch")):
                self.assertEqual(dashboard.ci_data(False), fresh)
            self.assertEqual(fresh[0], self.RUNS)
            self.assertTrue(fresh[1].endswith("+00:00"))

    def test_failed_fetch_does_not_erase_the_cached_results(self):
        with tempfile.TemporaryDirectory() as root, patch.object(dashboard, "ROOT", Path(root)):
            with patch.object(dashboard, "ci_runs", return_value=self.RUNS):
                fresh = dashboard.ci_data(True)
            with patch.object(dashboard, "ci_runs", return_value=None):
                self.assertEqual(dashboard.ci_data(True), fresh)

    def test_missing_or_corrupt_cache_is_empty(self):
        with tempfile.TemporaryDirectory() as root, patch.object(dashboard, "ROOT", Path(root)):
            self.assertEqual(dashboard.ci_data(False), ([], None))
            path = Path(root) / "docs/dashboard/ci_runs.json"
            path.parent.mkdir(parents=True)
            for content in ["invalid", "[]", json.dumps({"runs": [{}], "fetched": "yesterday"})]:
                path.write_text(content)
                self.assertEqual(dashboard.ci_data(False), ([], None))

    def test_gap_card_uses_the_latest_section(self):
        with tempfile.TemporaryDirectory() as root, patch.object(dashboard, "ROOT", Path(root)):
            path = Path(root) / "gaps.md"
            path.write_text("# Gaps\n\n## Old batch\nold\n\n## New batch\ncurrent\n")
            self.assertEqual(dashboard.latest_section("gaps.md"), "New batch\ncurrent")


if __name__ == "__main__":
    unittest.main()
