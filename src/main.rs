mod render;
mod sphere;
mod tectonics;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, ensure};
use clap::Parser;

use crate::sphere::SphereMesh;
use crate::tectonics::{Simulation, TectonicParams};

/// Used to convert angular speeds into familiar linear speeds.
const EARTH_RADIUS_CM: f64 = 6.371e8;

/// Simulates tectonic plates on a spherical planet and writes map snapshots as PNG images.
#[derive(Parser, Debug)]
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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
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
    println!("Planet has {} cells", mesh.len());
    let params = TectonicParams {
        continental_fraction: cli.continental_fraction,
        ..TectonicParams::default()
    };
    let mut simulation = Simulation::new(mesh, cli.plates, params, cli.seed);

    let steps = (cli.duration / cli.dt).round() as u64;
    let snapshot_every = ((cli.snapshot_interval / cli.dt).round() as u64).max(1);
    let start = Instant::now();
    snapshot(&simulation, &cli)?;
    for step in 1..=steps {
        simulation.step(cli.dt);
        if step % snapshot_every == 0 || step == steps {
            snapshot(&simulation, &cli)?;
        }
    }
    println!("Finished in {:.1?}", start.elapsed());
    Ok(())
}

fn snapshot(simulation: &Simulation, cli: &Cli) -> anyhow::Result<()> {
    let statistics = simulation.statistics();
    let largest_plate = statistics.plate_areas.iter().max().copied().unwrap_or(0);
    let living_plates = statistics.plate_areas.iter().filter(|&&a| a > 0).count();
    println!(
        "t = {:6.1} Myr | continental {:5.1}% | land {:5.1}% | mean ocean age {:5.1} Myr | \
         elevation {:6.0} m ..{:6.0} m | mean speed {:4.1} cm/yr | {} plates, largest {:4.1}%",
        statistics.time,
        100.0 * statistics.continental_fraction,
        100.0 * statistics.land_fraction,
        statistics.mean_oceanic_age,
        statistics.min_elevation,
        statistics.max_elevation,
        statistics.mean_speed * EARTH_RADIUS_CM / 1e6,
        living_plates,
        100.0 * largest_plate as f64 / simulation.mesh().len() as f64,
    );

    let time = simulation.time().round() as u64;
    for (name, image) in [
        ("elevation", render::render_elevation(simulation, cli.width)),
        ("plates", render::render_plates(simulation, cli.width)),
    ] {
        let path = cli.output.join(format!("{name}_{time:05}.png"));
        image
            .save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}
