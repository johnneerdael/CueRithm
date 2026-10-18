#!/usr/bin/env python3
"""Verify the original synthetic PCM recipes; optionally write research inputs."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct


def samples(recipe, rate):
    pcm = bytearray(recipe["frames"] * 4)
    beat = 0
    while True:
        start = math.floor(recipe["offset_frames"] + beat * 60 * rate / recipe["bpm"] + 0.5)
        if start >= recipe["frames"]:
            break
        gain = 1 / 16 if start < recipe["quiet_until_frames"] else 1
        for index in range(min(4096, recipe["frames"] - start)):
            sign = 1 if index % 512 < 256 else -1
            struct.pack_into("<f", pcm, 4 * (start + index), (4096 - index) / 4096 * sign * gain)
        if recipe["offbeat_hats"]:
            hat = start + math.floor(30 * rate / recipe["bpm"] + 0.5)
            for index in range(min(256, recipe["frames"] - hat)):
                offset = 4 * (hat + index)
                previous = struct.unpack_from("<f", pcm, offset)[0]
                sign = 1 if index % 2 else -1
                struct.pack_into("<f", pcm, offset, previous + (256 - index) / 2048 * sign)
        beat += 1
    return pcm


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="Write PCM to a new private research directory")
    args = parser.parse_args()
    if args.output:
        args.output.mkdir(parents=True, exist_ok=False)
    reference = json.loads(Path(__file__).with_name("reference.json").read_text())
    for case in reference["cases"]:
        pcm = samples(case["recipe"], reference["source"]["sample_rate"])
        actual = hashlib.sha256(pcm).hexdigest()
        if actual != case["pcm_sha256"]:
            raise ValueError(f"PCM hash mismatch: {case['name']}: {actual}")
        if args.output:
            (args.output / f"{case['name']}.f32").write_bytes(pcm)
        print(f"Verified {case['name']}")


if __name__ == "__main__":
    main()
