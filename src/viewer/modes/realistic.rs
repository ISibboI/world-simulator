//! The realistic mode (#92): the planet as a camera in space would see it. For now it shows
//! the topographic planet as a placeholder (Sebastian on #148); the realistic surface (colours
//! from the climate and crust, relief true to scale, sub-cell detail, atmosphere) follows in
//! later steps (`notes/issue-92/plan.md`).
//!
//! It is the only mode with a choice of light: the simulation's sun by default, with the sun
//! and, where a camera would see them, the stars behind the planet (see
//! `super::super::lighting` and `super::super::sky`). The data modes are always lit from the
//! camera.

use super::topographic::elevation_scale;
use super::{Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Swatch};
use crate::render;
use crate::viewer::lighting::Light;

pub struct Realistic;

/// Index of the light option.
pub const LIGHT: usize = 0;

const OPTIONS: &[ModeOption] = &[ModeOption {
    label: Light::LABEL,
    hover: Light::HOVER,
    kind: OptionKind::Choice(Light::CHOICES),
}];

impl RenderMode for Realistic {
    fn label(&self) -> &'static str {
        "Realistic"
    }

    fn options(&self) -> &'static [ModeOption] {
        OPTIONS
    }

    fn legend(&self, _options: &OptionValues, _data: Option<&ModeData>) -> Legend {
        Legend {
            description: "The planet in space, lit by the simulation's sun. Stars show where a \
                          camera would see them: on the night side, with the sun hidden or out \
                          of view. The surface is the topographic map for now (relief \
                          exaggerated); realistic colours follow."
                .into(),
            scale: Some(elevation_scale()),
            swatches: vec![
                Swatch::fill(render::RIVER_COLOR, "River"),
                Swatch::fill(render::ICE_SHEET_COLOR, "Ice sheet"),
                Swatch::fill(render::SEA_ICE_COLOR, "Sea ice"),
            ],
            arrows: Vec::new(),
        }
    }
}

/// The light chosen in the options of the realistic mode.
pub fn light(options: &OptionValues) -> Light {
    Light::from_index(options.get(LIGHT) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_lights_by_default() {
        let options = OptionValues::defaults(Realistic.options());
        assert_eq!(light(&options), Light::Sun);
        let mut options = options;
        options.set(LIGHT, 2);
        assert_eq!(light(&options), Light::Camera);
    }
}
