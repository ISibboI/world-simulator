//! Equirectangular renderings of the simulation state.

use crate::math;
use glam::DVec3;
use image::{Rgb, RgbImage};

use crate::planet::Planet;
use crate::tectonics::Simulation;

/// Renders an equirectangular map of `width × width/2` pixels, colouring each pixel by the world
/// cell closest to it.
fn render(simulation: &Simulation, width: u32, color: impl Fn(u32) -> Rgb<u8>) -> RgbImage {
    let height = (width / 2).max(1);
    let mesh = simulation.mesh();
    let mut image = RgbImage::new(width, height);
    let mut row_start = mesh.nearest(DVec3::Z);

    for y in 0..height {
        let latitude = (0.5 - (f64::from(y) + 0.5) / f64::from(height)) * std::f64::consts::PI;
        let mut cell = row_start;
        for x in 0..width {
            let longitude = ((f64::from(x) + 0.5) / f64::from(width) - 0.5) * std::f64::consts::TAU;
            let position = DVec3::new(
                math::cos(latitude) * math::cos(longitude),
                math::cos(latitude) * math::sin(longitude),
                math::sin(latitude),
            );
            cell = mesh.nearest_from(position, cell);
            if x == 0 {
                row_start = cell;
            }
            image.put_pixel(x, y, color(cell));
        }
    }
    image
}

/// Closed depressions with at least this much lake water (m) are drawn as lakes, shallower ones
/// as land (Sebastian's rule on #128).
pub const LAKE_DEPTH: f64 = 1.0;

/// What a cell shows at its surface: the ocean, a lake or land (see [`Water::of`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Water {
    Ocean,
    /// A lake with its depth (m).
    Lake(f64),
    Land,
}

impl Water {
    /// The surface of a cell at `elevation` (m, relative to sea level): in a closed depression
    /// (which the ocean does not reach) a lake if it holds at least [`LAKE_DEPTH`] of water
    /// (`lake_depth`, m), otherwise land, also below sea level (#58, #128); elsewhere ocean below
    /// sea level and land above it.
    ///
    /// The depressions and lakes are those of the last erosion step; the elevation is the one
    /// after its surface changes, which can move coastal cells (deltas, wave erosion) across sea
    /// level. Taking the ocean from the sign of the elevation outside depressions keeps the coast
    /// where the relief has it.
    pub fn of(elevation: f64, depression: bool, lake_depth: f64) -> Self {
        if depression {
            if lake_depth >= LAKE_DEPTH {
                Water::Lake(lake_depth)
            } else {
                Water::Land
            }
        } else if elevation < 0.0 {
            Water::Ocean
        } else {
            Water::Land
        }
    }

    /// The water of cell `cell` of `planet`, as [`Water::of`].
    pub fn of_cell(planet: &Planet, cell: u32) -> Self {
        let water = planet.surface_water();
        let index = cell as usize;
        Water::of(
            planet.surface_elevation(cell),
            water.depression.get(index).copied().unwrap_or(false)
                && !water.ocean.get(index).copied().unwrap_or(false),
            water.lake_depth.get(index).copied().unwrap_or(0.0),
        )
    }

    pub fn is_ocean(self) -> bool {
        self == Water::Ocean
    }

    /// The height (m, relative to sea level) of the surface drawn for a cell at `elevation`: the
    /// sea surface over the ocean, the water level over a lake (the lake floor is the
    /// elevation), the ground on land.
    pub fn surface_height(self, elevation: f64) -> f64 {
        match self {
            Water::Ocean => 0.0,
            Water::Lake(depth) => elevation + depth,
            Water::Land => elevation,
        }
    }
}

/// Elevation map with a hypsometric colour scheme, lakes and rivers (see [`surface_color`];
/// without permanent ice).
pub fn render_elevation(planet: &Planet, width: u32, river_threshold: f64) -> RgbImage {
    let simulation = planet.tectonics();
    render(simulation, width, |cell| {
        surface_color(
            planet.surface_elevation(cell),
            Water::of_cell(planet, cell),
            0.0,
            planet.discharge()[cell as usize],
            river_threshold,
        )
    })
}

/// Colours of rivers, of ice sheets and of sea ice (also permanent lake ice).
pub const RIVER_COLOR: [u8; 3] = [30, 80, 200];
pub const ICE_SHEET_COLOR: [u8; 3] = [240, 245, 250];
pub const SEA_ICE_COLOR: [u8; 3] = [200, 220, 235];
/// Colour of land below sea level (dry closed depressions), darker than the lowlands.
pub const BELOW_SEA_LEVEL_COLOR: [u8; 3] = [45, 105, 60];
/// Colours of lakes by depth (m): turquoise, set apart from the blues of the ocean.
pub const LAKE_STOPS: &[(f64, [f64; 3])] = &[
    (0.0, [110.0, 205.0, 215.0]),
    (100.0, [60.0, 165.0, 190.0]),
    (1000.0, [25.0, 100.0, 140.0]),
];

/// The colour of a lake of `depth` m.
pub fn lake_color(depth: f64) -> Rgb<u8> {
    gradient(LAKE_STOPS, depth)
}

/// The colour of a cell on the planet surface for the 3D view and the elevation map: elevation
/// colours of the ocean and of land, lakes, rivers on land, and permanent ice (sea and lake ice,
/// ice sheets), from its elevation (m), water ([`Water::of`]), permanent ice share and discharge
/// (m³/yr).
pub fn surface_color(
    elevation: f64,
    water: Water,
    ice: f64,
    discharge: f64,
    river_threshold: f64,
) -> Rgb<u8> {
    match water {
        _ if ice > 0.5 => {
            if water == Water::Land {
                Rgb(ICE_SHEET_COLOR)
            } else {
                Rgb(SEA_ICE_COLOR)
            }
        }
        Water::Ocean => elevation_color(elevation.min(-0.001)),
        Water::Lake(depth) => lake_color(depth),
        Water::Land if discharge >= river_threshold => Rgb(RIVER_COLOR),
        Water::Land if elevation < 0.0 => Rgb(BELOW_SEA_LEVEL_COLOR),
        Water::Land => elevation_color(elevation),
    }
}

/// Temperature map of a per-cell field in °C, with the coastlines of the ocean drawn dark.
pub fn render_temperature(planet: &Planet, width: u32, temperature: &[f64]) -> RgbImage {
    const STOPS: &[(f64, [f64; 3])] = &[
        (-50.0, [60.0, 0.0, 90.0]),
        (-30.0, [40.0, 60.0, 190.0]),
        (-10.0, [120.0, 180.0, 240.0]),
        (0.0, [235.0, 245.0, 250.0]),
        (10.0, [250.0, 230.0, 120.0]),
        (20.0, [245.0, 150.0, 50.0]),
        (30.0, [210.0, 40.0, 30.0]),
        (40.0, [110.0, 0.0, 20.0]),
    ];
    let simulation = planet.tectonics();
    let mesh = simulation.mesh();
    let is_land = |cell: u32| !Water::of_cell(planet, cell).is_ocean();
    render(simulation, width, |cell| {
        let coast = is_land(cell)
            && mesh
                .neighbors(cell)
                .iter()
                .any(|&neighbor| !is_land(neighbor));
        let Rgb(color) = gradient(STOPS, temperature[cell as usize]);
        if coast {
            Rgb(color.map(|c| c / 2))
        } else {
            Rgb(color)
        }
    })
}

/// Precipitation map: land (with lakes and closed depressions) coloured from desert to
/// rainforest, ocean in grey.
pub fn render_precipitation(planet: &Planet, width: u32) -> RgbImage {
    const STOPS: &[(f64, [f64; 3])] = &[
        (0.0, [230.0, 200.0, 140.0]),
        (0.25, [220.0, 220.0, 120.0]),
        (0.75, [120.0, 190.0, 80.0]),
        (1.5, [30.0, 130.0, 50.0]),
        (3.0, [20.0, 70.0, 140.0]),
    ];
    let simulation = planet.tectonics();
    render(simulation, width, |cell| {
        if Water::of_cell(planet, cell).is_ocean() {
            let shade =
                (90.0 + 60.0 * (planet.precipitation()[cell as usize] / 3.0).min(1.0)) as u8;
            Rgb([shade, shade, shade.saturating_add(20)])
        } else {
            gradient(STOPS, planet.precipitation()[cell as usize])
        }
    })
}

/// Plate map: each plate has its own colour, continental crust is drawn brighter, and plate
/// boundaries are black.
pub fn render_plates(simulation: &Simulation, width: u32) -> RgbImage {
    let mesh = simulation.mesh();
    render(simulation, width, |cell| {
        let plate = simulation.owner(cell).plate;
        let on_boundary = mesh
            .neighbors(cell)
            .iter()
            .any(|&neighbor| simulation.owner(neighbor).plate != plate);
        if on_boundary {
            return Rgb([0, 0, 0]);
        }
        let [r, g, b] = plate_color(plate);
        let scale = if simulation.surface_crust(cell).is_continental() {
            1.0
        } else {
            0.55
        };
        Rgb([r, g, b].map(|c| (f64::from(c) * scale) as u8))
    })
}

fn plate_color(plate: u32) -> [u8; 3] {
    // Golden-angle hue spacing gives distinct colours for neighbouring indices.
    let hue = (f64::from(plate) * 137.507_764) % 360.0;
    hsv_to_rgb(hue, 0.65, 0.95)
}

fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> [u8; 3] {
    let c = value * saturation;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    [r, g, b].map(|channel| ((channel + m) * 255.0).round() as u8)
}

pub fn elevation_color(elevation: f64) -> Rgb<u8> {
    gradient(ELEVATION_STOPS, elevation)
}

/// Linear interpolation between colour stops, which must be sorted by value.
fn gradient(stops: &[(f64, [f64; 3])], value: f64) -> Rgb<u8> {
    let value = value.clamp(stops[0].0, stops[stops.len() - 1].0);
    let upper = stops
        .iter()
        .position(|&(stop, _)| stop >= value)
        .unwrap_or(stops.len() - 1)
        .max(1);
    let (low, low_color) = stops[upper - 1];
    let (high, high_color) = stops[upper];
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    Rgb([0, 1, 2].map(|i| (low_color[i] + (high_color[i] - low_color[i]) * t).round() as u8))
}

/// The hypsometric colour scale of elevation in metres.
pub const ELEVATION_STOPS: &[(f64, [f64; 3])] = &[
    (-7000.0, [5.0, 15.0, 60.0]),
    (-4000.0, [20.0, 50.0, 130.0]),
    (-200.0, [60.0, 130.0, 200.0]),
    // Below 0 m the ocean, at or above 0 m land (land below sea level, in closed depressions,
    // has a colour of its own, see `surface_color`).
    (-0.001, [120.0, 180.0, 220.0]),
    (0.0, [70.0, 140.0, 70.0]),
    (1000.0, [170.0, 170.0, 90.0]),
    (2500.0, [140.0, 100.0, 60.0]),
    (4500.0, [120.0, 110.0, 105.0]),
    (6000.0, [250.0, 250.0, 250.0]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_depressions_are_lakes_or_land() {
        // A lake needs 1 m of water; a dry depression is land, also below sea level.
        assert_eq!(Water::of(-120.0, true, 35.0), Water::Lake(35.0));
        assert_eq!(Water::of(-120.0, true, 0.9), Water::Land);
        assert_eq!(Water::of(400.0, true, 1.0), Water::Lake(1.0));
        assert_eq!(Water::of(-120.0, false, 0.0), Water::Ocean);
        assert_eq!(Water::of(0.0, false, 0.0), Water::Land);
        assert_eq!(Water::Lake(35.0).surface_height(-120.0), -85.0);
        assert_eq!(Water::Land.surface_height(-120.0), -120.0);
        assert_eq!(Water::Ocean.surface_height(-4000.0), 0.0);
    }

    #[test]
    fn surface_colors_tell_ocean_lakes_and_land_apart() {
        let color = |elevation, water| surface_color(elevation, water, 0.0, 0.0, 1e9);
        let ocean = color(-120.0, Water::Ocean);
        let lake = color(-120.0, Water::Lake(30.0));
        let dry = color(-120.0, Water::Land);
        assert_eq!(ocean, elevation_color(-120.0));
        assert_eq!(lake, lake_color(30.0));
        assert_eq!(dry, Rgb(BELOW_SEA_LEVEL_COLOR));
        assert!(ocean != lake && lake != dry && ocean != dry);
        // Land above sea level keeps the hypsometric colours; rivers only on land.
        assert_eq!(color(500.0, Water::Land), elevation_color(500.0));
        assert_eq!(
            surface_color(500.0, Water::Lake(10.0), 0.0, 1e12, 1e9),
            lake_color(10.0)
        );
        assert_eq!(
            surface_color(500.0, Water::Land, 0.0, 1e12, 1e9),
            Rgb(RIVER_COLOR)
        );
        // Permanent ice: water ice on lakes and the sea, ice sheets on land.
        assert_eq!(
            surface_color(500.0, Water::Lake(10.0), 1.0, 0.0, 1e9),
            Rgb(SEA_ICE_COLOR)
        );
        assert_eq!(
            surface_color(-50.0, Water::Land, 1.0, 0.0, 1e9),
            Rgb(ICE_SHEET_COLOR)
        );
    }
}
