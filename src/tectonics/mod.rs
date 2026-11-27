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

mod activity;
mod boundaries;
mod breakup;
mod cover;
mod crust;
mod deformation;
mod display;
mod dynamics;
mod events;
mod generation;
mod initial_sediment;
mod kernels;
mod ledger;
mod mantle;
mod matching;
mod polarity;
mod sheet;
mod statistics;
mod topology;
mod weak_zones;

pub use activity::{SeismicSource, VolcanicSource};
pub use boundaries::{GapStatistics, TrenchSediment};
pub use breakup::{BreakupParams, BreakupStatistics, SectionRecord};
pub use cover::{CohortMasses, CoverBudget, CoverMasses, CoverTerm};
pub use crust::*;
pub use deformation::MIN_BAND_STRENGTH;
pub use display::{BoundaryClass, BoundaryMap};
pub use events::{
    EventCounts, PLUME_SPLIT_DISTANCES, PlateEventParams, SplitCause, WEAK_BELT_THRESHOLD,
};
pub use generation::InitialConfiguration;
pub use initial_sediment::{InitialSedimentParams, PlatformEmplacement};
pub use ledger::MassLedger;
pub use mantle::{MantleParams, Plume};
pub use matching::{MatchingParams, MatchingStatistics};
pub use sheet::{SheetParams, SheetStatistics};
pub use statistics::*;
pub use weak_zones::{
    MIN_STRENGTH, combine_strength, fabric_strength, orogenic_strength, thermal_strength,
};

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
    /// A cell whose dry elevation is more than this (m) below all its neighbours and whose crust
    /// is thinner than all of theirs is a one-cell hole: the crust of the neighbours of its
    /// plate flows into it at full mobility, whatever its thickness (0 disables; #129). Airy
    /// isostasy per cell lets a single thin column sink, which the lithosphere's flexural
    /// strength and lower-crustal flow would not allow over one ~110 km cell.
    pub hole_flow_depth: f64,
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
    /// Factor on the load of the plume swells' potential energy in the deforming plates and
    /// in the breakup analysis (1 = physical, 0 disables it; for ablations, #51; see
    /// `MantleFlow::plume_swell_gradient`).
    pub swell_load: f64,
    /// Depth-integrated strength of lithosphere above a plume head that supplies magma,
    /// relative to its strength without, for opening (dyking): magma-assisted rifting needs
    /// about an order of magnitude less tectonic force than amagmatic rifting of thick
    /// lithosphere (Buck 2004, 2006; Bialas, Buck & Qin 2010). 1 disables it (for ablations;
    /// see `weak_zones::magma_weakening`, #51).
    pub magma_weakening: f64,
    /// Factor on the extension fabric that stretching marks (1 = physical, 0: no fabric; for
    /// ablations; see [`Crust::rifted`], #54).
    pub rift_fabric: f64,
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
    /// Bodies of water below sea level of at least this fraction of the surface are seas
    /// connected to the ocean by straits narrower than a cell (see
    /// [`crate::sea::MIN_SEA_FRACTION`]); smaller closed ones are lakes or dry. Off by default
    /// ([`crate::sea::NO_SIZE_RULE`], #173): every closed body is a lake or dry.
    pub min_sea_fraction: f64,
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
    /// Deformation of the plates (non-rigid plates, see [`sheet`]).
    pub sheet: SheetParams,
    /// Breakup of plates by their internal stress (see [`breakup`]).
    pub breakup: BreakupParams,
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
            hole_flow_depth: 300.0,
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
            swell_load: 1.0,
            magma_weakening: 0.1,
            rift_fabric: 1.0,
            ridge_age: 20.0,
            basal_drag: 1e15,
            continental_drag_factor: 4.0,
            collision_resistance: 0.0,
            boundary_coupling_width: 200e3,
            track_material_budget: false,
            ocean_volume: 1.335e18,
            min_sea_fraction: crate::sea::NO_SIZE_RULE,
            mantle: MantleParams::default(),
            matching: MatchingParams::default(),
            events: PlateEventParams::default(),
            polarity_memory: 20.0,
            segment_radius: 500e3,
            polarity_flip_time: 3.0,
            max_substep_movement: 0.5,
            sheet: SheetParams::default(),
            breakup: BreakupParams::default(),
        }
    }
}

impl TectonicParams {
    /// Sets the tunable parameter `name` (the field name, with `events.` for
    /// [`PlateEventParams`] fields and `initial_sediment.` for [`InitialSedimentParams`]
    /// fields, `matching.` for [`MatchingParams`] fields; `initial_sediment.enabled`,
    /// `initial_sediment.biogenic_pelagic`, `matching.distinct` and `mantle.steady_start` are
    /// off for 0 and on otherwise) to `value`.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if let Some(name) = name.strip_prefix("sheet.") {
            return self.sheet.set(name, value);
        }
        if let Some(name) = name.strip_prefix("breakup.") {
            return self.breakup.set(name, value);
        }
        let sediment = &mut self.initial_sediment;
        let field = match name {
            "matching.distinct" => {
                self.matching.distinct = value != 0.0;
                return Ok(());
            }
            "mantle.steady_start" => {
                self.mantle.steady_start = value != 0.0;
                return Ok(());
            }
            "matching.distinct_depth" => {
                self.matching.distinct_depth = value.max(0.0) as usize;
                return Ok(());
            }
            "matching.distinct_radius" => &mut self.matching.distinct_radius,
            "matching.distinct_tolerance" => &mut self.matching.distinct_tolerance,
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
            "hole_flow_depth" => &mut self.hole_flow_depth,
            "boundary_coupling_width" => &mut self.boundary_coupling_width,
            "slab_pull" => &mut self.slab_pull,
            "slab_sinking_speed" => &mut self.slab_sinking_speed,
            "ocean_volume" => &mut self.ocean_volume,
            "min_sea_fraction" => &mut self.min_sea_fraction,
            "rift_width" => &mut self.rift_width,
            "breakup_thickness" => &mut self.breakup_thickness,
            "min_stretching_rate" => &mut self.min_stretching_rate,
            "continental_coast_thickness" => &mut self.continental_coast_thickness,
            "continental_interior_thickness" => &mut self.continental_interior_thickness,
            "continental_margin_width" => &mut self.continental_margin_width,
            "ridge_push" => &mut self.ridge_push,
            "swell_load" => &mut self.swell_load,
            "magma_weakening" => &mut self.magma_weakening,
            "rift_fabric" => &mut self.rift_fabric,
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
    /// A serial number that stays with the plate while its index changes (removing a plate
    /// gives the last one its index): plates born later get larger ids, see
    /// [`Simulation::plate_id`]. For display only.
    id: u32,
    /// The deformation of the plate's lithosphere (see [`sheet`]).
    sheet: sheet::PlateSheet,
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

/// How the surface changes of crust samples shown at several cells were shared so far (Gt;
/// see [`Simulation::apply_surface_changes`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct SurfaceSharing {
    /// Gross surface change of all cells: removed bedrock mass plus the absolute mass of the
    /// sediment changes.
    pub gross: f64,
    /// Part of [`Self::gross`] given to hidden samples as residuals of duplicated samples.
    pub to_hidden: f64,
    /// Part of [`Self::gross`] kept by duplicated samples because their plate had no or too
    /// few hidden samples of their class.
    pub kept: f64,
    /// Requested erosion that found no bedrock left (not applied).
    pub clamped: f64,
}

/// Changes of the sea level (m, as an offset of dry elevations) so far per process. Each term is
/// the difference between two exact solves of the sea level (see
/// [`crate::sea::sea_level`]).
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
    /// Water lost to space by atmospheric escape (see [`Simulation::remove_water`]): the
    /// change when the water is removed. A lower bound of its effect: while the sea level
    /// stands at a pass (see [`Simulation::sea_level_deficit`]) the removed water first reduces
    /// the deficit, and the level falls only when a later process unpins it (in that term).
    pub escape: f64,
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
            + self.escape
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

/// The summed surface changes of the cells showing one crust sample (see
/// [`Simulation::apply_surface_changes`]).
struct SampleChange {
    owner: Ownership,
    /// Number of cells showing the sample, and one of them.
    cells: u32,
    cell: u32,
    thickness: f64,
    sediment: f64,
    /// Whether the column is of continental origin, before the change.
    class: u8,
    /// Residual of the bedrock change as mass (kg/m²), for duplicated samples.
    bedrock_mass: f64,
}

/// Most extra cells' worth of the residual of a plate that one hidden sample takes in one step
/// (see [`Simulation::apply_surface_changes`]).
const MAX_EXTRA_CELLS_PER_HIDDEN_SAMPLE: f64 = 2.0;

/// Passes that hand on what exceeds the cap of the hidden samples (see
/// [`MAX_EXTRA_CELLS_PER_HIDDEN_SAMPLE`]).
const FILL_PASSES: usize = 8;

/// Applies a change of bedrock thickness and sediment thickness (m) to `crust`: deposits are
/// new sediment, sediment removal beyond the available sediment comes from the bedrock at the
/// same mass, and erosion thins the bedrock from the top (never below zero). Returns the
/// applied change of the areal mass and the requested one (they differ only when the bedrock
/// runs out).
fn apply_surface_change(
    crust: &mut Crust,
    thickness: f64,
    sediment: f64,
    cover: &mut CoverBudget,
) -> (f64, f64) {
    let column = *crust;
    let before = crust.mass();
    let expected = thickness * crust.density + sediment * SEDIMENT_ROCK_DENSITY;
    let new_sediment = crust.sediment + sediment;
    let mut new_thickness = crust.thickness + thickness;
    if new_sediment < 0.0 {
        new_thickness += new_sediment * SEDIMENT_ROCK_DENSITY / crust.density;
    }
    if sediment > 0.0 {
        // New deposits are new rock: they do not belong to the cover cohort.
        crust.add_sediment(sediment, Cohort::NONE);
    } else {
        crust.remove_sediment_to(new_sediment.max(0.0));
    }
    // Erosion removes crust from the top; deposition never adds crystalline crust.
    crust.thin_from_top(new_thickness.max(0.0));
    cover.record_change(CoverTerm::Surface, &column, crust);
    (crust.mass() - before, expected)
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
    /// Budget of the continental cover and the cover cohort by process (see [`CoverBudget`]).
    cover_budget: CoverBudget,
    /// The [`CoverMasses`] the cover budget starts from.
    cover_start: CoverMasses,
    /// Sea level as an offset of dry elevations (m) relative to the datum of the isostasy, set
    /// so that the ocean holds [`TectonicParams::ocean_volume`].
    sea_level: f64,
    /// Water held in lakes (m³, see [`Simulation::set_lake_water`]).
    lake_water: f64,
    /// The sea level after the initial setup (m).
    initial_sea_level: f64,
    /// Changes of the sea level so far (m) per process.
    sea_level_budget: SeaLevelBudget,
    /// How surface changes of samples shown at several cells were shared so far.
    surface_sharing: SurfaceSharing,
    /// Simulated time in Myr.
    time: f64,
    /// The id of the next plate born (see [`Plate::id`]).
    next_plate_id: u32,
    /// The forces of the last torque balance, for the deformation of the plates.
    force_balance: Option<dynamics::ForceBalance>,
    sheet_geometry: sheet::SheetGeometry,
    sheet_statistics: SheetStatistics,
    /// Totals of the breakup by stress and the sections of the last step (see [`breakup`]).
    breakup_statistics: BreakupStatistics,
    section_records: Vec<SectionRecord>,
    /// When the sections were last analysed (NaN: never).
    last_section_analysis: f64,
}

/// A tectonic step in progress (see [`Simulation::begin_step`]).
#[derive(Debug, Clone)]
pub struct TectonicStep {
    dt: f64,
    substeps: usize,
    done: usize,
    /// Felsic crust and sediment at the start, for the material budget.
    start: (f64, f64),
}

impl TectonicStep {
    /// Substeps still to take.
    pub fn substeps_left(&self) -> usize {
        self.substeps - self.done
    }
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
            .map(|index| Plate {
                id: index as u32,
                rotation: DQuat::IDENTITY,
                angular_velocity: DVec3::ZERO,
                crust: vec![None; cell_count],
                lookup_cache: (0..cell_count as u32).collect(),
                consumed: vec![false; cell_count],
                tracked: None,
                matched_rotation: DQuat::IDENTITY,
                initiated_at: None,
                sheet: sheet::PlateSheet::new(cell_count),
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
                    rifted: 0.0,
                    reheated: NO_REHEATING,
                    cohort: Cohort::NONE,
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
            cover_budget: CoverBudget::default(),
            cover_start: [0.0; 3],
            sea_level: 0.0,
            lake_water: 0.0,
            initial_sea_level: 0.0,
            sea_level_budget: SeaLevelBudget::default(),
            surface_sharing: SurfaceSharing::default(),
            time: 0.0,
            next_plate_id: plate_count as u32,
            force_balance: None,
            sheet_geometry: sheet::SheetGeometry::default(),
            sheet_statistics: SheetStatistics::default(),
            breakup_statistics: BreakupStatistics::default(),
            section_records: Vec::new(),
            last_section_analysis: f64::NAN,
        };
        simulation.sheet_geometry = sheet::SheetGeometry::new(&simulation.mesh);
        // The sediment cover is part of the initial planet (a stock, not a source); it is
        // added after the plate setup and with its own random numbers, so that it changes
        // nothing else.
        if simulation.params.initial_sediment.enabled {
            simulation.add_initial_sediment(seed, &is_continental, &plate_of_cell);
        }
        simulation.ledger = MassLedger::new(simulation.total_mass());
        simulation.start_cover_budget(0..3);
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

    /// The number of plates; their ids are `0..plate_count()`.
    pub fn plate_count(&self) -> usize {
        self.plates.len()
    }

    /// The effective number of plates `1 / Σ aᵢ²` of the plates' area shares `aᵢ` (an
    /// inverse Simpson index): the plate count weighted towards the large plates, which tiny
    /// plates that come and go hardly change. Equal plates give their count.
    pub fn effective_plate_count(&self) -> f64 {
        let squares: f64 = self.plate_area_fractions().iter().map(|a| a * a).sum();
        if squares > 0.0 { 1.0 / squares } else { 0.0 }
    }

    /// The id of a plate that stays the same while the plate exists, whereas its index
    /// (`plate`) changes when another plate is removed; ids are never reused.
    pub fn plate_id(&self, plate: u32) -> u32 {
        self.plates[plate as usize].id
    }

    /// The orientation of a plate: the rotation from its local frame (in which its crust is
    /// stored) into the world frame.
    pub fn plate_rotation(&self, plate: u32) -> DQuat {
        self.plates[plate as usize].rotation
    }

    /// The rotation axis of a plate scaled by its rotation speed in rad/Myr.
    pub fn angular_velocity(&self, plate: u32) -> DVec3 {
        self.plates[plate as usize].angular_velocity
    }

    /// The crust at the surface of a world cell.
    pub fn surface_crust(&self, cell: u32) -> Crust {
        let owner = self.owner(cell);
        self.plates[owner.plate as usize].crust[owner.local as usize]
            .expect("owners always point to existing crust")
    }

    /// Advances the simulation by `dt` Myr, splitting it into substeps such that no cell moves
    /// more than [`TectonicParams::max_substep_movement`] cell spacings per substep. The same as
    /// [`Simulation::begin_step`], [`Simulation::next_substep`] and [`Simulation::end_step`] (which
    /// the planet uses).
    #[cfg(test)]
    pub fn step(&mut self, dt: f64) {
        let mut step = self.begin_step(dt);
        while step.substeps_left() > 0 {
            self.next_substep(&mut step);
        }
        self.end_step(step);
    }

    /// Starts a step of `dt` Myr (the substeps as in `Simulation::step`), which then takes
    /// [`Simulation::next_substep`] for every substep and [`Simulation::end_step`]. Between these
    /// parts the state is consistent, but the time is part-way through the step.
    pub fn begin_step(&mut self, dt: f64) -> TectonicStep {
        let mantle_velocity = self
            .mantle
            .velocities(&self.mesh, self.params.planet_radius);
        self.update_plate_velocities(&mantle_velocity);
        self.deform_plates(dt);
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
        TectonicStep {
            dt,
            substeps,
            done: 0,
            start: self.felsic_and_sediment(),
        }
    }

    /// Takes the next substep of `step`.
    pub fn next_substep(&mut self, step: &mut TectonicStep) {
        debug_assert!(step.substeps_left() > 0);
        self.substep(step.dt / step.substeps as f64);
        step.done += 1;
    }

    /// Ends `step` after its substeps: crustal flow, plate events, the mantle and the sea level.
    pub fn end_step(&mut self, step: TectonicStep) {
        debug_assert_eq!(step.substeps_left(), 0);
        let TectonicStep { dt, start, .. } = step;
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
        self.record_partitioning(&boundaries);

        let subduction = self.subduction_rates(&boundaries);
        let slabs = self.sinking_slabs(&boundaries);
        self.mantle.step(dt, &subduction, &slabs, &mut self.rng);
        if self.mantle.needs_priming() {
            // The first solve's trenches fill the mantle's memory (#213).
            self.mantle.prime(dt, &subduction, &slabs, &mut self.rng);
        }

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

    /// The planet's water (m³): the ocean's and the lakes' (see
    /// [`TectonicParams::ocean_volume`]).
    pub fn water_volume(&self) -> f64 {
        self.params.ocean_volume
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

    /// Removes `volume` m³ of water from the ocean (lost to space, #210), at most what the ocean
    /// holds besides the lakes; the planet's water ([`TectonicParams::ocean_volume`]) shrinks
    /// and the sea level is solved again.
    pub fn remove_water(&mut self, volume: f64) {
        let volume = volume.clamp(0.0, self.ocean_volume().max(0.0));
        self.params.ocean_volume -= volume;
        self.lake_water = self.lake_water.min(self.params.ocean_volume);
        let sea_level = self.solve_sea_level();
        self.sea_level_budget.escape += sea_level - self.sea_level;
        self.sea_level = sea_level;
    }

    /// The active mantle plumes.
    pub fn plumes(&self) -> &[Plume] {
        self.mantle.plumes()
    }

    /// What the carbon cycle's degassing is computed from (#154): new, subducted and
    /// metamorphosed crust and sediment so far (kg), and the plumes' flux now. Cheap (no
    /// statistics).
    pub fn carbon_drivers(&self) -> crate::carbon::TectonicDrivers {
        let radius = self.params.planet_radius;
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / self.mesh.len() as f64;
        let ledger = &self.ledger;
        crate::carbon::TectonicDrivers {
            new_sea_floor: ledger.new_sea_floor * cell_area,
            subducted_crust: -(ledger.subducted_oceanic + ledger.subducted_transitional)
                * cell_area,
            subducted_sediment: -ledger.subducted_sediment * cell_area,
            metamorphic_sediment: (self.metamorphosed.0 + self.metamorphosed.1) * 1e12
                + self.trench_sediment.stacked * cell_area,
            // `+ 0.0`: the empty sum is −0.
            plume_flux: self.plumes().iter().map(Plume::flux).sum::<f64>() + 0.0,
        }
    }

    fn substep(&mut self, dt: f64) {
        self.time += dt;
        self.move_plates(dt);
        self.transport_crust(dt);
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
            (self.sea_level, self.params.min_sea_fraction),
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
    /// [`crate::sea::sea_level`]); zero otherwise.
    pub fn sea_level_deficit(&self) -> f64 {
        self.solve_sea_level_with_deficit().deficit
    }

    /// The change of the sea level since the start (m): positive where the sea stands higher
    /// against the dry elevations than at the start (see [`Self::sea_level_budget`]).
    pub fn sea_level_change(&self) -> f64 {
        self.sea_level - self.initial_sea_level
    }

    /// The ocean: the cells below sea level connected to the world ocean (see [`crate::sea`]).
    pub fn ocean_mask(&self) -> Vec<bool> {
        crate::sea::ocean_mask(
            &self.mesh,
            &self.surface_elevations(),
            self.params.min_sea_fraction,
            true,
        )
    }

    /// See [`TectonicParams::min_sea_fraction`].
    pub fn min_sea_fraction(&self) -> f64 {
        self.params.min_sea_fraction
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
            (self.sea_level, self.params.min_sea_fraction),
        )
    }

    /// Surface elevation (bedrock plus sediment) of every world cell.
    pub fn surface_elevations(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_elevation(cell))
            .collect()
    }

    /// An id per world cell that the cells showing the same crust sample share (dense, below the
    /// cell count, in order of first appearance).
    pub fn sample_ids(&self) -> Vec<u32> {
        let cell_count = self.mesh.len();
        let mut slot = vec![u32::MAX; self.plates.len() * cell_count];
        let mut next = 0;
        (0..cell_count)
            .map(|cell| {
                let index = &mut slot[sample_index(self.owners[cell], cell_count)];
                if *index == u32::MAX {
                    *index = next;
                    next += 1;
                }
                *index
            })
            .collect()
    }

    /// Surface elevation of every world cell as the surface processes see it (#112, option b):
    /// cells showing the same crust sample get the elevation of that sample at the mean dynamic
    /// topography of those cells, so they are one column at one level (they differ otherwise
    /// only by the dynamic topography of their cells, and the sample takes the mean of their
    /// changes, see [`Self::apply_surface_changes`]). A sample shown at one cell gives exactly
    /// [`Self::surface_elevation`].
    pub fn sample_surface_elevations(&self) -> Vec<f64> {
        let cell_count = self.mesh.len();
        // Per shown sample (dense index as in `apply_surface_changes`): the summed dynamic
        // topography of its cells and their number, summed in cell order.
        let mut slot = vec![u32::MAX; self.plates.len() * cell_count];
        let mut sums: Vec<(f64, u32)> = Vec::new();
        for cell in 0..cell_count {
            let index = &mut slot[sample_index(self.owners[cell], cell_count)];
            if *index == u32::MAX {
                *index = sums.len() as u32;
                sums.push((0.0, 0));
            }
            let sum = &mut sums[*index as usize];
            sum.0 += self.dynamic_topography[cell];
            sum.1 += 1;
        }
        (0..cell_count as u32)
            .map(|cell| {
                let owner = self.owners[cell as usize];
                let (dynamic, count) = sums[slot[sample_index(owner, cell_count)] as usize];
                if count == 1 {
                    self.surface_elevation(cell)
                } else {
                    let crust = self.surface_crust(cell);
                    crate::isostasy::to_surface(
                        crust.dry_elevation(self.time) + dynamic / f64::from(count)
                            - self.sea_level,
                    )
                }
            })
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
                let before = *crust;
                crust.remove_sediment_to(crust.sediment - mass / SEDIMENT_ROCK_DENSITY);
                crust.add_crust(
                    mass / METASEDIMENT_DENSITY,
                    METASEDIMENT_DENSITY,
                    (1.0, tracked),
                    max_thickness,
                );
                self.cover_budget
                    .record_change(CoverTerm::Metamorphism, &before, crust);
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

    /// Area of one cell in m² (every crust sample counts as one cell of area).
    fn cell_area(&self) -> f64 {
        4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / self.mesh.len() as f64
    }

    /// How surface changes of samples shown at several cells were shared so far.
    pub fn surface_sharing(&self) -> SurfaceSharing {
        self.surface_sharing
    }

    /// Applies per-cell changes of crust thickness and sediment thickness to the crust at the
    /// surface, conserving mass without multiplying the change of a cell (#57).
    ///
    /// Each crust sample gets the mean change of the cells showing it, so every cell shows
    /// about its own change. The books treat every sample as one cell of area, so a sample shown
    /// at k cells takes only one k-th of their summed mass change; the rest (the residual) goes
    /// to the samples hidden by the resampling, which hold mass but no area: per plate and
    /// class (column of continental origin or not), over its hidden samples of that class that
    /// lie under the plate's own cells (not under another plate), each in proportion to the
    /// same kind of change at the cell it lies under, at most two extra cells' worth per hidden
    /// sample and step. What they cannot take (no such samples, or too few) stays on the shown
    /// sample (up to the sum of its cells' changes). Thickness changes are metres of the
    /// shown sample's bedrock; residuals are moved as mass and turned back into metres with
    /// each recipient's density.
    ///
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
        let cell_count = self.mesh.len();
        // Per shown sample: owner, cells showing it, one of them, summed thickness and sediment
        // changes.
        let mut changes: Vec<SampleChange> = Vec::new();
        // Per crust sample: its position in `changes`, or `u32::MAX` if hidden.
        let mut index = vec![u32::MAX; self.plates.len() * cell_count];
        for cell in 0..cell_count {
            let owner = self.owners[cell];
            let slot = &mut index[sample_index(owner, cell_count)];
            if *slot == u32::MAX {
                *slot = changes.len() as u32;
                changes.push(SampleChange {
                    owner,
                    cells: 0,
                    cell: cell as u32,
                    thickness: 0.0,
                    sediment: 0.0,
                    class: 0,
                    bedrock_mass: 0.0,
                });
            }
            let change = &mut changes[*slot as usize];
            change.cells += 1;
            change.thickness += thickness_delta[cell];
            change.sediment += sediment_delta[cell];
        }

        // Residuals per plate and class: removed bedrock mass, deposited and removed sediment
        // mass (kg/m², bedrock ≤ 0, deposits ≥ 0, removal ≤ 0), and the number of extra
        // cells (k − 1 summed over the duplicated samples).
        let mut residuals = vec![[[0.0; 3]; 2]; self.plates.len()];
        let mut extra_cells = vec![[0u32; 2]; self.plates.len()];
        let mut change = 0.0;
        let mut clamped = 0.0;
        let mut duplicated = 0;
        let mut gross = 0.0;
        for sample in &mut changes {
            let crust = self.plates[sample.owner.plate as usize].crust[sample.owner.local as usize]
                .as_mut()
                .expect("owners always point to existing crust");
            // The class before the change decides where the residual goes, in every pass.
            sample.class = u8::from(crust.is_continental_origin_column());
            let k = f64::from(sample.cells);
            let (thickness, sediment) = (sample.thickness / k, sample.sediment / k);
            gross += (sample.thickness * crust.density).abs()
                + (sample.sediment * SEDIMENT_ROCK_DENSITY).abs();
            if sample.cells > 1 {
                duplicated += 1;
                // The bedrock residual as mass, at the density the erosion used.
                sample.bedrock_mass = (k - 1.0) * thickness * crust.density;
                let (plate, class) = (sample.owner.plate as usize, usize::from(sample.class));
                let residual = &mut residuals[plate][class];
                residual[0] += sample.bedrock_mass;
                let sediment_mass = (k - 1.0) * sediment * SEDIMENT_ROCK_DENSITY;
                residual[if sediment_mass > 0.0 { 1 } else { 2 }] += sediment_mass;
                extra_cells[plate][class] += sample.cells - 1;
            }
            let (applied, expected) =
                apply_surface_change(crust, thickness, sediment, &mut self.cover_budget);
            change += applied;
            clamped += (expected - applied).abs();
        }

        // Spread the residuals over the hidden samples of the plate and class. Each part goes
        // to the hidden samples in proportion to the same kind of change (bedrock removal,
        // deposition, sediment removal) at the cell they lie under, so a hidden sample takes
        // about the change of its place (equal shares if no cell has such a change). No hidden
        // sample takes more than two extra cells' worth of a part, the excess goes to the
        // others
        // ([`MAX_EXTRA_CELLS_PER_HIDDEN_SAMPLE`]): a plate with few hidden samples would
        // otherwise pile its whole residual onto them, and they would surface as spikes or
        // pits. What they cannot take stays on the duplicated samples.
        // (hidden local vertex, its cell)
        let mut recipients: [Vec<(u32, u32)>; 2] = [Vec::new(), Vec::new()];
        let mut shares: Vec<[f64; 3]> = Vec::new();
        let mut to_hidden = 0.0;
        let mut kept = 0.0;
        for plate in 0..self.plates.len() {
            if extra_cells[plate] == [0, 0] {
                continue;
            }
            recipients[0].clear();
            recipients[1].clear();
            let rotation = self.plates[plate].rotation;
            for (local, crust) in self.plates[plate].crust.iter().enumerate() {
                let Some(crust) = crust else { continue };
                if index[plate * cell_count + local] != u32::MAX {
                    continue;
                }
                // Not under another plate: overridden crust waiting to be consumed is not
                // ground of this plate. The search starts at a cell showing a neighbour.
                let start = self
                    .mesh
                    .neighbors(local as u32)
                    .iter()
                    .map(|&n| index[plate * cell_count + n as usize])
                    .find(|&slot| slot != u32::MAX)
                    .map_or(0, |slot| changes[slot as usize].cell);
                let cell = self
                    .mesh
                    .nearest_from(rotation * self.mesh.vertex(local as u32), start);
                if self.owners[cell as usize].plate as usize != plate {
                    continue;
                }
                recipients[usize::from(crust.is_continental_origin_column())]
                    .push((local as u32, cell));
            }
            for class in 0..2 {
                if extra_cells[plate][class] == 0 {
                    continue;
                }
                let targets = &recipients[class];
                let parts = residuals[plate][class];
                // Weights per part: the same kind of change at the recipient's cell.
                let weight = |part: usize, cell: u32| -> f64 {
                    let cell = cell as usize;
                    match part {
                        0 => (-thickness_delta[cell]).max(0.0),
                        1 => sediment_delta[cell].max(0.0),
                        _ => (-sediment_delta[cell]).max(0.0),
                    }
                };
                let cap = MAX_EXTRA_CELLS_PER_HIDDEN_SAMPLE / f64::from(extra_cells[plate][class]);
                // Shares per recipient and part, filled up to the cap: in proportion to the
                // weights, what exceeds the cap goes to the recipients below it (by weight while
                // any of them has weight, then equally).
                shares.clear();
                shares.resize(targets.len(), [0.0; 3]);
                let mut given = [0.0; 3];
                for part in 0..3 {
                    let mut remaining = 1.0;
                    for _ in 0..FILL_PASSES {
                        if remaining <= 1e-12 {
                            break;
                        }
                        let open = |share: &[f64; 3]| share[part] < cap;
                        let (mut sum, mut count) = (0.0, 0usize);
                        for (share, &(_, cell)) in shares.iter().zip(targets) {
                            if open(share) {
                                sum += weight(part, cell);
                                count += 1;
                            }
                        }
                        if count == 0 {
                            break;
                        }
                        let mut excess = 0.0;
                        for (share, &(_, cell)) in shares.iter_mut().zip(targets) {
                            if !open(share) {
                                continue;
                            }
                            share[part] += remaining
                                * if sum > 0.0 {
                                    weight(part, cell) / sum
                                } else {
                                    1.0 / count as f64
                                };
                            if share[part] > cap {
                                excess += share[part] - cap;
                                share[part] = cap;
                            }
                        }
                        remaining = excess;
                    }
                    given[part] = 1.0 - remaining;
                }
                for (share, &(local, _)) in shares.iter().zip(targets) {
                    let crust = self.plates[plate].crust[local as usize]
                        .as_mut()
                        .expect("recipients have crust");
                    let (applied, expected) = apply_surface_change(
                        crust,
                        parts[0] * share[0] / crust.density,
                        (parts[1] * share[1] + parts[2] * share[2]) / SEDIMENT_ROCK_DENSITY,
                        &mut self.cover_budget,
                    );
                    change += applied;
                    clamped += (expected - applied).abs();
                }
                to_hidden += (0..3)
                    .map(|part| parts[part].abs() * given[part])
                    .sum::<f64>();
                // The rest stays on the duplicated samples of this plate and class.
                let keep = given.map(|given| (1.0 - given).max(0.0));
                if keep.iter().all(|&keep| keep <= 0.0) {
                    continue;
                }
                for sample in changes.iter().filter(|c| {
                    c.cells > 1 && c.owner.plate as usize == plate && usize::from(c.class) == class
                }) {
                    let k = f64::from(sample.cells);
                    let sediment_mass = (k - 1.0) / k * sample.sediment * SEDIMENT_ROCK_DENSITY;
                    let sediment_keep = keep[if sediment_mass > 0.0 { 1 } else { 2 }];
                    let crust = self.plates[plate].crust[sample.owner.local as usize]
                        .as_mut()
                        .expect("owners always point to existing crust");
                    kept += (sample.bedrock_mass * keep[0]).abs()
                        + (sediment_mass * sediment_keep).abs();
                    let (applied, expected) = apply_surface_change(
                        crust,
                        sample.bedrock_mass * keep[0] / crust.density,
                        sediment_mass * sediment_keep / SEDIMENT_ROCK_DENSITY,
                        &mut self.cover_budget,
                    );
                    change += applied;
                    clamped += (expected - applied).abs();
                }
            }
        }
        let to_gt = self.cell_area() / 1e12;
        let sharing = &mut self.surface_sharing;
        sharing.gross += gross * to_gt;
        sharing.to_hidden += to_hidden * to_gt;
        sharing.kept += kept * to_gt;
        sharing.clamped += clamped * to_gt;
        tracing::debug!(
            duplicated,
            to_hidden_gt = to_hidden * to_gt,
            kept_gt = kept * to_gt,
            clamped_gt = clamped * to_gt,
            "Surface changes shared"
        );
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

    /// Water lost to space lowers the sea level by about its volume over the ocean's area, all
    /// of it in the `escape` term of the budget; never more than the ocean holds.
    #[test]
    fn removed_water_lowers_the_sea_level() {
        let mut simulation = simulation(3);
        let before = simulation.sea_level;
        let ocean_cells = simulation
            .surface_elevations()
            .iter()
            .filter(|&&e| e < 0.0)
            .count() as f64;
        let cell_area = 4.0 * std::f64::consts::PI * simulation.params.planet_radius.powi(2)
            / simulation.mesh.len() as f64;
        let volume = 1e15;
        simulation.remove_water(volume);
        let drop = before - simulation.sea_level;
        // The sea floor rises as the water's load goes (Airy).
        let expected = volume / (ocean_cells * cell_area)
            * (1.0 - crate::isostasy::WATER_DENSITY / crate::isostasy::MANTLE_DENSITY);
        assert!((drop / expected - 1.0).abs() < 0.05, "{drop} {expected}");
        let (budget, residual) = simulation.sea_level_budget();
        assert_eq!(budget.escape, simulation.sea_level - before);
        assert!(residual.abs() < 1e-6);
        let water = simulation.water_volume();
        simulation.set_lake_water(0.25 * water);
        simulation.remove_water(2.0 * water);
        assert!((simulation.water_volume() - 0.25 * water).abs() < 1e-6 * water);
        assert!(simulation.ocean_volume().abs() < 1e-6 * water);
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
            let volume = crate::isostasy::ocean_volume(&elevations, cell_area) + pores;
            let target = simulation.ocean_volume();
            let deficit = simulation.sea_level_deficit();
            if deficit > 0.0 {
                // The sea level stands on a sill (or a cell whose pores fill) and spills into a
                // basin it cannot fill: the basin counts as ocean, but holds only the deficit.
                assert!(
                    volume + deficit >= target * (1.0 - 1e-9),
                    "{volume} vs {target}"
                );
                return;
            }
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
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
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
                        rifted: 0.0,
                        reheated: NO_REHEATING,
                        cohort: Cohort::NONE,
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
                    rifted: 0.0,
                    reheated: NO_REHEATING,
                    cohort: Cohort::NONE,
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
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
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
                rifted: 0.0,
                reheated: NO_REHEATING,
                cohort: Cohort::NONE,
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
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
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

    /// The cover budget's recorded terms account for the whole change of the cover and of
    /// both cohort tracers by surface processes and metamorphism (#35); during tectonic steps
    /// the sinks at trenches only remove, and the youngest-first cohort stays at least as large
    /// as the well-mixed one.
    #[test]
    fn cover_budget_records_surface_changes_and_metamorphism() {
        let mut simulation = simulation(3);
        simulation.step(1.0);
        simulation.start_cover_cohort();
        let cells = simulation.mesh.len();
        for step in 0..30 {
            let sites_before = [
                CoverTerm::Subducted,
                CoverTerm::Accreted,
                CoverTerm::Collision,
            ]
            .map(|term| simulation.cover_budget.term(term));
            simulation.step(1.0);
            for (term, before) in [
                CoverTerm::Subducted,
                CoverTerm::Accreted,
                CoverTerm::Collision,
            ]
            .into_iter()
            .zip(sites_before)
            {
                let after = simulation.cover_budget.term(term);
                assert!((0..3).all(|i| after[i] <= before[i] + 1e-9), "{term:?}");
            }
            // Erosion and deposition, deep enough on some cells for metamorphism.
            let thickness: Vec<f64> = (0..cells).map(|c| -((c % 7) as f64) * 10.0).collect();
            let sediment: Vec<f64> = (0..cells)
                .map(|c| match c % 9 {
                    0 => 3000.0,
                    1..=4 => -40.0 * (c % 5) as f64,
                    _ => 20.0,
                })
                .collect();
            let masses = simulation.cover_masses();
            let recorded = simulation.cover_budget.total();
            simulation.apply_surface_changes(&thickness, &sediment, SurfaceSeaLevel::default());
            let change = simulation.cover_masses();
            let recorded_change = simulation.cover_budget.total();
            for i in 0..3 {
                let actual = change[i] - masses[i];
                let booked = recorded_change[i] - recorded[i];
                assert!(
                    (actual - booked).abs() <= 1e-9 * masses[0],
                    "step {step}, mass {i}: changed {actual}, recorded {booked}"
                );
            }
            let left = simulation.cover_cohort();
            assert!(left.anywhere[1] >= left.anywhere[0] - 1e-6 * left.anywhere[0].max(1.0));
        }
        let budget = simulation.cover_budget();
        assert!(budget.term(CoverTerm::Surface)[1] < 0.0);
        assert!(budget.term(CoverTerm::Metamorphism)[0] < 0.0);
    }

    /// Two neighbours of cell 0 on its plate.
    fn same_plate_neighbors(simulation: &Simulation) -> (u32, u32) {
        let plate = simulation.owners[0].plate;
        let mut neighbors = simulation
            .mesh
            .neighbors(0)
            .iter()
            .copied()
            .filter(|&n| simulation.owners[n as usize].plate == plate);
        (neighbors.next().unwrap(), neighbors.next().unwrap())
    }

    fn crust_of(simulation: &Simulation, owner: Ownership) -> Option<Crust> {
        simulation.plates[owner.plate as usize].crust[owner.local as usize]
    }

    /// Mass change (kg/m²) that the erosion asks for: bedrock metres of the shown sample plus
    /// sediment.
    fn requested(simulation: &Simulation, thickness: &[f64], sediment: &[f64]) -> f64 {
        (0..simulation.mesh.len() as u32)
            .map(|cell| {
                let crust = simulation.surface_crust(cell);
                thickness[cell as usize] * crust.density
                    + sediment[cell as usize] * SEDIMENT_ROCK_DENSITY
            })
            .sum()
    }

    #[test]
    fn shared_samples_get_the_mean_and_hidden_samples_the_rest() {
        let mut simulation = simulation(4);
        // Show the sample of cell 0 also at a neighbour; the neighbour's sample is hidden.
        let (neighbor, _) = same_plate_neighbors(&simulation);
        let hidden = simulation.owners[neighbor as usize];
        simulation.owners[neighbor as usize] = simulation.owners[0];
        // The hidden sample is of another density (but the same class).
        let shown = simulation.surface_crust(0);
        {
            let crust = simulation.plates[hidden.plate as usize].crust[hidden.local as usize]
                .as_mut()
                .unwrap();
            *crust = shown;
            crust.density = shown.density + 100.0;
            crust.sediment = 0.0;
        }
        let hidden_before = crust_of(&simulation, hidden).unwrap();
        let cells = simulation.mesh.len();
        let mut thickness = vec![0.0; cells];
        let mut sediment = vec![0.0; cells];
        thickness[0] = -10.0;
        thickness[neighbor as usize] = -20.0;
        sediment[0] = 4.0;
        sediment[neighbor as usize] = 2.0;
        let mass = simulation.total_mass();
        // The edit above is outside the ledger: compare its residual before and after.
        let residual = simulation.ledger.checked(mass).residual;
        let expected = requested(&simulation, &thickness, &sediment);
        simulation.apply_surface_changes(&thickness, &sediment, SurfaceSeaLevel::default());
        // Both cells show the mean, not the sum.
        let after = simulation.surface_crust(neighbor);
        assert!((after.thickness - (shown.thickness - 15.0)).abs() < 1e-9);
        assert!((after.sediment - (shown.sediment + 3.0)).abs() < 1e-9);
        // The hidden sample took the rest, as mass.
        let hidden_after = crust_of(&simulation, hidden).unwrap();
        let removed = (hidden_before.thickness - hidden_after.thickness) * hidden_before.density;
        assert!((removed - 15.0 * shown.density).abs() < 1e-6, "{removed}");
        assert!((hidden_after.sediment - 3.0).abs() < 1e-9);
        // Mass is conserved: the change is what the cells asked for.
        let change = simulation.total_mass() - mass;
        assert!((change - expected).abs() < 1e-3, "{change} vs {expected}");
        let after = simulation.ledger.checked(simulation.total_mass()).residual;
        assert!((after - residual).abs() < 1e-6, "{after} vs {residual}");
    }

    #[test]
    fn cells_showing_one_sample_are_one_column_for_the_surface() {
        let mut simulation = simulation(4);
        let (neighbor, _) = same_plate_neighbors(&simulation);
        simulation.owners[neighbor as usize] = simulation.owners[0];
        // Different dynamic topography under the two cells, on land.
        simulation.dynamic_topography[0] = 40.0;
        simulation.dynamic_topography[neighbor as usize] = -20.0;
        simulation.sea_level = simulation.surface_crust(0).dry_elevation(simulation.time) - 500.0;
        let per_cell = simulation.surface_elevations();
        let merged = simulation.sample_surface_elevations();
        assert!((per_cell[0] - per_cell[neighbor as usize] - 60.0).abs() < 1e-9);
        // Both at the sample's elevation at the mean dynamic topography of its cells (+10 m if
        // no other cell shows it).
        let cells: Vec<usize> = (0..simulation.mesh.len())
            .filter(|&c| simulation.owners[c] == simulation.owners[0])
            .collect();
        let mean = cells
            .iter()
            .map(|&c| simulation.dynamic_topography[c])
            .sum::<f64>()
            / cells.len() as f64;
        let expected = 500.0 + mean;
        assert!((merged[0] - expected).abs() < 1e-9, "{}", merged[0]);
        assert_eq!(merged[0], merged[neighbor as usize]);
        // Every sample shown at one cell keeps its surface bit for bit.
        let mut shown = vec![0u32; simulation.plates.len() * simulation.mesh.len()];
        for cell in 0..simulation.mesh.len() {
            shown[sample_index(simulation.owners[cell], simulation.mesh.len())] += 1;
        }
        for cell in 0..simulation.mesh.len() {
            if shown[sample_index(simulation.owners[cell], simulation.mesh.len())] == 1 {
                assert_eq!(merged[cell].to_bits(), per_cell[cell].to_bits());
            }
        }
    }

    #[test]
    fn hidden_samples_under_another_plate_get_nothing() {
        let mut simulation = simulation(4);
        let (a, b) = same_plate_neighbors(&simulation);
        let (under_other, recipient) =
            (simulation.owners[a as usize], simulation.owners[b as usize]);
        // Cell a shows a sample of another plate, cell b the sample of cell 0. Both hidden
        // samples are of the class of the sample of cell 0.
        let other = (0..simulation.mesh.len())
            .map(|cell| simulation.owners[cell])
            .find(|owner| owner.plate != simulation.owners[0].plate)
            .unwrap();
        simulation.owners[a as usize] = other;
        simulation.owners[b as usize] = simulation.owners[0];
        let shown = simulation.surface_crust(0);
        for owner in [under_other, recipient] {
            simulation.plates[owner.plate as usize].crust[owner.local as usize] = Some(shown);
        }
        let mut thickness = vec![0.0; simulation.mesh.len()];
        thickness[0] = -10.0;
        thickness[b as usize] = -10.0;
        let sediment = vec![0.0; simulation.mesh.len()];
        let mass = simulation.total_mass();
        let expected = requested(&simulation, &thickness, &sediment);
        simulation.apply_surface_changes(&thickness, &sediment, SurfaceSeaLevel::default());
        let untouched = crust_of(&simulation, under_other).unwrap();
        assert_eq!(untouched.thickness, shown.thickness);
        assert_eq!(untouched.sediment, shown.sediment);
        // The other hidden sample took the residual: the cells show −10 m, not −20 m.
        let taken = crust_of(&simulation, recipient).unwrap();
        assert!((taken.thickness - (shown.thickness - 10.0)).abs() < 1e-9);
        assert!((simulation.surface_crust(0).thickness - (shown.thickness - 10.0)).abs() < 1e-9);
        assert!((simulation.total_mass() - mass - expected).abs() < 1e-3);
    }

    #[test]
    fn a_hidden_sample_takes_at_most_two_extra_cells_worth() {
        let mut simulation = simulation(4);
        // The sample of cell 0 is shown at all its same-plate neighbours (4 or more extra
        // cells); only the sample of one neighbour stays as a hidden sample of the same class,
        // the others are consumed.
        let plate = simulation.owners[0].plate;
        let neighbors: Vec<u32> = simulation
            .mesh
            .neighbors(0)
            .iter()
            .copied()
            .filter(|&n| simulation.owners[n as usize].plate == plate)
            .collect();
        assert!(neighbors.len() >= 4, "{neighbors:?}");
        let shown = simulation.surface_crust(0);
        let hidden = simulation.owners[neighbors[0] as usize];
        simulation.plates[hidden.plate as usize].crust[hidden.local as usize] = Some(shown);
        for &n in &neighbors[1..] {
            let gone = simulation.owners[n as usize];
            simulation.plates[gone.plate as usize].crust[gone.local as usize] = None;
        }
        for &n in &neighbors {
            simulation.owners[n as usize] = simulation.owners[0];
        }
        let extra = neighbors.len() as f64;
        let mut thickness = vec![0.0; simulation.mesh.len()];
        thickness[0] = -10.0;
        for &n in &neighbors {
            thickness[n as usize] = -10.0;
        }
        let sediment = vec![0.0; simulation.mesh.len()];
        let mass = simulation.total_mass();
        let expected = requested(&simulation, &thickness, &sediment);
        simulation.apply_surface_changes(&thickness, &sediment, SurfaceSeaLevel::default());
        // The hidden sample takes two extra cells' worth (20 m), the shown sample the rest.
        let taken = shown.thickness - crust_of(&simulation, hidden).unwrap().thickness;
        assert!((taken - 20.0).abs() < 1e-9, "{taken}");
        let on_shown = shown.thickness - simulation.surface_crust(0).thickness;
        assert!(
            (on_shown - (10.0 * (extra + 1.0) - 20.0)).abs() < 1e-9,
            "{on_shown}"
        );
        assert!((simulation.total_mass() - mass - expected).abs() < 1e-3);
    }

    #[test]
    fn removing_more_sediment_than_there_is_takes_bedrock_of_the_same_mass() {
        let mut simulation = simulation(4);
        let (neighbor, _) = same_plate_neighbors(&simulation);
        let hidden = simulation.owners[neighbor as usize];
        simulation.owners[neighbor as usize] = simulation.owners[0];
        let shown = simulation.surface_crust(0);
        simulation.plates[hidden.plate as usize].crust[hidden.local as usize] = Some(Crust {
            sediment: 0.0,
            ..shown
        });
        let mut sediment = vec![0.0; simulation.mesh.len()];
        let removal = shown.sediment + 100.0;
        sediment[0] = -removal;
        sediment[neighbor as usize] = -removal;
        let thickness = vec![0.0; simulation.mesh.len()];
        let mass = simulation.total_mass();
        let expected = requested(&simulation, &thickness, &sediment);
        simulation.apply_surface_changes(&thickness, &sediment, SurfaceSeaLevel::default());
        // The shown sample loses all its sediment and 100 m of sediment's mass in bedrock;
        // the hidden one (no sediment) the whole removal in bedrock.
        let after = simulation.surface_crust(0);
        assert_eq!(after.sediment, 0.0);
        let bedrock = (shown.thickness - after.thickness) * shown.density;
        assert!(
            (bedrock - 100.0 * SEDIMENT_ROCK_DENSITY).abs() < 1e-6,
            "{bedrock}"
        );
        let taken = crust_of(&simulation, hidden).unwrap();
        let bedrock = (shown.thickness - taken.thickness) * shown.density;
        assert!(
            (bedrock - removal * SEDIMENT_ROCK_DENSITY).abs() < 1e-6,
            "{bedrock}"
        );
        assert!((simulation.total_mass() - mass - expected).abs() < 1e-3);
    }

    #[test]
    fn shared_samples_without_hidden_samples_keep_the_sum() {
        let mut simulation = simulation(4);
        // The neighbour shows the sample of cell 0 because its own vertex has no crust (a
        // consumed vertex filled from next door).
        let (neighbor, _) = same_plate_neighbors(&simulation);
        let gone = simulation.owners[neighbor as usize];
        simulation.plates[gone.plate as usize].crust[gone.local as usize] = None;
        simulation.owners[neighbor as usize] = simulation.owners[0];
        let before = simulation.surface_crust(0);
        let mut thickness = vec![0.0; simulation.mesh.len()];
        thickness[0] = -10.0;
        thickness[neighbor as usize] = -20.0;
        let sediment = vec![0.0; simulation.mesh.len()];
        let mass = simulation.total_mass();
        let expected = requested(&simulation, &thickness, &sediment);
        simulation.apply_surface_changes(&thickness, &sediment, SurfaceSeaLevel::default());
        assert!((simulation.surface_crust(0).thickness - (before.thickness - 30.0)).abs() < 1e-9);
        assert!(crust_of(&simulation, gone).is_none());
        assert!((simulation.total_mass() - mass - expected).abs() < 1e-3);
        assert!(simulation.surface_sharing().kept > 0.0);
    }
}
