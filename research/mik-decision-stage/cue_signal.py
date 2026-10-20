"""The conditioned cue signal: the per-beat scores of the request after the client's ramp and band-pass.

The client (DetectLikelyCuePointsV3) multiplies the raw ``CalculateScore`` values by a quadratic ramp, then forms
``S = H1 + 0.5 * (H1 - H3) - H32`` with ``H_w`` the Hann-weighted sum of width ``w``. The request carries the raw scores, so the
same conditioning is applied here before anything is picked from them.
"""
import numpy as np


def hanning_sum(values, width):
    """Hann-weighted sum of the given width around every index (edges clipped), the client's ``HanningSum`` for all indices at once."""
    half = int((width - 1.0) * 0.5)
    n = len(values)
    out = np.zeros(n)
    for k in range(-half, half + 1):
        t = k / (width + 1.0) + 0.5
        if not 0.0 < t < 1.0:
            continue
        weight = 0.5 - 0.5 * np.cos(2.0 * np.pi * t)
        lo, hi = max(0, -k), min(n, n - k)
        out[lo:hi] += weight * values[lo + k:hi + k]
    return out / ((width + 1.0) * 0.5)


def ramp(n, start):
    """Weight per index: zero before start + 32, rising quadratically over 32 beats, and the mirror at the rounded end."""
    span = n - start
    shifted = span - 64 if span - 64 >= 0 else span - 33
    end = (shifted & ~31) + start
    i = np.arange(n)
    p = i - start + 32
    left = np.abs(p - 64)
    right = np.abs(p - (shifted & ~31))
    d = np.minimum(left, right).astype(float)
    weight = np.minimum(1.0, (d / 32.0) ** 2)
    weight[(i < start + 32) | (i > end - 32)] = 0.0
    weight[:start] = 0.0
    return weight, end


def conditioned(scores, start):
    """Return the band-passed signal ``S`` (index space of the request's score array) and the rounded end index."""
    z = np.asarray(scores, float)
    weight, end = ramp(len(z), start)
    v = z * weight
    h1, h3, h32 = hanning_sum(v, 1.0), hanning_sum(v, 3.0), hanning_sum(v, 32.0)
    return h1 + (h1 - h3) * 0.5 - h32, end
