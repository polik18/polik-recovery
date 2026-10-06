import importlib.util
import pathlib
import tempfile
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("generate_release_evidence.py")
SPEC = importlib.util.spec_from_file_location("release_evidence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(MODULE)


class ReleaseEvidenceTests(unittest.TestCase):
    def test_sha256_known_value(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "artifact.bin"
            path.write_bytes(b"abc")
            self.assertEqual(
                MODULE.sha256(path),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            )

    def test_license_screen_fails_closed(self):
        packages = [
            {"id": "a", "name": "missing", "version": "1.0.0", "license": None},
            {"id": "b", "name": "blocked", "version": "1.0.0", "license": "AGPL-3.0-only"},
        ]
        result = MODULE.license_screen(packages, ["AGPL-"])
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["missing_license"], ["missing@1.0.0"])
        self.assertEqual(result["forbidden_hits"][0]["package"], "blocked@1.0.0")

    def test_workspace_version_requires_one_version(self):
        metadata = {
            "workspace_members": ["a", "b"],
            "packages": [
                {"id": "a", "version": "1.0.0"},
                {"id": "b", "version": "2.0.0"},
            ],
        }
        with self.assertRaises(ValueError):
            MODULE.workspace_version(metadata)


if __name__ == "__main__":
    unittest.main()
