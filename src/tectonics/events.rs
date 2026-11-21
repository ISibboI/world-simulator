//! Events that change the plate configuration: rifting above mantle plumes, initiation of
//! subduction at old passive margins, suturing of plates after a continental collision, and
//! capture of tiny plates by a neighbour.
//!
//! Subduction between two plates that already share a boundary needs no event: a polarity
//! (which plate would subduct, by buoyancy) exists from their first contact, and the boundary
//! becomes a trench as soon as they converge (see [`super::dynamics`]).

use glam::{DQuat, DVec3};
use rand::RngExt;
use tracing::debug;

use super::dynamics::{BoundaryCell, BoundaryKind};
use super::generation::random_unit_vector;
use super::weak_zones::{Break, PLUME_HEAD_RADIUS};
use super::*;

/// Parameters of plate birth and merging.
#[derive(Clone, Debug)]
pub struct PlateEventParams {
    /// A plume rifts the plate above it after this many Myr.
    pub rift_delay: f64,
    /// Plumes rift plates with continental crust within this angular distance (radians), the
    /// usual setting of plume-related break-up.
    pub rift_continent_radius: f64,
    /// Plumes also rift plates without nearby continental crust that cover at least this
    /// fraction of the surface.
    pub min_oceanic_rift_area: f64,
    /// Neither part of a split plate may cover less than this fraction of the surface, except
    /// the subducting part of a subduction initiation (see [`Self::min_subducting_area`]).
    pub min_plate_area: f64,
    /// The subducting part of a subduction initiation, and the ocean region it starts in, may
    /// cover as little as this fraction of the surface: small enclosed and marginal seas close
    /// by subduction of their floor below their rim (Mediterranean: Calabrian and Hellenic
    /// slabs; South Caspian below the Apsheron–Balkhan sill). The default is the capture size
    /// [`Self::min_plate_area_before_capture`]; plates that subduct are not captured (see
    /// [`Simulation::capture_tiny_plates`]).
    pub min_subducting_area: f64,
    /// No plates are split while there are this many plates.
    pub max_plates: usize,
    /// Oceanic lithosphere at a passive margin may start to subduct from this age in Myr.
    pub initiation_age: f64,
    /// From this age (Myr) on, the hazard of subduction initiation at a passive margin has its
    /// full value; between [`Self::initiation_age`] and this age it rises quadratically.
    pub initiation_full_age: f64,
    /// Mean waiting time (Myr) until 1000 km of passive margin with lithosphere of at least
    /// [`Self::initiation_full_age`] starts to subduct. The 180 Myr old margins of the
    /// Atlantic have not subducted yet, so it is long.
    pub initiation_waiting_time: f64,
    /// Mean waiting time (Myr) until 1000 km of passive margin with lithosphere of at least
    /// [`Self::initiation_full_age`] starts to subduct within [`Self::invasion_reach`] of a
    /// trench where the same plate already subducts: subduction invades old ocean along strike
    /// from existing trench tips (Scotia, Lesser Antilles, Gibraltar into the Atlantic; Duarte et
    /// al. 2013), much faster than at an isolated passive margin. Three Atlantic invasions within
    /// ~50 Myr along a few thousand km of margin near trench tips give ~65–100 Myr.
    pub invasion_waiting_time: f64,
    /// Distance (m) from a trench of the same plate within which passive margins start to
    /// subduct with [`Self::invasion_waiting_time`].
    pub invasion_reach: f64,
    /// Mean waiting time (Myr) until 1000 km of an existing plate boundary with old oceanic
    /// lithosphere on one side starts to subduct. Plate boundaries are weak zones, so
    /// subduction starts there much more easily than at a passive margin (Stern & Gerya 2018).
    pub boundary_initiation_waiting_time: f64,
    /// Whether plates break along their weakest lithosphere (see [`super::weak_zones`]; 1) or
    /// along the reference great circle (0, for comparison).
    pub weak_zone_splits: f64,
    /// Hazard of subduction initiation at an age step inside an oceanic plate (an old oceanic
    /// cell next to oceanic crust of the same plate at least [`Self::initiation_age_contrast`]
    /// younger: a fracture zone), relative to a passive margin of the same age (0 turns it off).
    /// Fracture zones are the classic weak zones for nucleation inside oceanic plates (the old,
    /// dense side sinks below the young side: Izu–Bonin–Mariana; Stern & Bloomer 1992; Stern &
    /// Gerya 2018), but spontaneous collapse is rare, so they are no weaker than passive margins.
    pub fracture_zone_initiation: f64,
    /// At an existing plate boundary, oceanic lithosphere only starts to subduct below oceanic
    /// lithosphere that is at least this much (Myr) younger.
    pub initiation_age_contrast: f64,
    /// Length of slab (m) that a new subduction zone at a passive margin starts with: the
    /// foundering lithosphere that triggers it.
    pub initiation_slab_length: f64,
    /// Displacement (m) on the new thrust that a subduction initiation event starts with (see
    /// the initiation strength in [`super::dynamics`]). The hazards above are calibrated to
    /// initiations that succeeded, so by default the event stands for a completed initiation:
    /// as much as its slab, which leaves no strength to overcome. 0 makes the foundering slab
    /// work against the full strength of the plate (the hazards are then attempt rates).
    pub initiation_displacement: f64,
    /// Plates whose continents have been colliding for this many Myr with the collision stalled
    /// (see [`Self::suture_convergence`]) merge.
    pub suture_time: f64,
    /// A collision counts as stalled while its colliding cells (those not clearly diverging)
    /// converge on average slower than this (cm/yr). A collision that keeps converging stays
    /// a plate boundary (India–Asia after 50 Myr at ~4 cm/yr, Arabia–Eurasia at ~2 cm/yr); plates
    /// merge once convergence across the belt has ceased. The slowest active collision belts
    /// that are still plate boundaries converge at ~0.2–0.5 cm/yr (Alps; PB2002).
    pub suture_convergence: f64,
    /// Minimum number of colliding boundary cells for a pair of plates to count as colliding.
    pub suture_min_cells: usize,
    /// A colliding pair only sutures while its colliding cells are at least this fraction of
    /// its trench cells: plates still subducting along long trenches keep moving independently.
    pub suture_trench_ratio: f64,
    /// Colliding pairs that cannot suture end their collision locally after this many Myr of
    /// contact (slab break-off follows collision onset by ~10 Myr): smaller colliding terranes
    /// (arcs, microcontinents, slivers) move to the plate of the larger continent they collide
    /// with (see [`Simulation::accrete_terranes`]).
    pub terrane_accretion_time: f64,
    /// Largest terrane that accretes, as a fraction of the surface (~1%: India-sized).
    pub terrane_max_area: f64,
    /// A terrane accretes only if it is at most this fraction of the largest colliding
    /// component of the other plate that it touches.
    pub terrane_size_ratio: f64,
    /// A pair accretes terranes at most once per this many Myr (no back-and-forth).
    pub terrane_cooldown: f64,
    /// Plates smaller than this fraction of the surface are captured by a neighbour.
    pub min_plate_area_before_capture: f64,
    /// Detached pieces of plates covering at least this fraction of the surface become plates
    /// of their own; smaller ones are captured by a neighbour.
    pub min_breakup_area: f64,
}

impl Default for PlateEventParams {
    fn default() -> Self {
        Self {
            rift_delay: 20.0,
            rift_continent_radius: 0.17,
            min_oceanic_rift_area: 0.15,
            min_plate_area: 0.01,
            min_subducting_area: 0.003,
            max_plates: 40,
            initiation_age: 80.0,
            initiation_full_age: 180.0,
            initiation_waiting_time: 1000.0,
            invasion_waiting_time: 100.0,
            invasion_reach: 1500e3,
            boundary_initiation_waiting_time: 150.0,
            initiation_age_contrast: 20.0,
            fracture_zone_initiation: 1.0,
            weak_zone_splits: 1.0,
            initiation_slab_length: 150e3,
            initiation_displacement: 150e3,
            suture_time: 30.0,
            suture_convergence: 0.5,
            suture_min_cells: 10,
            suture_trench_ratio: 1.0,
            terrane_accretion_time: 12.0,
            terrane_max_area: 0.01,
            terrane_size_ratio: 0.5,
            terrane_cooldown: 30.0,
            min_plate_area_before_capture: 0.003,
            min_breakup_area: 0.01,
        }
    }
}

/// A place where subduction may start.
#[derive(Clone, Copy, Debug)]
pub(super) enum Initiation {
    /// An oceanic cell at a passive margin.
    PassiveMargin(u32),
    /// An old oceanic cell next to younger oceanic crust of the same plate (a fracture zone).
    FractureZone { cell: u32, young: u32 },
    /// The boundary between two plates at a cell.
    Boundary {
        subducting: u32,
        overriding: u32,
        cell: u32,
    },
}

impl PlateEventParams {
    /// Hazard of subduction initiation (per Myr and metre of passive margin) for oceanic
    /// lithosphere of the given age in Myr: zero when young, rising quadratically from
    /// [`Self::initiation_age`] to [`Self::initiation_full_age`].
    pub fn initiation_hazard(&self, age: f64) -> f64 {
        self.hazard(age, self.initiation_waiting_time)
    }

    /// Like [`Self::initiation_hazard`], near a trench of the same plate (subduction invasion).
    pub fn invasion_hazard(&self, age: f64) -> f64 {
        self.hazard(age, self.invasion_waiting_time)
    }

    /// Like [`Self::initiation_hazard`], at an existing plate boundary.
    pub fn boundary_initiation_hazard(&self, age: f64) -> f64 {
        self.hazard(age, self.boundary_initiation_waiting_time)
    }

    fn hazard(&self, age: f64, waiting_time: f64) -> f64 {
        if age < self.initiation_age {
            return 0.0;
        }
        let maturity = ((age - self.initiation_age)
            / (self.initiation_full_age - self.initiation_age).max(1e-9))
        .min(1.0);
        maturity * maturity / (waiting_time * 1e6)
    }
}

/// How many plate events of each kind happened so far.
#[derive(Clone, Debug, Default)]
pub struct EventCounts {
    /// Plates split by plumes.
    pub rifts: usize,
    /// Plates split at passive margins by subduction initiation.
    pub initiations: usize,
    /// Subduction induced at existing plate boundaries without subduction (no new plate).
    pub induced_subductions: usize,
    /// Subduction induced at the overriding side of a trench, which reverses the polarity.
    pub polarity_reversals: usize,
    /// Detached pieces of plates that became plates of their own.
    pub breakups: usize,
    /// Small detached pieces of plates captured by a neighbour (no change of the plate count).
    pub piece_captures: usize,
    /// Cells in pieces that broke up, and in pieces that were captured.
    pub breakup_cells: usize,
    pub captured_piece_cells: usize,
    /// Plates merged after continental collisions.
    pub sutures: usize,
    /// Tiny plates captured by a neighbour.
    pub captures: usize,
    /// Plates that disappeared by subduction.
    pub consumed: usize,
    /// Passive-margin initiations whose split did not happen (a part would be too small).
    pub failed_initiations: usize,
    /// Splits that did not happen because there were already
    /// [`PlateEventParams::max_plates`] plates.
    pub blocked_splits: usize,
    /// Continental crust volume (thickness × cells, m) lost in merges: hidden samples of the
    /// absorbed plate.
    pub merge_lost_continental_volume: f64,
    /// Length (km) of contacts between crust resisting subduction on both sides created by
    /// plume rifts and passive-margin initiations, in total and farther than
    /// [`PlateEventParams::rift_continent_radius`] from the plume or margin cell.
    /// Terranes accreted to another plate, their cells, and slab segments that broke off
    /// behind them.
    pub terrane_accretions: usize,
    pub accreted_terrane_cells: usize,
    pub slab_breakoffs: usize,
    pub rift_contact_km: f64,
    /// Splits along the weakest lithosphere (see [`super::weak_zones`]), and splits that fell
    /// back to a great circle because no cut along weak lithosphere divided the plate.
    pub weak_zone_splits: usize,
    /// Splits along weak lithosphere found by cutting again after a part was too small.
    pub recut_splits: usize,
    /// Plume rifts whose new boundary passes within half the plume head's radius of the plume.
    pub head_rifts: usize,
    /// Passive-margin initiations whose whole ocean region broke off along its coasts because no
    /// break along weak lithosphere left both parts large enough.
    pub coastal_splits: usize,
    /// Continental cells on orogenic belts and all continental cells, on the boundaries that
    /// rifts created and on the two rifted plates: rifts follow sutures if the first ratio is
    /// higher than the second.
    pub rift_suture_cells: [usize; 4],
    /// Sum of the root-mean-square distances (km) of new boundaries from their best-fitting
    /// great circle, and the number of splits measured: rifts, initiations.
    pub split_deviation_km: [f64; 2],
    pub measured_splits: [usize; 2],
    pub great_circle_splits: usize,
    /// Subduction initiations at age steps (fracture zones) inside oceanic plates.
    pub fracture_zone_initiations: usize,
    /// Initiations undone within 50 Myr by merging their two parts again: by suture, capture
    /// of a tiny plate, capture of a detached piece, terrane accretion.
    pub undone_initiations: [usize; 4],
    /// Failed initiations: no consistent division, overriding (new) part too small,
    /// subducting (old) part too small.
    pub failed_initiation_reasons: [usize; 3],
    pub rift_far_contact_km: f64,
    pub initiation_contact_km: f64,
    pub initiation_far_contact_km: f64,
}

/// Length (rad, ~2600 km) of the lines of cells from a plume across the reference great circle
/// of a rift into the two parts that move apart: the plume head pushes the lithosphere on either
/// side of it apart, so the break nucleates at the plume (longer than the band of
/// [`super::weak_zones`] in which the break forms, so that the lines reach the rigid blocks).
const SPINE_LENGTH: f64 = 0.4;

/// Orientations of the reference great circle through a plume that a rift tries (evenly spaced
/// over 180°): the plume pushes the lithosphere apart in every direction, and the plate breaks
/// where that is easiest.
const RIFT_ORIENTATIONS: usize = 6;

/// How often a split along weak lithosphere that leaves a part too small is cut again.
const MAX_RECUTS: usize = 3;

/// Smallest connected age step (cells) that counts as a fracture zone.
const MIN_FRACTURE_ZONE_CELLS: usize = 3;

/// Initiations whose two parts merge again within this many Myr count as undone.
const UNDONE_INITIATION_TIME: f64 = 50.0;

/// Why two plates merge.
#[derive(Clone, Copy, Debug)]
enum Merge {
    Suture,
    Capture,
    Piece,
    Terrane,
}

/// What a split of a plate is anchored to (see [`Simulation::split_locally`]).
#[derive(Clone, Copy, Debug)]
enum SplitAnchor {
    /// Subduction initiation at the passive margin next to the oceanic `cell`.
    Margin { cell: u32 },
    /// Subduction initiation at an age step inside a plate: the old oceanic `cell` next to the
    /// younger oceanic cell `young`.
    FractureZone { cell: u32, young: u32 },
    /// A plume rift from the cell `nucleus` above the plume; `cut` is a cell of the continent the
    /// rift may divide, if any.
    Rift { nucleus: u32, cut: Option<u32> },
}

impl EventCounts {
    pub fn births(&self) -> usize {
        self.rifts + self.initiations + self.breakups
    }

    pub fn deaths(&self) -> usize {
        self.sutures + self.captures + self.consumed
    }
}

impl Simulation {
    /// Runs the plate events for a time step of `dt` Myr. `boundaries` are the current
    /// [`Self::boundary_cells`], classified with the slab lengths after this step's update.
    pub(super) fn plate_events(&mut self, dt: f64, boundaries: &[BoundaryCell]) {
        let configuration = self.configuration_changes;
        self.suture_colliding_plates(dt, boundaries);
        self.capture_tiny_plates();
        // Splits append plates, so plates from here on are born in this step.
        let first_new_plate = self.plates.len() as u32;
        self.rift_above_plumes();
        if self.configuration_changes == configuration {
            self.initiate_subduction(dt, first_new_plate, boundaries);
        } else {
            let boundaries = self.boundary_cells();
            self.initiate_subduction(dt, first_new_plate, &boundaries);
        }
        self.separate_detached_pieces();
        self.remove_dead_plates();
    }

    /// A rigid plate is one connected region. Pieces of a plate that lost their connection to its
    /// largest piece (its middle was consumed or overridden, like the Farallon plate breaking up
    /// into the Juan de Fuca and Cocos plates) become plates of their own and move
    /// independently; pieces smaller than [`PlateEventParams::min_breakup_area`]
    /// (or beyond the plate limit) are captured by the neighbour they share most boundary with.
    pub(super) fn separate_detached_pieces(&mut self) {
        let cell_count = self.mesh.len();
        let min_cells = self.params.events.min_breakup_area * cell_count as f64;
        let plate_count = self.plates.len() as u32;
        for plate in 0..plate_count {
            let mut pieces = self.plate_pieces(plate);
            if pieces.len() <= 1 {
                continue;
            }
            // The largest piece stays (the first one of equal size).
            let largest = (0..pieces.len())
                .max_by(|&a, &b| pieces[a].len().cmp(&pieces[b].len()).then(b.cmp(&a)))
                .expect("pieces exist");
            pieces.swap_remove(largest);
            for piece in pieces {
                let independent = piece.len() as f64 >= min_cells
                    && self.plates.len() < self.params.events.max_plates;
                let neighbour = if independent {
                    None
                } else {
                    // The neighbour sharing most boundary, ties to the lower index.
                    let mut shared: HashMap<u32, usize> = HashMap::default();
                    for &cell in &piece {
                        for &other in self.mesh.neighbors(cell) {
                            let other = self.owners[other as usize].plate;
                            if other != plate {
                                *shared.entry(other).or_default() += 1;
                            }
                        }
                    }
                    shared
                        .into_iter()
                        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                        .map(|(other, _)| other)
                };
                let Some(new_plate) = self.split_plate_cells(plate, &piece) else {
                    continue;
                };
                match neighbour {
                    Some(neighbour) => {
                        // The new plate is the last one, so merging renumbers no other plate.
                        self.merge_and_record(neighbour, new_plate, Merge::Piece);
                        self.event_counts.piece_captures += 1;
                        self.event_counts.captured_piece_cells += piece.len();
                    }
                    None => {
                        self.event_counts.breakups += 1;
                        self.event_counts.breakup_cells += piece.len();
                        debug!(
                            plate,
                            new_plate,
                            cells = piece.len(),
                            time = self.time,
                            "Plate broke up"
                        );
                    }
                }
            }
        }
    }

    /// The connected pieces (lists of world cells) of a plate.
    pub(super) fn plate_pieces(&self, plate: u32) -> Vec<Vec<u32>> {
        let cell_count = self.mesh.len();
        let mut visited = vec![false; cell_count];
        let mut pieces = Vec::new();
        let mut stack = Vec::new();
        for start in 0..cell_count as u32 {
            if visited[start as usize] || self.owners[start as usize].plate != plate {
                continue;
            }
            visited[start as usize] = true;
            stack.push(start);
            let mut piece = Vec::new();
            while let Some(cell) = stack.pop() {
                piece.push(cell);
                for &neighbor in self.mesh.neighbors(cell) {
                    if !visited[neighbor as usize] && self.owners[neighbor as usize].plate == plate
                    {
                        visited[neighbor as usize] = true;
                        stack.push(neighbor);
                    }
                }
            }
            pieces.push(piece);
        }
        pieces
    }

    /// Area of each plate as a fraction of the surface.
    fn plate_area_fractions(&self) -> Vec<f64> {
        let mut area = vec![0.0; self.plates.len()];
        for owner in &self.owners {
            area[owner.plate as usize] += 1.0;
        }
        let cells = self.mesh.len() as f64;
        area.iter().map(|a| a / cells).collect()
    }

    /// Splits `plate` along its weakest lithosphere near the great circle with the given normal
    /// without cutting through crust that resists subduction (see [`Self::local_split_piece`])
    /// if both parts are large enough. Returns the new plate on the side the normal points to.
    fn split_locally(&mut self, plate: u32, normal: DVec3, anchor: SplitAnchor) -> Option<u32> {
        if self.plates.len() >= self.params.events.max_plates {
            self.event_counts.blocked_splits += 1;
            debug!(
                plate,
                time = self.time,
                "Split blocked by the maximum number of plates"
            );
            return None;
        }
        let initiation = !matches!(anchor, SplitAnchor::Rift { .. });
        let total = self
            .owners
            .iter()
            .filter(|owner| owner.plate == plate)
            .count();
        let params = &self.params.events;
        let cells = self.mesh.len() as f64;
        // A subducting (old) side may be smaller than a regular plate: small seas start to
        // subduct too (see `PlateEventParams::min_subducting_area`).
        let min_old = match anchor {
            SplitAnchor::Rift { .. } => params.min_plate_area,
            SplitAnchor::Margin { .. } | SplitAnchor::FractureZone { .. } => {
                params.min_subducting_area
            }
        } * cells;
        let min_new = params.min_plate_area * cells;
        // A break along weak lithosphere that leaves a part too small is cut again with the
        // rigid block of that part reaching closer to the break, so that the part grows, up to
        // `MAX_RECUTS` times; then the split does not happen.
        let mut failure = 0;
        let mut division = None;
        let weak = params.weak_zone_splits > 0.0;
        let mut scale = [1.0, 1.0];
        for attempt in 0..=MAX_RECUTS + 1 {
            if attempt == MAX_RECUTS + 1 {
                // Last resort at a passive margin: the plate breaks along the continent–ocean
                // transitions, the weak zone of a margin: the whole ocean region subducts.
                if !weak || !matches!(anchor, SplitAnchor::Margin { .. }) {
                    break;
                }
                scale = [0.0, 0.0];
            }
            let Some((new_side, great_circle)) =
                self.local_split_piece(plate, normal, anchor, weak, scale)
            else {
                failure = 0;
                break;
            };
            failure = 0;
            let piece: Vec<u32> = (0..self.mesh.len() as u32)
                .filter(|&cell| new_side[cell as usize])
                .collect();
            if (piece.len() as f64) < min_new {
                failure = 1;
            } else if ((total - piece.len()) as f64) < min_old {
                failure = 2;
            }
            if failure == 0 {
                if scale == [0.0, 0.0] {
                    self.event_counts.coastal_splits += 1;
                } else if attempt > 0 {
                    self.event_counts.recut_splits += 1;
                }
                division = Some((piece, great_circle));
                break;
            }
            debug!(
                plate,
                total,
                new = piece.len(),
                attempt,
                ?anchor,
                time = self.time,
                "Split leaves a part too small"
            );
            if great_circle {
                break;
            }
            scale[failure - 1] *= 0.5;
        }
        let Some((piece, great_circle)) = division else {
            if initiation {
                self.event_counts.failed_initiation_reasons[failure] += 1;
            }
            return None;
        };
        let new_plate = self.split_plate_cells(plate, &piece)?;
        let kind = usize::from(initiation);
        self.record_split_geometry(plate, new_plate, anchor);
        self.event_counts.split_deviation_km[kind] += self.boundary_deviation_km(plate, new_plate);
        self.event_counts.measured_splits[kind] += 1;
        if great_circle {
            self.event_counts.great_circle_splits += 1;
        } else {
            self.event_counts.weak_zone_splits += 1;
        }
        Some(new_plate)
    }

    /// The world cells of `plate` that go to the new plate in [`Self::split_locally`], and
    /// whether the division fell back to the reference great circle:
    ///
    /// * the plate breaks along its weakest lithosphere within a band around the great circle
    ///   with the given normal (see [`super::weak_zones`]); with `weak` false, or if that finds
    ///   no division, along the great circle itself;
    /// * an ocean region of a margin or fracture-zone initiation without contact to other
    ///   plates breaks off whole along its coast;
    /// * each connected component of crust that resists subduction goes wholly to one side
    ///   (splits never create continent–continent contacts), except the continent a rift cuts;
    /// * at a passive-margin initiation the margin's continent goes to the new side, the ocean
    ///   within [`PlateEventParams::rift_continent_radius`] of the margin cell to the old
    ///   (subducting) side, so the new boundary follows the coast there; at a fracture zone the
    ///   younger side is new, the older side old;
    /// * pieces disconnected from their side's main piece (the one holding the margin cell or
    ///   continent; for rifts the largest) move to the other side, so both parts are connected
    ///   (fragments of the plate that touch no other part of it stay where they are).
    ///
    /// Returns `None` if no consistent division is found.
    fn local_split_piece(
        &self,
        plate: u32,
        normal: DVec3,
        anchor: SplitAnchor,
        weak: bool,
        scale: [f64; 2],
    ) -> Option<(Vec<bool>, bool)> {
        let cell_count = self.mesh.len();
        let in_plate = |cell: u32| self.owners[cell as usize].plate == plate;
        // Connected components of crust that resists subduction.
        let mut component = vec![u32::MAX; cell_count];
        let mut component_count = 0u32;
        let mut stack = Vec::new();
        for start in 0..cell_count as u32 {
            if !in_plate(start)
                || component[start as usize] != u32::MAX
                || !self.surface_crust(start).resists_subduction()
            {
                continue;
            }
            let id = component_count;
            component_count += 1;
            component[start as usize] = id;
            stack.push(start);
            while let Some(cell) = stack.pop() {
                for &neighbor in self.mesh.neighbors(cell) {
                    if in_plate(neighbor)
                        && component[neighbor as usize] == u32::MAX
                        && self.surface_crust(neighbor).resists_subduction()
                    {
                        component[neighbor as usize] = id;
                        stack.push(neighbor);
                    }
                }
            }
        }
        let cut = match anchor {
            SplitAnchor::Rift {
                cut: Some(cell), ..
            } => component[cell as usize],
            _ => u32::MAX,
        };
        // The sides of the break through the weakest lithosphere (see `weak_zones`). Rifts may
        // cut only their continent; other crust that resists subduction stays whole.
        let passable = |cell: u32| {
            let id = component[cell as usize];
            id == u32::MAX || id == cut
        };
        let rigid: Vec<u32> = component
            .iter()
            .map(|&id| if id == cut { u32::MAX } else { id })
            .collect();
        let enclosed = match anchor {
            // An ocean region without contact to other plates breaks off along its coast, and so
            // does the ocean region of a margin as a last resort (`scale` zero, see
            // `split_locally`).
            SplitAnchor::Margin { cell } | SplitAnchor::FractureZone { cell, .. } => {
                let region = self.own_region(cell, &passable);
                let coastal = scale == [0.0, 0.0];
                (coastal || !region.iter().any(|&other| self.at_plate_boundary(other))).then(|| {
                    let mut side = vec![true; cell_count];
                    for &other in &region {
                        side[other as usize] = false;
                    }
                    side
                })
            }
            SplitAnchor::Rift { .. } => None,
        };
        let side = if enclosed.is_some() || !weak {
            enclosed
        } else {
            match anchor {
                SplitAnchor::Margin { cell } => {
                    // The margin's continent on the new side, the ocean along the margin on the
                    // old (subducting) side.
                    let mut forced: Vec<(u32, bool)> = Vec::new();
                    for &neighbor in self.mesh.neighbors(cell) {
                        if component[neighbor as usize] != u32::MAX {
                            forced.push((neighbor, true));
                        }
                    }
                    let origin = self.mesh.vertex(cell);
                    let radius = self.params.events.rift_continent_radius;
                    forced.extend(
                        (0..cell_count as u32)
                            .filter(|&other| {
                                in_plate(other)
                                    && component[other as usize] == u32::MAX
                                    && self.mesh.vertex(other).angle_between(origin) <= radius
                            })
                            .map(|other| (other, false)),
                    );
                    forced.push((cell, false));
                    let geometry = Break::Circle { normal };
                    self.weak_zone_cut(plate, &geometry, scale, &forced, &rigid)
                        .map(|(side, _)| side)
                }
                SplitAnchor::FractureZone { cell, young } => {
                    // The plate breaks along the age step: its old side subducts below its
                    // young side.
                    let mut forced = vec![(cell, false), (young, true)];
                    forced.extend(self.fracture_zone_cells(cell));
                    let geometry = Break::Circle { normal };
                    self.weak_zone_cut(plate, &geometry, scale, &forced, &rigid)
                        .map(|(side, _)| side)
                }
                SplitAnchor::Rift { nucleus, .. } => {
                    // The plume pushes the lithosphere apart in every direction; the plate
                    // breaks where that takes the least force, through the plume's weakened
                    // lithosphere and along weak zones around it (see `weak_zones`). The
                    // reference circle runs through the plume in several orientations. The
                    // head pushes the lithosphere on its two sides apart, so the break
                    // nucleates at the plume: lines of cells from the plume, perpendicular to
                    // the circle, are tied to the two parts. The arms may leave the plume in
                    // any direction between these lines, so the angle at the plume is not set.
                    let origin = self.mesh.vertex(nucleus);
                    let width = self.mesh.mean_edge_angle();
                    let total = (0..cell_count).filter(|&c| in_plate(c as u32)).count();
                    let min = self.params.events.min_plate_area * cell_count as f64;
                    (0..RIFT_ORIENTATIONS)
                        .filter_map(|step| {
                            let angle =
                                step as f64 * std::f64::consts::PI / RIFT_ORIENTATIONS as f64;
                            let normal = DQuat::from_axis_angle(origin, angle) * normal;
                            let geometry = Break::Circle { normal };
                            let across = origin.cross(normal);
                            let forced: Vec<(u32, bool)> = (0..cell_count as u32)
                                .filter(|&other| other != nucleus && in_plate(other))
                                .filter_map(|other| {
                                    let r = self.mesh.vertex(other);
                                    (r.dot(across).abs() < width
                                        && r.angle_between(origin) <= SPINE_LENGTH)
                                        .then(|| (other, r.dot(normal) > 0.0))
                                })
                                .collect();
                            self.weak_zone_cut(plate, &geometry, scale, &forced, &rigid)
                        })
                        // The weakest break among those that leave both parts large enough
                        // (else the weakest; the area test cuts it again).
                        .min_by(|a, b| {
                            let small = |side: &[bool]| {
                                let positive = side.iter().filter(|&&s| s).count();
                                (positive as f64) < min || ((total - positive) as f64) < min
                            };
                            small(&a.0).cmp(&small(&b.0)).then(a.1.total_cmp(&b.1))
                        })
                        .map(|(side, _)| side)
                }
            }
        };
        let great_circle = !weak && side.is_none();
        let side = if great_circle {
            (0..cell_count as u32)
                .map(|cell| self.mesh.vertex(cell).dot(normal) > 0.0)
                .collect()
        } else {
            side?
        };
        // The side holding most cells of each component.
        let mut counts = vec![(0usize, 0usize); component_count as usize];
        for cell in 0..cell_count {
            let id = component[cell];
            if id != u32::MAX {
                counts[id as usize].1 += 1;
                if side[cell] {
                    counts[id as usize].0 += 1;
                }
            }
        }
        let majority: Vec<bool> = counts
            .iter()
            .map(|&(positive, size)| 2 * positive > size)
            .collect();
        let mut new_side = vec![false; cell_count];
        for cell in 0..cell_count as u32 {
            if !in_plate(cell) {
                continue;
            }
            let id = component[cell as usize];
            new_side[cell as usize] = if id == u32::MAX || id == cut {
                side[cell as usize]
            } else {
                majority[id as usize]
            };
        }
        // Anchors of the main pieces: (new side, old side).
        let anchors = match anchor {
            SplitAnchor::Margin { cell } => {
                let margin: Vec<u32> = self
                    .mesh
                    .neighbors(cell)
                    .iter()
                    .map(|&neighbor| component[neighbor as usize])
                    .filter(|&id| id != u32::MAX)
                    .collect();
                let first = (0..cell_count as u32)
                    .find(|&other| margin.contains(&component[other as usize]))?;
                for other in 0..cell_count {
                    if margin.contains(&component[other]) {
                        new_side[other] = true;
                    }
                }
                // The trench follows the coast near the initiation: ocean there subducts.
                let origin = self.mesh.vertex(cell);
                let radius = self.params.events.rift_continent_radius;
                for other in 0..cell_count as u32 {
                    if in_plate(other)
                        && component[other as usize] == u32::MAX
                        && self.mesh.vertex(other).angle_between(origin) <= radius
                    {
                        new_side[other as usize] = false;
                    }
                }
                new_side[cell as usize] = false;
                Some((first, cell))
            }
            SplitAnchor::FractureZone { cell, young } => {
                new_side[cell as usize] = false;
                new_side[young as usize] = true;
                Some((young, cell))
            }
            SplitAnchor::Rift { .. } => None,
        };
        // Move pieces disconnected from their side's main piece to the other side.
        let mut piece = vec![u32::MAX; cell_count];
        for _ in 0..8 {
            piece.fill(u32::MAX);
            let mut sizes: Vec<(bool, usize)> = Vec::new();
            // Whether each piece has a neighbour of the plate on the other side.
            let mut touches: Vec<bool> = Vec::new();
            for start in 0..cell_count as u32 {
                if !in_plate(start) || piece[start as usize] != u32::MAX {
                    continue;
                }
                let id = sizes.len() as u32;
                let side = new_side[start as usize];
                let (mut size, mut touching) = (0, false);
                piece[start as usize] = id;
                stack.push(start);
                while let Some(cell) = stack.pop() {
                    size += 1;
                    for &neighbor in self.mesh.neighbors(cell) {
                        if !in_plate(neighbor) {
                            continue;
                        }
                        if new_side[neighbor as usize] != side {
                            touching = true;
                        } else if piece[neighbor as usize] == u32::MAX {
                            piece[neighbor as usize] = id;
                            stack.push(neighbor);
                        }
                    }
                }
                sizes.push((side, size));
                touches.push(touching);
            }
            let main_of = |side: bool| -> Option<u32> {
                match anchors {
                    Some((new, old)) => Some(piece[if side { new } else { old } as usize]),
                    None => (0..sizes.len() as u32)
                        .filter(|&id| sizes[id as usize].0 == side)
                        .max_by_key(|&id| sizes[id as usize].1),
                }
            };
            let mains = [main_of(false), main_of(true)];
            if mains
                .iter()
                .enumerate()
                .any(|(side, main)| main.is_some_and(|id| sizes[id as usize].0 != (side == 1)))
            {
                // An anchor ended up on the wrong side.
                return None;
            }
            let mut flipped = false;
            for cell in 0..cell_count {
                if piece[cell] == u32::MAX {
                    continue;
                }
                let side = new_side[cell];
                // Pieces already detached from the rest of the plate (fragments surrounded by
                // other plates) stay; the connectivity rule handles them.
                if Some(piece[cell]) != mains[side as usize] && touches[piece[cell] as usize] {
                    new_side[cell] = !side;
                    flipped = true;
                }
            }
            if !flipped {
                return Some((new_side, great_circle));
            }
        }
        None
    }

    /// Connected regions (mesh neighbours of the same plate) of cells whose crust does not
    /// resist subduction: the region of each cell (`u32::MAX` for other cells) and the number
    /// of cells of each region.
    fn ocean_regions(&self) -> (Vec<u32>, Vec<usize>) {
        let cell_count = self.mesh.len();
        let ocean: Vec<bool> = (0..cell_count as u32)
            .map(|cell| !self.surface_crust(cell).resists_subduction())
            .collect();
        let mut region = vec![u32::MAX; cell_count];
        let mut sizes = Vec::new();
        let mut stack = Vec::new();
        for start in 0..cell_count {
            if !ocean[start] || region[start] != u32::MAX {
                continue;
            }
            let id = sizes.len() as u32;
            let plate = self.owners[start].plate;
            let mut size = 0;
            region[start] = id;
            stack.push(start as u32);
            while let Some(cell) = stack.pop() {
                size += 1;
                for &neighbor in self.mesh.neighbors(cell) {
                    let neighbor = neighbor as usize;
                    if ocean[neighbor]
                        && region[neighbor] == u32::MAX
                        && self.owners[neighbor].plate == plate
                    {
                        region[neighbor] = id;
                        stack.push(neighbor as u32);
                    }
                }
            }
            sizes.push(size);
        }
        (region, sizes)
    }

    /// Diagnostics of a new boundary between plates `a` and `b` (cells of `b` next to `a`): for
    /// rifts, how many of its continental cells and of all continental cells of both plates
    /// lie on orogenic belts (whether rifts follow sutures); the boundary cells at debug level.
    fn record_split_geometry(&mut self, a: u32, b: u32, anchor: SplitAnchor) {
        let boundary: Vec<u32> = (0..self.mesh.len() as u32)
            .filter(|&cell| {
                self.owners[cell as usize].plate == b
                    && self
                        .mesh
                        .neighbors(cell)
                        .iter()
                        .any(|&neighbor| self.owners[neighbor as usize].plate == a)
            })
            .collect();
        debug!(?anchor, ?boundary, time = self.time, "New plate boundary");
        let SplitAnchor::Rift { nucleus, .. } = anchor else {
            return;
        };
        let origin = self.mesh.vertex(nucleus);
        let distance = boundary
            .iter()
            .map(|&cell| self.mesh.vertex(cell).angle_between(origin))
            .fold(f64::INFINITY, f64::min);
        if distance <= 0.5 * PLUME_HEAD_RADIUS {
            self.event_counts.head_rifts += 1;
        }
        debug!(
            distance,
            continental = self.surface_crust(nucleus).is_continental_origin(),
            time = self.time,
            "Rift boundary distance from the plume"
        );
        let mut counts = [0usize; 4];
        for cell in 0..self.mesh.len() as u32 {
            let plate = self.owners[cell as usize].plate;
            if plate != a && plate != b {
                continue;
            }
            let crust = self.surface_crust(cell);
            if !crust.is_continental_origin() {
                continue;
            }
            let suture = usize::from(crust.orogeny > NO_OROGENY);
            counts[2] += suture;
            counts[3] += 1;
            if boundary.binary_search(&cell).is_ok() {
                counts[0] += suture;
                counts[1] += 1;
            }
        }
        for (total, count) in self.event_counts.rift_suture_cells.iter_mut().zip(counts) {
            *total += count;
        }
    }

    /// Root-mean-square distance (km) of the boundary between plates `a` and `b` (cells of `b`
    /// next to `a`) from its best-fitting great circle: ~0 for a straight cut.
    fn boundary_deviation_km(&self, a: u32, b: u32) -> f64 {
        let cells: Vec<DVec3> = (0..self.mesh.len() as u32)
            .filter(|&cell| {
                self.owners[cell as usize].plate == b
                    && self
                        .mesh
                        .neighbors(cell)
                        .iter()
                        .any(|&neighbor| self.owners[neighbor as usize].plate == a)
            })
            .map(|cell| self.mesh.vertex(cell))
            .collect();
        if cells.len() < 3 {
            return 0.0;
        }
        // The normal of the best-fitting plane through the centre is the eigenvector of the
        // smallest eigenvalue of Σ r rᵀ: power iteration on (trace − Σ r rᵀ).
        let moment = cells.iter().fold(glam::DMat3::ZERO, |sum, r| {
            sum + glam::DMat3::from_cols(*r * r.x, *r * r.y, *r * r.z)
        });
        let trace = moment.x_axis.x + moment.y_axis.y + moment.z_axis.z;
        let shifted = glam::DMat3::from_diagonal(DVec3::splat(trace)) - moment;
        let mut normal = cells[0].cross(cells[cells.len() - 1]) + DVec3::new(0.3, 0.2, 0.1);
        for _ in 0..100 {
            normal = (shifted * normal).normalize_or_zero();
            if normal == DVec3::ZERO {
                return 0.0;
            }
        }
        let mean_square =
            cells.iter().map(|r| r.dot(normal).powi(2)).sum::<f64>() / cells.len() as f64;
        mean_square.sqrt() * self.params.planet_radius / 1000.0
    }

    /// The fracture zone through the old oceanic `cell`: the connected old oceanic cells of its
    /// plate next to oceanic crust of the plate at least
    /// [`PlateEventParams::initiation_age_contrast`] younger (old side: false), and those
    /// younger cells (young side: true).
    fn fracture_zone_cells(&self, cell: u32) -> Vec<(u32, bool)> {
        let plate = self.owners[cell as usize].plate;
        let contrast = self.params.events.initiation_age_contrast;
        let younger = |old: u32| -> Vec<u32> {
            let age = self.surface_crust(old).age(self.time);
            self.mesh
                .neighbors(old)
                .iter()
                .copied()
                .filter(|&neighbor| {
                    let crust = self.surface_crust(neighbor);
                    self.owners[neighbor as usize].plate == plate
                        && crust.is_oceanic()
                        && crust.age(self.time) + contrast <= age
                })
                .collect()
        };
        let mut side = vec![None; self.mesh.len()];
        let mut seen = vec![false; self.mesh.len()];
        seen[cell as usize] = true;
        let mut stack = vec![cell];
        while let Some(old) = stack.pop() {
            let young = younger(old);
            if young.is_empty()
                || !self.surface_crust(old).is_oceanic()
                || side[old as usize] == Some(true)
            {
                continue;
            }
            side[old as usize] = Some(false);
            for young in young {
                if side[young as usize].is_none() {
                    side[young as usize] = Some(true);
                }
            }
            for &neighbor in self.mesh.neighbors(old) {
                if !seen[neighbor as usize] && self.owners[neighbor as usize].plate == plate {
                    seen[neighbor as usize] = true;
                    stack.push(neighbor);
                }
            }
        }
        side.into_iter()
            .enumerate()
            .filter_map(|(cell, side)| side.map(|side| (cell as u32, side)))
            .collect()
    }

    /// The connected cells of the plate of `cell` that `passable` allows, from `cell`.
    fn own_region(&self, cell: u32, passable: &dyn Fn(u32) -> bool) -> Vec<u32> {
        let plate = self.owners[cell as usize].plate;
        let mut seen = vec![false; self.mesh.len()];
        seen[cell as usize] = true;
        let mut region = Vec::new();
        let mut stack = vec![cell];
        while let Some(at) = stack.pop() {
            region.push(at);
            for &neighbor in self.mesh.neighbors(at) {
                if !seen[neighbor as usize]
                    && self.owners[neighbor as usize].plate == plate
                    && passable(neighbor)
                {
                    seen[neighbor as usize] = true;
                    stack.push(neighbor);
                }
            }
        }
        region
    }

    /// Length (km) of the contact between crust resisting subduction on `a` and on `b` (cells
    /// of `a` next to such cells of `b`), in total and farther than
    /// [`PlateEventParams::rift_continent_radius`] from `anchor`.
    fn split_contact_km(&self, a: u32, b: u32, anchor: DVec3) -> (f64, f64) {
        let km_per_cell = self.boundary_length_per_cell() / 1000.0;
        let radius = self.params.events.rift_continent_radius;
        let (mut total, mut far) = (0.0, 0.0);
        for cell in 0..self.mesh.len() as u32 {
            if self.owners[cell as usize].plate != a
                || !self.surface_crust(cell).resists_subduction()
            {
                continue;
            }
            if self.mesh.neighbors(cell).iter().any(|&neighbor| {
                self.owners[neighbor as usize].plate == b
                    && self.surface_crust(neighbor).resists_subduction()
            }) {
                total += km_per_cell;
                if self.mesh.vertex(cell).angle_between(anchor) > radius {
                    far += km_per_cell;
                }
            }
        }
        (total, far)
    }

    /// Marks the crust along the contact of plates `a` and `b` (cells of either next to the
    /// other; on `a` only cells in `only`, if given) as a suture: an orogenic belt formed now
    /// (see [`Crust::orogeny`]).
    fn mark_suture(&mut self, a: u32, b: u32, only: Option<&[bool]>) {
        let time = self.time;
        let selected = |cell: u32| only.is_none_or(|only| only[cell as usize]);
        let contact: Vec<Ownership> = (0..self.mesh.len() as u32)
            .filter(|&cell| {
                let plate = self.owners[cell as usize].plate;
                let neighbors = self.mesh.neighbors(cell);
                if plate == a {
                    selected(cell)
                        && neighbors
                            .iter()
                            .any(|&neighbor| self.owners[neighbor as usize].plate == b)
                } else if plate == b {
                    neighbors.iter().any(|&neighbor| {
                        self.owners[neighbor as usize].plate == a && selected(neighbor)
                    })
                } else {
                    false
                }
            })
            .map(|cell| self.owners[cell as usize])
            .collect();
        for owner in contact {
            if let Some(crust) =
                self.plates[owner.plate as usize].crust[owner.local as usize].as_mut()
                && crust.is_continental_origin()
            {
                crust.orogeny = time;
            }
        }
    }

    /// Merges `absorbed` into `into` and records the continental crust lost.
    fn merge_and_record(&mut self, into: u32, absorbed: u32, kind: Merge) {
        // Both parts of a recent initiation merging again undo it.
        let (a, b) = (
            self.plates[into as usize].initiated_at,
            self.plates[absorbed as usize].initiated_at,
        );
        if let (Some(a), Some(b)) = (a, b)
            && a == b
            && self.time - a <= UNDONE_INITIATION_TIME
        {
            self.event_counts.undone_initiations[kind as usize] += 1;
        }
        let lost = self.merge_plates(into, absorbed);
        self.event_counts.merge_lost_continental_volume += lost;
    }

    /// A plume that has been rising below a plate with continental crust nearby (or below a
    /// very large plate) for long enough splits it along its weakest lithosphere near the plume
    /// (see [`Self::local_split_piece`]). The plume's outflow then drives the two parts apart.
    fn rift_above_plumes(&mut self) {
        let params = self.params.events.clone();
        let candidates: Vec<(usize, DVec3)> = self
            .mantle
            .plumes()
            .iter()
            .enumerate()
            .filter(|(_, plume)| !plume.has_rifted && plume.age >= params.rift_delay)
            .map(|(index, plume)| (index, plume.position))
            .collect();

        for (index, position) in candidates {
            let cell = self.mesh.nearest(position);
            let plate = self.owners[cell as usize].plate;
            // The nearest continental crust of the plate within reach of the plume.
            let continent = (0..self.mesh.len() as u32)
                .filter(|&other| {
                    self.owners[other as usize].plate == plate
                        && self.surface_crust(other).is_continental_origin()
                })
                .map(|other| (other, self.mesh.vertex(other).angle_between(position)))
                .filter(|&(_, angle)| angle < params.rift_continent_radius)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(other, _)| other);
            let large = self.plate_area_fractions()[plate as usize] >= params.min_oceanic_rift_area;
            if continent.is_none() && !large {
                continue;
            }
            // The first reference circle through the plume crosses the nearest continent (else
            // a random one); the split tries the others too (see `local_split_piece`).
            let direction = random_unit_vector(&mut self.rng);
            let random_normal =
                (direction - position * direction.dot(position)).normalize_or_zero();
            let towards = continent.map_or(DVec3::ZERO, |other| {
                position.cross(self.mesh.vertex(other)).normalize_or_zero()
            });
            let normal = if towards == DVec3::ZERO {
                random_normal
            } else {
                towards
            };
            if normal == DVec3::ZERO {
                continue;
            }
            let anchor = SplitAnchor::Rift {
                nucleus: cell,
                cut: continent,
            };
            if let Some(new_plate) = self.split_locally(plate, normal, anchor) {
                self.event_counts.rifts += 1;
                let (total, far) = self.split_contact_km(plate, new_plate, position);
                self.event_counts.rift_contact_km += total;
                self.event_counts.rift_far_contact_km += far;
                debug!(plate, new_plate, time = self.time, "Plume rifted a plate");
            }
            // Each plume rifts at most once, also if the split was not possible.
            self.mantle.mark_rifted(index);
        }
    }

    /// Old oceanic lithosphere may start to sink, at two kinds of places:
    ///
    /// * at a passive margin (ocean and continent on the same plate): the plate splits along
    ///   the margin and the oceanic part subducts below the continental part;
    /// * at an existing plate boundary that is not diverging and where it does not subduct yet
    ///   (transform faults, fracture zones, locked margins, the overriding side of a trench),
    ///   where it meets younger oceanic or continental crust of the other plate: the old side
    ///   starts to subduct below the other plate (induced nucleation, like Izu–Bonin–Mariana at
    ///   a transform fault; at a trench, a polarity reversal: old, dense lithosphere does not
    ///   stay above much younger, buoyant lithosphere).
    ///
    /// Both start with a short slab. Initiation is a random process with a hazard per unit
    /// length that rises with the age of the lithosphere (see
    /// [`PlateEventParams::initiation_waiting_time`]); at most one initiation happens per step,
    /// and not on plates from `first_new_plate` on, which were born in this step. `boundaries`
    /// are the current [`Self::boundary_cells`].
    pub(super) fn initiate_subduction(
        &mut self,
        dt: f64,
        first_new_plate: u32,
        boundaries: &[BoundaryCell],
    ) {
        let candidates = self.initiation_candidates(first_new_plate, boundaries);
        let total: f64 = candidates.iter().map(|(_, hazard)| hazard).sum();
        if total * dt > 0.5 {
            debug!(
                expected = total * dt,
                time = self.time,
                "Subduction initiation saturated: at most one per step"
            );
        }
        if candidates.is_empty() || !self.rng.random_bool(1.0 - (-total * dt).exp()) {
            return;
        }

        let mut pick = self.rng.random_range(0.0..total);
        let initiation = candidates
            .iter()
            .find(|(_, hazard)| {
                pick -= hazard;
                pick <= 0.0
            })
            .map_or(candidates[candidates.len() - 1].0, |(initiation, _)| {
                *initiation
            });
        let (cell, anchor) = match initiation {
            Initiation::PassiveMargin(cell) => (cell, SplitAnchor::Margin { cell }),
            Initiation::FractureZone { cell, young } => {
                (cell, SplitAnchor::FractureZone { cell, young })
            }
            Initiation::Boundary {
                subducting,
                overriding,
                cell,
            } => {
                self.induce_subduction(subducting, overriding, cell);
                return;
            }
        };

        // The margin (or the age step) runs perpendicular to the direction from the old ocean
        // towards the continent (or the younger ocean).
        let position = self.mesh.vertex(cell);
        let plate = self.owners[cell as usize].plate;
        let towards: DVec3 = match anchor {
            // Down the age gradient of the oceanic neighbours of the same plate.
            SplitAnchor::FractureZone { .. } => {
                let age = self.surface_crust(cell).age(self.time);
                self.mesh
                    .neighbors(cell)
                    .iter()
                    .filter(|&&neighbor| {
                        self.owners[neighbor as usize].plate == plate
                            && self.surface_crust(neighbor).is_oceanic()
                    })
                    .map(|&neighbor| {
                        (self.mesh.vertex(neighbor) - position)
                            * (age - self.surface_crust(neighbor).age(self.time))
                    })
                    .sum()
            }
            _ => self
                .mesh
                .neighbors(cell)
                .iter()
                .filter(|&&neighbor| {
                    self.owners[neighbor as usize].plate == plate
                        && self.surface_crust(neighbor).is_continental_origin()
                })
                .map(|&neighbor| self.mesh.vertex(neighbor) - position)
                .sum(),
        };
        let normal = (towards - position * towards.dot(position)).normalize_or_zero();
        if normal == DVec3::ZERO {
            return;
        }
        // The new plate is the continental (younger) side; the old oceanic side subducts below
        // it.
        if let Some(overriding_plate) = self.split_locally(plate, normal, anchor) {
            self.event_counts.initiations += 1;
            let fracture_zone = matches!(anchor, SplitAnchor::FractureZone { .. });
            self.plates[plate as usize].initiated_at = Some(self.time);
            self.plates[overriding_plate as usize].initiated_at = Some(self.time);
            if fracture_zone {
                self.event_counts.fracture_zone_initiations += 1;
            }
            let (total, far) = self.split_contact_km(plate, overriding_plate, position);
            self.event_counts.initiation_contact_km += total;
            self.event_counts.initiation_far_contact_km += far;
            self.seed_margin_subduction(plate, overriding_plate, cell, fracture_zone);
            debug!(
                oceanic_plate = plate,
                overriding_plate,
                fracture_zone,
                time = self.time,
                "Subduction started inside a plate"
            );
        } else {
            self.event_counts.failed_initiations += 1;
        }
    }

    /// Starts subduction of `oceanic_plate` below `overriding_plate` along their new common
    /// boundary near `cell` where old oceanic crust faces continental crust (at a fracture zone:
    /// oceanic crust at least [`PlateEventParams::initiation_age_contrast`] younger), and at
    /// `cell`.
    fn seed_margin_subduction(
        &mut self,
        oceanic_plate: u32,
        overriding_plate: u32,
        cell: u32,
        fracture_zone: bool,
    ) {
        let params = &self.params.events;
        let (slab_length, min_age) = (params.initiation_slab_length, params.initiation_age);
        let contrast = params.initiation_age_contrast;
        let rotations = self.rotations();
        let pair = (oceanic_plate, overriding_plate);
        // Only the stretch of the margin near the initiation, not every coast of the
        // overriding plate.
        let origin = self.mesh.vertex(cell);
        let mut seeds = vec![cell];
        for candidate in 0..self.mesh.len() as u32 {
            if self.owners[candidate as usize].plate != oceanic_plate {
                continue;
            }
            let crust = self.surface_crust(candidate);
            let age = crust.age(self.time);
            if !crust.is_oceanic() || age < min_age {
                continue;
            }
            if self.mesh.neighbors(candidate).iter().any(|&neighbor| {
                let other = self.surface_crust(neighbor);
                self.owners[neighbor as usize].plate == overriding_plate
                    && if fracture_zone {
                        other.is_oceanic() && other.age(self.time) + contrast <= age
                    } else {
                        other.is_continental_origin()
                    }
            }) && self.mesh.vertex(candidate).angle_between(origin)
                <= params.rift_continent_radius
            {
                seeds.push(candidate);
            }
        }
        for seed in seeds {
            let r = self.mesh.vertex(seed);
            if self
                .polarities
                .segment_at(pair.0, pair.1, r, &rotations)
                .is_none()
            {
                let subduction = polarity::Subduction {
                    overriding: overriding_plate,
                    slab_length,
                    displacement: params.initiation_displacement,
                    initiation: true,
                };
                self.polarities
                    .set(pair, subduction, r, self.time, &rotations);
            }
        }
        self.configuration_changes += 1;
    }

    /// Places where subduction may start (see [`Self::initiate_subduction`]) with their hazard
    /// per Myr.
    pub(super) fn initiation_candidates(
        &self,
        first_new_plate: u32,
        boundaries: &[BoundaryCell],
    ) -> Vec<(Initiation, f64)> {
        let params = &self.params.events;
        let margin_length = self.boundary_length_per_cell();
        let min_convergence =
            self.params.min_trench_convergence / (self.params.planet_radius * 100.0 / 1e6);
        // Candidate places with their hazard (per Myr).
        let mut candidates: Vec<(Initiation, f64)> = Vec::new();
        // Old oceanic cells next to continental crust of the same plate (passive margins) or to
        // much younger oceanic crust of the same plate (fracture zones), in a connected ocean
        // region of the plate large enough to subduct as a plate of its own.
        let (ocean_region, ocean_region_cells) = self.ocean_regions();
        let min_cells = params.min_subducting_area * self.mesh.len() as f64;
        let near_trench = self.near_own_trench(boundaries, params.invasion_reach);
        // Candidates at age steps, before the test of their extent.
        let mut steps: Vec<(Initiation, f64)> = Vec::new();
        for cell in 0..self.mesh.len() as u32 {
            let owner = self.owners[cell as usize];
            if owner.plate >= first_new_plate {
                continue;
            }
            let crust = self.surface_crust(cell);
            let age = crust.age(self.time);
            if !crust.is_oceanic()
                || age < params.initiation_age
                || ocean_region_cells
                    .get(ocean_region[cell as usize] as usize)
                    .is_none_or(|&cells| (cells as f64) < min_cells)
            {
                continue;
            }
            let mut at_margin = false;
            let mut at_boundary = false;
            // The youngest much younger oceanic neighbour of the same plate.
            let mut young: Option<(f64, u32)> = None;
            for &neighbor in self.mesh.neighbors(cell) {
                if self.owners[neighbor as usize].plate != owner.plate {
                    at_boundary = true;
                    continue;
                }
                let other = self.surface_crust(neighbor);
                if other.is_continental_origin() {
                    at_margin = true;
                } else if other.is_oceanic() {
                    let other_age = other.age(self.time);
                    if other_age + params.initiation_age_contrast <= age
                        && young.is_none_or(|(youngest, _)| other_age < youngest)
                    {
                        young = Some((other_age, neighbor));
                    }
                }
            }
            if at_margin {
                let hazard = if near_trench[cell as usize] {
                    params.invasion_hazard(age)
                } else {
                    params.initiation_hazard(age)
                };
                candidates.push((Initiation::PassiveMargin(cell), hazard * margin_length));
            } else if let Some((_, young)) = young
                && !at_boundary
                && params.fracture_zone_initiation > 0.0
            {
                steps.push((
                    Initiation::FractureZone { cell, young },
                    params.fracture_zone_initiation * params.initiation_hazard(age) * margin_length,
                ));
            }
        }
        // Fracture zones are continuous age steps, not single-cell age noise of the resampling:
        // only connected steps with at least `MIN_FRACTURE_ZONE_CELLS` cells (~300 km) on both
        // sides count (the young side counted by the youngest neighbour of each old cell).
        let mut on_step = vec![false; self.mesh.len()];
        for (initiation, _) in &steps {
            if let Initiation::FractureZone { cell, .. } = initiation {
                on_step[*cell as usize] = true;
            }
        }
        // Extent of each connected step: the smaller of its old and its young cells.
        let mut young_of = vec![u32::MAX; self.mesh.len()];
        for (initiation, _) in &steps {
            if let Initiation::FractureZone { cell, young } = *initiation {
                young_of[cell as usize] = young;
            }
        }
        let mut step_size = vec![0usize; self.mesh.len()];
        let mut seen = vec![false; self.mesh.len()];
        for (initiation, _) in &steps {
            let Initiation::FractureZone { cell, .. } = *initiation else {
                continue;
            };
            if seen[cell as usize] {
                continue;
            }
            let plate = self.owners[cell as usize].plate;
            seen[cell as usize] = true;
            let mut members = vec![cell];
            let mut index = 0;
            while index < members.len() {
                let at = members[index];
                index += 1;
                for &neighbor in self.mesh.neighbors(at) {
                    if on_step[neighbor as usize]
                        && !seen[neighbor as usize]
                        && self.owners[neighbor as usize].plate == plate
                    {
                        seen[neighbor as usize] = true;
                        members.push(neighbor);
                    }
                }
            }
            let mut young: Vec<u32> = members.iter().map(|&m| young_of[m as usize]).collect();
            young.sort_unstable();
            young.dedup();
            let extent = members.len().min(young.len());
            for &member in &members {
                step_size[member as usize] = extent;
            }
        }
        candidates.extend(steps.into_iter().filter(|(initiation, _)| {
            matches!(initiation, Initiation::FractureZone { cell, .. }
                if step_size[*cell as usize] >= MIN_FRACTURE_ZONE_CELLS)
        }));
        // Old oceanic boundary cells facing younger or continental crust without subduction.
        for boundary in boundaries {
            if !matches!(boundary.kind, BoundaryKind::Other | BoundaryKind::Diffuse)
                || boundary.convergence <= -min_convergence
                || boundary.plate >= first_new_plate
                || boundary.other >= first_new_plate
                || boundary
                    .segment
                    .and_then(|id| self.polarities.segment(boundary.plate, boundary.other, id))
                    .is_some_and(|segment| {
                        segment.slab_length > 0.0 && segment.overriding == boundary.other
                    })
            {
                // Not a candidate, or the plate already subducts below the other one.
                continue;
            }
            let crust = self.surface_crust(boundary.cell);
            let age = crust.age(self.time);
            if !crust.is_oceanic() || age < params.initiation_age {
                continue;
            }
            let Some(&neighbor) = self
                .mesh
                .neighbors(boundary.cell)
                .iter()
                .find(|&&neighbor| self.owners[neighbor as usize].plate == boundary.other)
            else {
                continue;
            };
            let other_crust = self.surface_crust(neighbor);
            if other_crust.is_continental_origin()
                || (other_crust.is_oceanic()
                    && other_crust.age(self.time) + params.initiation_age_contrast <= age)
            {
                candidates.push((
                    Initiation::Boundary {
                        subducting: boundary.plate,
                        overriding: boundary.other,
                        cell: boundary.cell,
                    },
                    params.boundary_initiation_hazard(age) * margin_length,
                ));
            }
        }
        candidates
    }

    /// Whether each cell is within `reach` (m) of a trench cell of its own plate, measured in
    /// rings of cells of that plate.
    fn near_own_trench(&self, boundaries: &[BoundaryCell], reach: f64) -> Vec<bool> {
        let spacing = self.mesh.mean_edge_angle() * self.params.planet_radius;
        let max_rings = (reach / spacing).round() as u32;
        let mut rings = vec![u32::MAX; self.mesh.len()];
        let mut queue = std::collections::VecDeque::new();
        for boundary in boundaries {
            if matches!(boundary.kind, BoundaryKind::Trench { .. })
                && rings[boundary.cell as usize] == u32::MAX
            {
                rings[boundary.cell as usize] = 0;
                queue.push_back(boundary.cell);
            }
        }
        while let Some(cell) = queue.pop_front() {
            let ring = rings[cell as usize];
            if ring >= max_rings {
                continue;
            }
            let plate = self.owners[cell as usize].plate;
            for &neighbor in self.mesh.neighbors(cell) {
                if rings[neighbor as usize] == u32::MAX
                    && self.owners[neighbor as usize].plate == plate
                {
                    rings[neighbor as usize] = ring + 1;
                    queue.push_back(neighbor);
                }
            }
        }
        rings.into_iter().map(|ring| ring <= max_rings).collect()
    }

    /// Starts subduction of `subducting` below `overriding` in the boundary segment at `cell`
    /// with a short slab. If `overriding` subducted below `subducting` there so far, the
    /// polarity reverses.
    fn induce_subduction(&mut self, subducting: u32, overriding: u32, cell: u32) {
        let slab_length = self.params.events.initiation_slab_length;
        let time = self.time;
        let rotations = self.rotations();
        let subduction = polarity::Subduction {
            overriding,
            slab_length,
            displacement: self.params.events.initiation_displacement,
            initiation: true,
        };
        let reversal = self.polarities.set(
            (subducting, overriding),
            subduction,
            self.mesh.vertex(cell),
            time,
            &rotations,
        );
        self.configuration_changes += 1;
        if reversal {
            self.event_counts.polarity_reversals += 1;
        } else {
            self.event_counts.induced_subductions += 1;
        }
        debug!(
            subducting,
            overriding, time, reversal, "Subduction induced at a plate boundary"
        );
    }

    /// Tracks how long pairs of plates have been in continental collision (dominating their
    /// convergent boundary over trenches) with the collision stalled, and merges them once it has
    /// been stalled long enough. Contacts count unless the plates clearly move apart (collision
    /// resistance drives locked collisions to near-zero convergence).
    fn suture_colliding_plates(&mut self, dt: f64, boundaries: &[BoundaryCell]) {
        let min_convergence =
            self.params.min_trench_convergence / (self.params.planet_radius * 100.0 / 1e6);
        let stall_convergence =
            self.params.events.suture_convergence / (self.params.planet_radius * 100.0 / 1e6);
        // Per pair: colliding cells (not clearly diverging), converging colliding cells, trench
        // cells and the summed convergence (rad/Myr) of the colliding cells; collision motion
        // along the smoothed boundary normals (like dynamics).
        let mut contacts: HashMap<(u32, u32), (usize, usize, usize, f64)> = HashMap::default();
        // Colliding (cell, other plate) contacts that converge.
        let mut converging: HashSet<(u32, u32)> = HashSet::default();
        let normals: Vec<DVec3> = boundaries.iter().map(|boundary| boundary.normal).collect();
        for (boundary, &normal) in boundaries.iter().zip(&normals) {
            let pair = pair_key(boundary.plate, boundary.other);
            if boundary.kind == BoundaryKind::Collision {
                let r = self.mesh.vertex(boundary.cell);
                let convergence = (self.plates[boundary.plate as usize].angular_velocity
                    - self.plates[boundary.other as usize].angular_velocity)
                    .cross(r)
                    .dot(normal);
                let entry = contacts.entry(pair).or_default();
                if convergence > -min_convergence {
                    entry.0 += 1;
                    entry.3 += convergence;
                }
                if convergence > min_convergence {
                    entry.1 += 1;
                    converging.insert((boundary.cell, boundary.other));
                }
            } else if matches!(
                boundary.kind,
                BoundaryKind::Trench { .. } | BoundaryKind::Diffuse
            ) {
                // Diffuse zones were incipient trenches before they had their own state: they
                // hold a pair apart like trenches.
                contacts.entry(pair).or_default().2 += 1;
            }
        }
        let min_cells = self.params.events.suture_min_cells;
        // Pairs on their way to a suture: no terrane accretion.
        let mut suturing: HashSet<(u32, u32)> = HashSet::default();
        for (pair, state) in &mut self.polarities.pairs {
            // The continents collide, and collision dominates the convergent boundary: plates
            // that still subduct along long trenches keep moving independently.
            let (colliding, converging_cells, trenches, convergence) =
                contacts.get(pair).copied().unwrap_or_default();
            if colliding >= min_cells
                && colliding as f64 >= self.params.events.suture_trench_ratio * trenches as f64
            {
                state.collision_time += dt;
                // A collision that still converges across the belt is no suture in the making: it
                // stays a plate boundary and may accrete terranes.
                if convergence / (colliding as f64) < stall_convergence {
                    state.stall_time += dt;
                    suturing.insert(*pair);
                } else {
                    state.stall_time = (state.stall_time - dt).max(0.0);
                }
            } else {
                state.collision_time = (state.collision_time - dt).max(0.0);
                state.stall_time = (state.stall_time - dt).max(0.0);
            }
            if converging_cells >= min_cells {
                state.terrane_time += dt;
            } else {
                state.terrane_time = (state.terrane_time - dt).max(0.0);
            }
        }
        let sutures = self.event_counts.sutures;

        let suture_time = self.params.events.suture_time;
        // Merge one pair at a time, since merging renumbers plates.
        while let Some(&(a, b)) = self
            .polarities
            .pairs
            .iter()
            .filter(|(_, state)| state.stall_time >= suture_time)
            .map(|(pair, _)| pair)
            .min()
        {
            let area = self.plate_area_fractions();
            let (into, absorbed) = if area[a as usize] >= area[b as usize] {
                (a, b)
            } else {
                (b, a)
            };
            debug!(into, absorbed, time = self.time, "Plates sutured");
            self.mark_suture(into, absorbed, None);
            self.merge_and_record(into, absorbed, Merge::Suture);
            self.event_counts.sutures += 1;
        }

        // Pairs that converge but cannot suture end their collision locally (not in a step with
        // sutures, which renumber plates; at most once per `terrane_cooldown`).
        if self.event_counts.sutures != sutures {
            return;
        }
        let params = &self.params.events;
        let (accretion_time, cooldown) = (params.terrane_accretion_time, params.terrane_cooldown);
        let time = self.time;
        let mut ready: Vec<(u32, u32)> = self
            .polarities
            .pairs
            .iter()
            .filter(|(pair, state)| {
                state.terrane_time >= accretion_time
                    && !suturing.contains(*pair)
                    && state
                        .last_accretion
                        .is_none_or(|last| time - last >= cooldown)
            })
            .map(|(&pair, _)| pair)
            .collect();
        ready.sort_unstable();
        for (a, b) in ready {
            let accreted =
                self.accrete_terranes(a, b, &converging) + self.accrete_terranes(b, a, &converging);
            if let Some(state) = self.polarities.pairs.get_mut(&(a, b)) {
                state.terrane_time = 0.0;
                if accreted > 0 {
                    state.last_accretion = Some(time);
                }
            }
        }
    }

    /// Terrane accretion: the colliding components of `from` (connected crust that resists
    /// subduction touching such crust of `to`, converging at most of their contact cells, see
    /// `converging`) that are at most
    /// [`PlateEventParams::terrane_size_ratio`] of the largest component of `to` they touch
    /// and at most [`PlateEventParams::terrane_max_area`] of the surface move to `to` (arcs on
    /// an upper plate onto the lower plate's continent, after which the subduction polarity
    /// flips; terranes on a lower plate onto the upper plate, after which the trench steps back
    /// behind them by inheriting the polarity). Pieces of `from` that the removal cuts off from
    /// its largest part move along if small, else nothing moves (fragments that were already
    /// detached stay). The slabs of the pair within a
    /// segment radius of the terrane break off. Returns the number of cells moved. Plate
    /// indices stay the same.
    fn accrete_terranes(&mut self, from: u32, to: u32, converging: &HashSet<(u32, u32)>) -> usize {
        let cell_count = self.mesh.len();
        let max_cells = (self.params.events.terrane_max_area * cell_count as f64) as usize;
        let ratio = self.params.events.terrane_size_ratio;
        let resists: Vec<bool> = (0..cell_count as u32)
            .map(|cell| {
                let plate = self.owners[cell as usize].plate;
                (plate == from || plate == to) && self.surface_crust(cell).resists_subduction()
            })
            .collect();
        // Components of subduction-resisting crust of both plates.
        let mut component = vec![u32::MAX; cell_count];
        let mut sizes: Vec<usize> = Vec::new();
        let mut stack = Vec::new();
        for start in 0..cell_count {
            if !resists[start] || component[start] != u32::MAX {
                continue;
            }
            let id = sizes.len() as u32;
            let plate = self.owners[start].plate;
            let mut size = 0;
            component[start] = id;
            stack.push(start as u32);
            while let Some(cell) = stack.pop() {
                size += 1;
                for &neighbor in self.mesh.neighbors(cell) {
                    let neighbor = neighbor as usize;
                    if resists[neighbor]
                        && component[neighbor] == u32::MAX
                        && self.owners[neighbor].plate == plate
                    {
                        component[neighbor] = id;
                        stack.push(neighbor as u32);
                    }
                }
            }
            sizes.push(size);
        }
        // Per component of `from`: the largest component of `to` it touches, and its contact
        // cells (all, converging).
        let mut opposing = vec![0usize; sizes.len()];
        let mut contact = vec![(0usize, 0usize); sizes.len()];
        for cell in 0..cell_count {
            if !resists[cell] || self.owners[cell].plate != from {
                continue;
            }
            let id = component[cell] as usize;
            let mut touches = false;
            for &neighbor in self.mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                if resists[neighbor] && self.owners[neighbor].plate == to {
                    opposing[id] = opposing[id].max(sizes[component[neighbor] as usize]);
                    touches = true;
                }
            }
            if touches {
                contact[id].0 += 1;
                if converging.contains(&(cell as u32, to)) {
                    contact[id].1 += 1;
                }
            }
        }
        let moving_component = |id: usize| {
            opposing[id] > 0
                && 2 * contact[id].1 > contact[id].0
                && sizes[id] <= max_cells
                && sizes[id] as f64 <= ratio * opposing[id] as f64
        };
        let mut moving: Vec<bool> = (0..cell_count)
            .map(|cell| resists[cell] && moving_component(component[cell] as usize))
            .collect();
        let mut moved = moving.iter().filter(|&&m| m).count();
        if moved == 0 {
            return 0;
        }
        // The rest of `from` must stay connected: pieces the removal cuts off from its largest
        // part move along if small. Connected parts of `from` before the removal:
        let mut part = vec![u32::MAX; cell_count];
        let mut parts = 0u32;
        for start in 0..cell_count {
            if self.owners[start].plate != from || part[start] != u32::MAX {
                continue;
            }
            part[start] = parts;
            stack.push(start as u32);
            while let Some(cell) = stack.pop() {
                for &neighbor in self.mesh.neighbors(cell) {
                    let neighbor = neighbor as usize;
                    if self.owners[neighbor].plate == from && part[neighbor] == u32::MAX {
                        part[neighbor] = parts;
                        stack.push(neighbor as u32);
                    }
                }
            }
            parts += 1;
        }
        let mut piece = vec![u32::MAX; cell_count];
        let mut pieces: Vec<Vec<u32>> = Vec::new();
        for start in 0..cell_count {
            if self.owners[start].plate != from || moving[start] || piece[start] != u32::MAX {
                continue;
            }
            let id = pieces.len() as u32;
            let mut cells = Vec::new();
            piece[start] = id;
            stack.push(start as u32);
            while let Some(cell) = stack.pop() {
                cells.push(cell);
                for &neighbor in self.mesh.neighbors(cell) {
                    let neighbor = neighbor as usize;
                    if self.owners[neighbor].plate == from
                        && !moving[neighbor]
                        && piece[neighbor] == u32::MAX
                    {
                        piece[neighbor] = id;
                        stack.push(neighbor as u32);
                    }
                }
            }
            pieces.push(cells);
        }
        let Some(largest) = (0..pieces.len()).max_by_key(|&id| pieces[id].len()) else {
            // The whole plate would move: that is a suture, not a terrane.
            return 0;
        };
        let main_part = part[pieces[largest][0] as usize];
        let cut_off_pieces: Vec<usize> = (0..pieces.len())
            .filter(|&id| id != largest && part[pieces[id][0] as usize] == main_part)
            .collect();
        let cut_off: usize = cut_off_pieces.iter().map(|&id| pieces[id].len()).sum();
        if cut_off > max_cells {
            return 0;
        }
        for &id in &cut_off_pieces {
            for &cell in &pieces[id] {
                moving[cell as usize] = true;
            }
        }
        moved += cut_off;
        let terrane: Vec<u32> = (0..cell_count as u32)
            .filter(|&cell| moving[cell as usize])
            .collect();
        let positions: Vec<DVec3> = terrane.iter().map(|&cell| self.mesh.vertex(cell)).collect();
        self.mark_suture(from, to, Some(&moving));
        let Some(new_plate) = self.split_plate_cells(from, &terrane) else {
            return 0;
        };
        self.merge_and_record(to, new_plate, Merge::Terrane);
        let rotations = self.rotations();
        let radius = self.polarities.radius();
        self.event_counts.slab_breakoffs +=
            self.polarities
                .break_off((from, to), &positions, radius, &rotations);
        self.event_counts.terrane_accretions += 1;
        self.event_counts.accreted_terrane_cells += moved;
        debug!(
            from,
            to,
            cells = moved,
            time = self.time,
            "Terrane accreted"
        );
        moved
    }

    /// Tiny plates that no longer subduct below a neighbour are captured by the neighbour they
    /// move most similarly to (like fragments of the Farallon plate joining the Pacific plate),
    /// the smallest first.
    fn capture_tiny_plates(&mut self) {
        let min_area = self.params.events.min_plate_area_before_capture;
        loop {
            let area = self.plate_area_fractions();
            // Plates that still subduct below a neighbour keep moving on their own until they
            // are consumed (Juan de Fuca, Cocos); plates whose subduction stopped join a
            // neighbour (fragments of the Farallon plate joining the Pacific plate).
            let subducting: HashSet<u32> = self
                .polarities
                .pairs
                .iter()
                .flat_map(|(&(a, b), state)| {
                    state
                        .segments
                        .iter()
                        .filter(|segment| segment.slab_length > 0.0)
                        .map(move |segment| if segment.overriding == a { b } else { a })
                })
                .collect();
            let Some(tiny) = (0..self.plates.len())
                .filter(|&plate| {
                    area[plate] > 0.0
                        && area[plate] < min_area
                        && !subducting.contains(&(plate as u32))
                })
                .min_by(|&a, &b| area[a].total_cmp(&area[b]).then(a.cmp(&b)))
            else {
                return;
            };
            let tiny = tiny as u32;
            // Shared boundary length and summed relative speed per neighbour.
            let mut neighbours: HashMap<u32, (usize, f64)> = HashMap::default();
            for cell in 0..self.mesh.len() as u32 {
                if self.owners[cell as usize].plate != tiny {
                    continue;
                }
                let r = self.mesh.vertex(cell);
                for &neighbor in self.mesh.neighbors(cell) {
                    let other = self.owners[neighbor as usize].plate;
                    if other != tiny {
                        let relative = (self.plates[tiny as usize].angular_velocity
                            - self.plates[other as usize].angular_velocity)
                            .cross(r)
                            .length();
                        let entry = neighbours.entry(other).or_default();
                        entry.0 += 1;
                        entry.1 += relative;
                    }
                }
            }
            let Some((&into, _)) = neighbours.iter().min_by(|a, b| {
                let mean_a = a.1.1 / a.1.0 as f64;
                let mean_b = b.1.1 / b.1.0 as f64;
                mean_a
                    .total_cmp(&mean_b)
                    .then(b.1.0.cmp(&a.1.0))
                    .then(a.0.cmp(b.0))
            }) else {
                // A plate without neighbours covers the whole planet.
                return;
            };
            debug!(into, tiny, time = self.time, "Tiny plate captured");
            self.merge_and_record(into, tiny, Merge::Capture);
            self.event_counts.captures += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::CONTINENTAL_DENSITY;

    /// One plate: continental where x > 0.3, ocean of the given age elsewhere.
    fn passive_margin(params: TectonicParams, ocean_age: f64) -> Simulation {
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 1, params, 2);
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let x = simulation.mesh.vertex(local as u32).x;
            *crust = Some(if x > 0.3 {
                Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: NO_OROGENY,
                    cohort: 0.0,
                }
            } else {
                new_oceanic_crust(-ocean_age)
            });
        }
        simulation
    }

    #[test]
    fn subduction_invades_passive_margins_near_trenches() {
        let simulation = passive_margin(TectonicParams::default(), 250.0);
        let params = &simulation.params.events;
        let margin_length = simulation.boundary_length_per_cell();
        let candidates = simulation.initiation_candidates(u32::MAX, &[]);
        assert!(!candidates.is_empty());
        let normal = params.initiation_hazard(250.0) * margin_length;
        assert!(
            candidates
                .iter()
                .all(|(_, h)| (h - normal).abs() < 1e-12 * normal)
        );

        // A trench of the same plate in the ocean near the margin at −y; the margin cells within
        // the invasion reach of it start to subduct much more readily.
        let trench_cell = simulation
            .mesh
            .nearest(DVec3::new(0.2, -0.98, 0.0).normalize());
        assert!(simulation.surface_crust(trench_cell).is_oceanic());
        let trench = BoundaryCell {
            cell: trench_cell,
            plate: 0,
            other: 0,
            normal: DVec3::Z,
            convergence: 0.0,
            kind: BoundaryKind::Trench { slab_age: 250.0 },
            segment: None,
        };
        let candidates = simulation.initiation_candidates(u32::MAX, &[trench]);
        let invasion = params.invasion_hazard(250.0) * margin_length;
        let near = candidates
            .iter()
            .filter(|(_, h)| (h - invasion).abs() < 1e-12 * invasion)
            .count();
        assert!(
            near > 0 && near < candidates.len(),
            "{near} of {}",
            candidates.len()
        );
        assert!(invasion > 5.0 * normal);
    }

    #[test]
    fn initiation_hazard_rises_with_age() {
        let params = PlateEventParams::default();
        assert_eq!(params.initiation_hazard(params.initiation_age - 1.0), 0.0);
        let mut previous = 0.0;
        for age in [90.0, 120.0, 150.0, 179.0] {
            let hazard = params.initiation_hazard(age);
            assert!(hazard > previous, "{age}");
            previous = hazard;
        }
        assert_eq!(
            params.initiation_hazard(180.0),
            params.initiation_hazard(400.0)
        );
        // 1000 km of old margin waits on average the waiting time.
        let waiting = 1.0 / (params.initiation_hazard(400.0) * 1e6);
        assert!((waiting - params.initiation_waiting_time).abs() < 1e-9);
    }

    #[test]
    fn young_passive_margins_do_not_subduct() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = passive_margin(params, 70.0);
        for _ in 0..10 {
            simulation.initiate_subduction(1.0, u32::MAX, &[]);
        }
        assert_eq!(simulation.plates.len(), 1);
    }

    #[test]
    fn plates_born_in_the_same_step_do_not_subduct() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = passive_margin(params, 250.0);
        simulation.initiate_subduction(1.0, 0, &[]);
        assert_eq!(simulation.plates.len(), 1);
    }

    /// Two resting oceanic plates of the given ages without subduction polarity.
    fn resting_oceanic_plates(ages: [f64; 2]) -> Simulation {
        let mut params = TectonicParams::default();
        params.events.boundary_initiation_waiting_time = 1e-6;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 2, params, 3);
        simulation.polarities.clear();
        for (plate, age) in simulation.plates.iter_mut().zip(ages) {
            plate.angular_velocity = DVec3::ZERO;
            for crust in plate.crust.iter_mut().flatten() {
                *crust = new_oceanic_crust(-age);
            }
        }
        simulation
    }

    #[test]
    fn old_ocean_starts_subducting_below_younger_ocean_at_a_boundary() {
        let mut simulation = resting_oceanic_plates([200.0, 30.0]);
        let boundaries = simulation.boundary_cells();
        assert!(boundaries.iter().all(|b| b.kind == BoundaryKind::Other));
        simulation.initiate_subduction(1.0, u32::MAX, &boundaries);
        assert_eq!(simulation.plates.len(), 2);
        assert_eq!(simulation.event_counts.induced_subductions, 1);
        // One segment: the old plate subducts there.
        let segments = simulation.polarities.pair_segments(0, 1);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].overriding, 1);
        // The old plate's side of the boundary is a trench there, and only there.
        let boundaries = simulation.boundary_cells();
        let trenches = boundaries
            .iter()
            .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
            .count();
        let old_side = boundaries.iter().filter(|b| b.plate == 0).count();
        assert!(
            trenches > 0 && trenches < old_side,
            "{trenches} of {old_side}"
        );
        assert!(
            boundaries
                .iter()
                .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
                .all(|b| b.plate == 0)
        );
    }

    #[test]
    fn old_overriding_ocean_reverses_the_subduction_polarity() {
        let mut simulation = resting_oceanic_plates([200.0, 30.0]);
        // The young plate subducts below the old one.
        simulation.seed_subduction((0, 1), 0, 600e3);
        let segments = simulation.polarities.pair_segments(0, 1).len();
        assert!(segments > 1);
        let boundaries = simulation.boundary_cells();
        simulation.initiate_subduction(1.0, u32::MAX, &boundaries);
        assert_eq!(simulation.event_counts.polarity_reversals, 1);
        // Exactly one segment reversed, with a new short slab; the others are unchanged.
        let segments = simulation.polarities.pair_segments(0, 1);
        let reversed: Vec<_> = segments.iter().filter(|s| s.overriding == 1).collect();
        assert_eq!(reversed.len(), 1);
        assert_eq!(
            reversed[0].slab_length,
            simulation.params.events.initiation_slab_length
        );
        assert!(
            segments
                .iter()
                .filter(|s| s.overriding == 0)
                .all(|s| s.slab_length == 600e3)
        );
    }

    #[test]
    fn ocean_of_similar_age_does_not_start_subducting_at_a_boundary() {
        let mut simulation = resting_oceanic_plates([200.0, 190.0]);
        let boundaries = simulation.boundary_cells();
        simulation.initiate_subduction(1.0, u32::MAX, &boundaries);
        assert_eq!(simulation.event_counts.induced_subductions, 0);
        assert!(simulation.polarities.is_empty());
    }

    #[test]
    fn old_passive_margins_start_subducting() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = passive_margin(params, 250.0);
        simulation.initiate_subduction(1.0, u32::MAX, &[]);
        assert_eq!(simulation.plates.len(), 2);
        // The oceanic plate subducts below the continental one along the margin near the
        // initiation (a nucleus; segments propagate along strike as the trench grows).
        let segments = simulation.polarities.pair_segments(0, 1);
        assert!(!segments.is_empty());
        assert!(
            segments
                .iter()
                .all(|s| s.overriding == 1 && s.slab_length > 0.0)
        );
        let continental_cells = (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.owners[cell as usize].plate == 1)
            .filter(|&cell| simulation.surface_crust(cell).is_continental())
            .count();
        assert!(continental_cells > 0);
    }

    /// One plate with two continents (caps around +x and +y) and old ocean elsewhere.
    fn two_continents(params: TectonicParams, seed: u64) -> Simulation {
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 1, params, seed);
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let r = simulation.mesh.vertex(local as u32);
            *crust = Some(if r.x > 0.75 || r.y > 0.75 {
                Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: NO_OROGENY,
                    cohort: 0.0,
                }
            } else {
                new_oceanic_crust(-250.0)
            });
        }
        simulation
    }

    /// The plates showing the continent around the given axis.
    fn plates_of_continent(simulation: &Simulation, axis: DVec3) -> HashSet<u32> {
        (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.mesh.vertex(cell).dot(axis) > 0.75)
            .map(|cell| simulation.owners[cell as usize].plate)
            .collect()
    }

    #[test]
    fn passive_margin_initiation_does_not_cut_other_continents() {
        for seed in 0..8 {
            let mut params = TectonicParams::default();
            params.events.initiation_waiting_time = 1e-6;
            let mut simulation = two_continents(params, seed);
            let mass = simulation.total_mass();
            simulation.initiate_subduction(1.0, u32::MAX, &[]);
            assert_eq!(simulation.plates.len(), 2, "seed {seed}");
            assert_eq!(simulation.event_counts.initiation_contact_km, 0.0);
            assert_eq!(plates_of_continent(&simulation, DVec3::X).len(), 1);
            assert_eq!(plates_of_continent(&simulation, DVec3::Y).len(), 1);
            assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
            // Subduction is seeded only near the initiation, not along every coast.
            let segments = simulation.polarities.pair_segments(0, 1);
            assert!(!segments.is_empty());
            assert!(segments.iter().all(|s| s.overriding == 1));
            // Both plates are connected.
            simulation.separate_detached_pieces();
            assert_eq!(simulation.plates.len(), 2, "seed {seed}");
        }
    }

    #[test]
    fn small_enclosed_seas_start_subducting_below_their_rim() {
        // A continent covering the planet except an old sea around +z of ~0.8% of the surface,
        // below the regular minimum plate area.
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 1, params, 0);
        let sea = |r: DVec3| r.z > 0.984;
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let r = simulation.mesh.vertex(local as u32);
            *crust = Some(if sea(r) {
                new_oceanic_crust(-250.0)
            } else {
                Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: NO_OROGENY,
                    cohort: 0.0,
                }
            });
        }
        let sea_cells: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&cell| sea(simulation.mesh.vertex(cell)))
            .collect();
        let fraction = sea_cells.len() as f64 / simulation.mesh.len() as f64;
        assert!((0.006..0.01).contains(&fraction), "sea covers {fraction}");
        let mass = simulation.total_mass();
        simulation.initiate_subduction(1.0, u32::MAX, &[]);
        assert_eq!(simulation.event_counts.initiations, 1);
        assert_eq!(simulation.plates.len(), 2);
        // The whole sea is the subducting plate; the rim overrides it.
        let sea_plate = simulation.owners[sea_cells[0] as usize].plate;
        assert!(
            sea_cells
                .iter()
                .all(|&cell| simulation.owners[cell as usize].plate == sea_plate)
        );
        let plate_cells = simulation
            .owners
            .iter()
            .filter(|owner| owner.plate == sea_plate)
            .count();
        assert_eq!(plate_cells, sea_cells.len());
        let segments = simulation.polarities.pair_segments(0, 1);
        assert!(!segments.is_empty());
        assert!(segments.iter().all(|s| s.overriding != sea_plate));
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
    }

    /// Plate 0: old ocean (250 Myr) where x > 0, younger ocean (100 Myr) elsewhere; plate 1: a
    /// cap of younger ocean around −z.
    fn fracture_zone(params: TectonicParams) -> Simulation {
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 1, params, 0);
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let r = simulation.mesh.vertex(local as u32);
            let old = r.x > 0.0 && r.z > -0.7;
            *crust = Some(new_oceanic_crust(if old { -250.0 } else { -100.0 }));
        }
        let cap: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.mesh.vertex(cell).z < -0.7)
            .collect();
        simulation.split_plate_cells(0, &cap).unwrap();
        simulation.polarities.clear();
        simulation
    }

    #[test]
    fn old_ocean_subducts_below_younger_ocean_at_fracture_zones() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = fracture_zone(params);
        let candidates = simulation.initiation_candidates(u32::MAX, &[]);
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|(initiation, _)| matches!(
            initiation,
            Initiation::FractureZone { cell, young }
                if simulation.mesh.vertex(*cell).x > 0.0
                    && simulation.mesh.vertex(*young).x <= 0.0
        )));
        let mass = simulation.total_mass();
        simulation.initiate_subduction(1.0, u32::MAX, &[]);
        assert_eq!(
            simulation.event_counts.fracture_zone_initiations, 1,
            "{:?}",
            simulation.event_counts
        );
        assert_eq!(simulation.plates.len(), 3);
        // The young side overrides the old side.
        let old = simulation.owners[simulation.mesh.nearest(DVec3::X) as usize].plate;
        let young = simulation.owners[simulation.mesh.nearest(-DVec3::X) as usize].plate;
        assert_ne!(old, young);
        let segments = simulation.polarities.pair_segments(old, young);
        assert!(!segments.is_empty());
        assert!(segments.iter().all(|s| s.overriding == young));
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
    }

    #[test]
    fn uniform_ocean_has_no_fracture_zones() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = fracture_zone(params);
        for plate in &mut simulation.plates {
            for crust in plate.crust.iter_mut().flatten() {
                crust.birth = -250.0;
            }
        }
        assert!(simulation.initiation_candidates(u32::MAX, &[]).is_empty());
        // Single-cell age pits are resampling noise, not fracture zones.
        let pit = simulation.mesh.nearest(DVec3::X);
        simulation.plates[0].crust[simulation.owners[pit as usize].local as usize]
            .as_mut()
            .unwrap()
            .birth = -10.0;
        assert!(simulation.initiation_candidates(u32::MAX, &[]).is_empty());
    }

    #[test]
    fn plates_with_detached_cells_can_still_split() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = two_continents(params, 0);
        // An ocean cap around −z becomes another plate, except one cell in its middle, which
        // stays a detached fragment of plate 0.
        let fragment = simulation.mesh.nearest(-DVec3::Z);
        let cap: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.mesh.vertex(cell).z < -0.7 && cell != fragment)
            .collect();
        simulation.split_plate_cells(0, &cap).unwrap();
        simulation.polarities.clear();
        assert_eq!(simulation.owners[fragment as usize].plate, 0);
        simulation.initiate_subduction(1.0, u32::MAX, &[]);
        assert_eq!(
            simulation.event_counts.initiations, 1,
            "{:?}",
            simulation.event_counts
        );
        assert_eq!(simulation.event_counts.failed_initiations, 0);
        assert_eq!(simulation.plates.len(), 3);
    }

    #[test]
    fn plume_rifts_cut_only_the_nearby_continent() {
        for seed in 0..8 {
            let mut simulation = two_continents(TectonicParams::default(), seed);
            // A plume inside the +x continent.
            simulation.mantle.insert_plume(Plume {
                position: DVec3::new(0.95, 0.0, 0.3).normalize(),
                age: 50.0,
                lifetime: 200.0,
                peak_flux: 5e-3,
                has_rifted: false,
            });
            simulation.rift_above_plumes();
            assert_eq!(simulation.plates.len(), 2, "seed {seed}");
            assert_eq!(plates_of_continent(&simulation, DVec3::X).len(), 2);
            assert_eq!(plates_of_continent(&simulation, DVec3::Y).len(), 1);
            assert!(simulation.event_counts.rift_contact_km > 0.0);
        }
    }

    #[test]
    fn plume_rifts_run_through_the_plume() {
        for seed in 0..4 {
            let mut simulation = two_continents(TectonicParams::default(), seed);
            // Another plate around -z, so that the rift can end at plate boundaries.
            let cap: Vec<u32> = (0..simulation.mesh.len() as u32)
                .filter(|&cell| simulation.mesh.vertex(cell).z < -0.8)
                .collect();
            simulation.split_plate_cells(0, &cap).unwrap();
            simulation.polarities.clear();
            let position = DVec3::new(0.95, 0.0, 0.3).normalize();
            simulation.mantle.insert_plume(Plume {
                position,
                age: 50.0,
                lifetime: 200.0,
                peak_flux: 5e-3,
                has_rifted: false,
            });
            simulation.rift_above_plumes();
            assert_eq!(simulation.event_counts.rifts, 1, "seed {seed}");
            let plume = simulation.mesh.nearest(position);
            let plate = simulation.owners[plume as usize].plate;
            // The new boundary passes next to the plume.
            let distance = (0..simulation.mesh.len() as u32)
                .filter(|&cell| simulation.owners[cell as usize].plate != plate)
                .map(|cell| simulation.mesh.vertex(cell).angle_between(position))
                .fold(f64::INFINITY, f64::min);
            assert!(distance < 0.13, "seed {seed}: {distance}");
            assert_eq!(simulation.event_counts.head_rifts, 1, "seed {seed}");
        }
    }

    #[test]
    fn plume_rift_arms_follow_sutures_around_the_plume() {
        // A belt of old continent around the equator between two polar caps of another plate,
        // a plume at +x, and two old sutures leaving the plume towards the caps, each leaning
        // `lean` towards -y: the break runs through the plume along both sutures, so the
        // angle at the plume follows them (180° − 2 × lean) instead of being prescribed.
        let plume = DVec3::X;
        for (lean, seed) in [(30.0f64, 0), (30.0, 1), (15.0, 2), (15.0, 3), (45.0, 4)] {
            let mut simulation =
                Simulation::new(SphereMesh::icosphere(5), 1, TectonicParams::default(), seed);
            let lean_rad = lean.to_radians();
            let arms = [
                -DVec3::Z * lean_rad.cos() - DVec3::Y * lean_rad.sin(),
                DVec3::Z * lean_rad.cos() - DVec3::Y * lean_rad.sin(),
            ];
            let width = simulation.mesh.mean_edge_angle();
            for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
                let r = simulation.mesh.vertex(local as u32);
                let suture = arms.iter().any(|&arm| {
                    r.dot(arm) > 0.0
                        && r.angle_between(plume) < 0.9
                        && r.dot(plume.cross(arm)).abs().asin() < width
                });
                *crust = Some(Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: if suture { -500.0 } else { NO_OROGENY },
                    cohort: 0.0,
                });
            }
            let caps: Vec<u32> = (0..simulation.mesh.len() as u32)
                .filter(|&cell| simulation.mesh.vertex(cell).z.abs() > 0.5)
                .collect();
            simulation.split_plate_cells(0, &caps).unwrap();
            simulation.polarities.clear();
            simulation.mantle.insert_plume(Plume {
                position: plume,
                age: 50.0,
                lifetime: 200.0,
                peak_flux: 5e-3,
                has_rifted: false,
            });
            simulation.rift_above_plumes();
            assert_eq!(simulation.event_counts.rifts, 1, "lean {lean}, seed {seed}");
            assert_eq!(
                simulation.event_counts.head_rifts, 1,
                "lean {lean}, seed {seed}"
            );
            // Directions from the plume to the new boundary 0.25…0.45 rad away, north and
            // south of it.
            let mut directions = [DVec3::ZERO; 2];
            for cell in 0..simulation.mesh.len() as u32 {
                let r = simulation.mesh.vertex(cell);
                let plate = simulation.owners[cell as usize].plate;
                let distance = r.angle_between(plume);
                if plate == 1
                    || !(0.25..=0.45).contains(&distance)
                    || simulation
                        .mesh
                        .neighbors(cell)
                        .iter()
                        .all(|&other| simulation.owners[other as usize].plate == plate)
                {
                    continue;
                }
                let tangent = (r - plume * r.dot(plume)).normalize();
                directions[usize::from(r.z > 0.0)] += tangent;
            }
            let angle = directions[0].angle_between(directions[1]).to_degrees();
            let expected = 180.0 - 2.0 * lean;
            assert!(
                (angle - expected).abs() < 20.0,
                "lean {lean}, seed {seed}: angle {angle:.0}°, expected {expected:.0}°"
            );
        }
    }

    #[test]
    fn detached_pieces_become_plates_or_are_captured() {
        let mut params = TectonicParams::default();
        params.events.min_breakup_area = 0.05;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 2, params, 4);
        let cells = simulation.mesh.len() as u32;
        // Give plate 0 two detached pieces inside plate 1: a large cap around +z and a tiny one
        // around -z (moving the samples shown there; plate 0's own area stays its largest piece).
        let cap = |axis: DVec3, angle: f64| -> Vec<u32> {
            (0..cells)
                .filter(|&cell| simulation.mesh.vertex(cell).angle_between(axis) < angle)
                .collect()
        };
        let large = cap(DVec3::Z, 0.5);
        let tiny = cap(-DVec3::Z, 0.1);
        let main_plate = simulation.owners[simulation.mesh.nearest(DVec3::X) as usize].plate;
        let other = 1 - main_plate;
        // Plate 1 keeps only a cap around -x, far from both pieces (the random initial
        // partition may otherwise reach the poles).
        for cell in 0..cells {
            let plate = if simulation.mesh.vertex(cell).x < -0.8 {
                other
            } else {
                main_plate
            };
            let owner = simulation.owners[cell as usize];
            let crust = simulation.plates[owner.plate as usize].crust[cell as usize].take();
            simulation.plates[plate as usize].crust[cell as usize] = crust;
            simulation.owners[cell as usize] = Ownership { plate, local: cell };
        }
        for &cell in large.iter().chain(&tiny) {
            simulation.owners[cell as usize] = Ownership {
                plate: other,
                local: cell,
            };
            simulation.plates[other as usize].crust[cell as usize] = Some(new_oceanic_crust(-50.0));
            simulation.plates[main_plate as usize].crust[cell as usize] = None;
        }
        // Ensure the pieces are detached from the other plate's main part.
        let pieces = simulation.plate_pieces(other);
        assert!(pieces.len() >= 3, "{} pieces", pieces.len());
        let plates_before = simulation.plates.len();
        simulation.separate_detached_pieces();
        for plate in 0..simulation.plates.len() as u32 {
            assert!(
                simulation.plate_pieces(plate).len() <= 1,
                "plate {plate} is not connected"
            );
        }
        assert!(simulation.event_counts.breakups >= 1);
        assert!(simulation.event_counts.piece_captures >= 1);
        assert!(simulation.plates.len() > plates_before);
    }

    /// Two plates split at the equator: a large continent on plate 0 (x > 0.5, north), a small
    /// microcontinent on plate 1 just south of it, ocean elsewhere; plate 0 overrides plate 1
    /// along the whole boundary with a 200 km slab; plate 1 moves north (towards plate 0 at the
    /// microcontinent) with `speed` rad/Myr (negative: away). Sutures are blocked unless
    /// `suture_trench_ratio` is 0.
    fn microcontinent_collision(
        terrane_max_area: f64,
        speed: f64,
        suture_trench_ratio: f64,
    ) -> (Simulation, Vec<u32>) {
        let mut params = TectonicParams::default();
        params.events.suture_trench_ratio = suture_trench_ratio;
        params.events.terrane_accretion_time = 3.0;
        params.events.terrane_max_area = terrane_max_area;
        let mut simulation = Simulation::new(SphereMesh::icosphere(5), 1, params, 4);
        let continental = Crust {
            thickness: 35_000.0,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            cohort: 0.0,
        };
        let micro = |r: DVec3| r.x > 0.8 && r.z < 0.0 && r.z > -0.12;
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let r = simulation.mesh.vertex(local as u32);
            *crust = Some(if (r.x > 0.5 && r.z >= 0.0) || micro(r) {
                continental
            } else {
                new_oceanic_crust(-100.0)
            });
        }
        let south: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.mesh.vertex(cell).z < 0.0)
            .collect();
        simulation.split_plate_cells(0, &south).unwrap();
        simulation.polarities.clear();
        simulation.seed_subduction((0, 1), 0, 200e3);
        simulation.plates[1].angular_velocity = -DVec3::Y * speed;
        let terrane: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&cell| micro(simulation.mesh.vertex(cell)))
            .collect();
        (simulation, terrane)
    }

    #[test]
    fn colliding_microcontinents_accrete_to_the_larger_continent() {
        let (mut simulation, terrane) = microcontinent_collision(0.05, 0.004, 1e9);
        assert!(
            terrane
                .iter()
                .all(|&cell| simulation.owners[cell as usize].plate == 1)
        );
        let mass = simulation.total_mass();
        let changes = simulation.configuration_changes;
        for _ in 0..5 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.event_counts.sutures, 0);
        assert_eq!(simulation.event_counts.terrane_accretions, 1);
        assert_eq!(simulation.plates.len(), 2);
        assert!(
            terrane
                .iter()
                .all(|&cell| simulation.owners[cell as usize].plate == 0)
        );
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        assert!(simulation.configuration_changes > changes);
        // The slab broke off near the terrane but not far from it.
        let rotations = simulation.rotations();
        let near = simulation.mesh.vertex(terrane[0]);
        let far = -near;
        assert!(simulation.event_counts.slab_breakoffs > 0);
        let segments = simulation.polarities.pair_segments(0, 1);
        for segment in segments {
            let r = rotations[segment.overriding as usize] * segment.anchor;
            if r.dot(far) > 0.9 {
                assert!(segment.slab_length > 0.0);
            }
        }
        assert!(segments.iter().any(|segment| segment.slab_length > 0.0));
    }

    #[test]
    fn diverging_terranes_do_not_accrete() {
        let (mut simulation, terrane) = microcontinent_collision(0.05, -0.004, 1e9);
        for _ in 0..20 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.event_counts.terrane_accretions, 0);
        assert!(
            terrane
                .iter()
                .all(|&cell| simulation.owners[cell as usize].plate == 1)
        );
    }

    #[test]
    fn suturing_pairs_do_not_accrete_terranes() {
        // With the trench-ratio rule off, a stalled collision (0.3 cm/yr, below
        // `suture_convergence`) sutures after `suture_time` (default 30 Myr) instead of
        // accreting after the terrane accretion time.
        let (mut simulation, _) = microcontinent_collision(0.05, 0.0005, 0.0);
        for _ in 0..40 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.event_counts.terrane_accretions, 0);
        assert_eq!(simulation.event_counts.sutures, 1);
        assert_eq!(simulation.plates.len(), 1);
    }

    #[test]
    fn converging_collisions_do_not_suture_but_accrete_terranes() {
        // The same pair converging at 2.5 cm/yr is no suture in the making: it stays a plate
        // boundary and the microcontinent accretes.
        let (mut simulation, _) = microcontinent_collision(0.05, 0.004, 0.0);
        for _ in 0..40 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.event_counts.sutures, 0);
        assert_eq!(simulation.event_counts.terrane_accretions, 1);
        assert_eq!(simulation.plates.len(), 2);
    }

    #[test]
    fn terranes_above_the_size_limit_do_not_accrete() {
        let (mut simulation, terrane) = microcontinent_collision(0.001, 0.004, 1e9);
        for _ in 0..5 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.event_counts.terrane_accretions, 0);
        assert!(
            terrane
                .iter()
                .all(|&cell| simulation.owners[cell as usize].plate == 1)
        );
    }

    /// Two continental plates (all crust continental) with suture time 3 Myr, plate 1 moving
    /// towards plate 0 with `speed` rad/Myr along the y axis.
    fn continental_collision(speed: f64) -> Simulation {
        let mut params = TectonicParams::default();
        params.events.suture_time = 3.0;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 2, params, 4);
        for plate in &mut simulation.plates {
            for crust in plate.crust.iter_mut().flatten() {
                *crust = Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: NO_OROGENY,
                    cohort: 0.0,
                };
            }
        }
        simulation.seed_subduction((0, 1), 0, 0.0);
        simulation.plates[0].angular_velocity = DVec3::ZERO;
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        // Towards plate 0 along part of their boundary (away along the opposite part).
        let normal_axis = towards_axis(&simulation);
        simulation.plates[1].angular_velocity = normal_axis * speed;
        simulation
    }

    /// A rotation axis (unit) about which plate 1 moves straight towards plate 0 at one of their
    /// boundary cells (and nearly so along that part of the boundary).
    fn towards_axis(simulation: &Simulation) -> DVec3 {
        let boundary = simulation
            .boundary_cells()
            .into_iter()
            .find(|boundary| boundary.plate == 1)
            .expect("the plates share a boundary");
        // (ω × r) · n = ω · (r × n): the motion is along n for ω ∥ r × n.
        simulation
            .mesh
            .vertex(boundary.cell)
            .cross(boundary.normal)
            .normalize()
    }

    #[test]
    fn stalled_collisions_suture_plates() {
        let mut simulation = continental_collision(0.0);
        for _ in 0..5 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.plates.len(), 1);
        assert_eq!(simulation.event_counts.sutures, 1);
    }

    #[test]
    fn converging_collisions_do_not_suture() {
        // 3 cm/yr towards each other.
        let mut simulation = continental_collision(3.0 / 637.1);
        let converging = simulation
            .boundary_cells()
            .iter()
            .filter(|b| b.convergence > 0.5 / 637.1)
            .count();
        assert!(converging > 20, "{converging}");
        for _ in 0..10 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.plates.len(), 2);
        assert_eq!(simulation.event_counts.sutures, 0);
    }

    #[test]
    fn plumes_rift_large_plates() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 1, TectonicParams::default(), 6);
        simulation.mantle.insert_plume(Plume {
            position: DVec3::Z,
            age: 50.0,
            lifetime: 200.0,
            peak_flux: 5e-3,
            has_rifted: false,
        });
        simulation.rift_above_plumes();
        assert_eq!(simulation.plates.len(), 2);
        assert_eq!(simulation.event_counts.rifts, 1);
        assert!(simulation.mantle.plumes()[0].has_rifted);
    }

    #[test]
    fn tiny_plates_are_captured() {
        let mut params = TectonicParams::default();
        params.events.min_plate_area_before_capture = 0.05;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 30, params, 8);
        simulation.polarities.clear();
        let before = simulation.plates.len();
        simulation.capture_tiny_plates();
        assert!(simulation.plates.len() < before);
        assert_eq!(
            before - simulation.plates.len(),
            simulation.event_counts.captures
        );
        let area = simulation.plate_area_fractions();
        assert!(area.iter().all(|&a| a >= 0.05));
        for cell in 0..simulation.mesh.len() as u32 {
            simulation.surface_crust(cell);
        }
    }

    #[test]
    fn tiny_plates_that_subduct_are_not_captured() {
        let mut params = TectonicParams::default();
        params.events.min_plate_area_before_capture = 0.05;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 30, params, 8);
        simulation.polarities.clear();
        let area = simulation.plate_area_fractions();
        let smallest = (0..simulation.plates.len() as u32)
            .min_by(|&a, &b| area[a as usize].total_cmp(&area[b as usize]))
            .unwrap();
        assert!(area[smallest as usize] < 0.05);
        let neighbour = (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.owners[cell as usize].plate == smallest)
            .flat_map(|cell| simulation.mesh.neighbors(cell).to_vec())
            .map(|cell| simulation.owners[cell as usize].plate)
            .find(|&plate| plate != smallest)
            .unwrap();
        simulation.seed_subduction((smallest, neighbour), neighbour, 150e3);
        simulation.capture_tiny_plates();
        // The subducting plate is still there (possibly renumbered), all others are large.
        let area = simulation.plate_area_fractions();
        assert_eq!(area.iter().filter(|&&a| a < 0.05).count(), 1);
    }

    #[test]
    fn plume_rifts_through_old_ocean_do_not_become_trenches() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 1, TectonicParams::default(), 6);
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = new_oceanic_crust(-200.0);
        }
        simulation.mantle.insert_plume(Plume {
            position: DVec3::Z,
            age: 50.0,
            lifetime: 200.0,
            peak_flux: 5e-3,
            has_rifted: false,
        });
        simulation.rift_above_plumes();
        assert_eq!(simulation.plates.len(), 2);
        for _ in 0..2 {
            simulation.step(1.0);
            assert!(
                simulation
                    .boundary_cells()
                    .iter()
                    .filter(|b| b.convergence < 0.0)
                    .all(|b| !matches!(b.kind, BoundaryKind::Trench { .. }))
            );
        }
    }
}
