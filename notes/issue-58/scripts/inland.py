"""Inland below-sea-level depressions from cell dumps.
usage: inland.py label=DIR [label=DIR ...]   (DIR holds seed-S/cells_*.csv or sS/, mesh.csv); window 500-1000
Prints per set: mean over seeds (mean over snapshots) of: cells below sea level not connected to the
deepest cell (% of surface), number of such components, components of 1-3 cells, ocean share of
below-sea-level cells, and Welch t vs the first set."""
import csv, glob, math, os, re, sys
from collections import deque
import numpy as np
T0, T1 = 500, 1000

def load_nb(path):
    with open(path) as f:
        r = csv.reader(f); h = next(r); i = h.index('neighbors')
        return [list(map(int, row[i].split(';'))) for row in r]

def elev(path):
    with open(path) as f:
        r = csv.reader(f); h = next(r); i = h.index('elev')
        return np.array([float(row[i]) for row in r])

def metrics(e, nb):
    n = len(e); below = e < 0
    ocean = np.zeros(n, bool); s = int(np.argmin(e)); ocean[s] = True; q = deque([s])
    while q:
        c = q.popleft()
        for m in nb[c]:
            if below[m] and not ocean[m]:
                ocean[m] = True; q.append(m)
    inland = below & ~ocean
    seen = np.zeros(n, bool); comps = []
    for c in np.nonzero(inland)[0]:
        if seen[c]: continue
        seen[c] = True; q = deque([c]); k = 0
        while q:
            x = q.popleft(); k += 1
            for m in nb[x]:
                if inland[m] and not seen[m]:
                    seen[m] = True; q.append(m)
        comps.append(k)
    return {'inland below sea level (% of surface)': 100 * inland.sum() / n,
            'inland components': len(comps),
            'inland components of 1-3 cells': sum(1 for k in comps if k <= 3),
            'largest inland component (cells)': max(comps, default=0),
            'ocean share of below-sea-level cells (%)': 100 * ocean.sum() / max(below.sum(), 1)}

def run(d):
    out = {}
    for sd in sorted(glob.glob(os.path.join(d, '*'))):
        mp = os.path.join(sd, 'mesh.csv')
        if not os.path.exists(mp): continue
        nb = load_nb(mp); per = []
        for p in sorted(glob.glob(os.path.join(sd, 'cells_*.csv'))):
            t = float(re.search(r'cells_(\d+)', p).group(1))
            if T0 < t <= T1: per.append(metrics(elev(p), nb))
        if per:
            out[os.path.basename(sd)] = {k: sum(x[k] for x in per) / len(per) for k in per[0]}
    return out

sets = [(a.split('=')[0], run(a.split('=')[1])) for a in sys.argv[1:]]
keys = list(next(iter(sets[0][1].values())).keys())
print('| metric | ' + ' | '.join(l + (' | t' if i else '') for i, (l, _) in enumerate(sets)) + ' |')
print('|---|' + '---|' * (2 * len(sets) - 1))
for k in keys:
    row = []; base = None
    for i, (l, r) in enumerate(sets):
        v = [x[k] for x in r.values()]; m = sum(v) / len(v)
        var = sum((x - m) ** 2 for x in v) / (len(v) - 1) if len(v) > 1 else 0
        row.append(f'{m:.2f} ({min(v):.2f}…{max(v):.2f})')
        if i == 0: base = (m, var, len(v))
        else:
            se = math.sqrt(var / len(v) + base[1] / base[2]); row.append(f'{(m - base[0]) / se:+.1f}' if se else '—')
    print(f'| {k} | ' + ' | '.join(row) + ' |')
