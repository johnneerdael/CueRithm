"""Validate our normalized synthetic fixture contract, not a rekordbox schema.

The future native writer must extract and validate real database/ANLZ records
before these invariants can establish target compatibility. No writes occur here.
"""
import math


def validate(manifest):
    errors = []
    if (manifest.get("manifest_revision"), manifest.get("schema")) != (
        1, "synthetic-rb-contract-v1"
    ):
        return ["unsupported_revision"]
    external_ids = set()
    library_ids = set()
    for track in manifest["tracks"]:
        external_id = track["external_id"]
        if external_id in external_ids:
            errors.append("duplicate_external_id")
        external_ids.add(external_id)
        if track["library_id"] in library_ids:
            errors.append("duplicate_library_id")
        library_ids.add(track["library_id"])
        duration = track["duration_seconds"]
        if not math.isfinite(duration) or duration <= 0:
            errors.append("invalid_duration")
        asset = track["analysis_asset"]
        if asset not in manifest["assets"]:
            errors.append("missing_analysis_asset")
        elif manifest["assets"][asset] != track["pcm_identity"]:
            errors.append("asset_timeline_mismatch")
        if any(not math.isfinite(t) or not 0 <= t <= duration for t in track["cues"]):
            errors.append("invalid_cue_time")
        beats = track["beats"]
        if (not beats or any(not math.isfinite(t) or t > duration for t in beats)
                or any(b <= a for a, b in zip(beats, beats[1:]))):
            errors.append("invalid_beatgrid")
        # Negative leading beat anchors are valid; cue times are playable times.
    return errors
