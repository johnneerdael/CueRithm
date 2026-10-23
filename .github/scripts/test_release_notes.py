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


SHA = "0123456789abcdef0123456789abcdef01234567"


class Branded(unittest.TestCase):
    def page(self, version="0.1.8", **kwargs):
        return rn.render(["Organize your files"], ["tidy CI"], [], repo="owner/repo", sha=SHA, version=version, **kwargs)

    def test_the_unmodified_release_banner_comes_first_pinned_to_the_commit(self):
        text = self.page()
        url = f"https://raw.githubusercontent.com/owner/repo/{SHA}/docs/assets/brand/release-banner.png"
        self.assertEqual(text.splitlines()[0], f"![CueRithm. Know your music. Shape your set.]({url})")

    def test_the_stage_is_visible_but_source_details_are_collapsed(self):
        visible, collapsed = self.page().split("<details>")
        self.assertIn("CueRithm — Know your music. Shape your set.", visible)
        self.assertIn("**Pre-1.0 release.**", visible)
        self.assertNotIn("Source:", visible)
        self.assertIn(f"Source: [{SHA[:12]}](https://github.com/owner/repo/commit/{SHA}).", collapsed)
        self.assertIn("<summary>Build and source verification</summary>", collapsed)
        self.assertNotIn("**Pre-1.0 release.**", self.page(version="1.0.0"))

    def test_the_source_remains_available_without_internal_changes(self):
        text = rn.render(["**Organize your files.** Keep albums together."], [], [],
                         repo="owner/repo", sha=SHA, version="0.1.8")
        visible, collapsed = text.split("<details>")
        self.assertIn("- **Organize your files.** Keep albums together.", visible)
        self.assertIn(SHA[:12], collapsed)

    def test_the_page_reads_banner_then_changes_then_downloads_then_developer_detail(self):
        text = self.page()
        order = [text.index(part) for part in ["![CueRithm.", "CueRithm —", "**Pre-1.0", "## What's new", "## Downloads", "<details>", "Source:"]]
        self.assertEqual(order, sorted(order))

    def test_downloads_say_what_to_get_for_each_system_and_how_it_is_signed(self):
        text = self.page()
        table = text.split("## Downloads")[1].split("<details>")[0]
        self.assertIn("| Download | Platform | Signing / distribution |", table)
        for needle in ["macOS", "Apple Silicon", "notarized", "no Intel", "Windows", "SmartScreen", "Linux", "DEB"]:
            self.assertIn(needle, table)

    def test_download_links_match_the_published_asset_names(self):
        table = self.page().split("## Downloads")[1].split("<details>")[0]
        for name in ["Cuerithm-arm64-0.1.8.dmg", "Cuerithm-windows-x64-0.1.8.zip",
                     "Cuerithm-linux-x64-0.1.8.deb", "cuerithm-source.tar.gz", "THIRD_PARTY_NOTICES.md"]:
            self.assertIn(f"https://github.com/owner/repo/releases/download/v0.1.8/{name}", table)

    def test_no_banner_or_downloads_without_a_repository(self):
        text = rn.render(["Organize your files"], [], [])
        self.assertNotIn("release-banner.png", text)
        self.assertNotIn("## Downloads", text)


if __name__ == "__main__":
    unittest.main()
