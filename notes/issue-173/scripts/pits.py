"""Duplicates, single-cell pits and deep pits from cell dumps.
usage: pits.py DIR [T0 T1]   (DIR/seed-S/cells_TTTTT.csv, mesh.csv); prints per seed and mean.
A pit is a non-ocean cell more than 1 m below all its neighbours; deep pits more than 500 m.
Twins: neighbours on the same plate with identical crust thickness and sediment."""
import sys, os, glob
import numpy as np

COLS = ['elev', 'sed', 'thick', 'plate', 'dup', 'dyn', 'ocean', 'cls', 'dens']

def mesh(d):
    nb = []
    with open(os.path.join(d, 'mesh.csv')) as fh:
        fh.readline()
        for l in fh:
            n = [int(x) for x in l.strip().split(',')[4].split(';')]
            nb.append(n + [-1] * (6 - len(n)))
    return np.array(nb)

def load(path):
    with open(path) as fh:
        hdr = fh.readline().strip().split(',')
    a = np.loadtxt(path, delimiter=',', skiprows=1, usecols=[hdr.index(c) for c in COLS])
    return {c: a[:, i] for i, c in enumerate(COLS)}

def seed(d, t0, t1):
    nb = mesh(d); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    rows = []
    for t in range(t0, t1 + 1, 50):
        f = os.path.join(d, f'cells_{t:05d}.csv')
        if not os.path.exists(f):
            continue
        c = load(f); e = c['elev']; land = c['ocean'] == 0
        en = np.where(valid, e[nbz], np.inf); mn = en.min(1)
        depth = mn - e
        pit = land & (depth > 1)
        deep = land & (depth > 500)
        twin = valid & (c['thick'][nbz] == c['thick'][:, None]) & (c['sed'][nbz] == c['sed'][:, None]) & (c['plate'][nbz] == c['plate'][:, None])
        has_twin = twin.any(1)
        dup = c['dup'] > 1
        bnd = (valid & (c['plate'][nbz] != c['plate'][:, None])).any(1)
        rows.append(dict(
            dup=100 * dup.mean(), landdup=100 * (dup & land).sum() / land.sum(),
            pits=100 * pit.sum() / land.sum(), twinpits=100 * (pit & has_twin).sum() / max(pit.sum(), 1),
            deep=100 * deep.sum() / land.sum(), deepn=deep.sum(), deep_sub=100 * (deep & (e < 0)).sum() / max(deep.sum(), 1),
            deep_dup=100 * (deep & dup).sum() / max(deep.sum(), 1), deep_bnd=100 * (deep & bnd).sum() / max(deep.sum(), 1),
            pitdepth=np.median(depth[pit]) if pit.any() else 0,
            maxdup=c['dup'].max()))
    return {k: np.mean([r[k] for r in rows]) for k in rows[0]}

if __name__ == '__main__':
    d = sys.argv[1]; t0 = int(sys.argv[2]) if len(sys.argv) > 2 else 500; t1 = int(sys.argv[3]) if len(sys.argv) > 3 else 1000
    res = []
    for s in sorted(glob.glob(os.path.join(d, 'seed-*'))):
        r = seed(s, t0, t1); res.append(r)
        print(os.path.basename(s), ' '.join(f'{k}={v:.2f}' for k, v in r.items()))
    print('mean  ', ' '.join(f'{k}={np.mean([r[k] for r in res]):.2f}' for k in res[0]))
