"""Characterise deep (> 500 m) single-cell pits: crust class, thickness vs neighbour mean, age, sediment."""
import sys, os, glob
import numpy as np
from pits import mesh
COLS = ['elev','sed','thick','plate','dup','dyn','ocean','cls','dens','age','felsic','strain','orogenic']
def load(path):
    with open(path) as fh: hdr = fh.readline().strip().split(',')
    a = np.loadtxt(path, delimiter=',', skiprows=1, usecols=[hdr.index(c) for c in COLS])
    return {c: a[:, i] for i, c in enumerate(COLS)}
d = sys.argv[1]
for s in sorted(glob.glob(os.path.join(d, 'seed-*')))[:int(sys.argv[2]) if len(sys.argv)>2 else 3]:
    nb = mesh(s); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    for t in (600, 800, 1000):
        c = load(os.path.join(s, f'cells_{t:05d}.csv')); e = c['elev']; land = c['ocean'] == 0
        mn = np.where(valid, e[nbz], np.inf).min(1)
        deep = np.where(land & (mn - e > 500))[0]
        for i in deep:
            n = nb[i][nb[i] >= 0]
            print(f'{os.path.basename(s)} t={t} cell={i} depth={mn[i]-e[i]:.0f} e={e[i]:.0f} cls={c["cls"][i]:.0f} nbcls={",".join(str(int(x)) for x in c["cls"][n])} thick={c["thick"][i]/1e3:.1f} nbthick={np.mean(c["thick"][n])/1e3:.1f} sed={c["sed"][i]:.0f} nbsed={np.mean(c["sed"][n]):.0f} dens={c["dens"][i]:.0f} nbdens={np.mean(c["dens"][n]):.0f} age={c["age"][i]:.0f} nbage={np.mean(c["age"][n]):.0f} dyn={c["dyn"][i]:.0f} nbdyn={np.mean(c["dyn"][n]):.0f} plate={c["plate"][i]:.0f} nbplates={",".join(str(int(x)) for x in c["plate"][n])} oc_nb={int(c["ocean"][n].sum())}')
