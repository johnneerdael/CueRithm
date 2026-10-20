"""Structural events on the beat grid from per-beat low/mid/high band amplitudes (first version, to be calibrated by eye).

``detect(curves)`` takes an (n_beats, 3) array of band amplitudes (low, mid, high) and returns ``[(kind, beat), ...]`` with

* ``intro``      first beat where the groove (kick and bass band) is established
* ``breakdown``  the groove leaves for at least ``MIN_BREAK`` beats after it was on
* ``buildup``    the lowest point of mid/high activity before a drop, from which the activity rises into it
* ``drop``       the groove returns for at least ``MIN_ON`` beats after a breakdown
* ``outro``      the groove leaves for good (the region used to mix the next track in)

Positions are snapped to the phrase grid (16 beats from the downbeat) when a phrase boundary is within ``SNAP`` beats and
carries the largest change.
"""
import numpy as np

SMOOTH = 4            # beats averaged: one bar
ON, OFF = 0.62, 0.42  # hysteresis on the low band relative to its own recent level
REFERENCE = 128       # beats looked back for that recent level (its 90th percentile)
MIN_BREAK = 16        # shortest groove-off run that counts as a breakdown (beats)
MIN_ON = 32           # shortest groove-on run that counts as established (beats)
SNAP = 8              # beats searched around a boundary for the phrase line with the largest change


def smooth(x, width):
    kernel = np.ones(width) / width
    padded = np.pad(x, (width // 2, width - 1 - width // 2), mode='edge')
    return np.convolve(padded, kernel, mode='valid')


def runs(state):
    """(value, start, end) for consecutive equal values of a boolean array."""
    out, start = [], 0
    for i in range(1, len(state) + 1):
        if i == len(state) or state[i] != state[start]:
            out.append((bool(state[start]), start, i))
            start = i
    return out


def recent_level(x, window=REFERENCE, q=90):
    """Rolling high percentile of the previous ``window`` beats (including the current one), at least 32 beats of history."""
    padded = np.concatenate([np.full(window - 1, np.nan), x])
    view = np.lib.stride_tricks.sliding_window_view(padded, window)
    ref = np.nanpercentile(view, q, axis=1)
    return np.maximum(ref, 1e-9)


def hysteresis(level, on, off):
    state = np.zeros(len(level), bool)
    current = bool(level[0] > on)
    for i, v in enumerate(level):
        if current and v < off:
            current = False
        elif not current and v > on:
            current = True
        state[i] = current
    return state


def snap(beat, level, direction, width=SNAP, grid=16):
    """Move a boundary to the phrase line within ``width`` beats where the low band changes most in the given direction."""
    best, best_change = beat, -1e9
    for b in range(max(grid, beat - width), beat + width + 1):
        if b % grid:
            continue
        before = level[max(0, b - 8):b].mean() if b > 0 else 0.0
        after = level[b:b + 8].mean() if b < len(level) else 0.0
        change = (before - after) if direction < 0 else (after - before)
        if change > best_change:
            best, best_change = b, change
    return best


def slope_start(cross, relative, low, back=48, top=0.9, grid=8):
    """A breakdown is a descent: place it where the low band leaves its usual level, not where it finally crosses the threshold.

    Walk back from the crossing to the last beat that was still at ``top`` of the recent level (within ``back`` beats), then take
    the nearest 8-beat line, preferring the one with the larger change in the low band around it.
    """
    t = cross
    while t > max(1, cross - back) and relative[t - 1] < top:
        t -= 1
    best, best_change = None, -1e9
    for b in range(max(grid, ((t - grid) // grid) * grid), cross + grid + 1, grid):
        before = low[max(0, b - 8):b].mean() if b > 0 else 0.0
        after = low[b:b + 8].mean()
        change = before - after - 0.02 * abs(b - t)       # the change at the line, nearer the start of the descent preferred
        if change > best_change:
            best, best_change = b, change
    return best if best is not None else cross


def detect(curves):
    curves = np.asarray(curves, float)
    n = len(curves)
    scale = np.percentile(curves, 90, axis=0) + 1e-12
    norm = curves / scale
    low = smooth(norm[:, 0], SMOOTH)
    activity = smooth(0.5 * (norm[:, 1] + norm[:, 2]), SMOOTH)
    # the groove is "on" when the low band is close to what it has recently been, and never when it is far below the track's norm
    relative = np.minimum(low / recent_level(low), low / 0.8)
    state = hysteresis(relative, ON, OFF)
    segments = runs(state)
    # drop runs that are too short to matter: groove-on shorter than MIN_ON between two off runs is a stab, off shorter than MIN_BREAK a gap
    cleaned = []
    for value, s, e in segments:
        if cleaned and cleaned[-1][0] == value:
            cleaned[-1] = (value, cleaned[-1][1], e)
        elif (value and e - s < MIN_ON and cleaned) or (not value and e - s < MIN_BREAK and cleaned):
            cleaned[-1] = (cleaned[-1][0], cleaned[-1][1], e)
        else:
            cleaned.append((value, s, e))
    found = []
    first_on = next(((s) for v, s, e in cleaned if v), None)
    if first_on is not None:
        found.append(('intro', first_on if first_on == 0 else snap(first_on, low, +1)))
    for k, (value, s, e) in enumerate(cleaned):
        if value or s == 0 or k == 0:
            continue
        if e >= n - 1 or not any(v for v, _, _ in cleaned[k + 1:]):
            # the last groove-off run reaching the end of the track is the outro
            found.append(('outro', snap(s, low, -1)))
            continue
        found.append(('breakdown', slope_start(s, relative, low)))
        drop = snap(e, low, +1)
        found.append(('drop', drop))
        lo, hi = max(s + 8, drop - 64), drop - 8
        if hi > lo:
            window = activity[lo:hi]
            at = lo + int(np.argmin(window))
            if activity[max(at, drop - 4)] > 1.3 * activity[at] + 0.02:
                found.append(('buildup', (at // 8) * 8))
    return sorted(found, key=lambda kv: kv[1])
