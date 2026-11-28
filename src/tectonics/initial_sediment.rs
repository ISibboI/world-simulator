//! The sediment cover of the initial planet.
//!
//! A planet with an ocean and continents has a sedimentary shell from its earlier history; the
//! initial conditions place one where the processes that build it on Earth would have put it:
//!
//! * **Pelagic sediment** on oceanic crust accumulates while the crust ages: carbonate ooze while
//!   the sea floor lies above the carbonate compensation depth (CCD), then only red clay and
//!   siliceous ooze. So it is thin at ridges and thickens with age.
//! * **Continental rises** (terrigenous aprons of turbidites and hemipelagic mud) on oceanic
//!   crust next to passive margins: the supply from the continent decays with distance and
//!   accumulates over the age of the sea floor. A trench between the continent and the sea floor
//!   traps it, so only sea floor on the continent's own plate gets an apron.
//! * **Passive-margin prisms** on flooded continental crust: thinned margin crust subsides, and
//!   sediment fills the space up to the shelf (accommodation fill, as the erosion routing does).
//! * **Platform cover** on the continental interior: sedimentary basins and platforms alternate
//!   with bare shields. The generated crustal thickness is taken as the depth of the Moho below
//!   the surface, as Earth's crustal thicknesses are measured (cover included), so the cover
//!   replaces basement of the same thickness (see [`PlatformEmplacement`]); its lower density
//!   raises the surface a little (~0.3 km for 1.8 km of cover).
//!
//! The cover is part of the initial crust mass (a stock, not a source), and the sea level is
//! solved with it. Earth's sedimentary shell is ~2.5–2.7e9 Gt (~1.1e9 km³), most of it in fold
//! belts, which the initial planet does not have; platforms (~165e6 km³), passive margins
//! (~95e6 km³; Ronov 1982) and the ocean floor (GlobSed, Straume et al. 2019: 3.4e8 km³ including
//! margins) hold ~1–1.4e9 Gt.

use super::*;
use crate::isostasy::{
    dry_response, sediment_bulk_thickness, sediment_dry_elevation, sediment_for_bulk_thickness,
    sediment_for_dry_elevation, sediment_for_rise,
};
use crate::math;

/// How the platform cover is placed on the continental basement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformEmplacement {
    /// On top of the basement: the surface rises.
    OnTop,
    /// Replacing basement of the same thickness: the generated crustal thickness is the depth
    /// of the Moho below the surface, as Earth's crustal thicknesses are measured (cover
    /// included). The surface rises by the buoyancy of the light cover.
    MohoDepth,
    /// Replacing basement such that the surface stays (the cover fills space made by
    /// subsidence of the basement); in this model the water-saturated cover is light, so
    /// ~2.5 m of basement go per m of cover.
    Elevation,
}

/// Parameters of the initial sediment cover. Rates are rock-equivalent thicknesses (mass /
/// [`SEDIMENT_ROCK_DENSITY`]) per Myr: 1 m/Myr ≈ 0.2 g/cm²/kyr of mass accumulation.
#[derive(Clone, Debug)]
pub struct InitialSedimentParams {
    /// Whether the initial planet has a sediment cover (otherwise it starts bare).
    pub enabled: bool,
    /// Depth (m) of the carbonate compensation depth: ~4.5 km in today's oceans (Pacific
    /// ~4.2–4.5, Atlantic ~5 km).
    pub ccd_depth: f64,
    /// Pelagic accumulation above the CCD (carbonate ooze plus clay; m/Myr): carbonate mass
    /// accumulation rates on ridge flanks are ~0.3–2 g/cm²/kyr.
    pub carbonate_rate: f64,
    /// Pelagic accumulation below the CCD (red clay, siliceous ooze; m/Myr): ~0.05–0.2
    /// g/cm²/kyr.
    pub clay_rate: f64,
    /// Whether the pelagic sediment is biogenic, as on today's Earth: carbonate ooze of
    /// coccoliths and planktic foraminifera (Jurassic and later; Ridgwell 2005) above the CCD,
    /// siliceous ooze of radiolaria and diatoms below it. Without life (`false`, the abiotic
    /// preset) the sea floor collects only red clay of wind-blown dust, volcanic ash and
    /// hydrogenous precipitates at `clay_rate`, at any depth.
    pub biogenic_pelagic: bool,
    /// Whether the initial sediment carries carbonate (#200, see [`crate::tectonics::Cohort`]):
    /// the continental cover a share `platform_carbonate` of its mass, biogenic pelagic sediment
    /// the ooze it accumulated above the CCD (beyond `clay_rate`); rises and red clay none. Off
    /// with the carbonate sediment (`carbon.carbonate_sediment=0`).
    pub carbonate: bool,
    /// Carbonate share of the initial continental cover (platforms and shelf prisms):
    /// Phanerozoic platforms are ~20–25% carbonate (Ronov 1980); without calcifiers the
    /// carbonate share of the burial flux, ~10%.
    pub platform_carbonate: f64,
    /// Terrigenous accumulation on the sea floor at a passive margin (m/Myr; turbidites and
    /// hemipelagic mud of continental rises accumulate at tens to hundreds of m/Myr), decaying
    /// exponentially with the distance from the continental crust over `terrigenous_length` (m).
    pub terrigenous_rate: f64,
    pub terrigenous_length: f64,
    /// Mean bulk thickness (m) of the platform cover where the interior is covered: ~3 km on
    /// Earth's cratonic platforms (Ronov 1982: ~55e6 km² holding ~165e6 km³).
    pub platform_thickness: f64,
    /// Fraction of the continental crust without platform cover (exposed shields).
    pub shield_fraction: f64,
    /// Platform cover is at most this many times `platform_thickness` (the deepest
    /// intracratonic basins hold ~5–8 km), and the basement below it stays at least as thick
    /// as at the coast of the initial continents (so it stays continental crust).
    pub max_platform_factor: f64,
    /// How the platform cover is placed (`--set initial_sediment.platform_emplacement=` 0 on
    /// top, 1 Moho depth, 2 elevation). In 4-seed runs over 1000 Myr, keeping the elevation
    /// removed 9% of the felsic crust and flooded the continents more (44% vs 29% at 1000 Myr);
    /// on top raised the mean land to ~1.3 km at t = 0; keeping the Moho depth (land ~1.0 km,
    /// felsic −4.5%) followed main within the scatter between seeds.
    pub platform_emplacement: PlatformEmplacement,
    /// Weight (0–1) of low bare elevation, against the noise, in where the platform cover
    /// lies (platforms and basins lie low, shields stand high). Off by default: low ground of
    /// the initial continents is thinned crust near the coasts, where the minimum basement
    /// leaves little room for cover (0.5 gives 0.6e9 instead of 0.8e9 Gt), and in 4-seed runs
    /// over 1000 Myr it changed neither the early loss of freeboard nor the long-term state
    /// beyond the scatter between seeds.
    pub lowland_weight: f64,
    /// Flooded continental crust is filled with sediment up to this elevation (m, the shelf;
    /// as [`crate::erosion::ErosionParams::shelf_depth`]).
    pub shelf_depth: f64,
    /// Sediment on oceanic crust raises its surface at most to this elevation (m; the
    /// continental slope, as [`crate::erosion::ErosionParams::slope_depth`]).
    pub slope_depth: f64,
}

impl Default for InitialSedimentParams {
    fn default() -> Self {
        Self {
            enabled: true,
            ccd_depth: 4500.0,
            carbonate_rate: 4.0,
            clay_rate: 1.0,
            biogenic_pelagic: true,
            carbonate: true,
            platform_carbonate: 0.2,
            terrigenous_rate: 40.0,
            terrigenous_length: 250e3,
            platform_thickness: 3000.0,
            shield_fraction: 0.4,
            max_platform_factor: 2.0,
            platform_emplacement: PlatformEmplacement::MohoDepth,
            lowland_weight: 0.0,
            shelf_depth: -120.0,
            slope_depth: -2000.0,
        }
    }
}

/// Frequency (per planet radius) and octaves of the noise that shapes the platform cover: basins
/// and arches of several hundred to a few thousand km.
const PLATFORM_NOISE_FREQUENCY: f64 = 3.0;
const PLATFORM_NOISE_OCTAVES: u32 = 4;

/// The rank of each value among `values` as a fraction in [0, 1] (ties by index).
fn ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_unstable_by(|&a, &b| values[a].total_cmp(&values[b]).then(a.cmp(&b)));
    let mut rank = vec![0.0; values.len()];
    let last = (values.len().max(2) - 1) as f64;
    for (position, &index) in order.iter().enumerate() {
        rank[index] = position as f64 / last;
    }
    rank
}

/// Mixed into the seed for the random numbers of the cover, so that they do not change the
/// rest of the initial conditions.
const SEED_SALT: u64 = 0x5ED1_3E47_C0FE_0001;

/// Iterations of the shelf fill and the sea level, which rises as the fill displaces water.
const MAX_SEA_LEVEL_ITERATIONS: usize = 100;

impl InitialSedimentParams {
    /// Pelagic sediment (rock-equivalent m) on sea floor of age `age` (Myr) that lay above the
    /// CCD until the age `ccd_age`.
    pub(super) fn pelagic(&self, age: f64, ccd_age: f64) -> f64 {
        let age = age.max(0.0);
        if !self.biogenic_pelagic {
            return self.clay_rate * age;
        }
        self.carbonate_rate * age.min(ccd_age) + self.clay_rate * (age - ccd_age).max(0.0)
    }

    /// Terrigenous sediment (rock-equivalent m) on sea floor of age `age` at the distance
    /// `distance` (m) from the continental crust of its plate.
    pub(super) fn terrigenous(&self, age: f64, distance: f64) -> f64 {
        if !distance.is_finite() {
            return 0.0;
        }
        self.terrigenous_rate * age.max(0.0) * math::exp(-distance / self.terrigenous_length)
    }
}

impl Simulation {
    /// The crust of a world cell, mutably.
    pub(super) fn surface_crust_mut(&mut self, cell: u32) -> &mut Crust {
        let owner = self.owners[cell as usize];
        self.plates[owner.plate as usize].crust[owner.local as usize]
            .as_mut()
            .expect("owners always point to existing crust")
    }

    /// The age (Myr) at which sea floor of normal oceanic crust sinks below the CCD at the
    /// current sea level.
    fn ccd_age(&self, ccd_depth: f64) -> f64 {
        let depth = |age: f64| -new_oceanic_crust(-age).surface_elevation(0.0, self.sea_level);
        let (mut low, mut high) = (0.0, 1000.0);
        if depth(low) >= ccd_depth {
            return 0.0;
        }
        if depth(high) < ccd_depth {
            return f64::INFINITY;
        }
        for _ in 0..60 {
            let middle = 0.5 * (low + high);
            if depth(middle) < ccd_depth {
                low = middle;
            } else {
                high = middle;
            }
        }
        0.5 * (low + high)
    }

    /// Covers the initial planet with sediment (see the module documentation). Called while
    /// the crust samples are still at their cells (`plate_of_cell`), with the initial
    /// `is_continental` cells, before the ledger and the sea level are initialised; leaves
    /// [`Simulation::sea_level`] at the solved sea level.
    pub(super) fn add_initial_sediment(
        &mut self,
        seed: u64,
        is_continental: &[bool],
        plate_of_cell: &[u32],
    ) {
        let params = self.params.initial_sediment.clone();
        let cell_count = self.mesh.len();
        let radius = self.params.planet_radius;
        // Sediment buried deeper than this would metamorphose at once.
        let max_sediment = sediment_for_bulk_thickness(self.params.metamorphic_depth);
        self.sea_level = self.solve_sea_level();

        // Sea floor: pelagic sediment plus continental rises on passive margins.
        let ccd_age = self.ccd_age(params.ccd_depth);
        let margin_distance = great_circle_distance_within(
            &self.mesh,
            |cell| is_continental[cell as usize],
            |a, b| plate_of_cell[a as usize] == plate_of_cell[b as usize],
        );
        for cell in 0..cell_count as u32 {
            if is_continental[cell as usize] {
                continue;
            }
            let sea_level = self.sea_level;
            let crust = self.surface_crust_mut(cell);
            let age = crust.age(0.0);
            let distance = margin_distance[cell as usize] * radius;
            // At most up to the continental slope, where the erosion routing stops filling.
            let room = sediment_for_rise(
                crust.surface_elevation(0.0, sea_level),
                params.slope_depth,
                0.0,
            )
            .max(0.0);
            let column = params.pelagic(age, ccd_age) + params.terrigenous(age, distance);
            crust.sediment = column.min(room).min(max_sediment);
            // The ooze's carbonate (beyond the clay that also falls above the CCD), as a share
            // of the column (#200).
            if params.carbonate && params.biogenic_pelagic && column > 0.0 {
                let ooze =
                    (params.carbonate_rate - params.clay_rate).max(0.0) * age.max(0.0).min(ccd_age);
                crust.cohort.carbonate = (ooze / column).clamp(0.0, 1.0);
            }
        }

        // Platform cover: a noise field over the continents; the lowest `shield_fraction` stays
        // bare, the rest is covered in proportion to the noise above that threshold.
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ SEED_SALT);
        let noise = GradientNoise::new(&mut rng);
        let offset = DVec3::new(
            rng.random_range(0.0..256.0),
            rng.random_range(0.0..256.0),
            rng.random_range(0.0..256.0),
        );
        let continental: Vec<u32> = (0..cell_count as u32)
            .filter(|&cell| is_continental[cell as usize])
            .collect();
        // Platforms and basins lie low, shields stand high (and lose any cover first): the
        // field mixes the rank of the noise with the inverted rank of the bare elevation.
        let noise_rank = ranks(
            &continental
                .iter()
                .map(|&cell| {
                    noise.fbm(
                        self.mesh.vertex(cell) * PLATFORM_NOISE_FREQUENCY + offset,
                        PLATFORM_NOISE_OCTAVES,
                        0.5,
                        DVec3::new(5.3, 9.1, 2.7),
                    )
                })
                .collect::<Vec<f64>>(),
        );
        let elevation_rank = ranks(
            &continental
                .iter()
                .map(|&cell| self.surface_elevation(cell))
                .collect::<Vec<f64>>(),
        );
        let field: Vec<f64> = noise_rank
            .iter()
            .zip(&elevation_rank)
            .map(|(&noise, &elevation)| {
                (1.0 - params.lowland_weight) * noise + params.lowland_weight * (1.0 - elevation)
            })
            .collect();
        let mut sorted = field.clone();
        sorted.sort_unstable_by(f64::total_cmp);
        let threshold = sorted
            .get((params.shield_fraction.clamp(0.0, 1.0) * sorted.len() as f64) as usize)
            .copied()
            .unwrap_or(f64::INFINITY);
        let excess: Vec<f64> = field.iter().map(|&n| (n - threshold).max(0.0)).collect();
        let max_platform = params.max_platform_factor * params.platform_thickness;
        // Scale so that the covered cells have the mean thickness, with the cap applied.
        let covered = excess.iter().filter(|&&e| e > 0.0).count().max(1) as f64;
        let mean_excess = excess.iter().sum::<f64>() / covered;
        let mut scale = if mean_excess > 0.0 {
            params.platform_thickness / mean_excess
        } else {
            0.0
        };
        for _ in 0..50 {
            let mean = excess
                .iter()
                .map(|&e| (scale * e).min(max_platform))
                .sum::<f64>()
                / covered;
            if mean <= 0.0 || (mean - params.platform_thickness).abs() < 1e-6 {
                break;
            }
            scale *= params.platform_thickness / mean;
        }
        let platform: Vec<f64> = excess
            .iter()
            .map(|&e| sediment_for_bulk_thickness((scale * e).min(max_platform)))
            .collect();

        // Continental crust: platform cover where the bare surface lies above the shelf, a
        // prism filled up to the shelf below it (classified once, at the sea level of the bare
        // continents). Under the platform cover the basement thins (see `PlatformEmplacement`), but
        // stays at least as thick as at the coast.
        let min_basement = self.params.continental_coast_thickness;
        let mut bare: Vec<Crust> = Vec::with_capacity(continental.len());
        let mut on_platform = vec![false; continental.len()];
        for (index, &cell) in continental.iter().enumerate() {
            let sea_level = self.sea_level;
            let crust = self.surface_crust_mut(cell);
            bare.push(*crust);
            if crust.surface_elevation(0.0, sea_level) >= params.shelf_depth {
                on_platform[index] = true;
                let room = (crust.thickness - min_basement).max(0.0);
                let response = dry_response(crust.removal_isostatic_density());
                match params.platform_emplacement {
                    PlatformEmplacement::OnTop => crust.sediment = platform[index],
                    PlatformEmplacement::MohoDepth => {
                        crust.sediment = platform[index].min(sediment_for_bulk_thickness(room));
                        crust.thickness -= sediment_bulk_thickness(crust.sediment);
                    }
                    PlatformEmplacement::Elevation => {
                        crust.sediment =
                            platform[index].min(sediment_for_dry_elevation(room * response));
                        crust.thickness -= sediment_dry_elevation(crust.sediment) / response;
                    }
                }
            }
        }
        // The prism displaces water and raises the sea level, which deepens the shelf again:
        // iterate to the fixed point.
        for _ in 0..MAX_SEA_LEVEL_ITERATIONS {
            for (index, &cell) in continental.iter().enumerate() {
                if on_platform[index] {
                    continue;
                }
                let mut crust = bare[index];
                let elevation = crust.surface_elevation(0.0, self.sea_level);
                if elevation < params.shelf_depth {
                    crust.sediment =
                        sediment_for_rise(elevation, params.shelf_depth, 0.0).min(max_sediment);
                }
                *self.surface_crust_mut(cell) = crust;
            }
            let sea_level = self.solve_sea_level();
            let change = sea_level - self.sea_level;
            self.sea_level = sea_level;
            if change.abs() < 1e-3 {
                break;
            }
        }
        if params.carbonate {
            let share = params.platform_carbonate.clamp(0.0, 1.0);
            for &cell in &continental {
                let crust = self.surface_crust_mut(cell);
                if crust.sediment > 0.0 {
                    crust.cohort.carbonate = share;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::sediment_bulk_thickness;

    fn simulation(seed: u64, enabled: bool) -> Simulation {
        let mut params = TectonicParams::default();
        params.initial_sediment.enabled = enabled;
        Simulation::new(SphereMesh::icosphere(5), 8, params, seed)
    }

    /// Sediment mass (Gt) of the visible crust on an Earth-sized planet, split into continental
    /// and oceanic crust.
    fn stock(simulation: &Simulation) -> (f64, f64) {
        let cell_area = 4.0 * std::f64::consts::PI * simulation.params.planet_radius.powi(2)
            / simulation.mesh.len() as f64;
        let mut stock = (0.0, 0.0);
        for cell in 0..simulation.mesh.len() as u32 {
            let crust = simulation.surface_crust(cell);
            let mass = crust.sediment * SEDIMENT_ROCK_DENSITY * cell_area / 1e12;
            if crust.is_oceanic() {
                stock.1 += mass;
            } else {
                stock.0 += mass;
            }
        }
        stock
    }

    #[test]
    fn disabled_cover_leaves_the_initial_planet_bare() {
        let (with, without) = (simulation(4, true), simulation(4, false));
        for cell in 0..without.mesh.len() as u32 {
            assert_eq!(without.surface_crust(cell).sediment, 0.0);
            // The rest of the initial conditions does not change.
            assert_eq!(with.owner(cell), without.owner(cell));
            let (a, b) = (with.surface_crust(cell), without.surface_crust(cell));
            assert_eq!(a.birth, b.birth);
            assert_eq!(a.is_oceanic(), b.is_oceanic());
        }
    }

    #[test]
    fn the_initial_stock_is_earth_like() {
        for seed in 0..3 {
            let (continental, oceanic) = stock(&simulation(seed, true));
            // Platforms and margins ~0.6–1e9 Gt, ocean floor ~0.2–0.5e9 Gt.
            assert!(
                (5e8..1.3e9).contains(&continental),
                "continental {continental:e}"
            );
            assert!((1.5e8..6e8).contains(&oceanic), "oceanic {oceanic:e}");
        }
    }

    #[test]
    fn abiotic_pelagic_sediment_is_clay_only() {
        let biogenic = InitialSedimentParams::default();
        let abiotic = InitialSedimentParams {
            biogenic_pelagic: false,
            ..InitialSedimentParams::default()
        };
        // Sea floor that lay above the CCD for 40 of its 100 Myr.
        assert_eq!(biogenic.pelagic(100.0, 40.0), 4.0 * 40.0 + 1.0 * 60.0);
        assert_eq!(abiotic.pelagic(100.0, 40.0), 100.0);
        assert_eq!(abiotic.pelagic(100.0, f64::INFINITY), 100.0);
        assert_eq!(abiotic.pelagic(-5.0, 0.0), 0.0);
    }

    #[test]
    fn sea_floor_sediment_thickens_with_age_from_bare_ridges() {
        let params = InitialSedimentParams::default();
        let simulation = simulation(1, true);
        let ccd_age = simulation.ccd_age(params.ccd_depth);
        assert!((10.0..80.0).contains(&ccd_age), "CCD at {ccd_age} Myr");
        assert_eq!(params.pelagic(0.0, ccd_age), 0.0);
        let mut previous = 0.0;
        for age in [1.0, 10.0, 30.0, 60.0, 100.0, 180.0] {
            let pelagic = params.pelagic(age, ccd_age);
            assert!(pelagic > previous);
            previous = pelagic;
        }
        // Open-ocean pelagic cover on the oldest sea floor: a few hundred metres.
        let oldest = sediment_bulk_thickness(params.pelagic(180.0, ccd_age));
        assert!((250.0..900.0).contains(&oldest), "{oldest} m");
        // Continental rises: km thick at the foot of an old margin, thin far away.
        let foot = sediment_bulk_thickness(params.terrigenous(150.0, 100e3));
        let far = sediment_bulk_thickness(params.terrigenous(150.0, 1000e3));
        assert!((2000.0..8000.0).contains(&foot), "{foot} m");
        assert!(far < 200.0, "{far} m");

        // In the world: the youngest sea floor is (nearly) bare, old sea floor is covered.
        let mut young = Vec::new();
        let mut old = Vec::new();
        for cell in 0..simulation.mesh.len() as u32 {
            let crust = simulation.surface_crust(cell);
            if crust.is_oceanic() {
                let age = crust.age(0.0);
                if age < 2.0 {
                    young.push(crust.sediment);
                } else if age > 120.0 {
                    old.push(crust.sediment);
                }
            }
        }
        // (Next to a continent, young sea floor already has a little terrigenous mud.)
        let young_max = params.pelagic(2.0, ccd_age) + params.terrigenous(2.0, 0.0);
        assert!(young.iter().all(|&s| s <= young_max));
        // Sea-floor cover stays below the continental slope and the metamorphic depth.
        let slope = params.slope_depth;
        for cell in 0..simulation.mesh.len() as u32 {
            let crust = simulation.surface_crust(cell);
            if crust.is_oceanic() && crust.sediment > 0.0 {
                let bare = Crust {
                    sediment: 0.0,
                    ..crust
                };
                let floor = bare.surface_elevation(0.0, simulation.sea_level);
                if floor < slope {
                    assert!(simulation.surface_elevation(cell) <= slope + 50.0);
                }
                assert!(
                    sediment_bulk_thickness(crust.sediment)
                        <= simulation.params.metamorphic_depth + 1.0
                );
            }
        }
        // No terrigenous apron without a continent on the plate.
        assert_eq!(params.terrigenous(100.0, f64::INFINITY), 0.0);
        assert!(young.iter().filter(|&&s| s < 20.0).count() * 2 > young.len());
        assert!(old.iter().all(|&s| s > 150.0));
    }

    #[test]
    fn continental_margins_and_platforms_are_covered() {
        let (with, without) = (simulation(2, true), simulation(2, false));
        let shelf = with.params.initial_sediment.shelf_depth;
        let mut shelf_cells = 0;
        let mut covered = 0;
        let mut continental = 0;
        for cell in 0..with.mesh.len() as u32 {
            let bare = without.surface_crust(cell);
            if bare.is_oceanic() {
                continue;
            }
            continental += 1;
            let crust = with.surface_crust(cell);
            let elevation = with.surface_elevation(cell);
            // No cover reaches the depth of metamorphism.
            assert!(sediment_bulk_thickness(crust.sediment) <= with.params.metamorphic_depth + 1.0);
            // The cover does not change what the crust is.
            assert_eq!(crust.is_continental(), bare.is_continental());
            assert_eq!(crust.is_continental_origin(), bare.is_continental_origin());
            assert!(
                crust.thickness
                    >= with.params.continental_coast_thickness.min(bare.thickness) - 1e-6
            );
            assert!(
                sediment_bulk_thickness(crust.sediment)
                    <= with.params.initial_sediment.platform_thickness
                        * with.params.initial_sediment.max_platform_factor
                        + 1.0
                    || crust.thickness == bare.thickness
            );
            if crust.thickness < bare.thickness {
                // Platform cover keeps the Moho depth and raises the surface a little.
                covered += 1;
                let moho = crust.thickness + sediment_bulk_thickness(crust.sediment);
                assert!(
                    (moho - bare.thickness).abs() < 1e-6,
                    "{moho} vs {}",
                    bare.thickness
                );
                let bare_elevation = bare.surface_elevation(0.0, with.sea_level);
                let rise = elevation - bare_elevation;
                assert!((0.0..1000.0).contains(&rise), "rise {rise}");
                assert!(crust.is_continental_origin());
            } else if crust.sediment > 0.0 {
                // Flooded margins are filled to the shelf.
                shelf_cells += 1;
                assert!((elevation - shelf).abs() < 5.0, "shelf at {elevation} m");
            }
        }
        assert!(shelf_cells > continental / 20, "{shelf_cells} shelf cells");
        let covered_fraction = covered as f64 / continental as f64;
        assert!(
            (0.3..0.7).contains(&covered_fraction),
            "{covered_fraction} covered"
        );
    }

    #[test]
    fn the_cover_raises_the_sea_level_and_is_initial_stock() {
        let (with, without) = (simulation(5, true), simulation(5, false));
        // Sediment on the sea floor and the shelves displaces water.
        assert!(with.sea_level > without.sea_level);
        // The ledger starts from the covered planet.
        let ledger = with.ledger.checked(with.total_mass());
        assert!(ledger.residual.abs() < 1e-12 * with.total_mass());
        assert!(with.total_mass() > 0.0);
    }

    #[test]
    fn the_cover_is_deterministic_per_seed() {
        let (a, b, c) = (
            simulation(6, true),
            simulation(6, true),
            simulation(7, true),
        );
        let sediment = |s: &Simulation| -> Vec<f64> {
            (0..s.mesh.len() as u32)
                .map(|cell| s.surface_crust(cell).sediment)
                .collect()
        };
        assert_eq!(sediment(&a), sediment(&b));
        assert_ne!(sediment(&a), sediment(&c));
    }
}
