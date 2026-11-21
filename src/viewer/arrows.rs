//! Vector overlays: arrows on the sphere, for modes that show vector fields (plate motion,
//! ocean currents, winds).
//!
//! A mode gives a field of vectors (a [`Sample`] per point, e.g. the cells of a coarse grid,
//! see [`sample_cells`]); [`arrows`] scales them with an [`ArrowStyle`] (length and width grow
//! with the magnitude up to a reference value) and [`triangles`] turns them into geometry: a
//! shaft and a head in the plane tangent to the sphere at the arrow's base, lifted above the
//! displayed surface. The renderer draws them depth-tested against the planet (mountains in
//! front hide them), each with a dark border for contrast on any colour.

use glam::{DVec3, Vec3};

use crate::hash::HashSet;
use crate::sphere::SphereMesh;

/// A vector at a point of the sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// The base of the arrow: a point on (or above) the displayed surface.
    pub base: DVec3,
    /// The vector, tangent to the sphere at `base` (a radial part is removed), in the unit of
    /// the field (e.g. cm/yr).
    pub vector: DVec3,
}

/// How a field of vectors becomes arrows. Lengths and widths are in planet radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrowStyle {
    /// Magnitude (in the unit of the field) drawn with the full length and width; larger ones
    /// are drawn as large.
    pub reference: f64,
    /// Length and width grow with (magnitude / reference) to this power: 1 is proportional,
    /// 0.5 (square root) keeps slow vectors readable next to fast ones.
    pub exponent: f64,
    /// Length of an arrow of the reference magnitude.
    pub max_length: f64,
    /// Shaft width of an arrow of the reference magnitude, and of the smallest one.
    pub max_width: f64,
    pub min_width: f64,
    /// Arrows shorter than this fraction of `max_length` are left out.
    pub min_fraction: f64,
    /// Length of the head as a share of the arrow (at most), and its width as a multiple of
    /// the shaft width.
    pub head_length: f64,
    pub head_width: f64,
    /// Width of the dark border in planet radii.
    pub border: f64,
    /// Height of the arrow above its base in planet radii, so that it lies above the surface.
    pub lift: f64,
    /// Colour of the arrow and of its border (RGB with alpha, like the vertex colours of the planet).
    pub color: [f32; 4],
    pub border_color: [f32; 4],
}

impl ArrowStyle {
    /// A style for arrows spaced `spacing` planet radii apart (e.g. the mean edge length of
    /// the sampling grid), readable at the default zoom: arrows of the reference magnitude
    /// reach 90% of the way to the next one.
    pub fn for_spacing(spacing: f64, reference: f64) -> Self {
        Self {
            reference,
            exponent: 1.0,
            max_length: 0.9 * spacing,
            max_width: 0.16 * spacing,
            min_width: 0.06 * spacing,
            min_fraction: 0.05,
            head_length: 0.4,
            head_width: 2.4,
            border: 0.025 * spacing,
            lift: 0.004,
            color: [1.0, 0.95, 0.35, 1.0],
            border_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// The length of the arrow of a magnitude (planet radii).
    pub fn length(&self, magnitude: f64) -> f64 {
        self.max_length * self.fraction(magnitude)
    }

    /// The shaft width of the arrow of a magnitude (planet radii).
    pub fn width(&self, magnitude: f64) -> f64 {
        self.min_width + (self.max_width - self.min_width) * self.fraction(magnitude)
    }

    fn fraction(&self, magnitude: f64) -> f64 {
        if self.reference > 0.0 && magnitude.is_finite() {
            (magnitude / self.reference)
                .clamp(0.0, 1.0)
                .powf(self.exponent)
        } else {
            0.0
        }
    }
}

/// An arrow on the sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrow {
    /// The base of the arrow, already lifted above the surface.
    pub base: DVec3,
    /// Unit vector along the arrow, tangent to the sphere at the base.
    pub direction: DVec3,
    pub length: f64,
    /// Shaft width.
    pub width: f64,
}

/// The arrows of a field of samples; samples too small to be seen are left out.
pub fn arrows(samples: &[Sample], style: &ArrowStyle) -> Vec<Arrow> {
    samples
        .iter()
        .filter_map(|sample| {
            let up = sample.base.normalize_or_zero();
            let tangent = sample.vector - up * sample.vector.dot(up);
            let magnitude = tangent.length();
            let length = style.length(magnitude);
            if up == DVec3::ZERO
                || !(magnitude > 0.0 && magnitude.is_finite())
                || length < style.min_fraction * style.max_length
            {
                return None;
            }
            Some(Arrow {
                base: sample.base + up * style.lift,
                direction: tangent / magnitude,
                length,
                width: style.width(magnitude),
            })
        })
        .collect()
}

/// A vertex of the overlay: position and colour.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OverlayVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// The three triangles of an arrow (shaft quad and head), widened by `margin` on every side
/// (for the border), counter-clockwise seen from outside the sphere.
pub fn arrow_triangles(arrow: &Arrow, style: &ArrowStyle, margin: f64) -> [[DVec3; 3]; 3] {
    let up = arrow.base.normalize_or_zero();
    let along = arrow.direction;
    // `along × up` points to the right of the arrow seen from outside; `up × along` left.
    let left = up.cross(along).normalize_or_zero();
    let head_length = (style.head_length * arrow.length).max(0.0);
    let start = arrow.base - along * margin;
    let neck = arrow.base + along * (arrow.length - head_length);
    let tip = arrow.base + along * (arrow.length + 2.0 * margin);
    let half = arrow.width / 2.0 + margin;
    let head_half = arrow.width * style.head_width / 2.0 + 2.0 * margin;
    let (start_left, start_right) = (start + left * half, start - left * half);
    let (neck_left, neck_right) = (neck + left * half, neck - left * half);
    // The border of the head also reaches behind it.
    let head_back = neck - along * margin;
    let (head_left, head_right) = (head_back + left * head_half, head_back - left * head_half);
    [
        [start_right, neck_right, neck_left],
        [start_right, neck_left, start_left],
        [head_right, tip, head_left],
    ]
}

/// The overlay vertices of arrows (a list of triangles): all borders first, then all arrows, so
/// that the renderer, which does not write depth for overlays, draws every arrow over the
/// borders.
pub fn triangles(arrows: &[Arrow], style: &ArrowStyle) -> Vec<OverlayVertex> {
    let mut vertices = Vec::with_capacity(arrows.len() * 18);
    for (margin, color) in [(style.border, style.border_color), (0.0, style.color)] {
        for arrow in arrows {
            for triangle in arrow_triangles(arrow, style, margin) {
                vertices.extend(triangle.iter().map(|point| OverlayVertex {
                    position: point.as_vec3().to_array(),
                    color,
                }));
            }
        }
    }
    vertices
}

/// The cells of `mesh` nearest to the vertices of an icosphere with `subdivisions`: evenly
/// spread points for arrows, with their mean spacing in planet radii. Duplicates (on a mesh
/// coarser than the grid) are left out.
pub fn sample_cells(mesh: &SphereMesh, subdivisions: u32) -> (Vec<u32>, f64) {
    let grid = SphereMesh::icosphere(subdivisions);
    let mut cells: Vec<u32> = (0..grid.len() as u32)
        .map(|vertex| mesh.nearest(grid.vertex(vertex)))
        .collect();
    cells.sort_unstable();
    cells.dedup();
    (cells, grid.mean_edge_angle())
}

/// Where an arrow at `cell` starts: above the highest displayed point within `reach` (planet
/// radii, the longest arrow) of the cell, so that the arrow does not dive into the relief it
/// points at (with the exaggerated relief, mountains rise several cell spacings' worth above
/// the tangent plane), given the displayed positions of the vertices.
pub fn base_point(
    mesh: &SphereMesh,
    cell: u32,
    reach: f64,
    position: impl Fn(u32) -> Vec3,
) -> DVec3 {
    let up = mesh.vertex(cell).normalize_or(DVec3::X);
    // Cells within the reach: a walk over neighbours, as far as the angle allows.
    let min_cos = reach.min(std::f64::consts::FRAC_PI_2).cos();
    let mut seen: HashSet<u32> = HashSet::default();
    seen.insert(cell);
    let mut frontier = vec![cell];
    let mut radius = 1.0f64;
    while let Some(vertex) = frontier.pop() {
        radius = radius.max(f64::from(position(vertex).length()));
        for &neighbor in mesh.neighbors(vertex) {
            if mesh.vertex(neighbor).normalize_or(DVec3::X).dot(up) >= min_cos
                && seen.insert(neighbor)
            {
                frontier.push(neighbor);
            }
        }
    }
    up * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> ArrowStyle {
        ArrowStyle::for_spacing(0.1, 10.0)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn length_and_width_grow_with_the_magnitude_up_to_the_reference() {
        let style = style();
        assert!(close(style.length(10.0), style.max_length));
        assert!(close(style.length(20.0), style.max_length));
        assert!(close(style.length(5.0), style.max_length / 2.0));
        assert!(close(style.width(0.0), style.min_width));
        assert!(close(style.width(10.0), style.max_width));
        assert!(style.width(5.0) > style.min_width && style.width(5.0) < style.max_width);
        assert_eq!(style.length(f64::NAN), 0.0);
        let root = ArrowStyle {
            exponent: 0.5,
            ..style
        };
        assert!(close(root.length(2.5), style.max_length / 2.0));
        assert!(close(root.length(10.0), style.max_length));
    }

    #[test]
    fn arrows_are_tangent_and_lifted() {
        let style = style();
        let base = DVec3::new(1.0, 2.0, 2.0) / 3.0 * 1.01;
        // A vector with a radial part, which is dropped.
        let tangent = DVec3::new(2.0, -1.0, 0.0).normalize() * 5.0;
        let samples = [Sample {
            base,
            vector: tangent + base.normalize() * 7.0,
        }];
        let arrows = arrows(&samples, &style);
        assert_eq!(arrows.len(), 1);
        let arrow = arrows[0];
        assert!(close(arrow.direction.dot(base.normalize()), 0.0));
        assert!(close(arrow.direction.length(), 1.0));
        assert!(arrow.direction.dot(tangent) > 0.0);
        assert!(close(arrow.length, style.max_length / 2.0));
        assert!(close(arrow.base.length(), base.length() + style.lift));
    }

    #[test]
    fn tiny_and_zero_vectors_give_no_arrow() {
        let style = style();
        let base = DVec3::Z;
        let samples = [
            Sample {
                base,
                vector: DVec3::ZERO,
            },
            Sample {
                base,
                vector: DVec3::X * 0.01,
            },
            // Purely radial.
            Sample {
                base,
                vector: DVec3::Z * 3.0,
            },
            Sample {
                base,
                vector: DVec3::X * 3.0,
            },
        ];
        assert_eq!(arrows(&samples, &style).len(), 1);
    }

    #[test]
    fn arrow_geometry() {
        let style = style();
        let arrow = Arrow {
            base: DVec3::Z * 1.01,
            direction: DVec3::X,
            length: 0.08,
            width: 0.01,
        };
        let [shaft_a, shaft_b, head] = arrow_triangles(&arrow, &style, 0.0);
        // Everything lies in the tangent plane at the base: never below the base.
        for point in shaft_a.iter().chain(&shaft_b).chain(&head) {
            assert!(close(point.z, 1.01));
            assert!(point.length() >= 1.01 - 1e-12);
        }
        // The tip is at the arrow's length, the shaft starts at the base.
        assert!(close(head[1].x, 0.08));
        assert!(close(shaft_a[0].x, 0.0));
        // Shaft and head widths (across the arrow, along y).
        let width = |points: &[DVec3]| {
            let ys = points.iter().map(|point| point.y);
            ys.clone().fold(f64::MIN, f64::max) - ys.fold(f64::MAX, f64::min)
        };
        assert!(close(width(&shaft_a), 0.01));
        assert!(close(width(&head), 0.01 * style.head_width));
        // The head takes its share of the length, and the shaft ends where it starts.
        let head_length = style.head_length * 0.08;
        assert!(close(head[0].x, 0.08 - head_length));
        assert!(close(shaft_b[1].x, 0.08 - head_length));
        // Counter-clockwise seen from outside: the normals point away from the planet.
        for [a, b, c] in [shaft_a, shaft_b, head] {
            assert!((b - a).cross(c - a).z > 0.0);
        }
        // The border is larger on every side.
        let [border_a, _, border_head] = arrow_triangles(&arrow, &style, 0.002);
        assert!(border_a[0].x < 0.0);
        assert!(width(&border_a) > 0.01);
        assert!(border_head[1].x > 0.08);
    }

    #[test]
    fn borders_come_first() {
        let style = style();
        let arrow = Arrow {
            base: DVec3::Y,
            direction: DVec3::Z,
            length: 0.05,
            width: 0.01,
        };
        let vertices = triangles(&[arrow, arrow], &style);
        assert_eq!(vertices.len(), 2 * 2 * 9);
        assert!(vertices[..18].iter().all(|v| v.color == style.border_color));
        assert!(vertices[18..].iter().all(|v| v.color == style.color));
    }

    #[test]
    fn sample_cells_are_spread_and_distinct() {
        let mesh = SphereMesh::icosphere(4);
        let (cells, spacing) = sample_cells(&mesh, 2);
        // 10 · 4² + 2 grid points on a finer mesh: all distinct, about 15° apart.
        assert_eq!(cells.len(), 162);
        assert!(
            (spacing.to_degrees() - 15.0).abs() < 3.0,
            "{}",
            spacing.to_degrees()
        );
        let coarse = SphereMesh::icosphere(1);
        assert_eq!(sample_cells(&coarse, 3).0.len(), coarse.len());
    }

    #[test]
    fn bases_are_on_the_highest_neighbour() {
        let mesh = SphereMesh::icosphere(2);
        let cell = 5;
        let high = mesh.neighbors(cell)[0];
        let base = base_point(&mesh, cell, 0.5 * mesh.mean_edge_angle(), |vertex| {
            let radius = if vertex == high { 1.05 } else { 1.01 };
            mesh.vertex(vertex).as_vec3() * radius
        });
        // The neighbour is out of reach.
        assert!((base.length() - 1.01).abs() < 1e-6);
        let base = base_point(&mesh, cell, 1.2 * mesh.mean_edge_angle(), |vertex| {
            let radius = if vertex == high { 1.05 } else { 1.01 };
            mesh.vertex(vertex).as_vec3() * radius
        });
        assert!((base.length() - 1.05).abs() < 1e-6);
        assert!(close(base.normalize().dot(mesh.vertex(cell)), 1.0));
        // A peak two rings away counts with a reach that includes it.
        let ring = mesh.neighbors(cell);
        let far = ring
            .iter()
            .flat_map(|&n| mesh.neighbors(n).iter().copied())
            .find(|v| *v != cell && !ring.contains(v))
            .unwrap();
        let distance = mesh
            .vertex(far)
            .dot(mesh.vertex(cell))
            .clamp(-1.0, 1.0)
            .acos();
        let peak = |vertex: u32| {
            let radius = if vertex == far { 1.08 } else { 1.0 };
            mesh.vertex(vertex).as_vec3() * radius
        };
        assert!((base_point(&mesh, cell, distance * 1.01, peak).length() - 1.08).abs() < 1e-6);
        assert!((base_point(&mesh, cell, distance * 0.99, peak).length() - 1.0).abs() < 1e-6);
    }
}
