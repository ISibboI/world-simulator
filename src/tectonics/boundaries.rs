//! Plate boundaries: assigning cells to plates, subduction, arc growth, continental
//! collision and crustal flow.

use std::collections::VecDeque;

use glam::{DQuat, DVec3};

use crate::isostasy::{ARC_DENSITY, SEDIMENT_ROCK_DENSITY};

use super::deformation;
use super::ledger::Term;
use super::matching;
use super::*;

/// Rings beyond the arc or collision profile over which crust that exceeds the maximum
/// thickness spills.
const SPILL_RINGS: usize = 8;

/// Subduction erosion at the trench cells in `eroded` (one entry per subducted sample, with
/// the overriding plate), by `amount` (m of column over one cell, sediment first, down to
/// oceanic thickness) per event, after the final assignment of samples to cells (`owners`).
///
/// Each event erodes the sample its cell shows. A sample shown at k cells is thinned once by
/// the mean of its cells' erosion, `n / k × amount` for n events, not once per cell (#113): the
/// books count it as one cell of area. The rest, `(1 − 1/k) n × amount`, is the erosion of the
/// subducted slab area the extra cells stand for; it is taken in equal parts from the sample's
/// neighbours on its plate (the adjacent forearc), so the erosion per subducted sample stays
/// `amount` and in balance with arc growth. Every sample is thinned once with the sum of what it
/// gets, in sample order. Returns nothing; the ledger and cover budget book what was removed.
#[allow(clippy::too_many_arguments)]
fn erode_overriding_samples(
    mesh: &SphereMesh,
    plates: &mut [Plate],
    owners: &[Ownership],
    eroded: &[(u32, u32)],
    amount: f64,
    ledger: &mut MassLedger,
    cover_budget: &mut CoverBudget,
    rates: &mut SharedSampleRates,
) {
    if eroded.is_empty() {
        return;
    }
    // Per eroded sample, in the order of its first event: (sample, events, cells showing it).
    let mut slots: HashMap<Ownership, usize> = HashMap::default();
    let mut samples: Vec<(Ownership, u32, u32)> = Vec::new();
    for &(cell, plate) in eroded {
        let shown = owners[cell as usize];
        // Matching changes which sample of its plate a cell shows, never the plate.
        debug_assert_eq!(shown.plate, plate);
        if shown.plate != plate {
            continue;
        }
        let slot = *slots.entry(shown).or_insert_with(|| {
            samples.push((shown, 0, 0));
            samples.len() - 1
        });
        samples[slot].1 += 1;
    }
    for owner in owners {
        if let Some(&slot) = slots.get(owner) {
            samples[slot].2 += 1;
        }
    }
    // Thickness to erode per sample.
    let mut thinning: HashMap<Ownership, f64> = HashMap::default();
    let mut neighbours: Vec<Ownership> = Vec::new();
    for &(sample, events, cells) in &samples {
        // Every event's cell shows the sample, so cells ≥ 1.
        let cells = f64::from(cells.max(1));
        let events = f64::from(events);
        rates.erosion_events += events;
        rates.erosion_per_sample += events / cells;
        *thinning.entry(sample).or_default() += events / cells * amount;
        let rest = events * (1.0 - 1.0 / cells) * amount;
        if rest <= 0.0 {
            continue;
        }
        let plate = &plates[sample.plate as usize];
        neighbours.clear();
        neighbours.extend(
            mesh.neighbors(sample.local)
                .iter()
                .filter(|&&local| plate.crust[local as usize].is_some())
                .map(|&local| Ownership {
                    plate: sample.plate,
                    local,
                }),
        );
        if neighbours.is_empty() {
            // No forearc around it: the sample takes it all, as one cell would.
            *thinning.entry(sample).or_default() += rest;
            continue;
        }
        rates.erosion_to_neighbours += rest / amount;
        let share = rest / neighbours.len() as f64;
        for &neighbour in &neighbours {
            *thinning.entry(neighbour).or_default() += share;
        }
    }
    let mut thinning: Vec<(Ownership, f64)> = thinning.into_iter().collect();
    thinning.sort_unstable_by_key(|&(sample, _)| (sample.plate, sample.local));
    for (sample, mut amount) in thinning {
        let Some(crust) = &mut plates[sample.plate as usize].crust[sample.local as usize] else {
            continue;
        };
        let before = crust.mass();
        let column = *crust;
        let from_sediment = amount.min(crust.sediment);
        crust.remove_sediment_to(crust.sediment - from_sediment);
        amount -= from_sediment;
        crust.thin_from_below(
            (crust.thickness - amount).max(crust.thickness.min(OCEANIC_THICKNESS)),
        );
        ledger.record(Term::SubductionErosion, crust.mass() - before);
        cover_budget.record_change(CoverTerm::SubductionErosion, &column, crust);
    }
}

impl Simulation {
    /// Orogenic collapse: the weak lower crust of thick, hot crust flows down the gradient of
    /// surface elevation (lower-crustal channel flow; Clark & Royden 2000, Bird 1991), spreading
    /// orogens over their surroundings at constant mass. See [`Self::crustal_flow_inner`].
    pub(super) fn crustal_flow(&mut self, dt: f64) {
        let change = self.crustal_flow_inner(dt);
        self.ledger.record(Term::CrustalFlow, change);
    }

    /// Moves crystalline crust between neighbouring cells, driven by the difference of their dry
    /// elevations: a thickness diffusion with [`TectonicParams::collapse_diffusivity`] times a
    /// mobility that grows with the cube of the source's thickness above
    /// [`TectonicParams::crustal_flow_threshold`] (full over
    /// [`TectonicParams::crustal_flow_range`]). Receivers are crust of continental origin only
    /// (never oceanic crust, which would carry it into a trench), on another plate only where
    /// the plates do not converge. Explicit sub-cycles keep it stable; flows are summed per
    /// sample (a sample may show at several cells) and each pair moves at most what equalises
    /// it. A receiver takes at most its room below [`TectonicParams::max_crust_thickness`]
    /// (its incoming transfers are scaled down together), so no crust is lost at the cap.
    /// One-cell holes (see [`TectonicParams::hole_flow_depth`]) take crust from every neighbour
    /// of their plate with crust of continental origin at full mobility, down to half of the
    /// neighbour's thickness per step, until they are no longer holes (decided once per step).
    ///
    /// A sample shown at k cells has the edges of all k cells, but the books count it as one
    /// cell of area. Each transfer between two cells is therefore divided by the larger number
    /// of cells showing its source or its receiver sample, so a duplicated sample exchanges the
    /// mean of its cells' exchanges, once, not their sum (#113, like the surface changes of
    /// #57). The partner takes or gives exactly the same, so mass stays conserved locally.
    /// Returns the change of mass (zero up to rounding).
    fn crustal_flow_inner(&mut self, dt: f64) -> f64 {
        let params = &self.params;
        let cell_count = self.mesh.len();
        let edge = params.planet_radius * self.mesh.mean_edge_angle();
        let diffusivity = params.collapse_diffusivity * 1e6;
        let ratio = diffusivity * dt / (edge * edge);
        let cycles = (ratio / 0.2).ceil().max(1.0) as usize;
        let dt_cycle = dt / cycles as f64;
        // Laplacian weight on the hexagonal mesh: 2/(3 d²) per neighbour.
        let weight = diffusivity * dt_cycle * 2.0 / (3.0 * edge * edge);
        let min_convergence = params.min_trench_convergence / (params.planet_radius * 100.0 / 1e6);
        let mut change = 0.0;
        // Per crust sample (dense index): total requested outflow, and the flows applied.
        let mut outflow = vec![0.0; self.plates.len() * cell_count];
        // Per crust sample: total requested inflow.
        let mut inflow = vec![0.0; self.plates.len() * cell_count];
        // Per crust sample: (outgoing thickness, incoming thickness, incoming mass, incoming
        // felsic mass).
        let mut flows = vec![(0.0, 0.0, 0.0, 0.0, 0.0); self.plates.len() * cell_count];
        // (source sample, receiver sample, thickness, density, (felsic and metasediment
        // fractions, the source's thickness it keeps))
        #[allow(clippy::type_complexity)]
        let mut transfers: Vec<(usize, usize, f64, f64, (f64, f64, f64))> = Vec::new();
        let mut touched: Vec<usize> = Vec::new();
        let holes = self.one_cell_holes();
        // Per crust sample: the number of cells showing it.
        let mut shown = vec![0u32; self.plates.len() * cell_count];
        for &owner in &self.owners {
            shown[sample_index(owner, cell_count)] += 1;
        }
        let (mut per_cell, mut per_sample) = (0.0, 0.0);
        for _ in 0..cycles {
            for cell in 0..cell_count as u32 {
                let owner = self.owners[cell as usize];
                let source = self.surface_crust(cell);
                if !source.is_continental_origin_column() {
                    continue;
                }
                let excess = (source.thickness - params.crustal_flow_threshold).max(0.0);
                let feeds_hole =
                    self.mesh.neighbors(cell).iter().any(|&n| {
                        holes[n as usize] && self.owners[n as usize].plate == owner.plate
                    });
                if excess <= 0.0 && !feeds_hole {
                    continue;
                }
                let mobility = (excess / params.crustal_flow_range).min(1.0).powi(3);
                let source_elevation = source.dry_elevation(self.time);
                let response = crate::isostasy::dry_response(source.density);
                let r = self.mesh.vertex(cell);
                for &neighbor in self.mesh.neighbors(cell) {
                    let neighbor_owner = self.owners[neighbor as usize];
                    if neighbor_owner == owner {
                        continue;
                    }
                    let hole = holes[neighbor as usize] && neighbor_owner.plate == owner.plate;
                    if !hole && excess <= 0.0 {
                        continue;
                    }
                    let receiver = self.surface_crust(neighbor);
                    if !hole && !receiver.is_continental_origin_column() {
                        continue;
                    }
                    let drop = source_elevation - receiver.dry_elevation(self.time);
                    if drop <= 0.0 {
                        continue;
                    }
                    if neighbor_owner.plate != owner.plate {
                        // Not across converging boundaries: crust moved onto the lower plate
                        // would be consumed and stacked back.
                        let normal = (self.mesh.vertex(neighbor) - r).normalize_or_zero();
                        let convergence = (self.plates[owner.plate as usize].angular_velocity
                            - self.plates[neighbor_owner.plate as usize].angular_velocity)
                            .cross(r)
                            .dot(normal);
                        if convergence > -min_convergence {
                            continue;
                        }
                    }
                    let equalising =
                        drop / (response + crate::isostasy::dry_response(receiver.density));
                    // Into a hole, a source gives down to half its thickness, also below the
                    // flow threshold (its other transfers are scaled with the same outflow).
                    let (mobility, floor) = if hole {
                        (1.0, 0.5 * source.thickness)
                    } else {
                        (mobility, params.crustal_flow_threshold)
                    };
                    let from = sample_index(owner, cell_count);
                    let to = sample_index(neighbor_owner, cell_count);
                    let amount = (weight * mobility * drop / response).min(0.5 * equalising);
                    per_cell += amount * source.density;
                    let amount = amount / f64::from(shown[from].max(shown[to]));
                    per_sample += amount * source.density;
                    outflow[from] += amount;
                    inflow[to] += amount;
                    transfers.push((
                        from,
                        to,
                        amount,
                        source.density,
                        (source.felsic, source.metasediment, floor),
                    ));
                }
            }
            // A sample (possibly shown at several cells) gives at most its crust above the
            // threshold and takes at most its room below the cap (ignoring its own outflow);
            // its transfers are scaled down together.
            let thickness = |index: usize| {
                self.plates[index / cell_count].crust[index % cell_count]
                    .map_or(0.0, |c| c.thickness)
            };
            for &(from, to, amount, density, (felsic, metasediment, floor)) in &transfers {
                let available = (thickness(from) - floor).max(0.0);
                let room = (params.max_crust_thickness - thickness(to)).max(0.0);
                let scaled =
                    amount * (available / outflow[from]).min(1.0) * (room / inflow[to]).min(1.0);
                for index in [from, to] {
                    if flows[index] == (0.0, 0.0, 0.0, 0.0, 0.0) {
                        touched.push(index);
                    }
                }
                flows[from].0 += scaled;
                let flow = &mut flows[to];
                flow.1 += scaled;
                flow.2 += scaled * density;
                flow.3 += scaled * density * felsic;
                flow.4 += scaled * density * metasediment;
            }
            for &(from, to, ..) in &transfers {
                outflow[from] = 0.0;
                inflow[to] = 0.0;
            }
            transfers.clear();
            touched.sort_unstable();
            touched.dedup();
            for &index in &touched {
                let (plate, local) = (index / cell_count, index % cell_count);
                let (outgoing, incoming, incoming_mass, incoming_felsic, incoming_metasediment) =
                    flows[index];
                flows[index] = (0.0, 0.0, 0.0, 0.0, 0.0);
                let crust = self.plates[plate].crust[local]
                    .as_mut()
                    .expect("flows only between existing samples");
                let before = crust.mass();
                // Outgoing crust leaves density and felsic fraction unchanged.
                crust.thickness -= outgoing;
                if incoming > 0.0 {
                    crust.add_crust(
                        incoming,
                        incoming_mass / incoming,
                        (
                            incoming_felsic / incoming_mass.max(1e-9),
                            incoming_metasediment / incoming_mass.max(1e-9),
                        ),
                        params.max_crust_thickness,
                    );
                }
                change += crust.mass() - before;
            }
            touched.clear();
        }
        self.shared_rates.flow_per_cell += per_cell;
        self.shared_rates.flow_per_sample += per_sample;
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
            trench_sediment,
            cover_budget,
            shared_rates,
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
            // Where the segment's overriding plate lost the cell because buoyant crust of the
            // lower plate arrived and stays on top (see `Polarities::contact`), the upper
            // plate's arc crust is not subducted below it: it is accreted onto the arriving
            // crust (arc–continent collision: Taiwan, Banda, Kohistan) until the segment flips.
            // Normal oceanic crust of the upper plate still subducts (obduction of ophiolites is
            // minor).
            let overridden_upper_plate = !loser_crust.resists_subduction()
                && !loser_crust.is_oceanic()
                && polarities
                    .segment_at(
                        winner.plate,
                        contact.loser.plate,
                        mesh.vertex(contact.cell),
                        &rotations,
                    )
                    .is_some_and(|segment| segment.overriding == contact.loser.plate);
            // At a diffuse convergent zone (no through-going thrust yet, see
            // `polarity::Segment::localised`) the lower plate does not subduct: the convergence
            // is taken up by distributed shortening, which thickens the crust of the zone
            // (Central Indian Basin). Nothing goes into the mantle, and there is no arc.
            // This holds for all crust overlapped there, whichever side it comes from, where the
            // zone converges by the last velocity solve (see `polarity::Segment::subducting`).
            // Overlaps where it does not (slow or oblique zones, resampling noise along
            // transforms, a segment the solve has not seen yet) are handled as everywhere else;
            // stacking them thickened oceanic crust to 40–60 km in a first version.
            let diffuse = !loser_crust.resists_subduction()
                && !overridden_upper_plate
                && polarities
                    .segment_at_mut(
                        winner.plate,
                        contact.loser.plate,
                        mesh.vertex(contact.cell),
                        &rotations,
                    )
                    .filter(|segment| !segment.localised && segment.subducting)
                    .map(|segment| segment.stacked += loser_crust.mass())
                    .is_some();
            if overridden_upper_plate {
                trench_sediment.upper_plate_accreted += loser_crust.mass();
            }
            if loser_crust.resists_subduction() || overridden_upper_plate {
                trench_sediment.stacked += sediment_mass;
            }
            if diffuse {
                // The crystalline crust is stacked with its own composition, its sediment moves
                // along as sediment.
                ledger.record(Term::ShorteningConsumed, -loser_crust.mass());
                // It is the oceanic lithosphere that shortens: where the overriding crust is
                // continental, the crust is stacked onto the oceanic side of the zone (next to
                // the contact), not onto the continental margin.
                let continental_winner = plates[winner.plate as usize].crust[winner.local as usize]
                    .is_some_and(|crust| crust.is_continental_origin());
                let oceanic_side = continental_winner
                    .then(|| {
                        mesh.neighbors(contact.cell)
                            .iter()
                            .copied()
                            .find(|&neighbor| {
                                new_owners[neighbor as usize]
                                    .is_some_and(|owner| owner.plate == contact.loser.plate)
                            })
                    })
                    .flatten();
                let (cell, plate, partner) = match oceanic_side {
                    Some(neighbor) => (neighbor, contact.loser.plate, winner.plate),
                    None => (contact.cell, winner.plate, contact.loser.plate),
                };
                additions.push(CrustAddition {
                    cell,
                    plate,
                    partner,
                    thickness: loser_crust.thickness,
                    density: loser_crust.density,
                    felsic: loser_crust.felsic,
                    metasediment: loser_crust.metasediment,
                    kind: AdditionKind::Shortening,
                    sediment: loser_crust.sediment,
                    cohort: loser_crust.cohort,
                });
            } else if loser_crust.resists_subduction() || overridden_upper_plate {
                // Collision or terrane accretion: buoyant crust of the lower plate (continents,
                // thick arcs, plateaus) is stacked onto the overriding plate instead of
                // subducting, and so is the upper plate's crust overridden by it; sediment
                // becomes metasediment at the same mass.
                *collisions
                    .entry(pair_key(winner.plate, contact.loser.plate))
                    .or_default() += 1;
                ledger.record(Term::CollisionConsumed, -loser_crust.mass());
                cover_budget.record_removal(
                    CoverTerm::Collision,
                    &loser_crust,
                    0.0,
                    loser_crust.sediment,
                );
                let thickness = loser_crust.thickness + sediment_mass / METASEDIMENT_DENSITY;
                additions.push(CrustAddition {
                    cell: contact.cell,
                    plate: winner.plate,
                    partner: contact.loser.plate,
                    thickness,
                    density: loser_crust.mass() / thickness.max(1e-9),
                    // Sediment is eroded continental (felsic) material.
                    felsic: (loser_crust.felsic_mass() + sediment_mass)
                        / loser_crust.mass().max(1e-9),
                    // Stacked into an orogen, it becomes part of the continental basement.
                    metasediment: 0.0,
                    kind: AdditionKind::Collision,
                    sediment: 0.0,
                    cohort: Cohort::NONE,
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
                // Overlaps at diffuse zones that the last velocity solve did not see converging
                // (new segments, resampling noise) subduct as elsewhere; counted.
                if polarities
                    .segment_at(
                        winner.plate,
                        contact.loser.plate,
                        mesh.vertex(contact.cell),
                        &rotations,
                    )
                    .is_some_and(|segment| !segment.localised)
                {
                    polarities.statistics.diffuse_subducted += crust_mass + sediment_mass;
                }
                // Rock-equivalent thickness of the sediment and metasediment column.
                let column =
                    loser_crust.sediment + loser_crust.metasediment_mass() / SEDIMENT_ROCK_DENSITY;
                let upper_is_oceanic = plates[winner.plate as usize].crust[winner.local as usize]
                    .is_some_and(|crust| crust.is_oceanic_column());
                additions.push(CrustAddition {
                    cell: contact.cell,
                    plate: winner.plate,
                    partner: contact.loser.plate,
                    thickness: params.arc_crust_per_subduction,
                    density: ARC_DENSITY,
                    felsic: if upper_is_oceanic {
                        params.oceanic_arc_felsic
                    } else {
                        1.0
                    },
                    metasediment: 0.0,
                    kind: AdditionKind::Arc,
                    sediment: 0.0,
                    cohort: Cohort::NONE,
                });
                // At accretionary margins (thick trench fill, slow convergence; see
                // `Simulation::update_slab_lengths`) the sediment above a décollement is scraped
                // off into an accretionary wedge on the overriding plate, which becomes crust
                // (metasediment) at the same mass; the subduction channel and the rest are dragged
                // down. At erosive margins all sediment subducts and the forearc is eroded.
                let accretionary = polarities
                    .segment_at_mut(
                        winner.plate,
                        contact.loser.plate,
                        mesh.vertex(contact.cell),
                        &rotations,
                    )
                    .is_some_and(|segment| {
                        // The trench fill: sediment and the metasediment below it.
                        let fill = column;
                        segment.incoming.0 += fill;
                        segment.incoming.1 += fill * fill;
                        segment.accretionary
                    });
                // The décollement cuts through the column of sediment and metasediment
                // (frontal accretion and basal underplating: Makran, Kodiak, Shimanto); the
                // part above it is scraped off, sediment first.
                let accreted_column = if accretionary {
                    (column * params.decollement_fraction)
                        .min(column - params.subduction_channel_thickness)
                        .max(0.0)
                } else {
                    0.0
                };
                let accreted_sediment = accreted_column.min(loser_crust.sediment);
                // The décollement scrapes off the top of the column; the rest is subducted.
                cover_budget.record_removal(
                    CoverTerm::Accreted,
                    &loser_crust,
                    0.0,
                    accreted_sediment,
                );
                cover_budget.record_removal(
                    CoverTerm::Subducted,
                    &loser_crust,
                    accreted_sediment,
                    loser_crust.sediment,
                );
                let accreted_mass = accreted_column * SEDIMENT_ROCK_DENSITY;
                trench_sediment.record(
                    &loser_crust,
                    accreted_sediment * SEDIMENT_ROCK_DENSITY,
                    accretionary,
                );
                trench_sediment.metasediment_accreted +=
                    (accreted_column - accreted_sediment) * SEDIMENT_ROCK_DENSITY;
                trench_sediment.metasediment_subducted += loser_crust.metasediment_mass()
                    - (accreted_column - accreted_sediment) * SEDIMENT_ROCK_DENSITY;
                if accreted_mass > 0.0 {
                    // Booked as accretion when added; all sediment left with the slab above.
                    additions.push(CrustAddition {
                        cell: contact.cell,
                        plate: winner.plate,
                        partner: contact.loser.plate,
                        thickness: accreted_mass / METASEDIMENT_DENSITY,
                        density: METASEDIMENT_DENSITY,
                        felsic: 1.0,
                        metasediment: 0.0,
                        kind: AdditionKind::Accretion,
                        sediment: 0.0,
                        cohort: Cohort::NONE,
                    });
                }
                if !accretionary {
                    eroded.push((contact.cell, winner.plate));
                }
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
        let min_stretching_opening =
            params.min_stretching_rate / (params.planet_radius * 100.0 / 1e6);
        let rift_rings = (params.rift_width / (params.planet_radius * mesh.mean_edge_angle()))
            .ceil()
            .max(1.0) as usize;
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
                let local = plates[plate_index as usize].local_vertex(
                    mesh,
                    inverse_rotations[plate_index as usize],
                    neighbor,
                );
                // Continental rifting: where crust of continental origin thicker than the breakup
                // thickness opens (also slowly), the rift zone is stretched into the gap instead
                // of forming sea floor. This includes slots whose crust was consumed at an earlier
                // convergent contact: rifts often reopen old sutures and subduction edges
                // (stretching is mass-conserving, so it cannot bring consumed crust back).
                let rifting = {
                    let plate = &plates[plate_index as usize];
                    let parent_crust =
                        plate.crust[parent.local as usize].expect("owners point to crust");
                    parent_crust.is_continental_origin_column()
                        && parent_crust.thickness > params.breakup_thickness
                        && plate.crust[local as usize].is_none()
                        && (spreading
                            || opens_away(
                                &mut rings,
                                mesh,
                                &new_owners,
                                &angular_velocities,
                                neighbor,
                                plate_index,
                                min_stretching_opening,
                            ))
                };
                let plate = &mut plates[plate_index as usize];
                if rifting
                    && stretch_rift_marking(
                        mesh,
                        plate,
                        local,
                        rift_rings,
                        params.breakup_thickness,
                        time,
                        params.rift_fabric,
                    )
                {
                    gap_statistics.stretched_cells += 1;
                    plate.consumed[local as usize] = false;
                    new_owners[neighbor as usize] = Some(Ownership {
                        plate: plate_index,
                        local,
                    });
                } else if spreading {
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
                    } else if plate.crust[local as usize].is_none()
                        && mesh
                            .neighbors(local)
                            .iter()
                            .filter(|&&n| plate.crust[n as usize].is_some())
                            .count()
                            >= MIN_FILL_NEIGHBOURS
                    {
                        // Away from other plates, a spot inside the plate's crust is a
                        // resampling gap like any other and is healed from its neighbours (new
                        // sea floor there made one-cell holes in continents, #129).
                        new_owners[neighbor as usize] =
                            Some(heal_gap(mesh, plate, parent, local, true, gap_statistics));
                    } else {
                        // Away from other plates, uncovered area without crust around it in the
                        // plate is open sea floor.
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
        // Crust placed in gaps starts with a fresh front and viscosity of the deforming plates
        // (see `sheet`); its velocity follows at the next solve.
        for &(plate, cell) in &filled {
            let owner = owners[cell as usize];
            if owner.plate == plate {
                let sheet = &mut plates[plate as usize].sheet;
                sheet.front[owner.local as usize] = 1.0;
                sheet.viscosity[owner.local as usize] = f32::NAN;
            }
        }
        matching::update_crust_cells(mesh, plates, found, &filled, matching_scratch);
        if params.matching.distinct {
            matching::show_distinct_samples(
                mesh,
                plates,
                owners,
                time,
                &params.matching,
                matching_scratch,
                matching_statistics,
            );
        }
        // Subduction erosion: the slab scrapes material off the underside and front of the
        // overriding plate and carries it into the mantle, sediment first. On Earth this and
        // sediment subduction recycle about as much crust as arcs produce. It works on the
        // samples the cells finally show (see `erode_overriding_samples`).
        erode_overriding_samples(
            mesh,
            plates,
            owners,
            &eroded,
            params.subduction_erosion,
            ledger,
            cover_budget,
            shared_rates,
        );
        ResolvedBoundaries {
            additions,
            collisions,
        }
    }

    /// The one-cell holes (see [`TectonicParams::hole_flow_depth`]): per cell, whether its dry
    /// elevation is more than the hole depth below all its neighbours' and its crust thinner
    /// than all of theirs.
    pub(super) fn one_cell_holes(&self) -> Vec<bool> {
        let cell_count = self.mesh.len();
        let depth = self.params.hole_flow_depth;
        if depth <= 0.0 {
            return vec![false; cell_count];
        }
        let crusts: Vec<Crust> = (0..cell_count as u32)
            .map(|cell| self.surface_crust(cell))
            .collect();
        let elevations: Vec<f64> = crusts
            .iter()
            .map(|crust| crust.dry_elevation(self.time))
            .collect();
        (0..cell_count as u32)
            .map(|cell| {
                let (elevation, thickness) =
                    (elevations[cell as usize], crusts[cell as usize].thickness);
                self.mesh.neighbors(cell).iter().all(|&n| {
                    elevations[n as usize] > elevation + depth
                        && crusts[n as usize].thickness > thickness
                })
            })
            .collect()
    }

    /// Distributes new crust over the cells of the receiving plate around each addition,
    /// following the arc or collision profile. Crust that does not fit below the maximum
    /// thickness spills further outwards over up to [`SPILL_RINGS`] more rings, and is only
    /// removed (delamination) when there is no room left there either.
    pub(super) fn apply_additions(&mut self, additions: &[CrustAddition]) {
        let mut nominal = 0.0;
        for addition in additions {
            let mass =
                addition.thickness * addition.density + addition.sediment * SEDIMENT_ROCK_DENSITY;
            nominal += mass;
            let term = match addition.kind {
                AdditionKind::Arc => Term::Arcs,
                AdditionKind::Accretion => Term::Accretion,
                AdditionKind::Collision => Term::CollisionStacked,
                AdditionKind::Shortening => Term::ShorteningStacked,
            };
            self.ledger.record(term, mass);
        }
        let actual = self.apply_additions_inner(additions);
        self.ledger.record(Term::AdditionsLost, actual - nominal);
    }

    /// Whether `cell` borders the partner plate of `addition` along a segment where the
    /// partner overrides.
    fn borders_reversed_segment(
        &self,
        cell: u32,
        addition: &CrustAddition,
        rotations: &[DQuat],
    ) -> bool {
        self.mesh
            .neighbors(cell)
            .iter()
            .any(|&neighbor| self.owners[neighbor as usize].plate == addition.partner)
            && self
                .polarities
                .segment_at(
                    addition.plate,
                    addition.partner,
                    self.mesh.vertex(cell),
                    rotations,
                )
                .is_some_and(|segment| segment.overriding == addition.partner)
    }

    /// Returns the change of mass.
    fn apply_additions_inner(&mut self, additions: &[CrustAddition]) -> f64 {
        let rotations = self.rotations();
        let mut change = 0.0;
        let mut visited_in_event = vec![usize::MAX; self.mesh.len()];
        let mut frontier = Vec::new();
        let mut next_frontier = Vec::new();
        // Reached crust samples with their ring and profile weight (0 beyond the profile).
        let mut reached: Vec<(Ownership, usize, f64)> = Vec::new();
        let mut weights: Vec<f64> = Vec::new();
        let mut first_seen: Vec<bool> = Vec::new();
        let mut seen: HashSet<Ownership> = HashSet::default();
        let max_thickness = self.params.max_crust_thickness;
        let band_rings = deformation::band_rings(
            self.params.collision_band_width,
            self.params.planet_radius * self.mesh.mean_edge_angle(),
        );

        for (event_index, addition) in additions.iter().enumerate() {
            let profile = match addition.kind {
                AdditionKind::Collision | AdditionKind::Shortening => {
                    &self.params.collision_profile
                }
                AdditionKind::Arc => &self.params.arc_profile,
                AdditionKind::Accretion => &self.params.accretion_profile,
            };
            // Continental collisions are taken up by a band of the overriding plate (see
            // `deformation`), the other additions follow their profile. The band is a disc of
            // rings around the contact on the overriding plate (also across its oceanic cells,
            // which take no share); with a band, the reach for the spill and for the profile
            // fallback is the band's. Additions are applied in order, each seeing the state left
            // by the previous ones (including orogenic marks, which make cells softer).
            let band = addition.kind == AdditionKind::Collision && band_rings > 0;
            let rings = if band {
                (band_rings + 1).max(profile.len())
            } else {
                profile.len()
            };
            frontier.clear();
            frontier.push(addition.cell);
            visited_in_event[addition.cell as usize] = event_index;
            reached.clear();

            for ring in 0..rings + SPILL_RINGS {
                let weight = profile.get(ring).copied().unwrap_or(0.0);
                next_frontier.clear();
                for &cell in &frontier {
                    let owner = self.owners[cell as usize];
                    if owner.plate != addition.plate {
                        continue;
                    }
                    // Arc magmas rise only behind the trench of their own slab, not across a
                    // neighbouring segment of the pair where the partner plate overrides.
                    let weight = if addition.kind == AdditionKind::Arc
                        && self.borders_reversed_segment(cell, addition, &rotations)
                    {
                        0.0
                    } else {
                        weight
                    };
                    reached.push((owner, ring, weight));
                    for &neighbor in self.mesh.neighbors(cell) {
                        if visited_in_event[neighbor as usize] != event_index {
                            visited_in_event[neighbor as usize] = event_index;
                            next_frontier.push(neighbor);
                        }
                    }
                }
                std::mem::swap(&mut frontier, &mut next_frontier);
            }

            // A sample shown at several cells takes part once, at its nearest ring (`reached`
            // is in ring order), in a band and in a profile alike (#57): otherwise each of its
            // cells would show the crust added at all of them.
            first_seen.clear();
            for &(owner, ..) in &reached {
                first_seen.push(seen.insert(owner));
            }
            seen.clear();
            let time = self.time;
            let crust_at = |plates: &[Plate], owner: Ownership| {
                plates[owner.plate as usize].crust[owner.local as usize]
                    .expect("owners always point to existing crust")
            };
            // Band weights from the current state of the samples; `None` if no sample of the
            // band can take crust (no continental crust within reach, or all of it at the
            // plateau thickness): then the profile is used.
            let band_weights = |plates: &[Plate], weights: &mut Vec<f64>| {
                weights.clear();
                weights.extend(reached.iter().zip(&first_seen).map(
                    |(&(owner, ring, _), &first)| {
                        if first {
                            deformation::band_weight(
                                &crust_at(plates, owner),
                                time,
                                ring,
                                band_rings,
                            )
                        } else {
                            0.0
                        }
                    },
                ));
                weights.iter().sum::<f64>()
            };
            let use_band = band && band_weights(&self.plates, &mut weights) > 0.0;
            if !use_band {
                weights.clear();
                weights.extend(
                    reached
                        .iter()
                        .zip(&first_seen)
                        .map(|(&(_, _, weight), &first)| if first { weight } else { 0.0 }),
                );
            }
            let total_weight: f64 = weights.iter().sum();
            if total_weight <= 0.0 {
                continue;
            }
            // Distribute by the weights; in a band, what did not fit is distributed again by
            // the weights of the samples that still have room (a few passes), then whatever is
            // left spills outwards, nearest first.
            let mut remaining = addition.thickness;
            let passes = if use_band { BAND_PASSES } else { 1 };
            for pass in 0..passes {
                let total_weight = if pass == 0 {
                    total_weight
                } else {
                    band_weights(&self.plates, &mut weights)
                };
                if total_weight <= 0.0 || remaining <= addition.thickness * 1e-9 {
                    break;
                }
                let mut leftover = 0.0;
                for (&(owner, _, _), &weight) in reached.iter().zip(&weights) {
                    if weight <= 0.0 {
                        continue;
                    }
                    let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                        .as_mut()
                        .expect("owners always point to existing crust");
                    let share = remaining * weight / total_weight;
                    let fitting = share.min((max_thickness - crust.thickness).max(0.0));
                    let before = crust.mass();
                    crust.add_crust(
                        fitting,
                        addition.density,
                        (addition.felsic, addition.metasediment),
                        max_thickness,
                    );
                    // Sediment moved along goes onto the same cells, without a thickness cap
                    // (collisions carry none: their sediment is stacked as metasediment).
                    if pass == 0 {
                        let sediment = addition.sediment * weight / total_weight;
                        if sediment > 0.0 {
                            crust.add_sediment(sediment, addition.cohort);
                        }
                    }
                    change += crust.mass() - before;
                    leftover += share - fitting;
                    // Crust thickened by a continental collision is an orogenic belt. In a
                    // band, only where the thickening is material (see
                    // `MIN_OROGENIC_STRAIN`): a few metres from a distant contact do not deform
                    // the lithosphere of a whole cell.
                    if addition.kind == AdditionKind::Collision
                        && fitting > 0.0
                        && crust.is_continental_origin()
                        && crust.thickness > OROGENIC_THICKNESS
                        && (!use_band || fitting >= MIN_OROGENIC_STRAIN * crust.thickness)
                    {
                        crust.mark_orogeny(time);
                    }
                }
                remaining = leftover;
            }
            for &(owner, _, _) in &reached {
                if remaining <= 0.0 {
                    break;
                }
                let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                    .as_mut()
                    .expect("owners always point to existing crust");
                let fitting = remaining.min((max_thickness - crust.thickness).max(0.0));
                let before = crust.mass();
                crust.add_crust(
                    fitting,
                    addition.density,
                    (addition.felsic, addition.metasediment),
                    max_thickness,
                );
                change += crust.mass() - before;
                remaining -= fitting;
            }
        }
        change
    }
}

/// Passes of the weighted distribution of a collision over its band before the rest spills.
const BAND_PASSES: usize = 3;

/// Smallest thickening, relative to the column, that marks crust in a collision band as an
/// orogenic belt (see [`Crust::mark_orogeny`]).
const MIN_OROGENIC_STRAIN: f64 = 0.01;

pub(super) struct ResolvedBoundaries {
    pub(super) additions: Vec<CrustAddition>,
    /// Number of continental collision events per plate pair.
    pub(super) collisions: HashMap<(u32, u32), u32>,
}

/// Continental crust thicker than this (m) after receiving crust stacked in a collision counts as
/// an orogenic belt (see [`Crust::orogeny`]): normal continental crust is ~35 km thick.
const OROGENIC_THICKNESS: f64 = 35e3;

/// A crust sample overridden by another plate at a cell.
struct LostContact {
    cell: u32,
    winner: Ownership,
    loser: Ownership,
    loser_crust: Crust,
}

/// Sediment arriving at trenches so far, in kg/m² summed over subducted samples (multiply by the
/// cell area for kg).
#[derive(Clone, Debug, Default)]
pub struct TrenchSediment {
    /// By the rock-equivalent thickness of the arriving column: ≤ 1 km, 1–3, 3–5, 5–8, > 8 km.
    pub by_thickness: [f64; 5],
    /// Part arriving on thinned continental crust (felsic fraction > 0.2).
    pub on_continental_margins: f64,
    /// Part accreted to the overriding plate.
    pub accreted: f64,
    /// Part arriving at accretionary margins.
    pub at_accretionary_margins: f64,
    /// Crust and sediment of upper plates overridden by buoyant arriving crust and accreted to
    /// it instead of subducting (not sediment arriving at a trench; kept here as a trench
    /// statistic).
    pub upper_plate_accreted: f64,
    /// Sediment of buoyant crust stacked onto the overriding plate in collisions and terrane
    /// accretion, which becomes metasediment of the orogen.
    pub stacked: f64,
    /// Metasediment (see [`Crust::metasediment`]) arriving at trenches: scraped off into the
    /// accretionary wedge, and subducted.
    pub metasediment_accreted: f64,
    pub metasediment_subducted: f64,
}

impl TrenchSediment {
    fn record(&mut self, crust: &Crust, accreted_mass: f64, accretionary: bool) {
        let mass = crust.sediment * SEDIMENT_ROCK_DENSITY;
        let bin = [1000.0, 3000.0, 5000.0, 8000.0]
            .iter()
            .filter(|&&limit| crust.sediment > limit)
            .count();
        self.by_thickness[bin] += mass;
        if crust.felsic > 0.2 {
            self.on_continental_margins += mass;
        }
        self.accreted += accreted_mass;
        if accretionary {
            self.at_accretionary_margins += mass;
        }
    }
}

/// Counts of how uncovered cells were filled, accumulated over the simulation.
#[derive(Clone, Debug, Default)]
pub struct GapStatistics {
    /// Cells filled with new oceanic crust where plates spread apart.
    pub new_crust_cells: usize,
    /// Gaps filled by stretching continental crust of the rift zone.
    pub stretched_cells: usize,
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

/// Fills the empty local vertex `local` of `plate` at an opening rift by stretching the
/// plate's crust of continental origin within `rings` rings (its frame) that is thicker than
/// `breakup_thickness`. The N donors give N/(N + 1) of one donor's crust and sediment in total
/// (uniform stretching would give each 1/(N + 1)), weighted by 1/k for a donor in ring k (times
/// (h/35 km)² for thickened donors: extensional collapse, see `deformation`; thick donors
/// give larger fractions of larger columns, so in thickened crust the new sample is thicker
/// and breakup comes later: wide rifts), but
/// none thins below `breakup_thickness`: thinning is strongest next to the rift, so margins
/// taper towards the ocean. Mass is conserved. Stretching heats the lithosphere (McKenzie
/// 1978): crust stretched by β whose thermal uplift is the fraction U of a ridge's gets
/// U' = 1 − (1 − U)/β, after which it subsides with the GDH1 curve; the new sample has its
/// donors' mean state, further stretched by their mean thickness over its own (if thinner).
/// Returns false if there is no donor or the donors cannot fill the vertex with at least
/// `breakup_thickness` of crust: the continent breaks up and the gap becomes sea floor (no
/// felsic slivers thin enough to be subducted).
#[cfg(test)]
fn stretch_rift(
    mesh: &SphereMesh,
    plate: &mut Plate,
    local: u32,
    rings: usize,
    breakup_thickness: f64,
    time: f64,
) -> bool {
    stretch_rift_marking(mesh, plate, local, rings, breakup_thickness, time, 1.0)
}

/// `stretch_rift`, marking the extension fabric ([`Crust::rifted`]) with the factor `fabric`
/// ([`TectonicParams::rift_fabric`]).
fn stretch_rift_marking(
    mesh: &SphereMesh,
    plate: &mut Plate,
    local: u32,
    rings: usize,
    breakup_thickness: f64,
    time: f64,
    fabric: f64,
) -> bool {
    // Donors with their ring, in ring order (deterministic).
    let mut donors = Vec::new();
    let mut seen = vec![local];
    let mut frontier = vec![local];
    for ring in 1..=rings {
        let mut next = Vec::new();
        for &vertex in &frontier {
            for &neighbor in mesh.neighbors(vertex) {
                if seen.contains(&neighbor) {
                    continue;
                }
                seen.push(neighbor);
                next.push(neighbor);
                if plate.crust[neighbor as usize].is_some_and(|crust| {
                    crust.is_continental_origin_column() && crust.thickness > breakup_thickness
                }) {
                    donors.push((neighbor, ring));
                }
            }
        }
        frontier = next;
    }
    if donors.is_empty() {
        return false;
    }
    let count = donors.len() as f64;
    // Thickened crust stretches first (see `deformation::stretching_weight`); for donors of
    // normal thickness the weight is 1/ring.
    let weight = |donor: u32, ring: usize| {
        let crust = plate.crust[donor as usize].expect("donors have crust");
        deformation::stretching_weight(crust.thickness) / ring as f64
    };
    let weights: f64 = donors
        .iter()
        .map(|&(donor, ring)| weight(donor, ring))
        .sum();
    let base = count / (count + 1.0) / weights;
    // Fraction of its crust each donor gives.
    let shares: Vec<f64> = donors
        .iter()
        .map(|&(donor, ring)| {
            let crust = plate.crust[donor as usize].expect("donors have crust");
            (base * weight(donor, ring)).min(1.0 - breakup_thickness / crust.thickness)
        })
        .collect();
    let given: f64 = donors
        .iter()
        .zip(&shares)
        .map(|(&(donor, _), share)| {
            plate.crust[donor as usize].map_or(0.0, |c| c.thickness) * share
        })
        .sum();
    if given < breakup_thickness {
        return false;
    }
    let (mut thickness, mut mass, mut felsic, mut sediment) = (0.0, 0.0, 0.0, 0.0);
    let (mut metasediment, mut cohort) = (0.0, [0.0; 2]);
    // Mass-weighted thickness and thermal fraction of the donors after stretching (repeated
    // stretching compounds in the donors).
    let (mut donor_thickness, mut donor_heat) = (0.0, 0.0);
    let (mut orogeny, mut orogenic, mut rifted) = (NO_OROGENY, 0.0, 0.0);
    // A plume's heating is kept as a reset of the strength age (the latest of the donors'); it
    // does not enter the heat budget of the stretching, which follows `birth`.
    let mut reheated = NO_REHEATING;
    for (&(donor, _), &share) in donors.iter().zip(&shares) {
        let crust = plate.crust[donor as usize]
            .as_mut()
            .expect("donors have crust");
        orogeny = orogeny.max(crust.orogeny);
        reheated = reheated.max(crust.reheated);
        orogenic += crust.thickness * share * crust.density * crust.orogenic;
        rifted += crust.thickness * share * crust.density * crust.rifted;
        let beta = 1.0 / (1.0 - share);
        let fraction = 1.0 - (1.0 - thermal_fraction(crust.age(time))) / beta;
        if let Some(age) = age_for_thermal_fraction(fraction) {
            crust.birth = time - age;
        }
        let given = crust.thickness * share;
        thickness += given;
        mass += given * crust.density;
        felsic += given * crust.density * crust.felsic;
        metasediment += given * crust.density * crust.metasediment;
        donor_thickness += given * crust.density * (crust.thickness - given);
        donor_heat += given * crust.density * fraction;
        sediment += crust.sediment * share;
        let given_cohort = crust.cohort.amounts(crust.sediment * share);
        cohort[0] += given_cohort[0];
        cohort[1] += given_cohort[1];
        crust.thickness -= given;
        crust.sediment -= crust.sediment * share;
        // The stretching leaves an extension fabric (#54).
        crust.mark_extension((beta - 1.0) * fabric);
    }
    let beta = (donor_thickness / mass.max(1e-9)) / thickness.max(1e-9);
    let fraction = 1.0 - (1.0 - donor_heat / mass.max(1e-9)) / beta.max(1.0);
    let birth = age_for_thermal_fraction(fraction).map_or(time - 1000.0, |age| time - age);
    let mut crust = Crust {
        thickness,
        density: mass / thickness.max(1e-9),
        birth,
        sediment,
        felsic: felsic / mass.max(1e-9),
        metasediment: metasediment / mass.max(1e-9),
        orogeny: NO_OROGENY,
        orogenic: 0.0,
        rifted: 0.0,
        reheated,
        cohort: Cohort {
            mixed: cohort[0] / sediment.max(1e-9),
            lifo: cohort[1] / sediment.max(1e-9),
        },
    };
    crust.mix_orogeny(orogeny, orogenic / mass.max(1e-9), rifted / mass.max(1e-9));
    // The new sample is crust stretched by `beta` into the gap.
    crust.mark_extension((beta - 1.0) * fabric);
    plate.crust[local as usize] = Some(crust);
    true
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
    let (mut thickness, mut mass, mut sediment, mut felsic) = (0.0, 0.0, 0.0, 0.0);
    let (mut metasediment, mut cohort) = (0.0, [0.0; 2]);
    let mut birth = f64::NEG_INFINITY;
    let (mut orogeny, mut orogenic, mut rifted) = (NO_OROGENY, 0.0, 0.0);
    let mut reheated = NO_REHEATING;
    for &neighbor in &neighbors {
        let crust = plate.crust[neighbor as usize]
            .as_mut()
            .expect("enclosed by crust");
        orogeny = orogeny.max(crust.orogeny);
        reheated = reheated.max(crust.reheated);
        orogenic += crust.thickness * share * crust.density * crust.orogenic;
        rifted += crust.thickness * share * crust.density * crust.rifted;
        let given = crust.thickness * share;
        thickness += given;
        mass += given * crust.density;
        felsic += given * crust.density * crust.felsic;
        metasediment += given * crust.density * crust.metasediment;
        birth = birth.max(crust.birth);
        sediment += crust.sediment * share;
        let given_cohort = crust.cohort.amounts(crust.sediment * share);
        cohort[0] += given_cohort[0];
        cohort[1] += given_cohort[1];
        crust.thickness -= given;
        crust.sediment -= crust.sediment * share;
    }
    let mut crust = Crust {
        thickness,
        density: mass / thickness.max(1e-9),
        birth,
        sediment,
        felsic: felsic / mass.max(1e-9),
        metasediment: metasediment / mass.max(1e-9),
        orogeny: NO_OROGENY,
        orogenic: 0.0,
        rifted: 0.0,
        reheated,
        cohort: Cohort {
            mixed: cohort[0] / sediment.max(1e-9),
            lifo: cohort[1] / sediment.max(1e-9),
        },
    };
    crust.mix_orogeny(orogeny, orogenic / mass.max(1e-9), rifted / mass.max(1e-9));
    plate.crust[local as usize] = Some(crust);
    plate.consumed[local as usize] = false;
    statistics.enclosed_fills += 1;
    own
}

/// New crust added to a plate around a cell.
pub(super) struct CrustAddition {
    cell: u32,
    plate: u32,
    /// The other plate of the boundary (the subducting plate for arcs).
    partner: u32,
    /// Total volume as a thickness over one cell.
    thickness: f64,
    density: f64,
    /// Felsic and metasediment mass fractions (see [`Crust::felsic`], [`Crust::metasediment`]).
    felsic: f64,
    metasediment: f64,
    kind: AdditionKind,
    /// Sediment (rock-equivalent m over one cell) moved along with the crust, and its cohort
    /// fractions (see [`Crust::cohort`]).
    sediment: f64,
    cohort: Cohort,
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
    /// Crust of the lower plate stacked by distributed shortening at a diffuse convergent zone.
    Shortening,
}

pub(super) use super::crust::METASEDIMENT_DENSITY;

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
                            felsic: 1.0,
                            metasediment: 0.0,
                            orogeny: NO_OROGENY,
                            orogenic: 0.0,
                            rifted: 0.0,
                            reheated: NO_REHEATING,
                            cohort: Cohort::NONE,
                        };
                        assert!(crust.resists_subduction() && !crust.is_continental());
                    } else if plate == 1 && continental_upper_plate && z < 0.3 {
                        *crust = Crust {
                            thickness: 35_000.0,
                            density: crate::isostasy::CONTINENTAL_DENSITY,
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

    /// Plate 0 (south), oceanic with `sediment` m of sediment, subducts below plate 1 (north)
    /// for 20 Myr, converging at `speed` (rad/Myr).
    fn sediment_arriving_at_a_trench(sediment: f64, speed: f64) -> Simulation {
        let mut simulation = single_plate();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.sediment = sediment;
        }
        // An arc on the upper plate, thick enough to be eroded by the slab.
        for crust in simulation.plates[1].crust.iter_mut().flatten() {
            crust.thickness = 20_000.0;
            crust.density = crate::isostasy::ARC_DENSITY;
        }
        simulation.seed_subduction((0, 1), 1, 600e3);
        simulation.plates[0].angular_velocity = -DVec3::Y * speed;
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        for _ in 0..80 {
            simulation.substep(0.25);
            let boundaries = simulation.boundary_cells();
            simulation.update_slab_lengths(0.25, &boundaries);
        }
        simulation
    }

    /// Continental crust of an orogenic belt that was deformed at 400 Myr.
    fn suture_crust() -> Crust {
        Crust {
            thickness: 35_000.0,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: 400.0,
            orogenic: 1.0,
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        }
    }

    #[test]
    fn gaps_filled_with_oceanic_crust_do_not_inherit_the_suture_mark() {
        let mut simulation = single_plate();
        let local = 0;
        let neighbors = simulation.mesh.neighbors(local).to_vec();
        let parent = Ownership { plate: 0, local };
        let mut statistics = GapStatistics::default();
        // A gap at a coast: one neighbour is a suture, the others are sea floor. The fill is
        // oceanic and starts unweakened.
        let plate = &mut simulation.plates[0];
        plate.crust[local as usize] = None;
        plate.crust[neighbors[0] as usize] = Some(suture_crust());
        heal_gap(
            &simulation.mesh,
            plate,
            parent,
            local,
            true,
            &mut statistics,
        );
        let fill = plate.crust[local as usize].expect("an enclosed gap is filled");
        assert!(!fill.is_continental_origin());
        assert_eq!(fill.orogeny, NO_OROGENY);
        // A gap inside the belt is filled with the belt's lithosphere and stays weak.
        plate.crust[local as usize] = None;
        for &neighbor in &neighbors {
            plate.crust[neighbor as usize] = Some(suture_crust());
        }
        heal_gap(
            &simulation.mesh,
            plate,
            parent,
            local,
            true,
            &mut statistics,
        );
        let fill = plate.crust[local as usize].expect("an enclosed gap is filled");
        assert!(fill.is_continental_origin());
        assert_eq!(fill.orogeny, 400.0);
    }

    #[test]
    fn filled_oceanic_basins_subduct_below_their_wedge() {
        // Oceanic crust with 15 km of burial metasediment (a filled basin, which would be
        // felsic and buoyant enough to resist as a whole) and 2 km of sediment on top.
        let mut simulation = single_plate();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.add_crust(15_000.0, METASEDIMENT_DENSITY, (1.0, 1.0), 75_000.0);
            crust.sediment = 2000.0;
            assert!(crust.is_continental_origin_column() && !crust.resists_subduction());
            assert!(crust.is_oceanic() && !crust.is_continental_origin());
        }
        for crust in simulation.plates[1].crust.iter_mut().flatten() {
            crust.thickness = 20_000.0;
            crust.density = crate::isostasy::ARC_DENSITY;
        }
        simulation.seed_subduction((0, 1), 1, 600e3);
        simulation.plates[0].angular_velocity = -DVec3::Y * 0.004;
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        for _ in 0..80 {
            simulation.substep(0.25);
            let boundaries = simulation.boundary_cells();
            simulation.update_slab_lengths(0.25, &boundaries);
        }
        // It subducts (no collision), and the décollement scrapes off the top of the column
        // into the wedge: all the sediment and part of the metasediment.
        let ledger = &simulation.ledger;
        assert_eq!(ledger.collision_consumed, 0.0);
        assert!(ledger.subducted_oceanic < 0.0);
        let trench = &simulation.trench_sediment;
        assert!(trench.metasediment_accreted > 0.0, "{trench:?}");
        assert!(trench.metasediment_subducted > 0.0, "{trench:?}");
        let accreted = trench.accreted + trench.metasediment_accreted;
        assert!((ledger.accretion - accreted).abs() < 1e-6 * accreted);
    }

    #[test]
    fn thick_fill_at_slow_trenches_is_partly_accreted() {
        // 0.004 rad/Myr is 2.5 cm/yr, 0.03 rad/Myr 19 cm/yr.
        let slow_thick = sediment_arriving_at_a_trench(6000.0, 0.004);
        let trench = &slow_thick.trench_sediment;
        let arrived: f64 = trench.by_thickness.iter().sum();
        assert_eq!(arrived, trench.by_thickness[3], "all columns are 5–8 km");
        // Once the averaged fill exceeds the channel, the margin is accretionary and the top of
        // the column above the décollement is accreted.
        assert!(trench.at_accretionary_margins > 0.5 * arrived, "{trench:?}");
        let share = trench.accreted / arrived;
        let fraction = slow_thick.params.decollement_fraction;
        assert!(
            share > 0.5 * fraction && share <= fraction + 1e-9,
            "{share}"
        );
        // The ledger books the whole column as subducted and the wedge as accretion.
        let ledger = &slow_thick.ledger;
        assert!((ledger.accretion - trench.accreted).abs() < 1e-6 * arrived);
        assert!((-ledger.subducted_sediment - arrived).abs() < 1e-6 * arrived);

        // Thin fill: an erosive margin, all sediment subducts and the forearc is eroded more.
        let slow_thin = sediment_arriving_at_a_trench(500.0, 0.004);
        assert_eq!(slow_thin.trench_sediment.accreted, 0.0);
        assert!(slow_thin.ledger.subduction_erosion < ledger.subduction_erosion);
        // Fast convergence: erosive despite thick fill, except near the ends of the trench, where
        // the rotation about the y axis converges slowly.
        let fast_thick = sediment_arriving_at_a_trench(6000.0, 0.03);
        let fast = &fast_thick.trench_sediment;
        let fast_arrived: f64 = fast.by_thickness.iter().sum();
        assert!(fast_arrived > arrived);
        assert!(fast.accreted / fast_arrived < 0.5 * share, "{fast:?}");
    }

    /// Plate 0 (south), a stretched continental margin with a thick sediment wedge, moves north
    /// into plate 1, below which it would subduct; plate 1's crust is `upper` (oceanic if
    /// `None`).
    fn wedged_margin_arriving_at_a_trench(upper: Option<Crust>) -> MassLedger {
        let mut simulation = single_plate();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = Crust {
                thickness: 11_000.0,
                density: crate::isostasy::CONTINENTAL_DENSITY,
                birth: -1000.0,
                sediment: 8000.0,
                felsic: 1.0,
                metasediment: 0.0,
                orogeny: NO_OROGENY,
                orogenic: 0.0,
                rifted: 0.0,
                reheated: NO_REHEATING,
                cohort: Cohort::NONE,
            };
        }
        if let Some(upper) = upper {
            for crust in simulation.plates[1].crust.iter_mut().flatten() {
                *crust = upper;
            }
        }
        simulation.seed_subduction((0, 1), 1, 600e3);
        simulation.plates[0].angular_velocity = -DVec3::Y * 0.004;
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        for _ in 0..20 {
            simulation.substep(1.0);
        }
        simulation.ledger.clone()
    }

    #[test]
    fn wedged_continental_margins_jam_the_trench() {
        // Against an ocean, the margin stays on top and the ocean subducts below it.
        let ledger = wedged_margin_arriving_at_a_trench(None);
        assert_eq!(ledger.subducted_transitional, 0.0, "{ledger:?}");
        assert_eq!(ledger.subducted_sediment, 0.0, "{ledger:?}");
        assert!(ledger.subducted_oceanic < 0.0, "{ledger:?}");
        assert_eq!(ledger.collision_stacked, 0.0, "{ledger:?}");
        // Against an intra-oceanic arc too thin to resist subduction on its own, the arc rides
        // over the margin and is accreted onto it instead of subducting below it.
        let arc = Crust {
            thickness: 13_000.0,
            density: crate::isostasy::ARC_DENSITY,
            birth: -50.0,
            sediment: 0.0,
            felsic: 0.4,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        assert!(!arc.is_oceanic() && !arc.resists_subduction());
        // Most of it: after the segment flips (sustained arrival), the arc plate subducts.
        let ledger = wedged_margin_arriving_at_a_trench(Some(arc));
        assert!(
            ledger.collision_stacked > -4.0 * ledger.subducted_transitional,
            "{ledger:?}"
        );
        // Against a continent, the margin is stacked onto it with its sediment.
        let continent = Crust {
            thickness: 35_000.0,
            density: crate::isostasy::CONTINENTAL_DENSITY,
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
        let ledger = wedged_margin_arriving_at_a_trench(Some(continent));
        assert_eq!(ledger.subducted_transitional, 0.0, "{ledger:?}");
        assert_eq!(ledger.subducted_sediment, 0.0, "{ledger:?}");
        assert!(ledger.collision_stacked > 0.0, "{ledger:?}");
    }

    #[test]
    fn arc_crust_grows_behind_the_trench() {
        let mut simulation = single_plate();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        simulation.seed_subduction((0, 1), 1, 600e3);
        // A cell of the upper plate (1) at the boundary.
        let cell = (0..simulation.mesh.len() as u32)
            .find(|&cell| {
                simulation.owners[cell as usize].plate == 1
                    && simulation
                        .mesh
                        .neighbors(cell)
                        .iter()
                        .any(|&n| simulation.owners[n as usize].plate == 0)
            })
            .unwrap();
        let thickness = |s: &Simulation, cell: u32| s.surface_crust(cell).thickness;
        let before: Vec<f64> = (0..simulation.mesh.len() as u32)
            .map(|c| thickness(&simulation, c))
            .collect();
        simulation.apply_additions(&[CrustAddition {
            cell,
            plate: 1,
            partner: 0,
            thickness: 500.0,
            density: crate::isostasy::ARC_DENSITY,
            felsic: 0.4,
            metasediment: 0.0,
            kind: AdditionKind::Arc,
            sediment: 0.0,
            cohort: Cohort::NONE,
        }]);
        // Nothing on the trench cell (amagmatic forearc), most one ring behind it.
        assert_eq!(thickness(&simulation, cell), before[cell as usize]);
        let behind = simulation
            .mesh
            .neighbors(cell)
            .iter()
            .filter(|&&n| simulation.owners[n as usize].plate == 1)
            .any(|&n| thickness(&simulation, n) > before[n as usize]);
        assert!(behind);
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

    /// One continental plate covering the sphere (35 km, old).
    fn single_continent() -> Simulation {
        let mut simulation = single_plate();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = Crust {
                thickness: 35_000.0,
                density: crate::isostasy::CONTINENTAL_DENSITY,
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
        }
        simulation
    }

    #[test]
    fn rifts_stretch_continental_crust_with_thermal_subsidence() {
        // A margin already thinned to 20 km (its thermal anomaly is not damped like that of
        // normal continental crust; see `Crust::thermal_weight`).
        let mut simulation = single_continent();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.thickness = 20_000.0;
        }
        let local = 100u32;
        simulation.plates[0].crust[local as usize] = None;
        let mass = simulation.total_mass();
        let felsic = |simulation: &Simulation| -> f64 {
            simulation.plates[0]
                .crust
                .iter()
                .flatten()
                .map(|crust| crust.felsic_mass())
                .sum()
        };
        let felsic_before = felsic(&simulation);
        let time = 0.0;
        assert!(stretch_rift(
            &simulation.mesh,
            &mut simulation.plates[0],
            local,
            1,
            15e3,
            time
        ));
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        assert!((felsic(&simulation) / felsic_before - 1.0).abs() < 1e-12);
        let stretched = simulation.plates[0].crust[local as usize].unwrap();
        assert!(stretched.is_continental_origin());
        assert!(stretched.thickness < 20_000.0);
        // Stretching heats the lithosphere: thermal uplift now, subsidence later.
        let neighbor =
            simulation.plates[0].crust[simulation.mesh.neighbors(local)[0] as usize].unwrap();
        assert!(neighbor.age(time) < 1000.0);
        let now = neighbor.surface_elevation(time, 0.0);
        let later = neighbor.surface_elevation(time + 100.0, 0.0);
        assert!(later < now - 50.0, "{now} → {later}");
    }

    #[test]
    fn stretched_rifts_take_the_orogenic_share_of_their_donors() {
        // A gap next to one donor on an orogenic belt and others off it: the new cell gets the
        // mass-weighted share, the donors keep theirs, and the orogenic mass is conserved.
        let mut simulation = single_continent();
        let local = 100u32;
        let marked = simulation.mesh.neighbors(local)[0];
        simulation.plates[0].crust[marked as usize]
            .as_mut()
            .unwrap()
            .mark_orogeny(-300.0);
        // The belt sample was also heated by a plume: the new sample takes the latest reset.
        simulation.plates[0].crust[marked as usize]
            .as_mut()
            .unwrap()
            .reheat(-10.0, 25.0);
        simulation.plates[0].crust[local as usize] = None;
        let orogenic_mass = |simulation: &Simulation| -> f64 {
            simulation.plates[0]
                .crust
                .iter()
                .flatten()
                .map(|crust| crust.thickness * crust.density * crust.orogenic)
                .sum()
        };
        let before = orogenic_mass(&simulation);
        assert!(stretch_rift(
            &simulation.mesh,
            &mut simulation.plates[0],
            local,
            1,
            15e3,
            0.0
        ));
        let crust = |vertex: u32| simulation.plates[0].crust[vertex as usize].unwrap();
        let stretched = crust(local);
        assert_eq!(stretched.orogeny, -300.0);
        assert_eq!(stretched.reheated, -35.0);
        assert!(
            stretched.orogenic > 0.0 && stretched.orogenic < 0.5,
            "{}",
            stretched.orogenic
        );
        assert_eq!(crust(marked).orogenic, 1.0);
        for &neighbor in &simulation.mesh.neighbors(local)[1..] {
            assert_eq!(crust(neighbor).orogenic, 0.0);
        }
        assert!((orogenic_mass(&simulation) / before - 1.0).abs() < 1e-12);
    }

    #[test]
    fn thickened_crust_stretches_first_in_rifts() {
        let mut simulation = single_continent();
        let local = 100u32;
        let neighbors = simulation.mesh.neighbors(local).to_vec();
        let (thick, normal) = (neighbors[0], neighbors[1]);
        simulation.plates[0].crust[thick as usize]
            .as_mut()
            .unwrap()
            .thickness = 60e3;
        simulation.plates[0].crust[local as usize] = None;
        let mass = simulation.total_mass();
        assert!(stretch_rift(
            &simulation.mesh,
            &mut simulation.plates[0],
            local,
            1,
            15e3,
            0.0
        ));
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        let crust = |vertex: u32| simulation.plates[0].crust[vertex as usize].unwrap();
        // Fraction given: (60/35)² times that of a 35 km donor.
        let given_thick = 1.0 - crust(thick).thickness / 60e3;
        let given_normal = 1.0 - crust(normal).thickness / 35e3;
        assert!(
            (given_thick / given_normal - (60.0f64 / 35.0).powi(2)).abs() < 1e-9,
            "{given_thick} vs {given_normal}"
        );
    }

    #[test]
    fn collisions_deform_the_whole_sample() {
        // A sample with a small orogenic share (from an old belt next to it) that is thickened
        // by a collision is an orogenic belt as a whole.
        let mut simulation = single_continent();
        let cell = 100u32;
        simulation.plates[0].crust[cell as usize]
            .as_mut()
            .unwrap()
            .mix_orogeny(-500.0, 0.2, 0.0);
        simulation.apply_additions(&[CrustAddition {
            cell,
            plate: 0,
            partner: 0,
            thickness: 5000.0,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            felsic: 1.0,
            metasediment: 0.0,
            sediment: 0.0,
            cohort: Cohort::NONE,
            kind: AdditionKind::Collision,
        }]);
        let crust = simulation.plates[0].crust[cell as usize].unwrap();
        assert!(crust.thickness > OROGENIC_THICKNESS);
        assert_eq!((crust.orogeny, crust.orogenic), (simulation.time, 1.0));
    }

    /// Rings (mesh distance in cells) of all cells from `cell`.
    fn rings_from(simulation: &Simulation, cell: u32) -> Vec<usize> {
        let mut rings = vec![usize::MAX; simulation.mesh.len()];
        rings[cell as usize] = 0;
        let mut queue = VecDeque::from([cell]);
        while let Some(c) = queue.pop_front() {
            for &n in simulation.mesh.neighbors(c) {
                if rings[n as usize] == usize::MAX {
                    rings[n as usize] = rings[c as usize] + 1;
                    queue.push_back(n);
                }
            }
        }
        rings
    }

    fn collision_addition(cell: u32, thickness: f64) -> CrustAddition {
        CrustAddition {
            cell,
            plate: 0,
            partner: 0,
            thickness,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            felsic: 1.0,
            metasediment: 0.0,
            sediment: 0.0,
            cohort: Cohort::NONE,
            kind: AdditionKind::Collision,
        }
    }

    /// A continent on the ~440 km test mesh with a collision band of 5 rings.
    fn banded_continent() -> Simulation {
        let mut simulation = single_continent();
        let spacing = simulation.params.planet_radius * simulation.mesh.mean_edge_angle();
        simulation.params.collision_band_width = 5.0 * spacing;
        simulation
    }

    #[test]
    fn collision_bands_conserve_mass_and_spread_over_the_band() {
        let mut simulation = banded_continent();
        let cell = 100u32;
        let rings = rings_from(&simulation, cell);
        let before: Vec<f64> = (0..simulation.mesh.len() as u32)
            .map(|c| simulation.surface_crust(c).thickness)
            .collect();
        let mass = simulation.total_mass();
        let addition = collision_addition(cell, 35e3);
        let added = addition.thickness * addition.density;
        simulation.apply_additions(&[addition]);
        assert!((simulation.total_mass() - mass - added).abs() < 1e-9 * added);
        assert!(simulation.ledger.additions_lost.abs() < 1e-9 * added);
        let gained = |c: u32| simulation.surface_crust(c).thickness - before[c as usize];
        // Wider than the old profile, which put 1/8.5 of an addition onto the contact cell
        // alone and nothing beyond 3 rings.
        assert!(gained(cell) < 35e3 / 8.5, "{}", gained(cell));
        let mut by_ring = [0.0; 7];
        for c in 0..simulation.mesh.len() as u32 {
            if rings[c as usize] < by_ring.len() {
                by_ring[rings[c as usize]] += gained(c);
            } else {
                assert_eq!(gained(c), 0.0, "cell {c} beyond the band");
            }
        }
        assert!(by_ring[4] > 0.0 && by_ring[5] > 0.0, "{by_ring:?}");
        assert_eq!(by_ring[6], 0.0, "{by_ring:?}");
        // Per cell, the taper.
        let per_cell = |ring: usize| {
            let n = rings.iter().filter(|&&r| r == ring).count() as f64;
            by_ring[ring] / n
        };
        assert!((1..=5).all(|ring| per_cell(ring) < per_cell(ring - 1)));
    }

    #[test]
    fn samples_shown_at_several_cells_take_their_band_share_once() {
        // Two cells of ring 1 show the same sample: it gets one share, like any other sample of
        // its ring, and mass is conserved.
        let mut simulation = banded_continent();
        let cell = 100u32;
        let rings = rings_from(&simulation, cell);
        let ring_1: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&c| rings[c as usize] == 1)
            .collect();
        let (shown_twice, other) = (ring_1[0], ring_1[2]);
        let duplicate = ring_1[1];
        simulation.owners[duplicate as usize] = simulation.owners[shown_twice as usize];
        let mass = simulation.total_mass();
        let addition = collision_addition(cell, 35e3);
        let added = addition.thickness * addition.density;
        simulation.apply_additions(&[addition]);
        assert!((simulation.total_mass() - mass - added).abs() < 1e-9 * added);
        let gained = |c: u32| simulation.surface_crust(c).thickness - 35e3;
        assert!(
            (gained(shown_twice) / gained(other) - 1.0).abs() < 1e-9,
            "{} vs {}",
            gained(shown_twice),
            gained(other)
        );
    }

    #[test]
    fn samples_shown_at_several_cells_take_their_profile_share_once() {
        // Accretion spreads by a profile (no band): a sample shown at two ring-1 cells gets the
        // share of one cell, like any other sample of its ring, and mass is conserved (#57).
        let mut simulation = single_continent();
        let cell = 100u32;
        let rings = rings_from(&simulation, cell);
        let ring_1: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&c| rings[c as usize] == 1)
            .collect();
        let (shown_twice, other) = (ring_1[0], ring_1[2]);
        simulation.owners[ring_1[1] as usize] = simulation.owners[shown_twice as usize];
        let mass = simulation.total_mass();
        let addition = CrustAddition {
            kind: AdditionKind::Accretion,
            ..collision_addition(cell, 1000.0)
        };
        let added = addition.thickness * addition.density;
        simulation.apply_additions(&[addition]);
        assert!((simulation.total_mass() - mass - added).abs() < 1e-9 * added);
        let gained = |c: u32| simulation.surface_crust(c).thickness - 35e3;
        assert!(gained(other) > 0.0);
        assert!(
            (gained(shown_twice) / gained(other) - 1.0).abs() < 1e-9,
            "{} vs {}",
            gained(shown_twice),
            gained(other)
        );
    }

    #[test]
    fn weak_belts_take_up_more_of_a_collision() {
        let mut simulation = banded_continent();
        let cell = 100u32;
        let rings = rings_from(&simulation, cell);
        let in_ring_2: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&c| rings[c as usize] == 2)
            .collect();
        let (weak, strong) = (in_ring_2[0], in_ring_2[in_ring_2.len() / 2]);
        let local = simulation.owners[weak as usize].local as usize;
        simulation.plates[0].crust[local]
            .as_mut()
            .unwrap()
            .mix_orogeny(-500.0, 1.0, 0.0);
        simulation.apply_additions(&[collision_addition(cell, 35e3)]);
        let gained = |c: u32| simulation.surface_crust(c).thickness - 35e3;
        // Fresh belt, strength bounded at 0.5: 8× the share.
        assert!(
            (gained(weak) / gained(strong) - 8.0).abs() < 1e-6,
            "{} vs {}",
            gained(weak),
            gained(strong)
        );
    }

    #[test]
    fn full_bands_grow_outwards_and_stay_below_the_cap() {
        let mut simulation = banded_continent();
        let cell = 100u32;
        let rings = rings_from(&simulation, cell);
        let mass = simulation.total_mass();
        let mut added = 0.0;
        // The band holds ~91 cells × 35 km below the plateau.
        for _ in 0..150 {
            let addition = collision_addition(cell, 35e3);
            added += addition.thickness * addition.density;
            simulation.apply_additions(&[addition]);
        }
        let thickness = |c: u32| simulation.surface_crust(c).thickness;
        assert!((0..simulation.mesh.len() as u32).all(|c| thickness(c) <= 75e3 + 1e-6));
        // The contact reached the plateau; crust went beyond the band once it was full.
        assert!(thickness(cell) > 65e3, "{}", thickness(cell));
        let beyond =
            (0..simulation.mesh.len() as u32).any(|c| rings[c as usize] > 5 && thickness(c) > 35e3);
        assert!(beyond);
        let lost = simulation.ledger.additions_lost;
        assert!((simulation.total_mass() - mass - added - lost).abs() < 1e-9 * added);
    }

    #[test]
    fn collisions_without_continental_crust_in_the_band_use_the_profile() {
        // Oceanic crust all around (e.g. a terrane arriving at an oceanic edge): nothing lost.
        let mut simulation = single_plate();
        let spacing = simulation.params.planet_radius * simulation.mesh.mean_edge_angle();
        simulation.params.collision_band_width = 5.0 * spacing;
        let mass = simulation.total_mass();
        let addition = collision_addition(100, 10e3);
        let added = addition.thickness * addition.density;
        simulation.apply_additions(&[addition]);
        assert!((simulation.total_mass() - mass - added).abs() < 1e-9 * added);
        assert!(simulation.surface_crust(100).thickness > new_oceanic_crust(0.0).thickness);
    }

    #[test]
    fn one_cell_holes_fill_from_their_neighbours() {
        let mut simulation = single_continent();
        let cell = 100u32;
        let local = simulation.owners[cell as usize].local as usize;
        simulation.plates[0].crust[local]
            .as_mut()
            .unwrap()
            .thickness = 20_000.0;
        assert!(simulation.one_cell_holes()[cell as usize]);
        let mass = simulation.total_mass();
        let depth = |simulation: &Simulation| {
            let elevation = |c: u32| simulation.surface_crust(c).dry_elevation(0.0);
            simulation
                .mesh
                .neighbors(cell)
                .iter()
                .map(|&n| elevation(n))
                .fold(f64::INFINITY, f64::min)
                - elevation(cell)
        };
        assert!(depth(&simulation) > 1000.0);
        for _ in 0..20 {
            simulation.crustal_flow(1.0);
        }
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        let left = depth(&simulation);
        assert!(left < simulation.params.hole_flow_depth + 1.0, "{left}");
        assert!(left > 0.0, "{left}: filled to a peak");
        // Without holes (and below the crustal flow threshold) nothing moves.
        let mut flat = single_continent();
        let mass = flat.total_mass();
        flat.crustal_flow(1.0);
        assert_eq!(flat.total_mass(), mass);
        assert!(
            flat.plates[0]
                .crust
                .iter()
                .flatten()
                .all(|c| c.thickness == 35_000.0)
        );
    }

    #[test]
    fn thick_crust_collapses_onto_its_surroundings() {
        let mut simulation = single_continent();
        let cell = 100u32;
        let local = simulation.owners[cell as usize].local as usize;
        simulation.plates[0].crust[local]
            .as_mut()
            .unwrap()
            .thickness = 70_000.0;
        let mass = simulation.total_mass();
        for _ in 0..10 {
            simulation.crustal_flow(1.0);
        }
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        let peak = simulation.surface_crust(cell).thickness;
        assert!(peak < 65_000.0, "{peak}");
        let neighbor = simulation.surface_crust(simulation.mesh.neighbors(cell)[0]);
        assert!(neighbor.thickness > 35_000.0);
        assert!(
            simulation.plates[0]
                .crust
                .iter()
                .flatten()
                .all(|c| c.thickness > 0.0)
        );
    }

    #[test]
    fn collapse_loses_no_crust_at_the_thickness_cap() {
        // A light source stands higher than its dense surroundings although they are thicker
        // and close to the cap: the flow must stop at the cap instead of deleting crust.
        let mut simulation = single_continent();
        let max = simulation.params.max_crust_thickness;
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.thickness = max - 300.0;
            crust.density = 2850.0;
        }
        let cell = 100u32;
        let local = simulation.owners[cell as usize].local as usize;
        let source = simulation.plates[0].crust[local].as_mut().unwrap();
        source.thickness = max - 1000.0;
        source.density = 2650.0;
        let mass = simulation.total_mass();
        for _ in 0..10 {
            simulation.crustal_flow(1.0);
        }
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        assert!(
            simulation.plates[0]
                .crust
                .iter()
                .flatten()
                .all(|c| c.thickness <= max + 1e-6)
        );
        assert!(simulation.surface_crust(cell).thickness < max - 1000.0);
    }

    #[test]
    fn collapse_never_flows_onto_oceanic_crust() {
        let mut simulation = single_plate();
        let cell = 100u32;
        let local = simulation.owners[cell as usize].local as usize;
        simulation.plates[0].crust[local] = Some(Crust {
            thickness: 70_000.0,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        });
        simulation.crustal_flow(5.0);
        assert_eq!(simulation.surface_crust(cell).thickness, 70_000.0);
    }

    #[test]
    fn stretching_tapers_and_never_thins_below_breakup() {
        let mut simulation = single_continent();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.thickness = 30_000.0;
        }
        let local = 100u32;
        simulation.plates[0].crust[local as usize] = None;
        let mass = simulation.total_mass();
        // Repeated opening at the same spot thins the rift zone down to the breakup thickness.
        let (mut stretched, mut removed, mut last_heat) = (0, 0.0, 0.0);
        while stretch_rift(
            &simulation.mesh,
            &mut simulation.plates[0],
            local,
            2,
            15e3,
            0.0,
        ) {
            let new = simulation.plates[0].crust[local as usize].unwrap();
            assert!(new.thickness >= 15e3, "{}", new.thickness);
            assert!(((simulation.total_mass() + removed) / mass - 1.0).abs() < 1e-12);
            // Take the new sample away, so the next opening happens at the same spot again.
            let new = simulation.plates[0].crust[local as usize].take().unwrap();
            last_heat = thermal_fraction(new.age(0.0));
            removed += new.mass();
            stretched += 1;
            assert!(stretched < 100);
        }
        assert!(stretched > 1);
        let thickness = |vertex: u32| {
            simulation.plates[0].crust[vertex as usize]
                .unwrap()
                .thickness
        };
        let ring1 = simulation.mesh.neighbors(local)[0];
        let ring2 = simulation
            .mesh
            .neighbors(ring1)
            .iter()
            .copied()
            .find(|&v| v != local && !simulation.mesh.neighbors(local).contains(&v))
            .unwrap();
        assert!(thickness(ring1) >= 15e3 - 1e-6);
        // Breakup comes when the rift zone is thinned close to the breakup thickness (β ≈ 2 for
        // 30 km crust), and the last rift crust carries the heat of that stretching.
        assert!(thickness(ring1) < 16e3, "{}", thickness(ring1));
        assert!(last_heat > 0.3, "{last_heat}");
        assert!(
            thickness(ring1) < thickness(ring2),
            "{} {}",
            thickness(ring1),
            thickness(ring2)
        );
    }

    #[test]
    fn thin_crust_breaks_up_instead_of_stretching() {
        let mut simulation = single_continent();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.thickness = 14_000.0;
        }
        let local = 100u32;
        simulation.plates[0].crust[local as usize] = None;
        assert!(!stretch_rift(
            &simulation.mesh,
            &mut simulation.plates[0],
            local,
            2,
            15e3,
            0.0
        ));
        assert!(simulation.plates[0].crust[local as usize].is_none());
    }

    #[test]
    fn diverging_continents_stretch_instead_of_forming_sea_floor() {
        // Two continental hemispheres moving apart about the y axis.
        let mut simulation = single_continent();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        let mass = simulation.total_mass();
        simulation.plates[0].angular_velocity = DVec3::Y * 0.004;
        simulation.plates[1].angular_velocity = -DVec3::Y * 0.004;
        for _ in 0..10 {
            simulation.substep(1.0);
        }
        assert!(simulation.gap_statistics.stretched_cells > 0);
        // Thick continental crust stretches: no sea floor before it is thinned to breakup.
        assert_eq!(simulation.gap_statistics.new_crust_cells, 0);
        assert_eq!(simulation.ledger.new_sea_floor, 0.0);
        assert!(simulation.total_mass() > 0.9 * mass);
    }

    #[test]
    fn rifts_reopening_old_sutures_stretch_too() {
        // As above, but every empty slot of both plates held crust that was consumed at an
        // earlier convergent contact (the rift reopens an old suture).
        let mut simulation = single_continent();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        for plate in &mut simulation.plates {
            for (local, crust) in plate.crust.iter().enumerate() {
                if crust.is_none() {
                    plate.consumed[local] = true;
                }
            }
        }
        simulation.plates[0].angular_velocity = DVec3::Y * 0.004;
        simulation.plates[1].angular_velocity = -DVec3::Y * 0.004;
        for _ in 0..10 {
            simulation.substep(1.0);
        }
        assert!(simulation.gap_statistics.stretched_cells > 0);
        assert_eq!(simulation.gap_statistics.new_crust_cells, 0);
        assert_eq!(simulation.ledger.new_sea_floor, 0.0);
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
    fn filled_holes_take_the_orogenic_share_of_their_neighbours() {
        // A hole next to one sample of an orogenic belt: the fill gets the mass-weighted share
        // of its neighbours (not the belt's full mark), and the orogenic mass is conserved.
        let mut simulation = single_continent();
        let local = 100u32;
        let marked = simulation.mesh.neighbors(local)[0];
        simulation.plates[0].crust[marked as usize]
            .as_mut()
            .unwrap()
            .mark_orogeny(-300.0);
        // The belt sample was also heated by a plume: the new sample takes the latest reset.
        simulation.plates[0].crust[marked as usize]
            .as_mut()
            .unwrap()
            .reheat(-10.0, 25.0);
        simulation.plates[0].crust[local as usize] = None;
        let orogenic_mass = |simulation: &Simulation| -> f64 {
            simulation.plates[0]
                .crust
                .iter()
                .flatten()
                .map(|crust| crust.thickness * crust.density * crust.orogenic)
                .sum()
        };
        let before = orogenic_mass(&simulation);
        let parent = Ownership {
            plate: 0,
            local: marked,
        };
        heal_gap(
            &simulation.mesh,
            &mut simulation.plates[0],
            parent,
            local,
            true,
            &mut GapStatistics::default(),
        );
        let neighbours = simulation.mesh.neighbors(local).len() as f64;
        let filled = simulation.plates[0].crust[local as usize].unwrap();
        assert_eq!(filled.orogeny, -300.0);
        assert_eq!(filled.reheated, -35.0);
        assert!((filled.orogenic - 1.0 / neighbours).abs() < 1e-12);
        assert_eq!(
            simulation.plates[0].crust[marked as usize]
                .unwrap()
                .orogenic,
            1.0
        );
        assert!((orogenic_mass(&simulation) / before - 1.0).abs() < 1e-12);
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

    /// Two oceanic plates (south and north of the equator), plate 0 moving north into plate 1,
    /// with plate 0 subducting below plate 1 along an established trench or without any
    /// segments (a fresh contact).
    fn oceanic_plates_converging(trench: bool) -> Simulation {
        let mut simulation = single_plate();
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        if trench {
            simulation.seed_subduction((0, 1), 1, 600e3);
        }
        simulation.plates[0].angular_velocity = -DVec3::Y * 0.004;
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        simulation
    }

    /// Records the convergence of the segments as a velocity solve would.
    fn record_segment_rates(simulation: &mut Simulation) {
        let boundaries = simulation.boundary_cells();
        simulation.update_segment_motion(&boundaries, false);
    }

    #[test]
    fn diffuse_zones_shorten_instead_of_subducting() {
        let mut simulation = oceanic_plates_converging(false);
        // The first overlaps create the segments (the contact spreads along the boundary), still
        // unseen by a velocity solve.
        for _ in 0..20 {
            simulation.substep(1.0);
        }
        record_segment_rates(&mut simulation);
        assert!(
            simulation
                .polarities
                .segments()
                .all(|segment| !segment.localised)
        );
        // Sediment on the lower plate (polarity by buoyancy) moves along as sediment.
        let upper = simulation
            .polarities
            .segments()
            .next()
            .expect("segments")
            .overriding as usize;
        for (index, plate) in simulation.plates.iter_mut().enumerate() {
            for crust in plate.crust.iter_mut().flatten() {
                crust.sediment = if index == upper { 0.0 } else { 200.0 };
            }
        }
        // A converging diffuse zone: the overlapping crust is stacked onto the upper plate
        // (distributed shortening), nothing subducts and no arc grows.
        let before = simulation.ledger.clone();
        for _ in 0..20 {
            simulation.substep(1.0);
            record_segment_rates(&mut simulation);
        }
        let ledger = &simulation.ledger;
        let shortened = ledger.shortening_consumed - before.shortening_consumed;
        let subducted = ledger.subducted_oceanic - before.subducted_oceanic;
        assert!(shortened < 0.0);
        // Only segments created on the way (their first substeps) subduct.
        assert!(
            subducted.abs() < 0.1 * shortened.abs(),
            "{subducted} {shortened}"
        );
        assert!(ledger.arcs - before.arcs <= 0.1 * shortened.abs());
        // The sediment of the stacked crust moves along as sediment: it piles up on the
        // receiving cells and hardly any subducts.
        let max_sediment = |plate: usize| {
            simulation.plates[plate]
                .crust
                .iter()
                .flatten()
                .map(|crust| crust.sediment)
                .fold(0.0, f64::max)
        };
        assert!(max_sediment(upper) > 0.0);
        let subducted_sediment = ledger.subducted_sediment - before.subducted_sediment;
        assert!(
            subducted_sediment.abs() < 0.1 * shortened.abs(),
            "{subducted_sediment}"
        );
        let lost = ledger.additions_lost.abs();
        assert!(
            (ledger.shortening_stacked + ledger.shortening_consumed).abs() <= lost + 1e-6,
            "{ledger:?}"
        );
        assert!(
            simulation
                .polarities
                .segments()
                .all(|segment| !segment.localised && segment.slab_length == 0.0)
        );
        // The crust of the zone thickened, and still counts as oceanic.
        let thickest = (0..simulation.mesh.len() as u32)
            .map(|cell| simulation.surface_crust(cell))
            .filter(|crust| crust.is_oceanic())
            .map(|crust| crust.thickness)
            .fold(0.0, f64::max);
        assert!(thickest > OCEANIC_THICKNESS + 100.0, "{thickest}");
        // The zone converges at 2.5 cm/yr, faster than a diffuse zone creeps: it yields and
        // localises, and from then on the plate subducts and an arc grows.
        let boundaries = simulation.boundary_cells();
        simulation.update_slab_lengths(1.0, &boundaries);
        assert!(
            simulation
                .polarities
                .segments()
                .any(|segment| segment.localised)
        );
        let before = simulation.ledger.clone();
        for _ in 0..10 {
            simulation.substep(1.0);
        }
        assert!(simulation.ledger.subducted_oceanic < before.subducted_oceanic);
        assert!(simulation.ledger.arcs > before.arcs);
        // Along an established trench the same convergence subducts from the start.
        let mut trench = oceanic_plates_converging(true);
        record_segment_rates(&mut trench);
        for _ in 0..20 {
            trench.substep(1.0);
        }
        assert!(trench.ledger.subducted_oceanic < 0.0);
        assert_eq!(trench.ledger.shortening_consumed, 0.0);
        assert!(trench.ledger.arcs > 0.0);
    }

    #[test]
    fn oceanic_crust_shortens_on_its_own_side_of_an_ocean_continent_zone() {
        let mut simulation = oceanic_plates_converging(false);
        // Plate 1 (north) is a continent: it overrides, and the oceanic plate shortens.
        for crust in simulation.plates[1].crust.iter_mut().flatten() {
            *crust = Crust {
                thickness: 35_000.0,
                density: crate::isostasy::CONTINENTAL_DENSITY,
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
        }
        for _ in 0..20 {
            simulation.substep(1.0);
        }
        record_segment_rates(&mut simulation);
        let mafic = |simulation: &Simulation| -> f64 {
            simulation.plates[1]
                .crust
                .iter()
                .flatten()
                .map(|crust| crust.mass() - crust.felsic_mass())
                .sum()
        };
        let (before, consumed) = (mafic(&simulation), simulation.ledger.shortening_consumed);
        for _ in 0..10 {
            simulation.substep(1.0);
            record_segment_rates(&mut simulation);
        }
        assert!(simulation.ledger.shortening_consumed < consumed);
        // No oceanic crust was stacked onto the continent.
        assert!(mafic(&simulation) <= before + 1e-6 * before.abs());
    }

    #[test]
    fn overlaps_at_diffuse_zones_that_do_not_converge_are_not_stacked() {
        // Before a velocity solve has seen the zone (and at zones that do not converge, where
        // overlaps are resampling noise), overlaps are handled as everywhere else.
        let mut simulation = oceanic_plates_converging(false);
        for _ in 0..3 {
            simulation.substep(1.0);
        }
        assert_eq!(simulation.ledger.shortening_consumed, 0.0);
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

    /// A continent with a 70 km sample at `cell`, after one short crustal-flow step (one
    /// sub-cycle, no cap binding), with `duplicates` cells set to show the sample of their
    /// paired cell first. Returns the simulation and the thickness change per local sample.
    fn collapse_once(cell: u32, duplicates: &[(u32, u32)]) -> (Simulation, Vec<f64>) {
        let mut simulation = single_continent();
        let local = simulation.owners[cell as usize].local as usize;
        simulation.plates[0].crust[local]
            .as_mut()
            .unwrap()
            .thickness = 70_000.0;
        for &(shown_twice, at) in duplicates {
            simulation.owners[at as usize] = simulation.owners[shown_twice as usize];
        }
        let before: Vec<f64> = simulation.plates[0]
            .crust
            .iter()
            .map(|c| c.map_or(0.0, |c| c.thickness))
            .collect();
        let mass = simulation.total_mass();
        simulation.crustal_flow(0.01);
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        let change = simulation.plates[0]
            .crust
            .iter()
            .zip(&before)
            .map(|(c, before)| c.map_or(0.0, |c| c.thickness) - before)
            .collect();
        (simulation, change)
    }

    #[test]
    fn duplicated_sources_collapse_once() {
        // The 70 km sample shown at two adjacent cells exchanges over the 5 + 5 outer edges of
        // its two cells, each at half the per-edge amount: 10/12 of what it gives from one cell
        // over 6 edges, not 20/12 (#113).
        let cell = 100u32;
        let twin = single_continent().mesh.neighbors(cell)[0];
        let (simulation, single) = collapse_once(cell, &[]);
        let (_, twice) = collapse_once(cell, &[(cell, twin)]);
        let local = simulation.owners[cell as usize].local as usize;
        assert!(single[local] < 0.0);
        assert!(
            (twice[local] / single[local] - 10.0 / 12.0).abs() < 1e-9,
            "{} vs {}",
            twice[local],
            single[local]
        );
        // A neighbour of the first cell only gets half of its single-cell share.
        let mesh = &simulation.mesh;
        let outer = mesh
            .neighbors(cell)
            .iter()
            .copied()
            .find(|&n| n != twin && !mesh.neighbors(twin).contains(&n))
            .unwrap();
        let outer = simulation.owners[outer as usize].local as usize;
        assert!(single[outer] > 0.0);
        assert!((twice[outer] / single[outer] - 0.5).abs() < 1e-9);
        // The sample hidden by the twin takes no part.
        let hidden = simulation.owners[twin as usize].local as usize;
        assert_eq!(twice[hidden], 0.0);
    }

    #[test]
    fn duplicated_receivers_take_once() {
        // A neighbour sample shown at a second cell (not next to the source) receives half of
        // what it receives as one cell.
        let cell = 100u32;
        let mesh = single_continent().mesh;
        let receiver = mesh.neighbors(cell)[0];
        let far = mesh
            .neighbors(receiver)
            .iter()
            .copied()
            .find(|&n| n != cell && !mesh.neighbors(cell).contains(&n))
            .unwrap();
        let (simulation, single) = collapse_once(cell, &[]);
        let (_, twice) = collapse_once(cell, &[(receiver, far)]);
        let receiver = simulation.owners[receiver as usize].local as usize;
        let source = simulation.owners[cell as usize].local as usize;
        assert!(single[receiver] > 0.0);
        assert!((twice[receiver] / single[receiver] - 0.5).abs() < 1e-9);
        assert!((twice[source] / single[source] - 5.5 / 6.0).abs() < 1e-9);
    }

    /// Erodes `events` (contact cells) on a continent where `twin` shows the sample of `cell`;
    /// returns the thickness plus sediment change per local sample and the ledger term.
    fn erode(sediment: f64, twin: Option<u32>, events: &[u32]) -> (Simulation, Vec<f64>, f64) {
        let mut simulation = single_continent();
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.sediment = sediment;
        }
        if let Some(twin) = twin {
            simulation.owners[twin as usize] = simulation.owners[100];
        }
        let column = |c: &Option<Crust>| c.map_or(0.0, |c| c.thickness + c.sediment);
        let before: Vec<f64> = simulation.plates[0].crust.iter().map(column).collect();
        let mass = simulation.total_mass();
        let events: Vec<(u32, u32)> = events.iter().map(|&cell| (cell, 0)).collect();
        let Simulation {
            mesh,
            plates,
            owners,
            ledger,
            cover_budget,
            shared_rates,
            params,
            ..
        } = &mut simulation;
        erode_overriding_samples(
            mesh,
            plates,
            owners,
            &events,
            params.subduction_erosion,
            ledger,
            cover_budget,
            shared_rates,
        );
        let removed = simulation.total_mass() - mass;
        assert!((simulation.ledger.subduction_erosion - removed).abs() < 1e-9 * removed.abs());
        let change = simulation.plates[0]
            .crust
            .iter()
            .zip(&before)
            .map(|(c, before)| column(c) - before)
            .collect();
        (simulation, change, removed)
    }

    #[test]
    fn duplicated_overriding_samples_are_eroded_once() {
        let amount = TectonicParams::default().subduction_erosion;
        let cell = 100u32;
        let twin = single_continent().mesh.neighbors(cell)[0];
        // One event per cell at both cells showing the sample: thinned once; the other cell's
        // share comes from the sample's neighbours, so the erosion per event stays the same.
        let (simulation, both, removed_both) = erode(0.0, Some(twin), &[cell, twin]);
        let local = simulation.owners[cell as usize].local as usize;
        assert!((both[local] + amount).abs() < 1e-9);
        let neighbours: f64 = simulation
            .mesh
            .neighbors(local as u32)
            .iter()
            .map(|&n| both[n as usize])
            .sum();
        assert!((neighbours + amount).abs() < 1e-9, "{neighbours}");
        let rates = simulation.shared_sample_rates();
        assert_eq!(rates.erosion_events, 2.0);
        assert_eq!(rates.erosion_per_sample, 1.0);
        assert_eq!(rates.erosion_to_neighbours, 1.0);
        // Without the twin, two events at one cell erode its sample twice, as before.
        let (_, single, removed_single) = erode(0.0, None, &[cell, cell]);
        assert!((single[local] + 2.0 * amount).abs() < 1e-9);
        assert!(
            single
                .iter()
                .enumerate()
                .all(|(i, &c)| i == local || c == 0.0)
        );
        assert!((removed_both / removed_single - 1.0).abs() < 1e-12);
        // One event, shown at two cells: half on the sample, sediment first.
        let (eroded, half, _) = erode(100.0, Some(twin), &[cell]);
        assert!((half[local] + 0.5 * amount).abs() < 1e-9);
        let crust = eroded.plates[0].crust[local].unwrap();
        assert_eq!(crust.sediment, 0.0);
        assert!((crust.thickness - (35_000.0 - 0.5 * amount + 100.0)).abs() < 1e-9);
    }
}
