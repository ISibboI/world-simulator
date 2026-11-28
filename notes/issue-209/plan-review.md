# #209: plan review

An independent agent reviewed `plan.md` before the implementation. Verdict: sound, with fixes.
Findings and what was done about them:

| # | Finding | Done |
| --- | --- | --- |
| 1 | Three ocean masks exist (`is_ocean` = drainage, `is_sea` = drawn map, the log's `ocean_mask`) | Land fraction and land precipitation use `is_sea`, so the number matches the map; closed depressions below sea level count as land (#58) |
| 2 | The sea level is an offset against the dry elevations, not eustatic metres of water | Labelled "since start" with a tooltip "sea surface against the land" |
| 3 | Lake area/volume units; the volume includes sea water poured into basins; one step behind | m² → km² (shown as k/M km²), tooltip names the sea water |
| 4 | The forcing is the one the *next* climate update uses; the carbon cycle can be off | Tooltip says so; a flag shows "(fixed)" without the carbon cycle |
| 5 | Count plates by owning cells; > 1 % as the log; speed by the planet's radius, not Earth's | Done (`Planet::radius`) |
| 6 | Ice cover includes sea ice | Tooltip: sea ice, lake ice and ice sheets |
| 7, 8 | `set_mode` re-shows the displayed state; a restart must reset the shown record | The shown record is set only where a new state is shown (live, past frame, latest) and reset on restart |
| 9 | All producers and parsers of the record | Desktop thread, web worker (one `global` byte array), page parser, tests |
| 10 | Codec header size fixed; version bump after main's 13 | `HEADER` grows by `GlobalStatus::BYTES`; version 14 |
| 11 | A full history stops recording | The sparklines append the newest state if it is newer than the last frame |
| UI | 390 px: label + value + sparkline overflow | On narrow screens the sparklines get rows of their own under the value (checked on the phone screenshot) |
