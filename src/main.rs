mod climate;
mod erosion;
mod isostasy;
mod planet;
mod render;
mod sphere;
mod tectonics;
mod temperature;
mod viewer;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, ensure};
use clap::Parser;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::climate::ClimateParams;
use crate::erosion::{ErosionParams, WaveErosion};
use crate::planet::{Planet, PlanetStatistics};
use crate::sphere::SphereMesh;
use crate::tectonics::{
    InitialConfiguration, MassLedger, MaterialBudget, SeaLevelBudget, Simulation, TectonicParams,
    TrenchSediment,
};
use crate::temperature::TemperatureParams;

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
    #[arg(long, default_value_t = 6)]
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

    /// Write the state of every cell (elevation, sediment, crust) as CSV with each snapshot,
    /// for analysis.
    #[arg(long)]
    cell_dump: bool,

    /// Overrides a tectonic parameter for calibration, e.g. `--set basal_drag=8e14`, or an
    /// erosion parameter with the prefix `erosion.`; can be repeated. See `TectonicParams::set`
    /// and `ErosionParams::set` for the names.
    #[arg(long = "set", value_name = "NAME=VALUE")]
    overrides: Vec<String>,
}

/// Logs to stderr, filtered by `RUST_LOG` (default: `info`). Also captures `log` records of
/// dependencies.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();
}

const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_logging();
    ensure!(
        cli.subdivisions <= 8,
        "at most 8 subdivisions are supported"
    );
    ensure!(cli.plates >= 1, "there must be at least one plate");
    ensure!(cli.dt > 0.0, "the time step must be positive");
    ensure!(cli.duration >= 0.0, "the duration must not be negative");
    ensure!(cli.width >= 2, "the image width must be at least 2");
    std::fs::create_dir_all(&cli.output)
        .with_context(|| format!("creating output directory {}", cli.output.display()))?;

    let mesh = SphereMesh::icosphere(cli.subdivisions);
    info!(cells = mesh.len(), "Created planet mesh");
    let mut params = TectonicParams {
        continental_fraction: cli.continental_fraction,
        initial: cli.initial,
        ..TectonicParams::default()
    };
    params.matching.verify = cli.verify_matching;
    params.initial_sediment.enabled = !cli.no_initial_sediment;
    params.track_material_budget = cli.material_budget;
    let mut erosion = ErosionParams {
        planet_radius: EARTH_RADIUS,
        ..ErosionParams::default()
    };
    for assignment in &cli.overrides {
        let (name, value) = assignment
            .split_once('=')
            .with_context(|| format!("expected NAME=VALUE, got {assignment}"))?;
        let value: f64 = value
            .parse()
            .with_context(|| format!("parsing the value of {name}"))?;
        if let Some(name) = name.strip_prefix("erosion.") {
            erosion.set(name, value)?;
        } else {
            params.set(name, value)?;
        }
        info!(name, value, "Overrode parameter");
    }
    let simulation = Simulation::new(mesh, cli.plates, params, cli.seed);
    let erosion = (!cli.no_erosion).then_some(erosion);
    let planet = Planet::new(
        simulation,
        ClimateParams::default(),
        TemperatureParams::default(),
        erosion,
        EARTH_RADIUS,
    );

    if cli.headless {
        return simulate(planet, &cli, |_| {});
    }

    let mesh = planet.tectonics().mesh();
    let (vertex_count, faces) = (mesh.len(), mesh.faces().to_vec());
    viewer::run(vertex_count, faces, move |updates, wake_up| {
        std::thread::spawn(move || {
            let publish = |planet: &Planet| {
                let vertices =
                    viewer::surface_vertices(planet, cli.exaggeration, cli.river_threshold);
                let update = viewer::SurfaceUpdate {
                    time: planet.tectonics().time(),
                    vertices: std::sync::Arc::new(vertices),
                };
                // Sending fails once the window was closed; the process then exits anyway.
                if updates.send(update).is_ok() {
                    wake_up();
                }
            };
            if let Err(error) = simulate(planet, &cli, publish) {
                error!("Simulation failed: {error:#}");
            }
        });
    })
}

/// Runs the simulation, writing snapshots and calling `on_step` with the initial state and after
/// every step.
fn simulate(mut planet: Planet, cli: &Cli, mut on_step: impl FnMut(&Planet)) -> anyhow::Result<()> {
    let steps = (cli.duration / cli.dt).round() as u64;
    let snapshot_every = ((cli.snapshot_interval / cli.dt).round() as u64).max(1);
    let start = Instant::now();
    let mut previous_ledger: Option<PreviousSnapshot> = None;
    snapshot(&planet, cli, &mut previous_ledger)?;
    on_step(&planet);
    for step in 1..=steps {
        planet.step(cli.dt);
        on_step(&planet);
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
         plates > 1%: {}, \
         boundary ratio {:.2} | old passive margins {:.0} km, {:.3} initiations/Myr, \
         induced {:.3}/Myr",
        histogram.join(" "),
        100.0 * statistics.ancient_ocean_fraction,
        100.0 * statistics.ancient_ocean_at_continents,
        statistics.plates_above_one_percent,
        statistics.boundary_ratio,
        statistics.old_margin_length,
        statistics.initiation_rate,
        statistics.induced_rate,
    );
    info!(
        "    trenches {:.0} km, ridges {:.0} km, collisions {:.0} km ({:.0} converging, {:.0} diverging) | slab pull {:.2e} N, ridge push {:.2e} N | \
         rifts {}, initiations {}, breakups {}, induced {}, reversals {}, sutures {}, captures {} (+ {} pieces of {} cells; {} cells broke up), consumed {}, merge loss {:.0} km·cells, \
         blocked splits {}, terranes {} ({} cells, {} slab break-offs) | continental contacts made by rifts {:.0} km ({:.0} far), by initiations {:.0} km ({:.0} far)",
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
    );
    log_sediment_reservoirs(&planet_statistics);
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
    });
    let segments = &statistics.segment_events;
    info!(
        "    segments {} ({} with slab; slab at trenches {:.0} km) | created {}, merged {}, expired {}, \
         flips {}, continental overrides {}, initiation segments {} ({} grew to 300 km), \
         failed initiations {}",
        statistics.segments,
        statistics.slab_segments,
        statistics.mean_slab_length,
        segments.created,
        segments.merged,
        segments.expired,
        segments.flips,
        segments.continental_overrides,
        segments.initiation_segments,
        segments.successful_initiation_segments,
        statistics.events.failed_initiations,
    );
    info!(
        "    crust mass (Gt): continental {:.3e}, transitional {:.3e}, oceanic {:.3e}, \
         sediment {:.3e}, felsic {:.3e}, hidden {:.3e} | sediment overfill {:.0} km·cells, \
         metamorphosed {:.3e} (on other crust {:.3e})",
        statistics.continental_mass,
        statistics.transitional_mass,
        statistics.oceanic_mass,
        statistics.sediment_mass,
        statistics.felsic_mass,
        statistics.hidden_mass,
        planet_statistics.sediment_overfill / 1000.0,
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
        "    land precipitation {:4.2} m/yr (20–60° {:4.2} m/yr, {:.0}% of land below 0.25 m/yr) | \
         largest river {:8.0} m³/s | \
         sediment to ocean {:5.1} km³/yr (shelf {:.1}, slope {:.1}, rise {:.1}, fan {:.1}) | \
         sediment stock (Gt) continental {:.3e}, transitional {:.3e}, oceanic {:.3e}",
        planet_statistics.mean_land_precipitation,
        planet_statistics.midlatitude_land_precipitation,
        100.0 * planet_statistics.arid_land_fraction,
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
    info!(
        "    temperature global {:5.1} °C, equator {:5.1} °C, poles {:6.1} °C | \
         land seasonal range {:4.1} K | permanently frozen {:4.1}%",
        planet_statistics.global_temperature,
        planet_statistics.equatorial_temperature,
        planet_statistics.polar_temperature,
        planet_statistics.land_seasonal_range,
        100.0 * planet_statistics.frozen_fraction,
    );

    let time = simulation.time().round() as u64;
    if cli.cell_dump {
        let path = cli.output.join(format!("cells_{time:05}.csv"));
        write_cell_dump(planet, &path).with_context(|| format!("writing {}", path.display()))?;
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
}

/// Writes one CSV row per cell: latitude (°), surface elevation (m), rock-equivalent and bulk
/// sediment thickness (m), crust thickness (m), density, felsic fraction and metasediment
/// thickness (m), crust class (0 continental origin, 1 transitional, 2 oceanic), plate, age (Myr),
/// number of cells showing the same sample, whether the cell is at a coast, precipitation (m/yr),
/// discharge (m³/yr), longitude (°), and the annual mean, January and July surface temperatures
/// (°C).
fn write_cell_dump(planet: &Planet, path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write;
    let simulation = planet.tectonics();
    let mesh = simulation.mesh();
    let cell_count = mesh.len() as u32;
    let elevation = simulation.surface_elevations();
    let temperature = planet.temperature();
    let mut shown = std::collections::HashMap::new();
    for cell in 0..cell_count {
        *shown.entry(simulation.owner(cell)).or_insert(0u32) += 1;
    }
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        file,
        "cell,lat,elev,sed,bulk,thick,dens,felsic,meta,cls,plate,age,dup,coast,precip,discharge,\
             lon,t_annual,t_jan,t_jul"
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
             {:.3},{:.2},{:.2},{:.2}",
            mesh.vertex(cell).z.asin().to_degrees(),
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
            mesh.vertex(cell).y.atan2(mesh.vertex(cell).x).to_degrees(),
            temperature.annual_mean[cell as usize],
            temperature.monthly[0][cell as usize],
            temperature.monthly[6][cell as usize],
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
