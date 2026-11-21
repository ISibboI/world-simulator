#!/usr/bin/env python3
"""Earth's plate-boundary types from PB2002 (Bird 2003), at the original resolution and coarsened.

Usage: pb2002_classes.py PB2002_steps.dat.txt

Prints boundary length (10^3 km) and share per class
  * by Bird's own class (OSR, OTF, CRB, CTF, OCB, CCB, SUB), and
  * by the classification used for the model (see investigation.md, section 2): obliquity of the
    relative velocity against the boundary, recomputed from the step azimuths at the original
    resolution (~45 km steps) and after re-digitising every boundary with chords of L km, to see
    how much transform length a coarse boundary (model cells ~110 km) can still show.
Speeds are full relative speeds in cm/yr.
"""
import math
import sys
from collections import defaultdict

R = 6371.0
# Weight re-digitised chords by their chord length (as the model measures a boundary) instead of
# the summed original step lengths; set by --chord-length.
CHORD_LENGTH = "--chord-length" in sys.argv


def parse(path):
    steps = []
    for line in open(path):
        f = line.split()
        if len(f) < 14:
            continue
        pair = f[1].lstrip(":")
        cont = f[1].startswith(":")
        lon1, lat1, lon2, lat2 = map(float, f[2:6])
        length = float(f[6])
        step_az = float(f[7])
        v = float(f[8]) / 10.0  # mm/yr -> cm/yr
        v_az = float(f[9])
        div = float(f[10]) / 10.0
        rl = float(f[11]) / 10.0
        cls = f[-1].strip(":*")
        orogen = "*" in f[-1]
        steps.append(dict(pair=pair, cont=cont, p1=(lon1, lat1), p2=(lon2, lat2), length=length,
                          step_az=step_az, v=v, v_az=v_az, div=div, rl=rl, cls=cls, orogen=orogen))
    return steps


def xyz(lon, lat):
    lo, la = math.radians(lon), math.radians(lat)
    return (math.cos(la) * math.cos(lo), math.cos(la) * math.sin(lo), math.sin(la))


def azimuth(p1, p2):
    lo1, la1, lo2, la2 = map(math.radians, (*p1, *p2))
    y = math.sin(lo2 - lo1) * math.cos(la2)
    x = math.cos(la1) * math.sin(la2) - math.sin(la1) * math.cos(la2) * math.cos(lo2 - lo1)
    return math.degrees(math.atan2(y, x)) % 360


def gc_dist(p1, p2):
    a, b = xyz(*p1), xyz(*p2)
    d = sum(x * y for x, y in zip(a, b))
    return R * math.acos(max(-1.0, min(1.0, d)))


def classify(div, rl, crust, sub):
    """Model classification: transform if the velocity is within 20 deg of the strike (Bird's
    rule), else divergent/convergent by the sign of the normal component; convergent split by
    crust; 'slow' is reported separately as a speed band, not a class."""
    v = math.hypot(div, rl)
    angle_from_strike = math.degrees(math.atan2(abs(div), abs(rl))) if v > 0 else 0.0
    if sub:
        return "SUB"
    if angle_from_strike < 20:
        return "transform-" + crust
    if div > 0:
        return "divergent-" + crust
    return "convergent-" + crust


def report(title, items):
    """items: (class, length, speed, normal component)"""
    total = sum(i[1] for i in items)
    by = defaultdict(float)
    sp = defaultdict(list)
    for c, l, v, n in items:
        by[c] += l
        sp[c].append((v, l))
    print(f"\n## {title}: total {total/1e3:.1f} thousand km")
    print("| class | length (10^3 km) | share | median speed | < 2 cm/yr |")
    print("|---|---|---|---|---|")
    for c in sorted(by, key=lambda c: -by[c]):
        vs = sorted(sp[c])
        acc, med = 0.0, 0.0
        for v, l in vs:
            acc += l
            if acc >= by[c] / 2:
                med = v
                break
        slow = sum(l for v, l in vs if v < 2) / by[c]
        print(f"| {c} | {by[c]/1e3:.1f} | {100*by[c]/total:.0f}% | {med:.1f} | {100*slow:.0f}% |")
    allv = sorted((v, l) for _, l, v, _ in items)
    acc = 0
    for v, l in allv:
        acc += l
        if acc >= total / 2:
            print(f"all: median speed {v:.1f} cm/yr, < 2 cm/yr {100*sum(l for v,l in allv if v<2)/total:.0f}%,"
                  f" mean |normal| {sum(abs(n)*l for _,l,_,n in items)/total:.2f} cm/yr")
            break


def polylines(steps):
    lines, cur = [], []
    for s in steps:
        if cur and (not s["cont"] or s["pair"] != cur[-1]["pair"]):
            lines.append(cur)
            cur = []
        cur.append(s)
    if cur:
        lines.append(cur)
    return lines


def coarsen(steps, chord):
    """Re-digitise each boundary polyline with chords of about `chord` km; the velocity of a chord
    is the length-weighted mean velocity vector of its steps, its crust/subduction attribute the
    length-weighted majority."""
    out = []
    for line in polylines(steps):
        total = sum(s["length"] for s in line)
        n = max(1, round(total / chord))
        target = total / n
        groups, g, acc = [], [], 0.0
        for s in line:
            g.append(s)
            acc += s["length"]
            if acc >= target * 0.999 and len(groups) < n - 1:
                groups.append(g)
                g, acc = [], 0.0
        if g:
            groups.append(g)
        for g in groups:
            length = sum(s["length"] for s in g)
            ve = sum(s["v"] * math.sin(math.radians(s["v_az"])) * s["length"] for s in g) / length
            vn = sum(s["v"] * math.cos(math.radians(s["v_az"])) * s["length"] for s in g) / length
            v = math.hypot(ve, vn)
            v_az = math.degrees(math.atan2(ve, vn))
            az = azimuth(g[0]["p1"], g[-1]["p2"])
            div = v * math.sin(math.radians(az - v_az))
            rl = v * math.cos(math.radians(v_az - az))
            w = defaultdict(float)
            for s in g:
                w[attr(s)] += s["length"]
            a = max(w, key=w.get)
            out.append((a, gc_dist(g[0]["p1"], g[-1]["p2"]) if CHORD_LENGTH else length, v, div, rl))
    return out


def attr(s):
    if s["cls"] == "SUB":
        return "SUB"
    return "ocean" if s["cls"].startswith("O") else "continent"


def main():
    steps = parse([a for a in sys.argv[1:] if not a.startswith("--")][0])
    report("PB2002, Bird's classes", [(s["cls"], s["length"], s["v"], s["div"]) for s in steps])
    report("PB2002, Bird's classes, steps outside orogens only",
           [(s["cls"], s["length"], s["v"], s["div"]) for s in steps if not s["orogen"]])
    print(f"\nlength in orogens (diffuse zones): {sum(s['length'] for s in steps if s['orogen'])/1e3:.1f} thousand km")
    for chord in (0, 110, 220, 440):
        if chord == 0:
            items = [(attr(s), s["length"], s["v"], s["div"], s["rl"]) for s in steps]
            name = "original steps (mean 45 km)"
        else:
            items = coarsen(steps, chord)
            name = f"re-digitised with ~{chord} km chords"
        cls = []
        for a, l, v, div, rl in items:
            crust = "ocean" if a == "ocean" else "continent"
            cls.append((classify(div, rl, crust, a == "SUB"), l, v, div))
        report("PB2002, model classification, " + name, cls)
        # obliquity distribution of non-subduction boundaries
        hist = [0.0] * 9
        for a, l, v, div, rl in items:
            if a == "SUB" or v == 0:
                continue
            ang = math.degrees(math.atan2(abs(div), abs(rl)))
            hist[min(8, int(ang // 10))] += l
        hs = [0.0] * 9
        for a, l, v, div, rl in items:
            if a == "SUB" and v > 0:
                hs[min(8, int(math.degrees(math.atan2(abs(div), abs(rl))) // 10))] += l
        print("SUB length by angle from strike (10 deg bins): " + " ".join(f"{100*h/sum(hs):.0f}" for h in hs))
        t = sum(hist)
        print("non-SUB length by angle from strike (10 deg bins, 0 = pure strike-slip): "
              + " ".join(f"{100*h/t:.0f}" for h in hist))


if __name__ == "__main__":
    main()
