# #142: wider flags channel, metasediment, and which flags exclude each other

Sebastian on #142 (2026-11-24): "Expand bit flags. Are there any exclusivity relations between
the flags that allow to combine some?"

## Change

- The frame history keeps 16 bits of `CellInfo::flags` (was 8): the flags channel is an
  `Integer16` like the plate channel (`Integer8` is gone). 49 instead of 48 bytes per cell raw;
  the high byte is almost always zero or unchanged, so after the delta and lz4 it costs next to
  nothing. Codec version 7 (main has 6).
- New flag `CellInfo::METASEDIMENT` (bit 8): more than half of the crystalline crust (by mass)
  is metasediment (`Crust::metasediment > 0.5`). The crust mode shows it as its own type,
  "Metasediment (accretionary wedge)", in Okabe–Ito sky blue, ahead of the other types; the
  cell widget's crust row says "metasedimentary". At 500 Myr 0.2–0.6 % of the cells qualify
  (results.md), mostly accretionary wedges one cell wide.
- Free bits now: 9–15 (7 bits).

## Which combinations occur

Code: `Crust::is_oceanic` needs crust thinner than 12 km and denser than 2850 kg/m³, whose
crustal buoyancy stays far below `CONTINENTAL_BUOYANCY` (3000), so CONTINENTAL and OCEANIC
exclude each other by construction. FELSIC is a tracer (basement felsic share > 0.5), not
derived from the other two.

Empirically: 3 seeds (0–2), subdivisions 4, a frame every 50 Myr from 0 to 400 Myr
(69 174 cell-frames), with a temporary test tallying the flags of `cell_infos`:

| continental | oceanic | felsic | cell-frames |
|---|---|---|---|
| no | yes | no | 40 216 (oceanic) |
| yes | no | yes | 24 779 (continental) |
| no | no | yes | 3 888 (thinned continental) |
| no | no | no | 290 (arc, plateau) |
| yes | no | no | 1 (thick intra-oceanic arc) |
| yes | yes | – | 0 (impossible) |
| no | yes | yes | 0 (not seen) |

So the three crust bits carry 5 states (oceanic, two transitional, two continental): that
still needs 3 bits, so packing them into a crust-class field saves nothing (it would leave
room for 3 more crust states, e.g. metasediment, in the same 3 bits).

Boundary class: 10 classes plus "none" = 11 codes, so 4 bits are needed (3 bits hold 8).
DEFORMING_BAND is independent of it: bands occur inside plates (2 207 cell-frames without a
boundary class) and with every class except subducting cells; rifts were always in a band in
this sample, ridges almost never. Observed (class, band) pairs: 20 of 22, so 5 bits.

Total: the 8 bits used before are the minimum for these states (5 + 3); no bit can be saved by
packing. The 16-bit channel is therefore needed for any further flag, and it leaves 7 bits of
headroom. Packing was not done.
