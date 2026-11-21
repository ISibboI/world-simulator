//! The topographic mode, the default: the colours the simulation computes with the vertices
//! (`render::surface_color`: elevation, rivers, permanent ice), unchanged.

use super::{ColorScale, Legend, RenderMode, Swatch};
use crate::render;

pub struct Topographic;

impl RenderMode for Topographic {
    fn label(&self) -> &'static str {
        "Topographic"
    }

    fn legend(&self, _arrow_spacing: f64) -> Legend {
        Legend {
            description: "Elevation above and depth below sea level, with rivers and permanent \
                          ice. The relief is exaggerated.",
            scale: Some(elevation_scale()),
            swatches: vec![
                Swatch {
                    color: render::RIVER_COLOR,
                    label: "River",
                },
                Swatch {
                    color: render::ICE_SHEET_COLOR,
                    label: "Ice sheet",
                },
                Swatch {
                    color: render::SEA_ICE_COLOR,
                    label: "Sea ice",
                },
            ],
            arrows: None,
        }
    }
}

/// The hypsometric colours of `render` as a scale.
pub fn elevation_scale() -> ColorScale {
    ColorScale {
        stops: render::ELEVATION_STOPS
            .iter()
            .map(|&(value, color)| (value, color.map(|channel| channel.round() as u8)))
            .collect(),
        title: "Elevation (km)",
        ticks: vec![
            (-6000.0, "−6"),
            (-3000.0, "−3"),
            (0.0, "0"),
            (3000.0, "3"),
            (6000.0, "6"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_has_the_colours_of_the_maps() {
        let scale = elevation_scale();
        for value in [
            -7500.0, -5000.0, -100.0, -0.0005, 0.0, 1200.0, 4000.0, 7000.0,
        ] {
            let image::Rgb(expected) = render::elevation_color(value);
            assert_eq!(scale.color(value), Some(expected), "{value}");
        }
        let (low, high) = scale.range();
        assert!(
            scale
                .ticks
                .iter()
                .all(|&(tick, _)| tick >= low && tick <= high)
        );
    }
}
