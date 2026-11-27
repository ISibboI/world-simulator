import csv,sys,math,statistics as st
# usage: gap.py RUNDIR [elev_max] ; lowland gap vs band SST, |lat|<25, 5° bands, 500..1000 Myr
d=sys.argv[1]; emax=float(sys.argv[2]) if len(sys.argv)>2 else 300
times=[int(x) for x in (sys.argv[3].split(',') if len(sys.argv)>3 else ['500','600','700','800','900','1000'])]
LAPSE=6.5e-3
res=[]
for s in range(8):
  gaps=[];gaps_red=[];coastg=[];inlg=[];wet=[];dry=[];lowT=[];seaT=[];nland=0
  for t in times:
    fn=f"{d}/seed-{s}/cells_{t:05d}.csv"
    try: rows=list(csv.DictReader(open(fn)))
    except FileNotFoundError: continue
    sea={};land=[]
    for r in rows:
      lat=float(r['lat'])
      if abs(lat)>=25: continue
      b=math.floor(lat/5)
      if r['ocean']=='1' or (float(r['elev'])<0 and float(r['lake'])==0 and r['ocean']!='0'):
        sea.setdefault(b,[]).append(float(r['t_annual']))
      elif float(r['elev'])>=0 and float(r['elev'])<emax and float(r['lake'])==0:
        land.append((b,float(r['t_annual']),float(r['elev']),r['coast'],float(r['precip'])))
    sm={b:st.mean(v) for b,v in sea.items() if len(v)>5}
    for b,T,z,c,p in land:
      if b not in sm: continue
      g=T-sm[b]; gaps.append(g); gaps_red.append(g+LAPSE*z)
      (coastg if c!='0' else inlg).append(g)
      (wet if p>=1.0 else dry if p<0.25 else []).append(g)
  if gaps:
    res.append((s,len(gaps),st.mean(gaps),st.mean(gaps_red),st.mean(coastg) if coastg else float('nan'),st.mean(inlg) if inlg else float('nan'),len(coastg)/len(gaps),st.mean(wet) if wet else float('nan'),st.mean(dry) if dry else float('nan')))
print(f"{d} elev<{emax} times {times}")
print("seed n gap gap_sealevel coast inland coastfrac wet(P>=1) dry(P<0.25)")
for r in res: print(" ".join(f"{x:.2f}" if isinstance(x,float) else str(x) for x in r))
cols=list(zip(*res))
print("mean", " ".join(f"{st.mean(c):.2f}" for c in cols[2:]))
