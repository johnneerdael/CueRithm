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
    variant = recipe.get("variant", "")
    if variant.startswith("intro-"):
        for index in range(recipe["frames"]):
            value = (
                0.5 if index >= recipe["quiet_until_frames"]
                else 1 / 16 if variant == "intro-atmospheric"
                else (index // 4096) ** 2 / 65536 if variant == "intro-buildup"
                else 0
            )
            struct.pack_into("<f", pcm, index * 4, value)
        return pcm
    if variant == "quiet-bed":
        pcm[:] = struct.pack("<f", 1 / 256) * recipe["frames"]
    if variant == "soft-riser":
        count = min(4 * rate, recipe["frames"])
        pcm[:count * 4] = struct.pack("<f", 1 / 32) * count
    if variant == "late-bed":
        count = min(recipe["offset_frames"], recipe["frames"])
        pcm[:count * 4] = struct.pack("<f", 1 / 256) * count
    beat = 0
    while True:
        start = math.floor(recipe["offset_frames"] + beat * 60 * rate / recipe["bpm"] + 0.5)
        if start >= recipe["frames"]:
            break
        if variant == "soft-riser" and 2 <= beat <= 8:
            beat += 1
            continue
        gain = 1 / 128 if variant == "quiet-bed" else 1 / 16 if start < recipe["quiet_until_frames"] else 1
        for index in range(min(4096, recipe["frames"] - start)):
            sign = 1 if index % 512 < 256 else -1
            struct.pack_into("<f", pcm, 4 * (start + index), (4096 - index) / 4096 * sign * gain)
        if recipe["offbeat_hats"] and (variant != "sparse-half" or beat % 3 == 0):
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
    parser.add_argument("--reference", type=Path, default=Path(__file__).with_name("reference.json"))
    args = parser.parse_args()
    if args.output:
        args.output.mkdir(parents=True, exist_ok=False)
    reference = json.loads(args.reference.read_text())
    for case in reference["cases"]:
        pcm = samples(case["recipe"], case.get("sample_rate", reference["source"]["sample_rate"]))
        actual = hashlib.sha256(pcm).hexdigest()
        if actual != case["pcm_sha256"]:
            raise ValueError(f"PCM hash mismatch: {case['name']}: {actual}")
        if args.output:
            (args.output / f"{case['name']}.f32").write_bytes(pcm)
        print(f"Verified {case['name']}")


if __name__ == "__main__":
    main()
