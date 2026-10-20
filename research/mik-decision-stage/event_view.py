"""Band view of a track on its beat grid, with the Mixed In Key cues and detected structural events overlaid.

    python event_view.py --pairs DIR --audio DIR --out OUT.png --ids a,b,c      # contact sheet of the chosen pairs

The view mirrors what a DJ reads in rekordbox: one column per beat, the low band (kick and bass, red), the mid band (green)
and the high band (blue) stacked, thin lines every 16 beats and strong lines every 128. Black markers are the cues the
service returned, coloured markers are the events found by ``events.detect``. The grid (tempo, downbeat) comes from the
captured request, so nothing here uses a tag or a file name for a decision. Output stays private: it shows music structure.
"""
import argparse
import json
import os
import re
import subprocess
import unicodedata

import numpy as np

import events as ev
import mikdata as md

RATE = 22050
N_FFT, HOP = 2048, 512
BANDS = {'low': (20.0, 200.0), 'mid': (200.0, 2500.0), 'high': (2500.0, 10000.0)}


def decode(path):
    """Mono 22.05 kHz float32 through ffmpeg."""
    proc = subprocess.run(['ffmpeg', '-v', 'error', '-i', path, '-ac', '1', '-ar', str(RATE), '-f', 'f32le', '-'], capture_output=True, check=True)
    return np.frombuffer(proc.stdout, np.float32)


def band_power(signal):
    """Per-frame power in the three bands (low, mid, high), frames of HOP samples."""
    window = np.hanning(N_FFT).astype(np.float32)
    n = 1 + max(0, (len(signal) - N_FFT) // HOP)
    frames = np.lib.stride_tricks.as_strided(signal, (n, N_FFT), (signal.strides[0] * HOP, signal.strides[0]))
    spectrum = np.abs(np.fft.rfft(frames * window, axis=1)) ** 2
    freqs = np.fft.rfftfreq(N_FFT, 1.0 / RATE)
    return np.stack([spectrum[:, (freqs >= lo) & (freqs < hi)].sum(1) for lo, hi in BANDS.values()], 1)


def per_beat(power, tempo, downbeat, beats):
    """Mean band power per beat (beat k spans downbeat + k*60/tempo)."""
    period = 60.0 / tempo
    frame_time = (np.arange(len(power)) * HOP + N_FFT / 2) / RATE
    index = np.floor((frame_time - downbeat) / period).astype(int)
    out = np.zeros((beats, 3))
    count = np.zeros(beats)
    ok = (index >= 0) & (index < beats)
    np.add.at(out, index[ok], power[ok])
    np.add.at(count, index[ok], 1)
    return out / np.maximum(count, 1)[:, None]


def track_curves(request, audio_path):
    tempo, downbeat = request['Tempo'], request['DownbeatTime']
    duration = request['DurationInSeconds']
    beats = int((duration - downbeat) / (60.0 / tempo))
    curves = per_beat(band_power(decode(audio_path)), tempo, downbeat, beats)
    return np.sqrt(curves)  # amplitude-like, less dominated by the loudest peaks


def _key(name):
    """File name without extension, NFC, and without leading zeros of the leading track number ('058.' and '58.' match)."""
    stem = unicodedata.normalize('NFC', md.strip_audio_extension(name))  # names such as '58. Artist - Title' hold dots of their own
    return re.sub(r'^0+(\d)', r'\1', stem)


def find_audio(audio_dir, name):
    want = _key(name)
    for f in os.listdir(audio_dir):
        if not f.startswith('._') and _key(f) == want:
            return os.path.join(audio_dir, f)
    return None


def draw(ax, curves, cues, found, title):
    import matplotlib.pyplot as plt  # noqa: F401
    n = len(curves)
    scale = np.percentile(curves, 97, axis=0) + 1e-12
    shown = np.clip(curves / scale, 0, 1.15)
    x = np.arange(n)
    low, mid, high = shown[:, 0], shown[:, 1], shown[:, 2]
    ax.fill_between(x, 0, high, color='#3b82f6', alpha=0.85, linewidth=0, step='mid')
    ax.fill_between(x, 0, mid, color='#22c55e', alpha=0.75, linewidth=0, step='mid')
    ax.fill_between(x, 0, low, color='#ef4444', alpha=0.85, linewidth=0, step='mid')
    for b in range(0, n, 16):
        ax.axvline(b, color='k', alpha=0.28 if b % 128 == 0 else 0.07, linewidth=0.8 if b % 128 == 0 else 0.5)
    for b in cues:
        ax.plot([b, b], [0, 1.3], color='black', linewidth=1.2)
        ax.plot([b], [1.3], marker='v', color='black', markersize=4)
    colours = {'breakdown': '#f59e0b', 'buildup': '#a855f7', 'drop': '#111827', 'intro': '#0ea5e9', 'outro': '#64748b'}
    for kind, b in found:
        ax.plot([b], [-0.12], marker='^', color=colours.get(kind, 'k'), markersize=6, markeredgecolor='white', markeredgewidth=0.4)
        ax.text(b, -0.3, {'breakdown': 'BR', 'buildup': 'BU', 'drop': 'D', 'intro': 'IN', 'outro': 'OUT'}.get(kind, kind), color=colours.get(kind, 'k'), fontsize=5.5, ha='center')
    ax.set_xlim(0, n)
    ax.set_ylim(-0.4, 1.4)
    ax.set_yticks([])
    ax.set_title(title, fontsize=7, loc='left', pad=2)
    ax.tick_params(labelsize=6)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--pairs', required=True)
    ap.add_argument('--audio', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--ids', required=True, help='comma separated pair ids (file stems before .request.json)')
    a = ap.parse_args()
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    ids = [i for i in a.ids.split(',') if i]
    fig, axes = plt.subplots(len(ids), 1, figsize=(15, 1.55 * len(ids)), squeeze=False)
    for k, pid in enumerate(ids):
        request = json.load(open(os.path.join(a.pairs, pid + '.request.json')))
        response = json.load(open(os.path.join(a.pairs, pid + '.response.json')))
        path = find_audio(a.audio, request['EliteData']['FileName'])
        ax = axes[k][0]
        if not path:
            ax.set_title(f'{k + 1}: audio not found', fontsize=7, loc='left')
            continue
        curves = track_curves(request, path)
        period = 60.0 / request['Tempo']
        cues = [int(round((c['Time'] - request['DownbeatTime']) / period)) for c in response['CuePoints']]
        found = ev.detect(curves)
        draw(ax, curves, cues, found, f'{k + 1}  {request["Tempo"]:.1f} BPM  {len(curves)} beats  server cues {len(cues)}')
    fig.tight_layout(h_pad=0.4)
    fig.savefig(a.out, dpi=110)
    print('written', a.out)


if __name__ == '__main__':
    main()
