//! Subduction polarity per boundary segment.
//!
//! Where two plates converge, one of them (the overriding plate) stays at the surface and the
//! other subducts below it. Which one does is a property of a stretch of their boundary, not of
//! the pair of plates: polarity can change along strike (e.g. Tonga–Kermadec vs Puysegur along
//! the Pacific–Australian boundary). Each pair of plates in contact therefore has a list of
//! **segments**, each covering the part of the boundary within [`Polarities::radius`] of its
//! position, with its own polarity and its own slab.
//!
//! A segment is anchored in the frame of its overriding plate (trenches move with the overriding
//! plate to first order) and re-centred once per step on the contacts assigned to it. New
//! segments take the polarity of a nearby segment of the same pair, or else the more buoyant
//! side overrides. Continental crust arriving at a trench is never subducted: at that cell it
//! stays on top (terrane accretion) without changing the segment, and only if continental crust
//! keeps arriving along most of the segment for [`TectonicParams::polarity_flip_time`] does the
//! segment flip (subduction polarity reversal after arc–continent collision); the old slab
//! breaks off.

use glam::{DQuat, DVec3};

use super::*;

/// One stretch of a convergent boundary between two plates.
#[derive(Clone, Debug)]
pub(super) struct Segment {
    /// Identifier, unique among all segments ever created.
    pub(super) id: u32,
    /// Position (unit vector) in the local frame of the overriding plate.
    anchor: DVec3,
    /// The plate that stays at the surface.
    pub(super) overriding: u32,
    /// Length of slab (m) that has subducted below the overriding plate here.
    pub(super) slab_length: f64,
    /// Last time (Myr) the plates were in contact along this segment.
    pub(super) last_contact: f64,
    /// How long (Myr) continental crust has been arriving along most of the segment.
    arrival_time: f64,
    /// For segments seeded by subduction initiation: whether the slab grew to
    /// [`SUCCESSFUL_SLAB_LENGTH`].
    initiation: Option<bool>,
    /// Sum of the world positions of the contacts in the current step.
    contact_sum: DVec3,
    /// Contacts in the current pass: all, continental crust arriving below non-continental
    /// crust, and the reverse.
    overlaps: u32,
    arrivals: u32,
    reverse_arrivals: u32,
}

/// A segment seeded by subduction initiation counts as successful once its slab is this long (m).
const SUCCESSFUL_SLAB_LENGTH: f64 = 300e3;

/// Weight of the contacts of the current step when re-centring a segment.
const RECENTRE_WEIGHT: f64 = 0.5;

/// Subduction to set up in a segment (see [`Polarities::set`]).
#[derive(Clone, Copy, Debug)]
pub(super) struct Subduction {
    /// The plate that stays at the surface.
    pub(super) overriding: u32,
    /// Initial slab length (m).
    pub(super) slab_length: f64,
    /// Whether the segment counts as seeded by subduction initiation.
    pub(super) initiation: bool,
}

/// Which plate(s) a boundary segment goes to when a plate is split.
#[derive(Clone, Copy, Debug)]
pub(super) struct Side {
    /// The new plate.
    pub(super) new: bool,
    /// The remaining old plate.
    pub(super) old: bool,
}

/// State of a pair of plates in contact.
#[derive(Clone, Debug, Default)]
pub(super) struct PairState {
    /// Last time (Myr) the plates were in contact anywhere.
    pub(super) last_contact: f64,
    /// How long, in Myr, the continents of the two plates have been colliding without pause.
    pub(super) collision_time: f64,
    pub(super) segments: Vec<Segment>,
}

/// Counts of segment events so far.
#[derive(Clone, Debug, Default)]
pub struct SegmentStatistics {
    pub created: usize,
    pub merged: usize,
    pub expired: usize,
    /// Polarity reversals after sustained continental arrival.
    pub flips: usize,
    /// Cell contacts where continental crust of the lower plate stayed on top.
    pub continental_overrides: usize,
    /// Segments seeded by subduction initiation, and those whose slab grew to 300 km.
    pub initiation_segments: usize,
    pub successful_initiation_segments: usize,
}

/// Subduction polarity of all pairs of plates in contact, per boundary segment.
#[derive(Clone, Debug)]
pub(super) struct Polarities {
    /// Per pair of plates `(a, b)` with `a < b`.
    pub(super) pairs: HashMap<(u32, u32), PairState>,
    next_id: u32,
    /// Segment radius as an angle (radians).
    radius: f64,
    pub(super) statistics: SegmentStatistics,
}

/// The bits of the state of a pair and its segments, for comparisons in tests.
#[cfg(test)]
pub(super) type PairSnapshot = ((u32, u32), Vec<[u64; 7]>);

/// World position of a segment.
fn world(segment: &Segment, rotations: &[DQuat]) -> DVec3 {
    rotations[segment.overriding as usize] * segment.anchor
}

/// The other plate of a pair.
fn other_of(pair: (u32, u32), plate: u32) -> u32 {
    if pair.0 == plate { pair.1 } else { pair.0 }
}

/// Index of the segment nearest to `r` within `max_angle`, ties to the lower index.
fn nearest(segments: &[Segment], r: DVec3, max_angle: f64, rotations: &[DQuat]) -> Option<usize> {
    let min_dot = max_angle.cos();
    let mut best: Option<(usize, f64)> = None;
    for (index, segment) in segments.iter().enumerate() {
        let dot = world(segment, rotations).dot(r);
        if dot >= min_dot && best.is_none_or(|(_, best_dot)| dot > best_dot) {
            best = Some((index, dot));
        }
    }
    best.map(|(index, _)| index)
}

impl Polarities {
    /// No polarities yet; segments cover `radius` radians around their position.
    pub(super) fn new(radius: f64) -> Self {
        Self {
            pairs: HashMap::default(),
            next_id: 0,
            radius,
            statistics: SegmentStatistics::default(),
        }
    }

    /// The segment of the pair `(a, b)` covering the world position `r`, if any.
    pub(super) fn segment_at(
        &self,
        a: u32,
        b: u32,
        r: DVec3,
        rotations: &[DQuat],
    ) -> Option<&Segment> {
        let pair = self.pairs.get(&pair_key(a, b))?;
        nearest(&pair.segments, r, self.radius, rotations).map(|index| &pair.segments[index])
    }

    /// The segment with the given id of the pair `(a, b)`.
    pub(super) fn segment(&self, a: u32, b: u32, id: u32) -> Option<&Segment> {
        self.pairs
            .get(&pair_key(a, b))?
            .segments
            .iter()
            .find(|segment| segment.id == id)
    }

    /// All segments.
    pub(super) fn segments(&self) -> impl Iterator<Item = &Segment> {
        self.pairs.values().flat_map(|pair| &pair.segments)
    }

    fn new_segment(
        &mut self,
        r: DVec3,
        overriding: u32,
        slab_length: f64,
        time: f64,
        rotations: &[DQuat],
    ) -> Segment {
        let id = self.next_id;
        self.next_id += 1;
        self.statistics.created += 1;
        Segment {
            id,
            anchor: rotations[overriding as usize].inverse() * r,
            overriding,
            slab_length,
            last_contact: time,
            arrival_time: 0.0,
            initiation: None,
            contact_sum: DVec3::ZERO,
            overlaps: 0,
            arrivals: 0,
            reverse_arrivals: 0,
        }
    }

    /// Records a contact of plates `a` and `b` (with their crust at the cell) at the world
    /// position `r` and returns the plate whose crust stays on top there. Creates a segment if
    /// none covers the position.
    pub(super) fn contact(
        &mut self,
        (a, a_crust): (u32, Crust),
        (b, b_crust): (u32, Crust),
        r: DVec3,
        time: f64,
        rotations: &[DQuat],
    ) -> u32 {
        let key = pair_key(a, b);
        let radius = self.radius;
        let found = self
            .pairs
            .get(&key)
            .and_then(|pair| nearest(&pair.segments, r, radius, rotations));
        let index = match found {
            Some(index) => index,
            None => {
                // Polarity of a neighbouring segment along strike, else by buoyancy.
                let inherited = self.pairs.get(&key).and_then(|pair| {
                    nearest(&pair.segments, r, 2.0 * radius, rotations)
                        .map(|index| pair.segments[index].overriding)
                });
                let mut overriding =
                    inherited.unwrap_or(if a_crust.buoyancy(time) >= b_crust.buoyancy(time) {
                        a
                    } else {
                        b
                    });
                let (top_crust, bottom_crust) = if overriding == a {
                    (a_crust, b_crust)
                } else {
                    (b_crust, a_crust)
                };
                if bottom_crust.resists_subduction() && !top_crust.resists_subduction() {
                    overriding = other_of(key, overriding);
                }
                let segment = self.new_segment(r, overriding, 0.0, time, rotations);
                let pair = self.pairs.entry(key).or_default();
                pair.segments.push(segment);
                pair.segments.len() - 1
            }
        };
        let pair = self.pairs.get_mut(&key).expect("pair exists");
        pair.last_contact = time;
        let segment = &mut pair.segments[index];
        segment.last_contact = time;
        segment.contact_sum += r;
        segment.overlaps += 1;
        let (top, top_crust, bottom, bottom_crust) = if segment.overriding == a {
            (a, a_crust, b, b_crust)
        } else {
            (b, b_crust, a, a_crust)
        };
        if bottom_crust.resists_subduction() && !top_crust.resists_subduction() {
            // Buoyant crust does not subduct: it stays on top here.
            segment.arrivals += 1;
            self.statistics.continental_overrides += 1;
            bottom
        } else {
            if top_crust.resists_subduction() && !bottom_crust.resists_subduction() {
                segment.reverse_arrivals += 1;
            }
            top
        }
    }

    /// Ends a pass over all cells of `dt` Myr: segments along which continental crust has been
    /// arriving for `flip_time` flip their polarity, and their slab breaks off.
    pub(super) fn end_pass(&mut self, dt: f64, flip_time: f64, rotations: &[DQuat]) {
        for (&key, pair) in &mut self.pairs {
            for segment in &mut pair.segments {
                // Mostly arrivals, and clearly more than would arrive after a flip (hysteresis).
                let arriving = segment.overlaps > 0
                    && 2 * segment.arrivals > segment.overlaps
                    && segment.arrivals > 2 * segment.reverse_arrivals;
                if arriving {
                    segment.arrival_time += dt;
                } else {
                    segment.arrival_time = (segment.arrival_time - dt).max(0.0);
                }
                if segment.arrival_time >= flip_time {
                    let position = world(segment, rotations);
                    segment.overriding = other_of(key, segment.overriding);
                    segment.anchor = rotations[segment.overriding as usize].inverse() * position;
                    segment.slab_length = 0.0;
                    segment.arrival_time = 0.0;
                    self.statistics.flips += 1;
                }
                segment.overlaps = 0;
                segment.arrivals = 0;
                segment.reverse_arrivals = 0;
            }
        }
    }

    /// Ends a step at `time`: re-centres segments on their contacts, merges segments of a pair
    /// that came close, and forgets segments without contact or trench for `memory` Myr and
    /// pairs without segments and contact.
    pub(super) fn end_step(&mut self, time: f64, memory: f64, rotations: &[DQuat]) {
        let merge_dot = (0.5 * self.radius).cos();
        let statistics = &mut self.statistics;
        for pair in self.pairs.values_mut() {
            for segment in &mut pair.segments {
                if segment.contact_sum != DVec3::ZERO {
                    let current = world(segment, rotations);
                    let centre = segment.contact_sum.normalize();
                    let position = (current * (1.0 - RECENTRE_WEIGHT) + centre * RECENTRE_WEIGHT)
                        .normalize_or(current);
                    segment.anchor = rotations[segment.overriding as usize].inverse() * position;
                    segment.contact_sum = DVec3::ZERO;
                }
            }
            // Merge close segments of the same polarity.
            let mut i = 0;
            while i < pair.segments.len() {
                let mut j = i + 1;
                while j < pair.segments.len() {
                    let (first, second) = (&pair.segments[i], &pair.segments[j]);
                    let close = world(first, rotations).dot(world(second, rotations)) > merge_dot;
                    // Segments of opposite polarity stay apart (a fresh flip must survive).
                    if close && first.overriding == second.overriding {
                        let removed = pair.segments.remove(j);
                        let kept = &mut pair.segments[i];
                        if removed.slab_length > kept.slab_length {
                            kept.anchor = removed.anchor;
                            kept.slab_length = removed.slab_length;
                        }
                        kept.last_contact = kept.last_contact.max(removed.last_contact);
                        kept.arrival_time = kept.arrival_time.max(removed.arrival_time);
                        kept.initiation = match (kept.initiation, removed.initiation) {
                            (Some(a), Some(b)) => Some(a || b),
                            (a, b) => a.or(b),
                        };
                        statistics.merged += 1;
                    } else {
                        j += 1;
                    }
                }
                i += 1;
            }
            let before = pair.segments.len();
            // Segments with a trench count as in contact (see `update_slab_lengths`), so this
            // only drops segments left away from the boundary.
            pair.segments
                .retain(|segment| time - segment.last_contact <= memory);
            statistics.expired += before - pair.segments.len();
        }
        self.pairs
            .retain(|_, pair| !pair.segments.is_empty() || time - pair.last_contact <= memory);
    }

    /// Sets up `subduction` in the segment of the pair at the world position `r`, or in a new one
    /// there. Returns whether the other plate overrode it there before with a slab (a reversal).
    pub(super) fn set(
        &mut self,
        pair_plates: (u32, u32),
        subduction: Subduction,
        r: DVec3,
        time: f64,
        rotations: &[DQuat],
    ) -> bool {
        let Subduction {
            overriding,
            slab_length,
            initiation,
        } = subduction;
        let key = pair_key(pair_plates.0, pair_plates.1);
        let radius = self.radius;
        let found = self
            .pairs
            .get(&key)
            .and_then(|pair| nearest(&pair.segments, r, radius, rotations));
        let reversal;
        let index = match found {
            Some(index) => {
                let segment = &mut self.pairs.get_mut(&key).expect("found").segments[index];
                reversal = segment.overriding != overriding && segment.slab_length > 0.0;
                if segment.overriding != overriding {
                    let position = world(segment, rotations);
                    segment.overriding = overriding;
                    segment.anchor = rotations[overriding as usize].inverse() * position;
                    segment.arrival_time = 0.0;
                }
                segment.slab_length = slab_length;
                segment.last_contact = time;
                index
            }
            None => {
                reversal = false;
                let segment = self.new_segment(r, overriding, slab_length, time, rotations);
                let pair = self.pairs.entry(key).or_default();
                pair.segments.push(segment);
                pair.segments.len() - 1
            }
        };
        let pair = self.pairs.get_mut(&key).expect("pair exists");
        pair.last_contact = pair.last_contact.max(time);
        if initiation && pair.segments[index].initiation.is_none() {
            pair.segments[index].initiation = Some(false);
            self.statistics.initiation_segments += 1;
        }
        reversal
    }

    /// Like [`Self::set`], but leaves an existing segment at `r` unchanged.
    pub(super) fn seed(
        &mut self,
        pair_plates: (u32, u32),
        overriding: u32,
        r: DVec3,
        slab_length: f64,
        time: f64,
        rotations: &[DQuat],
    ) {
        if self
            .segment_at(pair_plates.0, pair_plates.1, r, rotations)
            .is_none()
        {
            let subduction = Subduction {
                overriding,
                slab_length,
                initiation: false,
            };
            self.set(pair_plates, subduction, r, time, rotations);
        }
    }

    /// Records that a segment's slab grew; counts successful initiations.
    pub(super) fn note_slab_growth(statistics: &mut SegmentStatistics, segment: &mut Segment) {
        if segment.initiation == Some(false) && segment.slab_length >= SUCCESSFUL_SLAB_LENGTH {
            segment.initiation = Some(true);
            statistics.successful_initiation_segments += 1;
        }
    }

    /// Segment radius as an angle (radians).
    pub(super) fn radius(&self) -> f64 {
        self.radius
    }

    /// Plate `plate` was split into itself and `new_plate` (with the same rotation). Segments of
    /// its pairs go to the plate(s) `side` gives for their world position (both near a split
    /// line); copies get their own id. The new plate has no history of collision with its
    /// neighbours.
    pub(super) fn split(
        &mut self,
        plate: u32,
        new_plate: u32,
        side: impl Fn(DVec3) -> Side,
        rotations: &[DQuat],
    ) {
        let keys: Vec<(u32, u32)> = self
            .pairs
            .keys()
            .copied()
            .filter(|&(a, b)| a == plate || b == plate)
            .collect();
        let mut new_pairs = Vec::new();
        for key in keys {
            let other = other_of(key, plate);
            let pair = self.pairs.get_mut(&key).expect("key exists");
            let mut staying = Vec::new();
            let mut moving = Vec::new();
            for segment in pair.segments.drain(..) {
                let side = side(world(&segment, rotations));
                if side.new {
                    let mut copy = segment.clone();
                    if side.old {
                        // Counted once, on the staying side.
                        copy.initiation = None;
                    }
                    if copy.overriding == plate {
                        // Same rotation, so the anchor stays valid.
                        copy.overriding = new_plate;
                    }
                    moving.push(copy);
                }
                if side.old || !side.new {
                    staying.push(segment);
                }
            }
            pair.segments = staying;
            new_pairs.push((
                pair_key(new_plate, other),
                PairState {
                    last_contact: pair.last_contact,
                    collision_time: 0.0,
                    segments: moving,
                },
            ));
        }
        for (key, mut pair) in new_pairs {
            for segment in &mut pair.segments {
                // Copies need their own identity.
                if self
                    .pairs
                    .values()
                    .any(|p| p.segments.iter().any(|s| s.id == segment.id))
                {
                    segment.id = self.next_id;
                    self.next_id += 1;
                }
            }
            self.pairs.insert(key, pair);
        }
    }

    /// Plate `absorbed` is merged into `into`: its segments with other plates move to the pairs
    /// of `into`; the pair of the two disappears. Call before the plates are renumbered.
    pub(super) fn merge(&mut self, into: u32, absorbed: u32, rotations: &[DQuat]) {
        self.pairs.remove(&pair_key(into, absorbed));
        let mut keys: Vec<(u32, u32)> = self
            .pairs
            .keys()
            .copied()
            .filter(|&(a, b)| a == absorbed || b == absorbed)
            .collect();
        keys.sort_unstable();
        for key in keys {
            let other = other_of(key, absorbed);
            let mut pair = self.pairs.remove(&key).expect("key exists");
            for segment in &mut pair.segments {
                if segment.overriding == absorbed {
                    let position = world(segment, rotations);
                    segment.overriding = into;
                    segment.anchor = rotations[into as usize].inverse() * position;
                }
            }
            let target = self.pairs.entry(pair_key(into, other)).or_default();
            target.last_contact = target.last_contact.max(pair.last_contact);
            target.collision_time = target.collision_time.max(pair.collision_time);
            target.segments.append(&mut pair.segments);
        }
    }

    /// Plate `plate` was removed and plate `last` took its index.
    pub(super) fn remove_plate(&mut self, plate: u32, last: u32) {
        self.pairs.retain(|&(a, b), _| a != plate && b != plate);
        if plate == last {
            return;
        }
        let renumber = |index: u32| if index == last { plate } else { index };
        self.pairs = self
            .pairs
            .drain()
            .map(|((a, b), mut pair)| {
                for segment in &mut pair.segments {
                    segment.overriding = renumber(segment.overriding);
                }
                (pair_key(renumber(a), renumber(b)), pair)
            })
            .collect();
    }

    /// A comparable description of the full state, for tests.
    #[cfg(test)]
    pub(super) fn snapshot(&self) -> Vec<PairSnapshot> {
        let mut pairs: Vec<_> = self
            .pairs
            .iter()
            .map(|(&key, pair)| {
                let mut segments: Vec<[u64; 7]> = pair
                    .segments
                    .iter()
                    .map(|s| {
                        [
                            u64::from(s.id),
                            u64::from(s.overriding),
                            s.anchor.x.to_bits(),
                            s.anchor.y.to_bits(),
                            s.slab_length.to_bits(),
                            s.arrival_time.to_bits(),
                            s.last_contact.to_bits(),
                        ]
                    })
                    .collect();
                segments.push([
                    pair.last_contact.to_bits(),
                    pair.collision_time.to_bits(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]);
                (key, segments)
            })
            .collect();
        pairs.sort_unstable_by_key(|(key, _)| *key);
        pairs
    }
}

#[cfg(test)]
impl Polarities {
    pub(super) fn clear(&mut self) {
        self.pairs.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// The segments of the pair `(a, b)`.
    pub(super) fn pair_segments(&self, a: u32, b: u32) -> &[Segment] {
        self.pairs
            .get(&pair_key(a, b))
            .map_or(&[], |pair| &pair.segments)
    }

    /// The longest slab of the pair `(a, b)`.
    pub(super) fn max_slab(&self, a: u32, b: u32) -> f64 {
        self.pair_segments(a, b)
            .iter()
            .map(|segment| segment.slab_length)
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::CONTINENTAL_DENSITY;
    use crate::sphere::SphereMesh;

    const RADIUS: f64 = 0.08;

    fn continent() -> Crust {
        Crust {
            thickness: 35_000.0,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
        }
    }

    fn ocean(age: f64) -> Crust {
        new_oceanic_crust(-age)
    }

    fn sub(overriding: u32, slab_length: f64) -> Subduction {
        Subduction {
            overriding,
            slab_length,
            initiation: false,
        }
    }

    fn identity() -> Vec<DQuat> {
        vec![DQuat::IDENTITY; 3]
    }

    #[test]
    fn the_denser_plate_subducts_on_first_contact() {
        let mut polarities = Polarities::new(RADIUS);
        let top = polarities.contact(
            (0, ocean(100.0)),
            (1, ocean(10.0)),
            DVec3::X,
            0.0,
            &identity(),
        );
        assert_eq!(top, 1);
        assert_eq!(polarities.pair_segments(0, 1)[0].overriding, 1);
    }

    #[test]
    fn continental_crust_stays_on_top_without_flipping_within_a_pass() {
        let mut polarities = Polarities::new(RADIUS);
        let rotations = identity();
        polarities.set((0, 1), sub(1, 600e3), DVec3::X, 0.0, &rotations);
        // Continental crust of both plates meets oceanic crust of the other at different cells of
        // the same segment, alternating.
        let near = |i: usize| (DVec3::X + DVec3::Y * (0.005 * i as f64)).normalize();
        for i in 0..10 {
            let top = if i % 2 == 0 {
                polarities.contact((0, continent()), (1, ocean(50.0)), near(i), 0.0, &rotations)
            } else {
                polarities.contact((0, ocean(50.0)), (1, continent()), near(i), 0.0, &rotations)
            };
            // The continental crust is on top at every cell.
            assert_eq!(top, if i % 2 == 0 { 0 } else { 1 });
        }
        let segments = polarities.pair_segments(0, 1);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].overriding, 1);
        assert_eq!(segments[0].slab_length, 600e3);
        // Mixed arrivals never flip the segment.
        for _ in 0..20 {
            for i in 0..10 {
                if i % 2 == 0 {
                    polarities.contact(
                        (0, continent()),
                        (1, ocean(50.0)),
                        near(i),
                        0.0,
                        &rotations,
                    );
                } else {
                    polarities.contact(
                        (0, ocean(50.0)),
                        (1, continent()),
                        near(i),
                        0.0,
                        &rotations,
                    );
                }
            }
            polarities.end_pass(1.0, 3.0, &rotations);
        }
        assert_eq!(polarities.statistics.flips, 0);
        assert_eq!(polarities.pair_segments(0, 1)[0].slab_length, 600e3);
    }

    #[test]
    fn sustained_continental_arrival_flips_one_segment() {
        let mut polarities = Polarities::new(RADIUS);
        // Plate 1 rotated, so that the anchor conversion matters.
        let rotations = vec![
            DQuat::IDENTITY,
            DQuat::from_rotation_z(0.3),
            DQuat::IDENTITY,
        ];
        polarities.set((0, 1), sub(1, 600e3), DVec3::X, 0.0, &rotations);
        polarities.set((0, 1), sub(1, 600e3), DVec3::Y, 0.0, &rotations);
        let pass = |polarities: &mut Polarities| {
            polarities.contact(
                (0, continent()),
                (1, ocean(50.0)),
                DVec3::X,
                0.0,
                &rotations,
            );
            polarities.contact(
                (0, ocean(50.0)),
                (1, ocean(10.0)),
                DVec3::Y,
                0.0,
                &rotations,
            );
            polarities.end_pass(1.0, 3.0, &rotations);
        };
        // A brief arrival does not flip.
        pass(&mut polarities);
        pass(&mut polarities);
        polarities.end_pass(1.0, 3.0, &rotations);
        polarities.end_pass(1.0, 3.0, &rotations);
        assert_eq!(polarities.statistics.flips, 0);
        // A sustained one does, only at the arriving segment, which keeps its position.
        for _ in 0..3 {
            pass(&mut polarities);
        }
        assert_eq!(polarities.statistics.flips, 1);
        let flipped = polarities.segment_at(0, 1, DVec3::X, &rotations).unwrap();
        assert_eq!(flipped.overriding, 0);
        assert_eq!(flipped.slab_length, 0.0);
        assert!(world(flipped, &rotations).dot(DVec3::X) > 0.999_999);
        let other = polarities.segment_at(0, 1, DVec3::Y, &rotations).unwrap();
        assert_eq!(other.overriding, 1);
        assert_eq!(other.slab_length, 600e3);
    }

    #[test]
    fn new_segments_inherit_the_polarity_along_strike() {
        let mut polarities = Polarities::new(RADIUS);
        let rotations = identity();
        polarities.set((0, 1), sub(0, 600e3), DVec3::X, 0.0, &rotations);
        // Just beyond the segment radius, plate 0 would be denser, but the neighbour decides.
        let r = (DVec3::X + DVec3::Y * (1.5 * RADIUS).tan()).normalize();
        let top = polarities.contact((0, ocean(150.0)), (1, ocean(10.0)), r, 0.0, &rotations);
        assert_eq!(top, 0);
        assert_eq!(polarities.pair_segments(0, 1).len(), 2);
        assert_eq!(polarities.pair_segments(0, 1)[1].slab_length, 0.0);
    }

    #[test]
    fn segments_away_from_the_boundary_expire() {
        let mut polarities = Polarities::new(RADIUS);
        let rotations = identity();
        polarities.set((0, 1), sub(1, 100e3), DVec3::X, 0.0, &rotations);
        polarities.set((0, 1), sub(1, 0.0), DVec3::Y, 90.0, &rotations);
        polarities.end_step(100.0, 20.0, &rotations);
        let segments = polarities.pair_segments(0, 1);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].last_contact, 90.0);
    }

    #[test]
    fn a_fresh_flip_is_not_merged_away() {
        let mut polarities = Polarities::new(RADIUS);
        let rotations = identity();
        polarities.set((0, 1), sub(1, 600e3), DVec3::X, 0.0, &rotations);
        let close = (DVec3::X + DVec3::Y * (0.3 * RADIUS).tan()).normalize();
        let segment = polarities.new_segment(close, 0, 0.0, 0.0, &rotations);
        polarities
            .pairs
            .get_mut(&(0, 1))
            .unwrap()
            .segments
            .push(segment);
        polarities.end_step(0.0, 20.0, &rotations);
        let segments = polarities.pair_segments(0, 1);
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn close_segments_of_the_same_polarity_merge() {
        let mut polarities = Polarities::new(RADIUS);
        let rotations = identity();
        polarities.set((0, 1), sub(1, 100e3), DVec3::X, 0.0, &rotations);
        let close = (DVec3::X + DVec3::Y * (0.6 * RADIUS).tan()).normalize();
        // Created directly, as a separate segment just outside the first one's reach.
        let segment = polarities.new_segment(close, 1, 300e3, 0.0, &rotations);
        polarities
            .pairs
            .get_mut(&(0, 1))
            .unwrap()
            .segments
            .push(segment);
        polarities.end_step(0.0, 20.0, &rotations);
        let segments = polarities.pair_segments(0, 1);
        // 0.6 R apart: not merged (merge distance R / 2).
        assert_eq!(segments.len(), 2);
        let closer = (DVec3::X + DVec3::Y * (0.3 * RADIUS).tan()).normalize();
        let segment = polarities.new_segment(closer, 1, 200e3, 0.0, &rotations);
        polarities
            .pairs
            .get_mut(&(0, 1))
            .unwrap()
            .segments
            .push(segment);
        polarities.end_step(0.0, 20.0, &rotations);
        let segments = polarities.pair_segments(0, 1);
        assert_eq!(segments.len(), 2);
        assert_eq!(polarities.max_slab(0, 1), 300e3);
    }

    #[test]
    fn splits_and_merges_carry_segments() {
        let mut polarities = Polarities::new(RADIUS);
        let rotations = identity();
        // Plate 0 overrides plate 2 at +x and at -x.
        polarities.set((0, 2), sub(0, 600e3), DVec3::X, 0.0, &rotations);
        polarities.set((0, 2), sub(0, 300e3), -DVec3::X, 0.0, &rotations);
        // Split plate 0 at the plane x = 0 into 0 (-x) and 1 (+x).
        polarities.split(
            0,
            1,
            |r| Side {
                new: r.x > 0.0,
                old: r.x <= 0.0,
            },
            &rotations,
        );
        assert_eq!(polarities.pair_segments(0, 2).len(), 1);
        assert_eq!(polarities.pair_segments(0, 2)[0].slab_length, 300e3);
        assert_eq!(polarities.pair_segments(1, 2).len(), 1);
        assert_eq!(polarities.pair_segments(1, 2)[0].overriding, 1);
        assert_eq!(polarities.pair_segments(1, 2)[0].slab_length, 600e3);
        // Merge 1 back into 0.
        polarities.merge(0, 1, &rotations);
        assert_eq!(polarities.pair_segments(0, 2).len(), 2);
        assert!(
            polarities
                .pair_segments(0, 2)
                .iter()
                .all(|s| s.overriding == 0)
        );
        assert!(polarities.pair_segments(1, 2).is_empty());
        // Removing plate 1 renumbers plate 2.
        polarities.remove_plate(1, 2);
        assert_eq!(polarities.pair_segments(0, 1).len(), 2);
    }

    #[test]
    fn initial_subduction_zones_have_slabs_before_the_first_step() {
        let simulation = Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), 1);
        let max = simulation.params.max_slab_length;
        assert!(
            simulation
                .polarities
                .segments()
                .any(|s| s.slab_length == max)
        );
        let boundaries = simulation.boundary_cells();
        assert!(boundaries.iter().any(|b| matches!(
            b.kind,
            super::super::dynamics::BoundaryKind::Trench { .. }
        ) || b.segment.is_some()));
    }

    #[test]
    fn segment_counts_stay_bounded_on_a_coarse_mesh() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), 2);
        for _ in 0..60 {
            simulation.step(1.0);
        }
        let boundary_cells = simulation.boundary_cells().len();
        let segments = simulation.polarities.segments().count();
        // Each segment covers at least a few cells of boundary.
        assert!(
            segments * 3 < boundary_cells,
            "{segments} segments, {boundary_cells} cells"
        );
    }
}
