import re, sys, os, statistics as st
R=os.path.expanduser(os.environ.get('RUNS','~/tasks/issue-207/runs'))
num=r'([-+]?\d+\.?\d*|NaN)'
def parse(path):
    snaps=[]; cur=None
    for line in open(path, errors='replace'):
        m=re.search(r'temperature global\s+'+num+r' °C, equator\s+'+num+r' °C, poles\s+'+num+r' °C.*permanently frozen\s+'+num+r'%',line)
        if m:
            cur=dict(T=float(m[1]),Teq=float(m[2]),Tpole=float(m[3]),frozen=float(m[4])); snaps.append(cur); continue
        m=re.search(r'snow on land: cover north Jan/Apr/Jul/Oct (.*?)%, south (.*?)% \| perennial '+num+r'% of land \(ice sheet without it '+num+r'%, it without ice sheet '+num+r'%\) \| reflects '+num+r' W/m² \| stored '+num+r' km³ \('+num+r' m.*warmest month < 0 °C on '+num+r'% of land, '+num+r'% of it',line)
        if m and cur is not None:
            n=[float(x) for x in m[1].split('/')]; s=[float(x) for x in m[2].split('/')]
            cur.update(nJan=n[0],nApr=n[1],nJul=n[2],nOct=n[3],sJan=s[0],sJul=s[2],per=float(m[3]),sheetonly=float(m[4]),peronly=float(m[5]),refl=float(m[6]),stor=float(m[7]),sl=float(m[8]),fz=float(m[9]),fzno=float(m[10]))
            continue
        m=re.search(r'^\S+\s+INFO world_simulator:\s+carbon: pCO₂ '+num+r' bar.*?forcing '+num+r' W/m².*φ '+num,line)
        if m and cur is not None:
            cur.update(co2=float(m[1]),F=float(m[2]),phi=float(m[3]))
    return snaps
def group(name):
    d=os.path.join(R,name); out={}
    for s in sorted(os.listdir(d)):
        if s.startswith('seed-'):
            snaps=parse(os.path.join(d,s,'log.txt'))
            sec=open(os.path.join(d,s,'seconds')).read().strip()
            out[int(s[5:])]=(snaps,float(sec))
    return out
keys=['T','Teq','Tpole','frozen','nJan','nJul','sJan','sJul','per','refl','stor','sl','fz','fzno','sheetonly','peronly','co2','F','phi']
def summarize(name):
    g=group(name)
    def mean_over(sel):
        vals={k:[] for k in keys}
        for seed,(snaps,_) in g.items():
            for i,sn in enumerate(snaps):
                t=50*i
                if sel(t):
                    for k in keys:
                        v=sn.get(k)
                        if v is not None and v==v: vals[k].append(v)
        return {k:(st.mean(v) if v else float('nan')) for k,v in vals.items()}
    late=mean_over(lambda t:t>=200)
    t0=mean_over(lambda t:t==0); t1000=mean_over(lambda t:t==1000)
    seedT=[st.mean([sn['T'] for i,sn in enumerate(sn_) if 50*i>=200]) for sn_,_ in g.values()]
    minT=min(sn['T'] for sn_,_ in g.values() for sn in sn_)
    maxF=max(sn['frozen'] for sn_,_ in g.values() for sn in sn_)
    secs=[s for _,s in g.values()]
    return dict(late=late,t0=t0,t1000=t1000,seedT=(min(seedT),max(seedT)),minT=minT,maxF=maxF,sec=st.mean(secs),n=len(g),nsnap=[len(s) for s,_ in g.values()])
for name in sys.argv[1:]:
    r=summarize(name)
    print(f"== {name}: n={r['n']} snaps={set(r['nsnap'])} runtime {r['sec']:.0f}s  min global T {r['minT']:.1f}  max frozen {r['maxF']:.1f}%  seed means T(200-1000) {r['seedT'][0]:.1f}..{r['seedT'][1]:.1f}")
    for lab in ['t0','late','t1000']:
        v=r[lab]
        print(f"  {lab:5s} T {v['T']:.1f} eq {v['Teq']:.1f} pole {v['Tpole']:.1f} frozen {v['frozen']:.1f}% | snow N Jan/Jul {v['nJan']:.1f}/{v['nJul']:.1f} S Jan/Jul {v['sJan']:.1f}/{v['sJul']:.1f} perennial {v['per']:.1f} refl {v['refl']:.2f} W/m2 stored {v['stor']:.0f} km3 ({v['sl']:.3f} m) | wm<0 {v['fz']:.1f}% of land, ice<0.5 on {v['fzno']:.0f}% | sheet-only {v['sheetonly']:.1f} per-only {v['peronly']:.1f} | CO2 {v['co2']:.4f} bar F {v['F']:.1f} phi {v['phi']:.2f}")
