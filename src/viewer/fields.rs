//! Extra per-cell fields that render modes can request from the simulation, beyond the
//! [`CellInfo`] that every surface update carries.
//!
//! Every update carries the attributes of [`CellInfo`] (112 bytes per cell),
//! which cover the scalar modes (elevation, temperature, precipitation, crust, sediment, …). Fields that only
//! some modes need, e.g. vectors for arrows, are [`Layer`]s: the view tells the simulation which
//! layers the active mode needs ([`Layers`]), and the simulation extracts and sends only those
//! with its updates, so they cost nothing while no mode shows them.
//!
//! A layer is a flat array of `f32`, [`Layer::components`] per cell in the order of the cells,
//! so that it can be sent to the page on the web as one transferred buffer.
//!
//! Adding a layer: a variant of [`Layer`], its entries in [`Layer::ALL`], [`Layer::key`],
//! [`Layer::components`] and [`Layer::extract`], and how [`Fields::from_frame`] rebuilds it for a
//! past frame of the history (from the cells and plates every frame keeps; a layer that
//! cannot be rebuilt from them needs its data added to the frames, see `super::history`); the
//! transport (threads and the web worker) handles every layer alike.

use std::sync::Arc;

use super::CellInfo;
use crate::planet::Planet;
use crate::sphere::SphereMesh;

/// A field that render modes can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Velocity of the plate at every cell in cm/yr, as a vector in world coordinates
    /// (tangent to the sphere): 3 components.
    PlateVelocity,
}

impl Layer {
    /// Every layer, in the order of their bits in [`Layers`].
    pub const ALL: [Layer; 1] = [Layer::PlateVelocity];

    /// The bit of the layer in [`Layers`].
    pub fn bit(self) -> u32 {
        1 << self as u32
    }

    /// The name of the layer in messages to the page (web).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Used by the web version.
    pub fn key(self) -> &'static str {
        match self {
            Layer::PlateVelocity => "plate_velocity",
        }
    }

    /// The number of `f32` values per cell.
    pub fn components(self) -> usize {
        match self {
            Layer::PlateVelocity => 3,
        }
    }

    /// The values of the layer for the current state of the planet: [`Layer::components`] per
    /// cell.
    pub fn extract(self, planet: &Planet) -> Vec<f32> {
        let simulation = planet.tectonics();
        let mesh = simulation.mesh();
        match self {
            Layer::PlateVelocity => (0..mesh.len() as u32)
                .flat_map(|cell| {
                    let omega = simulation.angular_velocity(simulation.owner(cell).plate);
                    let velocity = omega.cross(mesh.vertex(cell)) * CM_PER_YEAR;
                    velocity.as_vec3().to_array()
                })
                .collect(),
        }
    }
}

/// What the view knows about a plate: sent for every plate (by index, as in
/// [`CellInfo::plate`]) with every update and kept in every frame of the history, so that the
/// layers and modes derived from plates work for any past frame ([`Fields::from_frame`]).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PlateInfo {
    /// Angular velocity in rad/Myr.
    pub rotation: [f32; 3],
    /// The plate's id, which stays while its index changes (see
    /// [`crate::tectonics::Simulation::plate_id`]): plate colours follow it.
    pub id: u32,
}

impl PlateInfo {
    /// The angular velocity in rad/Myr.
    pub fn omega(&self) -> glam::DVec3 {
        glam::Vec3::from(self.rotation).as_dvec3()
    }
}

/// The [`PlateInfo`] of every plate, by index.
pub fn plate_infos(planet: &Planet) -> Vec<PlateInfo> {
    let simulation = planet.tectonics();
    (0..simulation.plate_count() as u32)
        .map(|plate| PlateInfo {
            rotation: simulation.angular_velocity(plate).as_vec3().to_array(),
            id: simulation.plate_id(plate),
        })
        .collect()
}

/// rad/Myr × m → cm/yr.
const CM_PER_YEAR: f64 = crate::EARTH_RADIUS * 100.0 / 1e6;

/// A set of [`Layer`]s, as bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Layers(pub u32);

impl Layers {
    pub const NONE: Layers = Layers(0);

    pub fn of(layers: &[Layer]) -> Self {
        Layers(layers.iter().fold(0, |bits, layer| bits | layer.bit()))
    }

    pub fn contains(self, layer: Layer) -> bool {
        self.0 & layer.bit() != 0
    }

    /// The layers of the set, in the order of [`Layer::ALL`].
    pub fn iter(self) -> impl Iterator<Item = Layer> {
        Layer::ALL
            .into_iter()
            .filter(move |&layer| self.contains(layer))
    }
}

/// The values of the requested layers for one surface state.
#[derive(Debug, Clone, Default)]
pub struct Fields {
    values: Vec<(Layer, Arc<Vec<f32>>)>,
}

impl Fields {
    /// Extracts the `layers` from the planet.
    pub fn extract(planet: &Planet, layers: Layers) -> Self {
        Self {
            values: layers
                .iter()
                .map(|layer| (layer, Arc::new(layer.extract(planet))))
                .collect(),
        }
    }

    /// Every layer for a frame of the history, rebuilt from its cells and plates (see
    /// [`plate_infos`]) on `mesh`.
    pub fn from_frame(mesh: &SphereMesh, cells: &[CellInfo], plates: &[PlateInfo]) -> Self {
        let values = Layer::ALL
            .into_iter()
            .map(|layer| {
                let values = match layer {
                    Layer::PlateVelocity => cells
                        .iter()
                        .enumerate()
                        .flat_map(|(cell, info)| {
                            let omega = plates
                                .get(info.plate as usize)
                                .map_or(glam::DVec3::ZERO, PlateInfo::omega);
                            (omega.cross(mesh.vertex(cell as u32)) * CM_PER_YEAR)
                                .as_vec3()
                                .to_array()
                        })
                        .collect(),
                };
                (layer, Arc::new(values))
            })
            .collect();
        Self { values }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Used by the web version.
    /// Adds the values of a layer (received from the simulation worker). Values of the wrong
    /// length for `cells` cells are refused.
    pub fn insert(&mut self, layer: Layer, values: Vec<f32>, cells: usize) -> Result<(), String> {
        if values.len() != cells * layer.components() {
            return Err(format!(
                "layer {} has {} values for {cells} cells",
                layer.key(),
                values.len()
            ));
        }
        self.values.retain(|(other, _)| *other != layer);
        self.values.push((layer, Arc::new(values)));
        Ok(())
    }

    /// The values of a layer, if it was sent.
    pub fn get(&self, layer: Layer) -> Option<&[f32]> {
        self.values
            .iter()
            .find(|(other, _)| *other == layer)
            .map(|(_, values)| values.as_slice())
    }

    /// The layers present.
    pub fn layers(&self) -> Layers {
        Layers(
            self.values
                .iter()
                .fold(0, |bits, (layer, _)| bits | layer.bit()),
        )
    }

    /// Every layer with its values, e.g. to send them.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Used by the web version.
    pub fn iter(&self) -> impl Iterator<Item = (Layer, &[f32])> {
        self.values
            .iter()
            .map(|(layer, values)| (*layer, values.as_slice()))
    }

    /// The vector of a 3-component layer at a cell.
    pub fn vector(&self, layer: Layer, cell: u32) -> Option<glam::Vec3> {
        debug_assert_eq!(layer.components(), 3);
        let values = self.get(layer)?;
        let start = cell as usize * 3;
        values
            .get(start..start + 3)
            .map(|v| glam::Vec3::new(v[0], v[1], v[2]))
    }

    /// Bytes of all layers.
    pub fn bytes(&self) -> usize {
        self.values
            .iter()
            .map(|(_, values)| values.len() * std::mem::size_of::<f32>())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_sets() {
        assert_eq!(Layers::NONE.iter().count(), 0);
        let layers = Layers::of(&[Layer::PlateVelocity]);
        assert!(layers.contains(Layer::PlateVelocity));
        assert_eq!(layers.iter().collect::<Vec<_>>(), [Layer::PlateVelocity]);
        // Bits are distinct.
        let bits: Vec<u32> = Layer::ALL.iter().map(|layer| layer.bit()).collect();
        for (index, bit) in bits.iter().enumerate() {
            assert_eq!(bit.count_ones(), 1);
            assert!(!bits[..index].contains(bit));
        }
    }

    #[test]
    fn inserted_values_are_checked_and_found() {
        let mut fields = Fields::default();
        assert!(
            fields
                .insert(Layer::PlateVelocity, vec![0.0; 5], 2)
                .is_err()
        );
        fields
            .insert(Layer::PlateVelocity, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2)
            .unwrap();
        assert_eq!(fields.layers(), Layers::of(&[Layer::PlateVelocity]));
        assert_eq!(
            fields.vector(Layer::PlateVelocity, 1),
            Some(glam::Vec3::new(4.0, 5.0, 6.0))
        );
        assert_eq!(fields.vector(Layer::PlateVelocity, 2), None);
        assert_eq!(fields.bytes(), 24);
    }

    #[test]
    fn plate_velocities_of_a_small_planet() {
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "2"]);
        let mut planet = crate::build_planet(&cli).unwrap();
        // Plate velocities come from the first step's torque balance.
        planet.step(cli.dt);
        let fields = Fields::extract(&planet, Layers::of(&[Layer::PlateVelocity]));
        let simulation = planet.tectonics();
        let mesh = simulation.mesh();
        let values = fields.get(Layer::PlateVelocity).unwrap();
        assert_eq!(values.len(), mesh.len() * 3);
        let mut fastest = 0.0f32;
        for cell in 0..mesh.len() as u32 {
            let velocity = fields.vector(Layer::PlateVelocity, cell).unwrap();
            let up = mesh.vertex(cell).as_vec3();
            // Tangent to the sphere, and a plausible plate speed.
            assert!(velocity.dot(up).abs() <= 1e-3 * velocity.length().max(1.0));
            assert!(velocity.length() < 50.0, "{velocity}");
            fastest = fastest.max(velocity.length());
        }
        assert!(fastest > 0.1, "plates move: {fastest} cm/yr");
        // Rebuilt for a frame of the history from the cells and plates, the same.
        let cells = crate::viewer::cell_infos(&planet);
        let rebuilt = Fields::from_frame(mesh, &cells, &plate_infos(&planet));
        let rebuilt = rebuilt.get(Layer::PlateVelocity).unwrap();
        for (a, b) in values.iter().zip(rebuilt) {
            assert!((a - b).abs() <= 1e-4 * a.abs().max(1.0), "{a} vs {b}");
        }
        // No layers requested, none extracted.
        assert_eq!(Fields::extract(&planet, Layers::NONE).bytes(), 0);
    }
}
