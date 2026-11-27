#!/usr/bin/env python3
"""Summarise escape runs: python3 analyse.py RUNDIR [RUNDIR...] (each with seed-*/log.txt)."""
import glob, re, sys, statistics as st

OCEAN_KG = 1.335e18 * 1025.0

num = r"([-+]?[0-9.]+(?:e[-+]?[0-9]+)?)"

def parse(path):
    snaps = []  # (time, global T, escape dict, escape sea level, CO2 bar)
    t = None; cur = {}
    for line in open(path, errors="replace"):
        m = re.search(r"INFO world_simulator: t =\s+([0-9.]+) Myr()", line)
        if m:
            t = float(m.group(1) or m.group(2))
        m = re.search(r"temperature global\s+" + num, line)
        if m:
            cur["T"] = float(m.group(1)); cur["t"] = t
        m = re.search(r"escape " + num + r" \| " + num + r" \| total", line)
        if m:
            cur["sl"] = float(m.group(1)); cur["slrate"] = float(m.group(2))
        m = re.search(r"carbon: pCO₂ " + num, line)
        if m:
            cur["co2"] = float(m.group(1))
        m = re.search(r"escape: tropics " + num + r" °C, stratospheric H₂O " + num + r".*?\| H " + num
                      + r" cm⁻² s⁻¹ \((\S+);.*?water " + num + r" kg/yr, lost so far " + num
                      + r" kg \| O₂ produced " + num + r" mol, sinks " + num + r", in the air " + num
                      + r" mol \(" + num + r" bar\)", line)
        if m:
            g = m.groups()
            cur.update(tropics=float(g[0]), f=float(g[1]), H=float(g[2]), regime=g[3],
                       rate=float(g[4]), lost=float(g[5]), o2prod=float(g[6]), o2sink=float(g[7]),
                       o2=float(g[8]), po2=float(g[9]))
            m2 = re.search(r"O₂ sinks " + num + r" Tmol/yr \(volcanic " + num + r" of " + num + r", seafloor "
                           + num + r" of " + num + r", weathering " + num + r" of " + num + r"\)", line)
            if m2:
                v = [float(x) for x in m2.groups()]
                cur.update(sink=v[0], volc=v[1], volc_cap=v[2], sf=v[3], sf_cap=v[4], wx=v[5], wx_cap=v[6])
            snaps.append(cur); cur = {}
    return snaps

for d in sys.argv[1:]:
    runs = {p: parse(p) for p in sorted(glob.glob(f"{d}/seed-*/log.txt"))}
    runs = {p: s for p, s in runs.items() if s}
    print(f"== {d}: {len(runs)} seeds")
    times = sorted({s["t"] for ss in runs.values() for s in ss if s.get("t") is not None})
    print(" t(Myr)  T(°C)  tropics  f_H2O     H(cm-2s-1) regime       water(kg/yr) lost(%ocean) sea level escape(m) O2 prod(mol) O2 air(bar) CO2(bar) | O2 sinks Tmol/yr: total volcanic(cap) seafloor(cap) weathering(cap)")
    for t in times:
        rows = [s for ss in runs.values() for s in ss if s.get("t") == t]
        if not rows or t not in (2, 4, 50, 100, 150, 200, 250, 300, 350, 400, 404, 500, 600, 700, 800, 900, 1000):
            continue
        m = lambda k: st.mean(r.get(k, float("nan")) for r in rows)
        reg = "/".join(sorted({r["regime"][:4] for r in rows}))
        print(f" {t:6.0f} {m('T'):6.1f} {m('tropics'):7.1f}  {m('f'):.2e}  {m('H'):.2e}  {reg:12s} {m('rate'):.2e}     {100*m('lost')/OCEAN_KG:7.3f}     {m('sl'):+8.1f}         {m('o2prod'):.2e}    {m('po2'):.2e}   {m('co2'):.3f}"
              + (f" | {m('sink'):.2f} {m('volc'):.2f}({m('volc_cap'):.2f}) {m('sf'):.2f}({m('sf_cap'):.2f}) {m('wx'):.2f}({m('wx_cap'):.2f})" if all('sink' in r for r in rows) else ""))
    last = [ss[-1] for ss in runs.values()]
    for p, ss in runs.items():
        s = ss[-1]
        loss_time = OCEAN_KG / s["rate"] / 1e9 if s["rate"] > 0 else float("inf")
        print(f"   {p.split('/')[-2]}: end T {s['T']:.1f}, lost {100*s['lost']/OCEAN_KG:.3f}% ({s['sl']:+.1f} m), "
              f"rate {s['rate']:.2e} kg/yr -> ocean in {loss_time:.1f} Gyr, O2 {s['po2']:.2e} bar, f {s['f']:.2e}")
