//! The global state of the planet that the view shows in its own panel ([`PlanetPanel`], #209):
//! the ramp-up phase (#213), the Sun, the climate, the surface and the plates, as a few numbers
//! per state.
//!
//! The simulation computes a [`GlobalStatus`] with every state it sends, from the planet and
//! the cell attributes it sends anyway (read only: the simulation is not affected). The record
//! travels with the state (as bytes to the web page, [`GlobalStatus::to_bytes`]) and with every
//! frame of the history, uncompressed in its header (see `super::history`), so that the panel
//! shows the values of the frame the time bar shows, and the sparklines read them for every
//! frame without decoding any.
//!
//! Values that the model does not have yet (radiogenic heat production, the mantle's
//! temperature and heat flow, a changing day length) are not shown; see #208.

use eframe::egui;

use super::{CellInfo, PlateInfo};
use crate::planet::Planet;

/// The `f32` values of [`GlobalStatus::to_bytes`].
const FLOATS: usize = 19;

/// The global state of a state of the planet (see the module documentation). Means over cells
/// are area-weighted, since the cells of the mesh have nearly equal areas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalStatus {
    /// When the ramp-up phase ended (Myr); NaN while it runs.
    pub ramp_up_end: f32,
    /// Whether the phase ended because the tectonics settled (not at its longest duration).
    pub ramp_up_settled: bool,
    /// Whether the carbon cycle holds the CO₂ of the start (during the phase).
    pub co2_held: bool,
    /// Whether the carbon cycle runs at all (`--no-carbon-cycle` keeps the CO₂ of the start).
    pub carbon_cycle: bool,
    /// The solar constant of the climate (W/m²), raised by the Sun's brightening.
    pub solar_constant: f32,
    /// Annual mean surface temperature (°C): global, between 10°S and 10°N, and poleward of
    /// 70°, as in the snapshot log.
    pub temperature: f32,
    pub equator_temperature: f32,
    pub polar_temperature: f32,
    /// CO₂ relative to the pre-industrial 280 ppm, and its greenhouse forcing (W/m²), which
    /// the next climate update uses.
    pub co2: f32,
    pub forcing: f32,
    /// Share of the surface under permanent ice (cover above one half): sea ice, lake ice and
    /// ice sheets.
    pub ice: f32,
    /// Mean precipitation over land (m/yr).
    pub land_precipitation: f32,
    /// Share of the surface that is land: not the ocean as drawn ([`CellInfo::is_sea`]; closed
    /// depressions below sea level and lakes count as land, #58), and share of continental
    /// crust.
    pub land: f32,
    pub continental: f32,
    /// Change of the sea level since the start (m), against the dry elevations of the crust
    /// (see `Simulation::sea_level_change`).
    pub sea_level: f32,
    /// Area (km²) and volume (km³) of the lakes in the last erosion step; the volume includes
    /// sea water poured into basins below their sill (#168).
    pub lake_area: f32,
    pub lake_volume: f32,
    /// Plates that own cells, and those of them covering more than 1 % of the surface (as the
    /// log's count).
    pub plates: u16,
    pub large_plates: u16,
    /// Mean surface speed of the plates over all cells (cm/yr).
    pub plate_speed: f32,
    /// Water lost to space so far as a global layer (m), and the O₂ that the escape left in the
    /// air (partial pressure, bar); NaN without the escape (#210, #212).
    pub water_lost: f32,
    pub oxygen: f32,
    /// Estimated long-term seismic moment rate of the whole planet (N·m/yr) and magma supply
    /// (km³/yr, intrusions included), the sums of the cells' (#110).
    pub seismic_moment: f32,
    pub magma_supply: f32,
}

impl GlobalStatus {
    /// Size of [`GlobalStatus::to_bytes`].
    pub const BYTES: usize = 1 + FLOATS * 4 + 2 * 2;

    /// The global state of `planet`, whose cells and plates (as the view gets them) are
    /// `cells` and `plates`.
    pub fn of(planet: &Planet, cells: &[CellInfo], plates: &[PlateInfo]) -> Self {
        let (ramp_up_end, ramp_up_settled) = match planet.ramp_up_status() {
            crate::ramp_up::RampUpStatus::Running => (f64::NAN, false),
            crate::ramp_up::RampUpStatus::Ended { at, settled } => (at, settled),
        };
        let mesh = planet.tectonics().mesh();
        let count = cells.len().max(1) as f64;
        let mut sums = Sums::default();
        let mut plate_cells = vec![0usize; plates.len()];
        for (index, cell) in cells.iter().enumerate() {
            let direction = mesh.vertex(index as u32);
            let latitude = crate::math::asin(direction.z).to_degrees().abs();
            let temperature = f64::from(cell.temperature);
            sums.temperature += temperature;
            if latitude < 10.0 {
                sums.equator += (temperature, 1);
            } else if latitude > 70.0 {
                sums.poles += (temperature, 1);
            }
            sums.ice += usize::from(cell.ice > 0.5);
            if !cell.is_sea() {
                sums.land += (f64::from(cell.precipitation), 1);
            }
            sums.continental += usize::from(cell.is_continental());
            sums.seismic += f64::from(cell.seismicity);
            sums.magma += f64::from(cell.magma_supply);
            if let Some(plate) = plates.get(cell.plate as usize) {
                plate_cells[cell.plate as usize] += 1;
                let rotation = glam::Vec3::from(plate.rotation).as_dvec3();
                sums.speed += rotation.cross(direction).length();
            }
        }
        let carbon = planet.carbon_state();
        let hydrology = planet.hydrology();
        let cell_area = planet.cell_area();
        let large = 0.01 * count;
        // rad/Myr at the surface → cm/yr.
        let cm_per_year = planet.radius() * 100.0 / 1e6;
        let area = 4.0 * std::f64::consts::PI * planet.radius().powi(2);
        let escape = planet.escape_state();
        Self {
            ramp_up_end: ramp_up_end as f32,
            ramp_up_settled,
            co2_held: planet.holds_co2(),
            carbon_cycle: planet.carbon_cycle_enabled(),
            solar_constant: planet.temperature().solar_constant as f32,
            temperature: (sums.temperature / count) as f32,
            equator_temperature: sums.equator.mean(),
            polar_temperature: sums.poles.mean(),
            co2: carbon.ratio as f32,
            forcing: carbon.forcing as f32,
            ice: (sums.ice as f64 / count) as f32,
            land_precipitation: sums.land.mean(),
            land: (sums.land.1 as f64 / count) as f32,
            continental: (sums.continental as f64 / count) as f32,
            sea_level: planet.tectonics().sea_level_change() as f32,
            lake_area: (hydrology.lake_cells * cell_area / 1e6) as f32,
            lake_volume: (hydrology.lake_volume / 1e9) as f32,
            plates: plate_cells.iter().filter(|&&cells| cells > 0).count() as u16,
            large_plates: plate_cells
                .iter()
                .filter(|&&cells| cells as f64 > large)
                .count() as u16,
            plate_speed: (sums.speed / count * cm_per_year) as f32,
            water_lost: escape.map_or(f32::NAN, |escape| {
                (escape.water_lost / crate::carbon::SEA_WATER_DENSITY / area) as f32
            }),
            oxygen: escape.map_or(f32::NAN, |escape| escape.oxygen_pressure as f32),
            // N·m yr⁻¹ km⁻² × km²; mm/yr × km² = 10⁻⁶ km³/yr.
            seismic_moment: (sums.seismic * cell_area / 1e6) as f32,
            magma_supply: (sums.magma * cell_area / 1e6 * 1e-6) as f32,
        }
    }

    /// The record as [`GlobalStatus::BYTES`] bytes (little endian).
    pub fn to_bytes(self) -> [u8; Self::BYTES] {
        let mut bytes = [0; Self::BYTES];
        bytes[0] = u8::from(self.ramp_up_settled)
            | u8::from(self.co2_held) << 1
            | u8::from(self.carbon_cycle) << 2;
        let mut at = 1;
        for value in self.values() {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            at += 4;
        }
        for count in [self.plates, self.large_plates] {
            bytes[at..at + 2].copy_from_slice(&count.to_le_bytes());
            at += 2;
        }
        bytes
    }

    /// The record of [`GlobalStatus::to_bytes`]; `None` if `bytes` is of another size.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTES {
            return None;
        }
        let float = |index: usize| {
            let at = 1 + 4 * index;
            f32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"))
        };
        let count = |index: usize| {
            let at = 1 + 4 * FLOATS + 2 * index;
            u16::from_le_bytes(bytes[at..at + 2].try_into().expect("2 bytes"))
        };
        Some(Self {
            ramp_up_settled: bytes[0] & 1 != 0,
            co2_held: bytes[0] & 2 != 0,
            carbon_cycle: bytes[0] & 4 != 0,
            ramp_up_end: float(0),
            solar_constant: float(1),
            temperature: float(2),
            equator_temperature: float(3),
            polar_temperature: float(4),
            co2: float(5),
            forcing: float(6),
            ice: float(7),
            land_precipitation: float(8),
            land: float(9),
            continental: float(10),
            sea_level: float(11),
            lake_area: float(12),
            lake_volume: float(13),
            plate_speed: float(14),
            water_lost: float(15),
            oxygen: float(16),
            seismic_moment: float(17),
            magma_supply: float(18),
            plates: count(0),
            large_plates: count(1),
        })
    }

    /// The values of [`GlobalStatus::to_bytes`] in their order.
    fn values(&self) -> [f32; FLOATS] {
        [
            self.ramp_up_end,
            self.solar_constant,
            self.temperature,
            self.equator_temperature,
            self.polar_temperature,
            self.co2,
            self.forcing,
            self.ice,
            self.land_precipitation,
            self.land,
            self.continental,
            self.sea_level,
            self.lake_area,
            self.lake_volume,
            self.plate_speed,
            self.water_lost,
            self.oxygen,
            self.seismic_moment,
            self.magma_supply,
        ]
    }

    /// The line about the ramp-up phase, and a second, quieter line that explains it.
    pub fn ramp_up_lines(&self) -> (String, &'static str) {
        if self.ramp_up_end.is_nan() {
            let held = if self.co2_held { " — CO₂ held" } else { "" };
            (
                format!("Ramp-up (tectonics settling){held}"),
                "Until the plate count and the degassing are steady.",
            )
        } else if self.ramp_up_settled {
            (
                format!("Ramp-up ended at {:.0} Myr", self.ramp_up_end),
                "The tectonics had settled; the carbon cycle sets the CO₂ since.",
            )
        } else {
            (
                format!("Ramp-up ended at {:.0} Myr", self.ramp_up_end),
                "Its longest duration, before the tectonics settled.",
            )
        }
    }
}

/// The time and global state of the frames for the sparklines (oldest first), thinned to at most
/// [`SPARKLINE_POINTS`].
pub type Series = Vec<(f64, GlobalStatus)>;

/// Most points of a sparkline.
pub const SPARKLINE_POINTS: usize = 160;

/// Size of a sparkline in points.
const SPARKLINE_SIZE: egui::Vec2 = egui::vec2(64.0, 16.0);

/// What the planet panel shows: the global state of the displayed state, when that is (Myr)
/// and the run's epoch, and the series of the sparklines.
pub struct PlanetView<'a> {
    pub global: &'a GlobalStatus,
    pub time: f64,
    pub epoch: f64,
    pub series: &'a Series,
}

/// Screens narrower than this (in points) start with the panel hidden, as the other panels.
const NARROW_SCREEN: f32 = 600.0;
/// Largest width of the panel in points: room for a label, a value and a sparkline.
const MAX_WIDTH: f32 = 360.0;
/// Narrower than this many times the size of body text, the sparklines get rows of their own
/// (a label, a value and a sparkline need about that much).
const SPARKLINES_BESIDE: f32 = 25.0;
/// Distance of the panel and its button from the edges of the window, in points.
const MARGIN: f32 = 8.0;

/// What the planet panel shows about the running simulation: its state, not its inputs (those
/// are in the parameter panel).
pub struct SimulationStatus<'a> {
    /// Simulated time of the newest state, if there is one yet.
    pub time: Option<f64>,
    /// Simulated duration of the running simulation in Myr.
    pub duration: f64,
    /// Why the simulation failed, if it did.
    pub failure: Option<&'a str>,
    /// What the simulation does until its first update (see `banner::starting_text`).
    pub starting: &'a str,
    /// The median time per step in seconds, once known.
    pub step_seconds: Option<f64>,
}

/// The panel of the simulation's state in the bottom right corner of the view, above the time
/// bar (#209): the time of the run and its pace (or why it failed), the step-time hint, the
/// ramp-up phase (#213) and the global statistics of the displayed state. Like the cell widget
/// it can be hidden, and starts hidden on narrow screens; hidden, a button with the time
/// remains, with the step-time hint above it, so that nothing is lost on a phone.
#[derive(Default)]
pub struct PlanetPanel {
    /// Whether the panel is shown; decided by the screen width on the first frame.
    open: Option<bool>,
}

impl PlanetPanel {
    /// Shows the panel or its button, `above` points above the bottom edge of the window (the
    /// time bar); the panel also above `cell` (the area of the cell widget in the bottom left
    /// corner) where the two would overlap (phones). `view` is `None` before the first state.
    /// `hint` draws the step-time hint, which is shown in either case. Returns the area it
    /// covers.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        status: &SimulationStatus,
        view: Option<&PlanetView>,
        above: f32,
        cell: Option<egui::Rect>,
        hint: Option<impl FnOnce(&mut egui::Ui)>,
    ) -> egui::Rect {
        let screen = ctx.content_rect();
        let open = *self.open.get_or_insert(screen.width() >= NARROW_SCREEN);
        let width = (screen.width() - 4.0 * MARGIN).clamp(100.0, MAX_WIDTH);
        if !open {
            let state = match (status.failure, status.time) {
                (Some(_), _) => "failed".to_owned(),
                (None, Some(time)) => format!("{time:.0} Myr"),
                (None, None) => "starting".to_owned(),
            };
            let ramp_up = view.is_some_and(|view| view.global.ramp_up_end.is_nan());
            let text = format!(
                "Planet · {state}{}",
                if ramp_up { " (ramp-up)" } else { "" }
            );
            return egui::Area::new(egui::Id::unique("planet panel button"))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-MARGIN, -MARGIN - above])
                .show(ctx, |ui| {
                    ui.set_max_width(width);
                    // The hint above the button, both right-aligned in the corner.
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        if let Some(hint) = hint {
                            hint(ui);
                        }
                        if ui
                            .button(text)
                            .on_hover_text("Show the state of the simulation and the planet")
                            .clicked()
                        {
                            self.open = Some(true);
                        }
                    });
                })
                .response
                .rect;
        }
        // Above the cell widget if they would overlap (the button is narrow enough beside it).
        let above = match cell {
            Some(cell) if cell.right() + MARGIN > screen.right() - MARGIN - width => {
                above.max(screen.bottom() - cell.top() + MARGIN)
            }
            _ => above,
        };
        let mut open = true;
        let response = egui::Window::new("Simulation and planet")
            .id(egui::Id::unique("planet panel"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-MARGIN, -MARGIN - above])
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            // A fixed width, so that values of other lengths do not resize it.
            .min_width(width)
            .max_width(width)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(((screen.height() - above) * 0.6).max(100.0))
                    .show(ui, |ui| {
                        run_status(ui, status);
                        if let Some(hint) = hint {
                            hint(ui);
                        }
                        if let Some(view) = view {
                            let (line, detail) = view.global.ramp_up_lines();
                            ui.label(line);
                            ui.label(egui::RichText::new(detail).weak());
                            ui.separator();
                            let body = egui::TextStyle::Body.resolve(ui.style()).size;
                            planet_rows(ui, view, width < SPARKLINES_BESIDE * body);
                        }
                    });
            });
        self.open = Some(open);
        response.map_or(egui::Rect::NOTHING, |response| response.response.rect)
    }
}

/// The time of the run and its pace, why it failed, or what it does before its first state.
fn run_status(ui: &mut egui::Ui, status: &SimulationStatus) {
    match (status.failure, status.time) {
        (Some(failure), _) => {
            ui.colored_label(ui.visuals().error_fg_color, "Simulation failed:");
            ui.colored_label(ui.visuals().error_fg_color, failure);
        }
        (None, Some(time)) => {
            let pace = status
                .step_seconds
                .map(|seconds| format!(" · {seconds:.2} s per step"))
                .unwrap_or_default();
            ui.label(format!("t = {time:.0} of {:.0} Myr{pace}", status.duration));
        }
        (None, None) => {
            ui.label(status.starting);
        }
    }
}

/// A value for the panel: `—` for NaN.
fn value(value: f32, format: impl FnOnce(f32) -> String) -> String {
    if value.is_finite() {
        format(value)
    } else {
        super::cell_info::NOT_APPLICABLE.to_owned()
    }
}

/// The rows of the planet panel: the epoch and the Sun, the climate, the surface and the
/// plates, each value with its unit; sparklines for the mean temperature, the CO₂ and the sea
/// level over the recorded frames, with a mark at the displayed one.
///
/// On a `narrow` panel the sparklines get rows of their own, under their values.
fn planet_rows(ui: &mut egui::Ui, view: &PlanetView, narrow: bool) {
    let global = view.global;
    let sun = crate::sun::Sun::of_run(view.epoch, view.time);
    let epoch = view.epoch + view.time / 1000.0;
    // Beside the value, or on a narrow screen under it, so that the panel fits.
    let spark = |ui: &mut egui::Ui, of: fn(&GlobalStatus) -> f32| {
        if narrow {
            ui.end_row();
            ui.label("");
        }
        sparkline(ui, view.series, view.time, of);
    };
    let group = |ui: &mut egui::Ui, title: &str| {
        ui.label(egui::RichText::new(title).strong());
        ui.end_row();
    };
    egui::Grid::new("planet rows")
        .num_columns(if narrow { 2 } else { 3 })
        .striped(true)
        .show(ui, |ui| {
            group(ui, "Time");
            row(
                ui,
                "Epoch",
                format!("{epoch:.3} Gyr"),
                "Gyr after the earliest start of a run (epoch 4 is the present day of the solar \
                 model; --epoch sets the start).",
            );
            ui.end_row();
            row(
                ui,
                "Star",
                format!("{:.0}% of today's Sun", 100.0 * sun.luminosity),
                "Luminosity of the Sun-like star at its age: it brightens on the main sequence.",
            );
            ui.end_row();
            row(
                ui,
                "Solar constant",
                value(global.solar_constant, |s| format!("{s:.0} W/m²")),
                "The sunlight at the planet's mean distance that the climate uses.",
            );
            ui.end_row();
            row(
                ui,
                "Day",
                "24 h (fixed)".to_owned(),
                "The rotation of the planet does not change yet (tidal braking: #208).",
            );
            ui.end_row();

            group(ui, "Climate");
            row(
                ui,
                "Mean temperature",
                value(global.temperature, |t| format!("{t:.1} °C")),
                "Annual mean surface temperature of the whole planet.",
            );
            spark(ui, |g| g.temperature);
            ui.end_row();
            row(
                ui,
                "Equator / poles",
                format!(
                    "{} / {}",
                    value(global.equator_temperature, |t| format!("{t:.1}")),
                    value(global.polar_temperature, |t| format!("{t:.1} °C"))
                ),
                "Annual mean surface temperature between 10°S and 10°N, and poleward of 70°.",
            );
            ui.end_row();
            // Held during the ramp-up phase: its line above says so.
            let co2_note = if global.carbon_cycle { "" } else { " (fixed)" };
            row(
                ui,
                "CO₂",
                value(global.co2, |ratio| format!("{}{co2_note}", co2(ratio))),
                "CO₂ of the atmosphere as a partial pressure, and relative to the pre-industrial \
                 280 ppm (PAL): set by the carbon cycle, held during the ramp-up phase, fixed \
                 without the carbon cycle. A young planet under a fainter Sun needs far more.",
            );
            spark(ui, |g| g.co2);
            ui.end_row();
            row(
                ui,
                "Greenhouse forcing",
                value(global.forcing, |f| format!("{f:+.2} W/m²")),
                "Forcing of that CO₂ relative to 280 ppm, which the next climate update uses.",
            );
            ui.end_row();
            row(
                ui,
                "Ice cover",
                value(global.ice, |ice| format!("{:.1}%", 100.0 * ice)),
                "Share of the surface under permanent ice: sea ice, lake ice and ice sheets.",
            );
            ui.end_row();
            row(
                ui,
                "Land precipitation",
                value(global.land_precipitation, |p| format!("{p:.2} m/yr")),
                "Mean precipitation over land.",
            );
            ui.end_row();

            row(
                ui,
                "Water lost",
                value(global.water_lost, |depth| format!("{depth:.2} m layer")),
                "Water lost to space so far (hydrogen escape), as a layer over the whole \
                 planet; — without the escape.",
            );
            ui.end_row();
            row(
                ui,
                "O₂",
                value(global.oxygen, |pressure| {
                    if pressure > 0.0 {
                        format!("{pressure:.1e} bar")
                    } else {
                        "none".to_owned()
                    }
                }),
                "Oxygen the escape left in the air (the sinks take most of it); — without the \
                 escape. The biosphere's oxygen is not modelled.",
            );
            ui.end_row();

            group(ui, "Surface");
            row(
                ui,
                "Land",
                value(global.land, |land| format!("{:.1}%", 100.0 * land)),
                "Share of the surface that is not ocean (lakes and dry basins below sea level \
                 count as land).",
            );
            ui.end_row();
            row(
                ui,
                "Continental crust",
                value(global.continental, |c| format!("{:.1}%", 100.0 * c)),
                "Share of the surface with continental crust, flooded shelves included: made by \
                 the tectonics (arcs, collisions) and recycled at trenches.",
            );
            ui.end_row();
            row(
                ui,
                "Sea level",
                value(global.sea_level, |level| {
                    format!("{level:+.0} m since start")
                }),
                "Change of the sea surface against the land (the dry elevations of the crust) \
                 since the start of the run.",
            );
            spark(ui, |g| g.sea_level);
            ui.end_row();
            row(
                ui,
                "Lakes",
                format!(
                    "{} · {}",
                    value(global.lake_area, |area| quantity(area, "km²")),
                    value(global.lake_volume, |volume| quantity(volume, "km³"))
                ),
                "Area and volume of the lakes (sea water poured into basins below their sill \
                 included).",
            );
            ui.end_row();

            group(ui, "Plates");
            row(
                ui,
                "Plates",
                format!("{} ({} > 1%)", global.plates, global.large_plates),
                "Plates owning cells, and those of them covering more than 1% of the surface.",
            );
            ui.end_row();
            row(
                ui,
                "Mean speed",
                value(global.plate_speed, |speed| format!("{speed:.1} cm/yr")),
                "Mean surface speed of the plates over the whole surface.",
            );
            ui.end_row();
            row(
                ui,
                "Seismic moment",
                value(global.seismic_moment, |moment| {
                    let exponent = moment.max(1.0).log10().floor();
                    format!(
                        "{:.1}·{} N·m/yr",
                        moment / 10f32.powf(exponent),
                        super::modes::power_of_ten(exponent as i32)
                    )
                }),
                "Estimated long-term seismic moment rate of the planet, from the plate \
                 boundaries and the deforming lithosphere (see the Tectonic activity mode). \
                 Earth: about 10²² N·m/yr, mostly at subduction zones.",
            );
            spark(ui, |g| g.seismic_moment);
            ui.end_row();
            row(
                ui,
                "Magma supply",
                value(global.magma_supply, |supply| format!("{supply:.1} km³/yr")),
                "Estimated long-term magma supply of ridges, arcs, hotspots and rifts, \
                 intrusions included (see the Tectonic activity mode). Earth: about 25–30 \
                 km³/yr, three quarters of it at the ridges.",
            );
            spark(ui, |g| g.magma_supply);
            ui.end_row();
        });
}

/// CO₂ from its ratio to the pre-industrial level: in ppm up to a few times that, as a partial
/// pressure in bar above.
fn co2(ratio: f32) -> String {
    let ppm = 280.0 * ratio;
    if ratio < 10.0 {
        format!("{ppm:.0} ppm ({ratio:.2}× PAL)")
    } else {
        format!(
            "{:.3} bar ({ratio:.0}× PAL)",
            f64::from(ratio) * crate::carbon::REFERENCE_PRESSURE
        )
    }
}

/// A non-negative quantity with a unit, in thousands or millions where large.
fn quantity(value: f32, unit: &str) -> String {
    if value >= 1e6 {
        format!("{:.2} M {unit}", value / 1e6)
    } else if value >= 1e4 {
        format!("{:.0}k {unit}", value / 1e3)
    } else {
        format!("{value:.0} {unit}")
    }
}

/// A label and its value, with an explanation on hover.
fn row(ui: &mut egui::Ui, label: &str, value: String, help: &str) {
    ui.label(label).on_hover_text(help);
    ui.label(value).on_hover_text(help);
}

/// A small line of a value over the recorded frames, with a dot at the frame at `time` (the
/// displayed one); nothing with fewer than two points.
fn sparkline(ui: &mut egui::Ui, series: &Series, time: f64, of: fn(&GlobalStatus) -> f32) {
    let points: Vec<(f64, f32)> = series
        .iter()
        .map(|(time, global)| (*time, of(global)))
        .filter(|(_, value)| value.is_finite())
        .collect();
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return;
    };
    if points.len() < 2 || last.0 <= first.0 {
        return;
    }
    let (low, high) = points
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), (_, v)| {
            (low.min(*v), high.max(*v))
        });
    let (rect, response) = ui.allocate_exact_size(SPARKLINE_SIZE, egui::Sense::hover());
    let span = (high - low).max(f32::EPSILON);
    let position = |(t, v): (f64, f32)| {
        egui::pos2(
            rect.left() + rect.width() * ((t - first.0) / (last.0 - first.0)) as f32,
            rect.bottom() - rect.height() * (v - low) / span,
        )
    };
    let stroke = egui::Stroke::new(1.2, ui.visuals().widgets.noninteractive.fg_stroke.color);
    let painter = ui.painter_at(rect.expand(3.0));
    painter.add(egui::Shape::line(
        points.iter().copied().map(position).collect(),
        stroke,
    ));
    // The point nearest to the displayed time.
    if let Some(&shown) = points
        .iter()
        .min_by(|a, b| (a.0 - time).abs().total_cmp(&(b.0 - time).abs()))
    {
        painter.circle_filled(position(shown), 2.5, ui.visuals().selection.stroke.color);
    }
    response.on_hover_text(format!(
        "{low:.2} to {high:.2} over {:.0} to {:.0} Myr",
        first.0, last.0
    ));
}

/// The frames' global states thinned to at most [`SPARKLINE_POINTS`] (evenly by frame), with
/// `latest` at the end if it is newer than the last frame (a full history).
pub fn thin<'a>(
    frames: impl ExactSizeIterator<Item = (f64, &'a GlobalStatus)>,
    latest: Option<(f64, GlobalStatus)>,
) -> Series {
    let count = frames.len();
    let stride = count.div_ceil(SPARKLINE_POINTS).max(1);
    let mut series: Series = frames
        .enumerate()
        .filter(|(index, _)| index % stride == 0 || index + 1 == count)
        .map(|(_, (time, global))| (time, *global))
        .collect();
    if let Some(latest) = latest
        && series.last().is_none_or(|last| latest.0 > last.0)
    {
        series.push(latest);
    }
    series
}

/// Sums over the cells for [`GlobalStatus::of`].
#[derive(Default)]
struct Sums {
    temperature: f64,
    equator: Mean,
    poles: Mean,
    /// Precipitation and count of the land cells.
    land: Mean,
    ice: usize,
    continental: usize,
    speed: f64,
    /// Seismic moment rate (N·m yr⁻¹ km⁻²) and magma supply (mm/yr) summed over the cells.
    seismic: f64,
    magma: f64,
}

/// A sum and a count.
#[derive(Default, Clone, Copy)]
struct Mean(f64, usize);

impl std::ops::AddAssign<(f64, usize)> for Mean {
    fn add_assign(&mut self, (value, count): (f64, usize)) {
        self.0 += value;
        self.1 += count;
    }
}

impl Mean {
    /// The mean; NaN without values.
    fn mean(self) -> f32 {
        if self.1 == 0 {
            f32::NAN
        } else {
            (self.0 / self.1 as f64) as f32
        }
    }
}

#[cfg(test)]
pub(super) fn example() -> GlobalStatus {
    GlobalStatus {
        ramp_up_end: f32::NAN,
        ramp_up_settled: false,
        co2_held: true,
        carbon_cycle: true,
        solar_constant: 1052.5,
        temperature: 14.2,
        equator_temperature: 27.1,
        polar_temperature: -21.4,
        co2: 1.3,
        forcing: 1.4,
        ice: 0.08,
        land_precipitation: 0.71,
        land: 0.29,
        continental: 0.38,
        sea_level: -12.5,
        lake_area: 1.9e6,
        lake_volume: 1.2e5,
        plates: 23,
        large_plates: 14,
        plate_speed: 4.1,
        water_lost: 0.8,
        oxygen: 2e-9,
        seismic_moment: 8.5e21,
        magma_supply: 21.3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bytes_give_the_record_back() {
        let status = example();
        let back = GlobalStatus::from_bytes(&status.to_bytes()).unwrap();
        assert!(back.ramp_up_end.is_nan());
        assert_eq!(
            GlobalStatus {
                ramp_up_end: 0.0,
                ..back
            },
            GlobalStatus {
                ramp_up_end: 0.0,
                ..status
            }
        );
        let ended = GlobalStatus {
            ramp_up_end: 42.0,
            ramp_up_settled: true,
            co2_held: false,
            carbon_cycle: false,
            ..status
        };
        assert_eq!(GlobalStatus::from_bytes(&ended.to_bytes()), Some(ended));
        assert_eq!(GlobalStatus::from_bytes(&[0; 3]), None);
    }

    #[test]
    fn large_values_are_shown_in_larger_units() {
        assert_eq!(quantity(1.9e6, "km²"), "1.90 M km²");
        assert_eq!(quantity(1.2e5, "km³"), "120k km³");
        assert_eq!(quantity(350.0, "km³"), "350 km³");
        assert_eq!(co2(1.0), "280 ppm (1.00× PAL)");
        assert!(co2(1760.0).ends_with("bar (1760× PAL)"), "{}", co2(1760.0));
    }

    #[test]
    fn the_series_is_thinned_and_ends_with_the_newest_state() {
        let frames: Vec<(f64, GlobalStatus)> =
            (0..1000).map(|step| (f64::from(step), example())).collect();
        let series = thin(frames.iter().map(|(time, global)| (*time, global)), None);
        assert!(series.len() <= SPARKLINE_POINTS + 1);
        assert_eq!(series.first().unwrap().0, 0.0);
        assert_eq!(series.last().unwrap().0, 999.0);
        let latest = Some((1500.0, example()));
        let series = thin(frames.iter().map(|(time, global)| (*time, global)), latest);
        assert_eq!(series.last().unwrap().0, 1500.0);
        // A latest state that is the last frame is not repeated.
        let series = thin(
            frames.iter().map(|(time, global)| (*time, global)),
            Some((999.0, example())),
        );
        assert_eq!(series.iter().filter(|(time, _)| *time == 999.0).count(), 1);
    }
}
