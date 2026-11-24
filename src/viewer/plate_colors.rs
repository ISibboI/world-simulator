//! Colours of the plates for the plate render mode: every plate keeps its colour for its whole
//! life, and a new plate gets one that none of its neighbours has.
//!
//! The view assigns the colours as states arrive (live and recorded alike, before a frame can be
//! shown again from the history), by the plates' ids ([`super::PlateInfo::id`]), which survive
//! the renumbering of plates. A new plate takes, among the colours its current neighbours do not
//! have, the one fewest plates have (ties: by its id, so that colours vary). Colours never
//! change afterwards: when plates drift into contact later, two neighbours can share a colour;
//! the boundary line between them still separates them.

use super::{CellInfo, PlateInfo};
use crate::hash::{HashMap, HashSet};
use crate::sphere::SphereMesh;

/// The plate colours: muted, so that boundary lines and arrows stand out (sRGB). Based on
/// ColorBrewer's "Set3" (12 colours) with four more of similar lightness.
pub const PALETTE: [[u8; 3]; 16] = [
    [141, 211, 199],
    [255, 255, 179],
    [190, 186, 218],
    [251, 128, 114],
    [128, 177, 211],
    [253, 180, 98],
    [179, 222, 105],
    [252, 205, 229],
    [217, 217, 217],
    [188, 128, 189],
    [204, 235, 197],
    [255, 237, 111],
    [229, 196, 148],
    [102, 194, 165],
    [196, 156, 148],
    [158, 218, 229],
];

/// The colour of every plate id seen in a run.
#[derive(Debug, Default, Clone)]
pub struct PlateColors {
    by_id: HashMap<u32, u8>,
}

impl PlateColors {
    /// Forgets all plates (a new simulation).
    pub fn clear(&mut self) {
        self.by_id.clear();
    }

    /// The colour of a plate id (one derived from the id if it was never seen).
    pub fn color(&self, id: u32) -> [u8; 3] {
        let index = self
            .by_id
            .get(&id)
            .map_or(id as usize % PALETTE.len(), |&index| usize::from(index));
        PALETTE[index]
    }

    /// Gives the plates of a state that have no colour yet one. Costs a pass over the cells only
    /// when there are new plates.
    pub fn update(&mut self, mesh: &SphereMesh, cells: &[CellInfo], plates: &[PlateInfo]) {
        let mut new: Vec<u32> = (0..plates.len() as u32)
            .filter(|&plate| !self.by_id.contains_key(&plates[plate as usize].id))
            .collect();
        if new.is_empty() || cells.len() != mesh.len() {
            return;
        }
        new.sort_unstable_by_key(|&plate| plates[plate as usize].id);
        // Neighbouring plates (by index) of the new plates, and the cells of every plate.
        let is_new: HashSet<u32> = new.iter().copied().collect();
        let mut neighbors: HashMap<u32, Vec<u32>> = HashMap::default();
        let mut present = vec![false; plates.len()];
        for (cell, info) in cells.iter().enumerate() {
            let Some(slot) = present.get_mut(info.plate as usize) else {
                continue;
            };
            *slot = true;
            if !is_new.contains(&info.plate) {
                continue;
            }
            for &neighbor in mesh.neighbors(cell as u32) {
                let other = cells[neighbor as usize].plate;
                if other != info.plate && (other as usize) < plates.len() {
                    let list = neighbors.entry(info.plate).or_default();
                    if !list.contains(&other) {
                        list.push(other);
                    }
                }
            }
        }
        // How many plates of the state have each colour.
        let mut uses = [0usize; PALETTE.len()];
        for (plate, info) in plates.iter().enumerate() {
            if present[plate]
                && let Some(&index) = self.by_id.get(&info.id)
            {
                uses[usize::from(index)] += 1;
            }
        }
        for plate in new {
            let id = plates[plate as usize].id;
            let taken: Vec<u8> = neighbors
                .get(&plate)
                .into_iter()
                .flatten()
                .filter_map(|&other| self.by_id.get(&plates[other as usize].id).copied())
                .collect();
            let start = id as usize % PALETTE.len();
            let index = (0..PALETTE.len())
                .map(|offset| ((start + offset) % PALETTE.len()) as u8)
                .min_by_key(|index| (taken.contains(index), uses[usize::from(*index)]))
                .expect("the palette has colours");
            uses[usize::from(index)] += 1;
            self.by_id.insert(id, index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(plate: u32) -> CellInfo {
        CellInfo {
            plate,
            ..bytemuck::Zeroable::zeroed()
        }
    }

    fn plates(ids: &[u32]) -> Vec<PlateInfo> {
        ids.iter()
            .map(|&id| PlateInfo {
                rotation: [0.0; 3],
                id,
                orientation: [0.0, 0.0, 0.0, 1.0],
            })
            .collect()
    }

    /// Cells of a mesh split into `count` plates by longitude.
    fn sectors(mesh: &SphereMesh, count: u32) -> Vec<CellInfo> {
        (0..mesh.len() as u32)
            .map(|vertex| {
                let v = mesh.vertex(vertex);
                let angle = v.y.atan2(v.x) + std::f64::consts::PI;
                cell(((angle / std::f64::consts::TAU * f64::from(count)) as u32).min(count - 1))
            })
            .collect()
    }

    fn neighbors_differ(
        colors: &PlateColors,
        mesh: &SphereMesh,
        cells: &[CellInfo],
        plates: &[PlateInfo],
    ) -> bool {
        (0..mesh.len() as u32).all(|vertex| {
            let plate = cells[vertex as usize].plate;
            mesh.neighbors(vertex).iter().all(|&neighbor| {
                let other = cells[neighbor as usize].plate;
                other == plate
                    || colors.color(plates[other as usize].id)
                        != colors.color(plates[plate as usize].id)
            })
        })
    }

    #[test]
    fn neighbours_differ_and_colours_stay() {
        let mesh = SphereMesh::icosphere(3);
        let cells = sectors(&mesh, 20);
        let ids: Vec<u32> = (0..20).collect();
        let plates = plates(&ids);
        let mut colors = PlateColors::default();
        colors.update(&mesh, &cells, &plates);
        assert!(neighbors_differ(&colors, &mesh, &cells, &plates));
        let before: Vec<[u8; 3]> = ids.iter().map(|&id| colors.color(id)).collect();

        // Renumbered (as when a plate is removed): indices 0 and 19 swap, the ids stay.
        let mut renumbered_plates = plates.clone();
        renumbered_plates.swap(0, 19);
        let renumbered: Vec<CellInfo> = cells
            .iter()
            .map(|info| match info.plate {
                0 => cell(19),
                19 => cell(0),
                other => cell(other),
            })
            .collect();
        colors.update(&mesh, &renumbered, &renumbered_plates);
        let after: Vec<[u8; 3]> = ids.iter().map(|&id| colors.color(id)).collect();
        assert_eq!(before, after);

        // Plate 3 splits: its eastern half becomes the new plate 20.
        let mut split = cells.clone();
        let mut split_plates = plates.clone();
        split_plates.push(PlateInfo {
            rotation: [0.0; 3],
            id: 20,
            orientation: [0.0, 0.0, 0.0, 1.0],
        });
        for (vertex, info) in split.iter_mut().enumerate() {
            if info.plate == 3 && mesh.vertex(vertex as u32).z > 0.0 {
                info.plate = 20;
            }
        }
        colors.update(&mesh, &split, &split_plates);
        assert_ne!(colors.color(20), colors.color(3));
        assert!(neighbors_differ(&colors, &mesh, &split, &split_plates));
        let unchanged: Vec<[u8; 3]> = ids.iter().map(|&id| colors.color(id)).collect();
        assert_eq!(before, unchanged);
        // A new simulation starts over.
        colors.clear();
        assert_eq!(colors.color(20), PALETTE[20 % PALETTE.len()]);
    }
}
