//! Plate motion: the velocity of the plates as arrows over the topographic colours. A small
//! example of a vector mode; the plate mode of #93 adds plate colours and boundaries.

use super::super::arrows::{ArrowStyle, Sample};
use super::super::fields::{Layer, Layers};
use super::{ArrowScale, Legend, ModeData, RenderMode};

pub struct PlateMotion;

/// Plate speed (cm/yr) drawn with the full arrow length; faster plates are drawn as long.
const REFERENCE_SPEED: f64 = 10.0;

/// Arrow length grows with the square root of the speed, so that slow plates still show their
/// direction next to fast ones.
fn style(arrow_spacing: f64) -> ArrowStyle {
    ArrowStyle {
        exponent: 0.5,
        ..ArrowStyle::for_spacing(arrow_spacing, REFERENCE_SPEED)
    }
}

impl RenderMode for PlateMotion {
    fn label(&self) -> &'static str {
        "Plate motion"
    }

    fn layers(&self) -> Layers {
        Layers::of(&[Layer::PlateVelocity])
    }

    fn arrows(&self, data: &ModeData) -> Option<(Vec<Sample>, ArrowStyle)> {
        let samples = data.vector_samples(Layer::PlateVelocity)?;
        Some((samples, style(data.arrow_spacing)))
    }

    fn legend(&self, arrow_spacing: f64) -> Legend {
        let style = style(arrow_spacing);
        Legend {
            description: "Velocity of the plates over the mantle (the planet's fixed frame), \
                          over the topographic colours. Arrow length grows with the square root \
                          of the speed, up to 10 cm/yr.",
            scale: None,
            swatches: Vec::new(),
            arrows: Some(ArrowScale::new(
                &style,
                &[1.0, 3.0, REFERENCE_SPEED],
                "cm/yr",
            )),
        }
    }
}
