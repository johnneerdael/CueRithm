"""Release stamping keeps Tauri, Rust provenance and locked packages in sync."""
import importlib.util
import json
from pathlib import Path
import tempfile
import tomllib
import unittest


spec = importlib.util.spec_from_file_location("stamp_version", Path(__file__).with_name("stamp-version.py"))
stamp_version = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stamp_version)


class StampVersionTest(unittest.TestCase):
    def fixture(self, root):
        (root / "Cargo.toml").write_text('''[workspace]
members = ["core", "adapter"]
[workspace.package]
version = "0.1.0"
edition = "2024"
[workspace.dependencies]
serde = "1"
''', encoding="utf-8")
        for name in ("core", "adapter"):
            (root / name).mkdir()
            (root / name / "Cargo.toml").write_text(
                f'[package]\nname = "{name}"\nversion.workspace = true\n', encoding="utf-8")
        (root / "Cargo.lock").write_text('''# Preserve Cargo formatting and external versions.
version = 4

[[package]]
name = "adapter"
version = "0.1.0"
dependencies = ["core 0.1.0", "core 0.1.0 (registry+https://example.invalid/index)"]

[[package]]
name = "core"
version = "0.1.0"

[[package]]
name = "core"
version = "0.1.0"
source = "registry+https://example.invalid/index"
checksum = "untouched"
''', encoding="utf-8")

    def test_nondefault_version_reaches_every_local_cargo_package_and_tauri(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            output = root / "generated/tauri.json"
            stamp_version.stamp("2.37.104", output, root)
            self.assertEqual(json.loads(output.read_text()), {"version": "2.37.104"})
            manifest = tomllib.loads((root / "Cargo.toml").read_text())
            self.assertEqual(manifest["workspace"]["package"]["version"], "2.37.104")
            self.assertEqual(manifest["workspace"]["dependencies"], {"serde": "1"})
            packages = tomllib.loads((root / "Cargo.lock").read_text())["package"]
            self.assertEqual([p["version"] for p in packages], ["2.37.104", "2.37.104", "0.1.0"])
            self.assertEqual(packages[0]["dependencies"], [
                "core 2.37.104", "core 0.1.0 (registry+https://example.invalid/index)"])
            self.assertEqual(packages[2]["checksum"], "untouched")
            self.assertTrue((root / "Cargo.lock").read_text().startswith("# Preserve"))
            # Running again is safe, including when package retries reuse checkout.
            snapshot = (root / "Cargo.lock").read_bytes()
            stamp_version.stamp("2.37.104", output, root)
            self.assertEqual((root / "Cargo.lock").read_bytes(), snapshot)

    def test_invalid_versions_leave_all_files_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            output = root / "generated/tauri.json"
            original = {p: p.read_bytes() for p in root.rglob("*") if p.is_file()}
            for version in ("v1.2.3", "01.2.3", "1.2", "1.2.3\n", "1.2.3-beta", "1.2.3/evil"):
                with self.subTest(version=version), self.assertRaises(ValueError):
                    stamp_version.stamp(version, output, root)
            self.assertFalse(output.exists())
            self.assertEqual({p: p.read_bytes() for p in original}, original)

    def test_unexpected_local_lock_version_fails_before_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            lock = root / "Cargo.lock"
            lock.write_text(lock.read_text().replace('name = "adapter"\nversion = "0.1.0"',
                                                    'name = "adapter"\nversion = "0.2.0"'))
            original = (root / "Cargo.toml").read_bytes(), lock.read_bytes()
            with self.assertRaisesRegex(ValueError, "Unexpected locked release identity"):
                stamp_version.stamp("2.37.104", root / "tauri.json", root)
            self.assertEqual(((root / "Cargo.toml").read_bytes(), lock.read_bytes()), original)
            self.assertFalse((root / "tauri.json").exists())


if __name__ == "__main__":
    unittest.main()
