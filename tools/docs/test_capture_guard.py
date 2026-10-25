"""Tests for the capture driver's sandbox check. Run: python3 -m unittest discover -s tools/docs"""
import os
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "capture"))
import cu  # noqa: E402

STARTED = "Sun Oct 25 09:00:00 2026"


class CaptureGuard(unittest.TestCase):
    def registry(self, text):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        path = os.path.join(folder.name, "capture-app")
        if text is not None:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(text)
        return path

    def check(self, path, running, started=STARTED):
        with mock.patch.object(cu, "running_cuerithm", return_value=running), \
                mock.patch.object(cu, "started_at", return_value=started):
            cu.require_capture_app(path)

    def test_the_sandboxed_app_alone_passes(self):
        self.check(self.registry(f"4242\n{STARTED}\n/data/copy\n"), ["4242"])

    def test_nothing_registered_stops(self):
        with self.assertRaisesRegex(RuntimeError, "launch-copy.sh"):
            self.check(self.registry(None), ["4242"])

    def test_a_malformed_registry_stops(self):
        with self.assertRaisesRegex(RuntimeError, "launch-copy.sh"):
            self.check(self.registry("4242\n"), ["4242"])

    def test_the_installed_app_running_next_to_it_stops(self):
        with self.assertRaisesRegex(RuntimeError, "only CueRithm"):
            self.check(self.registry(f"4242\n{STARTED}\n/data/copy\n"), ["4242", "16198"])

    def test_the_sandboxed_app_having_quit_stops(self):
        with self.assertRaisesRegex(RuntimeError, "only CueRithm"):
            self.check(self.registry(f"4242\n{STARTED}\n/data/copy\n"), [])

    def test_another_app_started_after_it_quit_stops(self):
        with self.assertRaisesRegex(RuntimeError, "only CueRithm"):
            self.check(self.registry(f"4242\n{STARTED}\n/data/copy\n"), ["4242"], started="Sun Oct 25 10:30:00 2026")


if __name__ == "__main__":
    unittest.main()
