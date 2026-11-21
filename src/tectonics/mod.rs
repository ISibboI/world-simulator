//! Rigid-plate tectonics on the unit sphere.
//!
//! Each plate is a rigid body rotating around its own Euler pole. A plate stores its crust in a
//! plate-local frame, indexed by the vertices of the shared [`SphereMesh`]. The world position of
//! local sample `j` of plate `p` is `p.rotation * mesh.vertex(j)`.
//!
//! In every substep, the plates are rotated, and then each world cell is assigned to the plate
//! whose crust covers it:
//!
//! * If no plate covers a cell, the plates diverge there, and new oceanic crust is created on one
//!   of the adjacent plates (sea-floor spreading).
//! * If several plates cover a cell, they converge there. One plate overrides:
//!   - Oceanic (thin, dense) crust of the other plates subducts and is destroyed. Melting above
//!     the slab adds new arc crust in a band behind the trench on the overriding plate (volcanic
//!     arcs, Andean margins).
//!   - Continental (thick, buoyant) crust cannot subduct. Where two continental crusts collide,
//!     the crust of the lower plate is stacked onto the overriding plate around the collision
//!     (crustal thickening, orogeny, terrane accretion), so continental crust is conserved.
//!
//! Every crust sample stores its thickness, density, time of formation and sediment cover; its
//! elevation follows from Airy isostasy plus the thermal subsidence of the lithosphere with age
//! (see [`crate::isostasy`]). Continental crust is not a separate type: crust counts as continental
//! when it is thick and light enough (see [`Crust::is_continental`]), so island arcs gradually
//! become continental crust as they thicken.
//!
//! Which plate would subduct is decided per segment of the boundary between two plates when they
//! first touch there, from the buoyancy of their crust or a neighbouring segment, and then kept
//! (subduction polarity, see [`polarity`]). Continental crust arriving at a trench stays on top,
//! and the segment flips once it keeps arriving. Where the plates converge, a segment's boundary
//! is a trench, with a slab that grows as it subducts.
//!
//! Plate velocities follow from a balance of forces on each plate (slab pull, ridge push, drag
//! against the mantle flow, collision resistance; see [`dynamics`]), with mantle flow from sinking
//! slabs and plumes ([`mantle`]). Plates are born by rifting above plumes and by subduction
//! initiation at old passive margins, and disappear by subduction, suturing after continental
//! collisions, or capture of tiny plates by a neighbour ([`events`]).

mod boundaries;
mod crust;
mod deformation;
mod dynamics;
mod events;
mod generation;
mod initial_sediment;
mod ledger;
mod mantle;
mod matching;
mod polarity;
mod statistics;
mod topology;
mod weak_zones;

pub use boundaries::{GapStatistics, TrenchSediment};
pub use crust::*;
pub use events::{EventCounts, PlateEventParams, WEAK_BELT_THRESHOLD};
pub use generation::InitialConfiguration;
pub use initial_sediment::{InitialSedimentParams, PlatformEmplacement};
pub use ledger::MassLedger;
pub use mantle::{MantleParams, Plume};
pub use matching::{MatchingParams, MatchingStatistics};
pub use statistics::*;

use boundaries::{METASEDIMENT_DENSITY, ResolvedBoundaries, pair_key};
use generation::*;

pub(crate) use crate::hash::{HashMap, HashSet};

use glam::{DQuat, DVec3};
use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use crate::isostasy::{CONTINENTAL_DENSITY, SEDIMENT_ROCK_DENSITY};
use crate::math;
use crate::sphere::SphereMesh;

/// Physical parameters of the simulation. Times are in Myr, elevations in metres.
#[derive(Clone, Debug)]
pub struct TectonicParams {
    /// Fraction of the initial plate boundaries that start as subduction zones.
    pub initial_subduction_fraction: f64,
    /// The fraction of the surface initially covered by continental crust.
    pub continental_fraction: f64,
    /// The number of large initial continents (cratons of the continent field; noise adds
    /// smaller fragments).
    pub continent_count: usize,
    /// Arrangement of the initial continents.
    pub initial: InitialConfiguration,
    /// Crust thickness at the edges of the initial continents in metres (thinned, partly
    /// flooded margins).
    pub continental_coast_thickness: f64,
    /// Great-circle distance (m) from the coast of the initial continents over which the crust
    /// thickens to the interior thickness (quadratically, most of it near the coast).
    pub continental_margin_width: f64,
    /// Crust thickness in the interior of the initial continents in metres.
    pub continental_interior_thickness: f64,
    /// Sediment cover of the initial planet.
    pub initial_sediment: InitialSedimentParams,
    /// Thickness (m) of sediment that the subduction channel carries down with the slab; the
    /// trench fill above it is scraped off into an accretionary wedge of the overriding plate
    /// (frontal accretion). Margins with more than ~1 km of trench fill are accretionary,
    /// thinner fill is subducted (von Huene & Scholl 1991, Clift & Vannucchi 2004). Sediment
    /// recycled through arc magmas is part of the arc crust.
    pub subduction_channel_thickness: f64,
    /// At accretionary margins, the fraction of the incoming sediment column above the
    /// décollement, scraped off into the accretionary wedge (the subduction channel always
    /// subducts). Thick-fill margins accrete about half of the column frontally (Nankai,
    /// Makran, Cascadia, Barbados).
    pub decollement_fraction: f64,
    /// A trench segment becomes accretionary when its time-averaged fill exceeds the
    /// subduction channel and its trench-normal convergence is below
    /// `accretionary_convergence` (cm/yr), and erosive again when the fill drops or the
    /// convergence exceeds `erosive_convergence` (Clift & Vannucchi 2004: accretion below
    /// ~7.6 cm/yr with > 1 km fill, erosion above ~6 cm/yr or with < 1 km fill).
    pub accretionary_convergence: f64,
    pub erosive_convergence: f64,
    /// Time (Myr) over which the trench fill of a segment is averaged.
    pub trench_fill_time: f64,
    /// Volume of new arc crust per subducted crust sample, as a thickness over one cell.
    ///
    /// Arcs grow by roughly 30–90 km³ per km of arc and Myr on Earth; at 50 km/Myr convergence
    /// and ~110 km cells, one subducted cell corresponds to roughly 1e13 m³ ≈ 800 m over a cell.
    pub arc_crust_per_subduction: f64,
    /// Thickness of crust (and sediment) removed from the overriding plate at the trench per
    /// subducted crust sample (subduction erosion), as a thickness over one cell.
    pub subduction_erosion: f64,
    /// Felsic fraction (see [`Crust::felsic`]) of arc crust built on normal oceanic crust:
    /// young intra-oceanic arcs are basaltic to andesitic (Izu–Bonin–Mariana, Tonga). Arcs on
    /// thickened arc or continental crust are fully felsic, so intra-oceanic arcs mature into
    /// continental crust once they have thickened.
    pub oceanic_arc_felsic: f64,
    /// Sediment buried deeper than this (bulk thickness, m) metamorphoses into felsic crust (greenschist facies, ~250–300 °C at 10–12 km under
    /// sediment-blanketed margins).
    pub metamorphic_depth: f64,
    /// Relative amounts of new arc crust at distances of 0, 1, 2, … cells from the trench:
    /// none in the amagmatic forearc, most at the volcanic front ~100–150 km behind the trench.
    pub arc_profile: Vec<f64>,
    /// Relative amounts of accreted sediment (accretionary wedge) at distances of 0, 1, …
    /// cells from the trench.
    pub accretion_profile: Vec<f64>,
    /// Relative amounts of stacked crust at distances of 0, 1, 2, … cells from a continental
    /// collision.
    pub collision_profile: Vec<f64>,
    /// Width (m) of the band of the overriding plate behind a continental collision that takes
    /// up the shortening (see [`deformation`]): the overridden crust is distributed over it by
    /// [`deformation::band_weight`] instead of by [`Self::collision_profile`]. Convergence
    /// between continents is taken up over zones ~200 km (Alps) to > 1000 km (Tibet) wide;
    /// 500 km is about half of Tibet's width behind the Himalayan front. 0 restores the profile.
    pub collision_band_width: f64,
    /// Factor on the resistance of a collision belt to further convergence from its excess
    /// gravitational potential energy (see [`deformation::excess_potential_energy`]); 1 is the
    /// physical value, 0 disables it.
    pub belt_gpe_resistance: f64,
    /// Crust thicker than this is removed from below (delamination).
    pub max_crust_thickness: f64,
    /// Crust thicker than this flows sideways under its own weight (lower-crustal flow,
    /// orogenic collapse), spreading thickened crust over a larger area; its mobility grows with
    /// the cube of the thickness above it.
    pub crustal_flow_threshold: f64,
    /// Thickness range above the threshold over which crustal flow reaches full strength.
    pub crustal_flow_range: f64,
    /// Diffusivity (km²/Myr) of orogenic collapse at full mobility: lower-crustal channel flow
    /// `ρ g h³ / (12 η)` times the isostatic response, ~3.7·10³ km²/Myr for a 15 km channel at
    /// η = 10¹⁹ Pa·s and ~3.7·10⁴ at 10¹⁸ (Clark & Royden 2000).
    pub collapse_diffusivity: f64,
    /// Planet radius in metres.
    pub planet_radius: f64,
    /// Effective slab pull per metre of trench at the reference slab age, in N/m. The gross
    /// negative buoyancy of a slab is several times larger, but most of it is balanced by the
    /// viscous resistance of the mantle to the sinking slab.
    pub slab_pull: f64,
    /// Slab age in Myr at which slab pull reaches [`Self::slab_pull`]; it scales with the square
    /// root of the age, like the thickness of the lithosphere.
    pub slab_pull_reference_age: f64,
    /// Age (Myr) at which oceanic lithosphere is as dense as the asthenosphere below it: younger
    /// lithosphere is buoyant (its crust and depleted mantle outweigh the thermal contraction)
    /// and resists subduction, older lithosphere pulls (~10 Myr, Cloos 1993; 10–30 Myr in other
    /// estimates, Oxburgh & Parmentier 1977; Afonso et al. 2007).
    pub neutral_buoyancy_age: f64,
    /// Lithosphere stops thickening after this age in Myr.
    pub slab_pull_max_age: f64,
    /// Plates must separate at least this fast (cm/yr) for a gap between them to become new sea
    /// floor; slower gaps are resampling artifacts. The slowest ridges spread at ~0.6–1.5 cm/yr.
    pub min_spreading_rate: f64,
    /// Continental crust that opens at least this fast (cm/yr), also below
    /// [`Self::min_spreading_rate`], is stretched (continental rifts open at 0.1–1 cm/yr).
    pub min_stretching_rate: f64,
    /// Width (m) of the zone on each side of a continental rift over which the crust is
    /// stretched (passive margins are 100–300 km wide).
    pub rift_width: f64,
    /// Continental crust thinner than this (m) breaks up: the rift opens into sea floor.
    pub breakup_thickness: f64,
    /// Plates must converge at least this fast (cm/yr) for a new trench to act (an existing slab
    /// keeps acting until the plates diverge faster than this).
    pub min_trench_convergence: f64,
    /// Smallest angle (degrees) between the relative motion and the strike of a boundary segment
    /// at which its lower plate subducts. Closer to the strike, the normal component cannot feed
    /// a slab: the contact is a transform (western Aleutians, Queen Charlotte margin), and a slab
    /// already there decays and breaks off (see [`Self::slab_decay_time`]). 20° is also the
    /// tolerance with which PB2002 (Bird 2003) classes boundaries as strike-slip.
    pub min_trench_obliquity: f64,
    /// Resistance (N per metre of boundary) of oceanic lithosphere to starting to subduct: the
    /// plate must be bent and a new thrust fault must break through it before a slab pulls. It
    /// opposes convergence at trenches without a slab and fades as the slab grows to
    /// [`Self::self_sustaining_slab_length`]. Initiation needs ~2–4 × 10¹² N/m of compression,
    /// more than ridge push alone supplies (McKenzie 1977; Mueller & Phillips 1991; Toth &
    /// Gurnis 1998; Gurnis et al. 2004), so most converging contacts of plates that are not
    /// forced together stay locked. Zero disables it.
    pub initiation_resistance: f64,
    /// Slab length (m) at which subduction sustains itself: the initiation resistance is gone
    /// (after ~100–150 km of convergence; Gurnis et al. 2004).
    pub self_sustaining_slab_length: f64,
    /// Convergence speed (cm/yr) below which a contact resisting subduction initiation is
    /// locked (the resisting force then falls linearly with the speed).
    pub locked_convergence: f64,
    /// Convergence speed (cm/yr) at which a diffuse convergent zone (no through-going thrust
    /// yet) reaches the strength [`Self::initiation_resistance`]. Below the strength, the zone creeps by distributed
    /// shortening at a speed proportional to the force; at the strength it yields, localises on
    /// a thrust and starts to subduct. Earth's diffuse oceanic convergent zones shorten at
    /// 0.25–0.6 cm/yr (Central Indian Basin: Bull & Scrutton 1992; Gordon et al. 1998), and
    /// three quarters of the length of PB2002's oceanic convergent boundaries without subduction
    /// converge slower than 1.4 cm/yr (Bird 2003): strain rates of ~10⁻¹⁶–10⁻¹⁵ s⁻¹ over a few
    /// hundred km.
    pub diffuse_convergence: f64,
    /// Slab pull reaches full strength once this much slab (m) has subducted (about the depth
    /// of the upper mantle; Conrad & Lithgow-Bertelloni 2002).
    pub max_slab_length: f64,
    /// Speed (cm/yr) at which slab resistance balances full slab pull of reference-age
    /// lithosphere: the fastest that slab pull alone can move a plate towards its trench
    /// relative to the mantle. Earth's fastest subducting plates move at ~8–10 cm/yr.
    pub slab_sinking_speed: f64,
    /// A decaying slab shorter than this (m) has broken off: the trench stops acting.
    pub min_slab_length: f64,
    /// Time constant (Myr) for a slab to sink away or break off once subduction stops.
    pub slab_decay_time: f64,
    /// Ridge push per metre of ridge in N/m.
    pub ridge_push: f64,
    /// Oceanic crust up to this age in Myr at a divergent boundary counts as a ridge flank.
    pub ridge_age: f64,
    /// Basal drag coefficient `c` in Pa·s/m: shear stress per relative velocity between plate
    /// and mantle (asthenosphere viscosity divided by its thickness).
    pub basal_drag: f64,
    /// Basal drag is this many times stronger under continental crust (deep lithospheric keels).
    pub continental_drag_factor: f64,
    /// Resistance to converging continental crust per collision boundary cell, as a multiple of
    /// the basal drag under one cell (normal motion only; strike-slip is resisted like at
    /// other boundaries, see [`Self::boundary_coupling_width`]). Calibrated (2026-11-17, after
    /// local splits and terrane accretion) on continental thickness, mean land and collision
    /// stacking: 50 (~2–3e12 N/m at 1–2 cm/yr, below Tibet's ~5–8e12; mean land 0.6–0.8 km,
    /// 2–6% > 50 km, stacking 13–23 Gt/yr; 30–100 give similar land within seed scatter).
    /// That force stood in for the potential energy of thickened belts, which
    /// [`Self::belt_gpe_resistance`] now models explicitly; recalibrated with it (#69, PR #81):
    /// 0 (mean land 0.80 km, 4.5% > 50 km, stacking 22 Gt/yr; 5–50 give 0.63–0.72 km), so
    /// collisions are resisted by the belts they build, not by a viscous stand-in.
    pub collision_resistance: f64,
    /// Resistance of plate boundaries to strike-slip motion along them: each
    /// metre of boundary resists like the basal drag under this many metres times a metre.
    /// Transform faults are weak (≈ 20 MPa over ≈ 15 km brittle depth at 5 cm/yr: ≈ 200 km;
    /// Forsyth & Uyeda 1975 found transform resistance small).
    pub boundary_coupling_width: f64,
    /// Whether to measure the material budget per step phase (see [`MaterialBudget`]); costs
    /// several passes over all crust samples per step.
    pub track_material_budget: bool,
    /// Volume of the water in the ocean in m³ (Earth: 1.335e18), which sets the sea level: open
    /// water plus the pore water of submerged sediment (which comes out of the ocean).
    pub ocean_volume: f64,
    /// Mantle flow below the plates.
    pub mantle: MantleParams,
    /// How plates are matched to cells.
    pub matching: MatchingParams,
    /// Rifting, subduction initiation and suturing.
    pub events: PlateEventParams,
    /// Boundary segments with their own subduction polarity cover this distance (m) around their
    /// position (at least three cell spacings).
    pub segment_radius: f64,
    /// A boundary segment flips its polarity once continental crust has been arriving along
    /// most of it for this long (Myr).
    pub polarity_flip_time: f64,
    /// Subduction polarity between two plates is forgotten after they were not in contact for
    /// this long.
    pub polarity_memory: f64,
    /// Maximum fraction of a cell spacing that a plate may move in one substep.
    pub max_substep_movement: f64,
}

impl Default for TectonicParams {
    fn default() -> Self {
        Self {
            initial_subduction_fraction: 0.5,
            continental_fraction: 0.4,
            continent_count: 7,
            initial: InitialConfiguration::Dispersed,
            continental_coast_thickness: 22_000.0,
            continental_interior_thickness: 35_000.0,
            continental_margin_width: 1000e3,
            initial_sediment: InitialSedimentParams::default(),
            subduction_channel_thickness: 1000.0,
            decollement_fraction: 0.45,
            accretionary_convergence: 6.0,
            erosive_convergence: 7.6,
            trench_fill_time: 10.0,
            arc_crust_per_subduction: 500.0,
            subduction_erosion: 300.0,
            oceanic_arc_felsic: 0.4,
            metamorphic_depth: 11_000.0,
            arc_profile: vec![0.0, 1.0, 0.5],
            accretion_profile: vec![1.0, 0.5],
            collision_profile: vec![1.0, 0.75, 0.5, 0.25],
            collision_band_width: 500e3,
            belt_gpe_resistance: 1.0,
            max_crust_thickness: 75_000.0,
            crustal_flow_threshold: 40_000.0,
            crustal_flow_range: 20_000.0,
            collapse_diffusivity: 1e4,
            planet_radius: 6.371e6,
            slab_pull: 2e13,
            slab_pull_reference_age: 100.0,
            neutral_buoyancy_age: 10.0,
            slab_pull_max_age: 120.0,
            min_spreading_rate: 0.5,
            min_trench_convergence: 0.1,
            min_trench_obliquity: 20.0,
            min_stretching_rate: 0.15,
            rift_width: 200e3,
            breakup_thickness: 15e3,
            initiation_resistance: 3e12,
            self_sustaining_slab_length: 150e3,
            locked_convergence: 0.1,
            diffuse_convergence: 1.0,
            max_slab_length: 600e3,
            slab_decay_time: 10.0,
            min_slab_length: 20e3,
            slab_sinking_speed: 10.0,
            ridge_push: 3e12,
            ridge_age: 20.0,
            basal_drag: 1e15,
            continental_drag_factor: 4.0,
            collision_resistance: 0.0,
            boundary_coupling_width: 200e3,
            track_material_budget: false,
            ocean_volume: 1.335e18,
            mantle: MantleParams::default(),
            matching: MatchingParams::default(),
            events: PlateEventParams::default(),
            polarity_memory: 20.0,
            segment_radius: 500e3,
            polarity_flip_time: 3.0,
            max_substep_movement: 0.5,
        }
    }
}

impl TectonicParams {
    /// Sets the tunable parameter `name` (the field name, with `events.` for
    /// [`PlateEventParams`] fields and `initial_sediment.` for [`InitialSedimentParams`]
    /// fields; `initial_sediment.enabled` and `initial_sediment.biogenic_pelagic` are off for 0 and
    /// on otherwise) to `value`.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        let sediment = &mut self.initial_sediment;
        let field = match name {
            "initial_sediment.enabled" => {
                sediment.enabled = value != 0.0;
                return Ok(());
            }
            "initial_sediment.biogenic_pelagic" => {
                sediment.biogenic_pelagic = value != 0.0;
                return Ok(());
            }
            "initial_sediment.platform_emplacement" => {
                sediment.platform_emplacement = match value {
                    0.0 => PlatformEmplacement::OnTop,
                    1.0 => PlatformEmplacement::MohoDepth,
                    2.0 => PlatformEmplacement::Elevation,
                    _ => anyhow::bail!("platform_emplacement is 0, 1 or 2"),
                };
                return Ok(());
            }
            "initial_sediment.ccd_depth" => &mut sediment.ccd_depth,
            "initial_sediment.carbonate_rate" => &mut sediment.carbonate_rate,
            "initial_sediment.clay_rate" => &mut sediment.clay_rate,
            "initial_sediment.terrigenous_rate" => &mut sediment.terrigenous_rate,
            "initial_sediment.terrigenous_length" => &mut sediment.terrigenous_length,
            "initial_sediment.platform_thickness" => &mut sediment.platform_thickness,
            "initial_sediment.shield_fraction" => &mut sediment.shield_fraction,
            "initial_sediment.max_platform_factor" => &mut sediment.max_platform_factor,
            "initial_sediment.lowland_weight" => &mut sediment.lowland_weight,
            "initial_sediment.shelf_depth" => &mut sediment.shelf_depth,
            "initial_sediment.slope_depth" => &mut sediment.slope_depth,
            "basal_drag" => &mut self.basal_drag,
            "continental_drag_factor" => &mut self.continental_drag_factor,
            "collision_resistance" => &mut self.collision_resistance,
            "collision_band_width" => &mut self.collision_band_width,
            "belt_gpe_resistance" => &mut self.belt_gpe_resistance,
            "initiation_resistance" => &mut self.initiation_resistance,
            "neutral_buoyancy_age" => &mut self.neutral_buoyancy_age,
            "self_sustaining_slab_length" => &mut self.self_sustaining_slab_length,
            "locked_convergence" => &mut self.locked_convergence,
            "min_trench_convergence" => &mut self.min_trench_convergence,
            "min_trench_obliquity" => &mut self.min_trench_obliquity,
            "diffuse_convergence" => &mut self.diffuse_convergence,
            "collapse_diffusivity" => &mut self.collapse_diffusivity,
            "boundary_coupling_width" => &mut self.boundary_coupling_width,
            "slab_pull" => &mut self.slab_pull,
            "slab_sinking_speed" => &mut self.slab_sinking_speed,
            "ocean_volume" => &mut self.ocean_volume,
            "rift_width" => &mut self.rift_width,
            "breakup_thickness" => &mut self.breakup_thickness,
            "min_stretching_rate" => &mut self.min_stretching_rate,
            "continental_coast_thickness" => &mut self.continental_coast_thickness,
            "continental_interior_thickness" => &mut self.continental_interior_thickness,
            "continental_margin_width" => &mut self.continental_margin_width,
            "ridge_push" => &mut self.ridge_push,
            "arc_crust_per_subduction" => &mut self.arc_crust_per_subduction,
            "subduction_erosion" => &mut self.subduction_erosion,
            "oceanic_arc_felsic" => &mut self.oceanic_arc_felsic,
            "metamorphic_depth" => &mut self.metamorphic_depth,
            "subduction_channel_thickness" => &mut self.subduction_channel_thickness,
            "decollement_fraction" => &mut self.decollement_fraction,
            "accretionary_convergence" => &mut self.accretionary_convergence,
            "erosive_convergence" => &mut self.erosive_convergence,
            "trench_fill_time" => &mut self.trench_fill_time,
            "events.initiation_age" => &mut self.events.initiation_age,
            "events.min_subducting_area" => &mut self.events.min_subducting_area,
            "events.fracture_zone_initiation" => &mut self.events.fracture_zone_initiation,
            "events.weak_zone_splits" => &mut self.events.weak_zone_splits,
            "events.invasion_waiting_time" => &mut self.events.invasion_waiting_time,
            "events.initiation_displacement" => &mut self.events.initiation_displacement,
            "events.invasion_reach" => &mut self.events.invasion_reach,
            "events.initiation_full_age" => &mut self.events.initiation_full_age,
            "events.initiation_waiting_time" => &mut self.events.initiation_waiting_time,
            "events.boundary_initiation_waiting_time" => {
                &mut self.events.boundary_initiation_waiting_time
            }
            "events.initiation_age_contrast" => &mut self.events.initiation_age_contrast,
            "events.min_oceanic_rift_area" => &mut self.events.min_oceanic_rift_area,
            "events.rift_continent_radius" => &mut self.events.rift_continent_radius,
            "events.suture_time" => &mut self.events.suture_time,
            "events.suture_convergence" => &mut self.events.suture_convergence,
            "events.suture_trench_ratio" => &mut self.events.suture_trench_ratio,
            "events.terrane_accretion_time" => &mut self.events.terrane_accretion_time,
            "events.terrane_max_area" => &mut self.events.terrane_max_area,
            "events.terrane_size_ratio" => &mut self.events.terrane_size_ratio,
            "events.terrane_cooldown" => &mut self.events.terrane_cooldown,
            "events.min_breakup_area" => &mut self.events.min_breakup_area,
            "mantle.plume_interval" => &mut self.mantle.plume_interval,
            "mantle.topography_response" => &mut self.mantle.topography_response,
            "mantle.plume_swell_height" => &mut self.mantle.plume_swell_height,
            "mantle.slab_offset" => &mut self.mantle.slab_offset,
            "mantle.plume_swell_width" => &mut self.mantle.plume_swell_width,
            _ => anyhow::bail!("unknown or untunable tectonic parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

pub struct Plate {
    /// Rotation from the plate-local frame into the world frame.
    pub rotation: DQuat,
    /// Rotation axis scaled by the rotation speed in rad/Myr.
    pub angular_velocity: DVec3,
    /// Crust samples in the plate-local frame, indexed by mesh vertex.
    pub crust: Vec<Option<Crust>>,
    /// For each world cell, the local vertex that was closest to it in the previous substep.
    lookup_cache: Vec<u32>,
    /// Local vertices whose crust was consumed (subducted or stacked onto another plate). Gaps
    /// there are never refilled, which would recreate consumed crust.
    consumed: Vec<bool>,
    /// Where the plate's crust was after the last matching, or `None` if unknown (new, split or
    /// merged plates). See [`matching`].
    tracked: Option<matching::TrackedCrust>,
    /// The rotation at the last matching.
    matched_rotation: DQuat,
    /// When a subduction initiation inside a plate split this plate from its partner (both
    /// parts carry it), for tracking initiations that are undone by merging the parts again.
    initiated_at: Option<f64>,
}

/// Initial ocean ages with Earth's roughly triangular age–area distribution: the oceanic cells
/// ranked by their great-circle `distance` from the ridges (ties by index) get the age
/// 180 Myr · (1 − √(1 − u)) of their quantile u (mean 60 Myr).
fn triangular_ages(distance: &[f64], is_continental: &[bool]) -> Vec<f64> {
    let mut oceanic: Vec<usize> = (0..distance.len())
        .filter(|&cell| !is_continental[cell])
        .collect();
    oceanic.sort_unstable_by(|&a, &b| distance[a].total_cmp(&distance[b]).then(a.cmp(&b)));
    let mut ages = vec![0.0; distance.len()];
    let count = oceanic.len().max(1) as f64;
    for (rank, &cell) in oceanic.iter().enumerate() {
        let quantile = (rank as f64 + 0.5) / count;
        ages[cell] = 180.0 * (1.0 - (1.0 - quantile).sqrt());
    }
    ages
}

/// Changes of the felsic crust and sediment masses per phase of the step (areal masses summed
/// over cells, kg/m²·cells): `(felsic, sediment)` each.
#[derive(Clone, Copy, Debug, Default)]
pub struct MaterialBudget {
    /// Substeps: subduction, collision stacking, arcs, accretion, subduction erosion, gaps.
    pub substeps: (f64, f64),
    /// Crustal flow and removal of dead plates.
    pub flow: (f64, f64),
    /// Plate events: sutures, captures, rifts, initiations, breakups.
    pub events: (f64, f64),
    /// Surface processes (erosion, deposition).
    pub surface: (f64, f64),
    /// Metamorphism of deeply buried sediment.
    pub metamorphism: (f64, f64),
}

/// Changes of the sea level (m, as an offset of dry elevations) so far per process. Each term is
/// the difference between two exact solves of the sea level (see
/// [`crate::isostasy::sea_level_for_volume_with_passes`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct SeaLevelBudget {
    /// Plate tectonics: motion, ageing and subsidence of the sea floor, subduction, accretion,
    /// collisions, plate events.
    pub tectonics: f64,
    /// Fluvial erosion and the routing of river sediment into lakes and the sea.
    pub fluvial: f64,
    /// Coastal and marine processes after routing: delta plains, overfill, diffusion, waves.
    pub coastal: f64,
    /// Erosion works on cells; samples shown at several cells get the sum of their changes, and
    /// hidden samples none. The difference to the cell-based estimate.
    pub sharing: f64,
    /// Burial metamorphism (pores close, the column subsides).
    pub metamorphism: f64,
    /// Changes of the dynamic topography (ocean basins deepen or shallow with the mantle flow).
    pub mantle: f64,
    /// Water held in lakes, which the ocean lacks (see [`Simulation::set_lake_water`]).
    pub lakes: f64,
}

impl SeaLevelBudget {
    pub fn total(&self) -> f64 {
        self.tectonics
            + self.fluvial
            + self.coastal
            + self.sharing
            + self.metamorphism
            + self.mantle
            + self.lakes
    }
}

/// The sea-level change (m) of one step of surface processes as solved on the cells: by fluvial
/// erosion and routing, and by the coastal processes after it.
#[derive(Clone, Copy, Debug, Default)]
pub struct SurfaceSeaLevel {
    pub fluvial: f64,
    pub coastal: f64,
}

/// Ratio of the length of a straight boundary measured as boundary cells × mean edge length to
/// its true length: boundaries on the mesh zig-zag (see the test in [`statistics`]).
const BOUNDARY_ZIGZAG: f64 = 1.1;

/// Position of crust sample `owner` in a dense per-sample array over all plates.
fn sample_index(owner: Ownership, cell_count: usize) -> usize {
    owner.plate as usize * cell_count + owner.local as usize
}

impl Plate {
    /// Maps a world cell to the nearest local vertex of this plate.
    fn local_vertex(&mut self, mesh: &SphereMesh, inverse_rotation: DQuat, cell: u32) -> u32 {
        let local_position = inverse_rotation * mesh.vertex(cell);
        let local = mesh.nearest_from(local_position, self.lookup_cache[cell as usize]);
        self.lookup_cache[cell as usize] = local;
        local
    }
}

/// The plate covering a world cell, and the local vertex of the plate's crust there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ownership {
    pub plate: u32,
    pub local: u32,
}

pub struct Simulation {
    mesh: SphereMesh,
    params: TectonicParams,
    plates: Vec<Plate>,
    owners: Vec<Ownership>,
    /// Subduction polarity of the boundary segments of plate pairs.
    polarities: polarity::Polarities,
    rng: ChaCha8Rng,
    mantle: mantle::MantleFlow,
    /// Dynamic topography of every world cell (m, dry): the deflection of the surface by the
    /// mantle flow below (see [`mantle::MantleFlow::dynamic_topography`]).
    dynamic_topography: Vec<f64>,
    /// How uncovered cells were filled so far.
    gap_statistics: GapStatistics,
    trench_sediment: TrenchSediment,
    /// Plate births and deaths so far.
    event_counts: EventCounts,
    /// Where crust mass came from and went to.
    ledger: MassLedger,
    /// How plates were matched to cells.
    matching_statistics: MatchingStatistics,
    matching_scratch: matching::MatchingScratch,
    /// Number of changes of the plate configuration (splits, merges, removals, induced
    /// subduction) so far; boundaries must be reclassified after each.
    configuration_changes: u64,
    /// Number of velocity solves so far whose collision and subduction resistances did not
    /// settle within the allowed iterations (see [`Self::update_plate_velocities`]).
    unsettled_solves: u64,
    /// Area of sea floor opened by diverging plates so far (one flank), in km².
    opened_area: f64,
    /// Area opened so far at diverging boundary cells showing crust of continental origin (half
    /// per side, km²): the kinematic expectation for rift stretching.
    continental_opened_area: f64,
    /// Mass (Gt) of crust carried into continental collisions by the plates' convergence so
    /// far (kinematic expectation for collision stacking).
    collision_inflow: f64,
    /// Sediment turned into crust by metamorphism so far, in Gt: on crust of continental origin
    /// and on other crust (basins, fans).
    metamorphosed: (f64, f64),
    /// Changes of the felsic crust and sediment masses so far (areal mass summed over cells, see
    /// [`Simulation::felsic_and_sediment`]) per phase of the step.
    material_budget: MaterialBudget,
    /// Sea level as an offset of dry elevations (m) relative to the datum of the isostasy, set
    /// so that the ocean holds [`TectonicParams::ocean_volume`].
    sea_level: f64,
    /// Water held in lakes (m³, see [`Simulation::set_lake_water`]).
    lake_water: f64,
    /// The sea level after the initial setup (m).
    initial_sea_level: f64,
    /// Changes of the sea level so far (m) per process.
    sea_level_budget: SeaLevelBudget,
    /// Simulated time in Myr.
    time: f64,
}

impl Simulation {
    pub fn new(mesh: SphereMesh, plate_count: usize, params: TectonicParams, seed: u64) -> Self {
        let mut owned_rng = ChaCha8Rng::seed_from_u64(seed);
        let rng = &mut owned_rng;
        assert!(plate_count >= 1, "there must be at least one plate");
        assert!(plate_count <= mesh.len(), "more plates than cells");
        let cell_count = mesh.len();

        // Place the continents.
        let continental_cells =
            (params.continental_fraction.clamp(0.0, 1.0) * cell_count as f64).round() as usize;
        let is_continental = continents(
            &mesh,
            continental_cells,
            params.continent_count,
            params.initial,
            rng,
        );

        // Partition the sphere into plates of varying size.
        let plate_of_cell = plate_partition(&mesh, plate_count, &is_continental, rng);

        // Continents rise towards their interior, oceans get older away from plate boundaries.
        let margin_angle =
            (params.continental_margin_width / params.planet_radius).max(mesh.mean_edge_angle());
        let coast_distance = great_circle_distance_to(&mesh, |cell| !is_continental[cell as usize]);
        let boundary_distance = great_circle_distance_to(&mesh, |cell| {
            mesh.neighbors(cell)
                .iter()
                .any(|&n| plate_of_cell[n as usize] != plate_of_cell[cell as usize])
        });

        // Ocean ages grow away from plate boundaries with Earth's roughly triangular age–area
        // distribution (see `triangular_ages`); first from all boundaries, which only matters
        // for choosing the initial subduction zones below, then away from the boundaries that
        // do not become trenches (ridges), so that old crust subducts at the trenches.
        let ocean_age = triangular_ages(&boundary_distance, &is_continental);

        let mut plates: Vec<Plate> = (0..plate_count)
            .map(|_| Plate {
                rotation: DQuat::IDENTITY,
                angular_velocity: DVec3::ZERO,
                crust: vec![None; cell_count],
                lookup_cache: (0..cell_count as u32).collect(),
                consumed: vec![false; cell_count],
                tracked: None,
                matched_rotation: DQuat::IDENTITY,
                initiated_at: None,
            })
            .collect();

        for cell in 0..cell_count {
            let crust = if is_continental[cell] {
                // Continents thicken from their coasts towards their interior, most of it near
                // the coast (thinned passive margins are a few hundred km wide), over the
                // great-circle distance from the nearest oceanic cell.
                let outside = 1.0 - (coast_distance[cell] / margin_angle).min(1.0);
                let inland = 1.0 - outside * outside;
                let thickness = params.continental_coast_thickness
                    + (params.continental_interior_thickness - params.continental_coast_thickness)
                        * inland
                    + rng.random_range(-500.0..500.0);
                Crust {
                    thickness,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: NO_OROGENY,
                    orogenic: 0.0,
                    reheated: NO_REHEATING,
                    cohort: 0.0,
                }
            } else {
                new_oceanic_crust(-ocean_age[cell])
            };
            plates[plate_of_cell[cell] as usize].crust[cell] = Some(crust);
        }

        // Start some plate boundaries as subduction zones; there the denser side subducts.
        let mut boundary_buoyancy: HashMap<(u32, u32), (f64, f64)> = HashMap::default();
        for cell in 0..cell_count as u32 {
            let plate = plate_of_cell[cell as usize];
            for &neighbor in mesh.neighbors(cell) {
                let other = plate_of_cell[neighbor as usize];
                if other != plate {
                    let crust = plates[plate as usize].crust[cell as usize]
                        .expect("every cell has initial crust");
                    let sums = boundary_buoyancy.entry(pair_key(plate, other)).or_default();
                    if plate < other {
                        sums.0 += crust.buoyancy(0.0);
                    } else {
                        sums.1 += crust.buoyancy(0.0);
                    }
                }
            }
        }
        let mut pairs: Vec<_> = boundary_buoyancy.into_iter().collect();
        pairs.sort_unstable_by_key(|((a, b), _)| (*a, *b));
        let mut initial_subduction = Vec::new();
        for ((a, b), (buoyancy_a, buoyancy_b)) in pairs {
            if rng.random_bool(params.initial_subduction_fraction.clamp(0.0, 1.0)) {
                let overriding = if buoyancy_a >= buoyancy_b { a } else { b };
                initial_subduction.push(((a, b), overriding));
            }
        }
        // Ages grow away from the boundaries that are no subduction zones.
        let trench_pairs: HashSet<(u32, u32)> =
            initial_subduction.iter().map(|&(pair, _)| pair).collect();
        let ridge_distance = great_circle_distance_to(&mesh, |cell| {
            let plate = plate_of_cell[cell as usize];
            mesh.neighbors(cell).iter().any(|&n| {
                let other = plate_of_cell[n as usize];
                other != plate && !trench_pairs.contains(&pair_key(plate, other))
            })
        });
        let ocean_age = triangular_ages(&ridge_distance, &is_continental);
        for (cell, &age) in ocean_age.iter().enumerate() {
            if !is_continental[cell] {
                plates[plate_of_cell[cell] as usize].crust[cell] = Some(new_oceanic_crust(-age));
            }
        }

        let segment_radius =
            (params.segment_radius / params.planet_radius).max(3.0 * mesh.mean_edge_angle());
        let polarities = polarity::Polarities::new(segment_radius);

        let owners = (0..cell_count as u32)
            .map(|cell| Ownership {
                plate: plate_of_cell[cell as usize],
                local: cell,
            })
            .collect();

        let mantle = mantle::MantleFlow::new(params.mantle.clone());
        let dynamic_topography = vec![0.0; mesh.len()];
        let mut simulation = Self {
            dynamic_topography,
            mesh,
            params,
            plates,
            owners,
            polarities,
            mantle,
            gap_statistics: GapStatistics::default(),
            trench_sediment: TrenchSediment::default(),
            event_counts: EventCounts::default(),
            ledger: MassLedger::default(),
            matching_statistics: MatchingStatistics::default(),
            matching_scratch: matching::MatchingScratch::default(),
            rng: owned_rng,
            configuration_changes: 0,
            unsettled_solves: 0,
            opened_area: 0.0,
            continental_opened_area: 0.0,
            collision_inflow: 0.0,
            metamorphosed: (0.0, 0.0),
            material_budget: MaterialBudget::default(),
            sea_level: 0.0,
            lake_water: 0.0,
            initial_sea_level: 0.0,
            sea_level_budget: SeaLevelBudget::default(),
            time: 0.0,
        };
        // The sediment cover is part of the initial planet (a stock, not a source); it is
        // added after the plate setup and with its own random numbers, so that it changes
        // nothing else.
        if simulation.params.initial_sediment.enabled {
            simulation.add_initial_sediment(seed, &is_continental, &plate_of_cell);
        }
        simulation.ledger = MassLedger::new(simulation.total_mass());
        simulation.sea_level = simulation.solve_sea_level();
        simulation.initial_sea_level = simulation.sea_level;
        // Initial subduction zones have fully developed slabs.
        let max_slab_length = simulation.params.max_slab_length;
        for (pair, overriding) in initial_subduction {
            simulation.seed_subduction(pair, overriding, max_slab_length);
        }
        simulation
    }

    /// True length (m) of a boundary per boundary cell: the mean edge length, corrected for the
    /// zig-zag of boundaries on the mesh.
    pub(super) fn boundary_length_per_cell(&self) -> f64 {
        self.params.planet_radius * self.mesh.mean_edge_angle() / BOUNDARY_ZIGZAG
    }

    /// Areal masses (kg/m², summed over all samples including hidden ones) of the felsic part of
    /// the crystalline crust (see [`Crust::felsic`]) and of sediment.
    pub(super) fn felsic_and_sediment(&self) -> (f64, f64) {
        let (mut felsic, mut sediment) = (0.0, 0.0);
        if !self.params.track_material_budget {
            return (felsic, sediment);
        }
        for plate in &self.plates {
            for crust in plate.crust.iter().flatten() {
                felsic += crust.felsic_mass();
                sediment += crust.sediment * SEDIMENT_ROCK_DENSITY;
            }
        }
        (felsic, sediment)
    }

    /// The material budget so far, if it is tracked.
    pub fn material_budget(&self) -> Option<MaterialBudget> {
        self.params
            .track_material_budget
            .then_some(self.material_budget)
    }

    /// Current rotations of all plates.
    fn rotations(&self) -> Vec<DQuat> {
        self.plates.iter().map(|plate| plate.rotation).collect()
    }

    /// Makes `overriding` override the other plate of `pair` with a slab of `slab_length` along
    /// their whole common boundary, in segments that do not exist yet.
    pub(super) fn seed_subduction(&mut self, pair: (u32, u32), overriding: u32, slab_length: f64) {
        let rotations = self.rotations();
        for cell in 0..self.mesh.len() as u32 {
            let plate = self.owners[cell as usize].plate;
            if plate != pair.0 && plate != pair.1 {
                continue;
            }
            let other = if plate == pair.0 { pair.1 } else { pair.0 };
            if self
                .mesh
                .neighbors(cell)
                .iter()
                .any(|&neighbor| self.owners[neighbor as usize].plate == other)
            {
                let r = self.mesh.vertex(cell);
                self.polarities
                    .seed(pair, overriding, r, slab_length, self.time, &rotations);
            }
        }
        self.configuration_changes += 1;
    }

    pub fn mesh(&self) -> &SphereMesh {
        &self.mesh
    }

    pub fn time(&self) -> f64 {
        self.time
    }

    /// The plate covering a world cell.
    pub fn owner(&self, cell: u32) -> Ownership {
        self.owners[cell as usize]
    }

    /// The crust at the surface of a world cell.
    pub fn surface_crust(&self, cell: u32) -> Crust {
        let owner = self.owner(cell);
        self.plates[owner.plate as usize].crust[owner.local as usize]
            .expect("owners always point to existing crust")
    }

    /// Advances the simulation by `dt` Myr, splitting it into substeps such that no cell moves
    /// more than [`TectonicParams::max_substep_movement`] cell spacings per substep.
    pub fn step(&mut self, dt: f64) {
        let mantle_velocity = self
            .mantle
            .velocities(&self.mesh, self.params.planet_radius);
        self.update_plate_velocities(&mantle_velocity);
        // The farthest any cell moves (a small plate turning about a pole inside it moves its
        // cells much less than its angular speed suggests).
        let max_angle = (0..self.mesh.len() as u32)
            .map(|cell| {
                self.plates[self.owners[cell as usize].plate as usize]
                    .angular_velocity
                    .cross(self.mesh.vertex(cell))
                    .length()
                    * dt
            })
            .fold(0.0, f64::max);
        let max_substep_angle = self.params.max_substep_movement * self.mesh.mean_edge_angle();
        let substeps = (max_angle / max_substep_angle).ceil().max(1.0) as usize;
        tracing::trace!(substeps, "Tectonic step");
        let start = self.felsic_and_sediment();
        for _ in 0..substeps {
            self.substep(dt / substeps as f64);
        }
        let after_substeps = self.felsic_and_sediment();
        let rotations = self.rotations();
        self.polarities
            .end_step(self.time, self.params.polarity_memory, &rotations);
        self.crustal_flow(dt);
        self.remove_dead_plates();
        let after_flow = self.felsic_and_sediment();
        let boundaries = self.boundary_cells();
        let slabs_changed = self.update_slab_lengths(dt, &boundaries);
        // Trenches whose slab broke off are no trenches any more.
        let boundaries = if slabs_changed {
            self.boundary_cells()
        } else {
            boundaries
        };
        let topology = self.configuration_changes;
        // The strength history of the lithosphere: orogenic belts cool, plumes heat the
        // lithosphere at their current positions, just before the splits read it.
        self.heal_orogenic_weakening(dt);
        self.reheat_above_plumes();
        self.plate_events(dt, &boundaries);
        let after_events = self.felsic_and_sediment();
        let budget = &mut self.material_budget;
        for (phase, (before, after)) in [
            (&mut budget.substeps, (start, after_substeps)),
            (&mut budget.flow, (after_substeps, after_flow)),
            (&mut budget.events, (after_flow, after_events)),
        ] {
            phase.0 += after.0 - before.0;
            phase.1 += after.1 - before.1;
        }
        // The boundaries only change if plates were split, merged or removed, or subduction was
        // induced.
        let boundaries = if self.configuration_changes == topology {
            boundaries
        } else {
            self.boundary_cells()
        };

        // Opening rate × boundary length over the diverging cells of both sides, halved.
        let (opening, continental_opening) = boundaries
            .iter()
            .filter(|boundary| boundary.convergence < 0.0)
            .fold((0.0, 0.0), |(all, continental), boundary| {
                let rate = -boundary.convergence;
                if self.surface_crust(boundary.cell).is_continental_origin() {
                    (all + rate, continental + rate)
                } else {
                    (all + rate, continental)
                }
            });
        let to_area = 0.5 * self.boundary_length_per_cell() / 1000.0 * self.params.planet_radius
            / 1000.0
            * dt;
        self.opened_area += opening * to_area;
        self.continental_opened_area += continental_opening * to_area;

        // Crust carried into collisions: convergence × boundary length × areal mass, half from
        // each side's boundary cells.
        let inflow: f64 = boundaries
            .iter()
            .filter(|boundary| {
                boundary.kind == dynamics::BoundaryKind::Collision && boundary.convergence > 0.0
            })
            .map(|boundary| {
                boundary.convergence
                    * self.params.planet_radius
                    * self.surface_crust(boundary.cell).mass()
            })
            .sum();
        self.collision_inflow += 0.5 * inflow * self.boundary_length_per_cell() * dt / 1e12;

        let subduction = self.subduction_rates(&boundaries);
        let slabs = self.sinking_slabs(&boundaries);
        self.mantle.step(dt, &subduction, &slabs, &mut self.rng);

        let sea_level = self.solve_sea_level();
        self.sea_level_budget.tectonics += sea_level - self.sea_level;
        self.sea_level = sea_level;

        self.dynamic_topography = self
            .mantle
            .dynamic_topography(&self.mesh, self.params.planet_radius);
        let sea_level = self.solve_sea_level();
        self.sea_level_budget.mantle += sea_level - self.sea_level;
        self.sea_level = sea_level;
    }

    /// Dynamic topography of every world cell in metres (dry, without the water load).
    pub fn dynamic_topography(&self) -> &[f64] {
        &self.dynamic_topography
    }

    /// Dry elevation (without the water load, see [`crate::isostasy`]) of a world cell showing
    /// `crust`: the crust's isostatic and thermal elevation plus the dynamic topography.
    fn cell_dry_elevation(&self, cell: u32, crust: &Crust) -> f64 {
        crust.dry_elevation(self.time) + self.dynamic_topography[cell as usize]
    }

    /// Areal mass (kg/m², summed over cells) of all crust, including hidden samples.
    pub fn total_mass(&self) -> f64 {
        ledger::plates_mass(&self.plates)
    }

    /// Volume of the ocean in m³: the water of the planet (see
    /// [`TectonicParams::ocean_volume`]) less the water in lakes (see
    /// [`Simulation::set_lake_water`]).
    pub fn ocean_volume(&self) -> f64 {
        self.params.ocean_volume - self.lake_water
    }

    /// Sets the water held in lakes (m³), which the ocean lacks: closed depressions, also those
    /// below sea level that the ocean does not reach (see [`crate::sea`]), hold lakes fed by
    /// their rivers, and their water is taken from the ocean, so that the planet's water is
    /// conserved. The sea level is solved again.
    pub fn set_lake_water(&mut self, volume: f64) {
        self.lake_water = volume.clamp(0.0, self.params.ocean_volume);
        let sea_level = self.solve_sea_level();
        self.sea_level_budget.lakes += sea_level - self.sea_level;
        self.sea_level = sea_level;
    }

    /// The active mantle plumes.
    pub fn plumes(&self) -> &[Plume] {
        self.mantle.plumes()
    }

    fn substep(&mut self, dt: f64) {
        self.time += dt;
        self.move_plates(dt);
        let ResolvedBoundaries {
            additions,
            collisions,
        } = self.resolve_ownership();
        let rotations = self.rotations();
        self.polarities
            .end_pass(dt, self.params.polarity_flip_time, &rotations);
        self.apply_additions(&additions);
        tracing::trace!(colliding_pairs = collisions.len(), "Substep");
    }

    fn move_plates(&mut self, dt: f64) {
        for plate in &mut self.plates {
            let delta = math::quat_from_scaled_axis(plate.angular_velocity * dt);
            plate.rotation = (delta * plate.rotation).normalize();
        }
    }

    /// Surface elevation (bedrock plus sediment) of a world cell relative to sea level in metres.
    pub fn surface_elevation(&self, cell: u32) -> f64 {
        crate::isostasy::to_surface(
            self.cell_dry_elevation(cell, &self.surface_crust(cell)) - self.sea_level,
        )
    }

    /// The part of the sea level due to sediment: the sea level minus the one without any
    /// sediment (m).
    pub(super) fn sediment_sea_level(&self) -> f64 {
        self.sediment_sea_level_of(|_, _, _| true, false)
    }

    /// The part of the sea level due to the sediment of the cells selected by
    /// `select(cell, crust, surface elevation)`: the sea level minus the one with their
    /// sediment (and with `metasediment` also the metasediment of their crust, see
    /// [`Crust::basement`]) removed (m). Each cell is counted as the sea-level solve counts it, so hidden samples do
    /// not count and samples shown at several cells count at each.
    pub fn sediment_sea_level_of(
        &self,
        select: impl Fn(u32, &Crust, f64) -> bool,
        metasediment: bool,
    ) -> f64 {
        let (dry, pores): (Vec<f64>, Vec<f64>) = (0..self.mesh.len() as u32)
            .map(|cell| {
                let crust = self.surface_crust(cell);
                let dynamic = self.dynamic_topography[cell as usize];
                if select(
                    cell,
                    &crust,
                    crate::isostasy::to_surface(
                        crust.dry_elevation(self.time) + dynamic - self.sea_level,
                    ),
                ) {
                    let rest = if metasediment {
                        crust.basement()
                    } else {
                        crust
                    };
                    (
                        rest.dry_elevation(self.time) + dynamic
                            - crate::isostasy::sediment_dry_elevation(crust.sediment),
                        0.0,
                    )
                } else {
                    (
                        crust.dry_elevation(self.time) + dynamic,
                        crate::isostasy::sediment_pores(crust.sediment),
                    )
                }
            })
            .unzip();
        let cell_area =
            4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / self.mesh.len() as f64;
        let without = crate::sea::sea_level(
            &self.mesh,
            &dry,
            &pores,
            cell_area,
            self.ocean_volume(),
            self.sea_level,
        )
        .level;
        self.sea_level - without
    }

    /// Rock-equivalent sediment thickness summed over the cells as the sea-level solve sees
    /// them (samples shown at several cells count at each, hidden ones not), and over all
    /// samples (m·cells).
    pub fn sediment_totals(&self) -> (f64, f64) {
        let cells = (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).sediment)
            .sum();
        let samples = self
            .plates
            .iter()
            .flat_map(|plate| plate.crust.iter().flatten())
            .map(|crust| crust.sediment)
            .sum();
        (cells, samples)
    }

    /// Metasediment (Gt, all samples including hidden ones) in crust whose basement is of
    /// continental origin and in other crust.
    pub fn metasediment_masses(&self) -> (f64, f64) {
        let cell_area =
            4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / self.mesh.len() as f64;
        let mut masses = (0.0, 0.0);
        for crust in self.plates.iter().flat_map(|p| p.crust.iter().flatten()) {
            let mass = crust.metasediment_mass() * cell_area / 1e12;
            if crust.basement().is_continental_origin() {
                masses.0 += mass;
            } else {
                masses.1 += mass;
            }
        }
        masses
    }

    /// The sea level at which the ocean holds [`TectonicParams::ocean_volume`], of which the
    /// pores of submerged sediment hold a part. Only the water connected to the world ocean
    /// counts (see [`crate::sea`]).
    fn solve_sea_level(&self) -> f64 {
        self.solve_sea_level_with_deficit().level
    }

    /// The part of [`TectonicParams::ocean_volume`] (m³) that the ocean does not hold because
    /// the sea level stands at a pass into a basin it cannot fill (see
    /// [`crate::isostasy::sea_level_for_volume_with_passes`]); zero otherwise.
    pub fn sea_level_deficit(&self) -> f64 {
        self.solve_sea_level_with_deficit().deficit
    }

    /// The ocean: the cells below sea level connected to the world ocean (see [`crate::sea`]).
    pub fn ocean_mask(&self) -> Vec<bool> {
        crate::sea::ocean_mask(&self.mesh, &self.surface_elevations())
    }

    fn solve_sea_level_with_deficit(&self) -> crate::isostasy::SeaLevel {
        let (dry, sediment): (Vec<f64>, Vec<f64>) = (0..self.mesh.len() as u32)
            .map(|cell| {
                let crust = self.surface_crust(cell);
                (self.cell_dry_elevation(cell, &crust), crust.sediment)
            })
            .unzip();
        let cell_area =
            4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / self.mesh.len() as f64;
        let pores: Vec<f64> = sediment
            .iter()
            .map(|&s| crate::isostasy::sediment_pores(s))
            .collect();
        crate::sea::sea_level(
            &self.mesh,
            &dry,
            &pores,
            cell_area,
            self.ocean_volume(),
            self.sea_level,
        )
    }

    /// Surface elevation (bedrock plus sediment) of every world cell.
    pub fn surface_elevations(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_elevation(cell))
            .collect()
    }

    /// Density of the crystalline crust of every world cell in kg/m³.
    pub fn bedrock_densities(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).density)
            .collect()
    }

    /// Density giving the isostatic response of removing crust from the top, per world cell.
    pub fn bedrock_isostatic_densities(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).removal_isostatic_density())
            .collect()
    }

    /// Thickness of the densified lower crust of every world cell (see
    /// [`Crust::dense_crust_thickness`]).
    pub fn dense_crust_thicknesses(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).dense_crust_thickness())
            .collect()
    }

    /// Sediment thickness of every world cell.
    pub fn sediment_thicknesses(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).sediment)
            .collect()
    }

    /// Sediment buried deeper than [`TectonicParams::metamorphic_depth`] becomes felsic crust
    /// (metasediment) at the same mass, as far as the crust stays below
    /// [`TectonicParams::max_crust_thickness`]. Its pores close, so the column subsides a
    /// little. Burial metamorphism does not depend on the basement: the deep fill of trapped
    /// oceanic basins (South Caspian, Precaspian) and of deep-sea fans metamorphoses too.
    fn metamorphose_sediment(&mut self) {
        let before = self.felsic_and_sediment();
        let keep = crate::isostasy::sediment_for_bulk_thickness(self.params.metamorphic_depth);
        let max_thickness = self.params.max_crust_thickness;
        // On crust of continental origin, and on other crust (basins, fans).
        let mut converted = [0.0; 2];
        for plate in &mut self.plates {
            for crust in plate.crust.iter_mut().flatten() {
                if crust.sediment <= keep {
                    continue;
                }
                let continental = crust.is_continental_origin();
                // Burial metasediment is tracked where it forms on crust that is not of
                // continental origin (basins and fans on oceanic, arc or thinned crust).
                let tracked = if crust.basement().is_continental_origin() {
                    0.0
                } else {
                    1.0
                };
                let mass = ((crust.sediment - keep) * SEDIMENT_ROCK_DENSITY)
                    .min((max_thickness - crust.thickness).max(0.0) * METASEDIMENT_DENSITY);
                if mass <= 0.0 {
                    continue;
                }
                crust.sediment -= mass / SEDIMENT_ROCK_DENSITY;
                crust.add_crust(
                    mass / METASEDIMENT_DENSITY,
                    METASEDIMENT_DENSITY,
                    (1.0, tracked),
                    max_thickness,
                );
                converted[usize::from(!continental)] += mass;
            }
        }
        let cell_area =
            4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / self.mesh.len() as f64;
        self.metamorphosed.0 += converted[0] * cell_area / 1e12;
        self.metamorphosed.1 += converted[1] * cell_area / 1e12;
        let after = self.felsic_and_sediment();
        self.material_budget.metamorphism.0 += after.0 - before.0;
        self.material_budget.metamorphism.1 += after.1 - before.1;
    }

    /// Applies per-cell changes of crust thickness and sediment thickness to the crust at the
    /// surface. A crust sample visible at several cells gets the sum of their changes: erosion
    /// works per cell, and about as many samples are hidden by resampling as are shown twice.
    /// Sediment removal beyond the available sediment comes from the bedrock at the same mass.
    /// `predicted` is the sea-level change of the surface processes as solved on the cells by
    /// the caller (see [`SurfaceSeaLevel`]); the difference to the solve on the samples is
    /// booked as [`SeaLevelBudget::sharing`].
    pub fn apply_surface_changes(
        &mut self,
        thickness_delta: &[f64],
        sediment_delta: &[f64],
        predicted: SurfaceSeaLevel,
    ) {
        let before = self.felsic_and_sediment();
        let mut changes: Vec<(Ownership, f64, f64)> = Vec::new();
        let cell_count = self.mesh.len();
        // Per crust sample: its position in `changes`, or `u32::MAX`.
        let mut index = vec![u32::MAX; self.plates.len() * cell_count];
        for cell in 0..cell_count {
            let owner = self.owners[cell];
            let slot = &mut index[sample_index(owner, cell_count)];
            if *slot == u32::MAX {
                *slot = changes.len() as u32;
                changes.push((owner, 0.0, 0.0));
            }
            let slot = *slot as usize;
            changes[slot].1 += thickness_delta[cell];
            changes[slot].2 += sediment_delta[cell];
        }
        let mut change = 0.0;
        for (owner, thickness, sediment) in changes {
            let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                .as_mut()
                .expect("owners always point to existing crust");
            let before = crust.mass();
            let new_sediment = crust.sediment + sediment;
            let mut new_thickness = crust.thickness + thickness;
            if new_sediment < 0.0 {
                new_thickness += new_sediment * SEDIMENT_ROCK_DENSITY / crust.density;
            }
            if sediment > 0.0 {
                // New deposits are new rock: they do not belong to the cover cohort.
                crust.add_sediment(sediment, 0.0);
            } else {
                crust.sediment = new_sediment.max(0.0);
            }
            // Erosion removes crust from the top; deposition never adds crystalline crust.
            crust.thin_from_top(new_thickness.max(0.0));
            change += crust.mass() - before;
        }
        self.ledger.record(ledger::Term::SurfaceProcesses, change);
        let after = self.felsic_and_sediment();
        self.material_budget.surface.0 += after.0 - before.0;
        self.material_budget.surface.1 += after.1 - before.1;

        // Sediment displaces water and eroded land no longer holds it back.
        let sea_level = self.solve_sea_level();
        let budget = &mut self.sea_level_budget;
        budget.fluvial += predicted.fluvial;
        budget.coastal += predicted.coastal;
        budget.sharing += sea_level - self.sea_level - predicted.fluvial - predicted.coastal;
        self.sea_level = sea_level;

        // Metamorphism closes the pores of deeply buried sediment, and the column subsides.
        self.metamorphose_sediment();
        let sea_level = self.solve_sea_level();
        self.sea_level_budget.metamorphism += sea_level - self.sea_level;
        self.sea_level = sea_level;
    }

    /// Starts tracking the continental sedimentary cover as a cohort (see [`Crust::cohort`]):
    /// all sediment now on crust whose basement is of continental origin belongs to it, nothing
    /// else does. Returns the cohort's mass in Gt.
    pub fn start_cover_cohort(&mut self) -> f64 {
        for crust in self
            .plates
            .iter_mut()
            .flat_map(|plate| plate.crust.iter_mut().flatten())
        {
            crust.cohort = f64::from(u8::from(crust.basement().is_continental_origin()));
        }
        self.cover_cohort().0
    }

    /// Mass (Gt, all samples including hidden ones) of the cover cohort (see
    /// [`Simulation::start_cover_cohort`]) still on crust whose basement is of continental
    /// origin, and still sediment anywhere.
    pub fn cover_cohort(&self) -> (f64, f64) {
        let cell_area =
            4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / self.mesh.len() as f64;
        let to_gt = cell_area * SEDIMENT_ROCK_DENSITY / 1e12;
        let mut masses = (0.0, 0.0);
        for crust in self.plates.iter().flat_map(|p| p.crust.iter().flatten()) {
            let mass = crust.sediment * crust.cohort * to_gt;
            if crust.basement().is_continental_origin() {
                masses.0 += mass;
            }
            masses.1 += mass;
        }
        masses
    }

    /// The changes of the sea level so far per process, and the remainder of the actual change
    /// since the start that they do not explain (zero up to rounding).
    pub fn sea_level_budget(&self) -> (SeaLevelBudget, f64) {
        let budget = self.sea_level_budget;
        (
            budget,
            self.sea_level - self.initial_sea_level - budget.total(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::{ARC_DENSITY, CONTINENTAL_DENSITY, OCEANIC_DENSITY};

    fn simulation(seed: u64) -> Simulation {
        Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), seed)
    }

    #[test]
    fn uniform_dynamic_topography_shifts_the_sea_level_only() {
        let mut simulation = simulation(3);
        let sea_level = simulation.solve_sea_level();
        let sediment_part = simulation.sediment_sea_level();
        let elevations = simulation.surface_elevations();
        simulation.dynamic_topography = vec![250.0; simulation.mesh.len()];
        let shifted = simulation.solve_sea_level();
        assert!((shifted - sea_level - 250.0).abs() < 1e-6);
        simulation.sea_level = shifted;
        assert!((simulation.sediment_sea_level() - sediment_part).abs() < 1e-6);
        for (cell, &before) in elevations.iter().enumerate() {
            assert!((simulation.surface_elevation(cell as u32) - before).abs() < 1e-6);
        }
    }

    #[test]
    fn deeply_buried_sediment_metamorphoses() {
        let mut simulation = simulation(2);
        let deep = 30_000.0;
        let (mut continental, mut oceanic) = (None, None);
        for (p, plate) in simulation.plates.iter_mut().enumerate() {
            for (local, crust) in plate.crust.iter_mut().enumerate() {
                let Some(crust) = crust else { continue };
                if crust.is_continental_origin() && continental.is_none() {
                    crust.sediment = deep;
                    continental = Some((p, local));
                } else if crust.is_oceanic() && oceanic.is_none() {
                    // A trapped basin filled with ~20 km of sediment (South Caspian); much
                    // thicker fills turn the basin into buoyant, continental-like crust.
                    crust.sediment = 20_000.0;
                    oceanic = Some((p, local));
                }
            }
        }
        let (continental, oceanic) = (continental.unwrap(), oceanic.unwrap());
        let crust = |s: &Simulation, (p, l): (usize, usize)| s.plates[p].crust[l].unwrap();
        let before = crust(&simulation, continental);
        let total = simulation.total_mass();
        simulation.metamorphose_sediment();
        let after = crust(&simulation, continental);
        // The sediment below the metamorphic depth became felsic crust at the same mass.
        let keep =
            crate::isostasy::sediment_for_bulk_thickness(simulation.params.metamorphic_depth);
        assert!((after.sediment - keep).abs() < 1e-6, "{}", after.sediment);
        assert!(after.thickness > before.thickness);
        assert!(after.felsic_mass() > before.felsic_mass());
        assert!((after.mass() - before.mass()).abs() < 1e-6 * before.mass());
        assert!((simulation.total_mass() - total).abs() < 1e-9 * total);
        assert!(simulation.metamorphosed.0 > 0.0);
        // So does deeply buried sediment on oceanic crust (trapped basins, fans), and the crust
        // still subducts.
        let ocean = crust(&simulation, oceanic);
        assert!((ocean.sediment - keep).abs() < 1e-6, "{}", ocean.sediment);
        assert!(!ocean.resists_subduction(), "{ocean:?}");
        assert!(simulation.metamorphosed.1 > 0.0);
    }

    #[test]
    fn the_ocean_holds_its_volume() {
        let mut simulation = simulation(1);
        let cell_area = 4.0 * std::f64::consts::PI * simulation.params.planet_radius.powi(2)
            / simulation.mesh.len() as f64;
        let check = |simulation: &Simulation| {
            // Open water plus the pore water of submerged sediment, over the ocean only (not
            // over closed depressions below sea level).
            let ocean = simulation.ocean_mask();
            let pores: f64 = (0..simulation.mesh.len() as u32)
                .filter(|&cell| ocean[cell as usize])
                .map(|cell| {
                    crate::isostasy::sediment_pores(simulation.surface_crust(cell).sediment)
                })
                .sum::<f64>()
                * cell_area;
            let elevations: Vec<f64> = simulation
                .surface_elevations()
                .iter()
                .zip(&ocean)
                .map(|(&e, &o)| if o { e } else { 0.0 })
                .collect();
            // The water that would spill over a pass into a basin the ocean cannot fill.
            let volume = crate::isostasy::ocean_volume(&elevations, cell_area)
                + pores
                + simulation.sea_level_deficit();
            let target = simulation.ocean_volume();
            // When the volume falls into the pores of a cell right at sea level, the sea level
            // stays at that cell (the pores fill at once): allow the pores of shoreline cells.
            let shoreline_pores = (0..simulation.mesh.len() as u32)
                .filter(|&cell| simulation.surface_elevation(cell).abs() < 1e-6)
                .map(|cell| {
                    crate::isostasy::sediment_pores(simulation.surface_crust(cell).sediment)
                })
                .fold(0.0, f64::max)
                * cell_area;
            assert!(
                (volume - target).abs() < 1e-9 * target + shoreline_pores,
                "{volume} vs {target}"
            );
        };
        check(&simulation);
        for _ in 0..5 {
            simulation.step(1.0);
            check(&simulation);
            // Sediment on the sea floor displaces water.
            let cells = simulation.mesh.len();
            let sediment: Vec<f64> = (0..cells as u32)
                .map(|c| {
                    if simulation.surface_elevation(c) < 0.0 {
                        50.0
                    } else {
                        0.0
                    }
                })
                .collect();
            let before = simulation.sea_level;
            simulation.apply_surface_changes(
                &vec![0.0; cells],
                &sediment,
                SurfaceSeaLevel::default(),
            );
            check(&simulation);
            assert!(simulation.sea_level > before);
        }
    }

    #[test]
    fn isostasy_gives_earth_like_elevations() {
        let ocean = |age: f64| new_oceanic_crust(-age);
        assert!((ocean(0.0).surface_elevation(0.0, 0.0) + 2600.0).abs() < 1.0);
        assert!((ocean(1000.0).surface_elevation(0.0, 0.0) + 5651.0).abs() < 1.0);
        assert!(!ocean(0.0).is_continental());

        let continent = |thickness| Crust {
            thickness,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: NO_REHEATING,
            cohort: 0.0,
        };
        // Normal continental crust stands a few hundred metres above sea level, doubly
        // thickened crust forms high plateaus like Tibet, thinned crust forms shelves.
        assert!((200.0..900.0).contains(&continent(35_000.0).surface_elevation(0.0, 0.0)));
        assert!((4500.0..5500.0).contains(&continent(70_000.0).surface_elevation(0.0, 0.0)));
        assert!((-500.0..0.0).contains(&continent(30_000.0).surface_elevation(0.0, 0.0)));
        assert!(continent(30_000.0).is_continental());
    }

    #[test]
    fn elevation_rises_with_thickness_at_any_age() {
        for age in [0.0, 1.0, 5.0, 20.0, 1000.0] {
            for density in [OCEANIC_DENSITY, ARC_DENSITY, CONTINENTAL_DENSITY] {
                let mut previous = f64::NEG_INFINITY;
                let mut thickness = 5000.0;
                while thickness <= 75_000.0 {
                    let crust = Crust {
                        thickness,
                        density,
                        birth: -age,
                        sediment: 0.0,
                        felsic: 1.0,
                        metasediment: 0.0,
                        orogeny: NO_OROGENY,
                        orogenic: 0.0,
                        reheated: NO_REHEATING,
                        cohort: 0.0,
                    };
                    let elevation = crust.dry_elevation(0.0);
                    // At least 0.5 m per 100 m of crust (the thermal uplift fading out over the
                    // continental transition leaves a margin of ~1 m per 100 m on young
                    // lithosphere).
                    assert!(
                        elevation > previous + 0.5,
                        "age {age}, density {density}, {thickness} m: {elevation} after {previous}"
                    );
                    previous = elevation;
                    thickness += 100.0;
                }
            }
        }
    }

    #[test]
    fn young_arcs_override_young_ocean() {
        for age in [0.0, 1.0, 5.0] {
            for thickness in [15_000.0, 20_000.0] {
                let arc = Crust {
                    thickness,
                    density: ARC_DENSITY,
                    birth: -age,
                    sediment: 0.0,
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: NO_OROGENY,
                    orogenic: 0.0,
                    reheated: NO_REHEATING,
                    cohort: 0.0,
                };
                assert!(arc.buoyancy(0.0) > new_oceanic_crust(0.0).buoyancy(0.0));
            }
        }
    }

    #[test]
    fn continental_crust_gets_no_oceanic_thermal_uplift() {
        let continent = |age: f64| Crust {
            thickness: 40_000.0,
            density: CONTINENTAL_DENSITY,
            birth: -age,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: NO_REHEATING,
            cohort: 0.0,
        };
        assert!(
            (continent(0.0).surface_elevation(0.0, 0.0)
                - continent(1000.0).surface_elevation(0.0, 0.0))
            .abs()
                < 1.0
        );
    }

    #[test]
    fn erosion_response_matches_the_elevation_formula() {
        for thickness in [30_000.0, 49_000.0, 60_000.0, 74_000.0] {
            let crust = Crust {
                thickness,
                density: CONTINENTAL_DENSITY,
                birth: -1000.0,
                sediment: 0.0,
                felsic: 1.0,
                metasediment: 0.0,
                orogeny: NO_OROGENY,
                orogenic: 0.0,
                reheated: NO_REHEATING,
                cohort: 0.0,
            };
            let removed = 10.0;
            let thinner = Crust {
                thickness: thickness - removed,
                ..crust
            };
            let expected = thinner.surface_elevation(0.0, 0.0);
            let predicted = crate::isostasy::add_layer(
                crust.surface_elevation(0.0, 0.0),
                -removed,
                crust.removal_isostatic_density(),
            );
            assert!(
                (predicted - expected).abs() < 0.01,
                "{thickness}: {predicted} vs {expected}"
            );
        }
    }

    #[test]
    fn thinned_continental_crust_is_not_ocean() {
        let eroded = Crust {
            thickness: OCEANIC_THICKNESS,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: NO_REHEATING,
            cohort: 0.0,
        };
        assert!(!eroded.is_oceanic());
        assert!(!eroded.is_continental());
        assert!(new_oceanic_crust(0.0).is_oceanic());
    }

    #[test]
    fn arcs_mature_into_continental_crust() {
        let mut crust = new_oceanic_crust(-80.0);
        let mut became_continental_at = None;
        for _ in 0..100 {
            crust.add_crust(500.0, ARC_DENSITY, (1.0, 0.0), 75_000.0);
            if crust.is_continental() && became_continental_at.is_none() {
                became_continental_at = Some(crust.thickness);
            }
        }
        let thickness = became_continental_at.expect("arc became continental");
        assert!((18_000.0..28_000.0).contains(&thickness), "{thickness}");
        // A mature arc emerges above sea level.
        assert!(crust.surface_elevation(0.0, 0.0) > 0.0);

        // Intra-oceanic arcs start basaltic to andesitic and become of continental origin once
        // the arc crust has thickened.
        let felsic = TectonicParams::default().oceanic_arc_felsic;
        let mut crust = new_oceanic_crust(-80.0);
        for _ in 0..100 {
            let added = if crust.is_oceanic() { felsic } else { 1.0 };
            crust.add_crust(500.0, ARC_DENSITY, (added, 0.0), 75_000.0);
        }
        assert!(crust.is_continental_origin(), "{crust:?}");
    }

    #[test]
    fn oceanic_depth_deepens_with_age() {
        let mut previous = oceanic_depth(0.0);
        for age in 1..300 {
            let depth = oceanic_depth(f64::from(age));
            assert!(depth < previous);
            previous = depth;
        }
    }

    #[test]
    fn initial_state_covers_sphere() {
        let simulation = simulation(0);
        let statistics = simulation.statistics();
        assert_eq!(
            statistics.plate_areas.iter().sum::<usize>(),
            simulation.mesh().len()
        );
        assert!(statistics.plate_areas.iter().all(|&area| area > 0));
        assert!((statistics.continental_fraction - 0.4).abs() < 0.01);
    }

    #[test]
    fn simulation_keeps_sphere_covered_and_plates_move() {
        let mut simulation = simulation(1);
        let initial_owners: Vec<Ownership> = simulation.owners.clone();
        for _ in 0..50 {
            simulation.step(2.0);
        }
        assert!((simulation.time() - 100.0).abs() < 1e-9);

        for cell in 0..simulation.mesh().len() as u32 {
            // Panics if an owner points to destroyed crust.
            simulation.surface_crust(cell);
        }
        let statistics = simulation.statistics();
        assert!(statistics.continental_fraction > 0.1);
        assert!(statistics.continental_fraction < 0.9);
        assert_ne!(initial_owners, simulation.owners);
        // New crust was created at ridges.
        assert!(
            (0..simulation.mesh().len() as u32)
                .any(|cell| simulation.surface_crust(cell).age(simulation.time()) < 10.0)
        );
    }

    #[test]
    fn simulation_is_deterministic() {
        let mut a = simulation(7);
        let mut b = simulation(7);
        for _ in 0..10 {
            a.step(1.0);
            b.step(1.0);
        }
        assert_eq!(a.owners, b.owners);
    }

    #[test]
    fn crust_mass_ledger_closes() {
        let mut simulation = simulation(3);
        let total = simulation.total_mass();
        let assert_closes = |simulation: &Simulation, context: &str| {
            let residual = simulation.ledger.checked(simulation.total_mass()).residual;
            assert!(
                residual.abs() < 1e-9 * total,
                "{context}: residual {residual} of total {total}"
            );
        };
        for step in 0..40 {
            simulation.step(1.0);
            assert_closes(&simulation, &format!("step {step}"));
            // Erosion-like changes, including a sample shown at several cells.
            let cells = simulation.mesh.len();
            let thickness: Vec<f64> = (0..cells).map(|c| -((c % 7) as f64)).collect();
            let sediment: Vec<f64> = (0..cells).map(|c| (c % 5) as f64 - 2.0).collect();
            simulation.apply_surface_changes(&thickness, &sediment, SurfaceSeaLevel::default());
            assert_closes(&simulation, &format!("surface changes {step}"));
        }
        // Plates moved and things happened.
        let ledger = simulation.ledger.checked(simulation.total_mass());
        assert!(ledger.new_sea_floor > 0.0);
        assert!(ledger.subducted_oceanic < 0.0);
    }

    #[test]
    fn shared_samples_get_the_sum_of_their_changes() {
        let mut simulation = simulation(4);
        // Show the sample of cell 0 also at its first neighbour.
        let neighbor = simulation.mesh.neighbors(0)[0];
        simulation.owners[neighbor as usize] = simulation.owners[0];
        let before = simulation.surface_crust(0);
        let mut thickness = vec![0.0; simulation.mesh.len()];
        thickness[0] = -10.0;
        thickness[neighbor as usize] = -20.0;
        simulation.apply_surface_changes(
            &thickness,
            &vec![0.0; simulation.mesh.len()],
            SurfaceSeaLevel::default(),
        );
        assert!((simulation.surface_crust(0).thickness - (before.thickness - 30.0)).abs() < 1e-9);
    }
}
