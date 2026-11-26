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
    ModeOption {
        label: "Detail",
        hover: "Sub-cell detail (coastlines, snow and tree lines, colour variation, relief), \
                generated on the fly with more octaves where a pixel covers less ground: Low \
                (2 octaves, one pattern, no relief) for slow devices, Medium (4), High (6, ridged \
                mountains)",
        kind: OptionKind::Choice(Quality::CHOICES),
    },
    ModeOption {
        label: "Atmosphere",
        hover: "The air and its haze, true to scale: a thin glow at the limb, a pale haze \
                towards the edge of the disc, sunlight reddened near the terminator, and a ring \
                of light around the planet when the sun is behind it",
        kind: OptionKind::Switch { default: true },
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
}

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
                          none without life), relief and atmosphere true to scale, sun glint on \
                          the water. \
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
    /// Whether the cell is water (ocean or lake).
    pub water_cell: bool,
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
    let layers = layers(cell, season, abiotic);
    Material {
        albedo: layers.albedo(),
        roughness: (layers.water_cell && layers.snow_cover <= 0.5)
            .then_some(layers.water_roughness),
        layers,
    }
}

fn layers(cell: &CellInfo, season: Period, abiotic: bool) -> Layers {
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
    // Ocean, lakes (at least `LAKE_DEPTH` deep) and land, also dry closed depressions below sea
    // level (#128).
    let body = cell.water_body();
    let ocean = body == WaterBody::Ocean;
    let water_cell = cell.is_water();
    let land = !water_cell;
    let water = if ocean {
        let depth = (-cell.elevation).max(1.0);
        let t = ((depth / SHELF_DEPTH).ln() / (DEEP_DEPTH / SHELF_DEPTH).ln()).clamp(0.0, 1.0);
        mix(SHELF, DEEP_OCEAN, t)
    } else if water_cell {
        LAKE
    } else {
        SHELF
    };
    let [east, north] = season.wind(cell);
    let wind = finite((east * east + north * north).sqrt(), 7.0);
    let water_roughness = water_roughness(wind);

    // Moisture index P / PET of the year (1 humid, < 0.2 arid).
    let moisture = if cell.potential_evaporation > 0.0 {
        finite(cell.precipitation / cell.potential_evaporation, 1.0)
    } else {
        1.0
    };
    let arid = 1.0 - smoothstep(0.1, 0.4, moisture);
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
    let (bare, plants) = (adjust(bare), adjust(plants));
    // Bump relief: high land and young orogens are rugged.
    let relief = if land {
        (smoothstep(300.0, 3000.0, cell.elevation) * 0.7 + 0.3 * cell.orogenic.clamp(0.0, 1.0))
            .clamp(0.0, 1.0)
    } else {
        0.0
    };

    let (snow, snow_cover) = if cell.ice > 0.5 {
        (if land { ICE_SHEET } else { SEA_ICE }, 1.0)
    } else if ocean {
        (SEA_ICE, freezing)
    } else if water_cell {
        // Frozen lakes look like snow-covered land.
        (SNOW, freezing)
    } else {
        // Seasonal snow where there is precipitation to fall as snow; forests hide most of it.
        let snow_cover = freezing * smoothstep(0.02, 0.15, precipitation);
        (mix(SNOW, SNOWY_FOREST, forested), snow_cover)
    };
    Layers {
        water_cell,
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
    let own: Vec<Layers> = data
        .cells
        .iter()
        .map(|cell| layers(cell, season, data.abiotic))
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
    // The attributes of a corner depend only on its cell: encoded once per cell rather than
    // once per corner (six times as often), since this runs on the page's main thread for
    // every new state (#174).
    let color = |albedo: [f32; 3], alpha: f32| {
        let [r, g, b] = albedo.map(srgb8);
        [r, g, b, unorm(alpha)]
    };
    let corners: Vec<Corner> = layers
        .iter()
        .zip(data.cells)
        .map(|(layers, info)| {
            // The coast lies where this crosses zero between a land and a water corner: about
            // in the middle of the edge, nearer to low land and shallow water.
            let coast = if let WaterBody::Lake(depth) = info.water_body() {
                // At least 0.5, so that the noise (±0.5) does not erase small lakes.
                -(0.5 + 0.5 * smoothstep(0.0, 200.0, depth))
            } else if layers.water_cell {
                -(0.4 + 0.6 * smoothstep(0.0, 2000.0, -info.elevation))
            } else {
                0.4 + 0.6 * smoothstep(0.0, 2000.0, info.elevation)
            };
            Corner {
                bare: color(layers.bare, layers.cover),
                plants: color(layers.plants, layers.snow_cover),
                snow: color(layers.snow, layers.water_roughness / 0.5),
                water: color(layers.water, layers.relief),
                coast,
            }
        })
        .collect();
    let plate = |cell: u32| data.cells[cell as usize].plate.min(u32::from(u16::MAX)) as u16;
    let mut vertices = Vec::with_capacity(faces.len() * 3);
    for &face in faces {
        let plates = face.map(plate);
        for (corner, &cell) in face.iter().enumerate() {
            let attributes = &corners[cell as usize];
            let vertex = &data.vertices[cell as usize];
            vertices.push(DetailVertex {
                position: vertex.position,
                normal: vertex.normal,
                bare: attributes.bare,
                plants: attributes.plants,
                snow: attributes.snow,
                water: attributes.water,
                coast: attributes.coast,
                plates: [plates[0], plates[1], plates[2], corner as u16],
            });
        }
    }
    vertices
}

/// The attributes of a cell's corners in [`detail_vertices`].
struct Corner {
    bare: [u8; 4],
    plants: [u8; 4],
    snow: [u8; 4],
    water: [u8; 4],
    coast: f32,
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

    #[test]
    fn closed_depressions_are_lakes_or_land() {
        // A dry depression below sea level is land (#128), a lake of at least 1 m is water.
        let dry = CellInfo {
            drainage: CellInfo::drainage_bits(Some(0), false, true, 0.5),
            lake_depth: 0.5,
            ..cell(-150.0, 20.0, 0.6, 1.2)
        };
        let layers_dry = layers(&dry, Period::Annual, false);
        assert!(!layers_dry.water_cell);
        let lake = CellInfo {
            drainage: CellInfo::drainage_bits(Some(0), false, true, 40.0),
            lake_depth: 40.0,
            ..cell(-150.0, 20.0, 0.6, 1.2)
        };
        let layers_lake = layers(&lake, Period::Annual, false);
        assert!(layers_lake.water_cell);
        assert!(close(layers_lake.water, LAKE));
        assert_eq!(layers_lake.relief, 0.0);
        // Below sea level outside depressions: the ocean.
        assert!(layers(&cell(-150.0, 20.0, 0.0, 0.0), Period::Annual, false).water_cell);
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
        // Cold ocean freezes in its winter month only.
        let sea = cell(-3000.0, 2.0, 0.5, 0.0);
        let mut sea = sea;
        sea.temperature_january = -12.0;
        assert!(close(
            material(&sea, Period::January, false).albedo,
            SEA_ICE
        ));
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
                let layers = layers(&cells[cell], Period::Annual, false);
                assert_eq!(vertex.coast < 0.0, layers.water_cell);
                assert!(vertex.coast.abs() >= 0.4 && vertex.coast.abs() <= 1.0);
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
