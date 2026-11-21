//! Finding the cell under the pointer, and its outline.
//!
//! The pointer defines a ray from the camera. The displayed surface lies between the ocean at
//! radius 1 and the highest exaggerated mountain, so the ray is marched through that shell in
//! steps of half a cell spacing, with the cell below each point found by the nearest-vertex
//! walk of [`SphereMesh`] (warm-started from the previous point). The first point that is not
//! above its cell's displayed radius is refined by bisection. With the default 40× relief
//! exaggeration, mountains stand up to ~0.05 planet radii high, a few cells: a plain
//! intersection with the sphere would pick a cell behind a mountain seen near the limb, or
//! one several cells off on its flank. The marching treats each cell as a terrace at its
//! vertex's radius, so the result can be off by about one cell on steep flanks, which is the
//! resolution of the data anyway.

use glam::{DVec3, Mat4, Vec2, Vec4Swizzles};

use crate::sphere::SphereMesh;

/// A ray with a unit direction.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: DVec3,
    pub direction: DVec3,
}

impl Ray {
    pub fn at(&self, t: f64) -> DVec3 {
        self.origin + self.direction * t
    }
}

/// The ray from the camera through a point of the view given in normalised device coordinates
/// (`x` and `y` from -1 to 1, `y` up), for the camera's `view_projection` matrix (wgpu depth
/// from 0 to 1).
pub fn pointer_ray(view_projection: Mat4, ndc: Vec2) -> Ray {
    let inverse = view_projection.inverse();
    let unproject = |depth: f32| {
        let point = inverse * ndc.extend(depth).extend(1.0);
        (point.xyz() / point.w).as_dvec3()
    };
    let near = unproject(0.0);
    let far = unproject(1.0);
    Ray {
        origin: near,
        direction: (far - near).normalize_or(DVec3::NEG_Z),
    }
}

/// The distances along the ray at which it enters and leaves the sphere of `radius` around the
/// origin, if it meets it.
pub fn ray_sphere(ray: &Ray, radius: f64) -> Option<(f64, f64)> {
    // |o + t d|² = r² with |d| = 1: t² + 2 b t + c = 0.
    let b = ray.origin.dot(ray.direction);
    let c = ray.origin.length_squared() - radius * radius;
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    Some((-b - root, -b + root))
}

/// Bisection steps that refine a hit; each halves the half-cell step.
const REFINEMENT_STEPS: usize = 6;

/// The cell whose displayed surface the ray meets first, if any. `radius(cell)` is the
/// displayed radius of a cell (at least about 1, the ocean), and `max_radius` the largest one.
pub fn pick(
    mesh: &SphereMesh,
    radius: impl Fn(u32) -> f64,
    max_radius: f64,
    ray: &Ray,
) -> Option<u32> {
    let (enter, leave) = ray_sphere(ray, max_radius.max(1.0))?;
    // The ray ends on the ocean sphere; a ray that misses it can still graze a mountain.
    let ocean = ray_sphere(ray, 1.0).map(|(enter, _)| enter);
    let end = ocean.unwrap_or(leave);
    let start = enter.max(0.0);
    if end < start {
        return None;
    }
    let step = 0.5 * mesh.mean_edge_angle();
    let mut cell = mesh.nearest(ray.at(start).normalize_or(DVec3::X));
    let below = |t: f64, cell: &mut u32| {
        let point = ray.at(t);
        *cell = mesh.nearest_from(point.normalize_or(DVec3::X), *cell);
        point.length() <= radius(*cell)
    };
    let (mut above, mut t) = (start, start);
    loop {
        if below(t, &mut cell) {
            if t > start {
                let (mut low, mut high) = (above, t);
                let mut hit = cell;
                for _ in 0..REFINEMENT_STEPS {
                    let middle = 0.5 * (low + high);
                    let mut candidate = hit;
                    if below(middle, &mut candidate) {
                        high = middle;
                        hit = candidate;
                    } else {
                        low = middle;
                    }
                }
                cell = hit;
            }
            return Some(cell);
        }
        if t >= end {
            // On the ocean sphere every cell is at or above the ray (up to rounding).
            return ocean.map(|_| cell);
        }
        above = t;
        t = (t + step).min(end);
    }
}

/// The outline of a cell: the polygon around its vertex through the centroids of the
/// triangles around it and the midpoints of the edges between them (the barycentric dual
/// cell), counter-clockwise when seen from outside. `position(cell)` is the displayed position
/// of a vertex; every segment of the outline lies in one displayed triangle, so it follows the
/// drawn surface.
pub fn outline(mesh: &SphereMesh, position: impl Fn(u32) -> DVec3, cell: u32) -> Vec<DVec3> {
    let up = mesh.vertex(cell);
    let east = if up.z.abs() < 0.9 { DVec3::Z } else { DVec3::X }
        .cross(up)
        .normalize();
    let north = up.cross(east);
    let mut neighbors = mesh.neighbors(cell).to_vec();
    neighbors.sort_by(|&a, &b| {
        let angle = |vertex: u32| {
            let offset = mesh.vertex(vertex) - up;
            offset.dot(north).atan2(offset.dot(east))
        };
        angle(a).total_cmp(&angle(b))
    });
    let center = position(cell);
    let mut points = Vec::with_capacity(2 * neighbors.len());
    for (index, &neighbor) in neighbors.iter().enumerate() {
        let next = neighbors[(index + 1) % neighbors.len()];
        let (a, b) = (position(neighbor), position(next));
        points.push((center + a + b) / 3.0);
        points.push((center + b) / 2.0);
    }
    points
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    fn close(a: DVec3, b: DVec3, tolerance: f64) -> bool {
        (a - b).length() < tolerance
    }

    #[test]
    fn ray_meets_sphere_at_both_sides() {
        let ray = Ray {
            origin: DVec3::new(0.0, 0.0, 5.0),
            direction: DVec3::NEG_Z,
        };
        let (enter, leave) = ray_sphere(&ray, 1.0).unwrap();
        assert!((enter - 4.0).abs() < 1e-12);
        assert!((leave - 6.0).abs() < 1e-12);
        let offset = Ray {
            origin: DVec3::new(0.6, 0.0, 5.0),
            ..ray
        };
        let (enter, _) = ray_sphere(&offset, 1.0).unwrap();
        assert!((offset.at(enter).length() - 1.0).abs() < 1e-12);
        assert!((offset.at(enter).z - 0.8).abs() < 1e-12);
        let miss = Ray {
            origin: DVec3::new(1.01, 0.0, 5.0),
            ..ray
        };
        assert!(ray_sphere(&miss, 1.0).is_none());
    }

    fn camera() -> Mat4 {
        let eye = Vec3::new(4.5, 0.0, 0.0);
        let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::ZERO, Vec3::Z);
        let projection = glam::camera::rh::proj::directx::perspective(0.6, 1.5, 0.1, 10.0);
        projection * view
    }

    #[test]
    fn pointer_ray_unprojects_the_view() {
        let view_projection = camera();
        let ray = pointer_ray(view_projection, Vec2::ZERO);
        assert!(close(ray.direction, DVec3::NEG_X, 1e-5));
        assert!((ray.origin.y).abs() < 1e-5 && ray.origin.z.abs() < 1e-5);
        // A point of the scene lies on the ray through its own projection.
        let point = Vec3::new(0.3, -0.4, 0.5);
        let clip = view_projection * point.extend(1.0);
        let ray = pointer_ray(view_projection, clip.xy() / clip.w);
        let along = (point.as_dvec3() - ray.origin).dot(ray.direction);
        assert!(close(ray.at(along), point.as_dvec3(), 1e-4));
    }

    /// A random direction from a simple deterministic sequence.
    fn directions(count: usize) -> impl Iterator<Item = DVec3> {
        (0..count).map(move |index| {
            let golden = (1.0 + 5f64.sqrt()) / 2.0;
            let z = 1.0 - 2.0 * (index as f64 + 0.5) / count as f64;
            let longitude = std::f64::consts::TAU * index as f64 / golden;
            let r = (1.0 - z * z).sqrt();
            DVec3::new(r * longitude.cos(), r * longitude.sin(), z)
        })
    }

    #[test]
    fn pick_on_a_smooth_sphere_is_the_nearest_cell_of_the_hit() {
        let mesh = SphereMesh::icosphere(4);
        for (index, target) in directions(500).enumerate() {
            // Rays from various cameras towards points of the visible hemisphere.
            let eye = (target + DVec3::new(0.3, -0.2, 0.1) * (index % 3) as f64).normalize() * 4.5;
            let ray = Ray {
                origin: eye,
                direction: (target - eye).normalize(),
            };
            let (enter, _) = ray_sphere(&ray, 1.0).unwrap();
            let expected = mesh.nearest(ray.at(enter).normalize());
            assert_eq!(pick(&mesh, |_| 1.0, 1.0, &ray), Some(expected));
        }
    }

    #[test]
    fn pick_misses_beside_the_planet() {
        let mesh = SphereMesh::icosphere(3);
        let ray = Ray {
            origin: DVec3::new(4.5, 0.0, 0.0),
            direction: (DVec3::new(0.0, 1.2, 0.0) - DVec3::new(4.5, 0.0, 0.0)).normalize(),
        };
        assert_eq!(pick(&mesh, |_| 1.0, 1.05, &ray), None);
    }

    #[test]
    fn pick_hits_a_mountain_in_front_of_the_ocean() {
        let mesh = SphereMesh::icosphere(5);
        let mountain = mesh.nearest(DVec3::new(1.0, 0.0, 0.3).normalize());
        let height = 1.05;
        let radius = |cell: u32| if cell == mountain { height } else { 1.0 };
        // A grazing ray at the mountain's top, from far beyond its side, whose ocean hit is
        // several cells behind the mountain.
        let up = mesh.vertex(mountain);
        let top = up * (height - 0.001);
        let tangent = DVec3::Y.cross(up).cross(up).normalize();
        let angle = 20f64.to_radians();
        let direction = (tangent * angle.cos() - up * angle.sin()).normalize();
        let ray = Ray {
            origin: top - direction * 4.0,
            direction,
        };
        let (ocean, _) = ray_sphere(&ray, 1.0).unwrap();
        assert_ne!(mesh.nearest(ray.at(ocean).normalize()), mountain);
        assert_eq!(pick(&mesh, radius, height, &ray), Some(mountain));
        // Without the mountain, the ray reaches the ocean.
        assert_eq!(
            pick(&mesh, |_| 1.0, 1.0, &ray),
            Some(mesh.nearest(ray.at(ocean).normalize()))
        );
    }

    #[test]
    fn outline_surrounds_the_cell() {
        let mesh = SphereMesh::icosphere(3);
        for cell in [0, 5, 11, 12, 100, 641] {
            let points = outline(&mesh, |vertex| mesh.vertex(vertex), cell);
            let up = mesh.vertex(cell);
            assert_eq!(points.len(), 2 * mesh.neighbors(cell).len());
            for (index, &point) in points.iter().enumerate() {
                // About as close to this cell as to any neighbour (on the boundary of its
                // Voronoi cell, which the barycentric dual follows closely), at about half an
                // edge.
                let next = points[(index + 1) % points.len()];
                let distance = (point - up).length();
                assert!(distance > 0.3 * mesh.mean_edge_angle());
                assert!(distance < 0.8 * mesh.mean_edge_angle());
                for &neighbor in mesh.neighbors(cell) {
                    assert!(
                        (point - mesh.vertex(neighbor)).length()
                            >= distance - 0.1 * mesh.mean_edge_angle()
                    );
                }
                // Counter-clockwise from outside.
                assert!((point - up).cross(next - up).dot(up) > 0.0);
            }
        }
    }
}
