//! The realistic mode (#92): the planet as a camera in space would see it.
//!
//! Every cell gets the layers of its surface on the CPU from its data ([`Layers`]), and the mode
//! draws its own de-indexed mesh with them ([`detail_vertices`]), which `realistic.wgsl` mixes
//! per pixel with sub-cell detail anchored to the plates and lights in linear light with the
//! simulation's sun:
//!
//! - **Water** (the ocean below sea level, lakes): a dark body colour by depth, from turquoise
//!   shelves to the deep blue of the open ocean, plus a sun glint (GGX reflection with the
//!   Fresnel factor of water) whose roughness follows the wind speed (Cox & Munk, 1954);
//! - **Ice:** permanent ice sheets and sea ice; in January and July also seasonal snow and sea
//!   ice where the month is below freezing;
//! - **Land:** bare ground (rock, soil, or sand where arid) under a vegetation cover that grows
//!   with the moisture index (precipitation over potential evaporation) and the warmth of the
//!   warmest month, from steppe and grassland to forest (none on an abiotic planet); tundra
//!   where the summers are cold; bare rock above the tree line of high mountains.
//!
//! The colours are reflectances (albedos) of the usual ranges (ocean water below 0.06 before the
//! glint, forest 0.04–0.1, grassland 0.08–0.2, desert 0.3–0.4, fresh snow 0.8). The relief is
//! true to scale (no exaggeration, decision 6 on #92). Rivers are left out (invisible from space
//! at this scale). The options choose the light (the sun by default), the season (the sun's
//! declination and distance, snow, sea ice and the greenness of the chosen month, or the annual
//! state) and the detail level ([`Quality`]). See `notes/issue-92/surface.md` and
//! `notes/issue-92/detail.md`.

use super::{Legend, ModeData, ModeOption, OptionKind, OptionValues, Period, RenderMode, Swatch};
use crate::viewer::CellInfo;
use crate::viewer::cell_info::WaterBody;
use crate::viewer::lighting::Light;
use crate::viewer::renderer::{DetailUniforms, DetailVertex, MAX_PLATES};

pub struct Realistic;

/// Index of the light option.
pub const LIGHT: usize = 0;
/// Index of the season option.
pub const SEASON: usize = 1;
/// Index of the detail option.
pub const DETAIL: usize = 2;
/// Index of the atmosphere switch.
pub const ATMOSPHERE: usize = 3;
/// Index of the clouds switch.
pub const CLOUDS: usize = 4;

const OPTIONS: &[ModeOption] = &[
    ModeOption {
        label: Light::LABEL,
        hover: Light::HOVER,
        kind: OptionKind::Choice(Light::CHOICES),
    },
    ModeOption {
        label: "Season",
        hover: "The annual state, or January (northern winter) or July (northern summer): the \
                sun's position, seasonal snow, lake and sea ice, and the greenness of the \
                vegetation",
        kind: OptionKind::Choice(Period::CHOICES),
    },
    ModeOption {
        label: "Detail",
        hover: "Sub-cell detail (coastlines, snow, ice and tree lines, colour variation, \
                relief along the mountain ranges), generated on the fly with more octaves where a \
                pixel covers less ground: Low (2 octaves, one pattern, no relief) for slow \
                devices, Medium (4), High (6, ridged mountains)",
        kind: OptionKind::Choice(Quality::CHOICES),
    },
    ModeOption {
        label: "Atmosphere",
        hover: "The air and its haze, true to scale: a thin glow at the limb, a pale haze \
                towards the edge of the disc, sunlight reddened near the terminator, and a ring \
                of light around the planet when the sun is behind it",
        kind: OptionKind::Switch { default: true },
    },
    ModeOption {
        label: "Clouds",
        hover: "Clouds derived from the simulated climate (the simulation has no clouds): \
                fronts and polar stratus where the air is humid, towering storms where it is \
                warm and wet (the tropical rain belt), stratocumulus over cold subtropical \
                seas; with the season. A decoration: their pattern is noise",
        kind: OptionKind::Switch { default: false },
    },
];

/// The detail level of the realistic mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

impl Quality {
    /// The choices of the option, in the order of the values; the first is the default.
    pub const CHOICES: &'static [&'static str] = &["Medium", "Low", "High"];

    pub fn of(options: &OptionValues) -> Self {
        match options.get(DETAIL) {
            1 => Quality::Low,
            2 => Quality::High,
            _ => Quality::Medium,
        }
    }

    /// The most octaves, the strength of the bump relief, and whether mountains are ridged.
    fn settings(self) -> (f32, f32, bool) {
        match self {
            Quality::Low => (2.0, 0.0, false),
            Quality::Medium => (4.0, 0.2, false),
            Quality::High => (6.0, 0.2, true),
        }
    }

    /// The clouds: the most octaves after the pressure field, whether the cloud tops have
    /// relief, and whether the clouds cast shadows.
    fn cloud_settings(self) -> (f32, bool, bool) {
        match self {
            Quality::Low => (2.0, false, false),
            Quality::Medium => (4.0, true, false),
            Quality::High => (6.0, true, true),
        }
    }
}

/// Reflectances of the surfaces in linear RGB.
const DEEP_OCEAN: [f32; 3] = [0.006, 0.016, 0.042];
const SHELF: [f32; 3] = [0.018, 0.060, 0.070];
/// Lakes: turbid, greenish shallow water, clear deep water, and the paler colour of shallow
/// saline lakes in dry, closed basins.
const LAKE_SHALLOW: [f32; 3] = [0.030, 0.050, 0.035];
const LAKE_DEEP: [f32; 3] = [0.009, 0.021, 0.028];
const SALINE_LAKE: [f32; 3] = [0.060, 0.110, 0.100];
/// Snow-covered lake ice, greyer than fresh snow on land, and bare (black) lake ice.
const LAKE_ICE: [f32; 3] = [0.60, 0.64, 0.70];
const BARE_LAKE_ICE: [f32; 3] = [0.24, 0.28, 0.32];
const ICE_SHEET: [f32; 3] = [0.78, 0.81, 0.86];
const SNOW: [f32; 3] = [0.80, 0.82, 0.85];
/// Sea ice: cold and snow-covered, and melting (wet, with melt ponds).
const SEA_ICE: [f32; 3] = [0.68, 0.72, 0.78];
const MELTING_SEA_ICE: [f32; 3] = [0.42, 0.48, 0.55];
/// A salt flat (playa) at the bottom of a dry closed basin.
const SALT_FLAT: [f32; 3] = [0.50, 0.48, 0.44];
const ROCK: [f32; 3] = [0.15, 0.135, 0.12];
const SOIL: [f32; 3] = [0.17, 0.13, 0.09];
const SAND: [f32; 3] = [0.45, 0.33, 0.20];
const STEPPE: [f32; 3] = [0.20, 0.17, 0.09];
const GRASSLAND: [f32; 3] = [0.075, 0.11, 0.035];
const FOREST: [f32; 3] = [0.03, 0.06, 0.025];
const TUNDRA: [f32; 3] = [0.11, 0.105, 0.075];
/// Reflectance of a snow-covered forest: the canopy hides most of the snow.
const SNOWY_FOREST: [f32; 3] = [0.22, 0.24, 0.26];

/// Depths in metres between which the water turns from the shelf colour to the deep ocean
/// (logarithmically).
const SHELF_DEPTH: f32 = 100.0;
const DEEP_DEPTH: f32 = 3000.0;
/// Lake depths in metres between which the water turns from the shallow colour to the deep one
/// (logarithmically).
const LAKE_SHALLOW_DEPTH: f32 = 3.0;
const LAKE_DEEP_DEPTH: f32 = 300.0;
/// Most roughness of the water surface (GGX α); the alpha channel of land is 1.
const MAX_ROUGHNESS: f32 = 0.45;
/// Lakes are fetch-limited: their waves are those of a weaker wind (factor on the speed).
const LAKE_FETCH: f32 = 0.5;
/// Surface slope (m/km) from which the bump relief starts to line up along the strike of a
/// mountain belt, and from which it does so fully.
const STRIKE_SLOPE: (f32, f32) = (2.0, 8.0);

impl RenderMode for Realistic {
    fn label(&self) -> &'static str {
        "Realistic"
    }

    fn options(&self) -> &'static [ModeOption] {
        OPTIONS
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let abiotic = data.is_some_and(|data| data.abiotic);
        let swatch = |albedo: [f32; 3], label: &'static str| {
            Swatch::fill(albedo.map(|c| (srgb(c) * 255.0).round() as u8), label)
        };
        let mut swatches = vec![
            swatch(DEEP_OCEAN, "Deep ocean"),
            swatch(SHELF, "Shelf"),
            swatch(SEA_ICE, "Sea ice"),
            swatch(LAKE_SHALLOW, "Lake"),
            swatch(SALINE_LAKE, "Saline lake"),
            swatch(ICE_SHEET, "Ice, snow"),
            swatch(SAND, "Desert"),
            swatch(SALT_FLAT, "Salt flat"),
            swatch(ROCK, "Rock"),
        ];
        let clouds = options.is_on(CLOUDS);
        if clouds {
            swatches.push(swatch([0.75; 3], "Cloud"));
        }
        if !abiotic {
            swatches.extend([
                swatch(STEPPE, "Steppe"),
                swatch(GRASSLAND, "Grassland"),
                swatch(FOREST, "Forest"),
                swatch(TUNDRA, "Tundra"),
            ]);
        }
        let mut description = String::from(
            "The planet in space, lit by the simulation's sun: surface colours \
                          derived from the simulated climate (vegetation from rain and warmth, \
                          none without life), relief and atmosphere true to scale, sun glint on \
                          the water. Lakes darken with depth, closed lakes in dry climates are \
                          saline and dry basins have salt flats; the seasons bring snow, lake \
                          and sea ice; mountain relief runs along the ranges. \
                          The view adapts to the light like an eye: after the sun goes behind \
                          the planet or out of view, more stars appear over a few seconds.",
        );
        if clouds {
            description.push_str(
                " Clouds are a decoration derived from the climate (the simulation has none): \
                 their amount and type follow the humidity, rain and warmth of the season; their \
                 pattern is noise fixed in space.",
            );
        }
        Legend {
            description: description.into(),
            scale: None,
            swatches,
            arrows: Vec::new(),
        }
    }
}

/// The light chosen in the options of the realistic mode.
pub fn light(options: &OptionValues) -> Light {
    Light::from_index(options.get(LIGHT) as usize)
}

/// The number of points along a ray of the atmosphere (`atmosphere.wgsl`), 0 when it is
/// switched off: more with a higher detail level.
pub fn atmosphere_samples(options: &OptionValues) -> u32 {
    if !options.is_on(ATMOSPHERE) {
        return 0;
    }
    match Quality::of(options) {
        Quality::Low => 8,
        Quality::Medium => 12,
        Quality::High => 16,
    }
}

/// The season chosen in the options of the realistic mode.
pub fn season(options: &OptionValues) -> Period {
    Period::of(options, SEASON)
}

/// The surface of a cell: its colour at the cell centre, and the layers the sub-cell detail
/// mixes per pixel (see [`Layers`]).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Reflectance at the cell centre in linear RGB.
    pub albedo: [f32; 3],
    /// For open water, the roughness of its surface (GGX α); `None` for land and ice.
    pub roughness: Option<f32>,
    pub layers: Layers,
}

/// The layers of a cell's surface, which the detail shader mixes per pixel; at the cell centre
/// they give the cell's colour. Reflectances in linear RGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layers {
    /// Whether the cell is water (ocean or lake), and whether it is the ocean.
    pub water_cell: bool,
    pub ocean: bool,
    /// Bare ground and the full plant cover of the land, and the share of the plants.
    pub bare: [f32; 3],
    pub plants: [f32; 3],
    pub cover: f32,
    /// Snow or ice (sea ice on water) and its share.
    pub snow: [f32; 3],
    pub snow_cover: f32,
    /// Open water and the roughness of its surface (GGX α).
    pub water: [f32; 3],
    pub water_roughness: f32,
    /// How rugged the land is, 0 (plains, water) to 1 (high young mountains): the amplitude of
    /// the bump relief.
    pub relief: f32,
}

#[cfg(test)]
impl Layers {
    /// The reflectance at the cell centre (without the shader's sharpening of the snow).
    pub fn albedo(&self) -> [f32; 3] {
        let ground = if self.water_cell {
            self.water
        } else {
            mix(self.bare, self.plants, self.cover)
        };
        mix(ground, self.snow, self.snow_cover)
    }
}

/// The surface of a cell in a season (the annual state for [`Period::Annual`]).
#[cfg(test)]
pub fn material(cell: &CellInfo, season: Period, abiotic: bool) -> Material {
    let layers = layers(cell, season, abiotic, false);
    Material {
        albedo: layers.albedo(),
        roughness: (layers.water_cell && layers.snow_cover <= 0.5)
            .then_some(layers.water_roughness),
        layers,
    }
}

/// The moisture index P / PET of the year (1 humid, < 0.2 arid; 1 where unknown).
fn moisture_index(cell: &CellInfo) -> f32 {
    if cell.potential_evaporation > 0.0 {
        let index = cell.precipitation / cell.potential_evaporation;
        if index.is_finite() { index } else { 1.0 }
    } else {
        1.0
    }
}

/// The ice cover of a lake `depth` m deep in a month with the mean temperature `t` (°C):
/// from −1 °C (like the snow on land), later the deeper the lake (its water stores the summer's
/// heat), fully frozen at −5 °C (shallow) to −10 °C (≥ 200 m). (The simulation's
/// `climate::ice_free` ramp of −2 to +2 °C stops the evaporation within a month; a lake
/// covered for most of the month needs a colder mean.)
fn lake_ice(t: f32, depth: f32) -> f32 {
    smoothstep(-1.0, -5.0 - 5.0 * smoothstep(10.0, 200.0, depth), t)
}

/// The sea-ice cover of a month for an ocean cell whose annual mean cover is `annual`, in its
/// hemisphere's winter (the colder of January and July) or summer. The frames hold only the
/// annual cover, and the month's temperature cannot tell melting ice (at 0 °C in the model)
/// from open water at its freezing point (−1.8 °C): the winter month gets twice the annual
/// cover, the summer month what remains, so that the two average to the annual cover.
fn sea_ice(annual: f32, winter: bool) -> f32 {
    let annual = annual.clamp(0.0, 1.0);
    if winter {
        (2.0 * annual).min(1.0)
    } else {
        (2.0 * annual - 1.0).max(0.0)
    }
}

fn layers(cell: &CellInfo, season: Period, abiotic: bool, closed_lake: bool) -> Layers {
    let finite = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
    let annual_temperature = finite(cell.temperature, 10.0);
    let temperature = finite(season.temperature(cell), annual_temperature);
    let warmest = finite(
        cell.temperature_january.max(cell.temperature_july),
        annual_temperature,
    );
    let precipitation = finite(season.precipitation(cell), cell.precipitation.max(0.0));
    let ice = finite(cell.ice, 0.0).clamp(0.0, 1.0);
    // The permanent ice as it shows in the annual state: the annual cover sharpened around one
    // half (the topographic view's rule), so that ice present most of the year is a closed
    // cover, not a pattern of floes as wide as the whole ice zone.
    let permanent = smoothstep(0.3, 0.7, ice);
    // Seasonal snow where the chosen month freezes (none in the annual state, which shows the
    // permanent ice only).
    let freezing = match season {
        Period::Annual => 0.0,
        _ => smoothstep(-1.0, -6.0, temperature),
    };
    // Ocean, lakes (at least `LAKE_DEPTH` deep) and land, also dry closed depressions below sea
    // level (#128).
    let body = cell.water_body();
    let ocean = body == WaterBody::Ocean;
    let water_cell = cell.is_water();
    let land = !water_cell;
    // Moisture index P / PET of the year (1 humid, < 0.2 arid).
    let moisture = moisture_index(cell);
    let arid = 1.0 - smoothstep(0.1, 0.4, moisture);
    let [east, north] = season.wind(cell);
    let wind = finite((east * east + north * north).sqrt(), 7.0);
    let (water, water_roughness) = match body {
        WaterBody::Ocean => {
            let depth = (-cell.elevation).max(1.0);
            let t = ((depth / SHELF_DEPTH).ln() / (DEEP_DEPTH / SHELF_DEPTH).ln()).clamp(0.0, 1.0);
            (mix(SHELF, DEEP_OCEAN, t), water_roughness(wind))
        }
        WaterBody::Lake(depth) => {
            let t = ((depth.max(LAKE_SHALLOW_DEPTH) / LAKE_SHALLOW_DEPTH).ln()
                / (LAKE_DEEP_DEPTH / LAKE_SHALLOW_DEPTH).ln())
            .clamp(0.0, 1.0);
            // Lakes whose water does not leave, in a dry climate, are saline (the simulation
            // has no salt: closure and aridity stand in for it): paler, the more so the
            // shallower.
            let saline = if closed_lake {
                (1.0 - smoothstep(0.2, 0.5, moisture)) * (1.0 - 0.6 * t)
            } else {
                0.0
            };
            (
                mix(mix(LAKE_SHALLOW, LAKE_DEEP, t), SALINE_LAKE, saline),
                water_roughness(LAKE_FETCH * wind),
            )
        }
        // Land: the colour of the sub-cell lakes the noise opens in humid lowlands (and of the
        // water next to it, if it has water neighbours, see `detail_vertices`).
        _ => (LAKE_SHALLOW, water_roughness(LAKE_FETCH * wind)),
    };

    let bare = mix(mix(ROCK, SOIL, 0.5), SAND, arid);
    let (plants, cover, forested) = if abiotic {
        (bare, 0.0, 0.0)
    } else {
        // Plants need a warm season (the tree line is near a warmest month of 10 °C) and
        // water; trees more of both than grasses.
        let growing = smoothstep(0.0, 6.0, warmest);
        let cover = smoothstep(0.08, 0.5, moisture) * growing;
        let trees = smoothstep(0.5, 1.0, moisture) * smoothstep(6.0, 12.0, warmest);
        let grass = mix(STEPPE, GRASSLAND, smoothstep(0.25, 0.6, moisture));
        // Outside the growing season (a cold or dry month) grasses brown.
        let dormant = match season {
            Period::Annual => 0.0,
            _ => (smoothstep(5.0, -2.0, temperature)).max(
                1.0 - smoothstep(
                    0.1,
                    0.4,
                    precipitation / cell.potential_evaporation.max(0.1),
                ),
            ),
        };
        let grass = mix(grass, STEPPE, 0.7 * dormant);
        (mix(grass, FOREST, trees), cover, trees)
    };
    // Where summers stay cold: tundra; above the tree line of high mountains: bare rock. (Both
    // layers alike, so that the mix of the two stays the cell's colour.)
    let tundra = if abiotic {
        0.0
    } else {
        smoothstep(8.0, 2.0, warmest) * (1.0 - arid)
    };
    let high = smoothstep(2500.0, 4500.0, cell.elevation);
    let adjust = |albedo: [f32; 3]| mix(mix(albedo, TUNDRA, tundra), ROCK, high);
    let (mut bare, plants) = (adjust(bare), adjust(plants));
    // A salt flat (playa) where the water of a dry closed basin collects and evaporates: at
    // its sink, in a dry climate.
    let mut cover = cover;
    if body == WaterBody::DryDepression && cell.receiver_slot().is_none() {
        // At most half the cell: the salt flats of Earth's largest basins are smaller than
        // one.
        let playa = 0.5 * (1.0 - smoothstep(0.15, 0.4, moisture));
        bare = mix(bare, SALT_FLAT, playa);
        cover *= 1.0 - playa;
    }
    // Bump relief: high land and young orogens are rugged; an ice sheet buries most of it.
    let relief = if land {
        (smoothstep(300.0, 3000.0, cell.elevation) * 0.7 + 0.3 * cell.orogenic.clamp(0.0, 1.0))
            .clamp(0.0, 1.0)
            * (1.0 - 0.6 * permanent)
    } else {
        0.0
    };

    // Snow and ice as continuous covers, so that their edges fall between the cells where the
    // interpolated cover crosses one half.
    let (snow, snow_cover) = if ocean {
        let cover = match season {
            Period::Annual => permanent,
            Period::January => sea_ice(ice, cell.temperature_january <= cell.temperature_july),
            Period::July => sea_ice(ice, cell.temperature_july < cell.temperature_january),
        };
        // Cold ice is snow-covered, ice near the melting point wet with melt ponds (the
        // model's own albedos switch at 0 °C).
        let cold = smoothstep(0.0, -10.0, temperature);
        (mix(MELTING_SEA_ICE, SEA_ICE, cold), cover)
    } else if let WaterBody::Lake(depth) = body {
        // Lakes ice over in their own way: not from the climate's `ice`, which is sea ice
        // below sea level and the snowfall-limited ice sheet above it. In the annual state where
        // the annual mean is far below freezing; in a month by its temperature, or where even
        // the warmest month freezes. The ice is
        // snow-covered where snow falls, grey and bare where it does not.
        let frozen = match season {
            // Frozen most of the year.
            Period::Annual => smoothstep(-10.0, -20.0, annual_temperature),
            // Frozen in the month, or all year where even the warmest month freezes.
            _ => lake_ice(temperature, depth).max(smoothstep(0.0, -5.0, warmest)),
        };
        let snowy = smoothstep(0.02, 0.15, precipitation);
        (mix(BARE_LAKE_ICE, LAKE_ICE, snowy), frozen)
    } else {
        // Seasonal snow where there is precipitation to fall as snow; forests hide most of it.
        let seasonal = freezing * smoothstep(0.02, 0.15, precipitation);
        let color = if permanent >= seasonal {
            ICE_SHEET
        } else {
            mix(SNOW, SNOWY_FOREST, forested)
        };
        (color, permanent.max(seasonal))
    };
    Layers {
        water_cell,
        ocean,
        bare,
        plants,
        cover,
        snow,
        snow_cover,
        water,
        water_roughness,
        relief,
    }
}

/// The realistic mode's de-indexed mesh with the layers of every cell, three own vertices per
/// triangle of the mesh (`faces`), at the displayed positions and normals of `data.vertices`.
///
/// Water cells get the land layers of their land neighbours (and land cells the water of their
/// water neighbours), so that a pixel that the noise turns into land next to a water cell shows
/// the land beside it.
pub fn detail_vertices(data: &ModeData, faces: &[[u32; 3]]) -> Vec<DetailVertex> {
    let season = season(data.options);
    if data.cells.len() != data.vertices.len() || data.cells.len() != data.mesh.len() {
        return Vec::new();
    }
    let closed = closed_lakes(data);
    let cells = lake_air(data);
    let own: Vec<Layers> = cells
        .iter()
        .zip(&closed)
        .map(|(cell, &closed)| layers(cell, season, data.abiotic, closed))
        .collect();
    let layers: Vec<Layers> = own
        .iter()
        .enumerate()
        .map(|(cell, layers)| {
            let neighbours = data.mesh.neighbors(cell as u32);
            let mut layers = *layers;
            let others: Vec<&Layers> = neighbours
                .iter()
                .map(|&neighbour| &own[neighbour as usize])
                .filter(|other| other.water_cell != layers.water_cell)
                .collect();
            if !others.is_empty() {
                let mean = |pick: &dyn Fn(&Layers) -> [f32; 3]| {
                    let sum = others.iter().fold([0.0; 3], |sum, other| {
                        let value = pick(other);
                        [0, 1, 2].map(|i| sum[i] + value[i])
                    });
                    sum.map(|value| value / others.len() as f32)
                };
                let count = others.len() as f32;
                if layers.water_cell {
                    layers.bare = mean(&|other| other.bare);
                    layers.plants = mean(&|other| other.plants);
                    layers.cover = others.iter().map(|other| other.cover).sum::<f32>() / count;
                } else {
                    layers.water = mean(&|other| other.water);
                    layers.water_roughness = others
                        .iter()
                        .map(|other| other.water_roughness)
                        .sum::<f32>()
                        / count;
                }
            }
            layers
        })
        .collect();
    let coasts: Vec<f32> = (0..data.cells.len())
        .map(|cell| coast_value(data, &own, cell))
        .collect();
    let gradients: Vec<glam::DVec3> = (0..data.cells.len())
        .map(|cell| surface_gradient(data, cell))
        .collect();
    let corners: Vec<u32> = (0..data.cells.len())
        .map(|cell| {
            let (strike, squeeze) = if own[cell].relief > 0.0 {
                strike(data, &gradients, cell, own[cell].relief)
            } else {
                ([0.0; 3], 0.0)
            };
            pack_corner(strike, squeeze, own[cell].ocean)
        })
        .collect();
    // The colours of a corner depend only on its cell: encoded once per cell rather than once
    // per corner (six times as often), since this runs on the page's main thread for every new
    // state (#174).
    let color = |albedo: [f32; 3], alpha: f32| {
        let [r, g, b] = albedo.map(srgb8);
        [r, g, b, unorm(alpha)]
    };
    let colors: Vec<CellColors> = layers
        .iter()
        .map(|layers| CellColors {
            bare: color(layers.bare, layers.cover),
            plants: color(layers.plants, layers.snow_cover),
            snow: color(layers.snow, layers.water_roughness / 0.5),
            water: color(layers.water, layers.relief),
        })
        .collect();
    let clouds: Vec<[u8; 4]> = if data.options.is_on(CLOUDS) {
        cloud_layers(data, season)
            .iter()
            .map(CloudLayer::encode)
            .collect()
    } else {
        vec![[0; 4]; data.cells.len()]
    };
    let plate = |cell: u32| data.cells[cell as usize].plate.min(u32::from(u16::MAX)) as u16;
    let mut vertices = Vec::with_capacity(faces.len() * 3);
    for &face in faces {
        let plates = face.map(plate);
        let ridges = face.map(|cell| corners[cell as usize]);
        for (corner, &cell) in face.iter().enumerate() {
            let colors = &colors[cell as usize];
            let vertex = &data.vertices[cell as usize];
            vertices.push(DetailVertex {
                position: vertex.position,
                normal: vertex.normal,
                bare: colors.bare,
                plants: colors.plants,
                snow: colors.snow,
                water: colors.water,
                coast: coasts[cell as usize],
                plates: [plates[0], plates[1], plates[2], corner as u16],
                ridges,
                clouds: clouds[cell as usize],
            });
        }
    }
    vertices
}

/// The colours of a cell's corners in [`detail_vertices`].
struct CellColors {
    bare: [u8; 4],
    plants: [u8; 4],
    snow: [u8; 4],
    water: [u8; 4],
}

/// A value in 0..1 as an 8-bit unsigned normalised value.
fn unorm(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A linear colour channel in 8-bit sRGB: `unorm(srgb(linear))`, exactly, but without its
/// `powf`, which dominated the cost of [`detail_vertices`] in the browser (#174). The table
/// holds the smallest value that encodes to each of 1..=255 (found with `srgb` itself, whose
/// encoding only grows with the value), and the code is the number of them not above `linear`;
/// NaN, negative values and zero encode to 0.
fn srgb8(linear: f32) -> u8 {
    static THRESHOLDS: std::sync::OnceLock<[f32; 255]> = std::sync::OnceLock::new();
    let thresholds = THRESHOLDS.get_or_init(|| {
        std::array::from_fn(|code| {
            // The encoding only grows with the value, and non-negative floats sort like their
            // bits: a bisection over the bits of 0..=1.
            let wanted = code as u8 + 1;
            let (mut low, mut high) = (0_u32, 1.0_f32.to_bits());
            while low < high {
                let middle = low + (high - low) / 2;
                if unorm(srgb(f32::from_bits(middle))) >= wanted {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            f32::from_bits(low)
        })
    });
    if linear > 0.0 {
        thresholds.partition_point(|&threshold| threshold <= linear) as u8
    } else {
        0
    }
}

/// The coast value of a cell: positive on land, negative on water, 0.4 to 1 in size. The coast
/// lies where it crosses zero between a land and a water corner: about in the middle of the
/// edge, nearer to low land and shallow water. The noise moves it by up to ±0.5, so it opens
/// sub-cell water on land below 0.5 and sub-cell islands in water above −0.5.
fn coast_value(data: &ModeData, layers: &[Layers], cell: usize) -> f32 {
    let info = &data.cells[cell];
    if let WaterBody::Lake(depth) = info.water_body() {
        // At least 0.5, so that the noise (±0.5) does not erase small lakes.
        return -(0.5 + 0.5 * smoothstep(0.0, 200.0, depth));
    }
    if layers[cell].water_cell {
        return -(0.4 + 0.6 * smoothstep(0.0, 2000.0, -info.elevation));
    }
    // Land: its height above the highest water surface beside it (a lake's level, the sea),
    // so that the shores of a lake on a plateau lie where those of the sea would.
    let level = data
        .mesh
        .neighbors(cell as u32)
        .iter()
        .filter(|&&neighbour| layers[neighbour as usize].water_cell)
        .map(|&neighbour| data.cells[neighbour as usize].surface_height() as f32)
        .fold(None, |level: Option<f32>, height| {
            Some(level.map_or(height, |level| level.max(height)))
        })
        .unwrap_or(0.0);
    let height = 0.4 + 0.6 * smoothstep(0.0, 2000.0, info.elevation - level);
    // Sub-cell lakes only in humid land: a floor above the noise's reach in dry land.
    let floor = 0.4 + 0.15 * (1.0 - smoothstep(0.3, 0.8, moisture_index(info)));
    height.max(floor)
}

/// The cells with the air temperatures over land below sea level: lakes whose floor is below
/// sea level and dry closed depressions. The climate gives such cells the surface temperature
/// of the ocean (it knows only land above sea level), which would keep a lake inside a frozen
/// continent open in winter; they take the mean of their neighbours above sea level instead
/// (spread inwards over up to 8 rings; cells with no such land in reach keep their own).
fn lake_air<'a>(data: &ModeData<'a>) -> std::borrow::Cow<'a, [CellInfo]> {
    let low_lake = |cell: &CellInfo| !cell.is_sea() && cell.elevation < 0.0;
    if !data.cells.iter().any(low_lake) {
        return std::borrow::Cow::Borrowed(data.cells);
    }
    let mut cells = data.cells.to_vec();
    // Cells with the air temperature of land: above sea level, or lakes already filled in.
    let mut known: Vec<bool> = cells.iter().map(|cell| cell.elevation >= 0.0).collect();
    for _ in 0..8 {
        let mut changed = Vec::new();
        for cell in 0..cells.len() {
            if known[cell] || !low_lake(&cells[cell]) {
                continue;
            }
            let neighbours: Vec<&CellInfo> = data
                .mesh
                .neighbors(cell as u32)
                .iter()
                .filter(|&&n| known[n as usize])
                .map(|&n| &cells[n as usize])
                .collect();
            if neighbours.is_empty() {
                continue;
            }
            let mean = |pick: fn(&CellInfo) -> f32| {
                neighbours.iter().map(|cell| pick(cell)).sum::<f32>() / neighbours.len() as f32
            };
            changed.push((
                cell,
                mean(|cell| cell.temperature),
                mean(|cell| cell.temperature_january),
                mean(|cell| cell.temperature_july),
            ));
        }
        if changed.is_empty() {
            break;
        }
        for (cell, annual, january, july) in changed {
            cells[cell].temperature = annual;
            cells[cell].temperature_january = january;
            cells[cell].temperature_july = july;
            known[cell] = true;
        }
    }
    std::borrow::Cow::Owned(cells)
}

/// Whether each lake cell belongs to a closed lake, one whose water does not leave: its
/// drainage, followed through the lake, ends at a cell without a receiver (the exit of an
/// endorheic lake), not at a spill into other land.
fn closed_lakes(data: &ModeData) -> Vec<bool> {
    let cells = data.cells;
    let is_lake = |cell: usize| matches!(cells[cell].water_body(), WaterBody::Lake(_));
    let mut closed: Vec<Option<bool>> = vec![None; cells.len()];
    let mut path = Vec::new();
    for start in 0..cells.len() {
        if !is_lake(start) || closed[start].is_some() {
            continue;
        }
        path.clear();
        let mut cell = start;
        let result = loop {
            if let Some(known) = closed[cell] {
                break known;
            }
            // A cycle (never in a drainage tree) counts as closed.
            if path.len() > cells.len() {
                break true;
            }
            path.push(cell);
            match cells[cell].receiver_slot() {
                None => break true,
                Some(slot) => match data.mesh.neighbors(cell as u32).get(slot) {
                    Some(&next) if is_lake(next as usize) => cell = next as usize,
                    _ => break false,
                },
            }
        };
        for &cell in &path {
            closed[cell] = Some(result);
        }
    }
    closed
        .into_iter()
        .map(|closed| closed == Some(true))
        .collect()
}

/// The gradient of the drawn surface height (the sea at 0) at a cell in m/km, as a tangent
/// vector in the world frame, by least squares over the neighbours.
fn surface_gradient(data: &ModeData, cell: usize) -> glam::DVec3 {
    let up = data.mesh.vertex(cell as u32);
    let height = |cell: usize| data.cells[cell].surface_height();
    let own = height(cell);
    let (east, north) = tangent_basis(up);
    let (mut xx, mut xy, mut yy, mut bx, mut by) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &neighbour in data.mesh.neighbors(cell as u32) {
        let offset = data.mesh.vertex(neighbour) - up;
        let (x, y) = (offset.dot(east), offset.dot(north));
        let rise = height(neighbour as usize) - own;
        xx += x * x;
        xy += x * y;
        yy += y * y;
        bx += x * rise;
        by += y * rise;
    }
    let determinant = xx * yy - xy * xy;
    if determinant.is_nan() || determinant.abs() <= 1e-20 {
        return glam::DVec3::ZERO;
    }
    // Metres per radian, then per km of the surface.
    let gx = (yy * bx - xy * by) / determinant;
    let gy = (xx * by - xy * bx) / determinant;
    (east * gx + north * gy) / (crate::EARTH_RADIUS / 1000.0)
}

/// How the bump relief of a land cell lines up along the strike of a mountain belt: the strike
/// `ŝ` (a unit tangent vector in the world frame; its sign does not matter) and the squeeze of
/// the noise along it, `σ = 1 − 1/k` for ridges `k` times as long as wide (`k` up to 5, σ up to
/// 0.8).
///
/// The strike runs across the slope of the surface. It comes from the structure tensor of the
/// surface gradients over the cell and its neighbours (`Σ ∇h ∇hᵀ`), which is sign-free, so that
/// the two flanks of a range agree and its crest, where the gradient vanishes, takes the
/// direction of its flanks. The strength grows with the slope along the principal direction
/// (`STRIKE_SLOPE`), the coherence of the directions `(λ₁ − λ₂)/(λ₁ + λ₂)` (its root) and the
/// ruggedness (full from one half).
fn strike(
    data: &ModeData,
    gradients: &[glam::DVec3],
    cell: usize,
    ruggedness: f32,
) -> ([f32; 3], f32) {
    let up = data.mesh.vertex(cell as u32);
    let (east, north) = tangent_basis(up);
    let (mut a, mut b, mut d) = (0.0, 0.0, 0.0);
    let neighbours = data.mesh.neighbors(cell as u32);
    for &other in std::iter::once(&(cell as u32)).chain(neighbours) {
        let gradient = gradients[other as usize];
        let (x, y) = (gradient.dot(east), gradient.dot(north));
        a += x * x;
        b += x * y;
        d += y * y;
    }
    let count = (neighbours.len() + 1) as f64;
    let mean = 0.5 * (a + d);
    let spread = (0.25 * (a - d) * (a - d) + b * b).sqrt();
    let (major, minor) = (mean + spread, mean - spread);
    if major.is_nan() || major <= 1e-12 {
        return ([0.0; 3], 0.0);
    }
    let coherence = (major - minor) / (major + minor);
    let slope = (major / count).sqrt();
    // Mountains of medium ruggedness already line up fully.
    let strength = smoothstep(STRIKE_SLOPE.0, STRIKE_SLOPE.1, slope as f32)
        * (coherence as f32).sqrt()
        * (2.0 * ruggedness).min(1.0);
    if strength.is_nan() || strength <= 0.0 {
        return ([0.0; 3], 0.0);
    }
    // The principal direction (the slope), and the strike across it.
    let (x, y) = if (major - a).abs() > (major - d).abs() {
        (b, major - a)
    } else {
        (major - d, b)
    };
    let strike = (east * -y + north * x).normalize_or_zero().as_vec3();
    if strike == glam::Vec3::ZERO {
        return ([0.0; 3], 0.0);
    }
    (strike.to_array(), 1.0 - 1.0 / (1.0 + 4.0 * strength))
}

/// East and north at a point of the unit sphere (any orthonormal tangent pair at the poles).
fn tangent_basis(up: glam::DVec3) -> (glam::DVec3, glam::DVec3) {
    let east = glam::DVec3::Z.cross(up);
    let east = if east.length_squared() > 1e-12 {
        east.normalize()
    } else {
        glam::DVec3::X
    };
    (east, up.cross(east))
}

/// The most squeeze of the relief noise along a strike (ridges five times as long as wide).
const MAX_SQUEEZE: f32 = 0.8;

/// Packs a corner's strike (snorm8 x, y, z in bits 0–23), the squeeze along it (bits 24–30, in
/// steps of `MAX_SQUEEZE / 127`) and whether the cell is ocean (bit 31), as `realistic.wgsl`
/// unpacks them.
fn pack_corner(strike: [f32; 3], squeeze: f32, ocean: bool) -> u32 {
    let snorm = |value: f32| u32::from((value.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8);
    let squeeze = (squeeze / MAX_SQUEEZE * 127.0).round().clamp(0.0, 127.0) as u32;
    snorm(strike[0])
        | snorm(strike[1]) << 8
        | snorm(strike[2]) << 16
        | squeeze << 24
        | u32::from(ocean) << 31
}

/// The clouds of a cell, from its climate (see [`cloud_layer`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudLayer {
    /// Share of the sky covered, 0 to 1.
    pub cover: f32,
    /// How convective the clouds are: small, crisp-edged elements (cumulus, 1) rather than
    /// smooth sheets (0).
    pub convective: f32,
    /// Typical optical depth of the clouds (the shader scales it from about half at their
    /// edges to 1.6 times in their cores).
    pub depth: f32,
    /// Height of the cloud tops in km (above sea level from [`cloud_layer`], above the ground
    /// from [`cloud_layers`]).
    pub top: f32,
}

impl CloudLayer {
    pub const CLEAR: CloudLayer = CloudLayer {
        cover: 0.0,
        convective: 0.0,
        depth: 0.0,
        top: 0.0,
    };

    /// The mean of `layers`: the mean cover, and the convective share, optical depth and top
    /// height weighted by the cover (clear for none).
    fn mean(layers: impl IntoIterator<Item = CloudLayer>) -> CloudLayer {
        let (mut count, mut cover, mut convective, mut depth, mut top) = (0, 0.0, 0.0, 0.0, 0.0);
        for layer in layers {
            count += 1;
            cover += layer.cover;
            convective += layer.cover * layer.convective;
            depth += layer.cover * layer.depth;
            top += layer.cover * layer.top;
        }
        if count == 0 || cover <= 0.0 {
            return CloudLayer::CLEAR;
        }
        CloudLayer {
            cover: cover / count as f32,
            convective: convective / cover,
            depth: depth / cover,
            top: top / cover,
        }
    }

    /// As the shader reads it (`DetailVertex::clouds`); `top` is above the ground there.
    fn encode(&self) -> [u8; 4] {
        [
            unorm(self.cover),
            unorm(self.convective),
            unorm(self.depth / 64.0),
            unorm(self.top / 16.0),
        ]
    }
}

/// Column relative humidity from which large-scale clouds form, and at which the sky is
/// overcast (the smoothstep of [`cloud_layer`]).
const CLOUD_HUMIDITY: (f32, f32) = (0.62, 0.86);
/// Column temperatures (°C) over which deep convection sets in; the precipitation (m/yr) from
/// which it forms, and the scale of its cover's growth with more.
const CONVECTION_WARMTH: (f32, f32) = (18.0, 26.0);
const CONVECTIVE_RAIN: (f32, f32) = (0.5, 1.2);
/// Shallow (trade) cumulus: its cover, and the column temperatures (°C) over which it forms.
const SHALLOW_CUMULUS: f32 = 0.25;
const SHALLOW_WARMTH: (f32, f32) = (10.0, 20.0);
/// Marine stratocumulus: the most cover of a deck, and the cold anomaly of the sea surface
/// (K below the zonal mean of the ocean) and the excess of evaporation over precipitation
/// (m/yr, subsidence) over which it forms.
const STRATOCUMULUS: f32 = 0.8;
const STRATOCUMULUS_COLD: (f32, f32) = (1.0, 5.0);
const STRATOCUMULUS_DRY: (f32, f32) = (0.1, 0.6);

/// The inputs of [`cloud_layer`] for a cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudClimate {
    /// Column relative humidity (see [`crate::climate::column_relative_humidity`]).
    pub humidity: f32,
    /// Temperature of the free troposphere (°C): the zonal mean of the sea-level temperature.
    pub column_temperature: f32,
    /// Precipitation in m/yr.
    pub precipitation: f32,
    /// How moist the surface is for shallow cumulus: 1 over the ocean, from the moisture index
    /// over land.
    pub moist: f32,
    /// The cover of a stratocumulus deck (see [`STRATOCUMULUS`]).
    pub stratocumulus: f32,
}

/// The clouds of a cell from four regimes, combined with random overlap:
///
/// - **large-scale cloud** (fronts, polar stratus, monsoon decks) from the column relative
///   humidity: smooth sheets, optical depth 6–20 and tops 4–8 km (thicker and higher in warmer
///   columns, so that cold polar clouds are thin);
/// - **deep convection** where the column is warm and it rains a lot: clusters of bright towers,
///   optical depth 40, tops 12 km;
/// - **shallow (trade) cumulus** in warm columns over moist surfaces: small, thin clouds,
///   optical depth 5, tops 2 km;
/// - **marine stratocumulus** over cold subtropical seas: cellular decks, optical depth 12,
///   tops 1.5 km.
///
/// The type (the convective share of the shader: small, crisp-edged elements rather than
/// sheets), the optical depth and the height of the tops are the regimes' mixed by their covers.
pub fn cloud_layer(climate: &CloudClimate) -> CloudLayer {
    let CloudClimate {
        humidity,
        column_temperature,
        precipitation,
        moist,
        stratocumulus,
    } = *climate;
    if !(humidity.is_finite() && column_temperature.is_finite() && precipitation.is_finite()) {
        return CloudLayer::CLEAR;
    }
    let large_scale = smoothstep(CLOUD_HUMIDITY.0, CLOUD_HUMIDITY.1, humidity);
    let deep = smoothstep(CONVECTION_WARMTH.0, CONVECTION_WARMTH.1, column_temperature)
        * (1.0 - (-(precipitation - CONVECTIVE_RAIN.0).max(0.0) / CONVECTIVE_RAIN.1).exp());
    let shallow = SHALLOW_CUMULUS
        * smoothstep(SHALLOW_WARMTH.0, SHALLOW_WARMTH.1, column_temperature)
        * if moist.is_finite() {
            moist.clamp(0.0, 1.0)
        } else {
            0.0
        };
    let stratocumulus = if stratocumulus.is_finite() {
        stratocumulus.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let covers = [large_scale, deep, shallow, stratocumulus];
    let cover = 1.0 - covers.iter().map(|c| 1.0 - c).product::<f32>();
    let sum: f32 = covers.iter().sum();
    if cover <= 0.0 || sum <= 0.0 {
        return CloudLayer::CLEAR;
    }
    let warm = smoothstep(-20.0, 15.0, column_temperature);
    let mixed = |values: [f32; 4]| (0..4).map(|i| covers[i] * values[i]).sum::<f32>() / sum;
    CloudLayer {
        cover,
        convective: mixed([0.0, 0.5, 1.0, 0.7]),
        depth: mixed([6.0 + 14.0 * warm, 40.0, 5.0, 12.0]),
        top: mixed([4.0 + 4.0 * warm, 12.0, 2.0, 1.5]),
    }
}

/// The clouds of every cell in a season (all clear where the climate is unknown).
///
/// The column temperature is the zonal mean of the sea-level temperature, as the water cycle
/// uses it for the saturation of the column; the cold anomaly of the sea surface is against the
/// zonal mean of the ocean. Months the water cycle did not solve fall back to the year.
pub fn cloud_layers(data: &ModeData, season: Period) -> Vec<CloudLayer> {
    let bins = latitude_bins(data);
    let layers = match season {
        // The model's annual precipitation is the mean of its months, each with its own
        // saturation: the mean of the clouds of January and July, rather than the clouds of the
        // mean.
        Period::Annual => {
            let january = month_cloud_layers(data, Period::January, &bins);
            let july = month_cloud_layers(data, Period::July, &bins);
            january
                .iter()
                .zip(&july)
                .map(|(a, b)| CloudLayer::mean([*a, *b]))
                .collect()
        }
        _ => month_cloud_layers(data, season, &bins),
    };
    // Clouds are smoother than the cells: one pass of averaging over the neighbours, so that
    // their edges do not trace the triangles where the cover jumps between cells.
    (0..layers.len())
        .map(|cell| {
            let neighbours = data.mesh.neighbors(cell as u32);
            let around = CloudLayer::mean(neighbours.iter().map(|&n| layers[n as usize]));
            let mut layer = CloudLayer::mean([layers[cell], around]);
            // The tops at least 1 km above the ground (the heights are above sea level).
            if layer.cover > 0.0 {
                let ground = data.cells[cell].elevation.max(0.0) / 1000.0;
                layer.top = (layer.top - ground).max(1.0);
            }
            layer
        })
        .collect()
}

/// The clouds of every cell in a month, `bins` the latitude bin of every cell (see
/// [`latitude_bins`]).
fn month_cloud_layers(data: &ModeData, season: Period, bins: &[u8]) -> Vec<CloudLayer> {
    let params = crate::climate::ClimateParams::default();
    let cells = data.cells;
    let temperature = |cell: &CellInfo| {
        let t = season.temperature(cell);
        if t.is_finite() { t } else { cell.temperature }
    };
    let pick = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
    let column = zonal_mean(bins, |cell| {
        let info = &cells[cell];
        Some(f64::from(temperature(info)) + params.lapse_rate * f64::from(info.elevation.max(0.0)))
    });
    let ocean = zonal_mean(bins, |cell| {
        let info = &cells[cell];
        info.is_sea().then(|| f64::from(temperature(info)))
    });
    // The saturated column of every latitude (the column temperature is zonal).
    let saturation: Vec<Option<crate::climate::ColumnSaturation>> = column
        .iter()
        .map(|&t| crate::climate::ColumnSaturation::new(t, &params))
        .collect();
    cells
        .iter()
        .zip(bins)
        .map(|(info, &bin)| {
            let bin = usize::from(bin);
            let precipitation = pick(season.precipitation(info), info.precipitation);
            let humidity = saturation[bin].map_or(f32::NAN, |column| {
                column.relative_humidity(f64::from(precipitation), f64::from(info.elevation)) as f32
            });
            let stratocumulus = if info.is_sea() {
                let anomaly = temperature(info) - ocean[bin] as f32;
                let evaporation = pick(season.evaporation(info), info.evaporation);
                STRATOCUMULUS
                    * smoothstep(STRATOCUMULUS_COLD.0, STRATOCUMULUS_COLD.1, -anomaly)
                    * smoothstep(
                        STRATOCUMULUS_DRY.0,
                        STRATOCUMULUS_DRY.1,
                        evaporation - precipitation,
                    )
            } else {
                0.0
            };
            let moist = if info.is_water() {
                1.0
            } else {
                smoothstep(0.3, 0.8, moisture_index(info))
            };
            cloud_layer(&CloudClimate {
                humidity,
                column_temperature: column[bin] as f32,
                precipitation,
                moist,
                stratocumulus,
            })
        })
        .collect()
}

/// Bins of latitude for [`zonal_mean`]: 1° wide, like the water cycle's (`climate.rs`).
const LATITUDE_BINS: usize = 180;

/// The latitude bin of every cell.
fn latitude_bins(data: &ModeData) -> Vec<u8> {
    (0..data.cells.len())
        .map(|cell| {
            let latitude = data.mesh.vertex(cell as u32).z.clamp(-1.0, 1.0).asin();
            (((latitude.to_degrees() + 90.0) / 180.0 * LATITUDE_BINS as f64) as usize)
                .min(LATITUDE_BINS - 1) as u8
        })
        .collect()
}

/// The zonal mean of `value` (cells without one left out) in every latitude bin, smoothed over a
/// few degrees like the water cycle's (`climate.rs`); NaN in latitudes without values.
fn zonal_mean(bins: &[u8], value: impl Fn(usize) -> Option<f64>) -> Vec<f64> {
    let mut sum = [0.0; LATITUDE_BINS];
    let mut count = [0.0; LATITUDE_BINS];
    for (cell, &bin) in bins.iter().enumerate() {
        if let Some(value) = value(cell).filter(|value| value.is_finite()) {
            sum[usize::from(bin)] += value;
            count[usize::from(bin)] += 1.0;
        }
    }
    // Gaussian smoothing with a width of 3 bins (3°), over ±9 bins.
    let weights: [f64; 19] = std::array::from_fn(|i| {
        let d = (i as f64 - 9.0) / 3.0;
        (-0.5 * d * d).exp()
    });
    (0..LATITUDE_BINS)
        .map(|b| {
            let (mut s, mut c) = (0.0, 0.0);
            for other in b.saturating_sub(9)..(b + 10).min(LATITUDE_BINS) {
                let w = weights[other + 9 - b];
                s += w * sum[other];
                c += w * count[other];
            }
            if c > 1e-9 { s / c } else { f64::NAN }
        })
        .collect()
}

/// The realistic mode's uniforms for the detail: the plates' orientations and the settings of
/// the chosen quality.
pub fn detail_uniforms(data: &ModeData) -> DetailUniforms {
    let mut orientations = [[0.0, 0.0, 0.0, 1.0]; MAX_PLATES];
    for (slot, plate) in orientations.iter_mut().zip(data.plates) {
        *slot = plate.orientation;
    }
    let (octaves, bump, ridged) = Quality::of(data.options).settings();
    DetailUniforms {
        orientations,
        detail: [
            // One cycle per cell spacing for the first octave.
            (1.0 / data.mesh.mean_edge_angle().max(1e-4)) as f32,
            octaves,
            bump,
            f32::from(u8::from(ridged)),
        ],
        clouds: if data.options.is_on(CLOUDS) {
            let (octaves, relief, shadows) = Quality::of(data.options).cloud_settings();
            [
                octaves,
                CLOUD_FREQUENCY,
                f32::from(u8::from(relief)),
                f32::from(u8::from(shadows)),
            ]
        } else {
            [0.0; 4]
        },
    }
}

/// Frequency of the clouds' first octave after the pressure field, in cycles per planet radius
/// (a wavelength of ~1000 km; see `clouds.wgsl`).
const CLOUD_FREQUENCY: f32 = 6.0;

/// Roughness (GGX α) of a water surface under a wind of `speed` m/s: the root of the mean
/// square slope of the waves, `0.003 + 0.00512 U` (Cox & Munk, 1954).
fn water_roughness(speed: f32) -> f32 {
    (0.003 + 0.005_12 * speed.max(0.0))
        .sqrt()
        .min(MAX_ROUGHNESS)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// Hermite step from 0 at `start` to 1 at `end` (also for `start > end`).
fn smoothstep(start: f32, end: f32, x: f32) -> f32 {
    let t = ((x - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Encodes a linear colour channel in sRGB.
fn srgb(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        12.92 * linear.max(0.0)
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(elevation: f32, temperature: f32, precipitation: f32, pet: f32) -> CellInfo {
        CellInfo {
            elevation,
            temperature,
            temperature_january: temperature - 10.0,
            temperature_july: temperature + 10.0,
            precipitation,
            precipitation_january: precipitation,
            precipitation_july: precipitation,
            potential_evaporation: pet,
            wind_east: 7.0,
            wind_january_east: 9.0,
            wind_july_east: 5.0,
            ..bytemuck::Zeroable::zeroed()
        }
    }

    #[test]
    fn closed_depressions_are_lakes_or_land() {
        // A dry depression below sea level is land (#128), a lake of at least 1 m is water.
        let dry = CellInfo {
            drainage: CellInfo::drainage_bits(Some(0), false, true, 0.5),
            lake_depth: 0.5,
            ..cell(-150.0, 20.0, 0.6, 1.2)
        };
        let layers_dry = layers(&dry, Period::Annual, false, false);
        assert!(!layers_dry.water_cell);
        let lake = CellInfo {
            drainage: CellInfo::drainage_bits(Some(0), false, true, 40.0),
            lake_depth: 40.0,
            ..cell(-150.0, 20.0, 0.6, 1.2)
        };
        let layers_lake = layers(&lake, Period::Annual, false, false);
        assert!(layers_lake.water_cell);
        // 40 m: between the shallow and the deep colour, fresh.
        let t = (40.0f32 / LAKE_SHALLOW_DEPTH).ln() / (LAKE_DEEP_DEPTH / LAKE_SHALLOW_DEPTH).ln();
        assert!(close(layers_lake.water, mix(LAKE_SHALLOW, LAKE_DEEP, t)));
        assert_eq!(layers_lake.relief, 0.0);
        // Below sea level outside depressions: the ocean.
        assert!(layers(&cell(-150.0, 20.0, 0.0, 0.0), Period::Annual, false, false).water_cell);
    }

    /// The encoding of [`detail_vertices`] before #174.
    fn srgb8_reference(linear: f32) -> u8 {
        unorm(srgb(linear))
    }

    #[test]
    fn srgb8_matches_the_power_function() {
        let check = |value: f32| {
            assert_eq!(
                srgb8(value),
                srgb8_reference(value),
                "{value:e} ({:#x})",
                value.to_bits()
            );
        };
        // A spread of values over 0..=1 (every 1021st float) and beyond, the floats around every
        // code boundary, and special values.
        let one = 1.0_f32.to_bits();
        for bits in (0..=one).step_by(1021) {
            check(f32::from_bits(bits));
        }
        for code in 1..=255_u8 {
            // The first float that encodes to `code` or more.
            let (mut low, mut high) = (0, one);
            while low < high {
                let middle = low + (high - low) / 2;
                if srgb8_reference(f32::from_bits(middle)) >= code {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            let boundary = low;
            for bits in boundary.saturating_sub(200)..=(boundary + 200).min(one) {
                check(f32::from_bits(bits));
            }
        }
        for value in [
            0.0,
            -0.0,
            -1.0,
            1e-30,
            0.003_130_8,
            0.5,
            1.0,
            1.000_001,
            2.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
        ] {
            check(value);
        }
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)
    }

    fn brightness(albedo: [f32; 3]) -> f32 {
        0.2126 * albedo[0] + 0.7152 * albedo[1] + 0.0722 * albedo[2]
    }

    fn greenness(albedo: [f32; 3]) -> f32 {
        albedo[1] / (albedo[0] + albedo[1] + albedo[2])
    }

    #[test]
    fn the_sun_lights_the_annual_state_by_default() {
        let options = OptionValues::defaults(Realistic.options());
        assert_eq!(light(&options), Light::Sun);
        assert_eq!(season(&options), Period::Annual);
        let mut options = options;
        options.set(LIGHT, 2);
        options.set(SEASON, 2);
        assert_eq!(light(&options), Light::Camera);
        assert_eq!(season(&options), Period::July);
    }

    #[test]
    fn deeper_water_is_darker_and_wind_roughens_it() {
        let shallow = material(&cell(-50.0, 20.0, 1.0, 0.0), Period::Annual, false);
        let deep = material(&cell(-5000.0, 20.0, 1.0, 0.0), Period::Annual, false);
        assert!(close(shallow.albedo, SHELF));
        assert!(close(deep.albedo, DEEP_OCEAN));
        assert!(brightness(deep.albedo) < brightness(shallow.albedo));
        let calm = water_roughness(0.0);
        let stormy = water_roughness(15.0);
        assert!((0.05..0.06).contains(&calm) && stormy > 0.25 && stormy <= MAX_ROUGHNESS);
        // January is windier in this cell: rougher.
        let january = material(&cell(-5000.0, 20.0, 1.0, 0.0), Period::January, false);
        assert!(january.roughness.unwrap() > deep.roughness.unwrap());
        // Water is told from land by the roughness (< 0.5) in the alpha channel.
        assert!(deep.roughness.unwrap() < 0.5);
    }

    #[test]
    fn plausible_albedos() {
        let forest = material(&cell(300.0, 22.0, 2.0, 1.2), Period::Annual, false).albedo;
        let grassland = material(&cell(300.0, 18.0, 0.6, 1.2), Period::Annual, false).albedo;
        let desert = material(&cell(300.0, 25.0, 0.05, 2.0), Period::Annual, false).albedo;
        let mut ice = cell(1500.0, -25.0, 0.2, 0.0);
        ice.ice = 1.0;
        let ice = material(&ice, Period::Annual, false).albedo;
        let ocean = material(&cell(-4000.0, 15.0, 1.0, 0.0), Period::Annual, false).albedo;
        // Usual ranges: ocean < 0.06, forest 0.05–0.1, grassland 0.1–0.2, desert 0.3–0.4,
        // ice 0.7–0.9; forest greener than desert.
        assert!(brightness(ocean) < 0.03, "{ocean:?}");
        assert!((0.04..0.1).contains(&brightness(forest)), "{forest:?}");
        assert!(
            (0.08..0.2).contains(&brightness(grassland)),
            "{grassland:?}"
        );
        assert!((0.3..0.42).contains(&brightness(desert)), "{desert:?}");
        assert!((0.7..0.9).contains(&brightness(ice)), "{ice:?}");
        assert!(greenness(forest) > greenness(desert) + 0.1);
    }

    #[test]
    fn drier_land_has_less_vegetation_and_abiotic_none() {
        let mut previous = 1.0;
        for precipitation in [3.0, 1.5, 0.8, 0.4, 0.2, 0.05] {
            let albedo = material(
                &cell(300.0, 20.0, precipitation, 1.5),
                Period::Annual,
                false,
            )
            .albedo;
            let green = greenness(albedo);
            assert!(green <= previous + 1e-6, "{precipitation}: {albedo:?}");
            previous = green;
        }
        let humid = cell(300.0, 20.0, 3.0, 1.5);
        let bare = material(&humid, Period::Annual, true).albedo;
        assert!(greenness(bare) < 0.35, "{bare:?}");
        assert!(greenness(material(&humid, Period::Annual, false).albedo) > 0.4);
    }

    #[test]
    fn seasons_bring_snow_and_sea_ice() {
        // A continental cell: annual 0 °C, January −10 °C, July +10 °C.
        let land = cell(400.0, 0.0, 0.5, 1.0);
        let annual = material(&land, Period::Annual, false).albedo;
        let january = material(&land, Period::January, false).albedo;
        let july = material(&land, Period::July, false).albedo;
        assert!(brightness(january) > 0.4, "{january:?}");
        assert!(brightness(july) < 0.2 && brightness(annual) < 0.2);
        // Cold ocean freezes in its winter month only (ice in part of the year).
        let sea = cell(-3000.0, 2.0, 0.5, 0.0);
        let mut sea = sea;
        sea.temperature_january = -12.0;
        sea.ice = 0.5;
        assert!(close(
            material(&sea, Period::January, false).albedo,
            SEA_ICE
        ));
        assert!(material(&sea, Period::July, false).roughness.is_some());
        assert!(material(&sea, Period::Annual, false).roughness.is_some());
    }

    #[test]
    fn lakes_by_depth_salinity_wind_and_ice() {
        let lake = |depth: f32, precipitation: f32| CellInfo {
            drainage: CellInfo::drainage_bits(Some(0), false, true, f64::from(depth)),
            lake_depth: depth,
            ..cell(200.0, 8.0, precipitation, 1.0)
        };
        let annual = |cell: &CellInfo, closed: bool| layers(cell, Period::Annual, false, closed);
        // Deeper is darker.
        let shallow = annual(&lake(2.0, 1.2), false);
        let deep = annual(&lake(300.0, 1.2), false);
        assert!(close(shallow.water, LAKE_SHALLOW) && close(deep.water, LAKE_DEEP));
        assert!(brightness(deep.water) < brightness(shallow.water));
        // Saline: only a closed lake in a dry climate, and a shallow one most.
        let dry_closed = annual(&lake(5.0, 0.15), true);
        assert!(brightness(dry_closed.water) > brightness(shallow.water));
        assert!(close(
            annual(&lake(5.0, 0.15), false).water,
            annual(&lake(5.0, 1.2), false).water
        ));
        assert!(close(
            annual(&lake(5.0, 1.2), true).water,
            annual(&lake(5.0, 1.2), false).water
        ));
        // The sub-cell lakes of humid land have the lake colour, not the ocean's.
        let land = annual(&cell(200.0, 8.0, 1.2, 1.0), false);
        assert!(close(land.water, LAKE_SHALLOW));
        // Calmer than the ocean under the same wind.
        let ocean = annual(&cell(-3000.0, 8.0, 1.0, 0.0), false);
        assert!(shallow.water_roughness < ocean.water_roughness);
        // Lake ice in a cold month, not in a mild one, nor in the annual state of a mild
        // climate.
        let mut cold = lake(50.0, 1.2);
        cold.temperature_january = -12.0;
        cold.temperature_july = 2.0;
        assert_eq!(layers(&cold, Period::January, false, false).snow_cover, 1.0);
        assert_eq!(layers(&cold, Period::July, false, false).snow_cover, 0.0);
        assert_eq!(annual(&cold, false).snow_cover, 0.0);
        assert!(close(
            layers(&cold, Period::January, false, false).snow,
            LAKE_ICE
        ));
        // A subarctic lake (annual −14 °C) thaws in a +10 °C July.
        let mut arctic = lake(7.0, 0.4);
        arctic.temperature = -14.0;
        arctic.temperature_january = -33.0;
        arctic.temperature_july = 10.0;
        assert_eq!(layers(&arctic, Period::July, false, false).snow_cover, 0.0);
        assert_eq!(
            layers(&arctic, Period::January, false, false).snow_cover,
            1.0
        );
        // Deep lakes freeze later than shallow ones.
        assert!(lake_ice(-5.0, 300.0) < lake_ice(-5.0, 5.0));
        assert_eq!(lake_ice(0.0, 5.0), 0.0);
    }

    #[test]
    fn dry_basins_have_salt_flats_at_their_sink() {
        let basin = |receiver: Option<usize>, precipitation: f32| CellInfo {
            drainage: CellInfo::drainage_bits(receiver, false, true, 0.0),
            ..cell(-50.0, 22.0, precipitation, 2.0)
        };
        let albedo = |cell: &CellInfo| layers(cell, Period::Annual, false, false);
        let sink = albedo(&basin(None, 0.1));
        assert!(!sink.water_cell);
        let slope = albedo(&basin(Some(1), 0.1));
        assert!(
            brightness(sink.bare) > brightness(slope.bare) + 0.05,
            "{sink:?}"
        );
        assert!(sink.cover <= 0.5 * slope.cover + 1e-6);
        assert!(brightness(sink.bare) < 0.5);
        // Not on the slopes of the basin, nor in a humid one.
        let humid = albedo(&basin(None, 4.0));
        assert!(close(humid.bare, albedo(&basin(Some(1), 4.0)).bare));
    }

    #[test]
    fn sea_ice_follows_the_month_within_the_annual_cover() {
        // The annual state shows the annual cover sharpened around one half.
        let mut sea = cell(-3000.0, -5.0, 0.3, 0.0);
        sea.ice = 0.75;
        assert_eq!(layers(&sea, Period::Annual, false, false).snow_cover, 1.0);
        sea.ice = 0.5;
        assert!((layers(&sea, Period::Annual, false, false).snow_cover - 0.5).abs() < 1e-6);
        // Ice all year stays in summer; ice never present never covers a winter.
        assert_eq!(sea_ice(1.0, false), 1.0);
        assert_eq!(sea_ice(0.0, true), 0.0);
        // Winter and summer average to the annual cover (Arctic-like ice stays in July).
        for annual in [0.1, 0.3, 0.5, 0.8] {
            let mean = 0.5 * (sea_ice(annual, true) + sea_ice(annual, false));
            assert!((mean - annual).abs() < 1e-6);
        }
        assert!(sea_ice(0.8, false) > 0.5);
        // Cold ice is snow-covered and brighter than melting ice; all below fresh snow.
        sea.temperature_january = -20.0;
        sea.temperature_july = -0.5;
        sea.ice = 0.95;
        let winter = layers(&sea, Period::January, false, false);
        let summer = layers(&sea, Period::July, false, false);
        assert!(winter.ocean && winter.snow_cover == 1.0);
        assert!(brightness(winter.snow) > brightness(summer.snow) + 0.15);
        assert!(brightness(winter.snow) < 0.86 && brightness(SALT_FLAT) < 0.86);
    }

    #[test]
    fn ridges_line_up_along_a_mountain_belt() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, plate_infos};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "4"]);
        let planet = crate::build_planet(&cli).unwrap();
        let mesh = planet.tectonics().mesh();
        let plates = plate_infos(&planet);
        // Land everywhere, with a north–south range 4 km high and ~400 km wide around
        // longitude 0 (slopes up to ~20 m/km).
        let longitude = |cell: u32| {
            let p = mesh.vertex(cell);
            p.y.atan2(p.x).to_degrees() as f32
        };
        let cells: Vec<CellInfo> = (0..mesh.len() as u32)
            .map(|cell| {
                let x = longitude(cell) / 2.0;
                CellInfo {
                    drainage: CellInfo::drainage_bits(Some(0), false, false, 0.0),
                    orogenic: 1.0,
                    ..cell_at(200.0 + 4000.0 * (-x * x).exp())
                }
            })
            .collect();
        let vertices = super::super::super::surface_vertices(mesh, &cells, 1.0, 1e11);
        let (plate_colors, fields) = (PlateColors::default(), Fields::default());
        let options = OptionValues::defaults(Realistic.options());
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &plates,
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &[],
            arrow_spacing: 0.1,
            options: &options,
            abiotic: false,
        };
        let gradients: Vec<glam::DVec3> = (0..cells.len())
            .map(|cell| surface_gradient(&data, cell))
            .collect();
        let latitude = |cell: u32| mesh.vertex(cell).z.asin().to_degrees();
        let (mut flank, mut crest, mut plain) = (0, 0, 0);
        let mut belt = Vec::new();
        for cell in 0..mesh.len() as u32 {
            if latitude(cell).abs() > 50.0 {
                continue;
            }
            let (t, squeeze) = strike(&data, &gradients, cell as usize, 1.0);
            let lon = longitude(cell).abs();
            if lon < 4.0 {
                // Along the belt wherever it lines up (ŝ_z² = cos² of the latitude for a
                // meridian).
                if squeeze > 0.05 {
                    let north = t[2] * t[2];
                    let meridian = latitude(cell).to_radians().cos().powi(2) as f32;
                    assert!(north > 0.8 * meridian, "{lon}: {t:?}");
                }
                if squeeze > 0.15 {
                    if lon < 1.5 { crest += 1 } else { flank += 1 }
                }
                belt.push(squeeze);
            } else if lon > 20.0 {
                assert_eq!(squeeze, 0.0);
                plain += 1;
            }
        }
        // Crests line up too (from the flanks), and most of the belt strongly.
        assert!(
            crest > 0 && flank > 0 && plain > 0,
            "{crest} {flank} {plain}"
        );
        belt.sort_by(f32::total_cmp);
        assert!(belt[belt.len() / 2] > 0.15, "{belt:?}");
    }

    #[test]
    fn lakes_below_sea_level_take_the_air_of_the_land_around() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, plate_infos};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let mesh = planet.tectonics().mesh();
        let plates = plate_infos(&planet);
        // Cold land at +500 m, a lake of two rings below sea level with the ocean's warmth.
        let lake: Vec<u32> = std::iter::once(0)
            .chain(mesh.neighbors(0).iter().copied())
            .collect();
        let cells: Vec<CellInfo> = (0..mesh.len() as u32)
            .map(|cell| {
                if lake.contains(&cell) {
                    CellInfo {
                        drainage: CellInfo::drainage_bits(Some(0), false, true, 300.0),
                        lake_depth: 300.0,
                        ..cell_at(-100.0)
                    }
                } else {
                    CellInfo {
                        temperature_january: -20.0,
                        ..cell_at(500.0)
                    }
                }
            })
            .collect();
        let vertices = super::super::super::surface_vertices(mesh, &cells, 1.0, 1e11);
        let (plate_colors, fields) = (PlateColors::default(), Fields::default());
        let options = OptionValues::defaults(Realistic.options());
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &plates,
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &[],
            arrow_spacing: 0.1,
            options: &options,
            abiotic: false,
        };
        let air = lake_air(&data);
        for &cell in &lake {
            assert_eq!(air[cell as usize].temperature_january, -20.0);
        }
        assert_eq!(cells[0].temperature_january, 0.0);
        // So the lake freezes in January.
        assert_eq!(
            layers(&air[0], Period::January, false, false).snow_cover,
            1.0
        );
    }

    #[test]
    fn corners_pack_as_the_shader_unpacks() {
        let word = pack_corner([0.6, -1.0, 0.0], 0.5, true);
        // The shader: bitcast<i32>(word << (24 − 8 k)) >> 24 per byte.
        let byte = |k: u32| ((word << (24 - 8 * k)) as i32 >> 24) as f32 / 127.0;
        assert!((byte(0) - 0.6).abs() < 0.01);
        assert_eq!(byte(1), -1.0);
        assert_eq!(byte(2), 0.0);
        let squeeze = ((word >> 24) & 127) as f32 / 127.0 * MAX_SQUEEZE;
        assert!((squeeze - 0.5).abs() < 0.005);
        assert_eq!(word >> 31, 1);
        assert_eq!(pack_corner([0.0; 3], 0.0, false), 0);
    }

    #[test]
    fn closed_lakes_have_no_outflow() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, plate_infos};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let mesh = planet.tectonics().mesh();
        let plates = plate_infos(&planet);
        // Two lakes of two cells each: in one the water flows to an exit without a receiver,
        // in the other it spills to the land beyond.
        let slot = |from: u32, to: u32| mesh.neighbors(from).iter().position(|&n| n == to);
        let a = 0;
        let a2 = mesh.neighbors(a)[0];
        let b = (0..mesh.len() as u32)
            .find(|&cell| mesh.vertex(cell).dot(mesh.vertex(a)) < -0.5)
            .unwrap();
        let b2 = mesh.neighbors(b)[0];
        let beyond = *mesh
            .neighbors(b2)
            .iter()
            .find(|&&n| n != b && !mesh.neighbors(b).contains(&n))
            .unwrap();
        let lake = |receiver: Option<usize>| CellInfo {
            drainage: CellInfo::drainage_bits(receiver, false, true, 20.0),
            lake_depth: 20.0,
            ..cell_at(100.0)
        };
        let cells: Vec<CellInfo> = (0..mesh.len() as u32)
            .map(|cell| match cell {
                c if c == a => lake(slot(a, a2)),
                c if c == a2 => lake(None),
                c if c == b => lake(slot(b, b2)),
                c if c == b2 => lake(slot(b2, beyond)),
                _ => cell_at(300.0),
            })
            .collect();
        let vertices = super::super::super::surface_vertices(mesh, &cells, 1.0, 1e11);
        let (plate_colors, fields) = (PlateColors::default(), Fields::default());
        let options = OptionValues::defaults(Realistic.options());
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &plates,
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &[],
            arrow_spacing: 0.1,
            options: &options,
            abiotic: false,
        };
        let closed = closed_lakes(&data);
        assert!(closed[a as usize] && closed[a2 as usize]);
        assert!(!closed[b as usize] && !closed[b2 as usize]);
        assert_eq!(closed.iter().filter(|&&c| c).count(), 2);
    }

    fn cell_at(elevation: f32) -> CellInfo {
        cell(elevation, 10.0, 1.0, 1.0)
    }

    #[test]
    fn high_mountains_are_bare_and_cold_summers_tundra() {
        let summit = material(&cell(5000.0, 5.0, 1.0, 0.8), Period::Annual, false).albedo;
        assert!(close(summit, ROCK), "{summit:?}");
        let mut cold = cell(200.0, -12.0, 0.4, 0.3);
        cold.temperature_july = 3.0;
        cold.temperature_january = -25.0;
        let tundra = material(&cold, Period::Annual, false).albedo;
        let distance = (0..3)
            .map(|i| (tundra[i] - TUNDRA[i]).abs())
            .fold(0.0, f32::max);
        assert!(distance < 0.03, "{tundra:?}");
    }

    #[test]
    fn the_detail_mesh_carries_the_layers_of_its_corners() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, plate_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = super::super::super::cell_infos(&planet);
        let plates = plate_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 1.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let options = OptionValues::defaults(Realistic.options());
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &plates,
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &[],
            arrow_spacing: 0.1,
            options: &options,
            abiotic: false,
        };
        let faces = mesh.faces();
        let detail = detail_vertices(&data, faces);
        assert_eq!(detail.len(), faces.len() * 3);
        let mut coasts = 0;
        for (triangle, face) in detail.chunks(3).zip(faces) {
            for (corner, vertex) in triangle.iter().enumerate() {
                let cell = face[corner] as usize;
                assert_eq!(vertex.position, vertices[cell].position);
                assert_eq!(usize::from(vertex.plates[3]), corner);
                let corner_plates = face.map(|cell| cells[cell as usize].plate as u16);
                assert_eq!(vertex.plates[..3], corner_plates);
                // The coast value has the sign of the corner's class: land or water.
                let layers = layers(&cells[cell], Period::Annual, false, false);
                assert_eq!(vertex.coast < 0.0, layers.water_cell);
                assert!(vertex.coast.abs() >= 0.4 && vertex.coast.abs() <= 1.0);
                // Dry land opens no sub-cell lakes: above the noise's reach (0.5).
                if !layers.water_cell && moisture_index(&cells[cell]) < 0.3 {
                    assert!(vertex.coast >= 0.55 - 1e-6, "{}", vertex.coast);
                }
                // The ocean flag of the corner.
                assert_eq!(vertex.ridges[corner] >> 31 == 1, layers.ocean);
            }
            if triangle[0].coast.signum() != triangle[1].coast.signum() {
                coasts += 1;
            }
        }
        // The planet has coasts, where water corners take the land of their neighbours.
        assert!(coasts > 0);
        // The plates' orientations reach the uniforms; one octave per cell spacing first.
        let uniforms = detail_uniforms(&data);
        assert_eq!(uniforms.orientations[0], plates[0].orientation);
        assert_eq!(uniforms.orientations[plates.len()], [0.0, 0.0, 0.0, 1.0]);
        let spacing = mesh.mean_edge_angle() as f32;
        assert!((uniforms.detail[0] * spacing - 1.0).abs() < 1e-3);
        assert_eq!(uniforms.detail[1], 4.0);
    }

    #[test]
    fn the_detail_moves_with_the_plate() {
        // The shader evaluates the noise at the point in the plate's frame: q⁻¹ · p. When the
        // plate turns by r (q → r q), its crust moves with it (p → r p), and the point in the
        // plate's frame stays: the detail does not change.
        use glam::{Quat, Vec3};
        let q = Quat::from_axis_angle(Vec3::new(0.3, 1.0, -0.2).normalize(), 0.8);
        let r = Quat::from_axis_angle(Vec3::new(-1.0, 0.4, 0.5).normalize(), 0.05);
        let p = Vec3::new(0.2, -0.7, 0.5).normalize();
        let before = q.inverse() * p;
        let after = (r * q).inverse() * (r * p);
        assert!((before - after).length() < 1e-6);
    }

    /// Runs `check` on the mode data of a small planet (3 subdivisions) whose cells `edit` has
    /// changed, with `options`.
    fn with_planet(
        edit: impl FnOnce(&mut [CellInfo], &crate::sphere::SphereMesh),
        options: &OptionValues,
        check: impl FnOnce(&ModeData),
    ) {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, plate_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let mut cells = super::super::super::cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        edit(&mut cells, mesh);
        let plates = plate_infos(&planet);
        let vertices = surface_vertices(mesh, &cells, 1.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &plates,
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &[],
            arrow_spacing: 0.1,
            options,
            abiotic: false,
        };
        check(&data);
    }

    fn clouds_on() -> OptionValues {
        let mut options = OptionValues::defaults(Realistic.options());
        options.set(CLOUDS, 1);
        options
    }

    #[test]
    fn cloud_regimes() {
        let climate = |humidity, column_temperature, precipitation, moist, stratocumulus| {
            cloud_layer(&CloudClimate {
                humidity,
                column_temperature,
                precipitation,
                moist,
                stratocumulus,
            })
        };
        // A dry subtropical desert: almost clear.
        let desert = climate(0.5, 24.0, 0.05, 0.0, 0.0);
        assert!(desert.cover < 0.05, "{desert:?}");
        // A wet storm track: overcast with sheets of cloud.
        let storm_track = climate(0.85, 6.0, 1.0, 1.0, 0.0);
        assert!(storm_track.cover > 0.8, "{storm_track:?}");
        assert!(storm_track.convective < 0.15, "{storm_track:?}");
        assert!(storm_track.top >= 4.0 && storm_track.top <= 8.0);
        // The tropical rain belt: mostly convective, thick, high tops.
        let itcz = climate(0.69, 27.0, 2.5, 1.0, 0.0);
        assert!(itcz.cover > 0.7, "{itcz:?}");
        assert!(
            itcz.convective > 0.4 && itcz.depth > 25.0 && itcz.top > 9.0,
            "{itcz:?}"
        );
        // The trades over the ocean: broken, thin, low cumulus.
        let trades = climate(0.63, 24.0, 0.5, 1.0, 0.0);
        assert!(trades.cover > 0.15 && trades.cover < 0.5, "{trades:?}");
        assert!(trades.convective > 0.7 && trades.depth < 12.0, "{trades:?}");
        // A stratocumulus deck adds cover, low tops.
        let deck = climate(0.55, 22.0, 0.1, 1.0, 0.8);
        assert!(deck.cover > 0.8 && deck.top < 2.5, "{deck:?}");
        // Cold polar cloud is thin.
        let polar = climate(0.85, -25.0, 0.2, 1.0, 0.0);
        assert!(polar.cover > 0.8 && polar.depth < 8.0, "{polar:?}");
        // Unknown climate: no clouds.
        assert_eq!(climate(f32::NAN, 10.0, 1.0, 1.0, 0.0), CloudLayer::CLEAR);
        assert_eq!(climate(0.8, f32::NAN, 1.0, 1.0, 0.0), CloudLayer::CLEAR);
        // Encoded for the shader within its ranges.
        let [cover, convective, depth, top] = itcz.encode();
        assert_eq!(cover, unorm(itcz.cover));
        assert!(convective > 100 && depth > 0 && top > 128);
    }

    #[test]
    fn clouds_follow_the_rain_belt_through_the_year() {
        // A synthetic zonal climate: an ocean planet whose rain belt lies at 10°S in January and
        // 10°N in July, dry subtropics, wet storm tracks at 50°.
        let belt = |latitude: f32, centre: f32| {
            0.3 + 2.5 * (-((latitude - centre) / 8.0).powi(2)).exp()
                + 0.9 * (-((latitude.abs() - 50.0) / 10.0).powi(2)).exp()
        };
        let edit = |cells: &mut [CellInfo], mesh: &crate::sphere::SphereMesh| {
            for (index, cell) in cells.iter_mut().enumerate() {
                let latitude = mesh.vertex(index as u32).z.asin().to_degrees() as f32;
                let temperature = 28.0 - 0.006 * latitude * latitude;
                *cell = CellInfo {
                    elevation: -4000.0,
                    temperature,
                    temperature_january: temperature,
                    temperature_july: temperature,
                    precipitation: 0.5 * (belt(latitude, -10.0) + belt(latitude, 10.0)),
                    precipitation_january: belt(latitude, -10.0),
                    precipitation_july: belt(latitude, 10.0),
                    evaporation: 1.0,
                    evaporation_january: 1.0,
                    evaporation_july: 1.0,
                    ..*cell
                };
            }
        };
        with_planet(edit, &clouds_on(), |data| {
            let band = |layers: &[CloudLayer], from: f64, to: f64| {
                let (mut sum, mut count) = (0.0, 0.0);
                for (index, layer) in layers.iter().enumerate() {
                    let latitude = data.mesh.vertex(index as u32).z.asin().to_degrees();
                    if latitude >= from && latitude < to {
                        sum += layer.cover;
                        count += 1.0;
                    }
                }
                sum / count
            };
            let january = cloud_layers(data, Period::January);
            let july = cloud_layers(data, Period::July);
            let year = cloud_layers(data, Period::Annual);
            assert!(band(&january, -15.0, -5.0) > band(&january, 5.0, 15.0) + 0.1);
            assert!(band(&july, 5.0, 15.0) > band(&july, -15.0, -5.0) + 0.1);
            // The subtropics are clearer than the rain belt and the storm tracks.
            assert!(band(&january, 20.0, 35.0) < band(&january, -15.0, -5.0) - 0.2);
            assert!(band(&january, 20.0, 35.0) < band(&january, 40.0, 60.0));
            // The year is the mean of January and July (before the smoothing, which is linear).
            let mean = 0.5 * (band(&january, -15.0, 15.0) + band(&july, -15.0, 15.0));
            assert!((band(&year, -15.0, 15.0) - mean).abs() < 0.03);
        });
    }

    #[test]
    fn clouds_only_with_the_switch() {
        let faces = |data: &ModeData| detail_vertices(data, data.mesh.faces());
        let options = OptionValues::defaults(Realistic.options());
        assert!(!options.is_on(CLOUDS));
        with_planet(
            |_, _| {},
            &options,
            |data| {
                assert!(faces(data).iter().all(|vertex| vertex.clouds == [0; 4]));
                assert_eq!(detail_uniforms(data).clouds, [0.0; 4]);
            },
        );
        with_planet(
            |_, _| {},
            &clouds_on(),
            |data| {
                let vertices = faces(data);
                let cloudy = vertices
                    .iter()
                    .filter(|vertex| vertex.clouds[0] > 0)
                    .count();
                assert!(
                    cloudy > vertices.len() / 2,
                    "{cloudy} of {}",
                    vertices.len()
                );
                // The tops lie at least 1 km above the ground.
                for (layer, cell) in cloud_layers(data, Period::Annual).iter().zip(data.cells) {
                    if layer.cover > 0.0 {
                        assert!(layer.top >= 1.0, "{layer:?}, {} m", cell.elevation);
                    }
                }
                let uniforms = detail_uniforms(data);
                assert_eq!(uniforms.clouds[0], 4.0);
                assert_eq!(uniforms.clouds[1], CLOUD_FREQUENCY);
            },
        );
        // Without a climate (NaN fields): no clouds.
        let unknown = |cells: &mut [CellInfo], _: &crate::sphere::SphereMesh| {
            for cell in cells {
                cell.precipitation = f32::NAN;
                cell.precipitation_january = f32::NAN;
                cell.precipitation_july = f32::NAN;
                cell.temperature = f32::NAN;
                cell.temperature_january = f32::NAN;
                cell.temperature_july = f32::NAN;
            }
        };
        with_planet(unknown, &clouds_on(), |data| {
            for season in [Period::Annual, Period::January, Period::July] {
                assert!(
                    cloud_layers(data, season)
                        .iter()
                        .all(|layer| *layer == CloudLayer::CLEAR)
                );
            }
        });
    }

    /// The value of `realistic.wgsl`'s gradient noise (a port for the tests).
    fn shader_noise(x: [f32; 3]) -> f32 {
        let gradient = |p: [i32; 3]| {
            let mut v = p.map(|c| {
                (c as u32)
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223)
            });
            let mix = |v: &mut [u32; 3]| {
                v[0] = v[0].wrapping_add(v[1].wrapping_mul(v[2]));
                v[1] = v[1].wrapping_add(v[2].wrapping_mul(v[0]));
                v[2] = v[2].wrapping_add(v[0].wrapping_mul(v[1]));
            };
            mix(&mut v);
            for c in &mut v {
                *c ^= *c >> 16;
            }
            mix(&mut v);
            v.map(|c| (c >> 8) as f32 / 8_388_608.0 - 1.0)
        };
        let i = x.map(f32::floor);
        let f = [0, 1, 2].map(|k| x[k] - i[k]);
        let u = f.map(|f| f * f * f * (f * (f * 6.0 - 15.0) + 10.0));
        let mut value = 0.0;
        for corner in 0..8 {
            let o = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
            let g = gradient([0, 1, 2].map(|k| i[k] as i32 + o[k]));
            let d: f32 = (0..3).map(|k| g[k] * (f[k] - o[k] as f32)).sum();
            let weight: f32 = (0..3)
                .map(|k| if o[k] == 1 { u[k] } else { 1.0 - u[k] })
                .product();
            value += weight * d;
        }
        value
    }

    /// A constant of `clouds.wgsl`.
    fn cloud_constant(name: &str) -> f32 {
        let source = include_str!("../clouds.wgsl");
        let start = format!("const {name}: f32 = ");
        let line = source
            .lines()
            .find_map(|line| line.strip_prefix(&start))
            .unwrap_or_else(|| panic!("{name}"));
        line.trim_end_matches(';').parse().unwrap()
    }

    #[test]
    fn the_cloud_cover_does_not_depend_on_the_level_of_detail() {
        // The share of points where `cloud_field` (without its swirl) exceeds the threshold of a
        // cover is the cover, for any number of octaves and both kinds of spectrum: the sum is
        // normalised by the amplitudes it summed, and the threshold is the normal quantile.
        let deviation = cloud_constant("NOISE_DEVIATION");
        let pressure_weight = cloud_constant("CLOUD_PRESSURE_WEIGHT");
        let pressure_frequency = cloud_constant("CLOUD_PRESSURE_FREQUENCY");
        let zonal = cloud_constant("CLOUD_ZONAL");
        assert!(include_str!("../clouds.wgsl").contains("return 0.5513 * log(p / (1.0 - p));"));
        let quantile = |p: f32| 0.5513 * (p / (1.0 - p)).ln();
        let points: Vec<[f32; 3]> = (0..20_000)
            .map(|i| {
                // A Fibonacci lattice on the sphere.
                let z = 1.0 - (2.0 * i as f32 + 1.0) / 20_000.0;
                let angle = i as f32 * 2.399_963;
                let r = (1.0 - z * z).sqrt();
                [r * angle.cos(), r * angle.sin(), z * zonal]
            })
            .collect();
        for octaves in [1.0, 2.0, 2.5, 4.0, 6.0_f32] {
            for (persistence, synoptic) in [(0.5, 1.0), (1.2, 0.1)] {
                let field = |q: &[f32; 3]| {
                    let w = pressure_weight * synoptic;
                    let pressure = shader_noise([
                        q[0] * pressure_frequency + 3.7,
                        q[1] * pressure_frequency - 1.3,
                        q[2] * pressure_frequency + 8.9,
                    ]);
                    let mut sum = -w * pressure / deviation;
                    let mut variance = w * w;
                    let (mut amplitude, mut frequency) = (1.0, CLOUD_FREQUENCY);
                    for octave in 0..octaves.ceil() as usize {
                        // The last, fractional octave fades in.
                        let a = amplitude * (octaves - octave as f32).min(1.0);
                        let offset = octave as f32 * 5.17 - 11.0;
                        sum += a * shader_noise(q.map(|c| c * frequency + offset)) / deviation;
                        variance += a * a;
                        amplitude *= persistence;
                        frequency *= 2.0;
                    }
                    sum / variance.sqrt()
                };
                let values: Vec<f32> = points.iter().map(field).collect();
                for cover in [0.1, 0.3, 0.5, 0.7, 0.9] {
                    let threshold = -quantile(cover);
                    let share = values.iter().filter(|&&v| v > threshold).count() as f32
                        / values.len() as f32;
                    // (Within 0.06: the pressure field has only a few dozen lows and highs
                    // on the sphere, whose mean is not quite zero.)
                    assert!(
                        (share - cover).abs() < 0.06,
                        "{octaves} octaves, persistence {persistence}: {cover} -> {share}"
                    );
                }
            }
        }
    }

    #[test]
    fn clouds_stay_below_the_knee() {
        // No cloud, nor a cloud with the ground beneath it, is brighter than the shoulder's knee
        // (the brightest surface), also with the atmosphere off: the exposure's cap holds with
        // the clouds. A port of `cloud_reflectance` and of the composition in `realistic.wgsl`
        // (reflectance factors at the capped incidence), over optical depths, sun heights,
        // shadows and grounds up to fresh snow and beyond.
        let knee = crate::viewer::atmosphere::SHOULDER_KNEE as f32;
        assert_eq!(cloud_constant("CLOUD_KNEE"), knee);
        let max_reflectance = cloud_constant("CLOUD_MAX_REFLECTANCE");
        assert!(max_reflectance < knee);
        let g = cloud_constant("CLOUD_G");
        let clouds = include_str!("../clouds.wgsl");
        assert!(
            clouds.contains("let x = (1.0 - CLOUD_G) * tau * 0.5 * (1.0 + 1.0 / max(mu0, 0.15));")
        );
        let reflectance = |tau: f32, mu0: f32| {
            let x = (1.0 - g) * tau * 0.5 * (1.0 + 1.0 / mu0.max(0.15));
            (x / (2.0 + x)).min(max_reflectance)
        };
        let shader = include_str!("../realistic.wgsl");
        assert!(
            shader.contains(
                "max(CLOUD_KNEE - cloud_reflection, 0.0) / max(brightest * shadow, 1e-4)"
            )
        );
        for tau in [0.0, 0.5, 2.0, 6.0, 15.0, 40.0, 64.0] {
            for mu0 in [0.02, 0.1, 0.3, 0.6, 1.0] {
                let r = reflectance(tau, mu0);
                for shadow_tau in [0.0, tau, 40.0] {
                    let shadow = 1.0 - reflectance(shadow_tau, mu0);
                    for ground in [0.0, 0.3, 0.6, 0.86, 0.95] {
                        let mut seen = 1.0 - r;
                        if r > 0.0 {
                            seen = seen.min((knee - r).max(0.0) / (ground * shadow).max(1e-4));
                        }
                        let total = r + seen * shadow * ground;
                        assert!(
                            total <= knee.max(ground) + 1e-6,
                            "τ {tau}, μ₀ {mu0}, shadow {shadow}, ground {ground}: {total}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn quality_levels() {
        let mut options = OptionValues::defaults(Realistic.options());
        assert_eq!(Quality::of(&options), Quality::Medium);
        options.set(DETAIL, 1);
        assert_eq!(Quality::of(&options), Quality::Low);
        assert_eq!(Quality::Low.settings(), (2.0, 0.0, false));
        options.set(DETAIL, 2);
        assert_eq!(Quality::of(&options).settings().0, 6.0);
    }
}
