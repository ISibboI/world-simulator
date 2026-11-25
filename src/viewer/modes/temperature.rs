//! Temperature: the surface temperature (of the land surface or the sea surface) as colours of
//! a diverging scale centred on 0 °C, as the annual mean or the mean of January or July (#94).
//!
//! Everything comes from the cells every frame keeps (annual, January and July temperatures at
//! 0.1 °C, see `history::codec`), so past frames show alike. The coastlines are drawn as lines,
//! since the colours are shown unlit and the relief would not show them otherwise. Optional
//! lines: the 0 °C isotherm of the shown temperature, and on land the annual mean of −10 °C,
//! the ice-sheet threshold of the simulation (`TemperatureParams::land_ice_temperature`: land
//! with enough snowfall is ice-free above −5 °C, half covered at −10 °C and fully below −15 °C).
//! The line uses the default of that parameter; `--set temperature.land_ice_temperature=…` does
//! not move it.

use glam::DVec3;

use super::super::overlay::{LineStyle, OverlayBuilder, isolines};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, Period, RenderMode,
    Shading, Swatch, SwatchShape, coastlines, rgba,
};
use crate::viewer::CellInfo;

pub struct Temperature;

/// Indices of the options.
const PERIOD: usize = 0;
const FREEZING_LINE: usize = 1;
const ICE_LINE: usize = 2;

const OPTIONS: [ModeOption; 3] = [
    ModeOption {
        label: "Mean of",
        hover: "The annual mean, or the mean of January (northern winter) or July (northern \
                summer)",
        kind: OptionKind::Choice(Period::CHOICES),
    },
    ModeOption {
        label: "0 °C isotherm",
        hover: "A line where the shown temperature is 0 °C",
        kind: OptionKind::Switch { default: false },
    },
    ModeOption {
        label: "Ice-sheet threshold",
        hover: "A line on land where the annual mean is −10 °C: with enough snowfall, land \
                there is half covered by ice sheets (partly from −5 °C, fully below −15 °C)",
        kind: OptionKind::Switch { default: false },
    },
];

/// The ice-sheet threshold of the simulation (annual mean on land, °C; see
/// `TemperatureParams::land_ice_temperature`).
const ICE_SHEET_TEMPERATURE: f32 = -10.0;

/// The colour scale: ColorBrewer's RdBu (colour-blind safe, a diverging scale with lightness
/// steps of about equal size), white at 0 °C, 10 °C per class on the cold side and 8 °C on the
/// warm side so that both ends are reached (annual means of polar plateaus and continental
/// winters reach −50 °C, summers of deserts 40 °C). Values beyond the ends are clamped.
const STOPS: [(f64, [u8; 3]); 11] = [
    (-50.0, [5, 48, 97]),
    (-40.0, [33, 102, 172]),
    (-30.0, [67, 147, 195]),
    (-20.0, [146, 197, 222]),
    (-10.0, [209, 229, 240]),
    (0.0, [247, 247, 247]),
    (8.0, [253, 219, 199]),
    (16.0, [244, 165, 130]),
    (24.0, [214, 96, 77]),
    (32.0, [178, 24, 43]),
    (40.0, [103, 0, 31]),
];

/// Colour of cells without a temperature.
const MISSING: [u8; 3] = [128, 128, 128];

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [25, 25, 25];
const ISOTHERM_WIDTH: f64 = 0.03;
/// Dark lines with a white border, which stand out on the pale colours around 0 and −10 °C.
const FREEZING_COLOR: [u8; 3] = [20, 20, 20];
const ICE_COLOR: [u8; 3] = [110, 30, 150];
const BORDER_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Lift of the lines above the surface (planet radii).
const LIFT: f64 = 0.003;

/// The colour scale for the shown period.
pub(super) fn scale(period: Period) -> ColorScale {
    ColorScale {
        stops: STOPS.to_vec(),
        title: match period {
            Period::Annual => "Annual mean temperature (°C)",
            Period::January => "January mean temperature (°C)",
            Period::July => "July mean temperature (°C)",
        }
        .into(),
        ticks: [-50, -40, -30, -20, -10, 0, 10, 20, 30, 40]
            .into_iter()
            .map(|value| (f64::from(value), value.to_string().into()))
            .collect(),
    }
}

/// The scale of the annual mean temperature, for other modes.
pub(super) fn annual_scale() -> ColorScale {
    scale(Period::Annual)
}

impl RenderMode for Temperature {
    fn label(&self) -> &'static str {
        "Temperature"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let period = Period::of(data.options, PERIOD);
        Some(scale(period).colors(
            data.cells.iter().map(|cell| period.temperature(cell)),
            MISSING,
        ))
    }

    fn shading(&self) -> Shading {
        Shading::Unlit
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let spacing = data.arrow_spacing;
        let position = |cell: u32| data.position(cell).as_dvec3();
        coastlines(
            data,
            overlay,
            COAST_WIDTH * spacing,
            rgba(COAST_COLOR),
            LIFT,
        );
        overlay.next_layer();
        let style = |color: [u8; 3]| LineStyle {
            width: ISOTHERM_WIDTH * spacing,
            border: 0.3 * ISOTHERM_WIDTH * spacing,
            color: rgba(color),
            border_color: BORDER_COLOR,
            lift: LIFT,
        };
        let mut draw = |pieces: Vec<[DVec3; 2]>, style: &LineStyle| {
            for [from, to] in pieces {
                overlay.line(from, to, style);
            }
        };
        if data.options.is_on(FREEZING_LINE) {
            let period = Period::of(data.options, PERIOD);
            let pieces = isolines(
                data.mesh.faces(),
                |cell| period.temperature(&data.cells[cell as usize]),
                0.0,
                position,
            );
            draw(pieces, &style(FREEZING_COLOR));
        }
        if data.options.is_on(ICE_LINE) {
            let land = |cell: &CellInfo| {
                if !cell.is_sea() {
                    cell.temperature
                } else {
                    f32::NAN
                }
            };
            let pieces = isolines(
                data.mesh.faces(),
                |cell| land(&data.cells[cell as usize]),
                ICE_SHEET_TEMPERATURE,
                position,
            );
            draw(pieces, &style(ICE_COLOR));
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let period = Period::of(options, PERIOD);
        let mut swatches = vec![Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline")];
        if data.is_some_and(|data| {
            data.cells
                .iter()
                .any(|cell| period.temperature(cell).is_nan())
        }) {
            swatches.insert(0, Swatch::fill(MISSING, "No data"));
        }
        if options.is_on(FREEZING_LINE) {
            swatches.push(Swatch::new(FREEZING_COLOR, SwatchShape::Line, "0 °C"));
        }
        if options.is_on(ICE_LINE) {
            swatches.push(Swatch::new(
                ICE_COLOR,
                SwatchShape::Line,
                "−10 °C annual (land)",
            ));
        }
        let period_text = match period {
            Period::Annual => "the annual mean",
            Period::January => "the mean of January, winter in the north",
            Period::July => "the mean of July, summer in the north",
        };
        Legend {
            description: format!(
                "Surface temperature (of the land or the sea surface), {period_text}. Land is \
                 colder with height. Around an annual mean of −10 °C, land with enough \
                 snowfall turns into ice sheets."
            )
            .into(),
            scale: Some(scale(period)),
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_diverging_around_freezing() {
        let scale = scale(Period::Annual);
        assert_eq!(scale.color(0.0), Some([247, 247, 247]));
        // Lightness falls monotonically away from 0 °C on both sides.
        let lightness = |value: f64| {
            let [r, g, b] = scale.color(value).unwrap().map(f64::from);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        for step in 1..50 {
            let t = f64::from(step);
            assert!(lightness(-t) < lightness(-t + 1.0), "{t}");
            if t <= 40.0 {
                assert!(lightness(t) < lightness(t - 1.0), "{t}");
            }
        }
        // Cold is blue, warm is red.
        let [r, _, b] = scale.color(-25.0).unwrap();
        assert!(b > r);
        let [r, _, b] = scale.color(25.0).unwrap();
        assert!(r > b);
        let (low, high) = scale.range();
        assert!(scale.ticks.iter().all(|&(t, _)| t >= low && t <= high));
    }

    #[test]
    fn options_choose_the_period() {
        let mut options = OptionValues::defaults(&OPTIONS);
        assert_eq!(Period::of(&options, PERIOD), Period::Annual);
        assert!(!options.is_on(FREEZING_LINE) && !options.is_on(ICE_LINE));
        options.set(PERIOD, 2);
        assert_eq!(Period::of(&options, PERIOD), Period::July);
        let legend = Temperature.legend(&options, None);
        assert!(legend.scale.unwrap().title.starts_with("July"));
        assert_eq!(legend.swatches.len(), 1);
        options.set(FREEZING_LINE, 1);
        options.set(ICE_LINE, 1);
        assert_eq!(Temperature.legend(&options, None).swatches.len(), 3);
    }

    #[test]
    fn a_planet_in_temperature() {
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
        let mut overlay_sizes = Vec::new();
        for (period, freezing, ice) in [(0, 0, 0), (1, 1, 0), (2, 1, 1)] {
            options.set(PERIOD, period);
            options.set(FREEZING_LINE, freezing);
            options.set(ICE_LINE, ice);
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
                abiotic: false,
            };
            let colors = Temperature.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            let shown = Period::of(&options, PERIOD);
            for (cell, color) in cells.iter().zip(&colors) {
                let expected = scale(shown)
                    .color(f64::from(shown.temperature(cell)))
                    .unwrap();
                assert_eq!(*color, rgba(expected));
            }
            let mut overlay = OverlayBuilder::default();
            Temperature.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            assert_eq!(vertices.len() % 3, 0);
            let has = |color: [u8; 3]| vertices.iter().any(|vertex| vertex.color == rgba(color));
            assert!(has(COAST_COLOR));
            assert_eq!(has(FREEZING_COLOR), freezing == 1);
            overlay_sizes.push(vertices.len());
        }
        // The isotherms add lines.
        assert!(overlay_sizes[0] < overlay_sizes[1] && overlay_sizes[0] < overlay_sizes[2]);
    }
}
