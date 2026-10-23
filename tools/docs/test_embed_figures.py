"""Tests for figure placement. Run: python3 -m unittest discover -s tools/docs"""
import unittest

import embed_figures

PAGE = ["# Title", "", '<p class="lead">Lead.</p>', "", "## Step { #step }", "", "Paragraph.", "", "## Next"]
BLOCK = ["", "<!-- figure:{n} -->", "<figure markdown>", "</figure>", "<!-- /figure -->", ""]


def place(lines, name, after):
    at = embed_figures.skip_placed(lines, embed_figures.insertion_point(lines, after))
    lines[at:at] = [line.replace("{n}", name) for line in BLOCK]


class Placement(unittest.TestCase):
    def test_figure_goes_after_the_heading_paragraph(self):
        lines = list(PAGE)
        place(lines, "one", "## Step { #step }")
        self.assertLess(lines.index("Paragraph."), lines.index("<!-- figure:one -->"))
        self.assertLess(lines.index("<!-- figure:one -->"), lines.index("## Next"))

    def test_figures_sharing_a_heading_keep_manifest_order(self):
        lines = list(PAGE)
        for name in ("one", "two", "three"):
            place(lines, name, "## Step { #step }")
        order = [lines.index(f"<!-- figure:{name} -->") for name in ("one", "two", "three")]
        self.assertEqual(order, sorted(order))
        self.assertLess(order[-1], lines.index("## Next"))

    def test_intro_figures_follow_the_lead(self):
        lines = list(PAGE)
        place(lines, "a", "intro")
        place(lines, "b", "intro")
        self.assertLess(lines.index('<p class="lead">Lead.</p>'), lines.index("<!-- figure:a -->"))
        self.assertLess(lines.index("<!-- figure:a -->"), lines.index("<!-- figure:b -->"))


if __name__ == "__main__":
    unittest.main()
