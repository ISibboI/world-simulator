import sys, numpy as np
sys.path.insert(0, '.')
from pits import mesh
d = sys.argv[1]
nb = mesh(d); valid = nb >= 0; nbz = np.where(valid, nb, 0)
import os
for t in (800, 1000):
    f = os.path.join(d, f'cells_{t:05d}.csv')
    hdr = open(f).readline().strip().split(',')
    a = np.loadtxt(f, delimiter=',', skiprows=1, usecols=[hdr.index(c) for c in ['elev','sed','thick','plate','dup','dyn','discharge']])
    e, sed, th, pl, dup, dyn, q = a.T
    land = e >= 0
    en = e[nbz]
    mn = np.where(valid, en, np.inf).min(1)
    pit = land & (e < mn - 1)
    twin = valid & (th[nbz] == th[:, None]) & (sed[nbz] == sed[:, None]) & (pl[nbz] == pl[:, None])
    has_twin = twin.any(1)
    dp = pit & (dup > 1)
    print(t, 'pits', pit.sum(), 'dup pits', dp.sum(), 'dup pits with adjacent twin', (dp & has_twin).sum())
    # for dup pits with twin: elevation diff to twin and dyn diff
    idx = np.where(dp & has_twin)[0]
    de = [ (e[nb[c][twin[c]]] - e[c]).min() for c in idx]
    dd = [ (dyn[nb[c][twin[c]]] - dyn[c]).min() for c in idx]
    print('  twin elev - pit elev: median %.1f, q90 %.1f; dyn diff median %.1f' % (np.median(de), np.percentile(de, 90), np.median(dd)))
    print('  pit depth below lowest neighbour (dup): median %.1f; (non-dup) median %.1f' % (np.median((mn-e)[dp]), np.median((mn-e)[pit & (dup<=1)])))
    # how many dup land cells have twin adjacent at all
    print('  dup land cells', (land&(dup>1)).sum(), 'with adjacent twin', (land&(dup>1)&has_twin).sum())
