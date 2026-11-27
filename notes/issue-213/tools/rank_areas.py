import sys, collections
ranks=collections.defaultdict(list); counts=[]
for f in sys.argv[1:]:
    for line in open(f):
        parts=line.strip().split(',')
        t=float(parts[0])
        if t<300: continue
        ids=parts[1:]; n=len(ids)
        c=collections.Counter(ids)
        a=sorted((v/n for v in c.values()), reverse=True)
        counts.append((len(a), sum(1 for x in a if x>=0.01), 1/sum(x*x for x in a)))
        for r,x in enumerate(a): ranks[r].append(x)
import statistics as st
print("maps",len(counts),"plates",st.mean(c[0] for c in counts),"above1%",st.mean(c[1] for c in counts),"eff",st.mean(c[2] for c in counts), "eff sd", st.pstdev([c[2] for c in counts]))
for r in sorted(ranks):
    v=ranks[r]
    print(r+1, len(v), round(st.mean(v)*100,2), round(st.median(v)*100,2))
