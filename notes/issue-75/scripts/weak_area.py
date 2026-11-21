"""Weak and orogenic continental area over time from cell dumps (#75/#53).
usage: weak_area.py label=DIR ...   (DIR holds seed-S/cells_TTTTT.csv; runs output layout)
Per snapshot, mean over seeds (min..max), of the continental cells (cls 0):
ever marked, never marked, last marked > 500 Myr ago, mean remaining orogenic weakening,
orogenic >= 0.5 / >= 0.25, and where the dump has them: strength < 0.75 (all causes) and
plume-heated (strength age younger than the crust's age and below 100 Myr: a plume reset that still weakens)."""
import csv, glob, os, re, sys
import numpy as np

def load(path):
    with open(path) as f:
        r = csv.reader(f); h = next(r); rows = list(r)
    H = {k: i for i, k in enumerate(h)}
    col = lambda k: [x[H[k]] for x in rows]
    d = {'cls': np.array([int(v) for v in col('cls')])}
    d['orogeny'] = np.array([float(v) if v else -np.inf for v in col('orogeny')])
    d['orogenic'] = np.array([float(v) for v in col('orogenic')])
    d['age'] = np.array([float(v) for v in col('age')])
    if 'strength' in H:
        d['strength'] = np.array([float(v) for v in col('strength')])
        d['strength_age'] = np.array([float(v) for v in col('strength_age')])
    return d

for arg in sys.argv[1:]:
    label, run = arg.split('=')
    series = {}
    for sdir in sorted(glob.glob(os.path.join(run, 'seed-*'))):
        for p in sorted(glob.glob(os.path.join(sdir, 'cells_*.csv'))):
            t = int(re.search(r'cells_(\d+)', p).group(1))
            d = load(p)
            cont = d['cls'] == 0
            n = cont.sum()
            pct = lambda m: 100.0 * (cont & m).sum() / n
            s = series.setdefault(t, {})
            add = lambda k, v: s.setdefault(k, []).append(v)
            add('ever marked', pct(d['orogeny'] > -np.inf))
            add('never marked', pct(d['orogeny'] == -np.inf))
            add('last marked > 500 Myr ago', pct((d['orogeny'] > -np.inf) & (d['orogeny'] < t - 500)))
            add('mean orogenic weakening', 100.0 * d['orogenic'][cont].sum() / n)
            add('orogenic >= 0.5', pct(d['orogenic'] >= 0.5))
            add('orogenic >= 0.25', pct(d['orogenic'] >= 0.25))
            if 'strength' in d:
                add('strength < 0.75', pct(d['strength'] < 0.75))
                heated = (d['strength_age'] < d['age'] - 1e-6) & (d['strength_age'] < 100.0)
                add('plume-heated (continental)', pct(heated))
                add('plume-heated (% of all cells)', 100.0 * heated.sum() / len(heated))
    if not series:
        continue
    ts = sorted(series)
    keys = []
    for t in ts:
        for k in series[t]:
            if k not in keys: keys.append(k)
    nseeds = len(series[ts[0]][keys[0]])
    print(f'## {label} ({nseeds} seeds): % of continental cells, mean (min–max) over seeds')
    print('| Myr | ' + ' | '.join(str(t) for t in ts) + ' |')
    print('|---|' + '---|' * len(ts))
    for k in keys:
        cells = []
        for t in ts:
            v = series[t].get(k)
            cells.append(f'{np.mean(v):.1f} ({min(v):.0f}–{max(v):.0f})' if v else '—')
        print(f'| {k} | ' + ' | '.join(cells) + ' |')
    print()
