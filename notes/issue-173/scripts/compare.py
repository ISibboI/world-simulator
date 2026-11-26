"""Before/after metrics for #173 (from #126/#168's script) per seed, means over snapshots 500..1000 Myr, Welch t vs the first set.
usage: compare.py label=DIR [label=DIR ...]  (DIR/seed-S/{cells_TTTTT.csv,mesh.csv,log.txt,lakes.csv})"""
import sys, os, glob, re
import numpy as np
from pits import mesh

COLS = ['lat', 'elev', 'sed', 'thick', 'plate', 'dup', 'dyn', 'ocean', 'lake', 'cls', 'discharge', 'precip', 'evap', 't_annual', 't_jan', 't_jul', 'lake_evap', 'lake_air', 'ice']
T0, T1 = 500, 1000

def load(path):
    with open(path) as fh:
        hdr = fh.readline().strip().split(',')
    cols = [c for c in COLS if c in hdr]
    a = np.loadtxt(path, delimiter=',', skiprows=1, usecols=[hdr.index(c) for c in cols])
    out = {c: a[:, i] for i, c in enumerate(cols)}
    out.setdefault('lake', np.zeros(len(a)))
    out.setdefault('lake_evap', np.full(len(a), np.nan))
    out.setdefault('lake_air', np.full(len(a), np.nan))
    out.setdefault('ice', np.full(len(a), np.nan))
    return out

LARGE = 20

def components(mask, nbz, valid):
    """Connected components of `mask`: label per cell (-1 outside) and sizes."""
    label = np.full(len(mask), -1)
    sizes = []
    for start in np.nonzero(mask)[0]:
        if label[start] >= 0:
            continue
        k = len(sizes); label[start] = k; stack = [start]; n = 0
        while stack:
            c = stack.pop(); n += 1
            for m in nbz[c][valid[c]]:
                if mask[m] and label[m] < 0:
                    label[m] = k; stack.append(m)
        sizes.append(n)
    return label, np.array(sizes, dtype=int)

def lake_climate(c, land, lake, nbz, valid, add):
    """Lake temperatures and ice, and the land around large lakes against land farther away at
    the same latitudes (difference; seeds diverge, so only such within-snapshot contrasts compare)."""
    rng = np.abs(c['t_jul'] - c['t_jan'])
    dry = land & ~lake
    label, sizes = components(lake, nbz, valid)
    add('largest lake (cells)', sizes.max() if len(sizes) else 0)
    large = lake & (label >= 0) & np.isin(label, np.nonzero(sizes >= LARGE)[0])
    add('cells of large lakes (≥ 20 cells; per snapshot)', large.sum())
    if lake.any():
        add('lake annual T (°C)', c['t_annual'][lake].mean())
        add('lake |Jul − Jan| (K)', rng[lake].mean())
        if not np.all(np.isnan(c['ice'])):
            add('lake cells with permanent ice (%)', 100 * (c['ice'][lake] > 0.5).mean())
        cold = lake & (np.minimum(c['t_jan'], c['t_jul']) < 0)
        add('lake cells with a month below 0 °C (%)', 100 * cold.sum() / lake.sum())
    # Land seasonal range relative to land at the same latitude (5° bins), and its mean.
    lat = c['lat']; bins = np.floor(lat / 5).astype(int)
    ref_r = {}; ref_t = {}; ref_p = {}
    for b in np.unique(bins[dry]):
        m = dry & (bins == b)
        ref_r[b] = rng[m].mean(); ref_t[b] = c['t_annual'][m].mean(); ref_p[b] = c['precip'][m].mean()
    def anomaly(mask, field, ref):
        return np.nanmean([field[i] - ref.get(bins[i], np.nan) for i in np.nonzero(mask)[0]]) if mask.any() else np.nan
    if large.any():
        add('large lakes |Jul − Jan| minus land at their latitudes (K)', anomaly(large, rng, ref_r))
        ring = large.copy(); rings = []
        for _ in range(3):
            nxt = ring | (valid & ring[nbz]).any(1)
            rings.append(nxt & ~ring & dry); ring = nxt
        r1, r23 = rings[0], rings[1] | rings[2]
        add('land next to large lakes: |Jul − Jan| anomaly (K)', anomaly(r1, rng, ref_r))
        add('land 2–3 cells from large lakes: |Jul − Jan| anomaly (K)', anomaly(r23, rng, ref_r))
        add('land next to large lakes: annual T anomaly (K)', anomaly(r1, c['t_annual'], ref_t))
        add('land next to large lakes: precipitation anomaly (m/yr)', anomaly(r1, c['precip'], ref_p))
    add('land without lakes: precipitation (m/yr)', c['precip'][dry].mean())
    add('land without lakes: arid, P < 0.25 m/yr (%)', 100 * (c['precip'][dry] < 0.25).mean())
    add('land without lakes: annual T (°C)', c['t_annual'][dry].mean())
    add('land without lakes: |Jul − Jan| (K)', rng[dry].mean())
    if not np.all(np.isnan(c['ice'])):
        add('surface with permanent ice (%)', 100 * (c['ice'] > 0.5).mean())

def dumps(d):
    nb = mesh(d); valid = nb >= 0; nbz = np.where(valid, nb, 0)
    acc = {}
    def add(k, v): acc.setdefault(k, []).append(v)
    for t in range(T0, T1 + 1, 50):
        f = os.path.join(d, f'cells_{t:05d}.csv')
        if not os.path.exists(f):
            continue
        c = load(f); e = c['elev']; land = c['ocean'] == 0; L = land.sum()
        en = np.where(valid, e[nbz], np.inf); mn = en.min(1); depth = mn - e
        mx = np.where(valid, e[nbz], -np.inf).max(1)
        pit = land & (depth > 1)
        twin = valid & (c['thick'][nbz] == c['thick'][:, None]) & (c['sed'][nbz] == c['sed'][:, None]) & (c['plate'][nbz] == c['plate'][:, None])
        dup = c['dup'] > 1
        add('cells on duplicated samples (%)', 100 * dup.mean())
        add('single-cell land pits (% of land)', 100 * pit.sum() / L)
        add('pits with a same-sample twin (% of pits)', 100 * (pit & twin.any(1)).sum() / max(pit.sum(), 1))
        add('pit depth median (m)', np.median(depth[pit]) if pit.any() else 0)
        add('pits > 20 m (% of land)', 100 * (land & (depth > 20)).sum() / L)
        add('pits > 100 m (% of land)', 100 * (land & (depth > 100)).sum() / L)
        add('pits > 500 m (per snapshot)', (land & (depth > 500)).sum())
        add('single-cell land peaks > 100 m (% of land)', 100 * (land & (e > mx + 100)).sum() / L)
        both = valid & land[:, None] & land[nbz]
        add('land neighbour |dE| (m)', np.abs(e[:, None] - e[nbz])[both].mean())
        add('land (% of surface)', 100 * land.mean())
        add('mean land (m)', e[land].mean())
        add('land p50 (m)', np.percentile(e[land], 50))
        add('land p90 (m)', np.percentile(e[land], 90))
        add('land p99 (m)', np.percentile(e[land], 99))
        add('ocean p50 (m)', np.percentile(e[~land], 50))
        lake = land & (c['lake'] > 0)
        add('lake cells (% of land)', 100 * lake.sum() / L)
        add('lakes on single-cell pits (% of lake cells)', 100 * (lake & pit).sum() / max(lake.sum(), 1))
        inland = land & (e < 0)
        dry_inland = inland & (c['lake'] <= 0)
        add('dry land below sea level (cells)', dry_inland.sum())
        add('dry land below sea level, mean elevation (m)', e[dry_inland].mean() if dry_inland.any() else np.nan)
        lake = land & (c['lake'] > 0)
        add('lake cells without evaporation excess (% of lake cells)', 100 * (lake & (c['lake_evap'] <= 0)).sum() / max(lake.sum(), 1))
        add('lake depth, median (m)', np.median(c['lake'][lake]) if lake.any() else np.nan)
        add('lake floor, median (m)', np.median(e[lake]) if lake.any() else np.nan)
        # Precipitation on and around lakes (rings of neighbours), land elsewhere, aridity.
        p = c['precip']
        ring = lake.copy(); rings = []
        for _ in range(3):
            nxt = ring | (valid & ring[nbz]).any(1)
            rings.append(nxt & ~ring & land); ring = nxt
        far = land & ~ring
        add('precipitation on lake cells (m/yr)', p[lake].mean() if lake.any() else np.nan)
        add('precipitation 1 cell from lakes (m/yr)', p[rings[0]].mean() if rings[0].any() else np.nan)
        add('precipitation 2–3 cells from lakes (m/yr)', p[rings[1] | rings[2]].mean() if (rings[1] | rings[2]).any() else np.nan)
        add('precipitation, land farther from lakes (m/yr)', p[far].mean())
        add('arid land, P < 0.25 m/yr (% of land)', 100 * (land & (p < 0.25)).sum() / L)
        lat = np.abs(c['lat'])
        for lo, hi in ((0, 30), (30, 60), (60, 90)):
            band = land & (lat >= lo) & (lat < hi)
            add(f'lake cells {lo}–{hi}° (% of land there)', 100 * (lake & band).sum() / max(band.sum(), 1))
        cold = land & (c['t_annual'] < 0)
        add('lake cells, annual T < 0 °C (% of such land)', 100 * (lake & cold).sum() / max(cold.sum(), 1))
        lake_climate(c, land, lake, nbz, valid, add)
        cont = c['cls'] == 0
        add('flooded continental crust (%)', 100 * (cont & ~land).sum() / max(cont.sum(), 1))
    return {k: np.mean(v) for k, v in acc.items()}

PAT = [
    ('endorheic (% of land)', r'endorheic ([\d.]+)% of land'),
    ('endorheic into single-cell pits (% of land)', r'\(([\d.]+)% into single-cell pits\)'),
    ('lakes (% of land, log)', r'lakes ([\d.]+)% of land'),
    ('depressions', r'depressions (\d+),'),
    ('endorheic depressions', r'depressions \d+, endorheic (\d+)'),
    ('duplicated cells (%, log)', r'resampling: ([\d.]+)% of cells show'),
    ('highest point (m)', r'elevation +-?\d+ m \.\. +(\d+) m'),
    ('mean speed (cm/yr)', r'mean speed +([\d.]+) cm/yr'),
    ('all closed depressions (% of land)', r'all closed depressions \d+ \(([\d.]+)% of'),
    ('playas (% of land)', r'playas \d+ \(([\d.]+)% of land'),
    ('lake evaporation (km³/yr)', r'lake evaporation ([\d.]+) km'),
    ('runoff to the ocean (km³/yr)', r'to the ocean ([\d.]+) km'),
    ('lake evaporation into the air (km³/yr)', r'\(into the air ([\d.]+)\)'),
    ('global precipitation (m/yr)', r'global precipitation ([\d.]+),'),
    ('global evaporation (m/yr)', r'global precipitation [\d.]+, evaporation ([\d.]+) m/yr'),
    ('clipped precipitation (%)', r'clipped ([\d.]+)%'),
    ('land runoff (m/yr)', r'runoff ([\d.]+) m/yr \|'),
    ('lake potential evaporation (m/yr)', r'lake potential evaporation ([\d.]+) m/yr'),
    ('Köppen A (% of land)', r'climate classes of land: A ([\d.]+)%'),
    ('Köppen B (% of land)', r'climate classes of land: .*B ([\d.]+)%'),
    ('Köppen C (% of land)', r'climate classes of land: .*C ([\d.]+)%'),
    ('Köppen D (% of land)', r'climate classes of land: .*D ([\d.]+)%'),
    ('Köppen E (% of land)', r'climate classes of land: .*E ([\d.]+)%'),
    ('global mean temperature (°C)', r'temperature global +([-\d.]+) °C'),
    ('land precipitation (m/yr)', r'land precipitation ([\d.]+) m/yr'),
    ('sea level (m)', r'    sea level ([-\d.]+) m \(dry'),
    ('lake water (m of sea level)', r'lake water \d+ km³ \(([\d.]+) m of sea level'),
    ('frozen surface (%, log)', r'permanently frozen +([\d.]+)%'),
    ('paired: large lakes, range water (K)', r'large lakes \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range ([\d.]+) /'),
    ('paired: large lakes, range land (K)', r'large lakes \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range [\d.]+ / ([\d.]+) K'),
    ('paired: large lakes, annual water (°C)', r'large lakes \d+ cells: annual ([-\d.]+) /'),
    ('paired: large lakes, annual land (°C)', r'large lakes \d+ cells: annual [-\d.]+ / ([-\d.]+) °C'),
    ('paired: large lakes, frozen water (%)', r'large lakes \d+ cells: .*?frozen (\d+) /'),
    ('paired: large lakes, frozen land (%)', r'large lakes \d+ cells: .*?frozen \d+ / (\d+)%'),
    ('paired: land next to large lakes, range water (K)', r'next to large lakes \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range ([\d.]+) /'),
    ('paired: land next to large lakes, range land (K)', r'next to large lakes \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range [\d.]+ / ([\d.]+) K'),
    ('paired: land next to large lakes, annual water (°C)', r'next to large lakes \d+ cells: annual ([-\d.]+) /'),
    ('paired: land next to large lakes, annual land (°C)', r'next to large lakes \d+ cells: annual [-\d.]+ / ([-\d.]+) °C'),
    ('paired: land 2-3 cells, range water (K)', r'2-3 cells from them \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range ([\d.]+) /'),
    ('paired: land 2-3 cells, range land (K)', r'2-3 cells from them \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range [\d.]+ / ([\d.]+) K'),
    ('paired: all lakes, range water (K)', r'land\): lakes \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range ([\d.]+) /'),
    ('paired: all lakes, range land (K)', r'land\): lakes \d+ cells: annual [-\d.]+ / [-\d.]+ °C, range [\d.]+ / ([\d.]+) K'),
    ('paired: all lakes, annual water (°C)', r'land\): lakes \d+ cells: annual ([-\d.]+) /'),
    ('paired: all lakes, annual land (°C)', r'land\): lakes \d+ cells: annual [-\d.]+ / ([-\d.]+) °C'),
    ('closed depressions below sea level (cells)', r'closed depressions\s+below sea level (\d+) cells'),
    ('… of them under lake water (cells)', r'below sea level \d+ cells \(lake water on ([\d.]+)\)'),
    ('deficit at a pass after the solve (m of sea level)', r'deficit \d+ km³ \(([\d.]+) m\)'),
    ('lakes (water bodies, #168)', r'spill: (\d+) lakes in'),
    ('depressions holding water', r'lakes in (\d+) depressions'),
    ('… holding several lakes (inner sills)', r'(\d+) of them with several'),
    ('sea-fed lakes', r'sea-fed (?:basins|lakes) (\d+) \('),
    ('sea-fed lakes, cells covered', r'sea-fed (?:basins|lakes) \d+ \(([\d.]+) cells covered'),
    ('sea water in basins (m of sea level)', r'sea water \d+ km³ = ([\d.]+) m'),
    ('sea-fed evaporation beyond own inflow (km³/yr)', r'beyond their own inflow (\d+) km'),
    ('endorheic into sea-fed basins (% of land)', r'endorheic land into them ([\d.]+)%'),
    ('poured over sills at step end (m of sea level)', r'end of the step \d+ km³ \(([\d.]+) m\)'),
]
MAX = {'lake water (m of sea level)', 'deficit at a pass after the solve (m of sea level)', 'sea water in basins (m of sea level)'}

def logs(d):
    f = os.path.join(d, 'log.txt')
    acc = {}
    t = None
    for line in open(f):
        m = re.search(r't = +([\d.]+) Myr', line)
        if m:
            t = float(m.group(1))
        if t is None or t < T0 or t > T1:
            continue
        for name, p in PAT:
            m = re.search(p, line)
            if m:
                acc.setdefault(name, []).append(float(m.group(1)))
    out = {k: np.mean(v) for k, v in acc.items()}
    for k in MAX:
        if k in acc:
            out[k + ', largest snapshot'] = max(acc[k])
            out[k + ', snapshots > 10 m'] = sum(v > 10 for v in acc[k])
    return out

def lakes(d):
    return {}

def _unused(d):
    f = os.path.join(d, 'lakes.csv')
    steps = []
    for line in open(f):
        parts = line.strip().split(',')
        steps.append(set(int(p.split(':')[0]) for p in parts[1:]))
    out = {}
    for lag in (1, 10):
        kept = [len(a & b) / max(len(a), 1) for a, b in zip(steps, steps[lag:])]
        out[f'lake cells still lake after {lag} Myr (%)'] = 100 * np.mean(kept)
    # lifetime of lake cells: mean run length of consecutive steps as lake
    runs = []
    current = {}
    for s in steps:
        for c in list(current):
            if c not in s:
                runs.append(current.pop(c))
        for c in s:
            current[c] = current.get(c, 0) + 1
    out['lake-cell run length (Myr, mean)'] = np.mean(runs) if runs else 0
    out['lake cells per step'] = np.mean([len(s) for s in steps])
    return out

def seed(d):
    r = {}
    r.update(dumps(d)); r.update(logs(d)); r.update(lakes(d))
    sec = os.path.join(d, 'seconds')
    if os.path.exists(sec):
        r['run time (s)'] = float(open(sec).read())
    return r

def welch(a, b):
    a, b = np.array(a), np.array(b)
    if len(a) < 2 or len(b) < 2:
        return float('nan')
    s = np.sqrt(a.var(ddof=1) / len(a) + b.var(ddof=1) / len(b))
    return (b.mean() - a.mean()) / s if s > 0 else float('nan')

sets = []
for arg in sys.argv[1:]:
    label, d = arg.split('=', 1)
    seeds = sorted(glob.glob(os.path.join(d, 'seed-*')), key=lambda p: int(p.rsplit('-', 1)[1]))
    res = [seed(s) for s in seeds if os.path.exists(os.path.join(s, f'cells_{T1:05d}.csv'))]
    sets.append((label, res))
keys = []
for _, res in sets:
    for r in res:
        for k in r:
            if k not in keys:
                keys.append(k)
print(f'snapshots {T0}-{T1} Myr; seeds: ' + ', '.join(f'{l}: {len(r)}' for l, r in sets))
print('| metric | ' + ' | '.join(f'{l} | t' if i else l for i, (l, _) in enumerate(sets)) + ' |')
print('|---|' + '---|' * (2 * len(sets) - 1))
for k in keys:
    base = [r.get(k, np.nan) for r in sets[0][1]]
    fmt = lambda v: f'{np.nanmean(v):.3g} ({np.nanmin(v):.3g}…{np.nanmax(v):.3g})' if not np.all(np.isnan(v)) else '–'
    row = [fmt(base)]
    for l, res in sets[1:]:
        v = [r.get(k, np.nan) for r in res]
        row.append(fmt(v))
        row.append(f'{welch(base, v):+.1f}')
    print(f'| {k} | ' + ' | '.join(row) + ' |')
