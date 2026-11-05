"""Installation checks must reject stale or corrupted package evidence."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("alpha_runtime", Path(__file__).with_name("runtime.py"))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "bin").mkdir()
        (self.root / "share/proj").mkdir(parents=True)
        (self.root / "bin/ug").write_bytes(b"binary")
        (self.root / "share/proj/proj.db").write_bytes(b"database")
        self.manifest = {"binary_sha256": hashlib.sha256(b"binary").hexdigest(),
                         "proj_db_sha256": hashlib.sha256(b"database").hexdigest()}
        (self.root / "manifest.json").write_text(json.dumps(self.manifest))
        (self.root / "readiness.json").write_text(json.dumps(
            {"passed": True, "binary_sha256": self.manifest["binary_sha256"]}))

    def test_valid_package_and_tampered_binary(self):
        self.assertEqual(runtime.verify_package(self.root), self.manifest)
        (self.root / "bin/ug").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "binary does not match"):
            runtime.verify_package(self.root)

    def test_tampered_database_and_failed_report(self):
        (self.root / "share/proj/proj.db").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "PROJ database"):
            runtime.verify_package(self.root)
        (self.root / "share/proj/proj.db").write_bytes(b"database")
        (self.root / "readiness.json").write_text(json.dumps(
            {"passed": False, "binary_sha256": self.manifest["binary_sha256"]}))
        with self.assertRaisesRegex(ValueError, "did not pass"):
            runtime.verify_package(self.root)

    def test_clean_runtime_rejects_gdal(self):
        with patch.object(runtime.shutil, "which", side_effect=lambda name: "/usr/bin/ogrinfo" if name == "ogrinfo" else None):
            with self.assertRaisesRegex(ValueError, "unexpected tools"):
                runtime.check_clean_runtime()

    def test_report_is_written_for_invalid_package(self):
        report = self.root / "result.json"
        (self.root / "manifest.json").write_text("invalid JSON")
        with patch("sys.argv", ["runtime.py", "--package", str(self.root), "--report", str(report)]):
            self.assertEqual(runtime.main(), 1)
        self.assertIs(json.loads(report.read_text())["passed"], False)
