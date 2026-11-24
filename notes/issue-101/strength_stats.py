import csv, sys, math, statistics as st
root=sys.argv[1]
def pct(v,qs=(10,50,90)):
    v=sorted(v); n=len(v)
    return "/".join(f"{v[min(n-1,int(q/100*n))]:.2f}" for q in qs) if v else "-"
for seed in range(4):
  for t in ("00100","00500"):
    cells=list(csv.DictReader(open(f"{root}/seed-{seed}/cells_{t}.csv")))
    bkind={}
    for r in csv.DictReader(open(f"{root}/seed-{seed}/boundaries_{t}.csv")):
        bkind.setdefault(int(r["cell"]),set()).add(r["kind"])
    groups={}
    cause_n={"young":0,"plume":0,"thinned":0,"orogenic":0}
    weak=0; s_all=[]
    for r in cells:
        s=float(r["strength"]); s_all.append(s)
        cls={"0":"cont","1":"trans","2":"ocean"}[r["cls"]]
        age=float(r["age"]); sa=float(r["strength_age"]); og=float(r["orogenic"])
        th=math.sqrt(min(max(sa,0)/100,1)); su=1-0.75*og
        hn=s/(th*su) if th*su>0 and s>0.0501 else 1.0
        groups.setdefault(cls,[]).append(s)
        c=int(r["cell"])
        for k in bkind.get(c,()): groups.setdefault("bnd_"+k,[]).append(s)
        if og>=0.25: groups.setdefault("orog>=.25",[]).append(s)
        if sa<age-0.5: groups.setdefault("plume-reset",[]).append(s)
        if s<0.75:
            weak+=1
            m=min(th,hn,su)
            if m==th: cause_n["plume" if sa<age-0.5 else "young"]+=1
            elif m==hn: cause_n["thinned"]+=1
            else: cause_n["orogenic"]+=1
    n=len(cells)
    print(f"seed {seed} t={int(t)}: strength p10/50/90 {pct(s_all)}; <0.5 {100*sum(x<0.5 for x in s_all)/n:.1f}%, <0.75 {100*weak/n:.1f}%; weak by cause "+", ".join(f"{k} {100*v/max(weak,1):.0f}%" for k,v in cause_n.items()))
    for g in sorted(groups):
        v=groups[g]; print(f"   {g:12s} n={len(v):6d} {100*len(v)/n:5.1f}%  p10/50/90 {pct(v)}  <0.5 {100*sum(x<0.5 for x in v)/len(v):4.0f}%")

# Continental cells on collision boundaries at full strength (>= 0.95), t = 500 Myr.
for seed in range(4):
    cells={r["cell"]:r for r in csv.DictReader(open(f"{root}/seed-{seed}/cells_00500.csv"))}
    col=set(r["cell"] for r in csv.DictReader(open(f"{root}/seed-{seed}/boundaries_00500.csv")) if r["kind"]=="c")
    cont=[cells[c] for c in col if cells[c]["cls"]=="0"]
    strong=[r for r in cont if float(r["strength"])>=0.95]
    never=[r for r in strong if r["orogeny"]==""]
    th=sorted(float(r["thick"]) for r in strong)
    print(f"seed {seed}: collision cells {len(col)}, continental {len(cont)}, strong {len(strong)} ({100*len(strong)/max(len(cont),1):.0f}%), never orogenic {len(never)}, median thickness {th[len(th)//2]/1000 if th else 0:.0f} km")
