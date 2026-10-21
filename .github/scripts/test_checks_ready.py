import unittest
from unittest.mock import patch
import io
import subprocess

from checks_ready import REQUIRED, combine, observations, ready, resolve, retry_policy


class ChecksReadyTest(unittest.TestCase):
    def passed(self):
        return {role: {"conclusion": "success"} for role in REQUIRED}

    def test_full_matrix_requires_every_component(self):
        checks = self.passed()
        self.assertTrue(ready(checks)[0])
        del checks["desktop/macos-15"]
        self.assertEqual(ready(checks), (False, ["desktop/macos-15"]))

    def test_windows_repair_retains_other_successes(self):
        retry = {"engine/windows-2025", "desktop/windows-2025"}
        current = {role: {"conclusion": "success"} for role in retry}
        retained = self.passed()
        for role in retry:
            retained[role] = {"conclusion": "failure"}
        self.assertTrue(ready(current, retained, retry)[0])

    def test_retry_cannot_inherit_an_old_success(self):
        role = "engine/windows-2025"
        self.assertFalse(ready({}, self.passed(), {role})[0])
        self.assertFalse(ready({role: {"conclusion": "failure"}}, self.passed(), {role})[0])

    def test_retained_pending_or_failed_component_blocks_release(self):
        for conclusion in (None, "failure", "cancelled"):
            retained = self.passed()
            retained["desktop/macos-15"] = {"conclusion": conclusion}
            self.assertFalse(ready({}, retained)[0])

    def test_current_failure_overrides_retained_success(self):
        self.assertFalse(ready({"desktop/macos-15": {"conclusion": "failure"}}, self.passed())[0])

    def test_later_repair_can_keep_the_first_repair_success(self):
        engine, desktop = "engine/windows-2025", "desktop/windows-2025"
        baseline = self.passed()
        first = combine({engine: {"conclusion": "success", "sha": "first"},
                         desktop: {"conclusion": "failure"}}, baseline, {engine, desktop})
        final = combine({desktop: {"conclusion": "success", "sha": "final"}}, first, {desktop})
        self.assertTrue(ready(final)[0])
        self.assertEqual(final[engine]["sha"], "first")
        self.assertEqual(final[desktop]["sha"], "final")

    def test_declaration_is_explicit_and_bounded(self):
        baseline = "a" * 40
        message = f"fix(ci): repair Windows [skip ci]\n\nCI-Baseline: {baseline}\nCI-Retry: engine/windows-2025 desktop/windows-2025\n"
        self.assertEqual(retry_policy(message), (baseline, {"engine/windows-2025", "desktop/windows-2025"}))
        self.assertEqual(retry_policy("ordinary commit"), (None, set()))
        for invalid in (message.replace("[skip ci]", ""), message.replace("engine/windows-2025", "unknown"),
                        message + f"CI-Baseline: {baseline}\n", message.replace("CI-Retry:", "Other:")):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                retry_policy(invalid)

    def test_observations_keep_the_latest_attempt_and_reject_other_sources(self):
        sha = "a" * 40
        runs = [{"id": 1, "head_sha": sha, "event": "push", "created_at": "2026-10-21T00:00:00Z"},
                {"id": 9, "head_sha": "b" * 40, "event": "push", "created_at": "2026-10-22T00:00:00Z"}]
        jobs = [{"id": 10, "name": "engine (windows-2025)", "conclusion": "success", "started_at": "2026-10-21T00:00:00Z"},
                {"id": 11, "name": "engine (windows-2025)", "conclusion": "failure", "started_at": "2026-10-21T01:00:00Z"},
                {"id": 12, "name": "engine-rust-floor", "conclusion": "skipped", "started_at": "2026-10-21T01:00:00Z"}]

        def api(path, paginate=False):
            self.assertTrue(paginate)
            self.assertNotIn("/runs/9/", path)
            return [{"workflow_runs": runs}] if "/workflows/" in path else [{"jobs": jobs}]

        with patch("checks_ready.gh", side_effect=api):
            found = observations("owner/repo", sha)
        self.assertEqual(set(found), {"engine/windows-2025"})
        self.assertEqual(found["engine/windows-2025"]["conclusion"], "failure")
        self.assertEqual(found["engine/windows-2025"]["sha"], sha)

    def test_new_unmapped_checks_fail_closed(self):
        run = {"id": 1, "head_sha": "a" * 40, "event": "push", "created_at": "2026-10-21T00:00:00Z"}
        with patch("checks_ready.gh", side_effect=[[{"workflow_runs": [run]}],
                   [{"jobs": [{"name": "new required check", "conclusion": "failure"}]}]]):
            with self.assertRaisesRegex(ValueError, "Unmapped release check"):
                observations("owner/repo", "a" * 40)

    def test_queued_rerun_overrides_a_previous_green_attempt(self):
        run = {"id": 1, "head_sha": "a" * 40, "event": "push", "created_at": "2026-10-21T00:00:00Z", "run_attempt": 2}
        jobs = [{"id": 10, "name": "engine (windows-2025)", "conclusion": "success", "run_attempt": 1, "started_at": "2026-10-21T01:00:00Z"},
                {"id": 11, "name": "engine (windows-2025)", "conclusion": None, "run_attempt": 2, "started_at": None}]
        with patch("checks_ready.gh", side_effect=lambda path, *args: [{"workflow_runs": [run]}] if "/workflows/" in path else [{"jobs": jobs}]):
            result = observations("owner/repo", "a" * 40)
        self.assertIsNone(result["engine/windows-2025"]["conclusion"])
        self.assertEqual(result["engine/windows-2025"]["job_id"], 11)

    def test_unobservable_new_attempt_cannot_reuse_old_green_jobs(self):
        run = {"id": 1, "head_sha": "a" * 40, "event": "push", "created_at": "2026-10-21T00:00:00Z", "run_attempt": 2}
        jobs = [{"id": 10, "name": "engine (windows-2025)", "conclusion": "success", "run_attempt": 1, "started_at": "2026-10-21T01:00:00Z"}]
        with patch("checks_ready.gh", side_effect=lambda path, *args: [{"workflow_runs": [run]}] if "/workflows/" in path else [{"jobs": jobs}]):
            result = observations("owner/repo", "a" * 40)
        self.assertIsNone(result["engine/windows-2025"]["conclusion"])
        self.assertFalse(ready(result)[0])

    def test_unmapped_skipped_checks_also_fail_closed(self):
        run = {"id": 1, "head_sha": "a" * 40, "event": "push", "created_at": "2026-10-21T00:00:00Z"}
        with patch("checks_ready.gh", side_effect=[[{"workflow_runs": [run]}],
                   [{"jobs": [{"name": "new check", "conclusion": "skipped"}]}]]):
            with self.assertRaisesRegex(ValueError, "Unmapped release check"):
                observations("owner/repo", "a" * 40)

    def test_partial_new_attempt_visibility_cannot_reuse_previous_matrix_greens(self):
        run = {"id": 1, "head_sha": "a" * 40, "event": "push", "created_at": "2026-10-21T00:00:00Z",
               "run_attempt": 2, "status": "in_progress"}
        jobs = [{"id": 10 + index, "name": f"engine ({os})", "conclusion": "success", "run_attempt": 1}
                for index, os in enumerate(("ubuntu-24.04", "macos-15", "windows-2025"))]
        jobs.append({"id": 99, "name": "engine-rust-floor", "conclusion": "success", "run_attempt": 2})
        def api(path, *args):
            if "/workflows/desktop.yml/" in path:
                return [{"workflow_runs": []}]
            return [{"workflow_runs": [run]}] if "/workflows/" in path else [{"jobs": jobs}]
        with patch("checks_ready.gh", side_effect=api):
            result = observations("owner/repo", "a" * 40)
        self.assertIsNone(result["engine/ubuntu-24.04"]["conclusion"])
        self.assertFalse(ready(result, self.passed())[0])

    def test_repair_chain_retains_the_exact_successful_sources(self):
        base, first, final = "a" * 40, "b" * 40, "c" * 40
        engine, desktop = "engine/windows-2025", "desktop/windows-2025"
        messages = {base: "original merge", first: f"fix [skip ci]\nCI-Baseline: {base}\nCI-Retry: {engine} {desktop}",
                    final: f"fix [skip ci]\nCI-Baseline: {first}\nCI-Retry: {desktop}"}
        checks = {base: {role: {"conclusion": "success", "sha": base} for role in REQUIRED},
                  first: {engine: {"conclusion": "success", "sha": first}, desktop: {"conclusion": "failure", "sha": first}},
                  final: {desktop: {"conclusion": "success", "sha": final}}}
        with patch("checks_ready.subprocess.check_output", side_effect=lambda command, **kwargs: messages[command[-1]]), \
             patch("checks_ready.subprocess.run") as ancestry, \
             patch("checks_ready.observations", side_effect=lambda repo, sha: checks[sha]), \
             patch("checks_ready.sys.stderr", io.StringIO()):
            result = resolve("owner/repo", final)
        self.assertTrue(ready(result)[0])
        self.assertEqual(result[engine]["sha"], first)
        self.assertEqual(result[desktop]["sha"], final)
        self.assertEqual(result["engine/macos-15"]["sha"], base)
        self.assertEqual(ancestry.call_count, 2)

    def test_unrelated_baseline_is_rejected_before_reading_checks(self):
        baseline, head = "a" * 40, "b" * 40
        message = f"fix [skip ci]\nCI-Baseline: {baseline}\nCI-Retry: engine/windows-2025"
        with patch("checks_ready.subprocess.check_output", return_value=message), \
             patch("checks_ready.subprocess.run", side_effect=subprocess.CalledProcessError(1, "git merge-base")), \
             patch("checks_ready.observations") as api:
            with self.assertRaises(subprocess.CalledProcessError):
                resolve("owner/repo", head)
        api.assert_not_called()


if __name__ == "__main__":
    unittest.main()
