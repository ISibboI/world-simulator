import re, sys, os, math, statistics as st
base = sys.argv[1]
sets = sys.argv[2:]
num = r'([-+]?[0-9.]+(?:e[-+]?[0-9]+)?)'
carb = re.compile(r'carbon: pCO₂ '+num+r' bar \('+num+r'× PAL\), forcing '+num+r' W/m² \| sources '+num+r' Tmol/yr \(ridges '+num+r', arcs '+num+r', plumes '+num+r', metamorphic '+num+r'\) \| sinks '+num+r' \(land '+num+r', seafloor '+num+r'\) \| φ (\S+), weathering intensity '+num+r', bottom water (\S+) °C, predicted ΔT '+num+r' K \| exogenic carbon '+num+r' mol.*time='+num)
snap = re.compile(r't = +'+num+r' Myr \| continental +'+num+r'% \| land +'+num+r'%')
temp = re.compile(r'temperature global +'+num+r' °C, equator +'+num+r' °C, poles +'+num+r' °C .*permanently frozen +'+num+r'%')
masses = re.compile(r'land masses \d+ \(\d+ specks, largest '+num+r'%\) \| continental crust masses \d+ \(\d+ specks, largest '+num+r'%\)')
prod = re.compile(r'sea floor production \(km²/yr\): kinematic '+num+r', created '+num)
runoff = re.compile(r'land precipitation '+num+r' m/yr.*runoff '+num+r' m/yr')
def parse(path):
    steps=[]; snaps=[]; cur=None
    for line in open(path, errors='replace'):
        m=carb.search(line)
        if m:
            g=m.groups()
            Fv=float(g[2])
            rr=math.exp(Fv/5.35) if Fv<=0 else math.exp(2*Fv/(5.35+math.sqrt(5.35**2+4*0.39*Fv)))
            steps.append(dict(p=rr*280e-6,r=rr,F=float(g[2]),D=float(g[3]),ridge=float(g[4]),arcs=float(g[5]),plumes=float(g[6]),meta=float(g[7]),W=float(g[8]),land=float(g[9]),sea=float(g[10]),phi=g[11],I=float(g[12]),bw=g[13],dT=float(g[14]),N=float(g[15]),t=float(g[16])))
            continue
        m=snap.search(line)
        if m:
            cur=dict(t=float(m.group(1)),cont=float(m.group(2)),landf=float(m.group(3))); snaps.append(cur); continue
        if cur is None: continue
        for rx,keys in ((temp,('T','Teq','Tpole','ice')),(masses,('largest_land','largest_cont')),(prod,('kin','created')),(runoff,('lprec','runoff'))):
            m=rx.search(line)
            if m:
                for k,v in zip(keys,m.groups()): cur[k]=float(v)
    return steps,snaps
def mean(x): return sum(x)/len(x) if x else float('nan')
def slope(xs,ys):
    mx,my=mean(xs),mean(ys); sxx=sum((x-mx)**2 for x in xs)
    return sum((x-mx)*(y-my) for x,y in zip(xs,ys))/sxx if sxx else float('nan')
def corr(a,b):
    ma,mb=mean(a),mean(b); sa=math.sqrt(sum((x-ma)**2 for x in a)); sb=math.sqrt(sum((x-mb)**2 for x in b))
    return sum((x-ma)*(y-mb) for x,y in zip(a,b))/(sa*sb) if sa and sb else float('nan')
for s in sets:
    print(f'===== {s}')
    rows=[]
    for seed in range(8):
        d=os.path.join(base,s,f'seed-{seed}')
        try: steps,snaps=parse(os.path.join(d,'log.txt'))
        except FileNotFoundError: continue
        secs=float(open(os.path.join(d,'seconds')).read().split()[0]) if os.path.exists(os.path.join(d,'seconds')) else float('nan')
        where=open(os.path.join(d,'where')).read().strip() if os.path.exists(os.path.join(d,'where')) else '?'
        late=[x for x in snaps if x['t']>=200]
        T=[x['T'] for x in snaps]
        st_late=[x for x in steps if x['t']>=200]
        # oscillation: sign reversals of step-to-step forcing change, with |change| > 0.05 W/m²
        dF=[b['F']-a['F'] for a,b in zip(steps,steps[1:]) if b['t']>20]
        rev=sum(1 for a,b in zip(dF,dF[1:]) if a*b<0 and abs(a)>0.05 and abs(b)>0.05)
        ac1=corr(dF[:-1],dF[1:]) if len(dF)>3 else float('nan')
        dT=[b['dT'] for b in steps if b['t']>160]
        # budget: N_end-N_after_window vs Σ (D-W) * 1e6 * 1e12
        act=[x for x in steps if x['t']>150]
        budget=float('nan')
        if len(act)>2:
            integ=sum((x['D']-x['W'])*1e18 for x in act[1:])
            budget=(act[-1]['N']-act[0]['N']-integ)/act[0]['N']
        # correlations (per step, after 100 Myr): ln r vs ridge source, detrended by 100-Myr running mean
        def detrend(v,w=100):
            out=[]
            for i in range(len(v)):
                lo=max(0,i-w//2); hi=min(len(v),i+w//2)
                out.append(v[i]-mean(v[lo:hi]))
            return out
        ss=[x for x in steps if x['t']>=100]
        lr=detrend([math.log(x['r']) for x in ss]); rd=detrend([x['ridge'] for x in ss]); ld=detrend([x['land'] for x in ss]); dg=detrend([x['D'] for x in ss])
        lag={L: corr(lr[L:],dg[:len(dg)-L]) for L in (0,5,10,20)}
        # snapshot correlation: ln r vs largest land mass share, land fraction
        sl=[x for x in snaps if x['t']>=100 and 'largest_land' in x]
        rmap={round(x['t']):x['r'] for x in steps}
        lr_s=[math.log(rmap.get(round(x['t']),float('nan'))) for x in sl]
        rows.append(dict(seed=seed,secs=secs,where=where,T0=snaps[0]['T'],T1=snaps[-1]['T'],Tl=mean([x['T'] for x in late]),Tmin=min(T),Tmax=max(T),
            drift=slope([x['t'] for x in late],[x['T'] for x in late])*1000,drift4=slope([x['t'] for x in snaps if x['t']>=400],[x['T'] for x in snaps if x['t']>=400])*1000,ac1=ac1,ice0=snaps[0]['ice'],icel=mean([x['ice'] for x in late]),icemax=max(x['ice'] for x in snaps),ice1=snaps[-1]['ice'],
            p0=steps[0]['p'] if steps else 0,p1=steps[-1]['p'],pmin=min(x['p'] for x in steps),pmax=max(x['p'] for x in steps),Fl=mean([x['F'] for x in st_late]),
            phi=steps[-1]['phi'],D=mean([x['D'] for x in st_late]),ridge=mean([x['ridge'] for x in st_late]),arcs=mean([x['arcs'] for x in st_late]),plumes=mean([x['plumes'] for x in st_late]),meta=mean([x['meta'] for x in st_late]),
            land=mean([x['land'] for x in st_late]),sea=mean([x['sea'] for x in st_late]),I=mean([x['I'] for x in st_late]),rev=rev,budget=budget,lag=lag,
            c_land=corr(lr_s,[x['largest_land'] for x in sl]),c_landf=corr(lr_s,[x['landf'] for x in sl]),c_prod=corr(lr_s,[x.get('created',float('nan')) for x in sl]),
            series=[(x['t'],x['T'],x['ice'],x.get('largest_land',float('nan')),x['landf']) for x in snaps], pser=[(x['t'],x['p'],x['F'],x['D'],x['land'],x['sea']) for x in steps if x['t']%50==0 or x['t'] in (1,10,20,30,75,125,175)]))
    if not rows: continue
    keys=['T0','T1','Tl','Tmin','Tmax','drift','drift4','ac1','ice0','icel','icemax','ice1','p0','p1','pmin','pmax','Fl','D','ridge','arcs','plumes','meta','land','sea','I','rev','budget','c_land','c_landf','c_prod','secs']
    for r in rows:
        print(f"seed {r['seed']} ({r['where']}): phi {r['phi']} " + ' '.join(f"{k}={r[k]:.3g}" for k in keys) + ' lag ' + ' '.join(f"{L}:{v:.2f}" for L,v in r['lag'].items()))
    print('MEAN ' + ' '.join(f"{k}={mean([r[k] for r in rows]):.3g}" for k in keys))
    print('MIN  ' + ' '.join(f"{k}={min(r[k] for r in rows):.3g}" for k in keys))
    print('MAX  ' + ' '.join(f"{k}={max(r[k] for r in rows):.3g}" for k in keys))
    # mean series
    ts=[x[0] for x in rows[0]['series']]
    print('t: T ice largest_land landf (seed means)')
    for i,t in enumerate(ts):
        vals=[r['series'][i] for r in rows if len(r['series'])>i]
        print(f"  {t:6.0f} {mean([v[1] for v in vals]):6.2f} {mean([v[2] for v in vals]):5.2f} {mean([v[3] for v in vals]):5.1f} {mean([v[4] for v in vals]):5.1f}")
    print('t: pCO2 bar  F  D  land seafloor (seed means)')
    for i in range(len(rows[0]['pser'])):
        vals=[r['pser'][i] for r in rows if len(r['pser'])>i]
        print(f"  {vals[0][0]:6.0f} {mean([v[1] for v in vals]):.4g} {mean([v[2] for v in vals]):6.2f} {mean([v[3] for v in vals]):6.2f} {mean([v[4] for v in vals]):6.2f} {mean([v[5] for v in vals]):6.2f}")
