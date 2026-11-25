"""Classify deep (> 500 m) single-cell pits by crust class, age and position."""
import sys, os, glob
import numpy as np
from pits import mesh
COLS = ['elev','sed','thick','plate','ocean','cls','age']
def load(path):
    with open(path) as fh: hdr = fh.readline().strip().split(',')
    a = np.loadtxt(path, delimiter=',', skiprows=1, usecols=[hdr.index(c) for c in COLS])
    return {c: a[:, i] for i, c in enumerate(COLS)}
cnt = {}
n = 0
for d in sys.argv[1:]:
  for s in sorted(glob.glob(os.path.join(d, 'seed-*'))):
    nb = mesh(s); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    for t in range(500, 1001, 50):
        f = os.path.join(s, f'cells_{t:05d}.csv')
        if not os.path.exists(f): continue
        c = load(f); e = c['elev']; land = c['ocean'] == 0
        mn = np.where(valid, e[nbz], np.inf).min(1)
        bnd = (valid & (c['plate'][nbz] != c['plate'][:, None])).any(1)
        thinner = c['thick'] < np.where(valid, c['thick'][nbz], np.inf).min(1) - 3000
        for i in np.where(land & (mn - e > 500))[0]:
            k = ('oceanic' if c['cls'][i] == 2 else 'transitional' if c['cls'][i] == 1 else 'continental',
                 'young<20' if c['age'][i] < 20 else 'old', 'boundary' if bnd[i] else 'interior', 'thinner>3km' if thinner[i] else 'not thinner')
            cnt[k] = cnt.get(k, 0) + 1; n += 1
for k, v in sorted(cnt.items(), key=lambda x: -x[1]):
    print(f'{100*v/n:5.1f}%  {v:4d}  {k}')
print('total', n)
