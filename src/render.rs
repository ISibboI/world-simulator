//! Equirectangular renderings of the simulation state.

use glam::DVec3;
use image::{Rgb, RgbImage};

use crate::tectonics::{CrustKind, Simulation};

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

/// Elevation map with a hypsometric colour scheme.
pub fn render_elevation(simulation: &Simulation, width: u32) -> RgbImage {
    render(simulation, width, |cell| {
        elevation_color(simulation.surface_crust(cell).elevation)
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
        let scale = match simulation.surface_crust(cell).kind {
            CrustKind::Continental => 1.0,
            CrustKind::Oceanic => 0.55,
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
    const STOPS: &[(f64, [f64; 3])] = &[
        (-7000.0, [5.0, 15.0, 60.0]),
        (-4000.0, [20.0, 50.0, 130.0]),
        (-200.0, [60.0, 130.0, 200.0]),
        (0.0, [120.0, 180.0, 220.0]),
        (0.1, [70.0, 140.0, 70.0]),
        (1000.0, [170.0, 170.0, 90.0]),
        (2500.0, [140.0, 100.0, 60.0]),
        (4500.0, [120.0, 110.0, 105.0]),
        (6000.0, [250.0, 250.0, 250.0]),
    ];

    let elevation = elevation.clamp(STOPS[0].0, STOPS[STOPS.len() - 1].0);
    let upper = STOPS
        .iter()
        .position(|&(stop, _)| stop >= elevation)
        .unwrap_or(STOPS.len() - 1)
        .max(1);
    let (low, low_color) = STOPS[upper - 1];
    let (high, high_color) = STOPS[upper];
    let t = ((elevation - low) / (high - low)).clamp(0.0, 1.0);
    Rgb([0, 1, 2].map(|i| (low_color[i] + (high_color[i] - low_color[i]) * t).round() as u8))
}
