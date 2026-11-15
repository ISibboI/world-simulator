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
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

use crate::climate::ClimateParams;
use crate::erosion::ErosionParams;
use crate::planet::Planet;
use crate::sphere::SphereMesh;
use crate::tectonics::{Simulation, TectonicParams};
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
    #[arg(long, default_value_t = 0.3)]
    continental_fraction: f64,

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

    /// Minimum discharge in m³/yr for a cell to be drawn as a river.
    #[arg(long, default_value_t = 1e11)]
    river_threshold: f64,

    /// Run without the 3D view and only write snapshot images.
    #[arg(long)]
    headless: bool,

    /// Vertical exaggeration of the relief in the 3D view.
    #[arg(long, default_value_t = 40.0)]
    exaggeration: f64,
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
    let params = TectonicParams {
        continental_fraction: cli.continental_fraction,
        ..TectonicParams::default()
    };
    let simulation = Simulation::new(mesh, cli.plates, params, cli.seed);
    let erosion = (!cli.no_erosion).then(|| ErosionParams {
        planet_radius: EARTH_RADIUS,
        ..ErosionParams::default()
    });
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
    snapshot(&planet, cli)?;
    on_step(&planet);
    for step in 1..=steps {
        planet.step(cli.dt);
        on_step(&planet);
        if step % snapshot_every == 0 || step == steps {
            snapshot(&planet, cli)?;
        }
    }
    info!("Finished in {:.1?}", start.elapsed());
    Ok(())
}

fn snapshot(planet: &Planet, cli: &Cli) -> anyhow::Result<()> {
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
        statistics.plate_births,
        statistics.plate_deaths,
        100.0 * largest_plate as f64 / simulation.mesh().len() as f64,
        statistics.plumes,
    );
    // rad/Myr × m → cm/yr
    let cm_per_year = EARTH_RADIUS * 100.0 / 1e6;
    info!(
        "    ocean age median {:4.0}, 90% {:4.0}, max {:4.0} Myr | deeper than 6 km {:4.1}%, \
         ocean older than 150 Myr {:4.1}% | \
         speed oceanic {:4.1}, continental {:4.1}, max {:5.1} cm/yr",
        statistics.oceanic_age_percentiles[0],
        statistics.oceanic_age_percentiles[1],
        statistics.oceanic_age_percentiles[2],
        100.0 * statistics.deep_fraction,
        100.0 * statistics.old_ocean_fraction,
        statistics.mean_oceanic_speed * cm_per_year,
        statistics.mean_continental_speed * cm_per_year,
        statistics.max_speed * cm_per_year,
    );
    info!(
        "    gaps filled: {} new sea floor, {} copies ({:.0} km·cells), {} shared, {} holes, \
         {} freed",
        statistics.gaps.new_crust_cells,
        statistics.gaps.copied_cells,
        statistics.gaps.copied_volume / 1000.0,
        statistics.gaps.shared_cells,
        statistics.gaps.hole_cells,
        statistics.gaps.freed_cells,
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
        "    land precipitation {:4.2} m/yr | largest river {:8.0} m³/s | \
         sediment to ocean {:5.1} km³/yr",
        planet_statistics.mean_land_precipitation,
        planet_statistics.largest_river / SECONDS_PER_YEAR,
        planet_statistics.sediment_flux / 1e9,
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
