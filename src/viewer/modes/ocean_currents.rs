//! Ocean currents: the wind-driven gyres of the climate model as arrows (#96).
//!
//! The model ([`crate::ocean`]) solves Stommel's vorticity balance for a streamfunction ψ on the
//! coarse mesh of the energy balance model (icosphere 4, about 440 km between cells): the
//! depth-integrated volume transport of the wind-driven circulation, with western boundary
//! currents one coarse cell wide. It has no depth structure, so there is no speed, only a
//! transport per unit width (shown in Sv per 100 km, 10 m²/s); the winds are an annual mean, so
//! the gyres change only with the land. Every cell keeps the transport vector and ψ,
//! interpolated to the fine mesh (see `TemperatureFields::ocean_current`), so past frames show
//! alike.
//!
//! The arrows stand at the cells nearest the vertices of the coarse mesh, the resolution of the
//! model. Their colour can tell whether the currents warm or cool the sea where they flow: the
//! annual mean heat the gyres deliver to the mixed layer in the model (`−u·∇T`: positive where
//! the water comes from warmer sea than it flows on to). The background shows the
//! transport, the gyres (ψ: clockwise or anticlockwise), or the sea-surface temperature;
//! streamlines are isolines of ψ (between two of them flows the same transport).

use super::super::arrows::{ArrowStyle, Sample, sample_cells};
use super::super::overlay::{LineStyle, OverlayBuilder, isolines};
use super::{
    ArrowScale, ColorScale, DEFAULT_ARROW_SPACING, Legend, ModeData, ModeOption, OptionKind,
    OptionValues, RenderMode, Shading, Swatch, SwatchShape, coastlines, rgba,
};
use crate::viewer::CellInfo;

pub struct OceanCurrents;

/// What the background shows: the value of the option [`BACKGROUND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Background {
    Transport,
    Gyres,
    SeaTemperature,
}

impl Background {
    fn of(options: &OptionValues) -> Self {
        match options.get(BACKGROUND) {
            1 => Background::Gyres,
            2 => Background::SeaTemperature,
            _ => Background::Transport,
        }
    }
}

/// Indices of the options.
const BACKGROUND: usize = 0;
const WARM_COLD: usize = 1;
const STREAMLINES: usize = 2;

const OPTIONS: [ModeOption; 3] = [
    ModeOption {
        label: "Colours",
        hover: "The transport of the currents, the gyres (clockwise or anticlockwise), or the \
                annual mean sea-surface temperature",
        kind: OptionKind::Choice(&["Transport", "Gyres", "Sea temp."]),
    },
    ModeOption {
        label: "Warm / cold",
        hover: "Arrows in red where the currents warm the sea (they bring warmer water than \
                they take away, more than 10 W/m² in the annual mean), in blue where they cool it",
        kind: OptionKind::Switch { default: true },
    },
    ModeOption {
        label: "Streamlines",
        hover: "Lines along the currents (isolines of the streamfunction): the same transport \
                flows between every two of them, so they crowd where the currents are strong",
        kind: OptionKind::Switch { default: false },
    },
];

/// Unit of the arrows and the transport scale: Sv per 100 km of width, in m²/s.
const UNIT: f64 = 10.0;
/// Transport (Sv per 100 km) drawn with the full arrow length; the strongest boundary currents
/// of the model reach about 1.5.
const REFERENCE: f64 = 1.0;

/// The transport vector of an ocean cell in m²/s (east, north), if the model has one there.
fn current(cell: &CellInfo) -> Option<[f32; 2]> {
    (cell.is_sea() && cell.current_east.is_finite() && cell.current_north.is_finite())
        .then_some([cell.current_east, cell.current_north])
}

/// The transport in Sv per 100 km, NaN where there is none.
fn transport(cell: &CellInfo) -> f32 {
    current(cell).map_or(f32::NAN, |[east, north]| {
        (east * east + north * north).sqrt() / UNIT as f32
    })
}

/// The streamfunction of an ocean cell in Sv, NaN elsewhere.
fn streamfunction(cell: &CellInfo) -> f32 {
    if cell.is_sea() {
        cell.streamfunction
    } else {
        f32::NAN
    }
}

/// Subdivisions of the coarse mesh of the climate model (`TemperatureParams`'s default, at most
/// those of the planet's mesh).
const COARSE_SUBDIVISIONS: u32 = 4;

/// Whether the gyres warm or cool the sea at an arrow: see [`kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Warm,
    Cold,
    Neutral,
}

/// Heating (W/m², annual mean) by the gyres beyond which a current counts as warming or cooling
/// the sea it reaches.
const HEATING_THRESHOLD: f32 = 10.0;

/// Whether the gyres warm the mixed layer of a cell (the current brings warmer water than it
/// takes away: the model's `ρ c_p γ Q T` upstream summed over the cell's edges, `−u·∇T`), cool it,
/// or neither.
fn kind(cell: &CellInfo) -> Kind {
    match cell.gyre_heating {
        heating if heating >= HEATING_THRESHOLD => Kind::Warm,
        heating if heating <= -HEATING_THRESHOLD => Kind::Cold,
        _ => Kind::Neutral,
    }
}

/// Arrow colours: red and blue as warm and cold, off-white for neither (all with the dark
/// border of the arrows, so they stand out on light and dark backgrounds).
const WARM: [u8; 3] = [235, 60, 40];
const COLD: [u8; 3] = [60, 190, 255];
const NEUTRAL: [u8; 3] = [250, 250, 245];

impl Kind {
    fn color(self) -> [u8; 3] {
        match self {
            Kind::Warm => WARM,
            Kind::Cold => COLD,
            Kind::Neutral => NEUTRAL,
        }
    }
}

/// The arrow style for arrows `spacing` apart: length with the square root of the transport, so
/// that the slow interior shows its direction next to the boundary currents.
fn style(spacing: f64, color: [u8; 3]) -> ArrowStyle {
    ArrowStyle {
        exponent: 0.5,
        color: rgba(color),
        ..ArrowStyle::for_spacing(spacing, REFERENCE)
    }
}

/// The cells arrows stand on (nearest the vertices of the coarse mesh), and their spacing in
/// planet radii.
fn arrow_cells(data: &ModeData) -> (Vec<u32>, f64) {
    sample_cells(data.mesh, COARSE_SUBDIVISIONS.min(data.mesh.subdivisions()))
}

/// The arrows of the currents (in Sv per 100 km) by kind (all [`Kind::Neutral`] without the
/// warm/cold colours).
fn arrows(data: &ModeData, warm_cold: bool) -> (Vec<(Kind, Sample)>, f64) {
    let (cells, spacing) = arrow_cells(data);
    let samples = data
        .tangent_samples(&cells, spacing, |cell| {
            current(cell).map(|[east, north]| [east / UNIT as f32, north / UNIT as f32])
        })
        .into_iter()
        .map(|(cell, sample)| {
            let kind = if warm_cold {
                kind(&data.cells[cell as usize])
            } else {
                Kind::Neutral
            };
            (kind, sample)
        })
        .collect();
    (samples, spacing)
}

/// Background colours. The transport: ColorBrewer's Purples (sequential, colour-blind safe) on
/// a logarithmic axis, so that both the slow interior and the boundary currents show; a hue
/// apart from the red, light blue and white arrows.
const PURPLES: [[u8; 3]; 9] = [
    [252, 251, 253],
    [239, 237, 245],
    [218, 218, 235],
    [188, 189, 220],
    [158, 154, 200],
    [128, 125, 186],
    [106, 81, 163],
    [84, 39, 143],
    [63, 0, 125],
];
/// The ends of the transport scale in Sv per 100 km; values beyond are clamped.
const TRANSPORT_RANGE: (f64, f64) = (0.02, 2.0);
const TRANSPORT_TICKS: [(f64, &str); 7] = [
    (0.02, "0.02"),
    (0.05, "0.05"),
    (0.1, "0.1"),
    (0.2, "0.2"),
    (0.5, "0.5"),
    (1.0, "1"),
    (2.0, "2"),
];

/// The gyres: ColorBrewer's PuOr (diverging, colour-blind safe), orange for clockwise gyres
/// (ψ > 0, the subtropical gyres of the north and the subpolar ones of the south), purple for
/// anticlockwise ones.
const PU_OR: [[u8; 3]; 11] = [
    [45, 0, 75],
    [84, 39, 136],
    [128, 115, 172],
    [178, 171, 210],
    [216, 218, 235],
    [247, 247, 247],
    [254, 224, 182],
    [253, 184, 99],
    [224, 130, 20],
    [179, 88, 6],
    [127, 59, 8],
];
/// The ends of the streamfunction scale in Sv.
const GYRE_RANGE: f64 = 15.0;

/// Land in a flat, light grey-beige, ocean that the model leaves out (seas too small or
/// enclosed for its coarse mesh) in a darker grey.
const LAND: [u8; 3] = [200, 195, 180];
const OUTSIDE: [u8; 3] = [140, 140, 140];

/// Lines: widths in multiples of the arrow spacing of the view, and colours.
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [25, 25, 25];
const STREAMLINE_WIDTH: f64 = 0.02;
const STREAMLINE_COLOR: [u8; 3] = [40, 40, 40];
const STREAMLINE_BORDER: [f32; 4] = [1.0, 1.0, 1.0, 0.8];
const LIFT: f64 = 0.003;

fn log(value: f64) -> f64 {
    crate::math::log10(value)
}

fn transport_scale() -> ColorScale {
    let (low, high) = (log(TRANSPORT_RANGE.0), log(TRANSPORT_RANGE.1));
    let last = (PURPLES.len() - 1) as f64;
    ColorScale {
        stops: PURPLES
            .iter()
            .enumerate()
            .map(|(index, &color)| (low + (high - low) * index as f64 / last, color))
            .collect(),
        title: "Transport (Sv per 100 km of width, logarithmic)".into(),
        ticks: TRANSPORT_TICKS
            .iter()
            .map(|&(value, label)| (log(value), label.into()))
            .collect(),
    }
}

fn gyre_scale() -> ColorScale {
    let last = (PU_OR.len() - 1) as f64;
    ColorScale {
        stops: PU_OR
            .iter()
            .enumerate()
            .map(|(index, &color)| (-GYRE_RANGE + 2.0 * GYRE_RANGE * index as f64 / last, color))
            .collect(),
        title: "Streamfunction (Sv): + clockwise, − anticlockwise".into(),
        ticks: [-15, -10, -5, 0, 5, 10, 15]
            .into_iter()
            .map(|value| (f64::from(value), value.to_string().into()))
            .collect(),
    }
}

/// The colour of a cell for a background scale of `value`: land, or ocean outside the model.
fn color(cell: &CellInfo, scale: &ColorScale, value: f32) -> [u8; 3] {
    if !cell.is_sea() {
        LAND
    } else {
        scale.color(f64::from(value)).unwrap_or(OUTSIDE)
    }
}

/// A round interval for about `count` streamlines between 0 and `max` (1, 2 or 5 × 10ⁿ).
fn streamline_interval(max: f64, count: f64) -> f64 {
    let raw = (max / count).max(1e-3);
    let magnitude = 10f64.powi(log(raw).floor() as i32);
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|step| step * magnitude)
        .find(|&step| step >= raw)
        .unwrap_or(10.0 * magnitude)
}

impl RenderMode for OceanCurrents {
    fn label(&self) -> &'static str {
        "Ocean currents"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let colors: Vec<[u8; 3]> = match Background::of(data.options) {
            Background::Transport => {
                let scale = transport_scale();
                data.cells
                    .iter()
                    .map(|cell| color(cell, &scale, transport(cell).max(1e-6).log10()))
                    .collect()
            }
            Background::Gyres => {
                let scale = gyre_scale();
                data.cells
                    .iter()
                    .map(|cell| {
                        // Only where the model has currents.
                        let psi = current(cell).map_or(f32::NAN, |_| cell.streamfunction);
                        color(cell, &scale, psi)
                    })
                    .collect()
            }
            Background::SeaTemperature => {
                let scale = super::temperature::annual_scale();
                data.cells
                    .iter()
                    .map(|cell| color(cell, &scale, cell.temperature))
                    .collect()
            }
        };
        Some(colors.into_iter().map(rgba).collect())
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
        if data.options.is_on(STREAMLINES) {
            let max = data
                .cells
                .iter()
                .map(|cell| f64::from(streamfunction(cell).abs()))
                .filter(|value| value.is_finite())
                .fold(0.0, f64::max);
            if max > 0.0 {
                let interval = streamline_interval(max, 6.0);
                let style = LineStyle {
                    width: STREAMLINE_WIDTH * spacing,
                    border: 0.3 * STREAMLINE_WIDTH * spacing,
                    color: rgba(STREAMLINE_COLOR),
                    border_color: STREAMLINE_BORDER,
                    lift: LIFT,
                };
                let steps = (max / interval).floor() as i32;
                for step in (-steps..=steps).filter(|&step| step != 0) {
                    let level = (f64::from(step) * interval) as f32;
                    for [from, to] in isolines(
                        data.mesh.faces(),
                        |cell| streamfunction(&data.cells[cell as usize]),
                        level,
                        |cell| data.position(cell).as_dvec3(),
                    ) {
                        overlay.line(from, to, &style);
                    }
                }
            }
        }
        overlay.next_layer();
        let (samples, spacing) = arrows(data, data.options.is_on(WARM_COLD));
        for kind in [Kind::Neutral, Kind::Cold, Kind::Warm] {
            let of_kind: Vec<Sample> = samples
                .iter()
                .filter(|(k, _)| *k == kind)
                .map(|&(_, sample)| sample)
                .collect();
            overlay.arrows(&of_kind, &style(spacing, kind.color()));
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let spacing = data.map_or(DEFAULT_ARROW_SPACING / 2.0, |data| arrow_cells(data).1);
        let background = Background::of(options);
        let mut swatches = vec![
            Swatch::fill(LAND, "Land"),
            Swatch::fill(OUTSIDE, "Sea without currents in the model"),
            Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline"),
        ];
        if options.is_on(STREAMLINES) {
            let interval = data
                .map(|data| {
                    data.cells
                        .iter()
                        .map(|cell| f64::from(streamfunction(cell).abs()))
                        .filter(|value| value.is_finite())
                        .fold(0.0, f64::max)
                })
                .filter(|&max| max > 0.0)
                .map(|max| streamline_interval(max, 6.0));
            swatches.push(Swatch::new(
                STREAMLINE_COLOR,
                SwatchShape::Line,
                match interval {
                    Some(interval) => format!("Streamline, {interval} Sv apart"),
                    None => "Streamline".to_owned(),
                },
            ));
        }
        let warm_cold = options.is_on(WARM_COLD);
        if warm_cold {
            swatches.push(Swatch::fill(WARM, "Current warms the sea (> 10 W/m²)"));
            swatches.push(Swatch::fill(COLD, "Current cools the sea (< −10 W/m²)"));
        }
        let scale = match background {
            Background::Transport => transport_scale(),
            Background::Gyres => gyre_scale(),
            Background::SeaTemperature => super::temperature::annual_scale(),
        };
        Legend {
            description: "Wind-driven ocean gyres of the climate model (Sverdrup interior, \
                          western boundary currents), computed on its coarse mesh of about \
                          440 km, with an arrow per cell of it. They are depth-integrated \
                          transports with no depth structure (1 Sv per 100 km is 1 cm/s in a \
                          1 km deep layer), driven by a fixed annual mean wind profile. Not \
                          modelled: deep and thermohaline circulation, upwelling and Ekman \
                          pumping, seasons, flow through straits or around islands (every coast \
                          is a wall), and circumpolar currents (friction alone sets them); the \
                          overturning under the trade winds carries heat only as a zonal mean."
                .into(),
            scale: Some(scale),
            swatches,
            arrows: vec![ArrowScale::new(
                &style(spacing, NEUTRAL),
                &[0.1, 0.3, REFERENCE],
                "Sv per 100 km",
            )],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamline_intervals_are_round() {
        assert_eq!(streamline_interval(60.0, 6.0), 10.0);
        assert_eq!(streamline_interval(55.0, 6.0), 10.0);
        assert_eq!(streamline_interval(25.0, 6.0), 5.0);
        assert_eq!(streamline_interval(9.0, 6.0), 2.0);
        assert_eq!(streamline_interval(0.5, 6.0), 0.1);
    }

    #[test]
    fn kinds_follow_the_heating() {
        let cell = |gyre_heating: f32| CellInfo {
            gyre_heating,
            ..bytemuck::Zeroable::zeroed()
        };
        assert_eq!(kind(&cell(30.0)), Kind::Warm);
        assert_eq!(kind(&cell(-30.0)), Kind::Cold);
        assert_eq!(kind(&cell(5.0)), Kind::Neutral);
        assert_eq!(kind(&cell(f32::NAN)), Kind::Neutral);
    }

    #[test]
    fn a_planet_with_ocean_currents() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "4"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = cell_infos(&planet);
        // The gyres reach the cells: currents in the ocean, none on land.
        let with_current = cells.iter().filter(|cell| current(cell).is_some()).count();
        assert!(with_current > cells.len() / 4, "{with_current}");
        assert!(
            cells
                .iter()
                .filter(|cell| cell.elevation >= 0.0)
                .all(|cell| cell.streamfunction.is_nan())
        );
        let fastest = cells.iter().map(transport).fold(0.0f32, f32::max);
        assert!((0.5..100.0).contains(&fastest), "{fastest} Sv per 100 km");
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 3);
        let mut options = OptionValues::defaults(&OPTIONS);
        for (background, warm_cold, streamlines) in [(0, 1, 0), (1, 0, 1), (2, 1, 1)] {
            options.set(BACKGROUND, background);
            options.set(WARM_COLD, warm_cold);
            options.set(STREAMLINES, streamlines);
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
            let colors = OceanCurrents.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            for (cell, color) in cells.iter().zip(&colors) {
                if cell.elevation >= 0.0 {
                    assert_eq!(*color, rgba(LAND));
                }
            }
            let (samples, _) = arrows(&data, warm_cold == 1);
            assert!(samples.len() > 100, "{}", samples.len());
            if warm_cold == 1 {
                assert!(samples.iter().any(|(kind, _)| *kind != Kind::Neutral));
            }
            let mut overlay = OverlayBuilder::default();
            OceanCurrents.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            let has = |color: [u8; 3]| vertices.iter().any(|vertex| vertex.color == rgba(color));
            assert!(has(COAST_COLOR) && has(NEUTRAL));
            assert_eq!(has(STREAMLINE_COLOR), streamlines == 1);
            let legend = OceanCurrents.legend(&options, Some(&data));
            assert_eq!(legend.arrows.len(), 1);
            assert!(legend.scale.is_some());
        }
    }
}
