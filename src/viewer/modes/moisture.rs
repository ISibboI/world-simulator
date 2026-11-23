//! Moisture: precipitation (annual, or of January or July) on a logarithmic sequential scale,
//! or the aridity index of land, the ratio of the annual precipitation to the potential
//! evaporation (#95).
//!
//! Everything comes from the cells every frame keeps (annual, January and July precipitation and
//! the annual potential evaporation of land at 1 mm/yr, see `history::codec`), so past frames
//! show alike. As in the temperature mode, the colours are shown unlit with the coastlines drawn
//! as lines.
//!
//! The aridity index `AI = P / PET` is UNEP's (Middleton & Thomas, 1997): hyper-arid below
//! 0.05, arid to 0.2, semi-arid to 0.5, dry sub-humid to 0.65, humid above. The simulation's
//! potential evaporation is that of a wet land surface at the land temperature with the humidity
//! of the air above (`climate::WaterCycle::potential_evaporation`), the same that its
//! Budyko-curve evapotranspiration uses: below AI = 1 land is water-limited and most of its rain
//! evaporates again, above it energy-limited and much of the rain runs off into rivers. The
//! ocean has no potential evaporation in the model and no aridity index. The potential
//! evaporation is aerodynamic only (no radiation term, unlike the Penman–Monteith PET UNEP's
//! class limits were set with), so the limits are nominal here; on ice sheets, colder than the
//! air above, it is about zero, so they are shown apart.

use super::super::overlay::{LineStyle, OverlayBuilder, isolines};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Shading,
    Swatch, SwatchShape, coastlines, rgba,
};
use crate::viewer::CellInfo;

pub struct Moisture;

/// What the mode shows: the value of the option [`SHOW`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Precipitation,
    Aridity,
}

impl View {
    fn of(options: &OptionValues) -> Self {
        match options.get(SHOW) {
            1 => View::Aridity,
            _ => View::Precipitation,
        }
    }
}

/// Which precipitation is shown: the value of the option [`PERIOD`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Period {
    Annual,
    January,
    July,
}

impl Period {
    fn of(options: &OptionValues) -> Self {
        match options.get(PERIOD) {
            1 => Period::January,
            2 => Period::July,
            _ => Period::Annual,
        }
    }

    /// The precipitation in m/yr.
    fn precipitation(self, cell: &CellInfo) -> f32 {
        match self {
            Period::Annual => cell.precipitation,
            Period::January => cell.precipitation_january,
            Period::July => cell.precipitation_july,
        }
    }
}

/// Indices of the options.
const SHOW: usize = 0;
const PERIOD: usize = 1;
const DESERT_LINE: usize = 2;

const OPTIONS: [ModeOption; 3] = [
    ModeOption {
        label: "Show",
        hover: "Precipitation, or the aridity index of land: precipitation divided by the \
                potential evaporation",
        kind: OptionKind::Choice(&["Precipitation", "Aridity"]),
    },
    ModeOption {
        label: "Season",
        hover: "The year, or the rate of January (northern winter) or July (northern summer), as \
                if it lasted a year; the aridity index is always that of the year",
        kind: OptionKind::Choice(&["Year", "January", "July"]),
    },
    ModeOption {
        label: "Desert edge",
        hover: "A line on land where the aridity index is 0.2: drier land is arid (desert), \
                wetter land semi-arid (steppe)",
        kind: OptionKind::Switch { default: false },
    },
];

/// The aridity index at the edge of deserts (UNEP: arid below, semi-arid above).
const DESERT_EDGE: f32 = 0.2;

/// What a cell is in the aridity view.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ground {
    Ocean,
    /// Land mostly under an ice sheet, where the index means little: the ice is colder than
    /// the air above, so it hardly evaporates and would count as humid however little snow falls.
    Ice,
    /// Land with its aridity index, NaN where it is not known (no potential evaporation, e.g.
    /// land that just rose from the sea, whose potential evaporation the water cycle has not
    /// computed yet).
    Land(f32),
}

/// Land at least this share under permanent ice is shown as ice in the aridity view.
const ICE_SHARE: f32 = 0.5;

fn ground(cell: &CellInfo) -> Ground {
    if cell.elevation < 0.0 {
        Ground::Ocean
    } else if cell.ice >= ICE_SHARE {
        Ground::Ice
    } else {
        Ground::Land(cell.aridity_index())
    }
}

/// The aridity index of land without ice, NaN elsewhere (for the desert-edge line, which so
/// ends at coasts and ice).
fn land_aridity(cell: &CellInfo) -> f32 {
    match ground(cell) {
        Ground::Land(index) => index,
        Ground::Ocean | Ground::Ice => f32::NAN,
    }
}

/// Precipitation colours: ColorBrewer's YlGnBu (sequential, colour-blind safe, lightness
/// falling monotonically from pale yellow to dark blue), evenly spread on a logarithmic axis
/// from [`PRECIPITATION_RANGE`]'s lower end to its upper one.
const YL_GN_BU: [[u8; 3]; 9] = [
    [255, 255, 217],
    [237, 248, 177],
    [199, 233, 180],
    [127, 205, 187],
    [65, 182, 196],
    [29, 145, 192],
    [34, 94, 168],
    [37, 52, 148],
    [8, 29, 88],
];

/// The ends of the precipitation scale in mm/yr; values beyond are clamped.
const PRECIPITATION_RANGE: (f64, f64) = (30.0, 5000.0);
const PRECIPITATION_TICKS: [u32; 7] = [50, 100, 200, 500, 1000, 2000, 5000];

/// Aridity colours: ColorBrewer's BrBG (diverging, colour-blind safe), on a logarithmic axis,
/// brown where it is dry, white where precipitation equals the potential evaporation (AI = 1),
/// blue-green where it is wet.
const BR_BG: [[u8; 3]; 11] = [
    [84, 48, 5],
    [140, 81, 10],
    [191, 129, 45],
    [223, 194, 125],
    [246, 232, 195],
    [245, 245, 245],
    [199, 234, 229],
    [128, 205, 193],
    [53, 151, 143],
    [1, 102, 94],
    [0, 60, 48],
];

/// The ends of the aridity scale; values beyond are clamped. The dry arm reaches hyper-arid
/// deserts, the wet arm rainforests and wet mountains.
const ARIDITY_RANGE: (f64, f64) = (0.02, 10.0);
const ARIDITY_TICKS: [(f64, &str); 7] = [
    (0.02, "0.02"),
    (0.05, "0.05"),
    (0.2, "0.2"),
    (0.5, "0.5"),
    (1.0, "1"),
    (3.0, "3"),
    (10.0, "10"),
];

/// Colours of the ocean (it has no aridity index) and of ice sheets in the aridity view, and of
/// cells without data.
/// The ocean is a darker blue-grey than the near-white middle of the aridity scale (AI = 1).
const OCEAN: [u8; 3] = [140, 152, 170];
/// Ice sheets in lilac: neither the near-white middle of the scale nor its teal wet arm.
const ICE: [u8; 3] = [200, 185, 225];
const MISSING: [u8; 3] = [90, 90, 90];

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [25, 25, 25];
const DESERT_WIDTH: f64 = 0.03;
/// A red line with a white border: it stands out on the browns and yellows around it.
const DESERT_COLOR: [u8; 3] = [200, 20, 30];
const BORDER_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Lift of the lines above the surface (planet radii).
const LIFT: f64 = 0.003;

/// The logarithm of a positive value, the axis of both scales.
fn log(value: f64) -> f64 {
    crate::math::log10(value)
}

/// A scale with `colors` spread evenly on a logarithmic axis from `low` to `high`, with
/// `ticks`. The values of the scale are the base-10 logarithms.
fn log_scale(
    colors: &[[u8; 3]],
    (low, high): (f64, f64),
    pivot: Option<f64>,
    title: &'static str,
    ticks: impl IntoIterator<Item = (f64, String)>,
) -> ColorScale {
    let (low, high) = (log(low), log(high));
    let last = colors.len() - 1;
    let stops = match pivot {
        // A diverging scale: the middle colour at the pivot, each arm spread over its side.
        Some(pivot) => {
            let pivot = log(pivot);
            let middle = last / 2;
            colors
                .iter()
                .enumerate()
                .map(|(index, &color)| {
                    let value = if index <= middle {
                        low + (pivot - low) * index as f64 / middle as f64
                    } else {
                        pivot + (high - pivot) * (index - middle) as f64 / (last - middle) as f64
                    };
                    (value, color)
                })
                .collect()
        }
        None => colors
            .iter()
            .enumerate()
            .map(|(index, &color)| (low + (high - low) * index as f64 / last as f64, color))
            .collect(),
    };
    ColorScale {
        stops,
        title: title.into(),
        ticks: ticks
            .into_iter()
            .map(|(value, label)| (log(value), label.into()))
            .collect(),
    }
}

/// The precipitation scale (log10 of mm/yr) for a period.
fn precipitation_scale(period: Period) -> ColorScale {
    log_scale(
        &YL_GN_BU,
        PRECIPITATION_RANGE,
        None,
        match period {
            Period::Annual => "Annual precipitation (mm/yr, logarithmic)",
            Period::January => "January precipitation (mm/yr, logarithmic)",
            Period::July => "July precipitation (mm/yr, logarithmic)",
        },
        PRECIPITATION_TICKS
            .into_iter()
            .map(|tick| (f64::from(tick), tick.to_string())),
    )
}

/// The aridity scale (log10 of the index).
fn aridity_scale() -> ColorScale {
    log_scale(
        &BR_BG,
        ARIDITY_RANGE,
        Some(1.0),
        "Aridity index: precipitation / potential evaporation (logarithmic)",
        ARIDITY_TICKS
            .into_iter()
            .map(|(value, label)| (value, label.to_owned())),
    )
}

/// The value on the axis of the scales: log10 of mm/yr or of the index, NaN where unknown. Zero
/// precipitation lies at the dry end.
fn axis(value: f32, scale_unit: f32) -> f32 {
    if value.is_nan() {
        f32::NAN
    } else {
        (value * scale_unit).max(1e-6).log10()
    }
}

/// The colour of a cell in the aridity view.
fn aridity_color(scale: &ColorScale, cell: &CellInfo) -> [u8; 3] {
    match ground(cell) {
        Ground::Ocean => OCEAN,
        Ground::Ice => ICE,
        Ground::Land(index) => scale.color(f64::from(axis(index, 1.0))).unwrap_or(MISSING),
    }
}

impl RenderMode for Moisture {
    fn label(&self) -> &'static str {
        "Moisture"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        Some(match View::of(data.options) {
            View::Precipitation => {
                let period = Period::of(data.options);
                precipitation_scale(period).colors(
                    data.cells
                        .iter()
                        .map(|cell| axis(period.precipitation(cell), 1000.0)),
                    MISSING,
                )
            }
            View::Aridity => {
                let scale = aridity_scale();
                data.cells
                    .iter()
                    .map(|cell| rgba(aridity_color(&scale, cell)))
                    .collect()
            }
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
            rgba(COAST_COLOR),
            LIFT,
        );
        if data.options.is_on(DESERT_LINE) {
            overlay.next_layer();
            let style = LineStyle {
                width: DESERT_WIDTH * spacing,
                border: 0.3 * DESERT_WIDTH * spacing,
                color: rgba(DESERT_COLOR),
                border_color: BORDER_COLOR,
                lift: LIFT,
            };
            // On the logarithmic axis, so that the line lies where the colours change.
            let pieces = isolines(
                data.mesh.faces(),
                |cell| axis(land_aridity(&data.cells[cell as usize]), 1.0),
                DESERT_EDGE.log10(),
                |cell| data.position(cell).as_dvec3(),
            );
            for [from, to] in pieces {
                overlay.line(from, to, &style);
            }
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let view = View::of(options);
        let period = Period::of(options);
        let mut swatches = Vec::new();
        let missing = data.is_some_and(|data| {
            data.cells.iter().any(|cell| match view {
                View::Precipitation => period.precipitation(cell).is_nan(),
                View::Aridity => matches!(ground(cell), Ground::Land(index) if index.is_nan()),
            })
        });
        if view == View::Aridity {
            swatches.push(Swatch::fill(OCEAN, "Ocean"));
            swatches.push(Swatch::fill(ICE, "Ice sheet"));
        }
        if missing {
            swatches.push(Swatch::fill(MISSING, "No data"));
        }
        swatches.push(Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline"));
        if options.is_on(DESERT_LINE) {
            swatches.push(Swatch::new(
                DESERT_COLOR,
                SwatchShape::Line,
                "Desert edge (0.2)",
            ));
        }
        let (description, scale) = match view {
            View::Precipitation => {
                let period_text = match period {
                    Period::Annual => "over the year",
                    Period::January => {
                        "in January, northern winter (the rate of the month, per year)"
                    }
                    Period::July => "in July, northern summer (the rate of the month, per year)",
                };
                (
                    format!(
                        "Precipitation (rain and snow) {period_text}. It is high where moist \
                         air rises: in the tropical convergence zone, the mid-latitude storm \
                         tracks and on windward slopes."
                    ),
                    precipitation_scale(period),
                )
            }
            View::Aridity => (
                "Aridity index of land: annual precipitation over potential evaporation. \
                 Below 0.05 hyper-arid, to 0.2 arid (desert), to 0.5 semi-arid, to 0.65 dry \
                 sub-humid, humid above. Cold land evaporates little and counts as humid even \
                 where little snow falls."
                    .to_owned(),
                aridity_scale(),
            ),
        };
        Legend {
            description: description.into(),
            scale: Some(scale),
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance (sRGB weights, without linearisation), for monotonicity checks.
    fn lightness(scale: &ColorScale, value: f64) -> f64 {
        let [r, g, b] = scale.color(value).unwrap().map(f64::from);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    #[test]
    fn precipitation_darkens_as_it_grows() {
        let scale = precipitation_scale(Period::Annual);
        let (low, high) = scale.range();
        assert!((low - log(30.0)).abs() < 1e-12 && (high - log(5000.0)).abs() < 1e-12);
        let steps = 100;
        for step in 1..=steps {
            let value = low + (high - low) * f64::from(step) / f64::from(steps);
            let before = low + (high - low) * f64::from(step - 1) / f64::from(steps);
            assert!(
                lightness(&scale, value) < lightness(&scale, before),
                "{value}"
            );
        }
        assert!(scale.ticks.iter().all(|&(t, _)| t >= low && t <= high));
        // Zero precipitation is at the dry end, NaN has no colour.
        assert_eq!(scale.color(f64::from(axis(0.0, 1000.0))), Some(YL_GN_BU[0]));
        assert!(axis(f32::NAN, 1000.0).is_nan());
        // 1 m/yr is 1000 mm/yr.
        assert!((axis(1.0, 1000.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn aridity_diverges_around_one() {
        let scale = aridity_scale();
        assert_eq!(scale.color(0.0), Some(BR_BG[5]));
        let (low, high) = scale.range();
        assert!((low - log(0.02)).abs() < 1e-12 && (high - log(10.0)).abs() < 1e-12);
        // Lightness falls away from AI = 1 on both arms.
        for step in 1..50 {
            let t = f64::from(step) / 50.0;
            assert!(lightness(&scale, low * t) < lightness(&scale, low * (t - 0.02)));
            assert!(lightness(&scale, high * t) < lightness(&scale, high * (t - 0.02)));
        }
        // Dry is brown (red over blue), wet blue-green.
        let [r, _, b] = scale.color(log(0.05)).unwrap();
        assert!(r > b);
        let [r, _, b] = scale.color(log(3.0)).unwrap();
        assert!(b > r);
        assert!(
            scale
                .ticks
                .iter()
                .all(|&(t, _)| t >= low - 1e-12 && t <= high + 1e-12)
        );
    }

    #[test]
    fn the_aridity_index_of_cells() {
        let mut cell: CellInfo = bytemuck::Zeroable::zeroed();
        cell.elevation = 100.0;
        cell.precipitation = 0.2;
        cell.potential_evaporation = 1.0;
        assert_eq!(ground(&cell), Ground::Land(0.2));
        assert!((land_aridity(&cell) - 0.2).abs() < 1e-6);
        // Without potential evaporation (not computed yet) the index is unknown.
        cell.potential_evaporation = 0.0;
        assert!(land_aridity(&cell).is_nan());
        assert!(matches!(ground(&cell), Ground::Land(index) if index.is_nan()));
        // Ice sheets and the ocean have none.
        cell.potential_evaporation = 1.0;
        cell.ice = 0.8;
        assert_eq!(ground(&cell), Ground::Ice);
        assert!(land_aridity(&cell).is_nan());
        cell.ice = 0.0;
        cell.elevation = -10.0;
        assert_eq!(ground(&cell), Ground::Ocean);
        assert!(land_aridity(&cell).is_nan());
    }

    #[test]
    fn options_choose_what_is_shown() {
        let mut options = OptionValues::defaults(&OPTIONS);
        assert_eq!(View::of(&options), View::Precipitation);
        assert_eq!(Period::of(&options), Period::Annual);
        assert!(!options.is_on(DESERT_LINE));
        options.set(PERIOD, 2);
        let legend = Moisture.legend(&options, None);
        assert!(legend.scale.unwrap().title.starts_with("July"));
        assert_eq!(legend.swatches.len(), 1);
        options.set(SHOW, 1);
        options.set(DESERT_LINE, 1);
        let legend = Moisture.legend(&options, None);
        assert!(legend.scale.unwrap().title.starts_with("Aridity"));
        // Ocean, ice, coastline and desert edge.
        assert_eq!(legend.swatches.len(), 4);
    }

    #[test]
    fn a_planet_in_moisture() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = cell_infos(&planet);
        assert!(cells.iter().any(|cell| cell.precipitation_july > 0.0));
        assert!(
            cells
                .iter()
                .any(|cell| cell.elevation >= 0.0 && cell.potential_evaporation > 0.0)
        );
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 2);
        let mut options = OptionValues::defaults(&OPTIONS);
        let mut overlay_sizes = Vec::new();
        for (show, period, desert) in [(0, 0, 0), (0, 1, 1), (1, 2, 1)] {
            options.set(SHOW, show);
            options.set(PERIOD, period);
            options.set(DESERT_LINE, desert);
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
            let colors = Moisture.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            for (cell, color) in cells.iter().zip(&colors) {
                let expected = if show == 0 {
                    let period = Period::of(&options);
                    precipitation_scale(period)
                        .color(f64::from(axis(period.precipitation(cell), 1000.0)))
                        .unwrap()
                } else {
                    aridity_color(&aridity_scale(), cell)
                };
                assert_eq!(*color, rgba(expected));
            }
            let mut overlay = OverlayBuilder::default();
            Moisture.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            let has = |color: [u8; 3]| vertices.iter().any(|vertex| vertex.color == rgba(color));
            assert!(has(COAST_COLOR));
            overlay_sizes.push(vertices.len());
        }
        // The desert edge adds lines (a fresh planet has deserts in the subtropics).
        assert!(overlay_sizes[0] <= overlay_sizes[1]);
    }

    /// Prints quantiles of the simulated precipitation and aridity, to choose the ranges of the
    /// scales (`cargo test --release moisture_distribution -- --ignored --nocapture`, with
    /// `MOISTURE_SUBDIVISIONS` and `MOISTURE_STEPS`).
    #[test]
    #[ignore]
    fn moisture_distribution() {
        use clap::Parser;
        let variable = |name: &str, default: u32| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(default)
        };
        let subdivisions = variable("MOISTURE_SUBDIVISIONS", 5);
        let steps = variable("MOISTURE_STEPS", 0);
        let cli = crate::Cli::parse_from([
            "world-simulator".to_owned(),
            format!("--subdivisions={subdivisions}"),
        ]);
        let mut planet = crate::build_planet(&cli).unwrap();
        for _ in 0..steps {
            planet.step(cli.dt);
        }
        let cells = crate::viewer::cell_infos(&planet);
        let quantiles = |name: &str, mut values: Vec<f32>| {
            values.retain(|value| value.is_finite());
            values.sort_by(f32::total_cmp);
            let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
            eprintln!(
                "{name:24} n {:6}  min {:9.4}  1% {:9.4}  5% {:9.4}  25% {:9.4}  50% {:9.4}  \
                 75% {:9.4}  95% {:9.4}  99% {:9.4}  max {:9.4}",
                values.len(),
                at(0.0),
                at(0.01),
                at(0.05),
                at(0.25),
                at(0.5),
                at(0.75),
                at(0.95),
                at(0.99),
                at(1.0)
            );
        };
        let select = |land: bool, value: &dyn Fn(&CellInfo) -> f32| -> Vec<f32> {
            cells
                .iter()
                .filter(|cell| (cell.elevation >= 0.0) == land)
                .map(value)
                .collect()
        };
        for (land, name) in [(true, "land"), (false, "ocean")] {
            quantiles(
                &format!("P year {name} (m)"),
                select(land, &|c| c.precipitation),
            );
            quantiles(
                &format!("P Jan {name} (m)"),
                select(land, &|c| c.precipitation_january),
            );
            quantiles(
                &format!("P Jul {name} (m)"),
                select(land, &|c| c.precipitation_july),
            );
        }
        quantiles("PET land (m)", select(true, &|c| c.potential_evaporation));
        quantiles("AI land (no ice)", select(true, &land_aridity));
        let land = select(true, &land_aridity);
        let share = |low: f32, high: f32| {
            land.iter().filter(|&&ai| ai >= low && ai < high).count() as f64
                / land.iter().filter(|ai| ai.is_finite()).count() as f64
        };
        eprintln!(
            "UNEP classes of land: hyper-arid {:.3}, arid {:.3}, semi-arid {:.3}, dry \
             sub-humid {:.3}, humid {:.3}",
            share(0.0, 0.05),
            share(0.05, 0.2),
            share(0.2, 0.5),
            share(0.5, 0.65),
            share(0.65, f32::INFINITY)
        );
    }
}
