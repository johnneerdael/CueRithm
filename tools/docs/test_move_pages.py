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


if __name__ == "__main__":
    unittest.main()
