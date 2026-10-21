import contextlib
import io
import json
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release_history as history
import release_notes


class PriorSource(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.git("init", "-q", "-b", "old")
        self.git("config", "user.name", "Release test")
        self.git("config", "user.email", "release-test@example.invalid")
        self.released = self.commit("Old change (#1)")
        self.git("tag", "v0.1.11")
        self.git("checkout", "-q", "--orphan", "main")
        self.baseline = self.commit("Rewritten old change (#1)")
        self.head = self.commit("New change (#23)")
        self.migrations = {"v0.1.11": {
            "released_sha": self.released,
            "equivalent_main_sha": self.baseline,
            "reason": "Verified source-history migration",
        }}
        self.addCleanup(patch.stopall)
        patch.object(history, "git", self.git).start()

    def git(self, *args):
        return subprocess.run(
            ["git", *args], cwd=self.directory.name, text=True,
            capture_output=True, check=True,
        ).stdout.strip()

    def commit(self, subject):
        self.git("commit", "-q", "--allow-empty", "-m", subject)
        return self.git("rev-parse", "HEAD")

    def test_initial_release_has_no_prior_source(self):
        self.git("tag", "-d", "v0.1.11")
        self.assertIsNone(history.prior_source(self.head, {}))

    def test_normal_history_uses_the_latest_global_tag_commit(self):
        self.git("tag", "v0.1.12", self.baseline)
        self.assertEqual(history.prior_source(self.head, self.migrations), self.baseline)

    def test_annotated_release_tag_resolves_to_its_commit(self):
        self.git("tag", "-a", "v0.1.12", self.baseline, "-m", "release")
        self.assertEqual(history.prior_source(self.head, self.migrations), self.baseline)

    def test_verified_migration_uses_the_equivalent_main_baseline(self):
        self.assertEqual(history.prior_source(self.head, self.migrations), self.baseline)

    def test_unknown_unrelated_history_does_not_fall_back_to_an_older_merged_tag(self):
        self.git("tag", "v0.1.10", self.baseline)
        with self.assertRaisesRegex(ValueError, "not an ancestor"):
            history.prior_source(self.head, {})

    def test_retargeted_declared_tag_is_rejected_even_if_it_is_now_an_ancestor(self):
        self.git("tag", "-f", "v0.1.11", self.baseline)
        with self.assertRaisesRegex(ValueError, "released_sha"):
            history.prior_source(self.head, self.migrations)

    def test_unrelated_migration_baseline_is_rejected(self):
        self.migrations["v0.1.11"]["equivalent_main_sha"] = self.released
        with self.assertRaisesRegex(ValueError, "baseline.*not an ancestor"):
            history.prior_source(self.head, self.migrations)

    def test_missing_migration_baseline_is_rejected(self):
        self.migrations["v0.1.11"]["equivalent_main_sha"] = "0" * 40
        with self.assertRaises(subprocess.CalledProcessError):
            history.prior_source(self.head, self.migrations)

    def test_abbreviated_migration_sha_is_rejected(self):
        self.migrations["v0.1.11"]["equivalent_main_sha"] = self.baseline[:12]
        with self.assertRaisesRegex(ValueError, "full commit SHA"):
            history.prior_source(self.head, self.migrations)

    def test_checked_source_before_migration_baseline_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "baseline.*not an ancestor"):
            history.prior_source(self.released, self.migrations)

    def test_notes_and_has_public_exclude_prs_before_the_migrated_baseline(self):
        requested = []

        def run(*args):
            if args[0] == "git":
                return self.git(*args[1:])
            requested.append(args[-1])
            return json.dumps({"body": "## Release notes\n- New public change"})

        for command in ("notes", "has-public"):
            with self.subTest(command=command):
                output = io.StringIO()
                with patch.object(history, "load_migrations", return_value=self.migrations), \
                     patch.object(release_notes, "run", side_effect=run), \
                     patch("sys.argv", ["release_notes.py", command, "--head", self.head, "--repo", "owner/repo"]), \
                     contextlib.redirect_stdout(output):
                    release_notes.main()
                self.assertEqual(requested.pop(), "repos/owner/repo/pulls/23")
                self.assertEqual(requested, [])
                self.assertIn("New public change" if command == "notes" else "true", output.getvalue())

    def test_internal_only_new_pr_does_not_replay_old_public_notes(self):
        def run(*args):
            if args[0] == "git":
                return self.git(*args[1:])
            number = args[-1].split("/")[-1]
            return json.dumps({"body": "## Release notes\n- " + (
                "Internal: repair tooling" if number == "23" else "Old public change")})

        output = io.StringIO()
        with patch.object(history, "load_migrations", return_value=self.migrations), \
             patch.object(release_notes, "run", side_effect=run), \
             patch("sys.argv", ["release_notes.py", "has-public", "--head", self.head, "--repo", "owner/repo"]), \
             contextlib.redirect_stdout(output):
            release_notes.main()
        self.assertEqual(output.getvalue().strip(), "false")


if __name__ == "__main__":
    unittest.main()
