//! Wind: the prevailing low-level winds of the climate model as arrows (#97), over their speed,
//! the precipitation or the temperature, for the year or for January or July.
//!
//! The winds are those that carry the moisture in the water cycle ([`crate::climate`]): the
//! zonal-mean profile of the three-cell circulation (trade winds converging on the ITCZ,
//! westerlies with a weak poleward drift, polar easterlies), centred on the ITCZ of each
//! longitude, which in every solved month lies where the moist static energy of the surface air
//! peaks (so it follows the seasons and varies with longitude between land and sea). The thermally
//! driven (monsoon) winds of land–sea contrasts are added if enabled
//! (`ClimateParams::thermal_wind_depth`, off by default). Every cell keeps the annual mean and
//! the January and July winds (east and north, 0.25 m/s in the frame history), so past frames
//! show alike.
//!
//! The arrows point where the air goes, with a length proportional to the speed. An optional
//! line marks the convergence zone (ITCZ): where the meridional wind changes from northward to
//! southward within the trade winds.

use super::super::arrows::{ArrowStyle, Sample};
use super::super::overlay::{LineStyle, OverlayBuilder, isolines};
use super::{
    ArrowScale, ColorScale, DEFAULT_ARROW_SPACING, Legend, ModeData, ModeOption, OptionKind,
    OptionValues, Period, RenderMode, Shading, Swatch, SwatchShape, coastlines, moisture, rgba,
    temperature,
};
use crate::viewer::CellInfo;

pub struct Wind;

/// What the background shows: the value of the option [`BACKGROUND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Background {
    Speed,
    Precipitation,
    Temperature,
}

impl Background {
    fn of(options: &OptionValues) -> Self {
        match options.get(BACKGROUND) {
            1 => Background::Precipitation,
            2 => Background::Temperature,
            _ => Background::Speed,
        }
    }
}

/// Indices of the options.
const PERIOD: usize = 0;
const BACKGROUND: usize = 1;
const CONVERGENCE: usize = 2;

const OPTIONS: [ModeOption; 3] = [
    ModeOption {
        label: "Season",
        hover: "The mean wind of the year, or of January (northern winter) or July (northern \
                summer); the colours show the same season",
        kind: OptionKind::Choice(Period::CHOICES),
    },
    ModeOption {
        label: "Colours",
        hover: "The wind speed, the precipitation (rain falls where the winds converge and on \
                windward slopes), or the surface temperature (the convergence zone follows the \
                warmest air)",
        kind: OptionKind::Choice(&["Speed", "Precipitation", "Temperature"]),
    },
    ModeOption {
        label: "Convergence zone",
        hover: "A line where the trade winds of the two hemispheres meet (the ITCZ): the air \
                rises there, and it rains",
        kind: OptionKind::Switch { default: true },
    },
];

/// Speed (m/s) drawn with the full arrow length: the westerlies of the model reach 8 m/s.
const REFERENCE: f64 = 8.0;

/// The wind of a cell in m/s (east, north), if it is known.
fn wind(period: Period, cell: &CellInfo) -> Option<[f32; 2]> {
    let [east, north] = period.wind(cell);
    (east.is_finite() && north.is_finite()).then_some([east, north])
}

/// The wind speed of a cell in m/s, NaN where it is not known.
fn speed(period: Period, cell: &CellInfo) -> f32 {
    wind(period, cell).map_or(f32::NAN, |[east, north]| east.hypot(north))
}

/// Arrows in off-white with the dark border of the arrows, apart from every background.
const ARROW: [u8; 3] = [250, 250, 245];

/// The arrow style for arrows `spacing` apart: length proportional to the speed.
fn style(spacing: f64) -> ArrowStyle {
    ArrowStyle {
        color: rgba(ARROW),
        ..ArrowStyle::for_spacing(spacing, REFERENCE)
    }
}

/// The arrows of the winds at the view's evenly spread cells.
fn arrows(data: &ModeData, period: Period) -> Vec<Sample> {
    data.tangent_samples(data.arrow_cells, data.arrow_spacing, |cell| {
        wind(period, cell)
    })
    .into_iter()
    .map(|(_, sample)| sample)
    .collect()
}

/// Speed colours: ColorBrewer's YlOrBr (sequential, colour-blind safe), a hue apart from the
/// other modes' scales, linear from calm to [`SPEED_RANGE`].
const YL_OR_BR: [[u8; 3]; 9] = [
    [255, 255, 229],
    [255, 247, 188],
    [254, 227, 145],
    [254, 196, 79],
    [254, 153, 41],
    [236, 112, 20],
    [204, 76, 2],
    [153, 52, 4],
    [102, 37, 6],
];
/// The top of the speed scale in m/s; faster winds are clamped.
const SPEED_RANGE: f64 = 10.0;

/// Colour of cells without data.
const MISSING: [u8; 3] = [128, 128, 128];

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [25, 25, 25];
const CONVERGENCE_WIDTH: f64 = 0.04;
const CONVERGENCE_COLOR: [u8; 3] = [30, 90, 200];
const BORDER_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const LIFT: f64 = 0.003;

/// Within the trade winds (easterlies of at least this speed, m/s, within [`TROPICS`]° of the
/// equator), the line where the meridional wind changes sign from northward to southward is the
/// convergence zone; the band keeps the subtropical divergence (where the trades turn into the
/// westerlies) and the polar easterlies out.
const TRADE_WIND: f32 = 1.0;
const TROPICS: f64 = 35.0;

fn speed_scale(period: Period) -> ColorScale {
    let last = (YL_OR_BR.len() - 1) as f64;
    ColorScale {
        stops: YL_OR_BR
            .iter()
            .enumerate()
            .map(|(index, &color)| (SPEED_RANGE * index as f64 / last, color))
            .collect(),
        title: match period {
            Period::Annual => "Speed of the annual mean wind (m/s)",
            Period::January => "Speed of the January mean wind (m/s)",
            Period::July => "Speed of the July mean wind (m/s)",
        }
        .into(),
        ticks: [0, 2, 4, 6, 8, 10]
            .into_iter()
            .map(|value| (f64::from(value), value.to_string().into()))
            .collect(),
    }
}

/// The value of a cell on a colour scale.
type CellValue = Box<dyn Fn(&CellInfo) -> f32>;

/// The colour scale of the background and the value of a cell on it.
fn background(options: &OptionValues) -> (ColorScale, CellValue) {
    let period = Period::of(options, PERIOD);
    match Background::of(options) {
        Background::Speed => (
            speed_scale(period),
            Box::new(move |cell| speed(period, cell)),
        ),
        Background::Precipitation => (
            moisture::precipitation_scale(period),
            Box::new(move |cell| moisture::axis(period.precipitation(cell), 1000.0)),
        ),
        Background::Temperature => (
            temperature::scale(period),
            Box::new(move |cell| period.temperature(cell)),
        ),
    }
}

/// The pieces of the convergence zone: the isoline of zero northward wind through the
/// triangles within the trade winds where the air south of it flows north and the air north of
/// it south (not where it diverges).
fn convergence_zone(data: &ModeData, period: Period) -> Vec<[glam::DVec3; 2]> {
    let min_z = crate::math::sin(TROPICS.to_radians());
    let north = |cell: u32| {
        let z = data.mesh.vertex(cell).z;
        match wind(period, &data.cells[cell as usize]) {
            Some([east, north]) if east <= -TRADE_WIND && z.abs() < min_z => north,
            _ => f32::NAN,
        }
    };
    let faces: Vec<[u32; 3]> = data
        .mesh
        .faces()
        .iter()
        .copied()
        .filter(|face| {
            // The corners with southward wind lie north of those with northward wind.
            let mut z = [0.0f64; 2];
            let mut count = [0.0f64; 2];
            for cell in *face {
                let side = usize::from(north(cell) < 0.0);
                z[side] += data.mesh.vertex(cell).z;
                count[side] += 1.0;
            }
            count[0] > 0.0 && count[1] > 0.0 && z[1] / count[1] > z[0] / count[0]
        })
        .collect();
    isolines(&faces, north, 0.0, |cell| data.position(cell).as_dvec3())
}

impl RenderMode for Wind {
    fn label(&self) -> &'static str {
        "Wind"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let (scale, value) = background(data.options);
        Some(scale.colors(data.cells.iter().map(&value), MISSING))
    }

    fn shading(&self) -> Shading {
        Shading::Unlit
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let spacing = data.arrow_spacing;
        let period = Period::of(data.options, PERIOD);
        coastlines(
            data,
            overlay,
            COAST_WIDTH * spacing,
            rgba(COAST_COLOR),
            LIFT,
        );
        if data.options.is_on(CONVERGENCE) {
            let style = LineStyle {
                width: CONVERGENCE_WIDTH * spacing,
                border: 0.3 * CONVERGENCE_WIDTH * spacing,
                color: rgba(CONVERGENCE_COLOR),
                border_color: BORDER_COLOR,
                lift: LIFT,
            };
            for [from, to] in convergence_zone(data, period) {
                overlay.line(from, to, &style);
            }
        }
        overlay.next_layer();
        overlay.arrows(&arrows(data, period), &style(spacing));
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let spacing = data.map_or(DEFAULT_ARROW_SPACING, |data| data.arrow_spacing);
        let period = Period::of(options, PERIOD);
        let (scale, value) = background(options);
        let mut swatches = Vec::new();
        if data.is_some_and(|data| data.cells.iter().any(|cell| value(cell).is_nan())) {
            swatches.push(Swatch::fill(MISSING, "No data"));
        }
        swatches.push(Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline"));
        if options.is_on(CONVERGENCE) {
            swatches.push(Swatch::new(
                CONVERGENCE_COLOR,
                SwatchShape::Line,
                "Convergence zone (ITCZ)",
            ));
        }
        let period_text = match period {
            Period::Annual => "the annual mean",
            Period::January => "the mean of January, winter in the north",
            Period::July => "the mean of July, summer in the north",
        };
        Legend {
            description: format!(
                "Prevailing low-level winds of the climate model, {period_text}; the arrows \
                 point where the air goes. They carry the moisture of the water cycle: trade \
                 winds meeting in the convergence zone (ITCZ), westerlies and polar easterlies, \
                 a fixed profile by latitude around the ITCZ of each longitude, which follows \
                 the warmest, moistest air with the seasons. Not modelled: winds steered or \
                 blocked by mountains, sea and land breezes, storms and day-to-day weather, and \
                 (by default) monsoon winds driven by land–sea temperature contrasts."
            )
            .into(),
            scale: Some(scale),
            swatches,
            arrows: vec![ArrowScale::new(
                &style(spacing),
                &[2.0, 5.0, REFERENCE],
                "m/s",
            )],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_choose_the_season_and_the_background() {
        let mut options = OptionValues::defaults(&OPTIONS);
        assert_eq!(Period::of(&options, PERIOD), Period::Annual);
        assert_eq!(Background::of(&options), Background::Speed);
        assert!(options.is_on(CONVERGENCE));
        options.set(PERIOD, 2);
        options.set(BACKGROUND, 1);
        assert_eq!(Period::of(&options, PERIOD), Period::July);
        assert_eq!(Background::of(&options), Background::Precipitation);
        let cell = CellInfo {
            wind_east: 3.0,
            wind_north: 4.0,
            wind_july_east: f32::NAN,
            ..bytemuck::Zeroable::zeroed()
        };
        assert_eq!(speed(Period::Annual, &cell), 5.0);
        assert!(speed(Period::July, &cell).is_nan());
        assert_eq!(wind(Period::January, &cell), Some([0.0, 0.0]));
    }

    #[test]
    fn the_speed_scale_darkens_with_the_speed() {
        let scale = speed_scale(Period::Annual);
        let lightness = |value: f64| {
            let [r, g, b] = scale.color(value).unwrap().map(f64::from);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        for step in 1..=10 {
            assert!(lightness(f64::from(step)) < lightness(f64::from(step - 1)));
        }
        assert_eq!(scale.range(), (0.0, SPEED_RANGE));
    }

    #[test]
    fn a_planet_in_the_wind() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows as view_arrows, cell_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "4"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        // Trade winds from the east in the tropics, westerlies in the mid-latitudes.
        let mean_east = |from: f64, to: f64| {
            let (sum, count) = (0..mesh.len() as u32)
                .filter(|&c| (from..to).contains(&mesh.vertex(c).z.asin().to_degrees().abs()))
                .fold((0.0, 0.0), |(sum, count), c| {
                    (sum + f64::from(cells[c as usize].wind_east), count + 1.0)
                });
            sum / count
        };
        assert!(mean_east(5.0, 20.0) < -2.0, "{}", mean_east(5.0, 20.0));
        assert!(mean_east(40.0, 55.0) > 4.0, "{}", mean_east(40.0, 55.0));
        assert!(cells.iter().all(|cell| cell.wind_january_east.is_finite()));
        assert!(cells.iter().all(|cell| cell.wind_july_north.is_finite()));

        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = view_arrows::sample_cells(mesh, 3);
        let mut options = OptionValues::defaults(&OPTIONS);
        for (period, background, convergence) in [(0, 0, 1), (1, 1, 1), (2, 2, 0)] {
            options.set(PERIOD, period);
            options.set(BACKGROUND, background);
            options.set(CONVERGENCE, convergence);
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
            let colors = Wind.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            assert!(colors.iter().all(|&color| color != rgba(MISSING)));
            let samples = arrows(&data, Period::of(&options, PERIOD));
            assert!(samples.len() > arrow_cells.len() / 2, "{}", samples.len());
            // The convergence zone runs around the planet in the tropics.
            let zone = convergence_zone(&data, Period::of(&options, PERIOD));
            assert!(zone.len() > 20, "{}", zone.len());
            for [from, _] in &zone {
                let latitude = (from.z / from.length()).asin().to_degrees();
                assert!(latitude.abs() < TROPICS, "{latitude}");
            }
            let mut overlay = OverlayBuilder::default();
            Wind.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            let has = |color: [u8; 3]| vertices.iter().any(|vertex| vertex.color == rgba(color));
            assert!(has(COAST_COLOR) && has(ARROW));
            assert_eq!(has(CONVERGENCE_COLOR), convergence == 1);
            let legend = Wind.legend(&options, Some(&data));
            assert_eq!(legend.arrows.len(), 1);
            assert!(legend.scale.is_some());
        }
    }
}
