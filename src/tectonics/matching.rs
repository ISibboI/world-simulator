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

use crate::math;
use glam::DQuat;

use super::*;

/// Width (in cells) of the ring around a plate's previous crust cells that is searched. Valid
/// for plates that moved less than one cell spacing since the last matching: a cell matches a
/// sample within about 0.6 cell spacings of it, and the sample was within 0.6 spacings of a
/// crust cell before it moved by less than one spacing, so the cell is at most ~2.2 spacings
/// from a crust cell; three rings reach at least ~2.6 spacings.
const RINGS: usize = 3;

/// Most cells one search of [`show_distinct_samples`] visits: where a plate covers more cells
/// than it has samples nearby, no chain exists, and the search would grow over the whole region.
const MAX_SEARCHED_CELLS: usize = 32;

/// How plates are matched to cells.
#[derive(Clone, Debug)]
pub struct MatchingParams {
    /// Search only around each plate's previous crust cells where possible (identical result).
    pub incremental: bool,
    /// Additionally run the full scan for incrementally matched plates and count crust found
    /// outside the searched cells (should be zero). Slow; for verification.
    pub verify: bool,
    /// Show every crust sample at one cell at most (see [`show_distinct_samples`]).
    pub distinct: bool,
    /// How far (in mean cell spacings) from a cell a sample it shows instead of its nearest one
    /// may lie.
    pub distinct_radius: f64,
    /// Longest chain of cells that pass their samples on to give a cell its own sample.
    pub distinct_depth: usize,
    /// Largest difference (m) between the dry elevation of the sample a cell shows instead of
    /// its nearest one and that of the nearest one.
    pub distinct_tolerance: f64,
}

impl Default for MatchingParams {
    fn default() -> Self {
        Self {
            incremental: true,
            verify: false,
            distinct: true,
            distinct_radius: 1.6,
            distinct_depth: 6,
            distinct_tolerance: 30.0,
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
    /// Cells that showed a sample also shown at a cell nearer to it (see
    /// [`show_distinct_samples`]), those given a sample of their own, and the chain steps
    /// (cells passing their sample on) that took.
    pub shared: u64,
    pub separated: u64,
    pub chain_steps: u64,
    /// Wall time spent in [`show_distinct_samples`] (s).
    pub distinct_seconds: f64,
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
    /// For [`show_distinct_samples`]: per local vertex of the current plate, the cell showing it
    /// (valid where `local_stamp` is the current generation), and per cell the step it was
    /// reached by in the search.
    holder: Vec<u32>,
    reached: Vec<(u32, u32)>,
    /// Per cell, the dry elevation of the sample it showed before the pass (valid where
    /// `cell_height_stamp` is `pass`; filled when the search first reaches the cell, before
    /// any chain changes its sample).
    height: Vec<f64>,
    cell_height_stamp: Vec<u32>,
    pass: u32,
    /// Per local vertex of the current plate, its dry elevation (valid where `height_stamp` is
    /// the current generation).
    sample_height: Vec<f64>,
    height_stamp: Vec<u32>,
    /// Per local vertex, the search step that last considered it (`visit`).
    seen: Vec<u32>,
    visit: u32,
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
    2.0 * math::acos(difference.w.abs().min(1.0))
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

/// Adds a crust sample created outside the ownership resolution (a plate's edge advancing,
/// see `sheet`) to the plate's tracked crust, with `near` a cell near it.
pub(super) fn track_sample(plate: &mut Plate, local: u32, near: u32) {
    if let Some(tracked) = plate.tracked.as_mut() {
        tracked.samples.push((local, near));
        if let Err(position) = tracked.cells.binary_search(&near) {
            tracked.cells.insert(position, near);
        }
    }
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

/// Gives every cell a crust sample of its own where possible (#112): nearest-vertex resampling
/// of a rotated plate shows some samples at two or more cells, because the plate's local mesh
/// and the world mesh differ in density by up to ±20% (icosphere cells range from 0.77× to 1.21×
/// the mean area) and are offset from each other; elsewhere the same mismatch leaves samples
/// hidden. Cells showing one sample would share one crust column and every change to it, so
/// the lower of two such neighbours (lower by the dynamic topography only) could never fill and
/// stayed a pit.
///
/// Of the cells showing one sample, the one nearest to it keeps it. Every other cell gets a
/// sample no cell shows, within [`MatchingParams::distinct_radius`] cell spacings, through a
/// chain of cells within that distance that pass their samples on (an augmenting path, found by
/// breadth-first search with at most [`MatchingParams::distinct_depth`] steps; the shortest
/// chain wins, nearer samples first). Every cell on the chain must show a sample whose dry
/// elevation differs by at most [`MatchingParams::distinct_tolerance`] from that of its
/// nearest sample: on slopes a sample from further away would make pits and peaks of up to the
/// slope times the distance, while same-sample twins make pits only where the relief is smaller
/// than the dynamic topography between them (a few tens of metres; #57). A cell for which no
/// chain exists (where the plate
/// covers more cells than it has samples nearby) keeps showing the shared sample. Only which
/// sample of its plate a cell shows changes, never which plate owns it; a sample shown in place
/// of the nearest one lies within a cell or two of the cell.
pub(super) fn show_distinct_samples(
    mesh: &SphereMesh,
    plates: &mut [Plate],
    owners: &mut [Ownership],
    time: f64,
    params: &MatchingParams,
    scratch: &mut MatchingScratch,
    statistics: &mut MatchingStatistics,
) {
    let start = web_time::Instant::now();
    let cell_count = mesh.len();
    scratch.height.resize(cell_count, 0.0);
    scratch.cell_height_stamp.resize(cell_count, 0);
    scratch.pass = scratch.pass.wrapping_add(1);
    if scratch.pass == 0 {
        scratch.cell_height_stamp.fill(0);
        scratch.pass = 1;
    }
    let pass = scratch.pass;
    if scratch.local_stamp.len() != cell_count {
        scratch.local_stamp = vec![0; cell_count];
        scratch.local_generation = 0;
    }
    if scratch.stamp.len() != cell_count {
        scratch.stamp = vec![0; cell_count];
        scratch.generation = 0;
    }
    scratch.holder.resize(cell_count, u32::MAX);
    scratch.sample_height.resize(cell_count, 0.0);
    scratch.height_stamp.resize(cell_count, 0);
    scratch.seen.resize(cell_count, 0);
    scratch.reached.resize(cell_count, (u32::MAX, u32::MAX));
    let min_dot = math::cos(params.distinct_radius * mesh.mean_edge_angle());

    // Cells per plate, in cell order.
    let mut offsets = vec![0u32; plates.len() + 1];
    for owner in owners.iter() {
        offsets[owner.plate as usize + 1] += 1;
    }
    for plate in 0..plates.len() {
        offsets[plate + 1] += offsets[plate];
    }
    let mut position = offsets.clone();
    let mut cells_by_plate = vec![0u32; cell_count];
    for (cell, owner) in owners.iter().enumerate() {
        cells_by_plate[position[owner.plate as usize] as usize] = cell as u32;
        position[owner.plate as usize] += 1;
    }

    let mut orphans: Vec<u32> = Vec::new();
    let mut queue: Vec<u32> = Vec::new();
    let mut candidates: Vec<(f64, u32)> = Vec::new();
    for (index, plate) in plates.iter_mut().enumerate() {
        let cells = &cells_by_plate[offsets[index] as usize..offsets[index + 1] as usize];
        if cells.is_empty() {
            continue;
        }
        scratch.local_generation = scratch.local_generation.wrapping_add(1);
        if scratch.local_generation == 0 {
            scratch.local_stamp.fill(0);
            scratch.height_stamp.fill(0);
            scratch.local_generation = 1;
        }
        let generation = scratch.local_generation;
        let rotation = plate.rotation;
        let inverse = rotation.inverse();
        // Closeness of a cell to a sample of this plate (cosine of the angle).
        let closeness =
            |cell: u32, local: u32| -> f64 { mesh.vertex(cell).dot(rotation * mesh.vertex(local)) };
        orphans.clear();
        for &cell in cells {
            let local = owners[cell as usize].local;
            let l = local as usize;
            if scratch.local_stamp[l] != generation {
                scratch.local_stamp[l] = generation;
                scratch.holder[l] = cell;
                continue;
            }
            let holder = scratch.holder[l];
            // The nearer cell keeps the sample (ties: the lower index, which came first).
            if closeness(cell, local) > closeness(holder, local) {
                scratch.holder[l] = cell;
                orphans.push(holder);
            } else {
                orphans.push(cell);
            }
        }
        if orphans.is_empty() {
            continue;
        }
        orphans.sort_unstable();
        statistics.shared += orphans.len() as u64;
        for &orphan in &orphans {
            // Breadth-first search for a chain ending at a sample no cell shows.
            scratch.generation = scratch.generation.wrapping_add(1);
            if scratch.generation == 0 {
                scratch.stamp.fill(0);
                scratch.generation = 1;
            }
            let search = scratch.generation;
            scratch.stamp[orphan as usize] = search;
            scratch.reached[orphan as usize] = (u32::MAX, u32::MAX);
            queue.clear();
            queue.push(orphan);
            let mut head = 0;
            let mut level_end = 1;
            let mut depth = 0;
            let mut found: Option<(u32, u32)> = None;
            'search: while head < queue.len() {
                if head == level_end {
                    depth += 1;
                    level_end = queue.len();
                    if depth >= params.distinct_depth {
                        break;
                    }
                }
                if queue.len() > MAX_SEARCHED_CELLS {
                    break;
                }
                let cell = queue[head];
                head += 1;
                let target = inverse * mesh.vertex(cell);
                let nearest = mesh.nearest_from(target, plate.lookup_cache[cell as usize]);
                candidates.clear();
                let c = cell as usize;
                if scratch.cell_height_stamp[c] != pass {
                    scratch.cell_height_stamp[c] = pass;
                    scratch.height[c] = plate.crust[owners[c].local as usize]
                        .expect("owners point to crust")
                        .dry_elevation(time);
                }
                let height = scratch.height[c];
                scratch.visit = scratch.visit.wrapping_add(1);
                if scratch.visit == 0 {
                    scratch.seen.fill(0);
                    scratch.visit = 1;
                }
                let visit = scratch.visit;
                let (sample_height, height_stamp, seen) = (
                    &mut scratch.sample_height,
                    &mut scratch.height_stamp,
                    &mut scratch.seen,
                );
                let mut consider = |local: u32, candidates: &mut Vec<(f64, u32)>| {
                    let l = local as usize;
                    if seen[l] == visit {
                        return;
                    }
                    seen[l] = visit;
                    let Some(crust) = &plate.crust[l] else {
                        return;
                    };
                    let dot = mesh.vertex(local).dot(target);
                    if dot < min_dot {
                        return;
                    }
                    if height_stamp[l] != generation {
                        height_stamp[l] = generation;
                        sample_height[l] = crust.dry_elevation(time);
                    }
                    if (sample_height[l] - height).abs() <= params.distinct_tolerance {
                        candidates.push((dot, local));
                    }
                };
                consider(nearest, &mut candidates);
                for &first in mesh.neighbors(nearest) {
                    consider(first, &mut candidates);
                    for &second in mesh.neighbors(first) {
                        consider(second, &mut candidates);
                    }
                }
                // Nearest first; ties by index (deterministic).
                candidates.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                for &(_, local) in &candidates {
                    let l = local as usize;
                    if scratch.local_stamp[l] != generation {
                        found = Some((cell, local));
                        break 'search;
                    }
                    let holder = scratch.holder[l];
                    if holder == cell || scratch.stamp[holder as usize] == search {
                        continue;
                    }
                    scratch.stamp[holder as usize] = search;
                    scratch.reached[holder as usize] = (cell, local);
                    queue.push(holder);
                }
            }
            let Some((mut cell, mut local)) = found else {
                continue;
            };
            // Pass the samples along the chain: each cell takes the sample of the next one.
            statistics.separated += 1;
            loop {
                scratch.local_stamp[local as usize] = generation;
                scratch.holder[local as usize] = cell;
                owners[cell as usize].local = local;
                let (previous, sample) = scratch.reached[cell as usize];
                if previous == u32::MAX {
                    break;
                }
                statistics.chain_steps += 1;
                (cell, local) = (previous, sample);
            }
        }
    }
    statistics.distinct_seconds += start.elapsed().as_secs_f64();
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
                            c.orogenic.to_bits(),
                            c.reheated.to_bits(),
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

    /// Cells showing a sample that another cell also shows.
    fn shared_cells(simulation: &Simulation, owners: &[Ownership]) -> usize {
        let cell_count = simulation.mesh.len();
        let mut shown = vec![0u32; simulation.plates.len() * cell_count];
        for &owner in owners {
            shown[sample_index(owner, cell_count)] += 1;
        }
        owners
            .iter()
            .filter(|&&owner| shown[sample_index(owner, cell_count)] > 1)
            .count()
    }

    #[test]
    fn distinct_samples_remove_most_shared_cells_and_keep_the_plates() {
        for seed in 0..3 {
            let mut params = TectonicParams::default();
            params.matching.distinct = false;
            let mut simulation = simulation(seed, params);
            for _ in 0..30 {
                simulation.step(1.0);
            }
            let before = simulation.owners.clone();
            let shared_before = shared_cells(&simulation, &before);
            let mut owners = before.clone();
            // Without the elevation tolerance (young sea floor has slopes everywhere).
            let params = MatchingParams {
                distinct_tolerance: f64::INFINITY,
                ..MatchingParams::default()
            };
            let mut statistics = MatchingStatistics::default();
            let Simulation {
                mesh,
                plates,
                matching_scratch,
                time,
                ..
            } = &mut simulation;
            let time = *time;
            show_distinct_samples(
                mesh,
                plates,
                &mut owners,
                time,
                &params,
                matching_scratch,
                &mut statistics,
            );
            let shared_after = shared_cells(&simulation, &owners);
            assert!(shared_before > 0);
            assert!(
                2 * shared_after < shared_before,
                "seed {seed}: {shared_before} → {shared_after} {statistics:?}"
            );
            let max_angle = params.distinct_radius * simulation.mesh.mean_edge_angle();
            for (cell, (old, new)) in before.iter().zip(&owners).enumerate() {
                // Same plate, a sample with crust, near the cell.
                assert_eq!(old.plate, new.plate);
                let plate = &simulation.plates[new.plate as usize];
                assert!(plate.crust[new.local as usize].is_some());
                if old != new {
                    let position = plate.rotation * simulation.mesh.vertex(new.local);
                    let angle =
                        math::acos(position.dot(simulation.mesh.vertex(cell as u32)).min(1.0));
                    assert!(angle <= max_angle * (1.0 + 1e-9), "{angle} > {max_angle}");
                }
            }

            // With the elevation tolerance, every cell shows a sample as high as its nearest one
            // within the tolerance.
            let mut owners = before.clone();
            let params = MatchingParams::default();
            show_distinct_samples(
                &simulation.mesh,
                &mut simulation.plates,
                &mut owners,
                time,
                &params,
                &mut simulation.matching_scratch,
                &mut statistics,
            );
            assert!(shared_cells(&simulation, &owners) < shared_before);
            let height = |owner: Ownership| {
                simulation.plates[owner.plate as usize].crust[owner.local as usize]
                    .unwrap()
                    .dry_elevation(time)
            };
            for (old, new) in before.iter().zip(&owners) {
                let difference = (height(*new) - height(*old)).abs();
                assert!(difference <= params.distinct_tolerance, "{difference}");
            }
        }
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
