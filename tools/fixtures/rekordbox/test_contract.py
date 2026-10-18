"""Original synthetic direct-sync manifests; never opens a DJ database."""
import copy
import unittest

from contract import validate


def fixture():
    return {
        "manifest_revision": 1,
        "schema": "synthetic-rb-contract-v1",
        "target_database": "test-db",
        "assets": {"ANLZ0000.DAT": "test-pcm"},
        "tracks": [{
            "library_id": "track-a", "external_id": "12",
            "pcm_identity": "test-pcm", "duration_seconds": 64.0,
            "analysis_asset": "ANLZ0000.DAT",
            "cues": [0.0, 32.0], "beats": [0.0, 0.5, 1.0],
        }],
    }


class ContractTests(unittest.TestCase):
    def test_consistent_manifest(self):
        self.assertEqual(validate(fixture()), [])

    def test_duplicate_target_ids(self):
        data = fixture()
        other = copy.deepcopy(data["tracks"][0])
        other["library_id"] = "track-b"
        data["tracks"].append(other)
        self.assertIn("duplicate_external_id", validate(data))

    def test_missing_analysis_asset(self):
        data = fixture()
        data["assets"].clear()
        self.assertIn("missing_analysis_asset", validate(data))

    def test_unknown_revisions_fail_closed(self):
        for field, value in [("schema", "unseen-rb-schema"), ("manifest_revision", 2)]:
            data = fixture()
            data[field] = value
            self.assertIn("unsupported_revision", validate(data))

    def test_cue_grid_and_asset_share_playable_timeline(self):
        for field, value, expected in [
            ("cues", [65.0], "invalid_cue_time"),
            ("beats", [1.0, 0.5], "invalid_beatgrid"),
            ("duration_seconds", float("nan"), "invalid_duration"),
        ]:
            data = fixture()
            data["tracks"][0][field] = value
            self.assertIn(expected, validate(data))
        data = fixture()
        data["assets"]["ANLZ0000.DAT"] = "different-pcm"
        self.assertIn("asset_timeline_mismatch", validate(data))


if __name__ == "__main__":
    unittest.main()
