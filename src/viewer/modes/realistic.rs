//! The realistic mode (#92): the planet as a camera in space would see it.
//!
//! Every cell gets a surface material on the CPU from its data ([`material`]), which the planet
//! shader lights in linear light with the simulation's sun (`planet.wgsl`, realistic branch):
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
//! glint, forest 0.04–0.1, grassland 0.08–0.2, desert 0.3–0.4, fresh snow 0.8), stored sRGB-encoded in
//! the vertex colours; the alpha channel tells the shader water (its roughness, < 0.5) from land
//! (1). The relief is true to scale (no exaggeration, decision 6 on #92). Rivers are left out
//! (invisible from space at this scale). The options choose the light (the sun by default) and
//! the season: the sun's declination and distance, snow, sea ice and the greenness of the
//! chosen month, or the annual state.
//!
//! Sub-cell detail, the atmosphere and clouds are later steps (`notes/issue-92/plan.md`); the
//! colours change in steps of a cell, as the data does.

use super::{Legend, ModeData, ModeOption, OptionKind, OptionValues, Period, RenderMode, Swatch};
use crate::viewer::CellInfo;
use crate::viewer::lighting::Light;

pub struct Realistic;

/// Index of the light option.
pub const LIGHT: usize = 0;
/// Index of the season option.
pub const SEASON: usize = 1;

const OPTIONS: &[ModeOption] = &[
    ModeOption {
        label: Light::LABEL,
        hover: Light::HOVER,
        kind: OptionKind::Choice(Light::CHOICES),
    },
    ModeOption {
        label: "Season",
        hover: "The annual state, or January (northern winter) or July (northern summer): the \
                sun's position, seasonal snow and sea ice, and the greenness of the vegetation",
        kind: OptionKind::Choice(Period::CHOICES),
    },
];

/// Reflectances of the surfaces in linear RGB.
const DEEP_OCEAN: [f32; 3] = [0.006, 0.016, 0.042];
const SHELF: [f32; 3] = [0.018, 0.060, 0.070];
const LAKE: [f32; 3] = [0.012, 0.030, 0.040];
const ICE_SHEET: [f32; 3] = [0.78, 0.81, 0.86];
const SNOW: [f32; 3] = [0.80, 0.82, 0.85];
const SEA_ICE: [f32; 3] = [0.55, 0.60, 0.66];
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
/// Most roughness of the water surface (GGX α); the alpha channel of land is 1.
const MAX_ROUGHNESS: f32 = 0.45;

impl RenderMode for Realistic {
    fn label(&self) -> &'static str {
        "Realistic"
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let season = season(data.options);
        Some(
            data.cells
                .iter()
                .map(|cell| {
                    let material = material(cell, season, data.abiotic);
                    let [r, g, b] = material.albedo.map(srgb);
                    [r, g, b, material.roughness.unwrap_or(1.0)]
                })
                .collect(),
        )
    }

    fn options(&self) -> &'static [ModeOption] {
        OPTIONS
    }

    fn legend(&self, _options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let abiotic = data.is_some_and(|data| data.abiotic);
        let swatch = |albedo: [f32; 3], label: &'static str| {
            Swatch::fill(albedo.map(|c| (srgb(c) * 255.0).round() as u8), label)
        };
        let mut swatches = vec![
            swatch(DEEP_OCEAN, "Deep ocean"),
            swatch(SHELF, "Shelf"),
            swatch(ICE_SHEET, "Ice, snow"),
            swatch(SAND, "Desert"),
            swatch(ROCK, "Rock"),
        ];
        if !abiotic {
            swatches.extend([
                swatch(STEPPE, "Steppe"),
                swatch(GRASSLAND, "Grassland"),
                swatch(FOREST, "Forest"),
                swatch(TUNDRA, "Tundra"),
            ]);
        }
        Legend {
            description: "The planet in space, lit by the simulation's sun: surface colours \
                          derived from the simulated climate (vegetation from rain and warmth, \
                          none without life), relief true to scale, sun glint on the water. \
                          The view adapts to the light like an eye: after the sun goes behind \
                          the planet or out of view, more stars appear over a few seconds."
                .into(),
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

/// The season chosen in the options of the realistic mode.
pub fn season(options: &OptionValues) -> Period {
    Period::of(options, SEASON)
}

/// The surface of a cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Reflectance in linear RGB.
    pub albedo: [f32; 3],
    /// For water, the roughness of its surface (GGX α); `None` for land and ice.
    pub roughness: Option<f32>,
}

/// The surface of a cell in a season (the annual state for [`Period::Annual`]).
pub fn material(cell: &CellInfo, season: Period, abiotic: bool) -> Material {
    let finite = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
    let annual_temperature = finite(cell.temperature, 10.0);
    let temperature = finite(season.temperature(cell), annual_temperature);
    let warmest = finite(
        cell.temperature_january.max(cell.temperature_july),
        annual_temperature,
    );
    let precipitation = finite(season.precipitation(cell), cell.precipitation.max(0.0));
    // Seasonal snow and sea ice where the chosen month freezes (none in the annual state,
    // which shows the permanent ice only).
    let freezing = match season {
        Period::Annual => 0.0,
        _ => smoothstep(-1.0, -6.0, temperature),
    };
    let land = cell.elevation >= 0.0;
    if cell.ice > 0.5 {
        let albedo = if land { ICE_SHEET } else { SEA_ICE };
        return Material {
            albedo,
            roughness: None,
        };
    }
    if !land || cell.lake_depth > 0.0 {
        let water = if land {
            LAKE
        } else {
            let depth = (-cell.elevation).max(1.0);
            let t = ((depth / SHELF_DEPTH).ln() / (DEEP_DEPTH / SHELF_DEPTH).ln()).clamp(0.0, 1.0);
            mix(SHELF, DEEP_OCEAN, t)
        };
        if freezing > 0.5 {
            return Material {
                albedo: SEA_ICE,
                roughness: None,
            };
        }
        let [east, north] = season.wind(cell);
        let wind = finite((east * east + north * north).sqrt(), 7.0);
        return Material {
            albedo: water,
            roughness: Some(water_roughness(wind)),
        };
    }

    // Moisture index P / PET of the year (1 humid, < 0.2 arid).
    let moisture = if cell.potential_evaporation > 0.0 {
        finite(cell.precipitation / cell.potential_evaporation, 1.0)
    } else {
        1.0
    };
    let arid = 1.0 - smoothstep(0.1, 0.4, moisture);
    let bare = mix(mix(ROCK, SOIL, 0.5), SAND, arid);
    let ground = if abiotic {
        bare
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
        let plants = mix(grass, FOREST, trees);
        let ground = mix(bare, plants, cover);
        // Where summers stay cold: tundra.
        mix(ground, TUNDRA, smoothstep(8.0, 2.0, warmest) * (1.0 - arid))
    };
    // Bare rock above the tree line of high mountains.
    let ground = mix(ground, ROCK, smoothstep(2500.0, 4500.0, cell.elevation));
    // Seasonal snow where there is precipitation to fall as snow; forests hide most of it.
    let snow_cover = freezing * smoothstep(0.02, 0.15, precipitation);
    let forested = if abiotic {
        0.0
    } else {
        smoothstep(0.5, 1.0, moisture) * smoothstep(6.0, 12.0, warmest)
    };
    let snow = mix(SNOW, SNOWY_FOREST, forested);
    Material {
        albedo: mix(ground, snow, snow_cover),
        roughness: None,
    }
}

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
        // Cold ocean freezes in its winter month only.
        let sea = cell(-3000.0, 2.0, 0.5, 0.0);
        let mut sea = sea;
        sea.temperature_january = -12.0;
        assert_eq!(material(&sea, Period::January, false).albedo, SEA_ICE);
        assert!(material(&sea, Period::July, false).roughness.is_some());
        assert!(material(&sea, Period::Annual, false).roughness.is_some());
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
}
