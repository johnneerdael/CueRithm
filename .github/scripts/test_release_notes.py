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

    def test_merge_commit_subjects_resolve_the_pr(self):
        public, _, _ = rn.entries([("1", "Merge pull request #7 from x/y")], {7: "## Release notes\n- Z"})
        self.assertEqual(public, ["Z"])


if __name__ == "__main__":
    unittest.main()
