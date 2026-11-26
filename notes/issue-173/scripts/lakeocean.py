import sys, os, glob
import numpy as np
d = sys.argv.pop(1)
sys.argv = sys.argv[:1]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pits import mesh
from compare import load, components
res = {'all': [], 'large': [], 'large_trop': [], 'evap_large': [], 'evap_ocean_band': []}
for sd in sorted(glob.glob(d + '/seed-*')):
    nb = mesh(sd); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    for T in range(500, 1001, 50):
        f = f'{sd}/cells_{T:05d}.csv'
        if not os.path.exists(f): continue
        c = load(f); ocean = c['ocean'] == 1; lake = ~ocean & (c['lake'] >= 1)
        label, sizes = components(lake, nbz, valid)
        large = lake & np.isin(label, np.nonzero(sizes >= 20)[0])
        bins = np.floor(c['lat'] / 5).astype(int)
        ref = {b: c['t_annual'][ocean & (bins == b)].mean() for b in np.unique(bins[ocean])}
        refe = {b: c['evap'][ocean & (bins == b)].mean() for b in np.unique(bins[ocean])}
        an = lambda m, fld, r: [fld[i] - r.get(bins[i], np.nan) for i in np.nonzero(m)[0]]
        res['all'] += an(lake, c['t_annual'], ref)
        res['large'] += an(large, c['t_annual'], ref)
        res['large_trop'] += an(large & (np.abs(c['lat']) < 30), c['t_annual'], ref)
        res['evap_large'] += list(c['evap'][large])
        res['evap_ocean_band'] += [refe.get(bins[i], np.nan) for i in np.nonzero(large)[0]]
for k, v in res.items():
    v = np.array(v); v = v[~np.isnan(v)]
    print(k, len(v), f'mean {v.mean():.2f} p10 {np.percentile(v,10):.2f} p90 {np.percentile(v,90):.2f}' if len(v) else '')
