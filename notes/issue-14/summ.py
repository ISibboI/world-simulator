import json, sys, numpy as np
from collections import defaultdict
for run in sys.argv[1:]:
    R = json.load(open(f'runs/{run}/ancient.json'))
    print(f'===== {run}: {len(R)} snapshots (500-1000 Myr)')
    by = defaultdict(list)
    for r in R: by[r['seed']].append(r)
    def per_seed(f):
        return np.array([np.mean([f(r) for r in by[s]]) for s in sorted(by)])
    def show(name, f, fmt='{:.2f}'):
        v = per_seed(f)
        print(f'{name:55s} mean ' + fmt.format(np.nanmean(v)) + '  seeds ' + ' '.join(fmt.format(x) for x in v))
    show('ancient >=200 % of sea floor', lambda r: 100*r['anc']/r['sf'])
    show('>=180 % of sea floor', lambda r: 100*r['a180']/r['sf'])
    show('>=200 % of all oceanic basement', lambda r: 100*r['anc_basement']/r['oc'])
    show('ancient sea floor Mkm2', lambda r: r['anc']*0.01245)
    show('sed share of >=200 basement in oceanic sed %', lambda r: 100*r['sed_share_anc_basement'], '{:.0f}')
    show('sed share of >=200 sea floor in sea-floor sed %', lambda r: 100*r['sed_share_anc_sf'], '{:.0f}')
    show('bulk on ancient sf mean km', lambda r: r['bulk_anc_mean']/1000)
    show('bulk on ancient sf median km', lambda r: r['bulk_anc_med']/1000)
    show('bulk on sf <200 mean km', lambda r: r['bulk_young_sf_mean']/1000)
    show('max sea-floor age', lambda r: r['max_age'], '{:.0f}')
    tot = np.array([sum(r['age_anc']) for r in R], float)
    a = np.array([r['age_anc'] for r in R], float).sum(0); print('ancient age split 200-300/300-500/>500 %:', np.round(100*a/a.sum(),0))
    show('ancient within 3 rings of non-oceanic crust %', lambda r: 100*r['near_cont'], '{:.0f}')
    show('sed of ancient within 3 rings %', lambda r: 100*r['sed_near_cont'], '{:.0f}')
    show('median rings to continent', lambda r: r['d_cont_med'], '{:.1f}')
    show('median rings to boundary', lambda r: r['d_bnd_med'], '{:.1f}')
    show('ancient with no own trench %', lambda r: 100*r['no_own_trench'], '{:.0f}')
    show('ancient > 5 rings from own trench %', lambda r: 100*r['far_trench'], '{:.0f}')
    show('on plates >=10% continental %', lambda r: 100*r['on_cont_plate'], '{:.0f}')
    show('plate area median % surface', lambda r: 100*r['plate_area_med']/40962, '{:.0f}')
    cats = defaultdict(lambda: [0,0.0,0.0]); n=0
    for r in R:
        n += r['anc']
        for k,(c,sh,b) in r['cats'].items():
            cats[k][0]+=c; cats[k][1]+=sh*r['anc']; cats[k][2]+=b*c
    # sed-share weighting approx: weight by anc cells
    print('categories (share of ancient cells, ~sed share, mean bulk km):')
    for k,(c,sh,b) in sorted(cats.items()):
        print(f'   {k:60s} {100*c/n:5.1f}%  sed~{100*sh/n:5.1f}%  {b/c/1000:5.2f} km')
    def agg(key):
        return np.nanmean(np.array([r[key] for r in R]),0)
    print('sf age>100 bulk p10/25/50/75/90 m', np.round(agg('old100')), 'n', np.mean([r['old100_n'] for r in R]))
    print(' ... >=5 rings from continent  ', np.round(agg('old100_abyssal')), 'n', np.mean([r['old100_abyssal_n'] for r in R]))
    print(' ... 100-200 Myr, abyssal      ', np.round(agg('old100_100_200_abyssal')))
    show('abyssal >100 Myr in 0.2-1.5 km %', lambda r: 100*r['frac_blanket'], '{:.0f}')
    print('sf age<50 abyssal bulk p10..p90', np.round(agg('young_abyssal_bulk')))
    show('sf mean bulk km', lambda r: r['sf_bulk_mean']/1000)
