import unittest

import pr_gate

GOOD = "## Release notes\n\n- Adds X\n\n## Validation\n\n- [x] cargo test\n"
HEAD = "a" * 40
BOT = {"login": pr_gate.CODEX, "type": "Bot"}
PR = {"state": "open", "draft": False, "base": {"ref": "main"}, "user": {"type": "User"},
      "head": {"sha": HEAD}, "body": GOOD}


def summary(sha, status="completed", unresolved=False):
    state = ('{"code_review":{"commit_sha":"%s","has_unresolved_findings":%s,"status":"%s"}}'
             % (sha, str(unresolved).lower(), status))
    return {"user": BOT, "body": f"<!-- codex-review-state:v1 {state} -->"}


class Body(unittest.TestCase):
    def test_accepts_filled_template(self):
        pr_gate.validate_body(GOOD + "\n## Not tested\n\n-\n")

    def test_rejects_placeholder_notes(self):
        with self.assertRaises(ValueError):
            pr_gate.validate_body("## Release notes\n\n<!-- hint -->\n\n-\n\n## Validation\n- [x] a")

    def test_rejects_unchecked_validation(self):
        with self.assertRaises(ValueError):
            pr_gate.validate_body("## Release notes\n- x\n## Validation\n- [x] a\n- [ ] b")
        with self.assertRaises(ValueError):
            pr_gate.validate_body("## Release notes\n- x\n## Validation\n- nothing")


class Gate(unittest.TestCase):
    def test_summary_state_for_head(self):
        self.assertEqual(pr_gate.evaluate(PR, [], [summary(HEAD)], [])[0], "success")

    def test_stale_or_running_review_waits(self):
        self.assertEqual(pr_gate.evaluate(PR, [], [summary("b" * 40)], [])[0], "pending")
        self.assertEqual(pr_gate.evaluate(PR, [], [summary(HEAD, "running")], [])[0], "pending")

    def test_receipt_short_sha(self):
        receipt = {"user": BOT, "body": "**Reviewed commit:** `aaaaaaaaaa`", "state": "COMMENTED"}
        self.assertEqual(pr_gate.evaluate(PR, [receipt], [], [])[0], "success")

    def test_ignores_other_authors(self):
        fake = {"user": {"login": "someone"}, "body": summary(HEAD)["body"]}
        self.assertEqual(pr_gate.evaluate(PR, [], [fake], [])[0], "pending")

    def test_unresolved_thread_blocks(self):
        self.assertEqual(pr_gate.evaluate(PR, [], [summary(HEAD)], [{"isResolved": False}])[0], "pending")

    def test_bad_body_fails(self):
        self.assertEqual(pr_gate.evaluate({**PR, "body": ""}, [], [summary(HEAD)], [])[0], "failure")

    def test_bot_authors_skip_body_check(self):
        bot = {**PR, "body": "", "user": {"type": "Bot"}}
        self.assertEqual(pr_gate.evaluate(bot, [], [summary(HEAD)], [])[0], "success")


REPO = "owner/repo"
BRANCH_PR = {**PR, "head": {"sha": HEAD, "ref": "feat/x", "repo": {"full_name": REPO}}}


def run(sha, status="completed", run_id=1):
    return {"id": run_id, "head_sha": sha, "status": status}


class HostedChecks(unittest.TestCase):
    def plan(self, state, runs=None, pr=BRANCH_PR):
        return pr_gate.check_actions(pr, REPO, state, runs or {"ci.yml": [], "desktop.yml": []})

    def test_a_passing_gate_dispatches_each_workflow_once(self):
        self.assertEqual(self.plan("success"), [("dispatch", "ci.yml"), ("dispatch", "desktop.yml")])

    def test_nothing_starts_before_codex_has_finished(self):
        self.assertEqual(self.plan("pending"), [])
        self.assertEqual(self.plan("failure"), [])

    def test_the_same_commit_is_never_dispatched_twice_even_after_a_failure(self):
        runs = {"ci.yml": [run(HEAD, "in_progress")], "desktop.yml": [run(HEAD, "completed")]}
        self.assertEqual(self.plan("success", runs), [])

    def test_only_unfinished_runs_of_older_commits_are_cancelled(self):
        old = "b" * 40
        runs = {"ci.yml": [run(old, "in_progress", 5), run(old, "completed", 6), run(old, "queued", 7)],
                "desktop.yml": [run(HEAD, "in_progress", 8)]}
        self.assertEqual(self.plan("pending", runs), [("cancel", 5), ("cancel", 7)])

    def test_a_new_commit_cancels_the_stale_run_then_waits_for_its_own_review(self):
        runs = {"ci.yml": [run("b" * 40, "in_progress", 5)], "desktop.yml": []}
        self.assertEqual(self.plan("pending", runs), [("cancel", 5)])

    def test_fork_branches_cannot_be_dispatched(self):
        fork = {**PR, "head": {"sha": HEAD, "ref": "feat/x", "repo": {"full_name": "someone/repo"}}}
        self.assertEqual(self.plan("success", pr=fork), [])


class Dispatch(unittest.TestCase):
    def test_before_the_merge_only_the_linux_runner_is_started(self):
        self.assertEqual(pr_gate.dispatch_payload("feat/x"), {"ref": "feat/x", "inputs": {"os": "ubuntu-24.04"}})

    def test_the_pre_merge_runner_is_linux(self):
        # Windows and macOS runs are slow and run after the merge, on main.
        self.assertTrue(pr_gate.PR_RUNNER.startswith("ubuntu-"))


if __name__ == "__main__":
    unittest.main()
