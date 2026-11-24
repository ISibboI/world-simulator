//! Lithosphere strength: where the lithosphere is weak, and why (#101). The strength is the one
//! the simulation uses to choose where plates break and where collision bands deform
//! (`tectonics::weak_zones::lithosphere_strength`), relative to old, undeformed lithosphere: the
//! product of a thermal term (thin, hot lithosphere: young sea floor, arcs, freshly rifted
//! crust, and lithosphere a plume heated, carried with the plate), the thinning of stretched
//! continental lithosphere, and the orogenic weakening of collision belts, sutures and accreted
//! terranes, which heals over ~200 Myr; at least 0.05.
//!
//! The mode shows the strength or one of its terms on a perceptual scale (viridis, reversed:
//! weak is bright), or the weakest term of every weak cell as a category. Optional overlays:
//! the plate boundaries and the deforming bands as in the Plates mode, so that rifts and
//! collisions can be compared with the weak zones, and the outline of the lithosphere weaker
//! than the floor of the deforming-band weights.
//!
//! Everything comes from the cells every frame keeps (the thermal and thinning terms are
//! stored as shares, see `history::codec`), so past frames show alike.

use super::super::overlay::{LineStyle, OverlayBuilder, isolines};
use super::plates::{boundaries, boundary_swatches, deforming_bands, deforming_swatch};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Shading,
    Swatch, SwatchShape, Text, coastlines, rgba,
};
use crate::tectonics::{MIN_BAND_STRENGTH, MIN_STRENGTH};
use crate::viewer::CellInfo;
use crate::viewer::cell_info::Weakness;

pub struct LithosphereStrength;

/// What the mode shows: the value of the option [`SHOW`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Strength,
    Weakest,
    Thermal,
    Thinning,
    Orogenic,
}

impl Shown {
    fn of(options: &OptionValues) -> Self {
        match options.get(SHOW) {
            1 => Shown::Weakest,
            2 => Shown::Thermal,
            3 => Shown::Thinning,
            4 => Shown::Orogenic,
            _ => Shown::Strength,
        }
    }

    /// The value on the colour scale, for all but [`Shown::Weakest`].
    fn value(self, cell: &CellInfo) -> f32 {
        match self {
            Shown::Strength | Shown::Weakest => cell.lithosphere_strength(),
            Shown::Thermal => cell.strength_thermal,
            Shown::Thinning => cell.strength_thinning,
            Shown::Orogenic => cell.strength_orogenic(),
        }
    }
}

/// Indices of the options.
const SHOW: usize = 0;
const BOUNDARIES: usize = 1;
const BANDS: usize = 2;
const OUTLINE: usize = 3;

const OPTIONS: [ModeOption; 4] = [
    ModeOption {
        label: "Show",
        hover: "The strength; the weakest of its terms (what weakens the lithosphere most); or \
                one term: thermal (young or plume-heated lithosphere), thinning (stretched \
                continental crust), orogenic (unhealed collision belts and sutures)",
        kind: OptionKind::Choice(&["Strength", "Weakest", "Thermal", "Thinning", "Orogenic"]),
    },
    ModeOption {
        label: "Plate boundaries",
        hover: "The plate boundaries by what they do, as in the Plates mode",
        kind: OptionKind::Switch { default: true },
    },
    ModeOption {
        label: "Deforming bands",
        hover: "Dots on crust deforming behind a collision or beside a rift",
        kind: OptionKind::Switch { default: false },
    },
    ModeOption {
        label: "Weak-zone outline",
        hover: "A line around lithosphere weaker than 0.5, the strength below which collision \
                bands no longer get any weaker in the model",
        kind: OptionKind::Switch { default: false },
    },
];

/// Viridis (perceptually uniform, colour-blind safe), reversed: weak lithosphere is bright.
const STOPS: [(f64, [u8; 3]); 9] = [
    (0.0, [253, 231, 37]),
    (0.125, [170, 220, 50]),
    (0.25, [92, 200, 99]),
    (0.375, [39, 173, 129]),
    (0.5, [33, 144, 141]),
    (0.625, [44, 113, 142]),
    (0.75, [59, 81, 139]),
    (0.875, [71, 44, 122]),
    (1.0, [68, 1, 84]),
];

/// Colours of the weakest terms (Okabe–Ito, colour-blind safe), and of lithosphere that is not
/// weak enough to show one.
fn weakness_color(weakness: Weakness) -> [u8; 3] {
    match weakness {
        Weakness::Young => [0, 114, 178],
        Weakness::Plume => [230, 159, 0],
        Weakness::Thinned => [0, 158, 115],
        Weakness::Orogenic => [204, 121, 167],
    }
}

/// The weakest term is shown, in full colour, only where the strength is below this; stronger
/// lithosphere is [`STRONG_COLOR`] (pale where it is weakened at all).
const WEAKEST_SHOWN_BELOW: f32 = 0.75;
const STRONG_COLOR: [u8; 3] = [95, 95, 100];
const SOMEWHAT_WEAK_COLOR: [u8; 3] = [170, 170, 175];

/// Colour of cells without a value.
const MISSING: [u8; 3] = [128, 128, 128];

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
/// Coastlines: pale grey on the colour scale (white is for inactive boundaries), dark on the
/// greys of the weakest-term view.
const COAST_COLOR: [u8; 3] = [185, 185, 185];
const COAST_COLOR_ON_GREY: [u8; 3] = [25, 25, 25];

fn coast_color(shown: Shown) -> [u8; 3] {
    if shown == Shown::Weakest {
        COAST_COLOR_ON_GREY
    } else {
        COAST_COLOR
    }
}
/// Boundary lines narrower than in the Plates mode, so that the colours beside them show.
const BOUNDARY_SCALE: f64 = 0.6;
const OUTLINE_WIDTH: f64 = 0.03;
/// Black with a white border: white lines are inactive boundaries.
const OUTLINE_COLOR: [u8; 3] = [0, 0, 0];
const OUTLINE_BORDER: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Lift of the lines above the surface (planet radii).
const LIFT: f64 = 0.003;

fn scale(shown: Shown) -> ColorScale {
    ColorScale {
        stops: STOPS.to_vec(),
        title: match shown {
            Shown::Strength | Shown::Weakest => "Lithosphere strength (relative to old)",
            Shown::Thermal => "Thermal term (√ of thermal age / 100 Myr)",
            Shown::Thinning => "Thinning term (crystalline crust without metasediment / 35 km)",
            Shown::Orogenic => "Orogenic term",
        }
        .into(),
        ticks: [0.0, 0.25, 0.5, 0.75, 1.0]
            .into_iter()
            .map(|value| (value, format!("{value}").into()))
            .collect(),
    }
}

/// The colour of a cell in the [`Shown::Weakest`] view.
fn weakest_color(cell: &CellInfo) -> [u8; 3] {
    let strength = cell.lithosphere_strength();
    if strength.is_nan() {
        return MISSING;
    }
    match cell.weakness() {
        Some(weakness) if strength < WEAKEST_SHOWN_BELOW => weakness_color(weakness),
        Some(_) => SOMEWHAT_WEAK_COLOR,
        None => STRONG_COLOR,
    }
}

impl RenderMode for LithosphereStrength {
    fn label(&self) -> &'static str {
        "Lithosphere strength"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let shown = Shown::of(data.options);
        Some(match shown {
            Shown::Weakest => data
                .cells
                .iter()
                .map(|cell| rgba(weakest_color(cell)))
                .collect(),
            _ => scale(shown).colors(data.cells.iter().map(|cell| shown.value(cell)), MISSING),
        })
    }

    fn shading(&self) -> Shading {
        Shading::Unlit
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let spacing = data.arrow_spacing;
        coastlines(
            data,
            overlay,
            COAST_WIDTH * spacing,
            rgba(coast_color(Shown::of(data.options))),
            LIFT,
        );
        overlay.next_layer();
        if data.options.is_on(OUTLINE) {
            let style = LineStyle {
                width: OUTLINE_WIDTH * spacing,
                border: 0.3 * OUTLINE_WIDTH * spacing,
                color: rgba(OUTLINE_COLOR),
                border_color: OUTLINE_BORDER,
                lift: LIFT,
            };
            for [from, to] in isolines(
                data.mesh.faces(),
                |cell| data.cells[cell as usize].lithosphere_strength(),
                MIN_BAND_STRENGTH as f32,
                |cell| data.position(cell).as_dvec3(),
            ) {
                overlay.line(from, to, &style);
            }
            overlay.next_layer();
        }
        if data.options.is_on(BANDS) {
            deforming_bands(data, overlay);
            overlay.next_layer();
        }
        if data.options.is_on(BOUNDARIES) {
            boundaries(data, overlay, BOUNDARY_SCALE);
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let shown = Shown::of(options);
        let mut swatches = Vec::new();
        if data.is_some_and(|data| data.cells.iter().any(|cell| shown.value(cell).is_nan())) {
            swatches.push(Swatch::fill(MISSING, "No data"));
        }
        if shown == Shown::Weakest {
            // One row each (a one-colour palette), so that no colour sits next to the label of
            // another.
            let row = |color: [u8; 3], label: Text| {
                Swatch::new(color, SwatchShape::Palette(vec![color]), label)
            };
            swatches.extend(Weakness::ALL.into_iter().map(|weakness| {
                row(
                    weakness_color(weakness),
                    match weakness {
                        Weakness::Young => "Young lithosphere (sea floor, arcs, rifts)",
                        Weakness::Plume => "Plume-heated",
                        Weakness::Thinned => "Thinned continental crust",
                        Weakness::Orogenic => "Orogenic belt, suture",
                    }
                    .into(),
                )
            }));
            swatches.push(row(
                SOMEWHAT_WEAK_COLOR,
                format!(
                    "Strength {WEAKEST_SHOWN_BELOW}–{}",
                    crate::viewer::cell_info::FULL_STRENGTH
                )
                .into(),
            ));
            swatches.push(row(STRONG_COLOR, "Full strength".into()));
        }
        swatches.push(Swatch::new(
            coast_color(shown),
            SwatchShape::Line,
            "Coastline",
        ));
        if options.is_on(OUTLINE) {
            swatches.push(Swatch::new(
                OUTLINE_COLOR,
                SwatchShape::Line,
                format!("Strength {MIN_BAND_STRENGTH}"),
            ));
        }
        if options.is_on(BOUNDARIES) {
            swatches.extend(boundary_swatches());
        }
        if options.is_on(BANDS) {
            swatches.push(deforming_swatch());
        }
        let description = match shown {
            Shown::Strength => format!(
                "Strength of the lithosphere relative to old, undeformed lithosphere (at least \
                 {MIN_STRENGTH}): where plates break and collisions deform. Young or \
                 plume-heated lithosphere, stretched continental crust and unhealed orogenic \
                 belts and sutures are weak; old ocean and cratons are strong."
            ),
            Shown::Weakest => format!(
                "What weakens the lithosphere most, where its strength is below \
                 {WEAKEST_SHOWN_BELOW}; the strength is the product of the terms."
            ),
            Shown::Thermal => "The thermal term: thin, hot lithosphere is weak. It grows with \
                               the square root of the thermal age, the time since the \
                               lithosphere formed at a ridge, was rifted, or a plume last \
                               heated it, and is full after 100 Myr."
                .to_owned(),
            Shown::Thinning => "The thinning term: stretched continental lithosphere is \
                                thinned with its crust, in proportion to the crust's thickness \
                                below 35 km (1 for oceanic lithosphere)."
                .to_owned(),
            Shown::Orogenic => "The orogenic term: collision belts, sutures and accreted \
                                terranes are weak (0.25 when fresh) and heal as they cool, \
                                over ~200 Myr."
                .to_owned(),
        };
        Legend {
            description: description.into(),
            scale: (shown != Shown::Weakest).then(|| scale(shown)),
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(thermal: f32, thinning: f32, orogenic: f32, age: f32) -> CellInfo {
        CellInfo {
            strength_thermal: thermal,
            strength_thinning: thinning,
            orogenic,
            age,
            ..bytemuck::Zeroable::zeroed()
        }
    }

    #[test]
    fn weak_is_bright_and_the_scale_is_monotonic() {
        let scale = scale(Shown::Strength);
        let lightness = |value: f64| {
            let [r, g, b] = scale.color(value).unwrap().map(f64::from);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        let mut last = f64::INFINITY;
        for step in 0..=20 {
            let value = f64::from(step) / 20.0;
            let now = lightness(value);
            assert!(now < last, "{value}: {now} ≥ {last}");
            last = now;
        }
    }

    #[test]
    fn the_strength_combines_the_terms_as_the_simulation_does() {
        let old = cell(1.0, 1.0, 0.0, 1000.0);
        assert_eq!(old.lithosphere_strength(), 1.0);
        assert_eq!(old.weakness(), None);
        // A fresh suture: 0.25 of old.
        let belt = cell(1.0, 1.0, 1.0, 1000.0);
        assert!((belt.lithosphere_strength() - 0.25).abs() < 1e-6);
        assert_eq!(belt.weakness(), Some(Weakness::Orogenic));
        // The floor of the ridge axis.
        let ridge = cell(0.0, 1.0, 0.0, 0.0);
        assert_eq!(ridge.lithosphere_strength(), MIN_STRENGTH as f32);
        assert_eq!(ridge.weakness(), Some(Weakness::Young));
        let rifted = cell(0.9, 0.5, 0.2, 10.0);
        assert_eq!(rifted.weakness(), Some(Weakness::Thinned));
        assert!(
            cell(f32::NAN, 1.0, 0.0, 1.0)
                .lithosphere_strength()
                .is_nan()
        );
        assert_eq!(cell(f32::NAN, 1.0, 0.0, 1.0).weakness(), None);
    }

    #[test]
    fn plume_heating_is_told_from_young_lithosphere_after_quantisation() {
        let terms = |age: f64| crate::tectonics::thermal_strength(age) as f32;
        // The term of the true age, the age as the history rounds it (0.25 Myr) and the term
        // as it rounds it (1/254): young lithosphere stays young, also next to the ridge axis.
        for step in 0..=400 {
            let age = f64::from(step) * 0.0625;
            let stored_age = ((age / 0.25).round() * 0.25) as f32;
            let stored_term = (terms(age) * 254.0).round() / 254.0;
            let young = cell(stored_term, 1.0, 0.0, stored_age);
            assert_eq!(young.weakness(), Some(Weakness::Young), "age {age}");
        }
        // Old ocean and a continent, heated to a thermal age of 25 Myr.
        for age in [150.0, 1000.0] {
            let heated = cell(terms(25.0), 1.0, 0.0, age);
            assert_eq!(heated.weakness(), Some(Weakness::Plume), "age {age}");
        }
        // Young sea floor heated a little more than its age.
        assert_eq!(
            cell(terms(10.0), 1.0, 0.0, 20.0).weakness(),
            Some(Weakness::Plume)
        );
    }

    #[test]
    fn the_weakest_view_shows_categories_only_for_weak_cells() {
        assert_eq!(
            weakest_color(&cell(1.0, 1.0, 1.0, 1000.0)),
            weakness_color(Weakness::Orogenic)
        );
        // Strength 0.85: weakened, but not a weak zone.
        assert_eq!(
            weakest_color(&cell(1.0, 0.85, 0.0, 1000.0)),
            SOMEWHAT_WEAK_COLOR
        );
        assert_eq!(weakest_color(&cell(1.0, 1.0, 0.0, 1000.0)), STRONG_COLOR);
    }
}
