"""Deformation statistics from the `deformation:` log lines (#70).
usage: deformation.py T0 T1 label=DIR [label=DIR ...]   (DIR holds seed-S/log.txt)
Means over seeds of the means over the snapshots in [T0, T1]."""
import glob, os, re, statistics as st, sys

PAT = {
    'deforming >= 1e-16/s (% of surface)': r'strain rate ≥ 1e-16/s ([\d.]+)% of the surface',
    '... on continental crust (% of surface)': r'\(continental ([\d.]+)%',
    '... share of continental crust (%)': r'([\d.]+)% of \\?the continental crust',
    '... plate interiors (% of surface)': r'plate interiors ([\d.]+)%',
    'deforming >= 1e-15/s (% of surface)': r'≥ 1e-15/s ([\d.]+)%',
    'weak samples': r'weak samples (\d+)',
    'rigid blocks': r'blocks (\d+)',
    'deformation speed rms (cm/yr)': r'speed rms ([\d.]+)',
    'deformation speed max (cm/yr)': r'speed rms [\d.]+ max ([\d.]+)',
    'CG iterations per solve': r'\(residual [^;]*; ([\d.]+) per solve',
    'collision convergence, rotations (cm/yr)': r'collisions converge at ([-\d.]+) cm/yr',
    'collision convergence, at the boundary (cm/yr)': r'rotations, ([-\d.]+) cm/yr at the boundary',
}
args = sys.argv[1:]
t0, t1 = float(args[0]), float(args[1])
sets = [a.split('=', 1) for a in args[2:]]
print('| metric | ' + ' | '.join(l for l, _ in sets) + ' |')
print('|---' * (len(sets) + 1) + '|')
for name, pattern in PAT.items():
    cells = []
    for label, base in sets:
        seeds = []
        for log in sorted(glob.glob(os.path.join(base, 'seed-*', 'log.txt'))):
            values, time = [], None
            for line in open(log, encoding='utf-8'):
                m = re.search(r't = +([\d.]+) Myr', line)
                if m:
                    time = float(m.group(1))
                if 'deformation:' in line and time is not None and t0 <= time <= t1:
                    m = re.search(pattern, line)
                    if m:
                        values.append(float(m.group(1)))
            if values:
                seeds.append(st.mean(values))
        cells.append(f'{st.mean(seeds):.2f} ({min(seeds):.2f}…{max(seeds):.2f})' if seeds else '—')
    print(f'| {name} | ' + ' | '.join(cells) + ' |')
