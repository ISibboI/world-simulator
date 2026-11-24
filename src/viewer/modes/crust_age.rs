//! Crust age: the age of the ocean floor as colours with isochrons, the continents in grey
//! (#98).
//!
//! The ocean floor is coloured by the age of its lithosphere (`CellInfo::age`) on Crameri's
//! *batlow* scale, reversed: young crust at the ridges light pink and orange, old crust dark
//! blue. Sea-floor age maps traditionally use a rainbow from young = red to old = blue; batlow
//! keeps that look but is perceptually uniform, its lightness rises monotonically (so it also
//! reads in greyscale) and it stays readable with colour-vision deficiencies (Crameri, Shephard
//! & Heron 2020, "The misuse of colour in science communication", which uses exactly the
//! sea-floor age map as its example). Isochrons, lines of equal age, show the spreading history:
//! closely spaced where spreading was slow, asymmetric where a ridge spread unevenly or jumped.
//!
//! Only normal oceanic crust is a sea floor with an age from a ridge. Continental crust is
//! grey: one neutral grey, or classes of grey by the time since its last orogeny (lighter is
//! more recent). The simulation does not keep a formation age of continental crust (its
//! `birth` is a thermal age: nominally 1000 Myr before the start, reset where a rift thins it),
//! so there is no option for that. Transitional crust (thick arcs, thinned continental margins)
//! has a colour of its own. Everything comes from the cells every frame keeps (ages at 0.25
//! Myr, see `history::codec`), so past frames show alike.

use super::super::overlay::{OverlayBuilder, isolines};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Shading,
    Swatch, SwatchShape, coastlines, rgba,
};
use crate::viewer::CellInfo;
use crate::viewer::overlay::LineStyle;

pub struct CrustAge;

/// Indices of the options.
const CONTINENTS: usize = 0;
const ISOCHRONS: usize = 1;
const COASTLINES: usize = 2;

const OPTIONS: [ModeOption; 3] = [
    ModeOption {
        label: "Continents",
        hover: "Continental crust in one grey, or in shades of grey by the time since its \
                last orogeny (lighter is more recent)",
        kind: OptionKind::Choice(&["Grey", "Last orogeny"]),
    },
    ModeOption {
        label: "Isochrons",
        hover: "Lines of equal sea-floor age",
        kind: OptionKind::Choice(&["20 Myr", "50 Myr", "Off"]),
    },
    ModeOption {
        label: "Coastlines",
        hover: "The coastlines; the edge of the grey is the edge of the continental crust, \
                which lies offshore where shelves are flooded",
        kind: OptionKind::Switch { default: true },
    },
];

/// How continental crust is shown: the value of the option [`CONTINENTS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Continents {
    Grey,
    Orogeny,
}

impl Continents {
    fn of(options: &OptionValues) -> Self {
        match options.get(CONTINENTS) {
            1 => Continents::Orogeny,
            _ => Continents::Grey,
        }
    }
}

/// The spacing of the isochrons in Myr, if they are shown.
fn isochron_interval(options: &OptionValues) -> Option<f32> {
    match options.get(ISOCHRONS) {
        0 => Some(20.0),
        1 => Some(50.0),
        _ => None,
    }
}

/// Lithosphere at least this old (Myr) is older than the simulation: the model gives it a
/// nominal birth 1000 Myr before the start (see `cell_info::OLD_LITHOSPHERE`), or sets it to
/// 1000 Myr where it is fully cooled; a little below, so that rounding (f32, the 0.25 Myr of
/// the frame history) does not matter.
const BEFORE_START: f32 = 999.5;

/// The oldest age on the scale (Myr); older ocean floor gets the colour of this age.
const SCALE_MAX: f64 = 200.0;

/// Crameri's batlow (v8, 256 entries) at every 20 Myr, reversed: entries 255, 230, 204, …, 0.
const STOPS: [[u8; 3]; 11] = [
    [250, 204, 250],
    [253, 185, 194],
    [251, 166, 137],
    [224, 150, 81],
    [179, 142, 47],
    [130, 130, 49],
    [87, 118, 71],
    [48, 104, 92],
    [24, 85, 98],
    [16, 61, 95],
    [1, 25, 89],
];

/// Oceanic crust with a nominal age (see [`BEFORE_START`]): a dark red, which the scale and
/// the greys do not have.
const OLD_OCEAN: [u8; 3] = [150, 40, 40];

/// Cells without a known age.
const MISSING: [u8; 3] = [255, 0, 255];

/// Continental crust in the "Grey" option.
const GREY: [u8; 3] = [160, 160, 160];

/// Transitional crust, neither oceanic nor continental: a greyish blue, unlike both the greys
/// and the scale (whose blues are much darker).
const TRANSITIONAL: [u8; 3] = [140, 160, 200];

/// Classes of grey: upper bounds (Myr, exclusive) with their colours and labels; the last
/// class takes the rest (earlier, or no orogeny). The greys stay in the middle of the
/// lightness range, away from the dark blue of old sea floor.
type Classes = [(f32, [u8; 3], &'static str); 4];

const OROGENY_CLASSES: Classes = [
    (50.0, [215, 215, 215], "Orogeny < 50 Myr ago"),
    (150.0, [185, 185, 185], "50–150 Myr"),
    (400.0, [150, 150, 150], "150–400 Myr"),
    (f32::INFINITY, [115, 115, 115], "None or older"),
];

/// The class colour of a value (NaN, e.g. no orogeny, falls into the last class).
fn class_color(classes: &Classes, value: f32) -> [u8; 3] {
    classes
        .iter()
        .find(|(bound, _, _)| value < *bound)
        .unwrap_or(&classes[classes.len() - 1])
        .1
}

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [25, 25, 25];
/// Isochrons: thin dark lines with a white border, which show on the light young floor and on
/// the dark old floor alike; thinner than the coastlines, which have no border.
const ISOCHRON_WIDTH: f64 = 0.015;
const ISOCHRON_COLOR: [u8; 3] = [45, 45, 45];
const ISOCHRON_BORDER: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Lift of the lines above the surface (planet radii).
const LIFT: f64 = 0.003;

/// Faces whose ages differ by more than this (Myr) are left out of the isochrons: an age jump
/// (a trench, a transform fault, a suture inside a plate) rather than a gradient. Spreading at
/// 1 cm/yr ages the floor by about 11 Myr per cell, so gradients stay well below it.
const MAX_FACE_SPAN: f32 = 50.0;

/// The colour scale of the ocean floor.
fn scale() -> ColorScale {
    ColorScale {
        stops: STOPS
            .iter()
            .enumerate()
            .map(|(index, &color)| (SCALE_MAX * index as f64 / 10.0, color))
            .collect(),
        title: "Sea-floor age (Myr)".into(),
        ticks: (0..=5)
            .map(|step| {
                let value = 40 * step;
                let label = if value == 200 {
                    "≥ 200".to_owned()
                } else {
                    value.to_string()
                };
                (f64::from(value), label.into())
            })
            .collect(),
    }
}

/// The sea-floor age of a cell: its age if its crust is oceanic and its age is not the nominal
/// one (see [`BEFORE_START`]), else NaN. Ocean floor of the initial planet has real ages (up to
/// a few hundred Myr) and counts.
fn sea_floor_age(cell: &CellInfo) -> f32 {
    if cell.is_oceanic() && cell.age < BEFORE_START {
        cell.age
    } else {
        f32::NAN
    }
}

/// The colour of a cell.
fn color(scale: &ColorScale, continents: Continents, cell: &CellInfo) -> [u8; 3] {
    if cell.is_continental() {
        match continents {
            Continents::Grey => GREY,
            Continents::Orogeny => class_color(&OROGENY_CLASSES, cell.since_orogeny),
        }
    } else if !cell.is_oceanic() {
        TRANSITIONAL
    } else if cell.age >= BEFORE_START {
        OLD_OCEAN
    } else {
        scale.color(f64::from(cell.age)).unwrap_or(MISSING)
    }
}

/// The faces for the isochrons: all three cells ocean floor of one plate, with ages that do
/// not jump (see [`MAX_FACE_SPAN`]).
fn isochron_faces(faces: &[[u32; 3]], cells: &[CellInfo]) -> Vec<[u32; 3]> {
    faces
        .iter()
        .copied()
        .filter(|face| {
            let [a, b, c] = face.map(|cell| &cells[cell as usize]);
            let ages = [a, b, c].map(sea_floor_age);
            if ages.iter().any(|age| age.is_nan()) || a.plate != b.plate || a.plate != c.plate {
                return false;
            }
            let (low, high) = ages
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), &age| {
                    (low.min(age), high.max(age))
                });
            high - low <= MAX_FACE_SPAN
        })
        .collect()
}

/// Statistics of the ocean floor for the legend: its share of the cells, mean and maximum
/// age (cells are of nearly equal area, so cell counts stand for areas).
fn sea_floor_statistics(cells: &[CellInfo]) -> Option<(f64, f64, f64)> {
    let (mut count, mut sum, mut max) = (0usize, 0.0, 0.0f64);
    for age in cells.iter().map(sea_floor_age).filter(|age| !age.is_nan()) {
        count += 1;
        sum += f64::from(age);
        max = max.max(f64::from(age));
    }
    (count > 0).then_some((count as f64 / cells.len() as f64, sum / count as f64, max))
}

impl RenderMode for CrustAge {
    fn label(&self) -> &'static str {
        "Crust age"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let scale = scale();
        let continents = Continents::of(data.options);
        Some(
            data.cells
                .iter()
                .map(|cell| rgba(color(&scale, continents, cell)))
                .collect(),
        )
    }

    fn shading(&self) -> Shading {
        Shading::Unlit
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let spacing = data.arrow_spacing;
        if data.options.is_on(COASTLINES) {
            coastlines(
                data,
                overlay,
                COAST_WIDTH * spacing,
                rgba(COAST_COLOR),
                LIFT,
            );
            overlay.next_layer();
        }
        let Some(interval) = isochron_interval(data.options) else {
            return;
        };
        let faces = isochron_faces(data.mesh.faces(), data.cells);
        let oldest = data
            .cells
            .iter()
            .map(sea_floor_age)
            .filter(|age| !age.is_nan())
            .fold(0.0f32, f32::max);
        let style = LineStyle {
            width: ISOCHRON_WIDTH * spacing,
            border: 0.4 * ISOCHRON_WIDTH * spacing,
            color: rgba(ISOCHRON_COLOR),
            border_color: ISOCHRON_BORDER,
            lift: LIFT,
        };
        let position = |cell: u32| data.position(cell).as_dvec3();
        let mut level = interval;
        while level <= oldest {
            for [from, to] in isolines(
                &faces,
                |cell| sea_floor_age(&data.cells[cell as usize]),
                level,
                position,
            ) {
                overlay.line(from, to, &style);
            }
            level += interval;
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let continents = Continents::of(options);
        let mut swatches = match continents {
            Continents::Grey => vec![Swatch::fill(GREY, "Continental crust")],
            Continents::Orogeny => OROGENY_CLASSES
                .iter()
                .map(|&(_, color, label)| Swatch::fill(color, label))
                .collect(),
        };
        let cells = data.map_or(&[][..], |data| data.cells);
        if cells
            .iter()
            .any(|cell| !cell.is_continental() && !cell.is_oceanic())
        {
            swatches.push(Swatch::fill(TRANSITIONAL, "Transitional crust"));
        }
        if cells
            .iter()
            .any(|cell| cell.is_oceanic() && cell.age >= BEFORE_START)
        {
            swatches.push(Swatch::fill(OLD_OCEAN, "Oceanic, nominal age ≥ 1000 Myr"));
        }
        if cells
            .iter()
            .any(|cell| cell.is_oceanic() && cell.age.is_nan())
        {
            swatches.push(Swatch::fill(MISSING, "No data"));
        }
        if options.is_on(COASTLINES) {
            swatches.push(Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline"));
        }
        if let Some(interval) = isochron_interval(options) {
            swatches.push(Swatch::new(
                ISOCHRON_COLOR,
                SwatchShape::Line,
                format!("Isochron every {interval:.0} Myr"),
            ));
        }
        let statistics =
            sea_floor_statistics(cells).map_or_else(String::new, |(share, mean, max)| {
                format!(
                    " Here the ocean floor covers {:.0}% of the surface, mean age {mean:.0} Myr, \
                 oldest {max:.0} Myr.",
                    100.0 * share
                )
            });
        let continents_text = match continents {
            Continents::Grey => "Continental crust is grey.",
            Continents::Orogeny => {
                "Continental crust in grey by the time since its last orogeny (collision, \
                 suture, accretion)."
            }
        };
        Legend {
            description: format!(
                "Age of the oceanic crust since it formed at a ridge: young and light at the \
                 ridges, older and darker towards trenches and passive margins. \
                 {continents_text} Transitional crust (thick arcs, thinned margins) has its own \
                 colour.{statistics}"
            )
            .into(),
            scale: Some(scale()),
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kinds of crust for test cells.
    #[derive(Clone, Copy)]
    enum Kind {
        Oceanic,
        Transitional,
        Continental,
    }

    fn cell(age: f32, kind: Kind, plate: u32) -> CellInfo {
        CellInfo {
            elevation: -3000.0,
            crust_thickness: 7000.0,
            age,
            sediment: 0.0,
            orogenic: 0.0,
            since_orogeny: f32::NAN,
            strength_thermal: 1.0,
            strength_thinning: 1.0,
            temperature: 0.0,
            temperature_january: 0.0,
            temperature_july: 0.0,
            precipitation: 0.0,
            precipitation_january: 0.0,
            precipitation_july: 0.0,
            potential_evaporation: 0.0,
            evaporation: 0.0,
            evaporation_january: 0.0,
            evaporation_july: 0.0,
            ice: 0.0,
            discharge: 0.0,
            lake_depth: 0.0,
            current_east: 0.0,
            current_north: 0.0,
            streamfunction: 0.0,
            gyre_heating: 0.0,
            wind_east: 0.0,
            wind_north: 0.0,
            wind_january_east: 0.0,
            wind_january_north: 0.0,
            wind_july_east: 0.0,
            wind_july_north: 0.0,
            climate_class: 0,
            plate,
            flags: match kind {
                Kind::Oceanic => CellInfo::OCEANIC,
                Kind::Transitional => 0,
                Kind::Continental => CellInfo::CONTINENTAL,
            },
            drainage: CellInfo::NO_RECEIVER,
        }
    }

    fn lightness([r, g, b]: [u8; 3]) -> f64 {
        0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)
    }

    #[test]
    fn the_scale_darkens_with_age_from_warm_to_blue() {
        let scale = scale();
        assert_eq!(scale.range(), (0.0, SCALE_MAX));
        for step in 1..=200 {
            let age = f64::from(step);
            assert!(
                lightness(scale.color(age).unwrap()) < lightness(scale.color(age - 1.0).unwrap()),
                "{age}"
            );
        }
        let [r, _, b] = scale.color(30.0).unwrap();
        assert!(r > b, "young crust is warm");
        let [r, _, b] = scale.color(180.0).unwrap();
        assert!(b > r, "old crust is blue");
        // Clamped above the scale.
        assert_eq!(scale.color(500.0), scale.color(SCALE_MAX));
        let (low, high) = scale.range();
        assert!(scale.ticks.iter().all(|&(t, _)| t >= low && t <= high));
    }

    #[test]
    fn each_kind_of_crust_has_its_colours() {
        let scale = scale();
        let grey = |[r, g, b]: [u8; 3]| r == g && g == b;
        for continents in [Continents::Grey, Continents::Orogeny] {
            for age in [0.0, 50.0, 250.0, 999.0, 1500.0] {
                let mut land = cell(age, Kind::Continental, 0);
                land.since_orogeny = age;
                let land = color(&scale, continents, &land);
                assert!(grey(land), "{continents:?} {age}");
                // Away from the dark end of the scale.
                assert!(lightness(land) > 100.0);
                let ocean = color(&scale, continents, &cell(age, Kind::Oceanic, 0));
                assert!(!grey(ocean) && ocean != TRANSITIONAL);
                let transitional = cell(age, Kind::Transitional, 0);
                assert_eq!(color(&scale, continents, &transitional), TRANSITIONAL);
            }
        }
        let young_land = cell(5.0, Kind::Continental, 0);
        assert_eq!(color(&scale, Continents::Grey, &young_land), GREY);
        // Classes: lighter is more recent; the last class takes the rest and NaN (none).
        assert_eq!(class_color(&OROGENY_CLASSES, 20.0), OROGENY_CLASSES[0].1);
        assert_eq!(class_color(&OROGENY_CLASSES, 150.0), OROGENY_CLASSES[2].1);
        assert_eq!(class_color(&OROGENY_CLASSES, 900.0), OROGENY_CLASSES[3].1);
        assert_eq!(
            class_color(&OROGENY_CLASSES, f32::NAN),
            OROGENY_CLASSES[3].1
        );
        for pair in OROGENY_CLASSES.windows(2) {
            assert!(pair[0].0 < pair[1].0);
            assert!(lightness(pair[0].1) > lightness(pair[1].1));
        }
        // Oceanic crust older than the simulation (also just below 1000 Myr after rounding)
        // has a colour of its own; a missing age shows as missing.
        for age in [999.75, 1000.0, 1100.0] {
            assert_eq!(
                color(&scale, Continents::Grey, &cell(age, Kind::Oceanic, 0)),
                OLD_OCEAN
            );
            assert!(sea_floor_age(&cell(age, Kind::Oceanic, 0)).is_nan());
        }
        let unknown = cell(f32::NAN, Kind::Oceanic, 0);
        assert_eq!(color(&scale, Continents::Grey, &unknown), MISSING);
        assert!(sea_floor_age(&cell(80.0, Kind::Continental, 0)).is_nan());
        assert!(sea_floor_age(&cell(80.0, Kind::Transitional, 0)).is_nan());
        assert_eq!(sea_floor_age(&cell(80.0, Kind::Oceanic, 0)), 80.0);
    }

    #[test]
    fn isochrons_skip_plate_boundaries_other_crust_and_age_jumps() {
        let cells = [
            cell(10.0, Kind::Oceanic, 0),
            cell(30.0, Kind::Oceanic, 0),
            cell(25.0, Kind::Oceanic, 0),
            cell(20.0, Kind::Oceanic, 1),
            cell(150.0, Kind::Oceanic, 0),
            cell(20.0, Kind::Continental, 0),
            cell(20.0, Kind::Transitional, 0),
            cell(1000.0, Kind::Oceanic, 0),
        ];
        let faces = [
            [0, 1, 2],
            [0, 1, 3],
            [0, 1, 4],
            [0, 1, 5],
            [0, 1, 6],
            [0, 1, 7],
        ];
        assert_eq!(isochron_faces(&faces, &cells), vec![[0, 1, 2]]);
    }

    #[test]
    fn options_and_legend() {
        let mut options = OptionValues::defaults(&OPTIONS);
        assert_eq!(Continents::of(&options), Continents::Grey);
        assert_eq!(isochron_interval(&options), Some(20.0));
        assert!(options.is_on(COASTLINES));
        // Continental crust, coastline, isochrons.
        assert_eq!(CrustAge.legend(&options, None).swatches.len(), 3);
        options.set(CONTINENTS, 1);
        options.set(ISOCHRONS, 2);
        options.set(COASTLINES, 0);
        assert_eq!(Continents::of(&options), Continents::Orogeny);
        assert_eq!(isochron_interval(&options), None);
        assert_eq!(CrustAge.legend(&options, None).swatches.len(), 4);
        options.set(ISOCHRONS, 1);
        assert_eq!(isochron_interval(&options), Some(50.0));
        assert_eq!(CrustAge.legend(&options, None).swatches.len(), 5);
        let cells = [
            cell(10.0, Kind::Oceanic, 0),
            cell(30.0, Kind::Oceanic, 0),
            cell(1.0, Kind::Continental, 0),
            cell(1.0, Kind::Transitional, 0),
        ];
        let (share, mean, max) = sea_floor_statistics(&cells).unwrap();
        assert!((share - 0.5).abs() < 1e-9);
        assert_eq!((mean, max), (20.0, 30.0));
        assert!(sea_floor_statistics(&cells[2..]).is_none());
    }

    #[test]
    fn a_planet_in_crust_age() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 2);
        let mut options = OptionValues::defaults(&OPTIONS);
        let mut sizes = Vec::new();
        for isochrons in [2, 1, 0] {
            options.set(ISOCHRONS, isochrons);
            let data = ModeData {
                mesh,
                vertices: &vertices,
                cells: &cells,
                plates: &[],
                plate_colors: &plate_colors,
                fields: &fields,
                arrow_cells: &arrow_cells,
                arrow_spacing,
                options: &options,
            };
            let colors = CrustAge.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            assert!(colors.iter().all(|&color| color != rgba(MISSING)));
            let mut overlay = OverlayBuilder::default();
            CrustAge.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            assert!(
                vertices
                    .iter()
                    .any(|vertex| vertex.color == rgba(COAST_COLOR))
            );
            sizes.push(vertices.len());
            let legend = CrustAge.legend(&options, Some(&data));
            assert!(legend.description.contains("mean age"));
        }
        // More isochrons with a smaller interval.
        assert!(sizes[0] < sizes[1] && sizes[1] < sizes[2], "{sizes:?}");
    }
}
