import csv, glob, os, sys, statistics as st
conv_ages, ridge_ages, convs = [], [], []
late = [0, 0]
triple = [0, 0]
for cp in sorted(glob.glob("seed-*/**/cells_*.csv", recursive=True)):
    t = int(os.path.basename(cp)[6:11])
    if t == 0: continue
    age = {}
    for r in csv.DictReader(open(cp)):
        if r["cls"] == "2" and float(r["age"]) < 3: age[int(r["cell"])] = float(r["age"])
    seen, rid, rows = set(), set(), {}
    for r in csv.DictReader(open(cp.replace("cells_", "boundaries_"))):
        c = int(r["cell"])
        if c not in age: continue
        rows.setdefault(c, []).append(r)
    for c, rs in rows.items():
        cv = max(float(r["conv"]) for r in rs)
        if cv > 0:
            conv_ages.append(age[c]); convs.append(cv)
            # also borders a ridge/diverging partner? (triple junction / ridge-trench)
            triple[0] += 1
            if any(float(r["conv"]) < 0 for r in rs): triple[1] += 1
        elif any(r["kind"] == "r" for r in rs):
            ridge_ages.append(age[c])
    if t >= 250:
        late[0] += len(age); late[1] += sum(1 for c, rs in rows.items() if max(float(r["conv"]) for r in rs) > 0)
f = lambda a, x: 100 * sum(1 for v in a if v < x) / len(a)
print(f"convergent young: n={len(conv_ages)}, age<1: {f(conv_ages,1):.0f}%, median age {st.median(conv_ages):.1f}, median conv {st.median(convs):.2f} cm/yr, >2cm/yr {100*sum(1 for v in convs if v>2)/len(convs):.0f}%")
print(f"ridge young: n={len(ridge_ages)}, age<1: {f(ridge_ages,1):.0f}%, median age {st.median(ridge_ages):.1f}")
print(f"convergent young cells that also border a diverging plate (triple junctions): {100*triple[1]/triple[0]:.0f}%")
print(f"late window 250-300 Myr: {100*late[1]/late[0]:.2f}%")
