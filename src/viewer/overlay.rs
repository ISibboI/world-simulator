//! What render modes draw over the planet: arrows, lines and markers, as one list of coloured
//! triangles for the renderer ([`OverlayVertex`]).
//!
//! A mode adds its shapes to an [`OverlayBuilder`] (see `modes::RenderMode::overlay`). Every
//! shape has a fill and, around it, a dark border for contrast on any colour. The renderer
//! draws the overlay depth-tested against the planet (mountains in front hide it) but without
//! writing depth, in the order of the triangles: so the builder puts, per layer, all borders
//! before all fills (a border never covers the fill of its neighbour), and later layers over
//! earlier ones (e.g. arrows over the lines they cross).
//!
//! Shapes lie in the plane tangent to the sphere at their position; positions are on the
//! displayed surface, and the shapes are lifted above it by the caller (see
//! [`super::arrows::ArrowStyle::lift`], [`LineStyle::lift`]).

use glam::DVec3;

use super::arrows::{self, ArrowStyle, OverlayVertex, Sample};

/// The width and colours of lines; widths in planet radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineStyle {
    pub width: f64,
    /// Width of the border on each side (0 for none).
    pub border: f64,
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    /// Height above the given points in planet radii.
    pub lift: f64,
}

/// Collects the shapes of an overlay; see the module documentation.
#[derive(Debug, Default)]
pub struct OverlayBuilder {
    /// Finished layers: their vertices, borders first.
    done: Vec<OverlayVertex>,
    borders: Vec<OverlayVertex>,
    fills: Vec<OverlayVertex>,
}

impl OverlayBuilder {
    /// Starts a new layer: what follows is drawn over everything before.
    pub fn next_layer(&mut self) {
        self.done.append(&mut self.borders);
        self.done.append(&mut self.fills);
    }

    /// The triangles, layer by layer, borders before fills.
    pub fn finish(mut self) -> Vec<OverlayVertex> {
        self.next_layer();
        self.done
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty() && self.borders.is_empty() && self.fills.is_empty()
    }

    fn push(target: &mut Vec<OverlayVertex>, triangle: [DVec3; 3], color: [f32; 4]) {
        target.extend(triangle.iter().map(|point| OverlayVertex {
            position: point.as_vec3().to_array(),
            color,
        }));
    }

    /// Arrows of a field of vectors (see [`arrows::arrows`]).
    pub fn arrows(&mut self, samples: &[Sample], style: &ArrowStyle) {
        for arrow in arrows::arrows(samples, style) {
            for triangle in arrows::arrow_triangles(&arrow, style, style.border) {
                Self::push(&mut self.borders, triangle, style.border_color);
            }
            for triangle in arrows::arrow_triangles(&arrow, style, 0.0) {
                Self::push(&mut self.fills, triangle, style.color);
            }
        }
    }

    /// A straight line from `from` to `to` (points on or above the surface), lifted by the
    /// style's lift. It reaches half its width beyond its ends, so that the segments of a
    /// polyline join without gaps.
    pub fn line(&mut self, from: DVec3, to: DVec3, style: &LineStyle) {
        let middle = (from + to) / 2.0;
        let up = middle.normalize_or_zero();
        let along = to - from;
        let along = (along - up * along.dot(up)).normalize_or_zero();
        if up == DVec3::ZERO || along == DVec3::ZERO {
            return;
        }
        let (from, to) = (from + up * style.lift, to + up * style.lift);
        let left = up.cross(along);
        let quad = |half: f64| {
            let (a, b) = (from - along * half, to + along * half);
            [
                [a - left * half, b - left * half, b + left * half],
                [a - left * half, b + left * half, a + left * half],
            ]
        };
        if style.border > 0.0 {
            for triangle in quad(style.width / 2.0 + style.border) {
                Self::push(&mut self.borders, triangle, style.border_color);
            }
        }
        for triangle in quad(style.width / 2.0) {
            Self::push(&mut self.fills, triangle, style.color);
        }
    }

    /// A triangle (corners counter-clockwise seen from outside, or in any order: they are
    /// sorted), e.g. the teeth of a subduction zone, with a border of `border` around it.
    pub fn triangle(
        &mut self,
        corners: [DVec3; 3],
        color: [f32; 4],
        border: f64,
        border_color: [f32; 4],
    ) {
        let [a, b, c] = corners;
        let up = ((a + b + c) / 3.0).normalize_or_zero();
        // Counter-clockwise seen from outside.
        let [a, b, c] = if (b - a).cross(c - a).dot(up) >= 0.0 {
            [a, b, c]
        } else {
            [a, c, b]
        };
        if border > 0.0 {
            let center = (a + b + c) / 3.0;
            // Pushed outwards from the centre: twice the border at the corners, which are
            // sharper than the sides.
            let grow = |point: DVec3| {
                let out = (point - center).normalize_or_zero();
                point + out * 2.0 * border
            };
            Self::push(&mut self.borders, [grow(a), grow(b), grow(c)], border_color);
        }
        Self::push(&mut self.fills, [a, b, c], color);
    }

    /// A small square marker (a diamond) of `radius` around `center` (a point on or above the
    /// surface), without border, e.g. to stipple an area.
    pub fn dot(&mut self, center: DVec3, radius: f64, color: [f32; 4]) {
        let up = center.normalize_or_zero();
        if up == DVec3::ZERO {
            return;
        }
        let east = DVec3::Z.cross(up).try_normalize().unwrap_or(DVec3::X);
        let north = up.cross(east);
        let [e, n, w, s] =
            [east, north, -east, -north].map(|direction| center + direction * radius);
        Self::push(&mut self.fills, [e, n, w], color);
        Self::push(&mut self.fills, [e, w, s], color);
    }
}

/// A piece of the boundary between two categories of cells (see [`category_boundaries`]): a
/// straight segment inside one triangle of the mesh, between the cells `sides` (one of each
/// category).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryPiece {
    pub from: DVec3,
    pub to: DVec3,
    pub sides: [u32; 2],
}

impl BoundaryPiece {
    /// The direction (tangent to the sphere, unit length) from the piece towards the side of
    /// `sides[side]`, given the positions of the cells.
    pub fn towards(&self, side: usize, position: impl Fn(u32) -> DVec3) -> DVec3 {
        let middle = (self.from + self.to) / 2.0;
        let up = middle.normalize_or_zero();
        let along = (self.to - self.from).normalize_or_zero();
        let across = up.cross(along).normalize_or_zero();
        let to_cell = position(self.sides[side]) - middle;
        if across.dot(to_cell) >= 0.0 {
            across
        } else {
            -across
        }
    }

    pub fn middle(&self) -> DVec3 {
        (self.from + self.to) / 2.0
    }
}

/// The boundaries between cells of different categories (e.g. plates, climate zones), as
/// straight pieces through the triangles of the mesh ("marching triangles"): in a triangle with
/// two categories, from the middle of one crossing edge to the middle of the other; in a
/// triangle with three, from the middle of each edge to the centre. Points lie on the displayed
/// triangles (`position` gives the displayed position of a cell), so the pieces join into lines
/// along the boundaries. The pieces are in the order of the faces (deterministic).
pub fn category_boundaries(
    faces: &[[u32; 3]],
    category: impl Fn(u32) -> u32,
    position: impl Fn(u32) -> DVec3,
) -> Vec<BoundaryPiece> {
    let mut pieces = Vec::new();
    for &[a, b, c] in faces {
        let (ka, kb, kc) = (category(a), category(b), category(c));
        if ka == kb && kb == kc {
            continue;
        }
        let middle = |x: u32, y: u32| (position(x) + position(y)) / 2.0;
        if ka != kb && kb != kc && ka != kc {
            let center = (position(a) + position(b) + position(c)) / 3.0;
            for (x, y) in [(a, b), (b, c), (c, a)] {
                pieces.push(BoundaryPiece {
                    from: middle(x, y),
                    to: center,
                    sides: [x, y],
                });
            }
            continue;
        }
        // The odd one out, and the other two.
        let (odd, x, y) = if ka == kb {
            (c, a, b)
        } else if kb == kc {
            (a, b, c)
        } else {
            (b, c, a)
        };
        pieces.push(BoundaryPiece {
            from: middle(odd, x),
            to: middle(odd, y),
            sides: [odd, x],
        });
    }
    pieces
}

/// Indices of `points` about `spacing` apart, anchored in space: per cube of a fixed grid of
/// that size, the point nearest its centre, then of those, greedily in grid order, the ones at
/// least 0.7 `spacing` from each other. A point stays chosen while the points around it move
/// a little (a boundary from one frame to the next), so markers and arrows along lines do not
/// jump around.
pub fn anchored(points: impl IntoIterator<Item = DVec3>, spacing: f64) -> Vec<usize> {
    let mut best: crate::hash::HashMap<glam::IVec3, (usize, DVec3, f64)> = Default::default();
    for (index, point) in points.into_iter().enumerate() {
        let scaled = point / spacing;
        let key = scaled.floor().as_ivec3();
        let distance = scaled.distance_squared(key.as_dvec3() + DVec3::splat(0.5));
        let entry = best.entry(key).or_insert((index, point, distance));
        if distance < entry.2 {
            *entry = (index, point, distance);
        }
    }
    let mut chosen: Vec<(glam::IVec3, usize, DVec3)> = best
        .into_iter()
        .map(|(key, (index, point, _))| (key, index, point))
        .collect();
    chosen.sort_unstable_by_key(|&(key, index, _)| (key.to_array(), index));
    thin(chosen.iter().map(|&(_, _, point)| point), 0.7 * spacing)
        .into_iter()
        .map(|kept| chosen[kept].1)
        .collect()
}

/// Indices of `points` that are at least `spacing` (chord length) apart, greedily in order:
/// a deterministic thinning.
pub fn thin(points: impl IntoIterator<Item = DVec3>, spacing: f64) -> Vec<usize> {
    // A coarse grid of cells of the spacing's size, so that each point checks its neighbours
    // only.
    let cell = |point: DVec3| (point / spacing).floor().as_ivec3();
    let mut grid: crate::hash::HashMap<glam::IVec3, Vec<DVec3>> = Default::default();
    let mut kept = Vec::new();
    for (index, point) in points.into_iter().enumerate() {
        let home = cell(point);
        let near = (-1..=1).any(|x| {
            (-1..=1).any(|y| {
                (-1..=1).any(|z| {
                    grid.get(&(home + glam::IVec3::new(x, y, z)))
                        .is_some_and(|others| {
                            others.iter().any(|other| other.distance(point) < spacing)
                        })
                })
            })
        });
        if !near {
            grid.entry(home).or_default().push(point);
            kept.push(index);
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::SphereMesh;

    fn style() -> LineStyle {
        LineStyle {
            width: 0.01,
            border: 0.002,
            color: [1.0, 0.0, 0.0, 1.0],
            border_color: [0.0, 0.0, 0.0, 1.0],
            lift: 0.0,
        }
    }

    #[test]
    fn layers_draw_borders_first_and_later_layers_last() {
        let mut overlay = OverlayBuilder::default();
        assert!(overlay.is_empty());
        overlay.line(DVec3::X, DVec3::new(1.0, 0.1, 0.0).normalize(), &style());
        overlay.line(DVec3::Y, DVec3::new(0.1, 1.0, 0.0).normalize(), &style());
        overlay.next_layer();
        let arrow_style = ArrowStyle::for_spacing(0.1, 10.0);
        overlay.arrows(
            &[Sample {
                base: DVec3::Z,
                vector: DVec3::X * 10.0,
            }],
            &arrow_style,
        );
        assert!(!overlay.is_empty());
        let vertices = overlay.finish();
        // Two lines of two triangles each with borders, then an arrow of three with borders.
        assert_eq!(vertices.len(), 2 * 2 * 2 * 3 + 2 * 3 * 3);
        let colors: Vec<[f32; 4]> = vertices.iter().map(|vertex| vertex.color).collect();
        assert!(colors[..12].iter().all(|&c| c == style().border_color));
        assert!(colors[12..24].iter().all(|&c| c == style().color));
        assert!(
            colors[24..33]
                .iter()
                .all(|&c| c == arrow_style.border_color)
        );
        assert!(colors[33..].iter().all(|&c| c == arrow_style.color));
    }

    #[test]
    fn lines_are_tangent_quads_of_their_width() {
        let mut overlay = OverlayBuilder::default();
        let style = LineStyle {
            border: 0.0,
            lift: 0.01,
            ..style()
        };
        overlay.line(DVec3::Z, DVec3::new(0.1, 0.0, 1.0).normalize(), &style);
        let vertices = overlay.finish();
        assert_eq!(vertices.len(), 6);
        let ys: Vec<f32> = vertices.iter().map(|vertex| vertex.position[1]).collect();
        let width =
            ys.iter().fold(f32::MIN, |a, &b| a.max(b)) - ys.iter().fold(f32::MAX, |a, &b| a.min(b));
        assert!((width - 0.01).abs() < 1e-6, "{width}");
        // Lifted above the points; counter-clockwise from outside.
        assert!(vertices.iter().all(|vertex| vertex.position[2] > 1.0));
        for triangle in vertices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| glam::Vec3::from(triangle[i].position));
            assert!((b - a).cross(c - a).dot(a) > 0.0);
        }
    }

    #[test]
    fn triangles_face_outwards_and_dots_are_drawn() {
        let mut overlay = OverlayBuilder::default();
        let corners = [
            DVec3::Z,
            DVec3::new(0.0, 0.1, 1.0),
            DVec3::new(0.1, 0.0, 1.0),
        ];
        overlay.triangle(corners, [1.0; 4], 0.01, [0.0, 0.0, 0.0, 1.0]);
        overlay.dot(DVec3::X, 0.01, [0.5; 4]);
        let vertices = overlay.finish();
        assert_eq!(vertices.len(), 3 + 3 + 6);
        for triangle in vertices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| glam::Vec3::from(triangle[i].position));
            assert!((b - a).cross(c - a).dot(a + b + c) > 0.0);
        }
    }

    #[test]
    fn category_boundaries_follow_the_split() {
        let mesh = SphereMesh::icosphere(3);
        // Northern and southern hemisphere, and a third category in a cap.
        let category = |cell: u32| {
            let z = mesh.vertex(cell).z;
            if z > 0.8 {
                2
            } else if z > 0.0 {
                1
            } else {
                0
            }
        };
        let pieces = category_boundaries(mesh.faces(), category, |cell| mesh.vertex(cell));
        assert!(!pieces.is_empty());
        for piece in &pieces {
            let [a, b] = piece.sides;
            assert_ne!(category(a), category(b));
            // Between the cells' rings: near the equator or the cap's edge.
            let z = piece.middle().normalize().z;
            assert!(z.abs() < 0.2 || (z - 0.8).abs() < 0.2, "{z}");
            // The direction towards a side points to it.
            let towards = piece.towards(0, |cell| mesh.vertex(cell));
            assert!(towards.dot(mesh.vertex(a) - piece.middle()) > 0.0);
            assert!(piece.towards(1, |cell| mesh.vertex(cell)).dot(towards) < 0.0);
        }
        // The pieces join: every end point is shared by another piece (closed lines).
        let key = |point: DVec3| (point * 1e6).round().as_ivec3();
        let mut ends: crate::hash::HashMap<glam::IVec3, usize> = Default::default();
        for piece in &pieces {
            *ends.entry(key(piece.from)).or_default() += 1;
            *ends.entry(key(piece.to)).or_default() += 1;
        }
        assert!(ends.values().all(|&count| count >= 2));
    }

    #[test]
    fn thinning_keeps_points_apart() {
        let points: Vec<DVec3> = (0..100)
            .map(|i| DVec3::new(f64::from(i) * 0.01, 0.0, 1.0))
            .collect();
        let kept = thin(points.iter().copied(), 0.05);
        assert_eq!(kept[0], 0);
        for pair in kept.windows(2) {
            assert!(points[pair[1]].distance(points[pair[0]]) >= 0.05);
        }
        assert!((16..=21).contains(&kept.len()), "{}", kept.len());
    }

    #[test]
    fn anchored_points_stay_when_the_others_move() {
        let line = |shift: f64| -> Vec<DVec3> {
            (0..200)
                .map(|i| DVec3::new(f64::from(i) * 0.005 + shift, 0.31, 1.0))
                .collect()
        };
        let spacing = 0.05;
        let before = line(0.0);
        let chosen = |points: &[DVec3]| -> Vec<DVec3> {
            anchored(points.iter().copied(), spacing)
                .into_iter()
                .map(|index| points[index])
                .collect()
        };
        let first = chosen(&before);
        assert!((15..=21).contains(&first.len()), "{}", first.len());
        for pair in first.windows(2) {
            assert!(pair[0].distance(pair[1]) >= 0.7 * spacing);
        }
        // Shifted by less than a point's distance, most chosen points stay close.
        let second = chosen(&line(0.002));
        let stayed = first
            .iter()
            .filter(|point| second.iter().any(|other| other.distance(**point) < 0.003))
            .count();
        assert!(
            stayed * 10 >= first.len() * 8,
            "{stayed} of {}",
            first.len()
        );
    }
}
