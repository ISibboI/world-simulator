//! Fluvial erosion, sediment transport and deposition.
//!
//! 1. **Drainage.** A priority flood from the ocean assigns every land cell a receiver, the cell
//!    its water drains into. Water in closed basins (lakes) spills over the lowest pass, so all
//!    rivers reach the ocean.
//! 2. **Discharge.** Precipitation is accumulated downstream along the receivers.
//! 3. **Incision.** Rivers cut into the land following the stream-power law
//!    `E = K · Q^m · S`, solved implicitly (Braun & Willett, 2013), which is stable for any time
//!    step. Loose sediment is removed before bedrock. Removing bedrock thins the crust, which then
//!    rises isostatically, so the surface only drops by a fraction of the removed thickness.
//! 4. **Deposition.** Sediment is carried downstream. It fills lakes, and from river mouths it
//!    fills the ocean outwards up to [`ErosionParams::shelf_depth`], so continental shelves
//!    prograde into the ocean.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::sphere::SphereMesh;

/// Parameters of erosion. Lengths in metres, times in years unless noted otherwise.
#[derive(Clone, Debug)]
pub struct ErosionParams {
    /// Planet radius in metres.
    pub planet_radius: f64,
    /// Erodibility `K` of the stream-power law, in m^(1-3m) yr^(m-1).
    pub erodibility: f64,
    /// Discharge exponent `m` of the stream-power law.
    pub discharge_exponent: f64,
    /// Fraction of eroded bedrock thickness by which the surface drops; the rest is compensated by
    /// isostatic rebound. Equal to `1 - crust density / mantle density`.
    pub isostatic_factor: f64,
    /// Sediment fills the ocean up to this elevation (negative, below sea level).
    pub shelf_depth: f64,
    /// Maximum number of ocean cells the sediment of one river mouth spreads over.
    pub max_shelf_cells: usize,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            planet_radius: 6.371e6,
            erodibility: 1.5e-7,
            discharge_exponent: 0.5,
            isostatic_factor: 1.0 - 2700.0 / 3300.0,
            shelf_depth: -120.0,
            max_shelf_cells: 256,
        }
    }
}

/// Changes caused by one erosion step, and diagnostic fields.
pub struct ErosionResult {
    /// Change of the bedrock surface elevation of each cell in metres.
    pub bedrock_delta: Vec<f64>,
    /// Change of the sediment thickness of each cell in metres.
    pub sediment_delta: Vec<f64>,
    /// Water discharge through each cell in m³/yr.
    pub discharge: Vec<f64>,
    /// Total sediment delivered to the ocean, in m³.
    pub sediment_to_ocean: f64,
}

/// Performs one erosion step of `dt_years` years.
///
/// `elevation` is the surface elevation (bedrock plus sediment), `sediment` the thickness of
/// loose sediment, and `precipitation` the precipitation in m/yr, all per mesh cell.
pub fn erode(
    mesh: &SphereMesh,
    elevation: &[f64],
    sediment: &[f64],
    precipitation: &[f64],
    params: &ErosionParams,
    dt_years: f64,
) -> ErosionResult {
    let cell_count = mesh.len();
    let cell_area = 4.0 * std::f64::consts::PI * params.planet_radius.powi(2) / cell_count as f64;
    let is_ocean = |cell: usize| elevation[cell] < 0.0;

    let Drainage {
        receiver,
        filled,
        order,
    } = drainage(mesh, elevation);

    // Discharge, accumulated from the sources downstream.
    let mut discharge = vec![0.0; cell_count];
    for cell in 0..cell_count {
        if !is_ocean(cell) {
            discharge[cell] = precipitation[cell].max(0.0) * cell_area;
        }
    }
    for &cell in order.iter().rev() {
        if let Some(receiver) = receiver[cell as usize] {
            discharge[receiver as usize] += discharge[cell as usize];
        }
    }

    // Implicit stream-power incision, from the ocean upstream so receivers are updated first.
    let mut new_elevation = elevation.to_vec();
    let mut bedrock_delta = vec![0.0; cell_count];
    let mut sediment_delta = vec![0.0; cell_count];
    let mut produced = vec![0.0; cell_count];
    for &cell in &order {
        let cell = cell as usize;
        let Some(receiver) = receiver[cell] else {
            continue;
        };
        let receiver_elevation = new_elevation[receiver as usize];
        if elevation[cell] <= receiver_elevation {
            continue; // Lake or flat: no incision.
        }
        let distance = mesh
            .vertex(cell as u32)
            .angle_between(mesh.vertex(receiver))
            * params.planet_radius;
        let factor =
            params.erodibility * dt_years * discharge[cell].powf(params.discharge_exponent)
                / distance;
        let target = (elevation[cell] + factor * receiver_elevation) / (1.0 + factor);
        let thickness = elevation[cell] - target;

        let from_sediment = thickness.min(sediment[cell]);
        let from_bedrock = thickness - from_sediment;
        sediment_delta[cell] -= from_sediment;
        bedrock_delta[cell] -= from_bedrock * params.isostatic_factor;
        produced[cell] = from_sediment + from_bedrock;
        new_elevation[cell] =
            elevation[cell] - from_sediment - from_bedrock * params.isostatic_factor;
    }

    // Route sediment downstream, filling lakes on the way.
    let mut flux = produced;
    let mut delivered = vec![0.0; cell_count];
    for &cell in order.iter().rev() {
        let cell = cell as usize;
        if is_ocean(cell) {
            continue;
        }
        let lake_room = if filled[cell] > elevation[cell] {
            (filled[cell] - new_elevation[cell]).max(0.0)
        } else {
            0.0
        };
        let deposit = flux[cell].min(lake_room);
        sediment_delta[cell] += deposit;
        new_elevation[cell] += deposit;
        let remaining = flux[cell] - deposit;
        match receiver[cell] {
            Some(receiver) if is_ocean(receiver as usize) => {
                delivered[receiver as usize] += remaining;
            }
            Some(receiver) => flux[receiver as usize] += remaining,
            None => {
                // A land cell without receiver only exists on planets without ocean.
                sediment_delta[cell] += remaining;
                new_elevation[cell] += remaining;
            }
        }
    }

    // Build shelves at river mouths: fill the ocean outwards from the mouth up to the shelf
    // depth, so that shelves prograde into the ocean.
    let mut sediment_to_ocean = 0.0;
    let mut visited_from = vec![usize::MAX; cell_count];
    let mut queue = VecDeque::new();
    let mut visited = Vec::new();
    for (mouth, &delivered) in delivered.iter().enumerate() {
        let mut remaining = delivered;
        if remaining <= 0.0 {
            continue;
        }
        sediment_to_ocean += remaining * cell_area;
        queue.clear();
        visited.clear();
        queue.push_back(mouth);
        visited_from[mouth] = mouth;
        while let Some(cell) = queue.pop_front() {
            visited.push(cell);
            let room = (params.shelf_depth - new_elevation[cell]).max(0.0);
            let deposit = remaining.min(room);
            sediment_delta[cell] += deposit;
            new_elevation[cell] += deposit;
            remaining -= deposit;
            if remaining <= 0.0 || visited.len() >= params.max_shelf_cells {
                break;
            }
            for &neighbor in mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                if is_ocean(neighbor) && visited_from[neighbor] != mouth {
                    visited_from[neighbor] = mouth;
                    queue.push_back(neighbor);
                }
            }
        }
        // When the shelf within reach is full, the delta builds up to sea level, which also
        // fills enclosed basins below sea level. Sediment beyond that is carried into the deep
        // ocean, beyond the reach of this model.
        for &cell in &visited {
            if remaining <= 0.0 {
                break;
            }
            let deposit = remaining.min((-new_elevation[cell]).max(0.0));
            sediment_delta[cell] += deposit;
            new_elevation[cell] += deposit;
            remaining -= deposit;
        }
    }

    ErosionResult {
        bedrock_delta,
        sediment_delta,
        discharge,
        sediment_to_ocean,
    }
}

struct Drainage {
    /// The cell each cell drains into; `None` for ocean cells.
    receiver: Vec<Option<u32>>,
    /// Elevation of the water surface: the cell elevation, or the spill level of a lake.
    filled: Vec<f64>,
    /// Cells ordered such that every cell comes after its receiver.
    order: Vec<u32>,
}

/// Priority-flood drainage (Barnes et al., 2014), starting from all ocean cells.
fn drainage(mesh: &SphereMesh, elevation: &[f64]) -> Drainage {
    /// Minimal rise of lake surfaces per cell, so that flow directions inside lakes are defined.
    const EPSILON: f64 = 1e-3;

    let cell_count = mesh.len();
    let mut receiver = vec![None; cell_count];
    let mut filled = elevation.to_vec();
    let mut visited = vec![false; cell_count];
    let mut order = Vec::with_capacity(cell_count);
    let mut heap = BinaryHeap::new();

    for cell in 0..cell_count {
        if elevation[cell] < 0.0 {
            visited[cell] = true;
            heap.push(Reverse((OrderedFloat(elevation[cell]), cell as u32)));
        }
    }
    if heap.is_empty() {
        let lowest = (0..cell_count)
            .min_by(|&a, &b| elevation[a].total_cmp(&elevation[b]))
            .expect("mesh has cells");
        visited[lowest] = true;
        heap.push(Reverse((OrderedFloat(elevation[lowest]), lowest as u32)));
    }

    while let Some(Reverse((OrderedFloat(level), cell))) = heap.pop() {
        order.push(cell);
        for &neighbor in mesh.neighbors(cell) {
            let n = neighbor as usize;
            if visited[n] {
                continue;
            }
            visited[n] = true;
            receiver[n] = Some(cell);
            filled[n] = elevation[n].max(level + EPSILON);
            heap.push(Reverse((OrderedFloat(filled[n]), neighbor)));
        }
    }

    Drainage {
        receiver,
        filled,
        order,
    }
}

#[derive(Clone, Copy)]
struct OrderedFloat(f64);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cone-shaped island with a crater lake on one flank, surrounded by ocean.
    fn island(mesh: &SphereMesh) -> Vec<f64> {
        (0..mesh.len() as u32)
            .map(|cell| {
                let angle = mesh.vertex(cell).angle_between(glam::DVec3::Z);
                3000.0 - angle.to_degrees() * 150.0
            })
            .collect()
    }

    #[test]
    fn all_water_reaches_the_ocean() {
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        // Dig a pit on the flank.
        let pit = mesh.nearest(glam::DVec3::new(0.05, 0.0, 1.0).normalize()) as usize;
        elevation[pit] -= 1500.0;
        assert!(
            mesh.neighbors(pit as u32)
                .iter()
                .all(|&n| elevation[n as usize] > elevation[pit])
        );
        let precipitation = vec![1.0; mesh.len()];
        let result = erode(
            &mesh,
            &elevation,
            &vec![0.0; mesh.len()],
            &precipitation,
            &ErosionParams::default(),
            1e6,
        );

        let cell_area = 4.0 * std::f64::consts::PI * 6.371e6f64.powi(2) / mesh.len() as f64;
        let land_cells = elevation.iter().filter(|&&e| e >= 0.0).count() as f64;
        let into_ocean: f64 = (0..mesh.len())
            .filter(|&c| elevation[c] < 0.0)
            .map(|c| result.discharge[c])
            .sum();
        assert!((into_ocean - land_cells * cell_area).abs() / into_ocean < 1e-9);

        // The pit collects sediment.
        assert!(result.sediment_delta[pit] > 0.0);
    }

    #[test]
    fn erosion_lowers_land_and_builds_shelves() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let precipitation = vec![1.0; mesh.len()];
        let result = erode(
            &mesh,
            &elevation,
            &vec![0.0; mesh.len()],
            &precipitation,
            &ErosionParams::default(),
            1e6,
        );

        assert!(result.bedrock_delta.iter().all(|&d| d <= 0.0));
        assert!(result.bedrock_delta.iter().any(|&d| d < 0.0));
        assert!(result.sediment_to_ocean > 0.0);
        // Sediment ends up in the ocean, building the shelf around the island.
        assert!((0..mesh.len()).any(|c| elevation[c] < 0.0 && result.sediment_delta[c] > 0.0));
        // Land is not raised except by lake deposits; this island has no lakes.
        for cell in 0..mesh.len() {
            if elevation[cell] >= 0.0 {
                assert!(result.bedrock_delta[cell] + result.sediment_delta[cell] <= 1e-9);
            }
        }
    }
}
