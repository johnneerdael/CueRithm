"""Tests for the external link rewriter. Run: python3 -m unittest discover -s tools/docs"""
import unittest

import update_external_links as links


class NewTarget(unittest.TestCase):
    def setUp(self):
        self.saved = dict(links.REDIRECTS)
        links.REDIRECTS.clear()
        links.REDIRECTS.update({
            "technical/key.md": "how-it-works/key.md",
            "guides/sync.md": {"to": "sync/index.md", "anchors": {"pioneer-usb": "sync/pioneer-usb.md", "choose-a-destination": "sync/index.md#which-software"}},
        })

    def tearDown(self):
        links.REDIRECTS.clear()
        links.REDIRECTS.update(self.saved)

    def test_whole_page_move_keeps_the_anchor(self):
        self.assertEqual(links.new_target("technical/key.md", "the-model"), ("how-it-works/key.md", "the-model"))

    def test_mapped_anchor_goes_to_its_page(self):
        self.assertEqual(links.new_target("guides/sync.md", "pioneer-usb"), ("sync/pioneer-usb.md", ""))
        self.assertEqual(links.new_target("guides/sync.md", "choose-a-destination"), ("sync/index.md", "which-software"))

    def test_unmapped_anchor_is_carried_over(self):
        self.assertEqual(links.new_target("guides/sync.md", "options"), ("sync/index.md", "options"))

    def test_unknown_page_is_left_alone(self):
        self.assertIsNone(links.new_target("index.md", ""))


if __name__ == "__main__":
    unittest.main()
