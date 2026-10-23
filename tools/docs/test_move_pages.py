"""Tests for the page mover's file staging. Run: python3 -m unittest discover -s tools/docs"""
import os
import tempfile
import unittest

import move_pages


def tree(root):
    out = {}
    for folder, _, files in os.walk(root):
        for name in files:
            path = os.path.join(folder, name)
            with open(path, encoding="utf-8") as handle:
                out[os.path.relpath(path, root)] = handle.read()
    return out


class WriteMoves(unittest.TestCase):
    def run_moves(self, pages, mapping):
        with tempfile.TemporaryDirectory() as docs:
            for name, text in pages.items():
                os.makedirs(os.path.dirname(os.path.join(docs, name)) or docs, exist_ok=True)
                with open(os.path.join(docs, name), "w", encoding="utf-8") as handle:
                    handle.write(text)
            outputs = {old: (mapping.get(old, old), text) for old, text in pages.items()}
            move_pages.write_moves(docs, outputs)
            return tree(docs)

    def test_plain_move(self):
        self.assertEqual(self.run_moves({"a.md": "A"}, {"a.md": "x/b.md"}), {"x/b.md": "A"})

    def test_chain_keeps_every_page(self):
        result = self.run_moves({"a.md": "A", "b.md": "B"}, {"a.md": "b.md", "b.md": "c.md"})
        self.assertEqual(result, {"b.md": "A", "c.md": "B"})

    def test_swap_keeps_both_pages(self):
        result = self.run_moves({"a.md": "A", "b.md": "B"}, {"a.md": "b.md", "b.md": "a.md"})
        self.assertEqual(result, {"b.md": "A", "a.md": "B"})

    def test_unmoved_page_is_rewritten_in_place(self):
        self.assertEqual(self.run_moves({"a.md": "A2"}, {}), {"a.md": "A2"})


class AmbiguousMaps(unittest.TestCase):
    def test_chain_and_swap_are_ambiguous(self):
        self.assertEqual(move_pages.ambiguous_moves({"a.md": "b.md", "b.md": "c.md"}), ["a.md"])
        self.assertEqual(move_pages.ambiguous_moves({"a.md": "b.md", "b.md": "a.md"}), ["a.md", "b.md"])

    def test_plain_moves_are_not_ambiguous(self):
        self.assertEqual(move_pages.ambiguous_moves({"a.md": "x/a.md", "b.md": "x/b.md"}), [])

    def test_fix_stale_refuses_an_ambiguous_map(self):
        with self.assertRaises(SystemExit):
            move_pages.fix_stale({"a.md": "b.md", "b.md": "c.md"})


class ExternalRewrites(unittest.TestCase):
    SITE = move_pages.SITE

    def test_chained_urls_are_rewritten_once(self):
        mapping = {"a.md": "b.md", "b.md": "c.md"}
        text = f"[A]({self.SITE}a/) and [B]({self.SITE}b/)"
        self.assertEqual(move_pages.rewrite_urls(text, mapping), f"[A]({self.SITE}b/) and [B]({self.SITE}c/)")

    def test_swapped_urls_both_change(self):
        mapping = {"a.md": "b.md", "b.md": "a.md"}
        text = f"{self.SITE}a/ {self.SITE}b/"
        self.assertEqual(move_pages.rewrite_urls(text, mapping), f"{self.SITE}b/ {self.SITE}a/")

    def test_repo_paths_are_rewritten_once_and_keep_anchors(self):
        mapping = {"a.md": "b.md", "b.md": "c.md"}
        text = "docs/user-guide/a.md#x and ../user-guide/b.md"
        self.assertEqual(move_pages.rewrite_repo_paths(text, mapping), "docs/user-guide/b.md#x and ../user-guide/c.md")

    def test_longer_urls_win_over_prefixes(self):
        mapping = {"x/index.md": "y/index.md", "x/page.md": "z/page.md"}
        text = f"{self.SITE}x/page/ {self.SITE}x/"
        self.assertEqual(move_pages.rewrite_urls(text, mapping), f"{self.SITE}z/page/ {self.SITE}y/")


class NavSkeleton(unittest.TestCase):
    def test_exact_paths_follow_the_move(self):
        text = "  - prepare/index.md\n  - prepare/add-tracks.md\n  - help/faq.md\n"
        mapping = {"prepare/index.md": "files/index.md", "prepare/add-tracks.md": "files/add-tracks.md"}
        self.assertEqual(move_pages.rewrite_nav(text, mapping), "  - files/index.md\n  - files/add-tracks.md\n  - help/faq.md\n")

    def test_a_glob_follows_pages_that_keep_their_names(self):
        mapping = {"tag/recipes/a.md": "tagging/recipes/a.md", "tag/recipes/b.md": "tagging/recipes/b.md"}
        self.assertEqual(move_pages.rewrite_nav("- tag/recipes/*.md", mapping), "- tagging/recipes/*.md")

    def test_a_glob_is_left_alone_when_pages_split_up(self):
        mapping = {"tag/recipes/a.md": "tagging/recipes/a.md", "tag/recipes/b.md": "rename/recipes/b.md"}
        self.assertEqual(move_pages.rewrite_nav("- tag/recipes/*.md", mapping), "- tag/recipes/*.md")

    def test_unrelated_globs_are_untouched(self):
        self.assertEqual(move_pages.rewrite_nav("- playlists/recipes/*.md", {"a.md": "b.md"}), "- playlists/recipes/*.md")


if __name__ == "__main__":
    unittest.main()
