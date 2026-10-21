import unittest

import release_notes as rn


class Release(unittest.TestCase):
    def test_version_increments_patch_on_the_base_line(self):
        self.assertEqual(rn.next_version("0.1.0", []), "0.1.0")
        self.assertEqual(rn.next_version("0.1.0", ["v0.1.0", "v0.1.4", "v0.2.9"]), "0.1.5")

    def test_entries_split_public_internal_and_direct(self):
        commits = [("1", "feat: a (#5)"), ("2", "ci: b (#6)"), ("3", "hotfix without pr")]
        bodies = {5: "## Release notes\n\n- Adds A\n- Fixes B\n", 6: "## Release notes\n- Internal: tidy CI\n"}
        public, internal, other = rn.entries(commits, bodies)
        self.assertEqual(public, ["Adds A", "Fixes B"])
        self.assertEqual(internal, ["tidy CI"])
        self.assertEqual(other, ["hotfix without pr"])

    def test_wrapped_bullets_stay_whole(self):
        body = "## Release notes\n- First line\n  continues here\n- Second\n"
        public, _, _ = rn.entries([("1", "x (#9)")], {9: body})
        self.assertEqual(public, ["First line continues here", "Second"])

    def test_merge_commit_subjects_resolve_the_pr(self):
        public, _, _ = rn.entries([("1", "Merge pull request #7 from x/y")], {7: "## Release notes\n- Z"})
        self.assertEqual(public, ["Z"])


class Page(unittest.TestCase):
    def test_users_see_their_changes_first_and_developer_detail_is_collapsed(self):
        text = rn.render(["Organize your files"], ["tidy CI"], ["test(autotag): reset sockets"])
        visible, collapsed = text.split("<details>")
        self.assertIn("## What's new", visible)
        self.assertIn("- Organize your files", visible)
        self.assertNotIn("tidy CI", visible)
        self.assertNotIn("test(autotag)", visible)
        self.assertIn("<summary>", collapsed)
        self.assertIn("tidy CI", collapsed)
        self.assertIn("test(autotag): reset sockets", collapsed)
        self.assertTrue(text.rstrip().endswith("</details>"))

    def test_no_developer_block_when_there_is_nothing_for_developers(self):
        text = rn.render(["Organize your files"], [], [])
        self.assertNotIn("<details>", text)
        self.assertIn("- Organize your files", text)

    def test_a_release_without_user_facing_changes_says_so_before_the_detail(self):
        text = rn.render([], ["tidy CI"], [])
        visible = text.split("<details>")[0]
        self.assertIn("No user-facing changes.", visible)
        self.assertNotIn("tidy CI", visible)

    def test_nothing_at_all_still_says_no_user_facing_changes(self):
        self.assertIn("No user-facing changes.", rn.render([], [], []))


if __name__ == "__main__":
    unittest.main()
