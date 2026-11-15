//! Discretisation of the unit sphere into an icosphere mesh.

use std::collections::HashMap;

use glam::DVec3;

/// A geodesic grid on the unit sphere, created by repeatedly subdividing an icosahedron.
///
/// Every vertex is a simulation cell. Vertices have five (the original twelve icosahedron
/// corners) or six neighbours.
pub struct SphereMesh {
    vertices: Vec<DVec3>,
    neighbors: Vec<Vec<u32>>,
    /// Triangles, wound counter-clockwise when seen from outside the sphere.
    faces: Vec<[u32; 3]>,
    mean_edge_angle: f64,
    subdivisions: u32,
}

/// The number of original icosahedron vertices, which are stored first in every mesh.
const ICOSAHEDRON_VERTEX_COUNT: u32 = 12;

impl SphereMesh {
    /// Creates an icosphere with `10 * 4^subdivisions + 2` vertices.
    pub fn icosphere(subdivisions: u32) -> Self {
        let t = (1.0 + 5f64.sqrt()) / 2.0;
        let mut vertices: Vec<DVec3> = [
            (-1.0, t, 0.0),
            (1.0, t, 0.0),
            (-1.0, -t, 0.0),
            (1.0, -t, 0.0),
            (0.0, -1.0, t),
            (0.0, 1.0, t),
            (0.0, -1.0, -t),
            (0.0, 1.0, -t),
            (t, 0.0, -1.0),
            (t, 0.0, 1.0),
            (-t, 0.0, -1.0),
            (-t, 0.0, 1.0),
        ]
        .into_iter()
        .map(|(x, y, z)| DVec3::new(x, y, z).normalize())
        .collect();

        let mut faces: Vec<[u32; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];

        for _ in 0..subdivisions {
            let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
            let mut midpoint = |a: u32, b: u32, vertices: &mut Vec<DVec3>| -> u32 {
                let key = (a.min(b), a.max(b));
                *midpoints.entry(key).or_insert_with(|| {
                    let index = u32::try_from(vertices.len()).expect("too many vertices");
                    vertices.push((vertices[a as usize] + vertices[b as usize]).normalize());
                    index
                })
            };

            let mut new_faces = Vec::with_capacity(faces.len() * 4);
            for [a, b, c] in faces {
                let ab = midpoint(a, b, &mut vertices);
                let bc = midpoint(b, c, &mut vertices);
                let ca = midpoint(c, a, &mut vertices);
                new_faces.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            faces = new_faces;
        }

        let mut neighbors: Vec<Vec<u32>> = vec![Vec::with_capacity(6); vertices.len()];
        for &[a, b, c] in &faces {
            for (from, to) in [(a, b), (b, c), (c, a), (b, a), (c, b), (a, c)] {
                let list = &mut neighbors[from as usize];
                if !list.contains(&to) {
                    list.push(to);
                }
            }
        }

        let (angle_sum, edge_count) = neighbors
            .iter()
            .enumerate()
            .flat_map(|(from, list)| list.iter().map(move |&to| (from, to as usize)))
            .fold((0.0, 0usize), |(sum, count), (from, to)| {
                let angle = vertices[from].dot(vertices[to]).clamp(-1.0, 1.0).acos();
                (sum + angle, count + 1)
            });

        Self {
            vertices,
            neighbors,
            faces,
            mean_edge_angle: angle_sum / edge_count as f64,
            subdivisions,
        }
    }

    /// The triangles of the mesh, wound counter-clockwise when seen from outside.
    pub fn faces(&self) -> &[[u32; 3]] {
        &self.faces
    }

    /// The number of times the icosahedron was subdivided.
    pub fn subdivisions(&self) -> u32 {
        self.subdivisions
    }

    /// The number of vertices (cells) of the mesh.
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    /// The unit-length position of a vertex.
    pub fn vertex(&self, index: u32) -> DVec3 {
        self.vertices[index as usize]
    }

    pub fn neighbors(&self, index: u32) -> &[u32] {
        &self.neighbors[index as usize]
    }

    /// The mean angle (in radians) between neighbouring vertices.
    pub fn mean_edge_angle(&self) -> f64 {
        self.mean_edge_angle
    }

    /// Finds the vertex closest to `target` (a unit vector) by walking greedily over the mesh,
    /// starting from `start`.
    ///
    /// This is fast when `start` is already close to the result, e.g. the answer of a previous
    /// query for a nearby point.
    pub fn nearest_from(&self, target: DVec3, start: u32) -> u32 {
        let mut current = start;
        let mut best = self.vertices[current as usize].dot(target);
        loop {
            let previous = current;
            for &neighbor in &self.neighbors[previous as usize] {
                let similarity = self.vertices[neighbor as usize].dot(target);
                if similarity > best {
                    best = similarity;
                    current = neighbor;
                }
            }
            if current == previous {
                return current;
            }
        }
    }

    /// Finds the vertex closest to `target` (a unit vector) without a starting hint.
    pub fn nearest(&self, target: DVec3) -> u32 {
        let start = (0..ICOSAHEDRON_VERTEX_COUNT)
            .max_by(|&a, &b| {
                let a = self.vertices[a as usize].dot(target);
                let b = self.vertices[b as usize].dot(target);
                a.total_cmp(&b)
            })
            .expect("mesh has vertices");
        self.nearest_from(target, start)
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::ChaCha8Rng;
    use rand::{RngExt, SeedableRng};

    use super::*;

    #[test]
    fn vertex_and_neighbor_counts() {
        for subdivisions in 0..5 {
            let mesh = SphereMesh::icosphere(subdivisions);
            assert_eq!(mesh.len(), 10 * 4usize.pow(subdivisions) + 2);
            for index in 0..mesh.len() as u32 {
                assert!((mesh.vertex(index).length() - 1.0).abs() < 1e-12);
                let expected = if index < ICOSAHEDRON_VERTEX_COUNT {
                    5
                } else {
                    6
                };
                assert_eq!(mesh.neighbors(index).len(), expected);
            }
        }
    }

    #[test]
    fn faces_are_wound_counter_clockwise_from_outside() {
        let mesh = SphereMesh::icosphere(3);
        assert_eq!(mesh.faces().len(), 20 * 4usize.pow(3));
        for &[a, b, c] in mesh.faces() {
            let (a, b, c) = (mesh.vertex(a), mesh.vertex(b), mesh.vertex(c));
            let normal = (b - a).cross(c - a);
            assert!(normal.dot(a + b + c) > 0.0);
        }
    }

    #[test]
    fn nearest_matches_brute_force() {
        let mesh = SphereMesh::icosphere(4);
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        for _ in 0..2000 {
            let target = DVec3::new(
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
                rng.random_range(-1.0..1.0),
            )
            .normalize();
            let brute_force = (0..mesh.len() as u32)
                .max_by(|&a, &b| {
                    mesh.vertex(a)
                        .dot(target)
                        .total_cmp(&mesh.vertex(b).dot(target))
                })
                .unwrap();
            let found = mesh.nearest(target);
            assert!(
                (mesh.vertex(found).dot(target) - mesh.vertex(brute_force).dot(target)).abs()
                    < 1e-12
            );
        }
    }
}
