"""Band structure of a captured song at beat resolution, from its 32-beat energy windows (levels learned by window_bands.py)."""
import numpy as np


def load_model(path):
    z = np.load(path)
    return {b: (z[f'{b}_0'], z[f'{b}_1'], z[f'{b}_2'], float(z[f'{b}_3'])) for b in ('low', 'mid', 'high')}


def window_levels(model, features, volume):
    """(K, 3) log band amplitude per window from the 183 features and the window volume."""
    X = np.column_stack([features, volume])
    return np.column_stack([(((X - mean) / scale) @ coef) + intercept for mean, scale, coef, intercept in (model['low'], model['mid'], model['high'])])


def beat_series(levels, n_beats):
    """Interpolate window levels (window k centred on beat 32k + 16) to one value per beat, as amplitude relative to the song's 90th percentile."""
    centres = 32 * np.arange(len(levels)) + 16
    grid = np.arange(n_beats)
    out = np.column_stack([np.interp(grid, centres, levels[:, j]) for j in range(3)])
    return np.exp(out - np.percentile(out, 90, axis=0))


def candidate_features(series, beat):
    """Columns for candidates at ``beat`` (array): band level at fixed offsets, change across the candidate, and dip depth."""
    n = len(series)
    def at(offset):
        return series[np.clip(beat + offset, 0, n - 1)]
    cols = []
    for offset in (-48, -32, -16, -8, 0, 8, 16, 32, 48, 64):
        cols.append(at(offset))
    before, after = (at(-24) + at(-16) + at(-8)) / 3, (at(8) + at(16) + at(24)) / 3
    cols += [after - before, np.log((after + 1e-3) / (before + 1e-3))]
    far_before, far_after = (at(-48) + at(-40) + at(-32)) / 3, (at(32) + at(40) + at(48)) / 3
    cols += [far_after - far_before]
    low_min_next = np.minimum.reduce([at(o)[:, 0] for o in (8, 16, 24, 32, 40, 48)])
    low_min_prev = np.minimum.reduce([at(-o)[:, 0] for o in (8, 16, 24, 32, 40, 48)])
    return np.column_stack(cols + [low_min_next, low_min_prev])
