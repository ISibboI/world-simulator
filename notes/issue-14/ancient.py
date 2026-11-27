"""Issue #14 check: where ancient sea floor sits, its sediment, and pelagic thickness on old sea floor.
Usage: ancient.py RUNDIR [t0 t1]  (RUNDIR has seed-*/cells_TTTTT.csv, boundaries_TTTTT.csv, mesh.csv)."""
import csv, sys, os, glob
from collections import deque
import numpy as np

run = sys.argv[1]
t0, t1 = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (500, 1000)

def load_mesh(path):
    nb = []
    with open(path) as f:
        r = csv.reader(f); next(r)
        for row in r:
            nb.append([int(x) for x in row[4].split(';')])
    return nb

def bfs(nb, sources, allowed=None):
    n = len(nb); d = np.full(n, 10**6, dtype=np.int64); q = deque()
    for s in sources:
        d[s] = 0; q.append(s)
    while q:
        c = q.popleft()
        for m in nb[c]:
            if d[m] > d[c] + 1 and (allowed is None or allowed[m]):
                d[m] = d[c] + 1; q.append(m)
    return d

def pct(a, ps=(10, 25, 50, 75, 90)):
    return [float(np.percentile(a, p)) if len(a) else float('nan') for p in ps]

rows = []
for sd in sorted(glob.glob(os.path.join(run, 'seed-*'))):
    seed = int(sd.split('-')[-1])
    mesh = os.path.join(sd, 'mesh.csv')
    if not os.path.exists(mesh):
        continue
    nb = load_mesh(mesh); n = len(nb)
    for path in sorted(glob.glob(os.path.join(sd, 'cells_*.csv'))):
        t = int(path[-9:-4])
        if t < t0 or t > t1:
            continue
        with open(path) as f:
            r = csv.reader(f); h = next(r); ix = {k: i for i, k in enumerate(h)}
            data = [row for row in r]
        col = lambda k, typ=float: np.array([typ(row[ix[k]]) for row in data])
        sed, bulk, cls, plate, age, ocean = col('sed'), col('bulk'), col('cls', int), col('plate', int), col('age'), col('ocean', int)
        trench_own = set()
        with open(os.path.join(sd, f'boundaries_{t:05d}.csv')) as f:
            r = csv.reader(f); hb = next(r); ib = {k: i for i, k in enumerate(hb)}
            for row in r:
                if row[ib['kind']] == 't':
                    trench_own.add(int(row[ib['cell']]))
        oc = cls == 2
        sf = oc & (ocean == 1)
        boundary = np.array([any(plate[m] != plate[c] for m in nb[c]) for c in range(n)])
        d_bnd = bfs(nb, np.nonzero(boundary)[0])
        d_cont = bfs(nb, np.nonzero(cls != 2)[0])
        # rings to a trench where the cell's own plate subducts, through its own plate
        d_tr = np.full(n, 10**6, dtype=np.int64)
        for p in np.unique(plate):
            mask = plate == p
            src = [c for c in trench_own if mask[c]]
            if src:
                dp = bfs(nb, src, mask)
                d_tr[mask] = dp[mask]
        plate_cont = {p: float(np.mean(cls[plate == p] == 0)) for p in np.unique(plate)}
        plate_area = {p: int(np.sum(plate == p)) for p in np.unique(plate)}
        # regions: connected sea-floor cells of one plate
        reg = np.full(n, -1); regions = []
        for c in range(n):
            if sf[c] and reg[c] < 0:
                k = len(regions); reg[c] = k; q = deque([c]); cells = []
                while q:
                    a = q.popleft(); cells.append(a)
                    for m in nb[a]:
                        if sf[m] and reg[m] < 0 and plate[m] == plate[a]:
                            reg[m] = k; q.append(m)
                cs = set(cells); per_other = per_land = per_n = 0
                for a in cells:
                    for m in nb[a]:
                        if m in cs: continue
                        per_n += 1
                        if plate[m] != plate[a]: per_other += 1
                        else: per_land += 1  # own plate, not sea floor: continent, arc, filled basin
                regions.append(dict(n=len(cells), open=bool(boundary[cells].any()),
                                    land=per_land / max(per_n, 1), plate=int(plate[cells[0]])))
        anc = sf & (age >= 200); a180 = sf & (age >= 180)
        cat = {}; catx = {}; catsed = {}
        for c in np.nonzero(anc)[0]:
            R = regions[reg[c]]
            if not R['open']:
                k = 'enclosed basin (no boundary contact)'
            elif R['land'] >= 0.5 and R['n'] < 2048:
                k = 'marginal/inland sea (open, rim >= 50% own land/continent)'
            elif d_bnd[c] > 10:
                k = 'open ocean, plate interior > 10 rings from a boundary'
            else:
                k = 'open ocean, <= 10 rings from a boundary'
            cat.setdefault(k, []).append(c)
            b = 'lt20' if R['n'] < 20 else ('lt123' if R['n'] < 123 else ('lt410' if R['n'] < 410 else ('lt2048' if R['n'] < 2048 else 'ge2048')))
            ab = '200-300' if age[c] < 300 else ('300-500' if age[c] < 500 else 'ge500')
            k2 = k.split(' (')[0].split(',')[0] + ('|interior' if 'interior' in k else '')
            catx[(k2, b, ab)] = catx.get((k2, b, ab), 0) + 1
            catsed[(k2, b, ab)] = catsed.get((k2, b, ab), 0.0) + float(sed[c])
        oc_sed = sed[oc].sum()
        rec = dict(seed=seed, t=t, sf=int(sf.sum()), anc=int(anc.sum()), a180=int(a180.sum()),
                   anc_basement=int((oc & (age >= 200)).sum()), oc=int(oc.sum()),
                   sed_share_anc_basement=float(sed[oc & (age >= 200)].sum() / oc_sed),
                   sed_share_anc_sf=float(sed[anc].sum() / max(sed[sf].sum(), 1e-9)),
                   bulk_anc_mean=float(bulk[anc].mean()) if anc.any() else float('nan'),
                   bulk_anc_med=float(np.median(bulk[anc])) if anc.any() else float('nan'),
                   bulk_young_sf_mean=float(bulk[sf & (age < 200)].mean()),
                   age_anc=[int(((age >= lo) & (age < hi) & anc).sum()) for lo, hi in ((200, 300), (300, 500), (500, 1e9))],
                   max_age=float(age[sf].max()),
                   cats={k: (len(v), float(sed[v].sum() / max(sed[anc].sum(), 1e-9)), float(np.mean(bulk[v]))) for k, v in cat.items()},
                   d_bnd_med=float(np.median(d_bnd[anc])) if anc.any() else float('nan'),
                   d_cont_med=float(np.median(d_cont[anc])) if anc.any() else float('nan'),
                   near_cont=float(np.mean(d_cont[anc] <= 3)) if anc.any() else float('nan'),
                   sed_near_cont=float(sed[anc & (d_cont <= 3)].sum() / max(sed[anc].sum(), 1e-9)),
                   far_trench=float(np.mean(d_tr[anc] > 5)) if anc.any() else float('nan'),
                   no_own_trench=float(np.mean(d_tr[anc] >= 10**6)) if anc.any() else float('nan'),
                   d_tr_med=float(np.median(d_tr[anc][d_tr[anc] < 10**6])) if (anc & (d_tr < 10**6)).any() else float('nan'),
                   on_cont_plate=float(np.mean([plate_cont[plate[c]] >= 0.1 for c in np.nonzero(anc)[0]])) if anc.any() else float('nan'),
                   plate_area_med=float(np.median([plate_area[plate[c]] for c in np.nonzero(anc)[0]])) if anc.any() else float('nan'),
                   old100=pct(bulk[sf & (age > 100)]),
                   old100_abyssal=pct(bulk[sf & (age > 100) & (d_cont >= 5)]),
                   old100_abyssal_n=int((sf & (age > 100) & (d_cont >= 5)).sum()),
                   old100_n=int((sf & (age > 100)).sum()),
                   old100_100_200_abyssal=pct(bulk[sf & (age > 100) & (age < 200) & (d_cont >= 5)]),
                   frac_blanket=float(np.mean((bulk[sf & (age > 100) & (d_cont >= 5)] >= 200) & (bulk[sf & (age > 100) & (d_cont >= 5)] <= 1500))) if (sf & (age > 100) & (d_cont >= 5)).any() else float('nan'),
                   young_abyssal_bulk=pct(bulk[sf & (age < 50) & (d_cont >= 5)]),
                   catx=[[list(k), v, catsed[k]] for k, v in catx.items()],
                   sf_bulk_mean=float(bulk[sf].mean()))
        rows.append(rec)
        print(seed, t, f"anc {100*rec['anc']/rec['sf']:.2f}% sf, 180 {100*rec['a180']/rec['sf']:.2f}%, sedshare {100*rec['sed_share_anc_basement']:.0f}%", file=sys.stderr)

import json
out = os.path.join(run, 'ancient.json')
json.dump(rows, open(out, 'w'))
print('wrote', out, len(rows))
