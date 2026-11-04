"""A release artifact must correspond to the binary that actually passed."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("alpha_package", Path(__file__).with_name("package.py"))
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class PackageTests(unittest.TestCase):
    def invoke(self, root, report):
        binary = root / "ug"
        binary.write_bytes(b"tested-binary")
        result = root / "report.json"
        result.write_text(json.dumps(report))
        proj = root / "proj"
        proj.mkdir(exist_ok=True)
        (proj / "proj.db").write_bytes(b"test database")
        notice = root / "COPYING"
        notice.write_text("Test notice")
        with patch("sys.argv", ["package.py", "--ug", str(binary), "--report", str(result), "--output", str(root / "dist"),
                                "--proj-data", str(proj), "--proj-license", str(notice)]):
            with patch.object(package.subprocess, "check_output", side_effect=["a" * 40, ""]):
                package.main()

    def test_rejects_failed_report_even_when_hash_matches(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(SystemExit):
                self.invoke(Path(tmp), {"passed": False, "binary_sha256": hashlib.sha256(b"tested-binary").hexdigest()})
            self.assertFalse((Path(tmp) / "dist").exists())

    def test_rejects_report_for_another_binary(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(SystemExit):
                self.invoke(Path(tmp), {"passed": True, "binary_sha256": "incorrect"})
            self.assertFalse((Path(tmp) / "dist").exists())

    def test_archive_is_repeatable_and_contains_exact_tested_bytes(self):
        report = {"passed": True, "binary_sha256": hashlib.sha256(b"tested-binary").hexdigest()}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.invoke(root, report)
            archive = next((root / "dist").glob("*.tar.gz"))
            first = archive.read_bytes()
            with tarfile.open(archive) as tar:
                executable = next(m for m in tar.getmembers() if m.name.endswith("/bin/ug"))
                self.assertEqual(executable.mode, 0o755)
                self.assertEqual(tar.extractfile(executable).read(), b"tested-binary")
                self.assertTrue(any(m.name.endswith("/share/proj/proj.db") for m in tar.getmembers()))
            self.invoke(root, report)
            self.assertEqual(archive.read_bytes(), first)


if __name__ == "__main__":
    unittest.main()
