#!/usr/bin/env python3
"""Where does Mixxx's decoder put a click, relative to CueRithm's FFmpeg clock? (macOS)

Mixxx on macOS decodes MP3, AAC, M4A and MP4 through Apple CoreAudio. This script makes
a click track in each format and tag variant, asks `coreaudio_probe` (which makes the
calls Mixxx makes) for the Mixxx frame of the first click, decodes the same file with
FFmpeg the way CueRithm does, and prints the difference. That difference is what the
Mixxx adapter adds to a CueRithm position.

Needs macOS with clang, afconvert, ffmpeg and lame on the PATH. Standard library only.

    python3 tools/mixxx-acceptance/decoder_table.py [--work DIR]

The table in docs/research/mixxx-sync-contract.md was produced with this script.
"""
import argparse
import array
import math
import struct
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

SAMPLE_RATE = 44100
CLICK_AT = SAMPLE_RATE  # one second in
HERE = Path(__file__).resolve().parent


def run(*args):
    return subprocess.run(args, check=True, capture_output=True)


def write_click_track(path):
    """Stereo 16-bit click: two cycles of a 2 kHz tone at near full scale, one second in.
    (A burst at the Nyquist rate would be removed by the encoder's lowpass.)"""
    frames = bytearray()
    burst = round(2 * SAMPLE_RATE / 2000)
    for index in range(SAMPLE_RATE * 3):
        value = 0
        if CLICK_AT <= index < CLICK_AT + burst:
            value = round(32000 * math.sin(2 * math.pi * 2000 * (index - CLICK_AT) / SAMPLE_RATE))
        frames += struct.pack("<hh", value, value)
    with wave.open(str(path), "wb") as out:
        out.setnchannels(2)
        out.setsampwidth(2)
        out.setframerate(SAMPLE_RATE)
        out.writeframes(bytes(frames))


def ffmpeg_onset(path):
    """CueRithm's clock: ffmpeg decode, stereo 44.1 kHz, first sample above 0.3 full scale
    (the probe's threshold, so a quiet mono downmix is still found)."""
    data = run("ffmpeg", "-v", "error", "-i", str(path), "-f", "s16le", "-ac", "2",
               "-ar", str(SAMPLE_RATE), "-").stdout
    samples = array.array("h")
    samples.frombytes(data)
    for index, value in enumerate(samples[0::2]):
        if abs(value) > 9830:
            return index
    return None


def patch_identifier(source, target, identifier):
    """Copy an MP3 with its LAME tag's four-byte encoder identifier replaced."""
    data = bytearray(source.read_bytes())
    at = data.find(b"Info") + 120
    assert at >= 120, "no Info frame to patch"
    data[at:at + 4] = identifier
    target.write_bytes(data)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", help="folder for generated files (default: a temporary one)")
    args = parser.parse_args()
    if sys.platform != "darwin":
        sys.exit("coreaudio_probe needs macOS; Linux and Windows decode MP3 with MAD (see the research notes)")
    work = Path(args.work) if args.work else Path(tempfile.mkdtemp(prefix="mixxx-decoders-"))
    work.mkdir(parents=True, exist_ok=True)
    probe = work / "coreaudio_probe"
    run("clang", "-framework", "AudioToolbox", "-framework", "CoreFoundation",
        str(HERE / "coreaudio_probe.c"), "-o", str(probe))

    wav = work / "click.wav"
    write_click_track(wav)
    # MP3 tag variants. D is plain `lame`; A has no Info frame at all.
    run("lame", "--quiet", "-b", "128", str(wav), str(work / "D.mp3"))
    run("lame", "--quiet", "-t", "-b", "128", str(wav), str(work / "A.mp3"))
    run("lame", "--quiet", "-V2", "--noreplaygain", str(wav), str(work / "lame_vbr.mp3"))
    run("lame", "--quiet", "-m", "m", "-b", "96", str(wav), str(work / "lame_mono.mp3"))
    run("ffmpeg", "-v", "error", "-y", "-i", str(wav), "-c:a", "libmp3lame", "-b:a", "128k", str(work / "ffmpeg_lame.mp3"))
    d = work / "D.mp3"
    patch_identifier(d, work / "id_replaced.mp3", b"XXXX")
    for name, identifier in [("GOGO", b"GOGO"), ("Bolt", b"Bolt"), ("Lavf", b"Lavf"), ("Lavc", b"Lavc")]:
        patch_identifier(d, work / f"id_{name}.mp3", identifier)
    # Break the tag CRC (two bytes at frame offset 190) but keep the LAME identifier.
    crc = bytearray(d.read_bytes())
    crc[crc.find(b"\xff\xfb") + 190] ^= 0xFF
    (work / "crc_broken.mp3").write_bytes(crc)
    # Other containers.
    run("afconvert", "-f", "m4af", "-d", "aac", str(wav), str(work / "afconvert_aac.m4a"))
    run("afconvert", "-f", "m4af", "-d", "alac", str(wav), str(work / "afconvert_alac.m4a"))
    run("afconvert", "-f", "adts", "-d", "aac", str(wav), str(work / "afconvert.aac"))
    run("ffmpeg", "-v", "error", "-y", "-i", str(wav), "-c:a", "aac", "-b:a", "192k", str(work / "ffmpeg_aac.m4a"))
    run("ffmpeg", "-v", "error", "-y", "-i", str(wav), "-c:a", "flac", str(work / "click.flac"))

    rows = [
        ("D.mp3", "lame CBR, LAME tag"),
        ("lame_vbr.mp3", "lame -V2, Xing + LAME tag"),
        ("lame_mono.mp3", "lame mono"),
        ("ffmpeg_lame.mp3", "FFmpeg libmp3lame"),
        ("crc_broken.mp3", "LAME tag, CRC broken"),
        ("A.mp3", "no Info frame (lame -t)"),
        ("id_replaced.mp3", "Info frame, identifier XXXX"),
        ("id_GOGO.mp3", "identifier GOGO"),
        ("id_Bolt.mp3", "identifier Bolt"),
        ("id_Lavf.mp3", "identifier Lavf (patched)"),
        ("id_Lavc.mp3", "identifier Lavc (patched)"),
        ("afconvert_aac.m4a", "AAC in M4A (Apple encoder)"),
        ("afconvert_alac.m4a", "ALAC in M4A"),
        ("afconvert.aac", "AAC, ADTS"),
        ("ffmpeg_aac.m4a", "AAC in M4A (FFmpeg encoder)"),
        ("click.flac", "FLAC"),
        ("click.wav", "WAV"),
    ]
    print(f"{'file':20} {'what':30} {'priming':>7} {'ffmpeg':>7} {'mixxx':>7} {'add to CueRithm':>16}")
    for name, what in rows:
        path = work / name
        line = dict(item.split("=") for item in run(str(probe), str(path)).stdout.decode().split())
        ffmpeg = ffmpeg_onset(path)
        mixxx = int(line["mixxx_frame"])
        shown = "None" if ffmpeg is None else f"{mixxx - ffmpeg:+d}"
        print(f"{name:20} {what:30} {line['table_priming']:>7} {ffmpeg!s:>7} {mixxx:>7} {shown:>16}")
    print(f"\nfiles are in {work}")


if __name__ == "__main__":
    main()
