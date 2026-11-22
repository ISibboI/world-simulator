"""Pit/peak metrics from cell dumps, per seed (snapshots 600..1000 Myr), Welch t vs the first set.
usage: pits.py label=DIR [label=DIR ...]   (DIR/seed-S/cells_TTTTT.csv, mesh.csv)"""
import sys, os, glob, math
import numpy as np

COLS = ['elev', 'sed', 'dup', 'precip', 'cls', 'thick']

def mesh(d):
    nb = []
    with open(os.path.join(d, 'mesh.csv')) as fh:
        fh.readline()
        for l in fh:
            n = [int(x) for x in l.strip().split(',')[4].split(';')]
            nb.append(n + [-1] * (6 - len(n)))
    return np.array(nb)

def cells(path):
    with open(path) as fh:
        hdr = fh.readline().strip().split(',')
    idx = [hdr.index(c) for c in COLS]
    a = np.loadtxt(path, delimiter=',', skiprows=1, usecols=idx)
    return {c: a[:, i] for i, c in enumerate(COLS)}

def seed_metrics(d):
    nb = mesh(d); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    acc = dict(sedspike=0, thickpit=0, pits=0, land=0, pit_dup=0, land_dup=0, peaks=0, peak_dup=0, peaks100=0, rough=[], p90=[], p99=[], meanland=[])
    for t in range(600, 1001, 100):
        f = os.path.join(d, f'cells_{t:05d}.csv')
        if not os.path.exists(f):
            return None
        c = cells(f); e = c['elev']; land = e >= 0; dup = c['dup'] > 1
        en = e[nbz]
        mn = np.where(valid, en, np.inf).min(1); mx = np.where(valid, en, -np.inf).max(1)
        pit = land & (e < mn - 1)
        peak = land & (e > mx + 1)
        acc['pits'] += pit.sum(); acc['land'] += land.sum()
        acc['pit_dup'] += (pit & dup).sum(); acc['land_dup'] += (land & dup).sum()
        acc['peaks'] += peak.sum(); acc['peak_dup'] += (peak & dup).sum()
        acc['peaks100'] += (land & (e > mx + 100)).sum()
        sd = c['sed'][nbz]; th = c['thick'][nbz]
        acc['sedspike'] += (land & (c['sed'] > np.where(valid, sd, np.inf).min(1) * 0 + np.where(valid, sd, -np.inf).max(1) + 2000)).sum()
        acc['thickpit'] += (land & (c['thick'] < np.where(valid, th, np.inf).min(1) - 3000)).sum()
        both = valid & land[:, None] & (en >= 0)
        acc['rough'].append(np.abs(e[:, None] - en)[both].mean())
        acc['p90'].append(np.percentile(e[land], 90)); acc['p99'].append(np.percentile(e[land], 99))
        acc['meanland'].append(e[land].mean())
    L = acc['land']
    return {
        'single-cell land pits (% of land)': 100 * acc['pits'] / L,
        'pits on duplicated samples (%)': 100 * acc['pit_dup'] / max(acc['pits'], 1),
        'land cells on duplicated samples (%)': 100 * acc['land_dup'] / L,
        'single-cell land peaks > 1 m (% of land)': 100 * acc['peaks'] / L,
        'peaks on duplicated samples (%)': 100 * acc['peak_dup'] / max(acc['peaks'], 1),
        'land peaks > 100 m above all neighbours (% of land)': 100 * acc['peaks100'] / L,
        'land cells with sediment > 2 km above every neighbour (per 10^4 land)': 1e4 * acc['sedspike'] / L,
        'land cells with crust > 3 km thinner than every neighbour (per 10^4 land)': 1e4 * acc['thickpit'] / L,
        'land neighbour |dE| (m)': np.mean(acc['rough']),
        'land p90 (m)': np.mean(acc['p90']), 'land p99 (m)': np.mean(acc['p99']),
        'mean land (m)': np.mean(acc['meanland']),
    }

def welch(a, b):
    a, b = np.array(a), np.array(b)
    if len(a) < 2 or len(b) < 2: return math.nan
    s = math.sqrt(a.var(ddof=1) / len(a) + b.var(ddof=1) / len(b))
    return (b.mean() - a.mean()) / s if s > 0 else math.nan

def main():
  sets = []
  for arg in sys.argv[1:]:
      label, d = arg.split('=', 1)
      res = [m for m in (seed_metrics(s) for s in sorted(glob.glob(os.path.join(d, 'seed-*')))) if m]
      sets.append((label, res))
  keys = list(sets[0][1][0].keys())
  print('snapshots 600-1000 Myr; seeds: ' + ', '.join(f'{l}: {len(r)}' for l, r in sets))
  print('| metric | ' + ' | '.join(l + (' | t' if i else '') for i, (l, _) in enumerate(sets)) + ' |')
  print('|---' * (1 + 2 * len(sets) - 1) + '|')
  for k in keys:
      row = [k]
      base = [r[k] for r in sets[0][1]]
      for i, (l, res) in enumerate(sets):
          v = [r[k] for r in res]
          row.append(f'{np.mean(v):.2f} ({min(v):.2f}…{max(v):.2f})')
          if i: row.append(f'{welch(base, v):+.1f}')
      print('| ' + ' | '.join(row) + ' |')

if __name__ == '__main__':
    main()
