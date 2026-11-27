"""Orogen heights from the cell dumps at 600…1000 Myr: land elevation percentiles and the share of
land above 2 and 3 km, mean over snapshots per seed; mean, range and Welch t against the first set.
usage: orogens.py label=DIR [label=DIR ...]   (DIR holds seed-<seed>/cells_<t>.csv)"""
import csv, glob, math, os, sys


def seed_metrics(d):
    rows = []
    for t in (600, 700, 800, 900, 1000):
        path = os.path.join(d, f'cells_{t:05d}.csv')
        if not os.path.exists(path):
            return None
        land, thick = [], []
        with open(path) as f:
            for r in csv.DictReader(f):
                e = float(r['elev'])
                if e > 0:
                    land.append(e)
                    thick.append(float(r['thick']))
        land.sort(); thick.sort()
        n = len(land)
        rows.append({
            'land p90 (m)': land[int(0.9 * n)],
            'land p99 (m)': land[int(0.99 * n)],
            'land > 2 km (% of land)': 100 * sum(e > 2000 for e in land) / n,
            'land > 3 km (% of land)': 100 * sum(e > 3000 for e in land) / n,
            'land crust p99 (km)': thick[int(0.99 * n)] / 1e3,
            'land crust > 50 km (% of land)': 100 * sum(x > 50e3 for x in thick) / n,
        })
    return {k: sum(r[k] for r in rows) / len(rows) for k in rows[0]}


sets = []
for a in sys.argv[1:]:
    label, d = a.split('=')
    res = [m for s in sorted(glob.glob(os.path.join(d, 'seed-*'))) if (m := seed_metrics(s))]
    sets.append((label, res))
keys = list(sets[0][1][0])
print('| metric | ' + ' | '.join(l + (' | t' if i else '') for i, (l, _) in enumerate(sets)) + ' |')
print('|---|' + '---|' * (2 * len(sets) - 1))
for k in keys:
    cells, base = [], None
    for i, (l, res) in enumerate(sets):
        v = [r[k] for r in res]
        n = len(v); m = sum(v) / n; var = sum((x - m) ** 2 for x in v) / (n - 1)
        cells.append(f'{m:.2f} ({min(v):.2f}…{max(v):.2f})' if abs(m) < 100 else f'{m:.0f} ({min(v):.0f}…{max(v):.0f})')
        if i == 0:
            base = (m, var, n)
        else:
            cells.append(f'{(m - base[0]) / math.sqrt(var / n + base[1] / base[2]):+.1f}')
    print(f'| {k} | ' + ' | '.join(cells) + ' |')
