//! Information about one cell of the planet: the per-cell attributes that the simulation sends
//! with every surface update ([`CellInfo`]), their formatting, and the widget of the 3D view
//! that shows them for the cell under the pointer or the selected one ([`CellInfoPanel`]).

use eframe::egui;
use glam::DVec3;

use crate::planet::Planet;

/// Per-cell attributes of a surface state, sent with every update next to the vertices.
///
/// Everything is stored as 32-bit values, so that an array of them can be sent as one byte
/// buffer (to the page on the web) and read back without parsing: 56 bytes per cell, 2.3 MB
/// per update at the default 6 subdivisions (41k cells).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInfo {
    /// Surface elevation (bedrock plus sediment) relative to sea level in metres.
    pub elevation: f32,
    /// Thickness of the crystalline crust in metres.
    pub crust_thickness: f32,
    /// Age of the lithosphere in Myr (see [`crate::tectonics::Crust::age`]).
    pub age: f32,
    /// Sediment as a rock-equivalent thickness in metres.
    pub sediment: f32,
    /// Share of the lithosphere deformed in an orogeny, 0 to 1.
    pub orogenic: f32,
    /// Time since the last orogeny in Myr, or NaN if there was none.
    pub since_orogeny: f32,
    /// Annual mean surface temperature in °C (NaN if unknown).
    pub temperature: f32,
    /// Mean surface temperature of the first month of the year (early January, the month
    /// starting at perihelion) and of the seventh (July) in °C, or NaN.
    pub temperature_january: f32,
    pub temperature_july: f32,
    /// Precipitation in m/yr.
    pub precipitation: f32,
    /// Permanent ice cover (sea ice, ice sheets), 0 to 1.
    pub ice: f32,
    /// Water discharge in m³/yr (zero while erosion is disabled).
    pub discharge: f32,
    /// The plate covering the cell.
    pub plate: u32,
    /// Bit flags, see [`CellInfo::CONTINENTAL`].
    pub flags: u32,
}

impl CellInfo {
    /// [`CellInfo::flags`]: the crust is continental (see
    /// [`crate::tectonics::Crust::is_continental`]), otherwise oceanic.
    pub const CONTINENTAL: u32 = 1;

    pub fn is_continental(&self) -> bool {
        self.flags & Self::CONTINENTAL != 0
    }
}

/// Lithosphere at least this old (Myr) is shown as "old": the simulation gives old continental
/// lithosphere a nominal birth 1000 Myr before its start.
const OLD_LITHOSPHERE: f32 = 1000.0;

/// The attributes of every cell for the current state of the planet. It costs one pass over
/// the cells, less than building the surface vertices.
pub fn cell_infos(planet: &Planet) -> Vec<CellInfo> {
    let simulation = planet.tectonics();
    let time = simulation.time();
    let temperature = planet.temperature();
    let field = |values: &[f64], cell: usize| values.get(cell).map_or(f32::NAN, |&v| v as f32);
    let month = |month: usize, cell: usize| {
        temperature
            .monthly
            .get(month)
            .map_or(f32::NAN, |values| field(values, cell))
    };
    (0..simulation.mesh().len())
        .map(|index| {
            let cell = index as u32;
            let crust = simulation.surface_crust(cell);
            CellInfo {
                elevation: simulation.surface_elevation(cell) as f32,
                crust_thickness: crust.thickness as f32,
                age: crust.age(time) as f32,
                sediment: crust.sediment as f32,
                orogenic: crust.orogenic as f32,
                since_orogeny: if crust.orogeny.is_finite() {
                    (time - crust.orogeny) as f32
                } else {
                    f32::NAN
                },
                temperature: field(&temperature.annual_mean, index),
                temperature_january: month(0, index),
                temperature_july: month(6, index),
                precipitation: field(planet.precipitation(), index),
                ice: field(&temperature.ice, index),
                discharge: field(planet.discharge(), index),
                plate: simulation.owner(cell).plate,
                flags: if crust.is_continental() {
                    CellInfo::CONTINENTAL
                } else {
                    0
                },
            }
        })
        .collect()
}

/// Latitude and longitude in degrees of a direction from the planet centre (+z is the north
/// pole, longitude 0 is +x, as in the maps).
pub fn latitude_longitude(direction: DVec3) -> (f64, f64) {
    let direction = direction.normalize_or(DVec3::X);
    let latitude = direction.z.clamp(-1.0, 1.0).asin().to_degrees();
    let longitude = direction.y.atan2(direction.x).to_degrees();
    (latitude, longitude)
}

/// "12.3° N, 45.6° E".
pub fn format_position(latitude: f64, longitude: f64) -> String {
    let north_south = if latitude < 0.0 { 'S' } else { 'N' };
    let east_west = if longitude < 0.0 { 'W' } else { 'E' };
    // Round first, so that -0.04 is "0.0° N" rather than "0.0° S".
    let latitude = (latitude * 10.0).round() / 10.0;
    let longitude = (longitude * 10.0).round() / 10.0;
    let north_south = if latitude == 0.0 { 'N' } else { north_south };
    let east_west = if longitude == 0.0 || longitude.abs() == 180.0 {
        'E'
    } else {
        east_west
    };
    format!(
        "{:.1}° {north_south}, {:.1}° {east_west}",
        latitude.abs(),
        longitude.abs()
    )
}

/// Placeholder for a value that does not apply to a cell or is not known.
pub const NOT_APPLICABLE: &str = "—";

/// One row of the widget.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: &'static str,
    /// The value, or [`NOT_APPLICABLE`].
    pub value: String,
    /// Whether the value applies to the cell; rows that do not are greyed out.
    pub applies: bool,
}

/// The labels of the rows, always all of them in this order, so that the widget does not
/// change its layout from cell to cell.
pub const ROW_LABELS: [&str; 13] = [
    "Position",
    "Elevation",
    "Surface",
    "Plate",
    "Crust",
    "Lithosphere age",
    "Sediment",
    "Orogeny",
    "Temperature",
    "Jan / Jul",
    "Precipitation",
    "Permanent ice",
    "River discharge",
];

fn row(label: &'static str, value: Option<String>) -> Row {
    match value {
        Some(value) => Row {
            label,
            value,
            applies: true,
        },
        None => Row {
            label,
            value: NOT_APPLICABLE.to_owned(),
            applies: false,
        },
    }
}

/// `Some(format(value))` if the value is known (not NaN).
fn known(value: f32, format: impl Fn(f32) -> String) -> Option<String> {
    value.is_finite().then(|| format(value))
}

fn format_temperature(value: f32) -> Option<String> {
    known(value, |t| format!("{t:.1} °C"))
}

/// The rows of the widget for a cell at `latitude`/`longitude` (degrees): always the rows of
/// [`ROW_LABELS`], with the values that do not apply to the cell (no orogeny, no ice, no river,
/// unknown values) as [`NOT_APPLICABLE`].
pub fn format_rows(info: &CellInfo, latitude: f64, longitude: f64) -> Vec<Row> {
    let land = info.elevation >= 0.0;
    let values: [Option<String>; 13] = [
        Some(format_position(latitude, longitude)),
        Some(format!("{:.0} m", info.elevation)),
        Some(
            match (land, info.ice > 0.5) {
                (true, false) => "land",
                (true, true) => "land, ice sheet",
                (false, false) => "ocean",
                (false, true) => "ocean, sea ice",
            }
            .to_owned(),
        ),
        Some(info.plate.to_string()),
        Some(format!(
            "{}, {:.1} km",
            if info.is_continental() {
                "continental"
            } else {
                "oceanic"
            },
            info.crust_thickness / 1000.0
        )),
        known(info.age, |age| {
            if age >= OLD_LITHOSPHERE {
                format!("old (≥ {OLD_LITHOSPHERE:.0} Myr)")
            } else if info.is_continental() {
                format!("{age:.0} Myr")
            } else {
                format!("{age:.0} Myr (ocean floor)")
            }
        }),
        Some(format!("{:.0} m (as rock)", info.sediment)),
        (info.since_orogeny.is_finite() || info.orogenic > 0.0).then(|| {
            format!(
                "{:.0}%, last {}",
                100.0 * info.orogenic,
                known(info.since_orogeny, |age| format!("{age:.0} Myr ago"))
                    .unwrap_or_else(|| NOT_APPLICABLE.to_owned())
            )
        }),
        format_temperature(info.temperature),
        (info.temperature_january.is_finite() || info.temperature_july.is_finite()).then(|| {
            let month = |t| format_temperature(t).unwrap_or_else(|| NOT_APPLICABLE.to_owned());
            format!(
                "{} / {}",
                month(info.temperature_january),
                month(info.temperature_july)
            )
        }),
        known(info.precipitation, |p| format!("{:.0} mm/yr", p * 1000.0)),
        (info.ice > 0.0).then(|| format!("{:.0}%", 100.0 * info.ice)),
        (land && info.discharge > 0.0).then(|| {
            format!(
                "{} m³/s",
                significant(f64::from(info.discharge) / crate::SECONDS_PER_YEAR)
            )
        }),
    ];
    ROW_LABELS
        .into_iter()
        .zip(values)
        .map(|(label, value)| row(label, value))
        .collect()
}

/// The rows of the widget while no cell is shown: all of them, without values.
pub fn empty_rows() -> Vec<Row> {
    ROW_LABELS
        .into_iter()
        .map(|label| row(label, None))
        .collect()
}

/// A number with about three significant digits and no exponent: 0.0123, 1.23, 123, 12300.
fn significant(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let digits = value.abs().log10().floor() as i32;
    let decimals = (2 - digits).max(0) as usize;
    let rounded = if digits > 2 {
        let scale = 10f64.powi(digits - 2);
        (value / scale).round() * scale
    } else {
        value
    };
    format!("{rounded:.decimals$}")
}

/// The short summary shown on the button of the hidden widget: position and elevation, padded
/// to a constant length (it is shown in a monospace font, so the button keeps its size).
pub fn format_summary(info: &CellInfo, latitude: f64, longitude: f64) -> String {
    format!(
        "{:>17} · {:>6.0} m",
        format_position(latitude, longitude),
        info.elevation
    )
}

/// Screens narrower than this (in points) start with the widget hidden; see
/// [`super::parameters`].
const NARROW_SCREEN: f32 = 600.0;
/// Largest width of the widget in points.
const MAX_WIDTH: f32 = 320.0;
/// Distance of the widget and its button from the corner of the window, in points.
const MARGIN: f32 = 8.0;

/// The cell the widget shows.
pub struct ShownCell<'a> {
    pub cell: u32,
    pub info: &'a CellInfo,
    /// Direction of the cell from the planet centre.
    pub direction: DVec3,
    /// Whether the cell is selected (clicked or tapped) rather than under the pointer.
    pub selected: bool,
}

/// The widget in the bottom left corner of the 3D view. Like the parameter panel, it can be
/// hidden; hidden, a button remains that shows a short summary of the cell.
#[derive(Default)]
pub struct CellInfoPanel {
    /// Whether the widget is shown; decided by the screen width on the first frame.
    open: Option<bool>,
    /// The highest top edge the open widget has reached since it was opened (points), so
    /// that the area it reserves does not change with the number of rows.
    reserved_top: Option<f32>,
}

impl CellInfoPanel {
    /// Shows the widget or its button and returns the area reserved for it: the area it
    /// covers, extended up to the highest it reached while open (e.g. a wrapped line), so that
    /// the parameter panel does not change size with it.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        shown: Option<ShownCell>,
        touch: bool,
    ) -> Option<egui::Rect> {
        let screen = ctx.content_rect();
        let open = *self.open.get_or_insert(screen.width() >= NARROW_SCREEN);
        let position = shown.as_ref().map(|shown| {
            let (latitude, longitude) = latitude_longitude(shown.direction);
            (shown, latitude, longitude)
        });
        if !open {
            let text = match &position {
                Some((shown, latitude, longitude)) => {
                    format!("ℹ {}", format_summary(shown.info, *latitude, *longitude))
                }
                None => "ℹ Cell".to_owned(),
            };
            self.reserved_top = None;
            return egui::Area::new(egui::Id::unique("cell info button"))
                .anchor(egui::Align2::LEFT_BOTTOM, [MARGIN, -MARGIN])
                .show(ctx, |ui| {
                    // One line: the area is as narrow as the screen allows.
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(text).monospace())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        )
                        .on_hover_text("Show information about the cell")
                        .clicked()
                    {
                        self.open = Some(true);
                    }
                })
                .response
                .rect
                .into();
        }

        let width = (screen.width() - 4.0 * MARGIN).clamp(100.0, MAX_WIDTH);
        let mut open = true;
        let response = egui::Window::new("Cell")
            .id(egui::Id::unique("cell info"))
            .anchor(egui::Align2::LEFT_BOTTOM, [MARGIN, -MARGIN])
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            // A fixed width, so that values of other lengths do not resize it.
            .min_width(width)
            .max_width(width)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((screen.height() * 0.45).max(100.0))
                    .show(ui, |ui| {
                        let rows = match &position {
                            Some((shown, latitude, longitude)) => {
                                format_rows(shown.info, *latitude, *longitude)
                            }
                            None => empty_rows(),
                        };
                        egui::Grid::new("cell info grid")
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                for row in rows {
                                    if row.applies {
                                        ui.label(row.label);
                                        ui.label(row.value);
                                    } else {
                                        ui.weak(row.label);
                                        ui.weak(row.value);
                                    }
                                    ui.end_row();
                                }
                            });
                        ui.weak(match &position {
                            Some((shown, ..)) if shown.selected => format!(
                                "Cell {}, selected; {} beside the planet to clear.",
                                shown.cell,
                                if touch { "tap" } else { "click" }
                            ),
                            Some((shown, ..)) => format!("Cell {}; click to keep it.", shown.cell),
                            None if touch => "Tap the planet to see a cell.".to_owned(),
                            None => "Point at the planet to see a cell.".to_owned(),
                        });
                    });
            });
        self.open = Some(open);
        let mut rect = response?.response.rect;
        if open {
            let top = self
                .reserved_top
                .map_or(rect.top(), |top| top.min(rect.top()));
            self.reserved_top = Some(top);
            rect.min.y = top;
        } else {
            self.reserved_top = None;
        }
        Some(rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> CellInfo {
        CellInfo {
            elevation: 1234.4,
            crust_thickness: 35_250.0,
            age: 1180.0,
            sediment: 512.3,
            orogenic: 0.25,
            since_orogeny: 87.6,
            temperature: 12.34,
            temperature_january: -3.21,
            temperature_july: 25.0,
            precipitation: 0.8124,
            ice: 0.0,
            discharge: 3.15576e11,
            plate: 7,
            flags: CellInfo::CONTINENTAL,
        }
    }

    /// The value of a row, or `None` if it does not apply.
    fn row(rows: &[Row], label: &str) -> Option<String> {
        let row = rows
            .iter()
            .find(|row| row.label == label)
            .unwrap_or_else(|| panic!("no row {label}"));
        if row.applies {
            Some(row.value.clone())
        } else {
            assert_eq!(row.value, NOT_APPLICABLE);
            None
        }
    }

    #[test]
    fn latitude_and_longitude_of_directions() {
        let close =
            |(a, b): (f64, f64), (c, d): (f64, f64)| (a - c).abs() < 1e-9 && (b - d).abs() < 1e-9;
        assert!(close(latitude_longitude(DVec3::X), (0.0, 0.0)));
        assert!(close(latitude_longitude(DVec3::Y * 3.0), (0.0, 90.0)));
        assert!(close(latitude_longitude(-DVec3::Y), (0.0, -90.0)));
        assert!(close(latitude_longitude(DVec3::Z), (90.0, 0.0)));
        let (latitude, longitude) = latitude_longitude(DVec3::new(-1.0, -1.0, 2f64.sqrt()));
        assert!((latitude - 45.0).abs() < 1e-9);
        assert!((longitude + 135.0).abs() < 1e-9);
    }

    #[test]
    fn positions_have_hemispheres() {
        assert_eq!(format_position(12.34, 45.66), "12.3° N, 45.7° E");
        assert_eq!(format_position(-12.34, -45.66), "12.3° S, 45.7° W");
        assert_eq!(format_position(-0.04, -0.04), "0.0° N, 0.0° E");
        assert_eq!(format_position(0.0, -179.99), "0.0° N, 180.0° E");
    }

    #[test]
    fn land_cell_rows() {
        let rows = format_rows(&info(), 10.0, 20.0);
        assert_eq!(row(&rows, "Position").unwrap(), "10.0° N, 20.0° E");
        assert_eq!(row(&rows, "Elevation").unwrap(), "1234 m");
        assert_eq!(row(&rows, "Surface").unwrap(), "land");
        assert_eq!(row(&rows, "Plate").unwrap(), "7");
        assert_eq!(row(&rows, "Crust").unwrap(), "continental, 35.2 km");
        assert_eq!(row(&rows, "Lithosphere age").unwrap(), "old (≥ 1000 Myr)");
        assert_eq!(row(&rows, "Sediment").unwrap(), "512 m (as rock)");
        assert_eq!(row(&rows, "Orogeny").unwrap(), "25%, last 88 Myr ago");
        assert_eq!(row(&rows, "Temperature").unwrap(), "12.3 °C");
        assert_eq!(row(&rows, "Jan / Jul").unwrap(), "-3.2 °C / 25.0 °C");
        assert_eq!(row(&rows, "Precipitation").unwrap(), "812 mm/yr");
        assert_eq!(row(&rows, "Permanent ice"), None);
        // 3.15576e11 m³/yr = 10000 m³/s.
        assert_eq!(row(&rows, "River discharge").unwrap(), "10000 m³/s");
    }

    fn ocean() -> CellInfo {
        CellInfo {
            elevation: -4321.0,
            crust_thickness: 7000.0,
            age: 42.4,
            orogenic: 0.0,
            since_orogeny: f32::NAN,
            temperature: f32::NAN,
            temperature_january: f32::NAN,
            temperature_july: f32::NAN,
            ice: 0.75,
            flags: 0,
            ..info()
        }
    }

    #[test]
    fn ocean_cell_rows() {
        let rows = format_rows(&ocean(), -5.0, -170.0);
        assert_eq!(row(&rows, "Elevation").unwrap(), "-4321 m");
        assert_eq!(row(&rows, "Surface").unwrap(), "ocean, sea ice");
        assert_eq!(row(&rows, "Crust").unwrap(), "oceanic, 7.0 km");
        assert_eq!(
            row(&rows, "Lithosphere age").unwrap(),
            "42 Myr (ocean floor)"
        );
        assert_eq!(row(&rows, "Orogeny"), None);
        assert_eq!(row(&rows, "Temperature"), None);
        assert_eq!(row(&rows, "Jan / Jul"), None);
        assert_eq!(row(&rows, "Permanent ice").unwrap(), "75%");
        // No rivers in the ocean.
        assert_eq!(row(&rows, "River discharge"), None);
    }

    #[test]
    fn every_cell_has_the_same_rows() {
        let labels = |rows: Vec<Row>| rows.into_iter().map(|row| row.label).collect::<Vec<_>>();
        let ice_sheet = CellInfo {
            ice: 1.0,
            discharge: 0.0,
            ..info()
        };
        let bare_land = CellInfo {
            orogenic: 0.0,
            since_orogeny: f32::NAN,
            discharge: 0.0,
            ..info()
        };
        for cell in [info(), ocean(), ice_sheet, bare_land] {
            assert_eq!(labels(format_rows(&cell, 1.0, 2.0)), ROW_LABELS);
        }
        assert_eq!(labels(empty_rows()), ROW_LABELS);
        assert!(empty_rows().iter().all(|row| !row.applies));
    }

    #[test]
    fn summary_has_a_constant_length() {
        assert_eq!(
            format_summary(&info(), 10.0, -20.0),
            " 10.0° N, 20.0° W ·   1234 m"
        );
        let length = |text: String| text.chars().count();
        let longest = length(format_summary(
            &CellInfo {
                elevation: -10_999.0,
                ..info()
            },
            -89.95,
            -179.4,
        ));
        assert_eq!(length(format_summary(&info(), 1.0, 2.0)), longest);
        assert_eq!(length(format_summary(&info(), 10.0, -20.0)), longest);
    }

    #[test]
    fn cell_info_is_whole_words() {
        // The web version sends it as u32 words.
        assert_eq!(std::mem::size_of::<CellInfo>(), 56);
        assert_eq!(std::mem::align_of::<CellInfo>(), 4);
    }

    #[test]
    fn old_oceanic_crust_has_no_ocean_floor_age() {
        let info = CellInfo { flags: 0, ..info() };
        let rows = format_rows(&info, 0.0, 0.0);
        assert_eq!(row(&rows, "Lithosphere age").unwrap(), "old (≥ 1000 Myr)");
    }

    #[test]
    fn cell_infos_of_a_small_planet() {
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "2"]);
        let planet = crate::build_planet(&cli).unwrap();
        let simulation = planet.tectonics();
        let infos = cell_infos(&planet);
        assert_eq!(infos.len(), simulation.mesh().len());
        for (cell, info) in infos.iter().enumerate() {
            let cell = cell as u32;
            assert_eq!(info.plate, simulation.owner(cell).plate);
            assert_eq!(
                f64::from(info.elevation),
                simulation.surface_elevation(cell) as f32 as f64
            );
            assert_eq!(
                info.is_continental(),
                simulation.surface_crust(cell).is_continental()
            );
            assert!(info.temperature.is_finite());
            assert!(info.temperature_january.is_finite() && info.temperature_july.is_finite());
            assert!(info.precipitation.is_finite() && info.precipitation >= 0.0);
            assert!(info.crust_thickness > 0.0);
        }
    }

    #[test]
    fn significant_digits() {
        assert_eq!(significant(0.012345), "0.0123");
        assert_eq!(significant(1.2345), "1.23");
        assert_eq!(significant(123.45), "123");
        assert_eq!(significant(12345.0), "12300");
        assert_eq!(significant(0.0), "0");
    }
}
