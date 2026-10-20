"""Key: two models of the server's decision from the 352 request features, both evaluated out of fold on the tuning split.

    python key_models.py --cache cache.npz --model svc|cnn [--seeds 5] [--limit 200]

Layout of the features (checked on every capture): four banks of 88 piano-bin values, bin j belongs to pitch class (j + 9) % 12. Both
models are built so that shifting the music by a semitone shifts the answer by a semitone:

* ``svc``: an RBF SVC (24 Camelot keys) trained with every training song shifted by -6..+5 semitones, labels shifted along. The shift
  augmentation alone moves it from 95.3% to 96.7%.
* ``cnn``: a small network scores each of the 12 possible tonics from the 76 bins that start at that tonic (two convolutions along the
  pitch axis, then dense layers, two outputs: minor and major), with weights shared across tonics; the 24 scores are one softmax. Averaging
  five seeds gives about 97.1%.

The server's confidence is not used. Genre, title and artist are not inputs (a controlled test showed they do not change the response).
"""
import argparse
import time

import numpy as np
from sklearn.model_selection import GroupKFold
from sklearn.preprocessing import StandardScaler
from sklearn.svm import SVC

import mikdata as md

BINS, WIDTH = 88, 76


def encode(keys):
    """Camelot code -> class index: mode * 12 + tonic pitch class (minor = 1)."""
    return np.array([md.CAMELOT[k][1] * 12 + md.CAMELOT[k][0] for k in keys])


def decode(index):
    return md.FROM_CAMELOT[(int(index) % 12, int(index) // 12)]


def banks(features):
    return np.sqrt(np.asarray(features, float)).reshape(len(features), 4, BINS)


def shift_bins(z, k):
    """Move every bank up by k semitones (bins), zero fill."""
    out = np.zeros_like(z)
    if k >= 0:
        out[..., k:] = z[..., :BINS - k]
    else:
        out[..., :BINS + k] = z[..., -k:]
    return out


# ---------------------------------------------------------------- SVC with shift augmentation

def fit_svc(features, labels, c=10.0, gamma_scale=0.5, shifts=range(-6, 6)):
    z = banks(features)
    shifts = list(shifts)
    x = np.concatenate([shift_bins(z, k).reshape(len(z), -1) for k in shifts])
    y = np.concatenate([(labels % 12 + k) % 12 + (labels // 12) * 12 for k in shifts])
    scaler = StandardScaler().fit(x)
    model = SVC(C=c, gamma=gamma_scale / x.shape[1], cache_size=2000).fit(scaler.transform(x), y)
    return lambda f: model.predict(scaler.transform(banks(f).reshape(len(f), -1)))


# ---------------------------------------------------------------- pitch-shared network

def candidates(z):
    """(n, 12, 4, 76): for every tonic t the 76 bins starting at the bin of pitch class t in the lowest octave."""
    start = np.array([(t - 9) % 12 for t in range(12)])[:, None] + np.arange(WIDTH)[None]
    return z[:, :, start].transpose(0, 2, 1, 3).astype(np.float32)


def fit_cnn(features, labels, seed=0, epochs=60, channels=16, kernel=5, hidden=256, drop=0.3, lr=2e-3, wd=1e-2):
    import torch
    import torch.nn as nn
    import torch.nn.functional as F
    device = 'mps' if torch.backends.mps.is_available() else 'cpu'
    torch.manual_seed(seed)
    c = candidates(banks(features))
    scale = float(c.std())

    class Scorer(nn.Module):
        def __init__(self):
            super().__init__()
            self.conv = nn.Sequential(nn.Conv1d(4, channels, kernel, padding=kernel // 2), nn.ReLU(),
                                      nn.Conv1d(channels, channels, kernel, padding=kernel // 2), nn.ReLU())
            self.mlp = nn.Sequential(nn.Dropout(drop), nn.Linear(channels * WIDTH, hidden), nn.ReLU(), nn.Dropout(drop),
                                     nn.Linear(hidden, hidden // 2), nn.ReLU(), nn.Linear(hidden // 2, 2))

        def forward(self, x):
            n = x.shape[0]
            z = self.conv(x.reshape(n * 12, 4, WIDTH)).flatten(1)
            return self.mlp(z).reshape(n, 12, 2).permute(0, 2, 1).reshape(n, 24)  # class = mode * 12 + tonic

    net = Scorer().to(device)
    opt = torch.optim.AdamW(net.parameters(), lr=lr, weight_decay=wd)
    xt, yt = torch.tensor(c / scale, device=device), torch.tensor(labels, device=device)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=lr, total_steps=epochs * int(np.ceil(len(xt) / 128)))
    for _ in range(epochs):
        net.train()
        order = torch.randperm(len(xt), device=device)
        for i in range(0, len(xt), 128):
            b = order[i:i + 128]
            loss = F.cross_entropy(net(xt[b]), yt[b])
            opt.zero_grad(); loss.backward(); opt.step(); sched.step()
    net.eval()

    def predict_proba(f):
        with torch.no_grad():
            return F.softmax(net(torch.tensor(candidates(banks(f)) / scale, device=device)), 1).cpu().numpy()
    return predict_proba


def fit_cnn_ensemble(features, labels, seeds=5, **kw):
    members = [fit_cnn(features, labels, seed=s, **kw) for s in range(seeds)]
    return lambda f: np.mean([m(f) for m in members], 0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--cache', required=True)
    ap.add_argument('--model', choices=['svc', 'cnn'], required=True)
    ap.add_argument('--seeds', type=int, default=5)
    ap.add_argument('--limit', type=int)
    a = ap.parse_args()
    d = np.load(a.cache, allow_pickle=True)
    songs = np.where(d['split'] == 'tuning')[0]
    if a.limit:
        songs = songs[:a.limit]
    x, y, h, conf = d['X'][songs], encode(d['key'][songs]), d['h'][songs], d['conf'][songs]
    t0 = time.time()
    pred = np.zeros(len(y), int)
    for tr, te in GroupKFold(min(5, len(songs))).split(x, y, h):
        if a.model == 'svc':
            pred[te] = fit_svc(x[tr], y[tr])(x[te])
        else:
            pred[te] = fit_cnn_ensemble(x[tr], y[tr], a.seeds)(x[te]).argmax(1)
    ok = pred == y
    print(f'{len(songs)} tuning songs, {a.model}: key agreement {ok.mean():.4f} (server confidence below 0.7: {ok[conf < .7].mean():.3f}, '
          f'at least 0.97: {ok[conf >= .97].mean():.3f}; server major keys: {ok[y < 12].mean():.3f} of {int((y < 12).sum())}) ({time.time() - t0:.0f}s)')


if __name__ == '__main__':
    main()
