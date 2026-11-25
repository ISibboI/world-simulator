"""Pits by the origin of the sample they show; needs cell dumps of a diagnostic build with `origin`, `origin_t` and `disp` columns (per-sample creation tags, not part of the branch)."""
import sys, numpy as np, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pits import mesh
NAMES = {0:'initial',1:'spreading',2:'reopened consumed',3:'rift stretch',4:'gap heal',5:'front advance',6:'merge'}
d = sys.argv[1]
nb = mesh(d); valid = nb >= 0; nbz = np.where(valid, nb, 0)
for t in [int(x) for x in sys.argv[2:]]:
    f = os.path.join(d, f'cells_{t:05d}.csv')
    hdr = open(f).readline().strip().split(',')
    cols = ['elev','ocean','origin','origin_t','disp','dup','thick','age']
    a = np.loadtxt(f, delimiter=',', skiprows=1, usecols=[hdr.index(c) for c in cols])
    c = {k: a[:, i] for i, k in enumerate(cols)}
    e = c['elev']; land = c['ocean'] == 0
    mn = np.where(valid, e[nbz], np.inf).min(1); depth = mn - e
    pit = land & (depth > 1); deep = land & (depth > 500); mid = land & (depth > 20)
    print(f't={t} land {land.sum()} pits {pit.sum()} >20m {mid.sum()} >500m {deep.sum()}')
    for name, m in [('land', land), ('pits', pit), ('pits>20m', mid), ('deep', deep)]:
        disp = c['disp'][m]
        org = c['origin'][m].astype(int)
        print(f'  {name:9s} disp>0.75: {100*(disp>0.75).mean():5.1f}%  disp>1.2: {100*(disp>1.2).mean():5.1f}% dup: {100*(c["dup"][m]>1).mean():5.1f}% | ' + ' '.join(f'{NAMES[k]}={100*(org==k).mean():.0f}%' for k in range(7) if (org==k).any()))
    for i in np.where(deep)[0]:
        print(f'    deep cell {i} depth {depth[i]:.0f} origin {NAMES[int(c["origin"][i])]} at {c["origin_t"][i]:.0f} age {c["age"][i]:.0f} thick {c["thick"][i]:.0f} disp {c["disp"][i]:.2f}')
