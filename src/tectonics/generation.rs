//! Random initial conditions: plate partition, continents, plate motions.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use glam::DVec3;
use rand::{Rng, RngExt};

use crate::sphere::SphereMesh;

use super::*;

pub(super) fn random_angular_velocity(params: &TectonicParams, rng: &mut impl Rng) -> DVec3 {
    let (min, max) = params.angular_speed_range;
    random_unit_vector(rng) * rng.random_range(min..=max)
}

pub(super) fn random_unit_vector(rng: &mut impl Rng) -> DVec3 {
    loop {
        let v = DVec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
        );
        let length_squared = v.length_squared();
        if length_squared > 1e-6 && length_squared <= 1.0 {
            return v / length_squared.sqrt();
        }
    }
}

pub(super) fn sample_distinct_cells(
    mesh: &SphereMesh,
    count: usize,
    rng: &mut impl Rng,
) -> Vec<u32> {
    let count = count.min(mesh.len());
    let mut cells = Vec::with_capacity(count);
    while cells.len() < count {
        let cell = rng.random_range(0..mesh.len() as u32);
        if !cells.contains(&cell) {
            cells.push(cell);
        }
    }
    cells
}

/// Grows regions from `seeds` with randomised edge costs, until `limit` cells are assigned.
/// Regions with a larger scale grow faster. Unassigned cells are `u32::MAX`.
pub(super) fn grow_regions(
    mesh: &SphereMesh,
    seeds: &[u32],
    scales: &[f64],
    limit: usize,
    rng: &mut impl Rng,
) -> Vec<u32> {
    let mut region = vec![u32::MAX; mesh.len()];
    let mut assigned = 0;
    let mut heap = BinaryHeap::new();
    for (index, &seed) in seeds.iter().enumerate() {
        heap.push(Reverse((0u64, seed, index as u32)));
    }

    while let Some(Reverse((cost, cell, index))) = heap.pop() {
        if assigned >= limit {
            break;
        }
        if region[cell as usize] != u32::MAX {
            continue;
        }
        region[cell as usize] = index;
        assigned += 1;
        for &neighbor in mesh.neighbors(cell) {
            if region[neighbor as usize] == u32::MAX {
                let step = rng.random_range(1.0..100.0) / scales[index as usize];
                heap.push(Reverse((cost + step as u64 + 1, neighbor, index)));
            }
        }
    }
    region
}

/// Graph distance (in edges) of every cell to the closest cell satisfying `is_source`.
/// Cells are at distance `u32::MAX` if there is no source.
pub(super) fn distance_to(mesh: &SphereMesh, is_source: impl Fn(u32) -> bool) -> Vec<u32> {
    let mut distance = vec![u32::MAX; mesh.len()];
    let mut queue = VecDeque::new();
    for cell in 0..mesh.len() as u32 {
        if is_source(cell) {
            distance[cell as usize] = 0;
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        for &neighbor in mesh.neighbors(cell) {
            if distance[neighbor as usize] == u32::MAX {
                distance[neighbor as usize] = distance[cell as usize] + 1;
                queue.push_back(neighbor);
            }
        }
    }
    distance
}
