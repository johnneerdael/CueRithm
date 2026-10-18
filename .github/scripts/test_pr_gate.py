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


if __name__ == "__main__":
    unittest.main()
