# #150: `--epoch` counts forwards

Sebastian's decision 3 on #148: the epoch grows with time; 0 is the earliest possible start of
a simulation, larger values are later starts, through today into the future, until the sun
leaves the main sequence.

## Mapping

| `--epoch` | time | age of the sun | S/S₀ (start) | default greenhouse forcing |
|---|---|---|---|---|
| 0 | 4.0 Ga (earliest start) | 0.57 Gyr | 0.741 | +61.8 W/m² |
| 0.5 | 3.5 Ga (Archean; old `--epoch 3.5`) | 1.07 Gyr | 0.766 | +55.9 W/m² |
| 3 | 1.0 Ga | 3.57 Gyr | 0.92 | +19 W/m² |
| **4 (default)** | **today** | 4.57 Gyr | 1 | 0 |
| 5 | +1 Gyr | 5.57 Gyr | 1.096 | −17.8 W/m² (floor) |
| 9.43 (max) | +5.43 Gyr | 10 Gyr (end of the sun model) | 1.91 | −17.8 W/m² (floor) |

Constants in `src/sun.rs`: `PRESENT_EPOCH = 4.0`, `EPOCH_ZERO_AGE = SOLAR_AGE − PRESENT_EPOCH =
0.57 Gyr`, `MAX_EPOCH = MAX_AGE − EPOCH_ZERO_AGE = 9.43`. Valid range `0..=9.43`; the old range
was `0..4.57` Ga before present. **Old value `x` = new value `4 − x`.**

### Why epoch 0 = 4.0 Ga (sun age 0.57 Gyr)

- The sun model starts at the zero-age main sequence (age 0), roughly when the planet formed; a
  planet then is molten (magma ocean, giant impacts, a Moon-forming impact at ~30–100 Myr).
  The simulation needs a solid lithosphere with plates and a liquid ocean, so its earliest
  start is later than the sun model's start.
- Liquid water existed by 4.4–4.3 Ga (Jack Hills zircons; Wilde et al., 2001; Mojzsis et al.,
  2001), but the oldest preserved crust is 4.03 Gyr old (Acasta Gneiss; Bowring & Williams,
  1999) and the end of the Hadean is conventionally put at 4.0 Ga. When plate tectonics in a
  modern sense started is debated (from > 4 Ga to ~3 Ga; e.g. Korenaga, 2013). 4.0 Ga, a planet
  age of ~0.55 Gyr, is a defensible "cooled enough for plates and oceans" date for an Earth-like
  planet, and #148 already called `--epoch 4.0` (old numbering) "first oceans/plates".
- It makes **today exactly 4** and the conversion from the old numbering `4 − x`, easy to
  remember. All values with one decimal convert exactly in floating point (0.5 ↔ 3.5, 0 ↔ 4, 4 ↔ 0); some with two decimals differ in the last bit (e.g. old 0.03 vs new 3.97).

### End of the range

The sun model ends at 10 Gyr (`MAX_AGE`), before the sun leaves the main sequence (10–11 Gyr,
Schröder & Smith, 2008), so the last epoch is 9.43, 5.43 Gyr from today.

## Code

- `sun::age_of_run(epoch, time)` is still the only conversion of the epoch to the sun's age
  (climate and view). It computes `SOLAR_AGE − (PRESENT_EPOCH − epoch) + time/1000`, i.e. through
  the time before present as before, so a run keeps its old results **bit for bit** when its
  epoch is renumbered (checked below).
- `presets::Epoch { gyr, co2 }` (was `before_present`), `Epoch::PRESENT` = epoch 4,
  `TemperatureParams::epoch` defaults to 4, the CLI default is `sun::PRESENT_EPOCH` (still
  today; #190 changes the default start).
- Future epochs: the default greenhouse forcing offsets the brighter sun only down to a CO₂ floor
  of 10 ppm (the C4 photosynthesis limit, Caldeira & Kasting, 1992; `5.35 ln(10/280)` = −17.8
  W/m², reached at epoch ~4.8). Without the floor the default would be a −200 W/m² "anti-
  greenhouse" at epoch 9.43, which no atmosphere can provide. So future planets warm.
- A warning when the sun at the run's end is more than 1.1 × today's (epoch + duration > ~5.0): a real ocean planet
  would enter a moist greenhouse and lose its water (Kasting, 1988; Kopparapu et al., 2013); the
  model has no water loss.
- View: `Sun::of_run` / `SunView::new` take the new epoch unchanged (through `age_of_run`).
- Log: the presets line shows `epoch=… ga=…` (Ga before present; negative in the future).
- Docs: CLI help of `--epoch` (also shown in the view's parameter panel), module docs of
  `sun` and `presets`, README paragraph.

## Compatibility

- Only the page URL (`epoch=`) and command lines carry the epoch; the view's saved settings
  (time bar, render mode) do not. The panel and the URL leave defaults out, so links made with
  the default epoch are unaffected.
- Old links and command lines with an explicit `epoch=x` now mean epoch `x` in the new
  numbering: old `epoch=3.5` (the Archean) now starts 3.5 Gyr after the earliest start, i.e.
  0.5 Ga, a different planet. No shim: the old and new ranges overlap (0..4.57 vs 0..9.43), so
  an old value cannot be recognised. The conversion is `4 − x` (README, notes). Simple and
  correct beats a heuristic; such links are rare (the epoch is recent and not the default).
- Old values above 4 (4.0–4.57 Ga, before the earliest start) map to negative epochs and are
  rejected with an error.

## Results

Quick headless runs (`--subdivisions 4 --duration 30`, seed 0), state hashes of all three
snapshots compared between `main` (1418167) and the branch:

| main | branch | state hashes |
|---|---|---|
| `--epoch 3.5` | `--epoch 0.5` | identical (… `d62c54507d210d90`) |
| default (`0`) | default (`4`) | identical (… `26c99dcf74053d1e`) |
| `--epoch 4.0` | `--epoch 0` | identical |

Future epochs run (30 Myr): epoch 7 ends at a global mean of 49.7 °C, epoch 9.43 at 113 °C, both
ice-free, with the moist-greenhouse warning. The model keeps liquid oceans there, which is
unphysical (no water loss, no boiling, the linear outgoing radiation of the energy balance
model); the range is offered because the issue asks for it.
