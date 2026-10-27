"""Coverage must distinguish implemented tools from saved oracle experiments."""
import importlib.util
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


if __name__ == "__main__":
    unittest.main()
