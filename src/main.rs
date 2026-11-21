// On the web, the snapshot images and dumps are not written, so their code is unused there; the
// native build still checks for dead code.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

mod climate;
mod erosion;
mod isostasy;
mod math;
mod ocean;
mod planet;
mod presets;
#[cfg(any(test, target_arch = "wasm32"))]
mod query;
mod render;
mod sparse;
mod sphere;
mod tectonics;
mod temperature;
mod viewer;
#[cfg(target_arch = "wasm32")]
mod web;

use std::ops::ControlFlow;
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use web_time::Instant;

use anyhow::{Context, ensure};
use clap::Parser;
#[cfg(not(target_arch = "wasm32"))]
use tracing::{debug, error};
use tracing::{info, warn};
#[cfg(not(target_arch = "wasm32"))]
use tracing_subscriber::EnvFilter;

use crate::erosion::WaveErosion;
use crate::planet::{CoverTurnover, Planet, PlanetStatistics};
use crate::presets::{Biosphere, Epoch, Parameters};
use crate::sphere::SphereMesh;
use crate::tectonics::{
    InitialConfiguration, MassLedger, MaterialBudget, SeaLevelBudget, Simulation, TrenchSediment,
};

/// The planet radius in metres; used for erosion, climate and to report linear speeds.
const EARTH_RADIUS: f64 = 6.371e6;

/// Simulates a planet with plate tectonics, an Earth-like water cycle and erosion, and writes
/// map snapshots as PNG images.
///
/// Log output is controlled with the `RUST_LOG` environment variable (default: `info`), e.g.
/// `RUST_LOG=debug` shows per-step timings.
#[derive(Parser, Debug, Clone)]
#[command(version, about)]
struct Cli {
    /// Seed for the random number generator.
    #[arg(long, default_value_t = 0)]
    seed: u64,

    /// Icosphere subdivisions. The planet has 10 * 4^n + 2 cells.
    #[arg(long, default_value_t = DEFAULT_SUBDIVISIONS)]
    subdivisions: u32,

    /// Number of tectonic plates.
    #[arg(long, default_value_t = 12)]
    plates: usize,

    /// Fraction of the surface initially covered by continental crust.
    #[arg(long, default_value_t = 0.4)]
    continental_fraction: f64,

    /// Initial arrangement of the continents.
    #[arg(long, value_enum, default_value_t = InitialConfiguration::Dispersed)]
    initial: InitialConfiguration,

    /// Simulated duration in Myr.
    #[arg(long, default_value_t = 500.0)]
    duration: f64,

    /// Time step in Myr. Steps are split into substeps automatically if plates move too fast.
    #[arg(long, default_value_t = 1.0)]
    dt: f64,

    /// Interval between snapshots in Myr.
    #[arg(long, default_value_t = 50.0)]
    snapshot_interval: f64,

    /// Directory to write snapshots into.
    #[arg(long, default_value = "output")]
    output: PathBuf,

    /// Width of the snapshot images in pixels.
    #[arg(long, default_value_t = 1024)]
    width: u32,

    /// Disable erosion.
    #[arg(long)]
    no_erosion: bool,

    /// Start with a bare planet instead of an initial sediment cover.
    #[arg(long)]
    no_initial_sediment: bool,

    /// Minimum discharge in m³/yr for a cell to be drawn as a river.
    #[arg(long, default_value_t = 1e11)]
    river_threshold: f64,

    /// Check the incremental plate matching against a full scan (slow; counts mismatches).
    #[arg(long)]
    verify_matching: bool,

    /// Run without the 3D view and only write snapshot images.
    #[arg(long)]
    headless: bool,

    /// Vertical exaggeration of the relief in the 3D view.
    #[arg(long, default_value_t = 40.0)]
    exaggeration: f64,

    /// Measure the budget of felsic crust and sediment per step phase (slower).
    #[arg(long)]
    material_budget: bool,

    /// Time (Myr) from which the continental sedimentary cover is tracked as a cohort, to log
    /// how long it is preserved (its half-life).
    #[arg(long, default_value_t = 500.0)]
    cover_cohort: f64,

    /// Write the state of every cell (elevation, sediment, crust) as CSV with each snapshot,
    /// for analysis.
    #[arg(long)]
    cell_dump: bool,

    /// Life on the planet, as far as the parameters see it: `modern` is today's vegetated Earth
    /// with a biogenic ocean, `abiotic` a planet without life (bare land, no transpiration,
    /// braided rivers, no biogenic pelagic sediment). See `presets::Biosphere`.
    #[arg(long, value_enum, default_value_t = Biosphere::Modern)]
    biosphere: Biosphere,

    /// Time of the planet's history in Ga before present (0 is today, 3.5 the Archean): sets the
    /// solar constant of the fainter young Sun and, unless `--co2` is given, a greenhouse forcing
    /// that offsets it. See `presets::Epoch`.
    #[arg(long, default_value_t = 0.0)]
    epoch: f64,

    /// CO₂ relative to the pre-industrial 280 ppm; the greenhouse forcing becomes
    /// 5.35 ln(ratio) W/m² instead of the default of the epoch.
    #[arg(long)]
    co2: Option<f64>,

    /// Overrides a tectonic parameter for calibration, e.g. `--set basal_drag=8e14`, or an
    /// erosion, climate or temperature parameter with the prefix `erosion.`, `climate.` or
    /// `temperature.`; can be repeated. See `TectonicParams::set`, `ErosionParams::set`,
    /// `ClimateParams::set` and `TemperatureParams::set` for the names. Overrides are applied
    /// after the presets (`--biosphere`, `--epoch`, `--co2`).
    #[arg(long = "set", value_name = "NAME=VALUE")]
    overrides: Vec<String>,

    /// Compute sin, exp, ln, atan2, … exactly as the web version does (the `libm` crate) instead
    /// of with the platform's math library, so that the same options give bit for bit the same
    /// planet as on the web and on every other platform with this flag. To reproduce a web link,
    /// also give `--subdivisions 5`, the web's default, unless the link sets it. Runs at about the
    /// same speed. Always on in the web version. See `src/math.rs`.
    #[arg(long)]
    web_math: bool,
}

/// Default icosphere subdivisions: 6 on the desktop, 5 on the web, where the simulation runs about
/// 2.4× slower (~0.4 s per step at 5) and 6 would take more than a second per step.
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_SUBDIVISIONS: u32 = 6;
#[cfg(target_arch = "wasm32")]
const DEFAULT_SUBDIVISIONS: u32 = 5;

/// Logs to stderr, filtered by `RUST_LOG` (default: `info`). Also captures `log` records of
/// dependencies.
#[cfg(not(target_arch = "wasm32"))]
fn init_logging() {
    use std::io::IsTerminal;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();
}

const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_logging();
    let planet = build_planet(&cli)?;
    std::fs::create_dir_all(&cli.output)
        .with_context(|| format!("creating output directory {}", cli.output.display()))?;

    if cli.headless {
        return simulate(planet, &cli, |_| ControlFlow::Continue(()));
    }

    // The parameter panel starts from the command line as given.
    let args = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    viewer::run(
        viewer::Launch { cli, args },
        simulation_thread_launcher(planet),
    )
}

/// Starts the simulations of the 3D view on a thread each: the first one with the planet built
/// at start-up, later ones (restarts from the parameter panel) with a new planet. A restarted
/// simulation first waits for the previous thread to end, so only one simulation runs at a time.
/// The previous simulation stops after its current step, because the view no longer receives
/// its updates.
#[cfg(not(target_arch = "wasm32"))]
fn simulation_thread_launcher(planet: Planet) -> viewer::StartSimulation {
    use viewer::SimulationEvent;

    let mut initial_planet = Some(planet);
    // The thread of the latest simulation and its cancellation flag.
    let mut previous: Option<(std::thread::JoinHandle<()>, Arc<AtomicBool>)> = None;
    Box::new(move |launch, events, wake_up| {
        let planet = initial_planet.take();
        let previous_thread = previous.take().map(|(thread, cancelled)| {
            cancelled.store(true, Ordering::Relaxed);
            thread
        });
        let cancelled = Arc::new(AtomicBool::new(false));
        let thread = {
            let cancelled = cancelled.clone();
            std::thread::spawn(move || {
                if let Some(thread) = previous_thread {
                    // A panic was reported when it happened.
                    let _ = thread.join();
                }
                // Replaced already while waiting (quick repeated restarts).
                if cancelled.load(Ordering::Relaxed) {
                    return;
                }
                let report = |message: String| {
                    error!("Simulation failed: {message}");
                    if events.send(SimulationEvent::Failed(message)).is_ok() {
                        wake_up();
                    }
                };
                let run = std::panic::AssertUnwindSafe(|| {
                    run_simulation_thread(planet, &launch.cli, &cancelled, &events, &*wake_up)
                });
                match std::panic::catch_unwind(run) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => report(format!("{error:#}")),
                    Err(panic) => {
                        let message = panic
                            .downcast_ref::<&str>()
                            .map(|message| (*message).to_owned())
                            .or_else(|| panic.downcast_ref::<String>().cloned())
                            .unwrap_or_default();
                        report(format!("the simulation panicked: {message}"));
                    }
                }
            })
        };
        previous = Some((thread, cancelled));
    })
}

/// Runs one simulation of the 3D view: with the `planet` built at start-up, or with a new one.
/// It stops early once `cancelled` is set or the view no longer receives its updates.
#[cfg(not(target_arch = "wasm32"))]
fn run_simulation_thread(
    planet: Option<Planet>,
    cli: &Cli,
    cancelled: &AtomicBool,
    events: &std::sync::mpsc::Sender<viewer::SimulationEvent>,
    wake_up: &dyn Fn(),
) -> anyhow::Result<()> {
    use viewer::{SimulationEvent, SurfaceUpdate};

    let planet = match planet {
        Some(planet) => planet,
        None => {
            std::fs::create_dir_all(&cli.output)
                .with_context(|| format!("creating output directory {}", cli.output.display()))?;
            build_planet(cli)?
        }
    };
    let mut first_update = true;
    simulate(planet, cli, |planet: &Planet| {
        if cancelled.load(Ordering::Relaxed) {
            return ControlFlow::Break(());
        }
        let mesh = planet.tectonics().mesh();
        let start = Instant::now();
        let vertices = viewer::surface_vertices(planet, cli.exaggeration, cli.river_threshold);
        let vertices_built = start.elapsed();
        let cells = viewer::cell_infos(planet);
        debug!(
            ?vertices_built,
            cells_built = ?start.elapsed() - vertices_built,
            "Surface update built"
        );
        let update = SurfaceUpdate {
            time: planet.tectonics().time(),
            vertices: Arc::new(vertices),
            cells: Arc::new(cells),
            // The triangles only with the first update of a simulation.
            faces: std::mem::take(&mut first_update).then(|| Arc::new(mesh.faces().to_vec())),
        };
        // Sending fails once the view started another simulation or was closed.
        if events.send(SimulationEvent::Surface(update)).is_err() {
            return ControlFlow::Break(());
        }
        wake_up();
        ControlFlow::Continue(())
    })
}

/// On the web, the same binary is the page (3D view) and the simulation worker; see [`web`].
#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

impl Cli {
    /// Checks the options that clap does not check itself.
    fn check(&self) -> anyhow::Result<()> {
        ensure!(
            self.subdivisions <= 8,
            "at most 8 subdivisions are supported"
        );
        ensure!(self.plates >= 1, "there must be at least one plate");
        let cells = 10 * 4usize.pow(self.subdivisions) + 2;
        ensure!(
            self.plates <= cells,
            "there can be at most as many plates as cells ({cells} at {} subdivisions)",
            self.subdivisions
        );
        ensure!(self.dt > 0.0, "the time step must be positive");
        ensure!(self.duration >= 0.0, "the duration must not be negative");
        ensure!(self.width >= 2, "the image width must be at least 2");
        Ok(())
    }

    /// The parameters of the run: the presets, then the options, then the overrides
    /// (`--set`), which are also returned in order.
    fn parameters(&self) -> anyhow::Result<(Parameters, Vec<(&str, f64)>)> {
        let epoch = Epoch::new(self.epoch, self.co2)?;
        let mut parameters = Parameters::new(EARTH_RADIUS, self.biosphere, epoch);
        let params = &mut parameters.tectonics;
        params.continental_fraction = self.continental_fraction;
        params.initial = self.initial;
        params.matching.verify = self.verify_matching;
        params.initial_sediment.enabled = !self.no_initial_sediment;
        params.track_material_budget = self.material_budget;
        let mut overrides = Vec::with_capacity(self.overrides.len());
        for assignment in &self.overrides {
            let (name, value) = assignment
                .split_once('=')
                .with_context(|| format!("expected NAME=VALUE, got {assignment}"))?;
            let value: f64 = value
                .parse()
                .with_context(|| format!("parsing the value of {name}"))?;
            parameters.set(name, value)?;
            overrides.push((name, value));
        }
        Ok((parameters, overrides))
    }

    /// Checks everything [`build_planet`] checks, without building the planet; used by the
    /// parameter panel of the 3D view before a restart.
    fn validate(&self) -> anyhow::Result<()> {
        self.check()?;
        self.parameters().map(|_| ())
    }
}

/// Checks the parameters and creates the initial planet.
fn build_planet(cli: &Cli) -> anyhow::Result<Planet> {
    cli.check()?;
    // Before anything is computed; only one simulation runs at a time.
    math::set_web_math(cli.web_math);
    if math::web_math() {
        info!("Using the math functions of the web version (`libm`)");
    }

    let mesh = SphereMesh::icosphere(cli.subdivisions);
    info!(cells = mesh.len(), "Created planet mesh");
    let (parameters, overrides) = cli.parameters()?;
    if cli.co2.is_some_and(|co2| co2 > 30.0) {
        warn!(
            "the logarithmic CO₂ forcing (Myhre et al., 1998) holds up to a few tens of times the \
             pre-industrial CO₂; above that it underestimates the forcing (Byrne & Goldblatt, 2014)"
        );
    }
    for (name, value) in overrides {
        info!(name, value, "Overrode parameter");
    }
    info!(
        biosphere = ?cli.biosphere,
        epoch_ga = cli.epoch,
        solar_constant = parameters.temperature.orbit.solar_constant,
        greenhouse_forcing = parameters.temperature.greenhouse_forcing,
        "Planet presets (after the overrides; `--set temperature.solar_constant` does not \
         change the greenhouse forcing)"
    );
    let Parameters {
        tectonics: params,
        erosion,
        climate,
        temperature,
    } = parameters;
    let simulation = Simulation::new(mesh, cli.plates, params, cli.seed);
    let erosion = (!cli.no_erosion).then_some(erosion);
    let mut planet = Planet::new(simulation, climate, temperature, erosion, EARTH_RADIUS);
    planet.track_cover_cohort_from(cli.cover_cohort);
    Ok(planet)
}

/// Runs the simulation, writing snapshots and calling `on_step` with the initial state and after
/// every step. The simulation stops early, without a final snapshot, if `on_step` breaks.
fn simulate(
    mut planet: Planet,
    cli: &Cli,
    mut on_step: impl FnMut(&Planet) -> ControlFlow<()>,
) -> anyhow::Result<()> {
    let steps = (cli.duration / cli.dt).round() as u64;
    let snapshot_every = ((cli.snapshot_interval / cli.dt).round() as u64).max(1);
    let start = Instant::now();
    let mut previous_ledger: Option<PreviousSnapshot> = None;
    snapshot(&planet, cli, &mut previous_ledger)?;
    if on_step(&planet).is_break() {
        info!("Simulation stopped at the start");
        return Ok(());
    }
    for step in 1..=steps {
        planet.step(cli.dt);
        if on_step(&planet).is_break() {
            info!(time = planet.tectonics().time(), "Simulation stopped");
            return Ok(());
        }
        if step % snapshot_every == 0 || step == steps {
            snapshot(&planet, cli, &mut previous_ledger)?;
        }
    }
    info!("Finished in {:.1?}", start.elapsed());
    Ok(())
}

fn snapshot(
    planet: &Planet,
    cli: &Cli,
    previous_ledger: &mut Option<PreviousSnapshot>,
) -> anyhow::Result<()> {
    let simulation = planet.tectonics();
    let planet_statistics = planet.statistics();
    let statistics = &planet_statistics.tectonics;
    let largest_plate = statistics.plate_areas.iter().max().copied().unwrap_or(0);
    let living_plates = statistics.plate_areas.iter().filter(|&&a| a > 0).count();
    info!(
        "t = {:6.1} Myr | continental {:5.1}% | land {:5.1}% | mean ocean age {:5.1} Myr | \
         elevation {:6.0} m ..{:6.0} m, land mean {:5.0} m | mean speed {:4.1} cm/yr | \
         {} plates ({} born, {} gone), largest {:4.1}%, {} plumes",
        statistics.time,
        100.0 * statistics.continental_fraction,
        100.0 * statistics.land_fraction,
        statistics.mean_oceanic_age,
        statistics.min_elevation,
        statistics.max_elevation,
        statistics.mean_land_elevation,
        // rad/Myr × m → cm/yr
        statistics.mean_speed * EARTH_RADIUS * 100.0 / 1e6,
        living_plates,
        statistics.events.births(),
        statistics.events.deaths(),
        100.0 * largest_plate as f64 / simulation.mesh().len() as f64,
        statistics.plumes,
    );
    // rad/Myr × m → cm/yr
    let cm_per_year = EARTH_RADIUS * 100.0 / 1e6;
    info!(
        "    ocean age median {:4.0}, 90% {:4.0}, max {:4.0} Myr | deeper than 6 km {:4.1}%, \
         ocean older than 150 Myr {:4.1}% | \
         speed oceanic {:4.1}, continental {:4.1}, max {:5.1}, 99% {:4.1}, \
         RMS of plates > 1% {:4.1} cm/yr",
        statistics.oceanic_age_percentiles[0],
        statistics.oceanic_age_percentiles[1],
        statistics.oceanic_age_percentiles[2],
        100.0 * statistics.deep_fraction,
        100.0 * statistics.old_ocean_fraction,
        statistics.mean_oceanic_speed * cm_per_year,
        statistics.mean_continental_speed * cm_per_year,
        statistics.max_speed * cm_per_year,
        statistics.speed_p99 * cm_per_year,
        statistics.rms_plate_speed * cm_per_year,
    );
    info!(
        "    sea level {:+.0} m (dry offset; changes by tectonics {:+.0} m, surface processes \
         {:+.0} m; sediment part {:+.0} m) | ocean volume {:.3e} km³ | crust of continental origin {:.1}% of the surface, \
         {:.0}% flooded, freeboard {:.0} m, thickness {:.0}/{:.0}/{:.0} km (10/50/90%), \
         {:.0}% > 50 km",
        statistics.sea_level,
        statistics.sea_level_budget.tectonics,
        statistics.sea_level_budget.total() - statistics.sea_level_budget.tectonics,
        statistics.sediment_sea_level,
        statistics.ocean_volume,
        100.0 * statistics.continental_origin_fraction,
        100.0 * statistics.flooded_continental_fraction,
        statistics.freeboard,
        statistics.continental_thickness[0],
        statistics.continental_thickness[1],
        statistics.continental_thickness[2],
        100.0 * statistics.thick_continental_fraction,
    );
    info!(
        "    boundary motion: relative speed median {:.1} cm/yr, {:.0}% slower than 2 cm/yr | \
         mean |cos| {:.2}, opening/closing {:.0}%, strike-slip {:.0}% | shear/slab-pull power \
         {:.3}",
        statistics.median_relative_speed * cm_per_year,
        100.0 * statistics.slow_boundary_fraction,
        statistics.mean_obliquity_cos,
        100.0 * statistics.normal_motion_fraction,
        100.0 * statistics.strike_slip_fraction,
        statistics.shear_power_ratio,
    );
    let histogram: Vec<String> = statistics
        .age_histogram
        .iter()
        .map(|fraction| format!("{:.0}", 100.0 * fraction))
        .collect();
    info!(
        "    ocean area by age (% per 20 Myr): {} | ≥ 200 Myr {:.1}% ({:.0}% at continents) | \
         sea floor: mean age {:.1} Myr, ≥ 200 Myr {:.2}%, filled ≥ 200 Myr {:.2}% of the surface | \
         plates > 1%: {}, \
         boundary ratio {:.2} | old passive margins {:.0} km, {:.3} initiations/Myr, \
         induced {:.3}/Myr",
        histogram.join(" "),
        100.0 * statistics.ancient_ocean_fraction,
        100.0 * statistics.ancient_ocean_at_continents,
        statistics.sea_floor_mean_age,
        100.0 * statistics.ancient_sea_floor_fraction,
        100.0 * statistics.ancient_filled_fraction,
        statistics.plates_above_one_percent,
        statistics.boundary_ratio,
        statistics.old_margin_length,
        statistics.initiation_rate,
        statistics.induced_rate,
    );
    info!(
        "    trenches {:.0} km, ridges {:.0} km, collisions {:.0} km ({:.0} converging, {:.0} diverging) | slab pull {:.2e} N, ridge push {:.2e} N | \
         rifts {}, initiations {}, breakups {}, induced {}, reversals {}, sutures {}, captures {} (+ {} pieces of {} cells; {} cells broke up), consumed {}, merge loss {:.0} km·cells, \
         blocked splits {}, terranes {} ({} cells, {} slab break-offs) | continental contacts made by rifts {:.0} km ({:.0} far), by initiations {:.0} km ({:.0} far) | \
         splits along weak zones {} ({} cut again, {} along the coast, {} rifts through the plume head; rift boundaries on sutures {:.0}% of their continental cells vs {:.0}% of the rifted plates), along great circles {}, boundary deviation from a great circle: rifts {:.0} km, initiations {:.0} km | \
         fracture-zone initiations {}, failed initiations (no division/overriding/subducting part too small) {}/{}/{}, \
         undone initiations (suture/capture/piece/terrane) {}/{}/{}/{}",
        statistics.trench_length,
        statistics.ridge_length,
        statistics.collision_length,
        statistics.collision_converging_length,
        statistics.collision_diverging_length,
        statistics.slab_pull,
        statistics.ridge_push,
        statistics.events.rifts,
        statistics.events.initiations,
        statistics.events.breakups,
        statistics.events.induced_subductions,
        statistics.events.polarity_reversals,
        statistics.events.sutures,
        statistics.events.captures,
        statistics.events.piece_captures,
        statistics.events.captured_piece_cells,
        statistics.events.breakup_cells,
        statistics.events.consumed,
        statistics.events.merge_lost_continental_volume / 1000.0,
        statistics.events.blocked_splits,
        statistics.events.terrane_accretions,
        statistics.events.accreted_terrane_cells,
        statistics.events.slab_breakoffs,
        statistics.events.rift_contact_km,
        statistics.events.rift_far_contact_km,
        statistics.events.initiation_contact_km,
        statistics.events.initiation_far_contact_km,
        statistics.events.weak_zone_splits,
        statistics.events.recut_splits,
        statistics.events.coastal_splits,
        statistics.events.head_rifts,
        100.0 * statistics.events.rift_suture_cells[0] as f64
            / statistics.events.rift_suture_cells[1].max(1) as f64,
        100.0 * statistics.events.rift_suture_cells[2] as f64
            / statistics.events.rift_suture_cells[3].max(1) as f64,
        statistics.events.great_circle_splits,
        statistics.events.split_deviation_km[0]
            / statistics.events.measured_splits[0].max(1) as f64,
        statistics.events.split_deviation_km[1]
            / statistics.events.measured_splits[1].max(1) as f64,
        statistics.events.fracture_zone_initiations,
        statistics.events.failed_initiation_reasons[0],
        statistics.events.failed_initiation_reasons[1],
        statistics.events.failed_initiation_reasons[2],
        statistics.events.undone_initiations[0],
        statistics.events.undone_initiations[1],
        statistics.events.undone_initiations[2],
        statistics.events.undone_initiations[3],
    );
    let events = &statistics.events;
    info!(
        "    rift belts (continental cells, boundary/plates): ever marked {}/{} of {}/{}, orogenic weakening {:.1}/{:.1}, weak (≥ {}) {}/{} | \
         plume rifts failed {}, break vs plate motion |cos| {:.3} over {} rifts",
        events.rift_suture_cells[0],
        events.rift_suture_cells[2],
        events.rift_suture_cells[1],
        events.rift_suture_cells[3],
        events.rift_orogenic[0],
        events.rift_orogenic[1],
        tectonics::WEAK_BELT_THRESHOLD,
        events.rift_weak_cells[0],
        events.rift_weak_cells[1],
        events.failed_plume_rifts,
        events.rift_motion_alignment.0 / events.rift_motion_alignment.1.max(1) as f64,
        events.rift_motion_alignment.1,
    );
    log_sediment_reservoirs(&planet_statistics);
    {
        let field = simulation.dynamic_topography();
        let (mut low, mut high) = (0.0f64, 0.0f64);
        let (mut continental, mut oceanic, mut continental_abs) =
            ((0.0, 0usize), (0.0, 0usize), 0.0);
        for (cell, &h) in field.iter().enumerate() {
            low = low.min(h);
            high = high.max(h);
            if simulation
                .surface_crust(cell as u32)
                .is_continental_origin()
            {
                continental.0 += h;
                continental.1 += 1;
                continental_abs += h.abs();
            } else {
                oceanic.0 += h;
                oceanic.1 += 1;
            }
        }
        info!(
            "    dynamic topography (m, dry): {low:+.0} .. {high:+.0} | mean on continental crust \
             {:+.0} (|h| {:.0}), elsewhere {:+.0}",
            continental.0 / continental.1.max(1) as f64,
            continental_abs / continental.1.max(1) as f64,
            oceanic.0 / oceanic.1.max(1) as f64,
        );
    }
    // Crust mass budget since the last snapshot, in Gt/yr.
    let cell_area =
        4.0 * std::f64::consts::PI * EARTH_RADIUS * EARTH_RADIUS / simulation.mesh().len() as f64;
    let new_crust_cells = statistics.gaps.new_crust_cells;
    let budget = simulation.material_budget();
    if let Some(PreviousSnapshot {
        time: previous_time,
        ledger: previous,
        new_crust_cells: previous_cells,
        opened_area: previous_opened,
        continental_opened_area: previous_continental_opened,
        stretched_cells: previous_stretched,
        budget: previous_budget,
        collision_inflow: previous_inflow,
        trench_sediment: previous_trench,
        wave_erosion: previous_wave,
        sediment_delivered: previous_delivered,
        cover_turnover: previous_cover,
        sea_level_budget: previous_sea_level,
        metamorphosed: previous_metamorphosed,
    }) = previous_ledger.as_ref()
    {
        let years = (statistics.time - previous_time) * 1e6;
        let sea_level_budget = &statistics.sea_level_budget;
        let terms = [
            (
                "tectonics",
                sea_level_budget.tectonics,
                previous_sea_level.tectonics,
            ),
            (
                "fluvial",
                sea_level_budget.fluvial,
                previous_sea_level.fluvial,
            ),
            (
                "coastal",
                sea_level_budget.coastal,
                previous_sea_level.coastal,
            ),
            (
                "sharing",
                sea_level_budget.sharing,
                previous_sea_level.sharing,
            ),
            (
                "metamorphism",
                sea_level_budget.metamorphism,
                previous_sea_level.metamorphism,
            ),
            ("mantle", sea_level_budget.mantle, previous_sea_level.mantle),
        ];
        let parts: Vec<String> = terms
            .iter()
            .map(|(name, now, before)| {
                format!("{name} {now:+.1} | {:+.1}", (now - before) / years * 1e8)
            })
            .collect();
        info!(
            "    sea-level budget (m so far | m per 100 Myr since the last snapshot): {} | \
             total {:+.1} | {:+.1}, unexplained {:+.4} m",
            parts.join(", "),
            sea_level_budget.total(),
            (sea_level_budget.total() - previous_sea_level.total()) / years * 1e8,
            statistics.sea_level_residual,
        );
        let wave = &planet_statistics.wave_erosion;
        let rate = |now: f64, before: f64| (now - before) / years;
        let rivers = rate(planet_statistics.sediment_delivered, *previous_delivered);
        let cliff_bedrock = rate(wave.cliff_bedrock, previous_wave.cliff_bedrock);
        info!(
            "    wave erosion (Gt/yr): cliff sediment {:.3}, cliff bedrock {:.3}, reworked {:.3}, \
             stayed {:.3} | rivers to the sea {:.2}, cliff bedrock {:.1}% of it",
            rate(wave.cliff_sediment, previous_wave.cliff_sediment),
            cliff_bedrock,
            rate(wave.reworked, previous_wave.reworked),
            rate(wave.stayed, previous_wave.stayed),
            rivers,
            100.0 * cliff_bedrock / rivers.max(1e-9),
        );
        let trench = &statistics.trench_sediment;
        info!(
            "    sediment turned into crust (Gt/yr): burial metamorphism {:.2} on continental \
             crust, {:.2} on other crust | trench accretion {:.2} | collision stacking {:.2} | \
             metasediment at trenches: accreted {:.2}, subducted {:.2}",
            (statistics.metamorphosed.0 - previous_metamorphosed.0) / years,
            (statistics.metamorphosed.1 - previous_metamorphosed.1) / years,
            (trench.accreted - previous_trench.accreted) / years,
            (trench.stacked - previous_trench.stacked) / years,
            (trench.metasediment_accreted - previous_trench.metasediment_accreted) / years,
            (trench.metasediment_subducted - previous_trench.metasediment_subducted) / years,
        );
        // Turnover of the continental sedimentary cover: its stock over what leaves it (gross
        // removal by surface processes, net per cell and step, and burial metamorphism).
        let cover = &planet_statistics.cover_turnover;
        let cover_eroded = rate(cover.eroded, previous_cover.eroded);
        let cover_burial = (statistics.metamorphosed.0 - previous_metamorphosed.0) / years;
        let cover_stock = planet_statistics.sediment_stock[0];
        info!(
            "    continental cover: stock {:.3e} Gt | surface processes (net per cell and step, a lower \
             bound of the gross turnover) eroded {:.2}, deposited \
             {:.2} Gt/yr | turnover half-life {:.0} Myr",
            cover_stock,
            cover_eroded,
            rate(cover.deposited, previous_cover.deposited),
            std::f64::consts::LN_2 * cover_stock / (cover_eroded + cover_burial).max(1e-9) / 1e6,
        );
        if let Some(cohort) = &planet_statistics.cover_cohort {
            let elapsed = statistics.time - cohort.start;
            let left = cohort.on_continents / cohort.initial.max(1e-9);
            info!(
                "    cover cohort of {:.0} Myr: {:.3e} Gt, {:.1}% left on continental crust \
                 ({:.1}% as sediment anywhere), preservation half-life {:.0} Myr",
                cohort.start,
                cohort.initial,
                100.0 * left,
                100.0 * cohort.anywhere / cohort.initial.max(1e-9),
                if elapsed > 0.0 && left < 1.0 {
                    elapsed * std::f64::consts::LN_2 / -math::ln(left.max(1e-12))
                } else {
                    f64::INFINITY
                },
            );
        }
        info!(
            "    collision stacking (Gt/yr): {:.1} stacked, {:.1} carried in by convergence",
            (statistics.ledger.collision_stacked - previous.collision_stacked) * cell_area
                / 1e12
                / years,
            (statistics.collision_inflow - previous_inflow) / years,
        );
        if let (Some(budget), Some(previous_budget)) = (budget, previous_budget) {
            log_material_budget(&budget, previous_budget, cell_area, years);
        }
        info!(
            "    sea floor production (km²/yr): kinematic {:.2}, created {:.2} | continental \
             opening: kinematic {:.3}, stretched {:.3}",
            (statistics.opened_area - previous_opened) / years,
            (new_crust_cells - previous_cells) as f64 * cell_area / 1e6 / years,
            (statistics.continental_opened_area - previous_continental_opened) / years,
            (statistics.gaps.stretched_cells - previous_stretched) as f64 * cell_area / 1e6 / years,
        );
        let rates: Vec<String> = statistics
            .ledger
            .terms()
            .iter()
            .zip(previous.terms())
            .filter(|((name, _), _)| *name != "total")
            .map(|((name, now), (_, before))| (name, (now - before) * cell_area / 1e12 / years))
            .filter(|(_, rate)| rate.abs() >= 0.005)
            .map(|(name, rate)| format!("{name} {rate:+.2}"))
            .collect();
        info!("    crust mass budget (Gt/yr): {}", rates.join(", "));
        let trench = &statistics.trench_sediment;
        let arrived: f64 = trench.by_thickness.iter().sum::<f64>()
            - previous_trench.by_thickness.iter().sum::<f64>();
        let shares: Vec<String> = trench
            .by_thickness
            .iter()
            .zip(&previous_trench.by_thickness)
            .map(|(now, before)| format!("{:.0}%", 100.0 * (now - before) / arrived.max(1e-9)))
            .collect();
        info!(
            "    trench sediment {:.1} Gt/yr, {:.0}% accreted, {:.0}% on continental margins, {:.0}% \
             at accretionary margins ({:.0}% of the trench length) | by column (≤ 1, 1–3, 3–5, \
             5–8, > 8 km): {} | upper plates accreted to buoyant arrivals {:.2} Gt/yr",
            arrived / years,
            100.0 * (trench.accreted - previous_trench.accreted) / arrived.max(1e-9),
            100.0 * (trench.on_continental_margins - previous_trench.on_continental_margins)
                / arrived.max(1e-9),
            100.0 * (trench.at_accretionary_margins - previous_trench.at_accretionary_margins)
                / arrived.max(1e-9),
            100.0 * statistics.accretionary_trench_length / statistics.trench_length.max(1e-9),
            shares.join(" "),
            (trench.upper_plate_accreted - previous_trench.upper_plate_accreted) / years,
        );
    }
    *previous_ledger = Some(PreviousSnapshot {
        time: statistics.time,
        ledger: statistics.ledger.clone(),
        new_crust_cells,
        opened_area: statistics.opened_area,
        continental_opened_area: statistics.continental_opened_area,
        stretched_cells: statistics.gaps.stretched_cells,
        budget,
        collision_inflow: statistics.collision_inflow,
        trench_sediment: statistics.trench_sediment.clone(),
        sea_level_budget: statistics.sea_level_budget,
        metamorphosed: statistics.metamorphosed,
        wave_erosion: planet_statistics.wave_erosion,
        sediment_delivered: planet_statistics.sediment_delivered,
        cover_turnover: planet_statistics.cover_turnover,
    });
    let segments = &statistics.segment_events;
    info!(
        "    segments {} ({} with slab; slab at trenches {:.0} km; incipient trenches {:.0} km converging at {:.1} cm/yr, others at {:.1} cm/yr; diffuse zones {:.0} km \
         converging at {:.2} cm/yr; {} shortened, mean {:.0} km, max {:.0} km) | created {}, merged {}, expired {}, \
         flips {}, continental overrides {}, initiation segments {} ({} grew to 300 km), \
         failed initiations {}, resumed subductions {}, localisations {} (stacked before so far {:.0} Gt), \
         subducted at diffuse zones so far {:.0} Gt, weak thrusts {} | unsettled velocity solves {}",
        statistics.segments,
        statistics.slab_segments,
        statistics.mean_slab_length,
        statistics.incipient_trench_length,
        statistics.incipient_convergence,
        statistics.mature_convergence,
        statistics.diffuse_length,
        statistics.diffuse_convergence,
        statistics.shortened_segments,
        statistics.mean_shortening,
        statistics.max_shortening,
        segments.created,
        segments.merged,
        segments.expired,
        segments.flips,
        segments.continental_overrides,
        segments.initiation_segments,
        segments.successful_initiation_segments,
        statistics.events.failed_initiations,
        segments.resumed_subductions,
        segments.localisations,
        segments.transient_stacked * cell_area / 1e12,
        segments.diffuse_subducted * cell_area / 1e12,
        statistics.weak_thrust_segments,
        statistics.unsettled_solves,
    );
    info!(
        "    crust mass (Gt): continental {:.3e}, transitional {:.3e}, oceanic {:.3e}, \
         sediment {:.3e}, felsic {:.3e}, hidden {:.3e} | sediment overfill {:.0} km·cells (terminal fans and \
         deltas {:.0} km·cells), metamorphosed {:.3e} (on other crust {:.3e})",
        statistics.continental_mass,
        statistics.transitional_mass,
        statistics.oceanic_mass,
        statistics.sediment_mass,
        statistics.felsic_mass,
        statistics.hidden_mass,
        planet_statistics.sediment_overfill / 1000.0,
        planet_statistics.terminal_fans / 1000.0,
        statistics.metamorphosed.0 + statistics.metamorphosed.1,
        statistics.metamorphosed.1,
    );
    info!(
        "    plate matching: {} incremental, {} full scans, {:.1} cells searched per plate and \
         cell, {} mismatches",
        statistics.matching.incremental,
        statistics.matching.full_scans,
        statistics.matching.searched_cells as f64
            / ((statistics.matching.incremental + statistics.matching.full_scans).max(1) as f64
                * simulation.mesh().len() as f64),
        statistics.matching.mismatches,
    );
    info!(
        "    gaps filled: {} new sea floor, {} stretched, {} revealed, {} filled holes, {} shared ({} inside \
         plates), {} holes, {} freed",
        statistics.gaps.new_crust_cells,
        statistics.gaps.stretched_cells,
        statistics.gaps.revealed_cells,
        statistics.gaps.enclosed_fills,
        statistics.gaps.shared_cells,
        statistics.gaps.interior_shared_cells,
        statistics.gaps.hole_cells,
        statistics.gaps.freed_cells,
    );
    info!(
        "    resampling: {:.1}% of cells show a duplicated sample (at most {} cells), {} enclosed \
         empty spots | continental {:.1}% of cells, {:.1}% by samples",
        100.0 * statistics.duplicated_cell_fraction,
        statistics.max_cells_per_sample,
        statistics.enclosed_empty,
        100.0 * statistics.continental_fraction,
        100.0 * statistics.continental_sample_fraction,
    );
    info!(
        "    land masses {} ({} specks, largest {:.0}%) | continental crust masses {} ({} specks, \
         largest {:.0}%)",
        statistics.land_masses.count,
        statistics.land_masses.specks,
        100.0 * statistics.land_masses.largest_share,
        statistics.continental_masses.count,
        statistics.continental_masses.specks,
        100.0 * statistics.continental_masses.largest_share,
    );
    info!(
        "    land precipitation {:4.2} m/yr (20–60° {:4.2} m/yr, {:.0}% of land below 0.25 m/yr), \
         runoff {:4.2} m/yr | global precipitation {:4.2}, evaporation {:4.2} m/yr (clipped {:.2}%) | ITCZ {:?}° | \
         largest river {:8.0} m³/s | \
         sediment to ocean {:5.1} km³/yr (shelf {:.1}, slope {:.1}, rise {:.1}, fan {:.1}) | \
         sediment stock (Gt) continental {:.3e}, transitional {:.3e}, oceanic {:.3e}",
        planet_statistics.mean_land_precipitation,
        planet_statistics.midlatitude_land_precipitation,
        100.0 * planet_statistics.arid_land_fraction,
        planet_statistics.land_runoff,
        planet_statistics.global_precipitation,
        planet_statistics.global_evaporation,
        100.0 * planet_statistics.clipped_precipitation,
        planet_statistics
            .itcz
            .iter()
            .map(|l| l.round() as i64)
            .collect::<Vec<_>>(),
        planet_statistics.largest_river / SECONDS_PER_YEAR,
        planet_statistics.sediment_flux / 1e9,
        planet_statistics.sediment_deposition[0] / 1e9,
        planet_statistics.sediment_deposition[1] / 1e9,
        planet_statistics.sediment_deposition[2] / 1e9,
        planet_statistics.sediment_deposition[3] / 1e9,
        planet_statistics.sediment_stock[0],
        planet_statistics.sediment_stock[1],
        planet_statistics.sediment_stock[2],
    );
    let hydrology = &planet_statistics.hydrology;
    let land_cells = hydrology.land_cells.max(1.0);
    info!(
        "    hydrology: runoff {:.0} km³/yr, to the ocean {:.0} km³/yr, lake evaporation {:.0} \
         km³/yr | endorheic {:.1}% of land ({:.1}% into single-cell pits), lakes {:.1}% of land, \
         lake potential evaporation {:.2} m/yr | depressions {}, endorheic {}, with inner sills \
         {} | sediment trapped in lakes {:.2} km³/yr",
        hydrology.runoff / 1e9,
        hydrology.to_ocean / 1e9,
        hydrology.lake_evaporation / 1e9,
        100.0 * hydrology.endorheic_cells / land_cells,
        100.0 * hydrology.endorheic_pit_cells / land_cells,
        100.0 * hydrology.lake_cells / land_cells,
        hydrology.lake_potential_evaporation / hydrology.lake_cells.max(1e-9),
        hydrology.lakes,
        hydrology.endorheic_lakes,
        hydrology.nested_lakes,
        hydrology.lake_sediment / 1e9,
    );
    info!(
        "    temperature global {:5.1} °C, equator {:5.1} °C, poles {:6.1} °C | \
         land seasonal range {:4.1} K | permanently frozen {:4.1}% | heat transport {:+.1}/{:+.1} PW \
         (ocean at 20°N {:+.1}, 20°S {:+.1}) | gyres {:.0} Sv",
        planet_statistics.global_temperature,
        planet_statistics.equatorial_temperature,
        planet_statistics.polar_temperature,
        planet_statistics.land_seasonal_range,
        100.0 * planet_statistics.frozen_fraction,
        planet_statistics.heat_transport[0],
        planet_statistics.heat_transport[1],
        planet_statistics.heat_transport[2],
        planet_statistics.heat_transport[3],
        planet_statistics.gyre_transport,
    );

    info!("    state hash {:016x}", planet.state_hash());

    #[cfg(not(target_arch = "wasm32"))]
    write_snapshot_files(planet, cli)?;
    #[cfg(target_arch = "wasm32")]
    let _ = cli;
    Ok(())
}

/// Writes the snapshot images (and with `--cell-dump` the cell and mesh CSVs) into `--output`.
/// Not on the web, which has no file system.
#[cfg(not(target_arch = "wasm32"))]
fn write_snapshot_files(planet: &Planet, cli: &Cli) -> anyhow::Result<()> {
    let simulation = planet.tectonics();
    let time = simulation.time().round() as u64;
    if cli.cell_dump {
        let path = cli.output.join(format!("cells_{time:05}.csv"));
        write_cell_dump(planet, &path).with_context(|| format!("writing {}", path.display()))?;
        let path = cli.output.join(format!("boundaries_{time:05}.csv"));
        write_boundary_dump(simulation, &path)
            .with_context(|| format!("writing {}", path.display()))?;
        let path = cli.output.join("mesh.csv");
        if !path.exists() {
            write_mesh_dump(simulation.mesh(), &path)
                .with_context(|| format!("writing {}", path.display()))?;
        }
    }
    for (name, image) in [
        (
            "elevation",
            render::render_elevation(planet, cli.width, cli.river_threshold),
        ),
        ("plates", render::render_plates(simulation, cli.width)),
        (
            "precipitation",
            render::render_precipitation(planet, cli.width),
        ),
        (
            "temperature",
            render::render_temperature(planet, cli.width, &planet.temperature().annual_mean),
        ),
        (
            "temperature_january",
            render::render_temperature(planet, cli.width, &planet.temperature().monthly[0]),
        ),
        (
            "temperature_july",
            render::render_temperature(planet, cli.width, &planet.temperature().monthly[6]),
        ),
    ] {
        let path = cli.output.join(format!("{name}_{time:05}.png"));
        image
            .save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
        debug!(path = %path.display(), "Wrote snapshot");
    }
    Ok(())
}

/// Logs the rates of change of felsic crust and sediment per step phase since the last snapshot.
fn log_material_budget(
    budget: &MaterialBudget,
    previous_budget: &MaterialBudget,
    cell_area: f64,
    years: f64,
) {
    let rate = |now: (f64, f64), before: (f64, f64)| {
        (
            (now.0 - before.0) * cell_area / 1e12 / years,
            (now.1 - before.1) * cell_area / 1e12 / years,
        )
    };
    let phases = [
        ("substeps", rate(budget.substeps, previous_budget.substeps)),
        ("flow", rate(budget.flow, previous_budget.flow)),
        ("events", rate(budget.events, previous_budget.events)),
        ("surface", rate(budget.surface, previous_budget.surface)),
        (
            "metamorphism",
            rate(budget.metamorphism, previous_budget.metamorphism),
        ),
    ];
    let total = phases
        .iter()
        .fold((0.0, 0.0), |sum, (_, (f, s))| (sum.0 + f, sum.1 + s));
    let parts: Vec<String> = phases
        .iter()
        .map(|(name, (f, s))| format!("{name} {f:+.1}/{s:+.1}"))
        .collect();
    info!(
        "    felsic crust/sediment (Gt/yr): {} | net {:+.1}/{:+.1}, together {:+.1}",
        parts.join(", "),
        total.0,
        total.1,
        total.0 + total.1,
    );
}

/// Cumulative values at the previous snapshot, for rates over the interval since.
struct PreviousSnapshot {
    time: f64,
    ledger: MassLedger,
    new_crust_cells: usize,
    opened_area: f64,
    continental_opened_area: f64,
    stretched_cells: usize,
    budget: Option<MaterialBudget>,
    collision_inflow: f64,
    trench_sediment: TrenchSediment,
    sea_level_budget: SeaLevelBudget,
    metamorphosed: (f64, f64),
    wave_erosion: WaveErosion,
    sediment_delivered: f64,
    cover_turnover: CoverTurnover,
}

/// Writes one CSV row per cell: latitude (°), surface elevation (m), rock-equivalent and bulk
/// sediment thickness (m), crust thickness (m), density, felsic fraction and metasediment
/// thickness (m), crust class (0 continental origin, 1 transitional, 2 oceanic), plate, age (Myr),
/// number of cells showing the same sample, whether the cell is at a coast, precipitation (m/yr),
/// discharge (m³/yr), longitude (°), the annual mean, January and July surface temperatures
/// (°C), evaporation or evapotranspiration (m/yr), and January and July precipitation (m/yr).
#[cfg(not(target_arch = "wasm32"))]
fn write_cell_dump(planet: &Planet, path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write;
    let simulation = planet.tectonics();
    let mesh = simulation.mesh();
    let cell_count = mesh.len() as u32;
    let elevation = simulation.surface_elevations();
    let temperature = planet.temperature();
    // Precipitation of the solved month nearest to `month` (0 = January).
    let cycle = planet.water_cycle();
    let month_precipitation = |month: usize, cell: u32| {
        let solved = cycle.monthly_precipitation.len().max(1);
        let slot = (month * solved / 12).min(solved - 1);
        cycle
            .monthly_precipitation
            .get(slot)
            .map_or(0.0, |p| p[cell as usize])
    };
    let mut shown = std::collections::HashMap::new();
    for cell in 0..cell_count {
        *shown.entry(simulation.owner(cell)).or_insert(0u32) += 1;
    }
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        file,
        "cell,lat,elev,sed,bulk,thick,dens,felsic,meta,cls,plate,age,dup,coast,precip,discharge,\
             lon,t_annual,t_jan,t_jul,evap,p_jan,p_jul,orogeny,dyn,orogenic,strength_age,strength"
    )?;
    for cell in 0..cell_count {
        let crust = simulation.surface_crust(cell);
        let class = if crust.is_continental_origin() {
            0
        } else if crust.is_oceanic() {
            2
        } else {
            1
        };
        let e = elevation[cell as usize];
        let coast = mesh
            .neighbors(cell)
            .iter()
            .any(|&m| (elevation[m as usize] < 0.0) != (e < 0.0));
        writeln!(
            file,
            "{cell},{:.3},{e:.1},{:.1},{:.1},{:.1},{:.1},{:.3},{:.1},{class},{},{:.1},{},{},{:.4},{:.4e},\
             {:.3},{:.2},{:.2},{:.2},{:.4},{:.4},{:.4},{},{:.1},{:.3},{:.1},{:.3}",
            math::asin(mesh.vertex(cell).z).to_degrees(),
            crust.sediment,
            isostasy::sediment_bulk_thickness(crust.sediment),
            crust.thickness,
            crust.density,
            crust.felsic,
            crust.metasediment_thickness(),
            simulation.owner(cell).plate,
            crust.age(simulation.time()),
            shown[&simulation.owner(cell)],
            u8::from(coast),
            planet.precipitation()[cell as usize],
            planet.discharge()[cell as usize],
            math::atan2(mesh.vertex(cell).y, mesh.vertex(cell).x).to_degrees(),
            temperature.annual_mean[cell as usize],
            temperature.monthly[0][cell as usize],
            temperature.monthly[6][cell as usize],
            planet.water_cycle().evaporation[cell as usize],
            month_precipitation(0, cell),
            month_precipitation(6, cell),
            if crust.orogeny > tectonics::NO_OROGENY {
                format!("{:.1}", crust.orogeny)
            } else {
                String::new()
            },
            simulation.dynamic_topography()[cell as usize],
            crust.orogenic,
            crust.strength_age(simulation.time()).min(1e4),
            simulation.lithosphere_strength(cell),
        )?;
    }
    file.flush()?;
    Ok(())
}

/// Writes the plate boundaries as CSV, for analysis: one line per boundary cell and neighbouring
/// plate (see [`tectonics::BoundaryRecord`]), with the angular velocity (rad/Myr) of the cell's
/// plate.
fn write_boundary_dump(
    simulation: &tectonics::Simulation,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    use std::io::Write;
    let omega = simulation.plate_angular_velocities();
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        file,
        "cell,plate,other,kind,conv,strike,segment,overriding,slab_km,age,oceanic,continental,wx,wy,wz"
    )?;
    let optional = |value: Option<u32>| value.map_or(String::new(), |value| value.to_string());
    for record in simulation.boundary_records() {
        let w = omega[record.plate as usize];
        writeln!(
            file,
            "{},{},{},{},{:.3},{:.3},{},{},{:.0},{:.1},{},{},{:.6},{:.6},{:.6}",
            record.cell,
            record.plate,
            record.other,
            record.kind,
            record.convergence,
            record.strike_slip,
            optional(record.segment),
            optional(record.overriding),
            record.slab_length / 1000.0,
            record.age,
            u8::from(record.oceanic),
            u8::from(record.continental),
            w.x,
            w.y,
            w.z,
        )?;
    }
    Ok(())
}

/// Writes the mesh (cell, position, neighbours separated by `;`) for analysing cell dumps.
#[cfg(not(target_arch = "wasm32"))]
fn write_mesh_dump(mesh: &sphere::SphereMesh, path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write;
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(file, "cell,x,y,z,neighbors")?;
    for cell in 0..mesh.len() as u32 {
        let r = mesh.vertex(cell);
        let neighbors: Vec<String> = mesh.neighbors(cell).iter().map(u32::to_string).collect();
        writeln!(
            file,
            "{cell},{:.6},{:.6},{:.6},{}",
            r.x,
            r.y,
            r.z,
            neighbors.join(";")
        )?;
    }
    file.flush()?;
    Ok(())
}

/// Logs where the sediment sits: its parts of the sea level, shelves and low land.
fn log_sediment_reservoirs(statistics: &PlanetStatistics) {
    let r = &statistics.sediment_reservoirs;
    info!(
        "    sediment part of the sea level (m): all {:+.1}, marine {:+.1} | continental land \
         {:+.1}, continental flooded {:+.1}, transitional {:+.1}, oceanic {:+.1}, filled basins \
         {:+.1} ({:.1}% of the surface; metasediment basins {:.1}%), above wave base {:+.1} | \
         with run-formed metasediment \
         {:+.1} | solver/total sediment {:.1}% | metasediment (Gt) on continental basement \
         {:.3e}, on other {:.3e}",
        r.all,
        r.marine,
        r.continental_land,
        r.continental_flooded,
        r.transitional,
        r.oceanic,
        r.filled_basins,
        100.0 * r.filled_basin_fraction,
        100.0 * r.metasediment_basin_fraction,
        r.above_wave_base,
        r.with_metasediment,
        100.0 * r.solver_ratio,
        r.metasediment.0,
        r.metasediment.1,
    );
    if !(0.97..=1.03).contains(&r.solver_ratio) {
        warn!(
            "    the sea-level solve sees {:.1}% of the sediment (hidden and shared samples)",
            100.0 * r.solver_ratio
        );
    }
    info!(
        "    land lower than 10 m {:.1}% of land | shelves (0 to -200 m) {:.1}% of the surface, \
         mean sediment {:.2} km, {:.1}% on oceanic crust, {:.1}% on filled basins",
        100.0 * r.low_land_fraction,
        100.0 * r.shelf_fraction,
        r.shelf_sediment / 1000.0,
        100.0 * r.shelf_on_oceanic,
        100.0 * r.shelf_on_filled_basins,
    );
}
