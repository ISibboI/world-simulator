import sys, os, re, glob
import numpy as np
d = sys.argv.pop(1)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pits import mesh
from compare import load, components

for sd in sorted(glob.glob(d + '/seed-*'), key=lambda p: int(p.rsplit('-', 1)[1])):
    nb = mesh(sd); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    water = {}
    t = None
    for line in open(sd + '/log.txt'):
        m = re.search(r't = +([\d.]+) Myr', line)
        if m: t = float(m.group(1))
        m = re.search(r'lake water \d+ km³ \(([\d.]+) m of sea level', line)
        if m and t is not None: water[int(round(t))] = float(m.group(1))
    for T in range(500, 1001, 50):
        f = f'{sd}/cells_{T:05d}.csv'
        if not os.path.exists(f) or water.get(T, 0) < 10: continue
        c = load(f); land = c['ocean'] == 0; lake = land & (c['lake'] >= 1)
        label, sizes = components(lake, nbz, valid)
        k = sizes.argmax(); big = label == k
        print(f"{os.path.basename(sd)} {T} Myr: lake water {water[T]:.0f} m; largest lake {sizes[k]} cells, lat {c['lat'][big].mean():+.0f}° ({c['lat'][big].min():+.0f}…{c['lat'][big].max():+.0f}), floor median {np.median(c['elev'][big]):.0f} m, depth median {np.median(c['lake'][big]):.0f} m; T annual {c['t_annual'][big].mean():.1f}, Jan {c['t_jan'][big].mean():.1f}, Jul {c['t_jul'][big].mean():.1f} °C; permanent ice {100*(c['ice'][big]>0.5).mean():.0f}%; P {c['precip'][big].mean():.2f} m/yr, E {c['evap'][big].mean():.2f}")
