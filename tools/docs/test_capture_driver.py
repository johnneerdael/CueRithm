"""Tests for the capture driver's pure helpers. Run: python3 -m unittest discover -s tools/docs"""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "capture"))
import capture  # noqa: E402


def texts(*values):
    return [{"idx": i, "depth": 4, "text": "text " + v} for i, v in enumerate(values)]


class PlayerTimes(unittest.TestCase):
    def test_remaining_time_after_a_lone_minus_element(self):
        # the layout of CueRithm 0.1.19: elapsed, a minus on its own, the remaining time, then the waveform's labels
        elements = texts("0:01.5", "\u2212", "4:31.9", "BEATGRID", "0:00.0", "0:32.0")
        self.assertEqual(capture.player_times(elements), (1.5, 271.9))

    def test_remaining_time_with_the_minus_inside_the_element(self):
        elements = texts("0:20.0", "\u22124:13.4", "0:08.8", "0:40.8")
        self.assertEqual(capture.player_times(elements), (20.0, 253.4))

    def test_an_ascii_minus_counts_too(self):
        elements = texts("0:00.0", "-4:33.4", "0:00.0", "0:32.0")
        self.assertEqual(capture.player_times(elements), (0.0, 273.4))

    def test_the_waveform_labels_are_never_taken_for_the_remaining_time(self):
        elements = texts("0:08.5", "0:00.0", "0:32.0")
        with self.assertRaises(LookupError):
            capture.player_times(elements)


if __name__ == "__main__":
    unittest.main()
