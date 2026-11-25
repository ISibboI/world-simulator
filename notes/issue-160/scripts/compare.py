"""Before/after metrics for #112/#129 per seed, means over snapshots 500..1000 Myr, Welch t vs the first set.
usage: compare.py label=DIR [label=DIR ...]  (DIR/seed-S/{cells_TTTTT.csv,mesh.csv,log.txt,lakes.csv})"""
import sys, os, glob, re
import numpy as np
from pits import mesh

COLS = ['elev', 'sed', 'thick', 'plate', 'dup', 'dyn', 'ocean', 'lake', 'cls', 'discharge']
T0, T1 = 500, 1000

def load(path):
    with open(path) as fh:
        hdr = fh.readline().strip().split(',')
    cols = [c for c in COLS if c in hdr]
    a = np.loadtxt(path, delimiter=',', skiprows=1, usecols=[hdr.index(c) for c in cols])
    out = {c: a[:, i] for i, c in enumerate(cols)}
    out.setdefault('lake', np.zeros(len(a)))
    return out

def dumps(d):
    nb = mesh(d); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    acc = {}
    def add(k, v): acc.setdefault(k, []).append(v)
    for t in range(T0, T1 + 1, 50):
        f = os.path.join(d, f'cells_{t:05d}.csv')
        if not os.path.exists(f):
            continue
        c = load(f); e = c['elev']; land = c['ocean'] == 0; L = land.sum()
        en = np.where(valid, e[nbz], np.inf); mn = en.min(1); depth = mn - e
        mx = np.where(valid, e[nbz], -np.inf).max(1)
        pit = land & (depth > 1)
        twin = valid & (c['thick'][nbz] == c['thick'][:, None]) & (c['sed'][nbz] == c['sed'][:, None]) & (c['plate'][nbz] == c['plate'][:, None])
        dup = c['dup'] > 1
        add('cells on duplicated samples (%)', 100 * dup.mean())
        add('single-cell land pits (% of land)', 100 * pit.sum() / L)
        add('pits with a same-sample twin (% of pits)', 100 * (pit & twin.any(1)).sum() / max(pit.sum(), 1))
        add('pit depth median (m)', np.median(depth[pit]) if pit.any() else 0)
        add('pits > 20 m (% of land)', 100 * (land & (depth > 20)).sum() / L)
        add('pits > 100 m (% of land)', 100 * (land & (depth > 100)).sum() / L)
        add('pits > 500 m (per snapshot)', (land & (depth > 500)).sum())
        add('single-cell land peaks > 100 m (% of land)', 100 * (land & (e > mx + 100)).sum() / L)
        both = valid & land[:, None] & land[nbz]
        add('land neighbour |dE| (m)', np.abs(e[:, None] - e[nbz])[both].mean())
        add('land (% of surface)', 100 * land.mean())
        add('mean land (m)', e[land].mean())
        add('land p50 (m)', np.percentile(e[land], 50))
        add('land p90 (m)', np.percentile(e[land], 90))
        add('land p99 (m)', np.percentile(e[land], 99))
        add('ocean p50 (m)', np.percentile(e[~land], 50))
        lake = land & (c['lake'] > 0)
        add('lake cells (% of land)', 100 * lake.sum() / L)
        add('lakes on single-cell pits (% of lake cells)', 100 * (lake & pit).sum() / max(lake.sum(), 1))
        cont = c['cls'] == 0
        add('flooded continental crust (%)', 100 * (cont & ~land).sum() / max(cont.sum(), 1))
    return {k: np.mean(v) for k, v in acc.items()}

PAT = [
    ('endorheic (% of land)', r'endorheic ([\d.]+)% of land'),
    ('endorheic into single-cell pits (% of land)', r'\(([\d.]+)% into single-cell pits\)'),
    ('lakes (% of land, log)', r'lakes ([\d.]+)% of land'),
    ('depressions', r'depressions (\d+),'),
    ('endorheic depressions', r'depressions \d+, endorheic (\d+)'),
    ('duplicated cells (%, log)', r'resampling: ([\d.]+)% of cells show'),
    ('highest point (m)', r'elevation +-?\d+ m \.\. +(\d+) m'),
    ('mean speed (cm/yr)', r'mean speed +([\d.]+) cm/yr'),
    ('all closed depressions (% of land)', r'all closed depressions \d+ \(([\d.]+)% of'),
    ('playas (% of land)', r'playas \d+ \(([\d.]+)% of land'),
    ('lake evaporation (km³/yr)', r'lake evaporation ([\d.]+) km'),
    ('runoff to the ocean (km³/yr)', r'to the ocean ([\d.]+) km'),
    ('global mean temperature (°C)', r'temperature global +([-\d.]+) °C'),
    ('land precipitation (m/yr)', r'land precipitation ([\d.]+) m/yr'),
    ('sea level (m)', r'    sea level ([-\d.]+) m \(dry'),
]

def logs(d):
    f = os.path.join(d, 'log.txt')
    acc = {}
    t = None
    for line in open(f):
        m = re.search(r't = +([\d.]+) Myr', line)
        if m:
            t = float(m.group(1))
        if t is None or t < T0 or t > T1:
            continue
        for name, p in PAT:
            m = re.search(p, line)
            if m:
                acc.setdefault(name, []).append(float(m.group(1)))
    return {k: np.mean(v) for k, v in acc.items()}

def lakes(d):
    return {}

def _unused(d):
    f = os.path.join(d, 'lakes.csv')
    steps = []
    for line in open(f):
        parts = line.strip().split(',')
        steps.append(set(int(p.split(':')[0]) for p in parts[1:]))
    out = {}
    for lag in (1, 10):
        kept = [len(a & b) / max(len(a), 1) for a, b in zip(steps, steps[lag:])]
        out[f'lake cells still lake after {lag} Myr (%)'] = 100 * np.mean(kept)
    # lifetime of lake cells: mean run length of consecutive steps as lake
    runs = []
    current = {}
    for s in steps:
        for c in list(current):
            if c not in s:
                runs.append(current.pop(c))
        for c in s:
            current[c] = current.get(c, 0) + 1
    out['lake-cell run length (Myr, mean)'] = np.mean(runs) if runs else 0
    out['lake cells per step'] = np.mean([len(s) for s in steps])
    return out

def seed(d):
    r = {}
    r.update(dumps(d)); r.update(logs(d)); r.update(lakes(d))
    sec = os.path.join(d, 'seconds')
    if os.path.exists(sec):
        r['run time (s)'] = float(open(sec).read())
    return r

def welch(a, b):
    a, b = np.array(a), np.array(b)
    if len(a) < 2 or len(b) < 2:
        return float('nan')
    s = np.sqrt(a.var(ddof=1) / len(a) + b.var(ddof=1) / len(b))
    return (b.mean() - a.mean()) / s if s > 0 else float('nan')

sets = []
for arg in sys.argv[1:]:
    label, d = arg.split('=', 1)
    seeds = sorted(glob.glob(os.path.join(d, 'seed-*')), key=lambda p: int(p.rsplit('-', 1)[1]))
    res = [seed(s) for s in seeds if os.path.exists(os.path.join(s, f'cells_{T1:05d}.csv'))]
    sets.append((label, res))
keys = list(sets[0][1][0].keys())
print(f'snapshots {T0}-{T1} Myr; seeds: ' + ', '.join(f'{l}: {len(r)}' for l, r in sets))
print('| metric | ' + ' | '.join(f'{l} | t' if i else l for i, (l, _) in enumerate(sets)) + ' |')
print('|---|' + '---|' * (2 * len(sets) - 1))
for k in keys:
    base = [r.get(k, np.nan) for r in sets[0][1]]
    row = [f'{np.nanmean(base):.3g} ({np.nanmin(base):.3g}…{np.nanmax(base):.3g})']
    for l, res in sets[1:]:
        v = [r.get(k, np.nan) for r in res]
        row.append(f'{np.nanmean(v):.3g} ({np.nanmin(v):.3g}…{np.nanmax(v):.3g})')
        row.append(f'{welch(base, v):+.1f}')
    print(f'| {k} | ' + ' | '.join(row) + ' |')
