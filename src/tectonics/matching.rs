//! Matching plates to world cells: which crust sample of which plate lies at each cell.
//!
//! For every cell and every plate, the nearest local vertex of the plate is looked up (a short
//! greedy walk from the previous result). Doing this for all cells and all plates in every
//! substep dominated the run time, although a plate only has crust at a fraction of the cells.
//!
//! **Incremental matching:** each plate remembers the cells where it had crust after the last
//! matching ([`TrackedCrust::cells`]) and its rotation then. A plate that has since rotated by
//! less than one cell spacing can only have crust at those cells or within a ring of
//! [`RINGS`] cells around them, so only these cells are searched. A plate that moved further,
//! or whose cells are unknown (new, split or merged plates), is searched at every cell (full
//! scan). The result is the same as a full scan, which [`MatchingParams::verify`] checks.
//!
//! Crust samples that no cell matched (hidden by the resampling, e.g. overridden crust waiting
//! to be consumed) are tracked too: the cell nearest to them joins the plate's crust cells, so
//! they are found as soon as they surface again. To find them without scanning every local
//! vertex, each plate also keeps the list of its crust samples ([`TrackedCrust::samples`]):
//! crust is only created where a cell is filled in a gap (and in splits and merges, which reset
//! the tracking), so the list is updated from the matched and filled samples.

use glam::DQuat;

use super::*;

/// Width (in cells) of the ring around a plate's previous crust cells that is searched. Valid
/// for plates that moved less than one cell spacing since the last matching: a cell matches a
/// sample within about 0.6 cell spacings of it, and the sample was within 0.6 spacings of a
/// crust cell before it moved by less than one spacing, so the cell is at most ~2.2 spacings
/// from a crust cell; three rings reach at least ~2.6 spacings.
const RINGS: usize = 3;

/// How plates are matched to cells.
#[derive(Clone, Debug)]
pub struct MatchingParams {
    /// Search only around each plate's previous crust cells where possible (identical result).
    pub incremental: bool,
    /// Additionally run the full scan for incrementally matched plates and count crust found
    /// outside the searched cells (should be zero). Slow; for verification.
    pub verify: bool,
}

impl Default for MatchingParams {
    fn default() -> Self {
        Self {
            incremental: true,
            verify: false,
        }
    }
}

/// Counts of how plates were matched to cells, accumulated over the simulation.
#[derive(Clone, Debug, Default)]
pub struct MatchingStatistics {
    /// Plate matchings that searched only around the previous crust cells.
    pub incremental: u64,
    /// Plate matchings that searched every cell.
    pub full_scans: u64,
    /// Cells searched in total.
    pub searched_cells: u64,
    /// Cells with crust of a plate that the incremental search did not visit (verification
    /// only; must be zero).
    pub mismatches: u64,
}

/// What a plate knows about where its crust was after the last matching.
#[derive(Clone, Debug)]
pub(super) struct TrackedCrust {
    /// World cells where the plate had crust (including the cells nearest to hidden samples).
    cells: Vec<u32>,
    /// All local vertices that had crust, each with a cell near it (the cell that matched it, or
    /// its nearest cell if it was hidden) to start the search for its nearest cell from.
    samples: Vec<(u32, u32)>,
}

/// Reusable buffers for the matching.
#[derive(Clone, Debug, Default)]
pub(super) struct MatchingScratch {
    /// Marks cells visited in the current plate's search with `generation`.
    stamp: Vec<u32>,
    generation: u32,
    /// Marks local vertices matched to a cell with `local_generation`.
    local_stamp: Vec<u32>,
    local_generation: u32,
}

/// The crust samples at each cell: for every cell, the (plate, local vertex) pairs of the plates
/// that have crust there, ordered by plate.
pub(super) struct CellMatches {
    offsets: Vec<u32>,
    entries: Vec<(u32, u32)>,
}

impl CellMatches {
    /// The (plate, local vertex) pairs at `cell`, ordered by plate.
    pub(super) fn at(&self, cell: u32) -> &[(u32, u32)] {
        let start = self.offsets[cell as usize] as usize;
        let end = self.offsets[cell as usize + 1] as usize;
        &self.entries[start..end]
    }
}

/// The angle (radians) by which any point moves when going from rotation `from` to `to`.
fn rotation_angle(from: DQuat, to: DQuat) -> f64 {
    let difference = to * from.inverse();
    2.0 * difference.w.abs().min(1.0).acos()
}

/// Finds the crust samples of all plates at all cells. Returns the matches and, per plate, the
/// matched (cell, local vertex) pairs (needed to update [`Plate::tracked`] afterwards).
pub(super) fn match_plates(
    mesh: &SphereMesh,
    plates: &mut [Plate],
    inverse_rotations: &[DQuat],
    alive: &[bool],
    params: &MatchingParams,
    scratch: &mut MatchingScratch,
    statistics: &mut MatchingStatistics,
) -> (CellMatches, Vec<Vec<(u32, u32)>>) {
    let cell_count = mesh.len();
    if scratch.stamp.len() != cell_count {
        scratch.stamp = vec![0; cell_count];
        scratch.generation = 0;
    }
    let max_rotation = mesh.mean_edge_angle();
    // (cell, local vertex) per plate.
    let mut found_per_plate: Vec<Vec<(u32, u32)>> = Vec::with_capacity(plates.len());

    for (index, plate) in plates.iter_mut().enumerate() {
        let mut searched: Vec<u32> = Vec::new();
        let mut found: Vec<(u32, u32)> = Vec::new();
        if !alive[index] {
            found_per_plate.push(found);
            continue;
        }
        let incremental = params.incremental
            && plate.tracked.is_some()
            && rotation_angle(plate.matched_rotation, plate.rotation) < max_rotation;
        if incremental {
            scratch.generation = scratch.generation.wrapping_add(1);
            if scratch.generation == 0 {
                scratch.stamp.fill(0);
                scratch.generation = 1;
            }
            let generation = scratch.generation;
            let mut frontier = Vec::new();
            let cells = plate.tracked.as_ref().map_or(&[][..], |t| &t.cells[..]);
            for &cell in cells {
                if scratch.stamp[cell as usize] != generation {
                    scratch.stamp[cell as usize] = generation;
                    searched.push(cell);
                    frontier.push(cell);
                }
            }
            for _ in 0..RINGS {
                let mut next = Vec::new();
                for &cell in &frontier {
                    for &neighbor in mesh.neighbors(cell) {
                        if scratch.stamp[neighbor as usize] != generation {
                            scratch.stamp[neighbor as usize] = generation;
                            searched.push(neighbor);
                            next.push(neighbor);
                        }
                    }
                }
                frontier = next;
            }
            statistics.incremental += 1;
            if params.verify {
                let inverse = inverse_rotations[index];
                for cell in 0..cell_count as u32 {
                    if scratch.stamp[cell as usize] == generation {
                        continue;
                    }
                    let local = mesh.nearest_from(
                        inverse * mesh.vertex(cell),
                        plate.lookup_cache[cell as usize],
                    );
                    if plate.crust[local as usize].is_some() {
                        statistics.mismatches += 1;
                    }
                }
            }
        } else {
            searched.extend(0..cell_count as u32);
            statistics.full_scans += 1;
        }
        statistics.searched_cells += searched.len() as u64;

        for &cell in &searched {
            let local = plate.local_vertex(mesh, inverse_rotations[index], cell);
            if plate.crust[local as usize].is_some() {
                found.push((cell, local));
            }
        }
        plate.matched_rotation = plate.rotation;
        found_per_plate.push(found);
    }

    // Group by cell with a stable counting sort, so that the plates at each cell stay in plate
    // order (as in a full scan over plates per cell).
    let mut offsets = vec![0u32; cell_count + 1];
    for &(cell, _) in found_per_plate.iter().flatten() {
        offsets[cell as usize + 1] += 1;
    }
    for cell in 0..cell_count {
        offsets[cell + 1] += offsets[cell];
    }
    let mut position = offsets.clone();
    let mut entries = vec![(0, 0); offsets[cell_count] as usize];
    for (plate, found) in found_per_plate.iter().enumerate() {
        for &(cell, local) in found {
            entries[position[cell as usize] as usize] = (plate as u32, local);
            position[cell as usize] += 1;
        }
    }
    (CellMatches { offsets, entries }, found_per_plate)
}

/// Updates each plate's tracked crust after ownership resolution: the matched cells (and cells
/// filled in gaps) where the plate still has crust, the nearest cells of hidden samples, and the
/// list of all samples.
pub(super) fn update_crust_cells(
    mesh: &SphereMesh,
    plates: &mut [Plate],
    found_per_plate: Vec<Vec<(u32, u32)>>,
    filled: &[(u32, u32)],
    scratch: &mut MatchingScratch,
) {
    if scratch.local_stamp.len() != mesh.len() {
        scratch.local_stamp = vec![0; mesh.len()];
        scratch.local_generation = 0;
    }
    let mut filled_per_plate: Vec<Vec<(u32, u32)>> = vec![Vec::new(); plates.len()];
    for &(plate, cell) in filled {
        let local = plates[plate as usize].lookup_cache[cell as usize];
        filled_per_plate[plate as usize].push((cell, local));
    }
    for ((plate, found), filled) in plates.iter_mut().zip(found_per_plate).zip(filled_per_plate) {
        scratch.local_generation = scratch.local_generation.wrapping_add(1);
        if scratch.local_generation == 0 {
            scratch.local_stamp.fill(0);
            scratch.local_generation = 1;
        }
        let generation = scratch.local_generation;
        let mut cells: Vec<u32> = Vec::new();
        let mut samples: Vec<(u32, u32)> = Vec::new();
        // Crust consumed during the resolution is gone; crust only appears in filled cells.
        for (cell, local) in found.into_iter().chain(filled) {
            if plate.crust[local as usize].is_some() {
                if scratch.local_stamp[local as usize] != generation {
                    scratch.local_stamp[local as usize] = generation;
                    samples.push((local, cell));
                }
                cells.push(cell);
            }
        }
        // Crust samples that no cell matched (hidden by the resampling): track the cell nearest
        // to them, so that they are found once a cell matches them. Without a previous sample
        // list (new, split or merged plates), every local vertex is checked.
        let previous = plate.tracked.take().map(|tracked| tracked.samples);
        let all;
        let candidates: &[(u32, u32)] = match &previous {
            Some(previous) => previous,
            None => {
                // No cell near the sample is known: start from the previous one.
                all = (0..plate.crust.len() as u32)
                    .map(|local| (local, u32::MAX))
                    .collect::<Vec<_>>();
                &all
            }
        };
        let mut last = 0;
        for &(local, near) in candidates {
            if plate.crust[local as usize].is_some()
                && scratch.local_stamp[local as usize] != generation
            {
                scratch.local_stamp[local as usize] = generation;
                let start = if near == u32::MAX { last } else { near };
                let cell = mesh.nearest_from(plate.rotation * mesh.vertex(local), start);
                last = cell;
                samples.push((local, cell));
                cells.push(cell);
            }
        }
        cells.sort_unstable();
        cells.dedup();
        plate.tracked = Some(TrackedCrust { cells, samples });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulation(seed: u64, params: TectonicParams) -> Simulation {
        Simulation::new(SphereMesh::icosphere(4), 10, params, seed)
    }

    /// Asserts that two simulations are in exactly the same state.
    fn assert_same_state(a: &Simulation, b: &Simulation, context: &str) {
        assert_eq!(a.plates.len(), b.plates.len(), "{context}: plate count");
        assert_eq!(a.owners, b.owners, "{context}: owners");
        for (index, (pa, pb)) in a.plates.iter().zip(&b.plates).enumerate() {
            assert_eq!(
                pa.rotation, pb.rotation,
                "{context}: rotation of plate {index}"
            );
            assert_eq!(
                pa.angular_velocity, pb.angular_velocity,
                "{context}: velocity of plate {index}"
            );
            for (local, (ca, cb)) in pa.crust.iter().zip(&pb.crust).enumerate() {
                let bits = |c: &Option<Crust>| {
                    c.map(|c| {
                        [
                            c.thickness.to_bits(),
                            c.density.to_bits(),
                            c.birth.to_bits(),
                            c.sediment.to_bits(),
                            c.orogeny.to_bits(),
                        ]
                    })
                };
                assert_eq!(
                    bits(ca),
                    bits(cb),
                    "{context}: crust of plate {index} at local vertex {local}"
                );
            }
        }
        let polarities = |s: &Simulation| s.polarities.snapshot();
        assert_eq!(polarities(a), polarities(b), "{context}: polarities");
        assert_eq!(
            a.total_mass().to_bits(),
            b.total_mass().to_bits(),
            "{context}: total mass"
        );
    }

    /// Runs a simulation with incremental matching (verified against full scans) and one with
    /// full scans only, and checks that they stay identical.
    fn compare_modes(seed: u64, mut params: TectonicParams, steps: usize) -> Simulation {
        params.matching.incremental = true;
        params.matching.verify = true;
        let mut incremental = simulation(seed, params.clone());
        params.matching.incremental = false;
        params.matching.verify = false;
        let mut full = simulation(seed, params);
        for step in 0..steps {
            incremental.step(1.0);
            full.step(1.0);
            assert_same_state(&incremental, &full, &format!("seed {seed}, step {step}"));
            assert_eq!(
                incremental.matching_statistics.mismatches, 0,
                "seed {seed}, step {step}: crust outside the searched cells"
            );
        }
        assert_eq!(full.matching_statistics.incremental, 0);
        incremental
    }

    #[test]
    fn incremental_matching_gives_the_same_result_as_full_scans() {
        for seed in 0..4 {
            let incremental = compare_modes(seed, TectonicParams::default(), 60);
            let statistics = &incremental.matching_statistics;
            // Most matchings were incremental.
            assert!(
                statistics.incremental > 4 * statistics.full_scans,
                "{statistics:?}"
            );
        }
    }

    #[test]
    fn plates_moving_more_than_a_cell_fall_back_to_full_scans() {
        // Up to three cells per substep, and fast plates.
        let defaults = TectonicParams::default();
        let params = TectonicParams {
            max_substep_movement: 3.0,
            basal_drag: defaults.basal_drag / 4.0,
            slab_sinking_speed: 1000.0,
            ..defaults
        };
        for seed in 0..3 {
            let incremental = compare_modes(seed, params.clone(), 40);
            // Some matchings had to fall back to full scans after the first one per plate.
            let statistics = &incremental.matching_statistics;
            assert!(statistics.full_scans > 20, "{statistics:?}");
        }
    }

    #[test]
    fn splits_and_merges_keep_both_modes_identical() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1.0;
        params.events.boundary_initiation_waiting_time = 1.0;
        params.events.initiation_age = 30.0;
        params.events.initiation_full_age = 40.0;
        params.events.rift_delay = 1.0;
        params.events.suture_time = 5.0;
        params.events.min_plate_area_before_capture = 0.02;
        params.mantle.plume_interval = 2.0;
        let mut births_and_deaths = 0;
        for seed in 0..3 {
            let incremental = compare_modes(seed, params.clone(), 80);
            births_and_deaths +=
                incremental.event_counts.births() + incremental.event_counts.deaths();
        }
        assert!(births_and_deaths > 5, "{births_and_deaths} plate events");
    }

    #[test]
    fn incremental_matching_searches_few_cells() {
        let mut params = TectonicParams::default();
        params.matching.incremental = true;
        let mut simulation = simulation(1, params);
        for _ in 0..30 {
            simulation.step(1.0);
        }
        let statistics = &simulation.matching_statistics;
        let matchings = (statistics.incremental + statistics.full_scans) as f64;
        let per_plate = statistics.searched_cells as f64 / matchings / simulation.mesh.len() as f64;
        // Each plate searches only a fraction of the cells on average (full scans included).
        assert!(per_plate < 0.4, "{per_plate}");
    }
}
