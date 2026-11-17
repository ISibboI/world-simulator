//! Plate boundaries: assigning cells to plates, subduction, arc growth, continental
//! collision and crustal flow.

use std::collections::VecDeque;

use glam::{DQuat, DVec3};

use crate::isostasy::{ARC_DENSITY, SEDIMENT_ROCK_DENSITY};

use super::ledger::Term;
use super::matching;
use super::*;

/// Rings beyond the arc or collision profile over which crust that exceeds the maximum
/// thickness spills.
const SPILL_RINGS: usize = 8;

impl Simulation {
    /// Thick crust spreads under its own weight towards thinner neighbouring crust of the same
    /// plate, conserving its volume (lower crustal flow, gravitational collapse of orogens).
    pub(super) fn crustal_flow(&mut self, dt: f64) {
        let change = self.crustal_flow_inner(dt);
        self.ledger.record(Term::CrustalFlow, change);
    }

    /// Returns the change of mass.
    fn crustal_flow_inner(&mut self, dt: f64) -> f64 {
        let params = &self.params;
        let rate = (params.crustal_flow_rate * dt).min(0.15);
        // Per crust sample: (outgoing thickness, incoming thickness, incoming mass per area),
        // with the position of each sample's entry in a dense per-sample index.
        let cell_count = self.mesh.len();
        let mut delta: Vec<(Ownership, (f64, f64, f64))> = Vec::new();
        let mut index = vec![u32::MAX; self.plates.len() * cell_count];
        let entry = |delta: &mut Vec<_>, index: &mut Vec<u32>, owner: Ownership| {
            let slot = &mut index[sample_index(owner, cell_count)];
            if *slot == u32::MAX {
                *slot = delta.len() as u32;
                delta.push((owner, (0.0, 0.0, 0.0)));
            }
            *slot as usize
        };
        for cell in 0..self.mesh.len() as u32 {
            let owner = self.owners[cell as usize];
            let crust = self.surface_crust(cell);
            for &neighbor in self.mesh.neighbors(cell) {
                // Each pair once, and only within a plate.
                let neighbor_owner = self.owners[neighbor as usize];
                if neighbor < cell || neighbor_owner.plate != owner.plate || neighbor_owner == owner
                {
                    continue;
                }
                let other = self.surface_crust(neighbor);
                let thickest = crust.thickness.max(other.thickness);
                let strength = ((thickest - params.crustal_flow_threshold)
                    / params.crustal_flow_range)
                    .clamp(0.0, 1.0);
                if strength <= 0.0 {
                    continue;
                }
                let flow = rate * strength * (crust.thickness - other.thickness) / 2.0;
                let (from, to, from_crust) = if flow > 0.0 {
                    (owner, neighbor_owner, crust)
                } else {
                    (neighbor_owner, owner, other)
                };
                let amount = flow.abs();
                let from = entry(&mut delta, &mut index, from);
                let to = entry(&mut delta, &mut index, to);
                delta[from].1.0 += amount;
                let receiver = &mut delta[to].1;
                receiver.1 += amount;
                receiver.2 += amount * from_crust.density;
            }
        }
        let mut change = 0.0;
        for (owner, (outgoing, incoming, incoming_mass)) in delta {
            let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                .as_mut()
                .expect("owners always point to existing crust");
            let before = crust.mass();
            // Outgoing crust leaves the density unchanged; incoming crust is mixed in by mass.
            crust.thickness = (crust.thickness - outgoing).max(0.0);
            if incoming > 0.0 {
                crust.add_crust(
                    incoming,
                    incoming_mass / incoming,
                    params.max_crust_thickness,
                );
            }
            change += crust.mass() - before;
        }
        change
    }

    /// Assigns every world cell to a plate, subducting crust at convergent boundaries and creating
    /// crust at divergent boundaries.
    pub(super) fn resolve_ownership(&mut self) -> ResolvedBoundaries {
        let Self {
            mesh,
            params,
            plates,
            owners,
            polarities,
            time,
            gap_statistics,
            ledger,
            matching_scratch,
            matching_statistics,
            ..
        } = self;
        let time = *time;
        let cell_count = mesh.len();
        let rotations: Vec<DQuat> = plates.iter().map(|p| p.rotation).collect();
        let inverse_rotations: Vec<DQuat> = rotations.iter().map(|r| r.inverse()).collect();
        let alive: Vec<bool> = plates
            .iter()
            .map(|p| p.crust.iter().any(Option::is_some))
            .collect();

        let mut new_owners: Vec<Option<Ownership>> = vec![None; cell_count];
        // Contacts where a sample of one plate is overridden by another plate.
        let mut lost_contacts: Vec<LostContact> = Vec::new();
        // Overriding crust at trenches, eroded from below by the subducting slab.
        let mut eroded = Vec::new();
        let mut additions = Vec::new();
        let mut collisions: HashMap<(u32, u32), u32> = HashMap::default();
        let mut candidates: Vec<(Ownership, Crust)> = Vec::with_capacity(plates.len());

        let (matches, found) = matching::match_plates(
            mesh,
            plates,
            &inverse_rotations,
            &alive,
            &params.matching,
            matching_scratch,
            matching_statistics,
        );
        // Cells filled in gaps, per plate, for updating the plates' crust cells.
        let mut filled: Vec<(u32, u32)> = Vec::new();

        for cell in 0..cell_count as u32 {
            candidates.clear();
            for &(plate, local) in matches.at(cell) {
                let crust = plates[plate as usize].crust[local as usize]
                    .expect("matched samples have crust");
                candidates.push((Ownership { plate, local }, crust));
            }

            let Some((winner, _)) =
                choose_overriding(&candidates, polarities, mesh.vertex(cell), &rotations, time)
            else {
                continue;
            };
            new_owners[cell as usize] = Some(winner);

            for &(loser, loser_crust) in &candidates {
                if loser != winner {
                    lost_contacts.push(LostContact {
                        cell,
                        winner,
                        loser,
                        loser_crust,
                    });
                }
            }
        }

        // A sample that is overridden at one cell but still forms the surface of another cell is
        // kept until it is covered everywhere: deleting it now would leave a hole behind the
        // trench. Each consumed sample is accounted for exactly once.
        // Per crust sample: 1 if it forms the surface of a cell, 2 once consumed.
        let mut state = vec![0u8; plates.len() * cell_count];
        for &owner in new_owners.iter().flatten() {
            state[sample_index(owner, cell_count)] = 1;
        }
        for contact in &lost_contacts {
            let state = &mut state[sample_index(contact.loser, cell_count)];
            if *state != 0 {
                continue;
            }
            *state = 2;
            let loser_plate = &mut plates[contact.loser.plate as usize];
            loser_plate.crust[contact.loser.local as usize] = None;
            loser_plate.consumed[contact.loser.local as usize] = true;
            let (winner, loser_crust) = (contact.winner, contact.loser_crust);
            let sediment_mass = loser_crust.sediment * SEDIMENT_ROCK_DENSITY;
            if loser_crust.resists_subduction() {
                // Collision or terrane accretion: buoyant crust of the lower plate (continents,
                // thick arcs, plateaus) is stacked onto the overriding plate instead of
                // subducting; its sediment becomes metasediment at the same mass.
                *collisions
                    .entry(pair_key(winner.plate, contact.loser.plate))
                    .or_default() += 1;
                ledger.record(Term::CollisionConsumed, -loser_crust.mass());
                let thickness = loser_crust.thickness + sediment_mass / METASEDIMENT_DENSITY;
                additions.push(CrustAddition {
                    cell: contact.cell,
                    plate: winner.plate,
                    thickness,
                    density: loser_crust.mass() / thickness.max(1e-9),
                    kind: AdditionKind::Collision,
                });
            } else {
                // Subduction: the lower plate's crust and most of its sediment sink into the
                // mantle; melting above the slab builds an arc on the overriding plate.
                let crust_mass = loser_crust.thickness * loser_crust.density;
                let term = if loser_crust.is_oceanic() {
                    Term::SubductedOceanic
                } else {
                    Term::SubductedTransitional
                };
                ledger.record(term, -crust_mass);
                ledger.record(Term::SubductedSediment, -sediment_mass);
                additions.push(CrustAddition {
                    cell: contact.cell,
                    plate: winner.plate,
                    thickness: params.arc_crust_per_subduction,
                    density: ARC_DENSITY,
                    kind: AdditionKind::Arc,
                });
                // Sediment beyond what the subduction channel carries down is scraped off at the
                // trench into an accretionary wedge on the overriding plate, which becomes crust
                // (metasediment) at the same mass.
                let accreted_mass = (loser_crust.sediment - params.subduction_channel_thickness)
                    .max(0.0)
                    * SEDIMENT_ROCK_DENSITY;
                if accreted_mass > 0.0 {
                    // Booked as accretion when added; all sediment left with the slab above.
                    additions.push(CrustAddition {
                        cell: contact.cell,
                        plate: winner.plate,
                        thickness: accreted_mass / METASEDIMENT_DENSITY,
                        density: METASEDIMENT_DENSITY,
                        kind: AdditionKind::Accretion,
                    });
                }
                eroded.push(winner);
            }
        }

        // Subduction erosion: the slab scrapes material off the underside and front of the
        // overriding plate and carries it into the mantle, sediment first. On Earth this and
        // sediment subduction recycle about as much crust as arcs produce.
        for ownership in eroded {
            if let Some(crust) =
                &mut plates[ownership.plate as usize].crust[ownership.local as usize]
            {
                let before = crust.mass();
                let mut amount = params.subduction_erosion;
                let from_sediment = amount.min(crust.sediment);
                crust.sediment -= from_sediment;
                amount -= from_sediment;
                crust.thickness =
                    (crust.thickness - amount).max(crust.thickness.min(OCEANIC_THICKNESS));
                ledger.record(Term::SubductionErosion, crust.mass() - before);
            }
        }

        // Visible samples are never consumed above, so no cell should lose its crust here; count
        // it if it happens anyway.
        for owner in &mut new_owners {
            if let Some(ownership) = owner
                && plates[ownership.plate as usize].crust[ownership.local as usize].is_none()
            {
                *owner = None;
                gap_statistics.freed_cells += 1;
            }
        }

        // Single-cell holes inside a plate are resampling artifacts rather than rifts, so they are
        // healed without new crust (see `heal_gap`).
        let holes: Vec<(u32, Ownership)> = (0..cell_count as u32)
            .filter(|&cell| new_owners[cell as usize].is_none())
            .filter_map(|cell| {
                let mut neighbors = mesh.neighbors(cell).iter().map(|&n| new_owners[n as usize]);
                let first = neighbors.next()??;
                neighbors
                    .all(|neighbor| neighbor.is_some_and(|n| n.plate == first.plate))
                    .then_some((cell, first))
            })
            .collect();
        gap_statistics.hole_cells += holes.len();
        // Holes show the neighbouring sample where the plate's crust there was consumed, else
        // they are healed without creating crust (see `heal_gap`).
        for (cell, template) in holes {
            filled.push((template.plate, cell));
            let plate_index = template.plate as usize;
            let plate = &mut plates[plate_index];
            let local = plate.local_vertex(mesh, inverse_rotations[plate_index], cell);
            new_owners[cell as usize] = Some(if plate.consumed[local as usize] {
                gap_statistics.shared_cells += 1;
                template
            } else {
                // A single-cell hole is surrounded by the plate.
                heal_gap(mesh, plate, template, local, true, gap_statistics)
            });
        }

        // Fill the remaining gaps, growing the adjacent plates layer by layer. Where the plate
        // moves away from a neighbouring plate, the gap is new sea floor (spreading ridge);
        // otherwise it is an artifact of resampling the rigid plates onto the cells (at trenches
        // and transform faults), healed without creating crust (see `heal_gap`).
        let angular_velocities: Vec<DVec3> = plates.iter().map(|p| p.angular_velocity).collect();
        let min_opening = params.min_spreading_rate / (params.planet_radius * 100.0 / 1e6);
        let mut rings = RingScratch::new(cell_count);
        let mut queue: VecDeque<u32> = (0..cell_count as u32)
            .filter(|&cell| new_owners[cell as usize].is_some())
            .collect();
        if queue.is_empty() {
            // All crust was destroyed, which can only happen in degenerate configurations.
            // Restart with the first plate covering the first cell.
            plates[0].crust[0] = Some(new_oceanic_crust(time));
            ledger.record(Term::NewSeaFloor, new_oceanic_crust(time).mass());
            new_owners[0] = Some(Ownership { plate: 0, local: 0 });
            plates[0].tracked = None;
            queue.push_back(0);
        }
        while let Some(cell) = queue.pop_front() {
            let parent = new_owners[cell as usize].expect("queued cells are owned");
            let plate_index = parent.plate;
            for &neighbor in mesh.neighbors(cell) {
                if new_owners[neighbor as usize].is_some() {
                    continue;
                }
                filled.push((plate_index, neighbor));
                let spreading = opens_away(
                    &mut rings,
                    mesh,
                    &new_owners,
                    &angular_velocities,
                    neighbor,
                    plate_index,
                    min_opening,
                );
                let plate = &mut plates[plate_index as usize];
                let local =
                    plate.local_vertex(mesh, inverse_rotations[plate_index as usize], neighbor);
                if spreading {
                    gap_statistics.new_crust_cells += 1;
                    if plate.crust[local as usize].is_none() {
                        ledger.record(Term::NewSeaFloor, new_oceanic_crust(time).mass());
                    }
                    plate.crust[local as usize].get_or_insert_with(|| new_oceanic_crust(time));
                    plate.consumed[local as usize] = false;
                    new_owners[neighbor as usize] = Some(Ownership {
                        plate: plate_index,
                        local,
                    });
                } else if plate.consumed[local as usize] {
                    let at_boundary = mesh.neighbors(neighbor).iter().any(|&n| {
                        new_owners[n as usize].is_some_and(|owner| owner.plate != plate_index)
                    });
                    if at_boundary {
                        // A sliver at a boundary where the plate's crust was consumed: refilling
                        // it would recreate consumed crust (and, at collisions, stack it again).
                        // Show the adjacent sample.
                        gap_statistics.shared_cells += 1;
                        new_owners[neighbor as usize] = Some(parent);
                    } else {
                        // Away from other plates, the uncovered area is open sea floor.
                        gap_statistics.new_crust_cells += 1;
                        let replaced = plate.crust[local as usize].map_or(0.0, |c| c.mass());
                        ledger.record(Term::NewSeaFloor, new_oceanic_crust(time).mass() - replaced);
                        plate.crust[local as usize] = Some(new_oceanic_crust(time));
                        plate.consumed[local as usize] = false;
                        new_owners[neighbor as usize] = Some(Ownership {
                            plate: plate_index,
                            local,
                        });
                    }
                } else {
                    // A resampling gap: healed without creating crust.
                    let interior = !mesh.neighbors(neighbor).iter().any(|&n| {
                        new_owners[n as usize].is_some_and(|owner| owner.plate != plate_index)
                    });
                    let plate = &mut plates[plate_index as usize];
                    new_owners[neighbor as usize] = Some(heal_gap(
                        mesh,
                        plate,
                        parent,
                        local,
                        interior,
                        gap_statistics,
                    ));
                }
                queue.push_back(neighbor);
            }
        }

        for (owner, new_owner) in owners.iter_mut().zip(new_owners) {
            *owner = new_owner.expect("all cells are owned after gap filling");
        }
        matching::update_crust_cells(mesh, plates, found, &filled, matching_scratch);
        ResolvedBoundaries {
            additions,
            collisions,
        }
    }

    /// Distributes new crust over the cells of the receiving plate around each addition,
    /// following the arc or collision profile. Crust that does not fit below the maximum
    /// thickness spills further outwards over up to [`SPILL_RINGS`] more rings, and is only
    /// removed (delamination) when there is no room left there either.
    pub(super) fn apply_additions(&mut self, additions: &[CrustAddition]) {
        let mut nominal = 0.0;
        for addition in additions {
            let mass = addition.thickness * addition.density;
            nominal += mass;
            let term = match addition.kind {
                AdditionKind::Arc => Term::Arcs,
                AdditionKind::Accretion => Term::Accretion,
                AdditionKind::Collision => Term::CollisionStacked,
            };
            self.ledger.record(term, mass);
        }
        let actual = self.apply_additions_inner(additions);
        self.ledger.record(Term::AdditionsLost, actual - nominal);
    }

    /// Returns the change of mass.
    fn apply_additions_inner(&mut self, additions: &[CrustAddition]) -> f64 {
        let mut change = 0.0;
        let mut visited_in_event = vec![usize::MAX; self.mesh.len()];
        let mut frontier = Vec::new();
        let mut next_frontier = Vec::new();
        // Reached crust samples with their profile weight (0 beyond the profile).
        let mut reached: Vec<(Ownership, f64)> = Vec::new();
        let max_thickness = self.params.max_crust_thickness;

        for (event_index, addition) in additions.iter().enumerate() {
            let profile = match addition.kind {
                AdditionKind::Collision => &self.params.collision_profile,
                AdditionKind::Arc | AdditionKind::Accretion => &self.params.arc_profile,
            };
            frontier.clear();
            frontier.push(addition.cell);
            visited_in_event[addition.cell as usize] = event_index;
            reached.clear();

            for ring in 0..profile.len() + SPILL_RINGS {
                let weight = profile.get(ring).copied().unwrap_or(0.0);
                next_frontier.clear();
                for &cell in &frontier {
                    let owner = self.owners[cell as usize];
                    if owner.plate != addition.plate {
                        continue;
                    }
                    reached.push((owner, weight));
                    for &neighbor in self.mesh.neighbors(cell) {
                        if visited_in_event[neighbor as usize] != event_index {
                            visited_in_event[neighbor as usize] = event_index;
                            next_frontier.push(neighbor);
                        }
                    }
                }
                std::mem::swap(&mut frontier, &mut next_frontier);
            }

            let total_weight: f64 = reached.iter().map(|(_, weight)| weight).sum();
            if total_weight <= 0.0 {
                continue;
            }
            // Distribute along the profile, then spill what did not fit outwards, nearest first.
            let mut leftover = 0.0;
            for &(owner, weight) in &reached {
                let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                    .as_mut()
                    .expect("owners always point to existing crust");
                let share = addition.thickness * weight / total_weight;
                let fitting = share.min((max_thickness - crust.thickness).max(0.0));
                let before = crust.mass();
                crust.add_crust(fitting, addition.density, max_thickness);
                change += crust.mass() - before;
                leftover += share - fitting;
            }
            for &(owner, _) in &reached {
                if leftover <= 0.0 {
                    break;
                }
                let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                    .as_mut()
                    .expect("owners always point to existing crust");
                let fitting = leftover.min((max_thickness - crust.thickness).max(0.0));
                let before = crust.mass();
                crust.add_crust(fitting, addition.density, max_thickness);
                change += crust.mass() - before;
                leftover -= fitting;
            }
        }
        change
    }
}

pub(super) struct ResolvedBoundaries {
    pub(super) additions: Vec<CrustAddition>,
    /// Number of continental collision events per plate pair.
    pub(super) collisions: HashMap<(u32, u32), u32>,
}

/// A crust sample overridden by another plate at a cell.
struct LostContact {
    cell: u32,
    winner: Ownership,
    loser: Ownership,
    loser_crust: Crust,
}

/// Counts of how uncovered cells were filled, accumulated over the simulation.
#[derive(Clone, Debug, Default)]
pub struct GapStatistics {
    /// Cells filled with new oceanic crust where plates spread apart.
    pub new_crust_cells: usize,
    /// Resampling gaps showing a hidden sample of the plate again.
    pub revealed_cells: usize,
    /// Enclosed empty spots of a plate filled with a share of the surrounding crust.
    pub enclosed_fills: usize,
    /// Gaps showing the adjacent sample (resampling gaps at plate edges, and slivers where the
    /// plate's crust was consumed).
    pub shared_cells: usize,
    /// Of those, resampling gaps away from other plates (holes too empty to fill).
    pub interior_shared_cells: usize,
    /// Single-cell holes inside a plate (healed or shared, also counted there).
    pub hole_cells: usize,
    /// Cells whose surface crust was consumed while visible (should stay zero).
    pub freed_cells: usize,
}

/// Whether `plate` moves away from any other plate near `cell` faster than `min_opening`
/// (rad/Myr), so that new sea floor forms at `cell`. The direction between two plates is taken
/// from the centroids of their cells within a few rings around `cell`, which is smoother than
/// single mesh edges, and the motion must not be too oblique (transform faults do not spread).
fn opens_away(
    scratch: &mut RingScratch,
    mesh: &SphereMesh,
    owners: &[Option<Ownership>],
    angular_velocities: &[DVec3],
    cell: u32,
    plate: u32,
    min_opening: f64,
) -> bool {
    const RINGS: usize = 3;
    /// Opening must be at least this fraction of the relative speed.
    const MIN_OBLIQUITY: f64 = 0.25;

    let RingScratch {
        visited,
        stamp,
        frontier,
        next,
        centroids,
    } = scratch;
    // A new stamp marks the cells visited in this call.
    *stamp += 1;
    if *stamp == 0 {
        visited.fill(0);
        *stamp = 1;
    }
    let stamp = *stamp;
    centroids.clear();
    frontier.clear();
    visited[cell as usize] = stamp;
    frontier.push(cell);
    for _ in 0..RINGS {
        next.clear();
        for &current in frontier.iter() {
            for &neighbor in mesh.neighbors(current) {
                if visited[neighbor as usize] == stamp {
                    continue;
                }
                visited[neighbor as usize] = stamp;
                next.push(neighbor);
                if let Some(owner) = owners[neighbor as usize] {
                    let position = mesh.vertex(neighbor);
                    match centroids.iter_mut().find(|(p, _)| *p == owner.plate) {
                        Some((_, sum)) => *sum += position,
                        None => centroids.push((owner.plate, position)),
                    }
                }
            }
        }
        std::mem::swap(frontier, next);
    }

    let r = mesh.vertex(cell);
    let tangent = |v: DVec3| (v - r * r.dot(v)).normalize_or_zero();
    let Some(&(_, own)) = centroids.iter().find(|(p, _)| *p == plate) else {
        return false;
    };
    centroids.iter().any(|&(other, centroid)| {
        if other == plate {
            return false;
        }
        let normal = tangent(centroid.normalize() - own.normalize());
        let relative =
            (angular_velocities[other as usize] - angular_velocities[plate as usize]).cross(r);
        let opening = relative.dot(normal);
        opening > min_opening && opening > MIN_OBLIQUITY * relative.length()
    })
}

/// Reusable buffers for [`opens_away`].
struct RingScratch {
    /// Per cell: the stamp of the last call that visited it.
    visited: Vec<u32>,
    stamp: u32,
    frontier: Vec<u32>,
    next: Vec<u32>,
    /// Per plate near the cell: the sum of the positions of its cells.
    centroids: Vec<(u32, DVec3)>,
}

impl RingScratch {
    fn new(cell_count: usize) -> Self {
        Self {
            visited: vec![0; cell_count],
            stamp: 0,
            frontier: Vec::new(),
            next: Vec::new(),
            centroids: Vec::new(),
        }
    }
}

/// Minimum number of neighbours with crust (in the plate's frame) for an empty spot inside a
/// plate to be filled.
const MIN_FILL_NEIGHBOURS: usize = 3;

/// Heals a resampling gap of `plate` at a cell whose local vertex `local` was not consumed,
/// next to the plate's sample `parent`, without creating crust; returns the sample to show.
/// `interior` tells whether no other plate is next to the cell.
///
/// Rigid plates resampled onto the cells leave gaps that are not real openings. A hidden sample
/// at the local vertex is shown again. An empty local vertex enclosed by crust of the plate (in
/// its own frame; e.g. spots that new sea floor never sampled, or holes left by merges), or one
/// inside the plate with at least [`MIN_FILL_NEIGHBOURS`] neighbours with crust (holes of a few
/// vertices fill from their rim), gets a sample made of a share of its neighbours' crust, which
/// conserves mass. Elsewhere (plate edges, boundaries) the gap shows the adjacent sample:
/// copying it would create crust from nothing.
fn heal_gap(
    mesh: &SphereMesh,
    plate: &mut Plate,
    parent: Ownership,
    local: u32,
    interior: bool,
    statistics: &mut GapStatistics,
) -> Ownership {
    let own = Ownership {
        plate: parent.plate,
        local,
    };
    if plate.crust[local as usize].is_some() {
        statistics.revealed_cells += 1;
        return own;
    }
    let neighbors: Vec<u32> = mesh
        .neighbors(local)
        .iter()
        .copied()
        .filter(|&neighbor| plate.crust[neighbor as usize].is_some())
        .collect();
    let enclosed = neighbors.len() == mesh.neighbors(local).len();
    if !enclosed && !(interior && neighbors.len() >= MIN_FILL_NEIGHBOURS) {
        statistics.shared_cells += 1;
        if interior {
            statistics.interior_shared_cells += 1;
        }
        return parent;
    }
    // Each of the k neighbours gives 1/(k + 1) of its crust and sediment. The lithosphere is as
    // young as the youngest neighbour's (an average with old continental lithosphere would make
    // oceanic crust ancient).
    let share = 1.0 / (neighbors.len() + 1) as f64;
    let (mut thickness, mut mass, mut sediment) = (0.0, 0.0, 0.0);
    let mut birth = f64::NEG_INFINITY;
    for &neighbor in &neighbors {
        let crust = plate.crust[neighbor as usize]
            .as_mut()
            .expect("enclosed by crust");
        let given = crust.thickness * share;
        thickness += given;
        mass += given * crust.density;
        birth = birth.max(crust.birth);
        sediment += crust.sediment * share;
        crust.thickness -= given;
        crust.sediment -= crust.sediment * share;
    }
    plate.crust[local as usize] = Some(Crust {
        thickness,
        density: mass / thickness.max(1e-9),
        birth,
        sediment,
    });
    plate.consumed[local as usize] = false;
    statistics.enclosed_fills += 1;
    own
}

/// New crust added to a plate around a cell.
pub(super) struct CrustAddition {
    cell: u32,
    plate: u32,
    /// Total volume as a thickness over one cell.
    thickness: f64,
    density: f64,
    kind: AdditionKind,
}

/// Where added crust comes from; decides its distribution and its ledger term.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AdditionKind {
    /// New arc crust from melting above a slab.
    Arc,
    /// Sediment scraped off at a trench.
    Accretion,
    /// Crust of the lower plate stacked in a continental collision.
    Collision,
}

/// Density of crust formed from sediment (metasediment) in kg/m³.
const METASEDIMENT_DENSITY: f64 = 2650.0;

pub(super) fn pair_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Chooses the plate whose crust stays at the surface among the plates covering a cell at the
/// world position `r`, recording the contact of each pair in their boundary segment there (see
/// [`polarity::Polarities::contact`]).
pub(super) fn choose_overriding(
    candidates: &[(Ownership, Crust)],
    polarities: &mut polarity::Polarities,
    r: DVec3,
    rotations: &[DQuat],
    time: f64,
) -> Option<(Ownership, Crust)> {
    if candidates.len() <= 1 {
        return candidates.first().copied();
    }
    // The plate on top for each pair of candidates at this cell.
    let mut on_top: Vec<((u32, u32), u32)> = Vec::new();
    for (i, &(a, a_crust)) in candidates.iter().enumerate() {
        for &(b, b_crust) in &candidates[i + 1..] {
            let top =
                polarities.contact((a.plate, a_crust), (b.plate, b_crust), r, time, rotations);
            on_top.push((pair_key(a.plate, b.plate), top));
        }
    }

    // Prefer the plate that is on top of all others; with three or more plates the relation may
    // be cyclic, in which case buoyancy decides.
    candidates
        .iter()
        .find(|(a, _)| {
            on_top
                .iter()
                .filter(|((p, q), _)| *p == a.plate || *q == a.plate)
                .all(|(_, top)| *top == a.plate)
        })
        .or_else(|| {
            candidates
                .iter()
                .max_by(|a, b| a.1.buoyancy(time).total_cmp(&b.1.buoyancy(time)))
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One oceanic plate covering the sphere.
    fn single_plate() -> Simulation {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 1, TectonicParams::default(), 3);
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = new_oceanic_crust(-50.0);
        }
        simulation
    }

    /// Plate 0 (south) carries a thick arc next to the equator and moves north into plate 1
    /// (north), below which it would subduct; plate 1 is oceanic or continental near the
    /// boundary.
    fn arc_arriving_at_a_trench(continental_upper_plate: bool) -> MassLedger {
        let mut simulation = single_plate();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        for plate in 0..2 {
            for (local, crust) in simulation.plates[plate].crust.iter_mut().enumerate() {
                let z = simulation.mesh.vertex(local as u32).z;
                if let Some(crust) = crust {
                    if plate == 0 && z > -0.3 {
                        *crust = Crust {
                            thickness: 18_000.0,
                            density: crate::isostasy::ARC_DENSITY,
                            birth: -50.0,
                            sediment: 0.0,
                        };
                        assert!(crust.resists_subduction() && !crust.is_continental());
                    } else if plate == 1 && continental_upper_plate && z < 0.3 {
                        *crust = Crust {
                            thickness: 35_000.0,
                            density: crate::isostasy::CONTINENTAL_DENSITY,
                            birth: -1000.0,
                            sediment: 0.0,
                        };
                    }
                }
            }
        }
        simulation.seed_subduction((0, 1), 1, 600e3);
        // Rotating plate 0 about the y axis moves its side at x > 0 northwards.
        simulation.plates[0].angular_velocity = -DVec3::Y * 0.004;
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        for _ in 0..20 {
            simulation.substep(1.0);
        }
        simulation.ledger.clone()
    }

    #[test]
    fn buoyant_arcs_are_not_subducted() {
        // Below an oceanic upper plate the arc stays on top; the forearc is consumed.
        let ledger = arc_arriving_at_a_trench(false);
        assert_eq!(ledger.subducted_transitional, 0.0, "{ledger:?}");
        assert!(ledger.subducted_oceanic < 0.0, "{ledger:?}");
        // Against a continent, the arc is accreted (stacked onto the overriding plate).
        let ledger = arc_arriving_at_a_trench(true);
        assert_eq!(ledger.subducted_transitional, 0.0, "{ledger:?}");
        assert!(ledger.collision_stacked > 0.0, "{ledger:?}");
    }

    #[test]
    fn enclosed_holes_are_filled_from_their_neighbours() {
        let mut simulation = single_plate();
        let local = 100;
        let removed = simulation.plates[0].crust[local].take().unwrap();
        let before = simulation.total_mass();
        let mut statistics = GapStatistics::default();
        let parent = Ownership {
            plate: 0,
            local: 101,
        };
        let shown = heal_gap(
            &simulation.mesh,
            &mut simulation.plates[0],
            parent,
            local as u32,
            true,
            &mut statistics,
        );
        assert_eq!(
            shown,
            Ownership {
                plate: 0,
                local: local as u32
            }
        );
        assert_eq!(statistics.enclosed_fills, 1);
        let filled = simulation.plates[0].crust[local].unwrap();
        assert!((filled.density - removed.density).abs() < 1e-9);
        assert!(filled.thickness > 0.0);
        // No crust was created.
        assert!((simulation.total_mass() / before - 1.0).abs() < 1e-12);
    }

    #[test]
    fn holes_of_a_few_vertices_inside_a_plate_are_filled_from_their_rim() {
        let mut simulation = single_plate();
        // Two neighbouring empty vertices: neither is fully enclosed.
        let local = 100u32;
        let other = simulation.mesh.neighbors(local)[0];
        simulation.plates[0].crust[local as usize] = None;
        simulation.plates[0].crust[other as usize] = None;
        let before = simulation.total_mass();
        let mut statistics = GapStatistics::default();
        let parent = Ownership { plate: 0, local: 0 };
        for vertex in [local, other] {
            let shown = heal_gap(
                &simulation.mesh,
                &mut simulation.plates[0],
                parent,
                vertex,
                true,
                &mut statistics,
            );
            assert_eq!(
                shown,
                Ownership {
                    plate: 0,
                    local: vertex
                }
            );
        }
        assert_eq!(statistics.enclosed_fills, 2);
        assert!((simulation.total_mass() / before - 1.0).abs() < 1e-12);
    }

    #[test]
    fn edge_gaps_show_the_adjacent_sample() {
        let mut simulation = single_plate();
        // An empty spot with an empty neighbour: at the edge of the plate's crust.
        let local = 100u32;
        let neighbor = simulation.mesh.neighbors(local)[0];
        simulation.plates[0].crust[local as usize] = None;
        simulation.plates[0].crust[neighbor as usize] = None;
        let before = simulation.total_mass();
        let mut statistics = GapStatistics::default();
        let parent = Ownership { plate: 0, local: 0 };
        let shown = heal_gap(
            &simulation.mesh,
            &mut simulation.plates[0],
            parent,
            local,
            false,
            &mut statistics,
        );
        assert_eq!(shown, parent);
        assert_eq!(statistics.shared_cells, 1);
        assert!(simulation.plates[0].crust[local as usize].is_none());
        assert_eq!(simulation.total_mass(), before);
    }

    #[test]
    fn rigid_rotation_creates_no_crust() {
        let mut simulation = single_plate();
        let before = simulation.total_mass();
        simulation.plates[0].angular_velocity = DVec3::new(0.3, 0.5, 0.8).normalize() * 0.004;
        for _ in 0..40 {
            simulation.substep(1.0);
        }
        let after = simulation.total_mass();
        assert!((after / before - 1.0).abs() < 1e-12, "{after} vs {before}");
        assert_eq!(simulation.gap_statistics.new_crust_cells, 0);
    }

    #[test]
    fn plates_sliding_past_each_other_keep_their_crust() {
        // Two oceanic hemispheres rotating in opposite senses about the pole: a transform
        // boundary along the equator.
        let mut simulation = single_plate();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        let samples = |simulation: &Simulation| -> Vec<usize> {
            simulation
                .plates
                .iter()
                .map(|plate| plate.crust.iter().flatten().count())
                .collect()
        };
        let before = samples(&simulation);
        simulation.plates[0].angular_velocity = DVec3::Z * 0.004;
        simulation.plates[1].angular_velocity = -DVec3::Z * 0.004;
        for _ in 0..40 {
            simulation.substep(1.0);
        }
        let after = samples(&simulation);
        for (b, a) in before.iter().zip(&after) {
            // Nothing is created at the boundary (at most a few enclosed fills); overlaps of
            // the resampled plates along the transform are subducted as noise (known), which
            // loses a little crust.
            assert!(*a <= *b + 10, "{before:?} → {after:?}");
            assert!(*a as f64 >= 0.97 * *b as f64, "{before:?} → {after:?}");
        }
        let shared_per_substep = simulation.gap_statistics.shared_cells as f64 / 40.0;
        assert!(
            shared_per_substep < 0.05 * simulation.mesh.len() as f64,
            "{shared_per_substep}"
        );
    }

    /// Plate 0 covers x < -0.02, plate 1 covers x > 0.02, the band between is uncovered. Returns
    /// whether plate 1 opens away from plate 0 at the uncovered cell nearest to +z.
    fn spreading(omega_0: DVec3, omega_1: DVec3) -> bool {
        let mesh = SphereMesh::icosphere(4);
        let owners: Vec<Option<Ownership>> = (0..mesh.len() as u32)
            .map(|cell| {
                let x = mesh.vertex(cell).x;
                if x < -0.02 {
                    Some(Ownership {
                        plate: 0,
                        local: cell,
                    })
                } else if x > 0.02 {
                    Some(Ownership {
                        plate: 1,
                        local: cell,
                    })
                } else {
                    None
                }
            })
            .collect();
        let gap = (0..mesh.len() as u32)
            .filter(|&cell| owners[cell as usize].is_none())
            .max_by(|&a, &b| mesh.vertex(a).z.total_cmp(&mesh.vertex(b).z))
            .unwrap();
        // 0.5 cm/yr in rad/Myr.
        let min_opening = 0.5 / 637.1;
        opens_away(
            &mut RingScratch::new(mesh.len()),
            &mesh,
            &owners,
            &[omega_0, omega_1],
            gap,
            1,
            min_opening,
        )
    }

    #[test]
    fn diverging_plates_make_new_sea_floor() {
        // At +z, ω = ±y gives velocities along ±x: the plates move apart at ~6 cm/yr each.
        assert!(spreading(-DVec3::Y * 0.01, DVec3::Y * 0.01));
    }

    #[test]
    fn transform_faults_and_trenches_do_not() {
        // ω = ±x gives velocities along ∓y at +z: fast sliding along the boundary.
        assert!(!spreading(-DVec3::X * 0.01, DVec3::X * 0.01));
        // Converging plates.
        assert!(!spreading(DVec3::Y * 0.01, -DVec3::Y * 0.01));
        // Spreading slower than the threshold.
        assert!(!spreading(DVec3::ZERO, DVec3::Y * 0.0003));
    }
}
