//! Plate boundaries: assigning cells to plates, subduction, arc growth, continental
//! collision and crustal flow.

use std::collections::{HashMap, VecDeque};

use glam::DQuat;

use crate::isostasy::{ARC_DENSITY, SEDIMENT_DENSITY};

use super::*;

/// Rings beyond the arc or collision profile over which crust that exceeds the maximum
/// thickness spills.
const SPILL_RINGS: usize = 8;

impl Simulation {
    /// Thick crust spreads under its own weight towards thinner neighbouring crust of the same
    /// plate, conserving its volume (lower crustal flow, gravitational collapse of orogens).
    pub(super) fn crustal_flow(&mut self, dt: f64) {
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
        let mut subducted = Vec::new();
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
                if loser == winner {
                    continue;
                }
                subducted.push(loser);
                if winner_crust.is_continental() && loser_crust.is_continental() {
                    // Continental collision: the crust of the lower plate is stacked onto the
                    // overriding plate instead of subducting.
                    *collisions
                        .entry(pair_key(winner.plate, loser.plate))
                        .or_default() += 1;
                    let sediment_mass = loser_crust.sediment * SEDIMENT_DENSITY;
                    let thickness = loser_crust.thickness + loser_crust.sediment;
                    additions.push(CrustAddition {
                        cell,
                        plate: winner.plate,
                        thickness,
                        density: (loser_crust.thickness * loser_crust.density + sediment_mass)
                            / thickness.max(1e-9),
                        collision: true,
                    });
                } else {
                    // Subduction: the lower plate's crust and sediment sink into the mantle, and
                    // melting above the slab builds an arc on the overriding plate.
                    additions.push(CrustAddition {
                        cell,
                        plate: winner.plate,
                        thickness: params.arc_crust_per_subduction,
                        density: ARC_DENSITY,
                        collision: false,
                    });
                }
            }
        }

        for ownership in subducted {
            plates[ownership.plate as usize].crust[ownership.local as usize] = None;
        }

        // A sample may have been subducted at one cell while owning another; that cell is now free.
        for owner in &mut new_owners {
            if let Some(ownership) = owner
                && plates[ownership.plate as usize].crust[ownership.local as usize].is_none()
            {
                *owner = None;
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
        for (cell, template) in holes {
            let plate_index = template.plate as usize;
            let plate = &mut plates[plate_index];
            let template_crust =
                plate.crust[template.local as usize].expect("owners point to existing crust");
            let local = plate.local_vertex(mesh, inverse_rotations[plate_index], cell);
            plate.crust[local as usize].get_or_insert(template_crust);
            new_owners[cell as usize] = Some(Ownership {
                plate: template.plate,
                local,
            });
        }

        // Fill the remaining gaps with new oceanic crust, growing the adjacent plates layer by
        // layer.
        let mut queue: VecDeque<u32> = (0..cell_count as u32)
            .filter(|&cell| new_owners[cell as usize].is_some())
            .collect();
        if queue.is_empty() {
            // All crust was destroyed, which can only happen in degenerate configurations.
            // Restart with the first plate covering the first cell.
            plates[0].crust[0] = Some(new_oceanic_crust());
            new_owners[0] = Some(Ownership { plate: 0, local: 0 });
            queue.push_back(0);
        }
        while let Some(cell) = queue.pop_front() {
            let plate_index = new_owners[cell as usize]
                .expect("queued cells are owned")
                .plate;
            for &neighbor in mesh.neighbors(cell) {
                if new_owners[neighbor as usize].is_some() {
                    continue;
                }
                let plate = &mut plates[plate_index as usize];
                let local =
                    plate.local_vertex(mesh, inverse_rotations[plate_index as usize], neighbor);
                plate.crust[local as usize].get_or_insert_with(new_oceanic_crust);
                new_owners[neighbor as usize] = Some(Ownership {
                    plate: plate_index,
                    local,
                });
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
        let mut visited_in_event = vec![usize::MAX; self.mesh.len()];
        let mut frontier = Vec::new();
        let mut next_frontier = Vec::new();
        // Reached crust samples with their profile weight (0 beyond the profile).
        let mut reached: Vec<(Ownership, f64)> = Vec::new();
        let max_thickness = self.params.max_crust_thickness;

        for (event_index, addition) in additions.iter().enumerate() {
            let profile = if addition.collision {
                &self.params.collision_profile
            } else {
                &self.params.arc_profile
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

/// New crust added to a plate around a cell.
pub(super) struct CrustAddition {
    cell: u32,
    plate: u32,
    /// Total volume as a thickness over one cell.
    thickness: f64,
    density: f64,
    /// Whether this is crust stacked in a continental collision rather than new arc crust.
    collision: bool,
}

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
                });
            polarity.last_contact = time;
            let (top, top_crust, bottom, bottom_crust) = if polarity.overriding == a.plate {
                (a, a_crust, b, b_crust)
            } else {
                (b, b_crust, a, a_crust)
            };
            if bottom_crust.is_continental() && !top_crust.is_continental() {
                polarity.overriding = bottom.plate;
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
