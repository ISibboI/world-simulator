//! Plate motion: the velocity of the plates as arrows over the topographic colours, each arrow
//! in the colour of its plate in the plate mode (see [`super::plates::plate_color`]), so that
//! the plates can be told apart by their arrows (#108).

use std::collections::BTreeMap;

use super::super::arrows::{ArrowStyle, Sample};
use super::super::fields::{Layer, Layers};
use super::super::overlay::OverlayBuilder;
use super::super::plate_colors::PALETTE;
use super::plates::plate_color;
use super::{
    ArrowScale, DEFAULT_ARROW_SPACING, Legend, ModeData, OptionValues, RenderMode, Swatch,
    SwatchShape, rgba,
};

pub struct PlateMotion;

/// Plate speed (cm/yr) drawn with the full arrow length; faster plates are drawn as long.
pub(super) const REFERENCE_SPEED: f64 = 10.0;

/// The colour of the example arrows in the legend: neutral, as the arrows take their plates'
/// colours.
const LEGEND_ARROW: [f32; 4] = [0.85, 0.85, 0.85, 1.0];

/// Arrow length grows with the square root of the speed, so that slow plates still show their
/// direction next to fast ones.
pub(super) fn style(arrow_spacing: f64) -> ArrowStyle {
    ArrowStyle {
        exponent: 0.5,
        ..ArrowStyle::for_spacing(arrow_spacing, REFERENCE_SPEED)
    }
}

/// The plate velocity at [`ModeData::arrow_cells`] with the plate colour of each cell, or
/// `None` while the layer has not arrived.
fn samples(data: &ModeData) -> Option<Vec<([u8; 3], Sample)>> {
    let samples = data.vector_samples(Layer::PlateVelocity)?;
    Some(
        samples
            .into_iter()
            .filter_map(|(cell, sample)| {
                let plate = data.cells.get(cell as usize)?.plate;
                Some((plate_color(data, plate), sample))
            })
            .collect(),
    )
}

impl RenderMode for PlateMotion {
    fn label(&self) -> &'static str {
        "Plate motion"
    }

    fn layers(&self) -> Layers {
        Layers::of(&[Layer::PlateVelocity])
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        let Some(samples) = samples(data) else {
            return;
        };
        // One batch per colour (the borders of all arrows lie under all fills anyway).
        let mut by_color: BTreeMap<[u8; 3], Vec<Sample>> = BTreeMap::new();
        for (color, sample) in samples {
            by_color.entry(color).or_default().push(sample);
        }
        let style = style(data.arrow_spacing);
        for (color, samples) in by_color {
            let style = ArrowStyle {
                color: rgba(color),
                ..style
            };
            overlay.arrows(&samples, &style);
        }
    }

    fn legend(&self, _options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let style = style(data.map_or(DEFAULT_ARROW_SPACING, |data| data.arrow_spacing));
        Legend {
            description: "Velocity of the lithosphere over the mantle (the planet's fixed \
                          frame): the plates' motion plus their internal deformation (past frames \
                          of the history: the plates' motion only), over the topographic colours. \
                          Each arrow has the colour of its plate in the Plates mode. Arrow length \
                          grows with the square root of the speed, up to 10 cm/yr."
                .into(),
            scale: None,
            swatches: vec![Swatch::new(
                PALETTE[0],
                SwatchShape::Palette(PALETTE[..8].to_vec()),
                "Plates",
            )],
            arrows: vec![ArrowScale {
                color: LEGEND_ARROW,
                ..ArrowScale::new(&style, &[1.0, 3.0, REFERENCE_SPEED], "cm/yr")
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_have_the_colours_of_their_plates_in_the_plates_mode() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, plate_infos, surface_vertices};
        use super::super::plates::Plates;
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let mut planet = crate::build_planet(&cli).unwrap();
        planet.step(cli.dt);
        let cells = cell_infos(&planet);
        let plates = plate_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let mut plate_colors = PlateColors::default();
        plate_colors.update(mesh, &cells, &plates);
        let fields = Fields::from_frame(mesh, &cells, &plates);
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 2);
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &plates,
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &arrow_cells,
            arrow_spacing,
            options: &OptionValues::default(),
            abiotic: false,
        };
        // Every arrow has the colour its cell has in the plates mode (there undarkened on
        // continental crust).
        let plate_mode = Plates.colors(&data).unwrap();
        let samples = samples(&data).unwrap();
        assert_eq!(samples.len(), arrow_cells.len());
        for (&cell, (color, _)) in arrow_cells.iter().zip(&samples) {
            let info = &cells[cell as usize];
            assert_eq!(*color, plate_colors.color(plates[info.plate as usize].id));
            if info.is_continental() {
                assert_eq!(rgba(*color), plate_mode[cell as usize]);
            }
        }
        // Several plates, so several colours, and the fills of the arrows are drawn in them.
        let colors: Vec<[u8; 3]> = samples.iter().map(|(color, _)| *color).collect();
        assert!(colors.iter().any(|color| *color != colors[0]));
        let mut overlay = OverlayBuilder::default();
        PlateMotion.overlay(&data, &mut overlay);
        let vertices = overlay.finish();
        assert!(!vertices.is_empty());
        for color in &colors {
            assert!(vertices.iter().any(|vertex| vertex.color == rgba(*color)));
        }
        // No arrow keeps the default colour.
        assert!(
            !vertices
                .iter()
                .any(|vertex| vertex.color == style(arrow_spacing).color)
        );
        let legend = PlateMotion.legend(&OptionValues::default(), Some(&data));
        assert_eq!(legend.arrows[0].color, LEGEND_ARROW);
    }
}
