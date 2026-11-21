"""Where plume rifts end up, from the debug lines of the events module.
usage: rifts.py DIR  (DIR/s*/log)"""
import glob, os, re, sys, collections
import numpy as np
KM = 6371.0
rows = []
for log in sorted(glob.glob(os.path.join(sys.argv[1], 's*', 'log'))):
    stage = None
    dist = None
    for line in open(log, errors='replace'):
        line = re.sub(r'\x1b\[[0-9;]*m', '', line)
        if 'Rift break stages' in line:
            f = dict(re.findall(r'(\w+)=([^ ]+)', line.split('Rift break stages')[1]))
            stage = f
        elif 'Rift boundary distance from the plume' in line:
            dist = dict(re.findall(r'(\w+)=([^ ]+)', line.split('from the plume')[1]))
        elif 'Plume rifted a plate' in line and stage and dist:
            f = dict(re.findall(r'(\w+)=([^ ]+)', line.split('Plume rifted a plate')[1]))
            cd = re.search(r'continent_distance=Some\(([\d.e-]+)\)', line)
            rows.append(dict(cut=float(stage['cut_distance']) * KM, rigid=float(stage['rigid_distance']) * KM,
                             final=float(dist['distance']) * KM, flipped=int(stage['flipped_cells']),
                             edge=float(f['edge']) * KM, nucleus_rigid=f['nucleus_rigid'] == 'true',
                             continent=float(cd.group(1)) * KM if cd else None, area=float(f['area']),
                             recut=stage['scale'] != '[1.0,', seed=os.path.basename(os.path.dirname(log))))
            stage = dist = None
n = len(rows)
print(f'plume rifts: {n}')
if not n: sys.exit()
far = [r for r in rows if r['final'] > 500]
print(f"closest boundary cell > 500 km from the plume: {len(far)} ({100*len(far)/n:.0f}%); median distance all {np.median([r['final'] for r in rows]):.0f} km")
def share(sel, of): return f"{len(sel)} ({100*len(sel)/max(1,len(of)):.0f}%)"
print('of the far rifts:')
a = [r for r in far if r['cut'] <= 250 and r['flipped'] > 0 and r['rigid'] <= 250]
b = [r for r in far if r['cut'] <= 250 and r['rigid'] > 250]
c = [r for r in far if r['cut'] > 250]
d = [r for r in far if r['cut'] <= 250 and r['rigid'] <= 250 and r['flipped'] == 0]
print('  cut passed within 250 km, then pieces were moved to the other side:', share(a, far))
print('  cut passed within 250 km, rigid crust assigned whole moved it away:', share(b, far))
print('  the cut itself was farther than 250 km:', share(c, far))
print('  other:', share(d, far))
for name, sel in [('far', far), ('near', [r for r in rows if r['final'] <= 500])]:
    if not sel: continue
    print(f"{name}: plume within 300 km of the plate edge {100*np.mean([r['edge'] <= 300 for r in sel]):.0f}%, median edge distance {np.median([r['edge'] for r in sel]):.0f} km, "
          f"plume under rigid crust {100*np.mean([r['nucleus_rigid'] for r in sel]):.0f}%, median cells moved {np.median([r['flipped'] for r in sel]):.0f}, "
          f"median plate area {100*np.median([r['area'] for r in sel]):.0f}%")
print('far rifts by plume distance from the plate edge: <= 150 km', share([r for r in far if r['edge'] <= 150], far), '| 150-500', share([r for r in far if 150 < r['edge'] <= 500], far), '| > 500', share([r for r in far if r['edge'] > 500], far))
print('all rifts with the plume <= 150 km from the edge:', share([r for r in rows if r['edge'] <= 150], rows), '; of those far:', share([r for r in rows if r['edge'] <= 150 and r['final'] > 500], [r for r in rows if r['edge'] <= 150]))
print('rifts with the plume > 500 km from the edge:', share([r for r in rows if r['edge'] > 500], rows), '; of those far:', share([r for r in rows if r['edge'] > 500 and r['final'] > 500], [r for r in rows if r['edge'] > 500]))
