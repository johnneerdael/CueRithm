"""Tests for the path detector. Run: python3 -m unittest discover -s tools/docs"""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "capture"))
import redact_paths  # noqa: E402


class PathLines(unittest.TestCase):
    def test_a_rooted_system_folder_is_a_path(self):
        self.assertTrue(redact_paths.is_path("/private/tmp/demo/Music/Beatport"))
        self.assertTrue(redact_paths.is_path("Remove watch folder /Users/someone/Music"))

    def test_a_home_shortcut_and_a_drive_letter_are_paths(self):
        self.assertTrue(redact_paths.is_path("~/Music/Beatport"))
        self.assertTrue(redact_paths.is_path("Saved to C:\\Users\\dj\\Music"))

    def test_an_audio_file_name_is_a_path_even_without_its_folder(self):
        self.assertTrue(redact_paths.is_path("Mix).flac"))
        self.assertTrue(redact_paths.is_path("Moonphazes, SLVR/SOLANCE - Knockout (Extended Mix).tlac"))  # a misread extension
        self.assertTrue(redact_paths.is_path("SLVR/Massano, Humans Musik - Underground (Extended Mix).flac"))

    def test_a_wrapped_release_folder_line_is_a_path(self):
        self.assertTrue(redact_paths.is_path("Massano, Humans Musik, Final Request, SLVR/Massano, Humans Musik - Underground (Extended"))

    def test_ordinary_text_and_titles_are_not(self):
        for text in ("Folder preparation presets", "Energy 7/10", "Anyma (ofc)", "4:31", "Warm-up / Peak", "Keys / 12A", "FLAC", "Export .cuerithm", "Energy 7/10", "CueRithm 0.1.19", "Anyma (ofc)"):
            self.assertFalse(redact_paths.is_path(text), text)


if __name__ == "__main__":
    unittest.main()
