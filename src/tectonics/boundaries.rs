//! Plate boundaries: assigning cells to plates, subduction, arc growth, continental
//! collision and crustal flow.

use std::collections::{HashMap, HashSet, VecDeque};

use glam::{DQuat, DVec3};

use crate::isostasy::{ARC_DENSITY, SEDIMENT_ROCK_DENSITY};

use super::ledger::{self, Term};
use super::*;

/// Rings beyond the arc or collision profile over which crust that exceeds the maximum
/// thickness spills.
const SPILL_RINGS: usize = 8;

impl Simulation {
    /// Thick crust spreads under its own weight towards thinner neighbouring crust of the same
    /// plate, conserving its volume (lower crustal flow, gravitational collapse of orogens).
    pub(super) fn crustal_flow(&mut self, dt: f64) {
        let before = ledger::plates_mass(&self.plates);
        self.crustal_flow_inner(dt);
        let after = ledger::plates_mass(&self.plates);
        self.ledger.record(Term::CrustalFlow, after - before);
    }

    fn crustal_flow_inner(&mut self, dt: f64) {
        let params = &self.params;
        let rate = (params.crustal_flow_rate * dt).min(0.15);
        // Per crust sample: (outgoing thickness, incoming thickness, incoming mass per area).
        let mut delta: HashMap<Ownership, (f64, f64, f64)> = HashMap::new();
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
                delta.entry(from).or_default().0 += amount;
                let receiver = delta.entry(to).or_default();
                receiver.1 += amount;
                receiver.2 += amount * from_crust.density;
            }
        }
        for (owner, (outgoing, incoming, incoming_mass)) in delta {
            let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                .as_mut()
                .expect("owners always point to existing crust");
            // Outgoing crust leaves the density unchanged; incoming crust is mixed in by mass.
            crust.thickness = (crust.thickness - outgoing).max(0.0);
            if incoming > 0.0 {
                crust.add_crust(
                    incoming,
                    incoming_mass / incoming,
                    params.max_crust_thickness,
                );
            }
        }
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
            ..
        } = self;
        polarities.retain(|_, polarity| *time - polarity.last_contact <= params.polarity_memory);
        let cell_count = mesh.len();
        let inverse_rotations: Vec<DQuat> = plates.iter().map(|p| p.rotation.inverse()).collect();
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
        let mut collisions: HashMap<(u32, u32), u32> = HashMap::new();
        let mut candidates: Vec<(Ownership, Crust)> = Vec::with_capacity(plates.len());

        for cell in 0..cell_count as u32 {
            candidates.clear();
            for (index, plate) in plates.iter_mut().enumerate() {
                if !alive[index] {
                    continue;
                }
                let local = plate.local_vertex(mesh, inverse_rotations[index], cell);
                if let Some(crust) = plate.crust[local as usize] {
                    let plate = index as u32;
                    candidates.push((Ownership { plate, local }, crust));
                }
            }

            let Some((winner, winner_crust)) = choose_overriding(&candidates, polarities, *time)
            else {
                continue;
            };
            new_owners[cell as usize] = Some(winner);

            for &(loser, loser_crust) in &candidates {
                if loser != winner {
                    lost_contacts.push(LostContact {
                        cell,
                        winner,
                        winner_crust,
                        loser,
                        loser_crust,
                    });
                }
            }
        }

        // A sample that is overridden at one cell but still forms the surface of another cell is
        // kept until it is covered everywhere: deleting it now would leave a hole behind the
        // trench. Each consumed sample is accounted for exactly once.
        let visible: HashSet<Ownership> = new_owners.iter().flatten().copied().collect();
        let mut consumed: HashSet<Ownership> = HashSet::new();
        for contact in &lost_contacts {
            if visible.contains(&contact.loser) || !consumed.insert(contact.loser) {
                continue;
            }
            let loser_plate = &mut plates[contact.loser.plate as usize];
            loser_plate.crust[contact.loser.local as usize] = None;
            loser_plate.consumed[contact.loser.local as usize] = true;
            let (winner, loser_crust) = (contact.winner, contact.loser_crust);
            let sediment_mass = loser_crust.sediment * SEDIMENT_ROCK_DENSITY;
            if contact.winner_crust.is_continental() && loser_crust.is_continental() {
                // Continental collision: the crust of the lower plate is stacked onto the
                // overriding plate instead of subducting; its sediment becomes metasediment at
                // the same mass.
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
                } else if loser_crust.is_continental() {
                    Term::SubductedContinental
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
                // Part of the sediment is scraped off at the trench into an accretionary wedge
                // on the overriding plate, which becomes crust (metasediment) at the same mass.
                let accreted_mass = sediment_mass * params.frontal_accretion;
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
        // filled with a copy of the neighbouring crust.
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
        // Holes are healed with a copy of the neighbouring crust, unless the crust there was
        // consumed; then the hole shows the neighbouring sample.
        for (cell, template) in holes {
            let plate_index = template.plate as usize;
            let plate = &mut plates[plate_index];
            let local = plate.local_vertex(mesh, inverse_rotations[plate_index], cell);
            if plate.consumed[local as usize] {
                gap_statistics.shared_cells += 1;
                new_owners[cell as usize] = Some(template);
            } else {
                let crust =
                    plate.crust[template.local as usize].expect("owners point to existing crust");
                if plate.crust[local as usize].is_none() {
                    ledger.record(Term::GapFilling, crust.mass());
                }
                plate.crust[local as usize].get_or_insert(crust);
                gap_statistics.copied_cells += 1;
                gap_statistics.copied_volume += crust.thickness;
                new_owners[cell as usize] = Some(Ownership {
                    plate: template.plate,
                    local,
                });
            }
        }

        // Fill the remaining gaps, growing the adjacent plates layer by layer. Where the plate
        // moves away from a neighbouring plate, the gap is new sea floor (spreading ridge);
        // otherwise it is an artifact of resampling the rigid plates onto the cells (at trenches
        // and transform faults), and the gap gets a copy of the adjacent crust.
        let angular_velocities: Vec<DVec3> = plates.iter().map(|p| p.angular_velocity).collect();
        let min_opening = params.min_spreading_rate / (params.planet_radius * 100.0 / 1e6);
        let mut queue: VecDeque<u32> = (0..cell_count as u32)
            .filter(|&cell| new_owners[cell as usize].is_some())
            .collect();
        if queue.is_empty() {
            // All crust was destroyed, which can only happen in degenerate configurations.
            // Restart with the first plate covering the first cell.
            plates[0].crust[0] = Some(new_oceanic_crust());
            ledger.record(Term::NewSeaFloor, new_oceanic_crust().mass());
            new_owners[0] = Some(Ownership { plate: 0, local: 0 });
            queue.push_back(0);
        }
        while let Some(cell) = queue.pop_front() {
            let parent = new_owners[cell as usize].expect("queued cells are owned");
            let plate_index = parent.plate;
            for &neighbor in mesh.neighbors(cell) {
                if new_owners[neighbor as usize].is_some() {
                    continue;
                }
                let spreading = opens_away(
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
                        ledger.record(Term::NewSeaFloor, new_oceanic_crust().mass());
                    }
                    plate.crust[local as usize].get_or_insert_with(new_oceanic_crust);
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
                        ledger.record(Term::NewSeaFloor, new_oceanic_crust().mass() - replaced);
                        plate.crust[local as usize] = Some(new_oceanic_crust());
                        plate.consumed[local as usize] = false;
                        new_owners[neighbor as usize] = Some(Ownership {
                            plate: plate_index,
                            local,
                        });
                    }
                } else {
                    // A resampling gap: heal it with a copy of the adjacent crust.
                    let crust =
                        plate.crust[parent.local as usize].expect("owners point to existing crust");
                    if plate.crust[local as usize].is_none() {
                        ledger.record(Term::GapFilling, crust.mass());
                    }
                    plate.crust[local as usize].get_or_insert(crust);
                    gap_statistics.copied_cells += 1;
                    gap_statistics.copied_volume += crust.thickness;
                    new_owners[neighbor as usize] = Some(Ownership {
                        plate: plate_index,
                        local,
                    });
                }
                queue.push_back(neighbor);
            }
        }

        for (owner, new_owner) in owners.iter_mut().zip(new_owners) {
            *owner = new_owner.expect("all cells are owned after gap filling");
        }
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
        let before = ledger::plates_mass(&self.plates);
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
        self.apply_additions_inner(additions);
        let actual = ledger::plates_mass(&self.plates) - before;
        self.ledger.record(Term::AdditionsLost, actual - nominal);
    }

    fn apply_additions_inner(&mut self, additions: &[CrustAddition]) {
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
                crust.add_crust(fitting, addition.density, max_thickness);
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
                crust.add_crust(fitting, addition.density, max_thickness);
                leftover -= fitting;
            }
        }
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
    winner_crust: Crust,
    loser: Ownership,
    loser_crust: Crust,
}

/// Counts of how uncovered cells were filled, accumulated over the simulation.
#[derive(Clone, Debug, Default)]
pub struct GapStatistics {
    /// Cells filled with new oceanic crust where plates spread apart.
    pub new_crust_cells: usize,
    /// Resampling gaps healed with a copy of the adjacent crust.
    pub copied_cells: usize,
    /// Crust thickness (m · cells) created by copies.
    pub copied_volume: f64,
    /// Gaps where the plate's crust was consumed, showing the adjacent sample instead.
    pub shared_cells: usize,
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

    let mut centroids: Vec<(u32, DVec3)> = Vec::new();
    let mut visited = vec![cell];
    let mut frontier = vec![cell];
    for _ in 0..RINGS {
        let mut next = Vec::new();
        for &current in &frontier {
            for &neighbor in mesh.neighbors(current) {
                if visited.contains(&neighbor) {
                    continue;
                }
                visited.push(neighbor);
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
        frontier = next;
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

/// Chooses the overriding plate among the plates covering a cell, recording and updating the
/// subduction polarity of each pair of plates in contact.
pub(super) fn choose_overriding(
    candidates: &[(Ownership, Crust)],
    polarities: &mut HashMap<(u32, u32), Polarity>,
    time: f64,
) -> Option<(Ownership, Crust)> {
    for (i, &(a, a_crust)) in candidates.iter().enumerate() {
        for &(b, b_crust) in &candidates[i + 1..] {
            let polarity = polarities
                .entry(pair_key(a.plate, b.plate))
                .or_insert_with(|| Polarity {
                    overriding: if a_crust.buoyancy() >= b_crust.buoyancy() {
                        a.plate
                    } else {
                        b.plate
                    },
                    last_contact: time,
                    collision_time: 0.0,
                    slab_length: 0.0,
                });
            polarity.last_contact = time;
            let (top, top_crust, bottom, bottom_crust) = if polarity.overriding == a.plate {
                (a, a_crust, b, b_crust)
            } else {
                (b, b_crust, a, a_crust)
            };
            if bottom_crust.is_continental() && !top_crust.is_continental() {
                // Continental crust arrived at the trench: the polarity flips, and the old slab
                // belonged to the other plate.
                polarity.overriding = bottom.plate;
                polarity.slab_length = 0.0;
            } else {
                polarity.overriding = top.plate;
            }
        }
    }

    // Prefer the plate that overrides all others; with three or more plates the relation may be
    // cyclic, in which case buoyancy decides.
    candidates
        .iter()
        .find(|(a, _)| {
            candidates.iter().all(|(b, _)| {
                a.plate == b.plate || polarities[&pair_key(a.plate, b.plate)].overriding == a.plate
            })
        })
        .or_else(|| {
            candidates
                .iter()
                .max_by(|a, b| a.1.buoyancy().total_cmp(&b.1.buoyancy()))
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        opens_away(&mesh, &owners, &[omega_0, omega_1], gap, 1, min_opening)
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
