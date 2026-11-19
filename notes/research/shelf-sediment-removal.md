# Research: how Earth keeps excess sediment off continental shelves (2026-11-19)

Question (user): the best way to get rid of the excess sediment on shelf cells. Model state: the
sediment stock on continental-origin crust grows ~+0.85 Gt/yr (8 seeds, 500–1000 Myr); 60–81% of
it lies on land, on thinned crust < 25 km, in columns > 6 km — overfill piled as flat coastal
land once margins within reach are full (session 2026-11-19). Flat land at +2 m has no slope,
so stream-power incision (base level = sea level) never removes it; the model has no hillslope
or transport-limited diffusion, and sediment and bedrock have the same erodibility.

## Findings

1. **The sedimentary mass is near steady state through cannibalistic recycling.** Sediment
   mass decays roughly exponentially with age: half-mass age of all sedimentary rocks
   ~600 Myr (carbonates 300–400, evaporites 200–300 Myr; Garrels & Mackenzie 1969); the mass
   deposited over Earth history is 4–6× the present mass; post-Archean recycling is mostly
   sediment reworked from older sediment (Veizer & Jansen 1979, 1985); Macrostrat gives a
   preservation half-life of ~30 Myr for all sedimentary rock and ~99 Myr for non-marine
   deposits (Peters & Husson 2017). So most erosion on Earth is of sedimentary rock, not of new
   crystalline basement.
2. **Sedimentary rock erodes faster than crystalline rock.** Sedimentary rocks or unconsolidated
   sediments cover ~2/3 of the land surface, crystalline ~1/3. Cosmogenic erosion rates:
   sedimentary outcrops ~20 mm/kyr, metamorphic ~11, igneous ~8.7 (≈ 2×). Stream-power K: ~4×
   higher in Neogene sediments than in metamorphic rock in Turkey for a 2× hardness difference;
   K ∝ 1/UCS² (Takai et al. 2026, ESurf), up to two orders of magnitude across lithologies (High
   Atlas). K also depends on hydrology and channel geometry (Whipple & Tucker 1999).
3. **Shelves bypass sediment offshore, but only partly.** Shelf-slope models: high-amplitude
   sea-level change sends at most ~20% of the supply to deep water, the share depending mainly
   on shelf width; margins build out fast at first, then approach an equilibrium (AAPG 2019).
   High-supply margins bypass more ("highstand fans"); gravity flows carry most of the bypassed
   volume (Stevenson et al. 2015). Clinoform progradation of deltas is classically modelled as
   diffusion (Kenyon & Turcotte 1985; Kaufman et al. 1991; alluvial basins Paola et al. 1992);
   a measured delta diffusivity is ~4–6 × 10³ m²/yr (Chesapeake tidal delta).
4. **Flexural loading creates room.** Over tens of Myr the load of large deltas subsides
   unidirectionally, with peripheral bulges hundreds of km along shore (ESurf 2024); without
   isostatic compensation deltas prograde much further. The model has local Airy isostasy
   (already ~0.44 of a submerged layer), no flexure.

## Options for the model, ranked

1. **Higher erodibility of sediment than of bedrock** (`erodibility` × ~2–4 while incision
   cuts sediment; findings 1–2). Exposed sediment is recycled before new crust is eroded, so net
   new sediment production falls and the stock approaches a steady state, as on Earth. Cheap:
   in the implicit solve, use K_sed for the part of the cut within the sediment layer and K for
   the bedrock below it (two-stage implicit step). Caveat: no effect on perfectly flat plains
   (no slope), so combine with 2.
2. **Transport-limited diffusion of low-gradient sediment** (coastal and alluvial plains,
   shelves): flux q = −κ ∇h of the sediment layer with κ ~ 10³–10⁴ m²/yr (finding 3; over a
   110 km cell L²/κ ≈ 1–10 Myr, the step scale). It moves overfilled delta plains seaward and
   builds clinoforms naturally, replacing the ad-hoc overfill pile. It moves sediment onto the
   slope (oceanic crust), where subduction and burial metamorphism now remove it.
3. Not recommended: deleting sediment (violates the mass budget); higher shelf caps (stock grows
   further); progradation alone (tested: oceanic stock and sea-level slope rise, continental
   growth unchanged); flexure (large change, mostly affects geometry, not the long-term budget).

Suggested test: 1 alone, then 1 + 2, 6 seeds × 1000 Myr on tktie, against the continental stock
growth (target ~0), net bedrock erosion (Earth's net crystalline erosion is likely ≲ 1/3–1/2 of
the gross 14–20 Gt/yr), the sediment part of the sea level slope (≤ 10 m per 100 Myr), flooded
continental crust (25–30%), and mean land.
