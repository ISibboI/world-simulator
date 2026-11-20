//! Wind-driven ocean gyres on the coarse mesh of the energy balance model.
//!
//! The depth-integrated volume transport `(U, V) = (−∂ψ/∂y, ∂ψ/∂x)` of the wind-driven
//! circulation follows Stommel's (1948) vorticity balance
//!
//! ```text
//! β ∂ψ/∂x + r ∇²ψ = curl τ / ρ₀
//! ```
//!
//! with the planetary vorticity gradient `β = 2Ω cos φ / R`, the wind stress `τ` and a bottom
//! friction `r`. In the interior this is Sverdrup's balance `β V = curl τ / ρ₀`, integrated
//! westward from the eastern coasts; along the western coasts the friction closes the gyres in
//! narrow, fast western boundary currents. The width of these currents, `r / β`, cannot be
//! narrower than a cell of the coarse mesh (~440 km), which sets `r`; the interior transport does
//! not depend on it. Continents and islands are walls (`ψ = 0`), so there is no flow around them.
//!
//! The currents carry the heat of the upper ocean: the heat flux across a cell edge is
//! `ρ c_p γ Q T` for the volume flux `Q` and the temperature `T` of the mixed layer upstream,
//! where `γ` is the share of the wind-driven transport that flows at the temperature of the
//! surface (the warm water above the thermocline).

use glam::DVec3;

use crate::sparse::{Hierarchy, Multigrid};
use crate::sphere::SphereMesh;

const EARTH_ROTATION: f64 = 7.292e-5;
const SEA_WATER_DENSITY: f64 = 1025.0;

/// Volume fluxes of the gyres across the edges between ocean cells.
#[derive(Clone, Debug, Default)]
pub struct Gyres {
    /// For every cell, the neighbours and the volume flux to each (m³/s, positive outwards).
    pub fluxes: Vec<Vec<(u32, f64)>>,
    /// The streamfunction (m³/s) of every cell.
    pub streamfunction: Vec<f64>,
}

pub struct GyreModel {
    hierarchy: Hierarchy,
    radius: f64,
}

impl GyreModel {
    pub fn new(mesh: &SphereMesh, radius: f64) -> Self {
        Self {
            hierarchy: Hierarchy::new(mesh, mesh.subdivisions().min(1)),
            radius,
        }
    }

    /// Solves for the gyres of the ocean cells (`ocean[c]`) under the zonal wind stress
    /// `stress(latitude in degrees)` (N/m², eastward) with the bottom friction `friction` (1/s).
    pub fn solve(
        &self,
        mesh: &SphereMesh,
        ocean: &[bool],
        stress: &dyn Fn(f64) -> f64,
        friction: f64,
    ) -> Gyres {
        let n = mesh.len();
        let spacing = mesh.mean_edge_angle() * self.radius;
        let laplacian = 2.0 / (3.0 * spacing * spacing);
        let mut values = Vec::with_capacity(n * 7);
        let mut rhs = vec![0.0; n];
        for cell in 0..n as u32 {
            let neighbors = mesh.neighbors(cell);
            if !ocean[cell as usize] {
                values.push(1.0);
                values.extend(std::iter::repeat_n(0.0, neighbors.len()));
                continue;
            }
            let position = mesh.vertex(cell);
            let latitude = position.z.clamp(-1.0, 1.0).asin();
            let east = east_of(position);
            let beta = 2.0 * EARTH_ROTATION * latitude.cos() / self.radius;
            // Forward (eastward) difference for ∂ψ/∂x: the Sverdrup interior is integrated
            // westward from the eastern coast.
            let weights: Vec<f64> = neighbors
                .iter()
                .map(|&j| (mesh.vertex(j) - position).normalize().dot(east).max(0.0))
                .collect();
            let norm: f64 = weights.iter().map(|w| w * w).sum::<f64>() * spacing;
            let mut diagonal = 0.0;
            let mut off = Vec::with_capacity(neighbors.len());
            for (&j, &w) in neighbors.iter().zip(&weights) {
                let mut value = friction * laplacian;
                diagonal -= friction * laplacian;
                if norm > 0.0 {
                    value += beta * w / norm;
                    diagonal -= beta * w / norm;
                }
                // Coasts are walls: ψ = 0 there.
                off.push(if ocean[j as usize] { value } else { 0.0 });
            }
            values.push(diagonal);
            values.extend(off);
            rhs[cell as usize] =
                wind_stress_curl(latitude, stress, self.radius) / SEA_WATER_DENSITY;
        }
        let multigrid = Multigrid::new(&self.hierarchy, values);
        let (streamfunction, _) = multigrid.solve(&rhs, 1e-8, 400);

        // Depth-integrated transport (m²/s) of every ocean cell from the gradient of ψ, and the
        // volume fluxes across the edges (hexagonal edge length spacing/√3).
        let edge = spacing / 3f64.sqrt();
        let transport: Vec<DVec3> = (0..n as u32)
            .map(|cell| {
                if !ocean[cell as usize] {
                    return DVec3::ZERO;
                }
                let position = mesh.vertex(cell);
                // Least-squares gradient over the neighbours.
                let neighbors = mesh.neighbors(cell);
                let mut gradient = DVec3::ZERO;
                for &j in neighbors {
                    let direction = (mesh.vertex(j) - position).normalize();
                    gradient +=
                        direction * (streamfunction[j as usize] - streamfunction[cell as usize]);
                }
                gradient *= 2.0 / (neighbors.len() as f64 * spacing);
                // (U, V) = k × ∇ψ … in vector form: transport = r̂ × ∇ψ.
                position.cross(gradient)
            })
            .collect();
        let fluxes = (0..n as u32)
            .map(|cell| {
                if !ocean[cell as usize] {
                    return Vec::new();
                }
                let position = mesh.vertex(cell);
                mesh.neighbors(cell)
                    .iter()
                    .filter(|&&j| ocean[j as usize])
                    .map(|&j| {
                        let direction = (mesh.vertex(j) - position).normalize();
                        let mean = 0.5 * (transport[cell as usize] + transport[j as usize]);
                        (j, mean.dot(direction) * edge)
                    })
                    .collect()
            })
            .collect();
        Gyres {
            fluxes,
            streamfunction,
        }
    }
}

impl Gyres {
    /// Heat delivered to every cell (W) by the currents carrying the share `share` of their
    /// volume at the temperature `temperature` (°C; relative to `reference`), upstream.
    pub fn heat_convergence(
        &self,
        temperature: &[f64],
        reference: f64,
        share: f64,
        heat_capacity: f64,
        out: &mut [f64],
    ) {
        out.fill(0.0);
        for (cell, fluxes) in self.fluxes.iter().enumerate() {
            for &(j, q) in fluxes {
                // Each edge once, from the cell with the lower index.
                if (j as usize) < cell {
                    continue;
                }
                let upstream = if q > 0.0 { cell } else { j as usize };
                let heat = SEA_WATER_DENSITY
                    * heat_capacity
                    * share
                    * q
                    * (temperature[upstream] - reference);
                out[cell] -= heat;
                out[j as usize] += heat;
            }
        }
    }
}

fn east_of(position: DVec3) -> DVec3 {
    let east = DVec3::Z.cross(position);
    if east.length_squared() < 1e-12 {
        DVec3::X
    } else {
        east.normalize()
    }
}

/// Curl (N/m³) of a zonal wind stress that depends on latitude only:
/// `−(1 / (R cos φ)) ∂(τ cos φ)/∂φ`.
fn wind_stress_curl(latitude: f64, stress: &dyn Fn(f64) -> f64, radius: f64) -> f64 {
    let h = 0.5f64.to_radians();
    let at = |l: f64| stress(l.to_degrees()) * l.cos();
    -(at(latitude + h) - at(latitude - h)) / (2.0 * h) / (radius * latitude.cos().max(0.05))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtropical_gyres_turn_clockwise_with_western_boundary_currents() {
        let mesh = SphereMesh::icosphere(4);
        let radius = 6.371e6;
        // An ocean basin between 60°W and 60°E.
        let ocean: Vec<bool> = (0..mesh.len() as u32)
            .map(|c| {
                let p = mesh.vertex(c);
                let longitude = p.y.atan2(p.x).to_degrees();
                longitude.abs() < 60.0 && p.z.abs() < 0.9
            })
            .collect();
        let model = GyreModel::new(&mesh, radius);
        let gyres = model.solve(
            &mesh,
            &ocean,
            &|l| crate::climate::surface_wind_stress(l, 4.5),
            1e-5,
        );
        // Subtropical gyres: clockwise in the north (ψ > 0 with U = −∂ψ/∂y), anticlockwise in
        // the south.
        let mean_at = |lat: f64, lon_from: f64, lon_to: f64| {
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| {
                    let p = mesh.vertex(c as u32);
                    let latitude = p.z.asin().to_degrees();
                    let longitude = p.y.atan2(p.x).to_degrees();
                    ocean[c]
                        && (latitude - lat).abs() < 4.0
                        && (lon_from..lon_to).contains(&longitude)
                })
                .collect();
            cells.iter().map(|&c| gyres.streamfunction[c]).sum::<f64>() / cells.len() as f64
        };
        let north = mean_at(30.0, -40.0, 40.0);
        let south = mean_at(-30.0, -40.0, 40.0);
        assert!(north > 5e6, "{north}");
        assert!(south < -5e6, "{south}");
        // The extreme of ψ lies in the west (western intensification).
        assert!(mean_at(30.0, -50.0, -20.0).abs() > mean_at(30.0, 20.0, 50.0).abs());
    }
}
