"""Thickened and orogenic continental crust from cell dumps, per seed (mean over 500-1000 Myr
snapshots), mean over seeds and Welch t against the first set.
usage: belts.py BASE_DIR SET_DIR...   (DIR/seed-S/cells_<Myr>.csv)"""
import csv, glob, math, os, re, sys
import numpy as np

def seed_metrics(sdir, t0=500, t1=1000):
    acc = {}
    for p in sorted(glob.glob(os.path.join(sdir, 'cells_*.csv'))):
        t = int(re.search(r'cells_(\d+)', p).group(1))
        if not t0 <= t <= t1:
            continue
        with open(p) as f:
            r = csv.reader(f); h = next(r); rows = list(r)
        H = {k: i for i, k in enumerate(h)}
        cont = np.array([x[H['cls']] == '0' for x in rows])
        th = np.array([float(x[H['thick']]) for x in rows])[cont]
        orog = np.array([float(x[H['orogenic']]) for x in rows])[cont]
        n = cont.sum()
        for k, v in [('continental > 40 km (%)', 100 * (th > 40e3).sum() / n),
                     ('continental > 50 km (%)', 100 * (th > 50e3).sum() / n),
                     ('continental > 60 km (%)', 100 * (th > 60e3).sum() / n),
                     ('continental thickness p99 (km)', np.percentile(th, 99) / 1e3),
                     ('orogenic share >= 0.5 (% of continental)', 100 * (orog >= 0.5).sum() / n),
                     ('mean orogenic share (%)', 100 * orog.mean())]:
            acc.setdefault(k, []).append(v)
    return {k: float(np.mean(v)) for k, v in acc.items()}

def welch(a, b):
    a, b = np.array(a), np.array(b)
    va, vb = a.var(ddof=1) / len(a), b.var(ddof=1) / len(b)
    return (b.mean() - a.mean()) / math.sqrt(va + vb) if va + vb > 0 else 0.0

sets = [(os.path.basename(d.rstrip('/')), [seed_metrics(s) for s in sorted(glob.glob(os.path.join(d, 'seed-*')))]) for d in sys.argv[1:]]
keys = list(sets[0][1][0].keys())
print('| metric | ' + ' | '.join(f'{n} (n={len(v)})' for n, v in sets) + ' |')
print('|---' * (len(sets) + 1) + '|')
for k in keys:
    base = [m[k] for m in sets[0][1]]
    cells = [f'{np.mean(base):.2f}']
    for n, v in sets[1:]:
        x = [m[k] for m in v]
        cells.append(f'{np.mean(x):.2f} (t {welch(base, x):+.1f})')
    print(f'| {k} | ' + ' | '.join(cells) + ' |')
