//! Equirectangular renderings of the simulation state.

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
                latitude.cos() * longitude.cos(),
                latitude.cos() * longitude.sin(),
                latitude.sin(),
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

/// Elevation map with a hypsometric colour scheme. Land cells whose discharge is at least
/// `river_threshold` m³/yr are drawn as rivers.
pub fn render_elevation(planet: &Planet, width: u32, river_threshold: f64) -> RgbImage {
    let simulation = planet.tectonics();
    render(simulation, width, |cell| {
        let elevation = simulation.surface_elevation(cell);
        if elevation >= 0.0 && planet.discharge()[cell as usize] >= river_threshold {
            Rgb([30, 80, 200])
        } else {
            elevation_color(elevation)
        }
    })
}

/// The colour of a cell on the planet surface for the 3D view: elevation colours, rivers, and
/// permanent ice (sea ice and ice sheets).
pub fn surface_color(planet: &Planet, cell: u32, river_threshold: f64) -> Rgb<u8> {
    let simulation = planet.tectonics();
    let elevation = simulation.surface_elevation(cell);
    let frozen = planet
        .temperature()
        .ice
        .get(cell as usize)
        .is_some_and(|&ice| ice > 0.5);
    if frozen {
        if elevation >= 0.0 {
            Rgb([240, 245, 250])
        } else {
            Rgb([200, 220, 235])
        }
    } else if elevation >= 0.0 && planet.discharge()[cell as usize] >= river_threshold {
        Rgb([30, 80, 200])
    } else {
        elevation_color(elevation)
    }
}

/// Temperature map of a per-cell field in °C, with coastlines drawn dark.
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
    let is_land = |cell: u32| simulation.surface_elevation(cell) >= 0.0;
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

/// Precipitation map: land coloured from desert to rainforest, ocean in grey.
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
        if simulation.surface_elevation(cell) < 0.0 {
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

fn elevation_color(elevation: f64) -> Rgb<u8> {
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

const ELEVATION_STOPS: &[(f64, [f64; 3])] = &[
    (-7000.0, [5.0, 15.0, 60.0]),
    (-4000.0, [20.0, 50.0, 130.0]),
    (-200.0, [60.0, 130.0, 200.0]),
    // Everything below 0 m is water, everything at or above 0 m is land, as in the simulation.
    (-0.001, [120.0, 180.0, 220.0]),
    (0.0, [70.0, 140.0, 70.0]),
    (1000.0, [170.0, 170.0, 90.0]),
    (2500.0, [140.0, 100.0, 60.0]),
    (4500.0, [120.0, 110.0, 105.0]),
    (6000.0, [250.0, 250.0, 250.0]),
];
