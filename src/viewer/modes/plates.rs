//! Plates: every plate in its own colour (see [`super::super::plate_colors`]), its boundaries
//! by what they do (the simulation's classification, see [`BoundaryClass`]), the deforming
//! bands of collisions and rifts, and arrows for the motion of the plates and for the relative
//! motion across their boundaries (#93).
//!
//! Everything comes from the cells and plates every frame keeps, so past frames show alike.

use glam::DVec3;

use super::super::arrows::{ArrowStyle, Sample, base_point};
use super::super::overlay::{
    BoundaryPiece, LineStyle, OverlayBuilder, anchored, category_boundaries,
};
use super::super::plate_colors::PALETTE;
use super::{
    ArrowScale, DEFAULT_ARROW_SPACING, Legend, ModeData, OptionValues, RenderMode, Swatch,
    SwatchShape, plate_motion, rgba,
};
use crate::tectonics::BoundaryClass;

pub struct Plates;

/// Oceanic crust is drawn in the plate's colour times this, so that the continents stand out.
const OCEANIC_SHADE: f32 = 0.72;

/// What a boundary line shows: the classes of the cells on its two sides combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Line {
    Subduction,
    Collision,
    Diffuse,
    Ridge,
    Rift,
    Transform,
    Convergent,
    Divergent,
    Inactive,
}

impl Line {
    const ALL: [Line; 9] = [
        Line::Subduction,
        Line::Collision,
        Line::Diffuse,
        Line::Ridge,
        Line::Rift,
        Line::Transform,
        Line::Convergent,
        Line::Divergent,
        Line::Inactive,
    ];

    /// The line between cells of these classes, and for a subduction zone the side (0 or 1)
    /// of the upper plate. A cell keeps one class (towards the most important of its
    /// neighbouring plates), so at triple junctions a side may describe another boundary: a
    /// subducting side wins, then an overriding one, then the more important class.
    fn of(classes: [Option<BoundaryClass>; 2]) -> (Self, Option<usize>) {
        use BoundaryClass::*;
        // A subducting side facing an overriding one, then an overriding side (the subducting
        // side across may subduct under a third plate), then a subducting side alone.
        for (side, class) in classes.iter().enumerate() {
            if *class == Some(Subducting) && classes[1 - side] == Some(Overriding) {
                return (Line::Subduction, Some(1 - side));
            }
        }
        for (side, class) in classes.iter().enumerate() {
            if *class == Some(Overriding) {
                return (Line::Subduction, Some(side));
            }
        }
        for (side, class) in classes.iter().enumerate() {
            if *class == Some(Subducting) {
                return (Line::Subduction, Some(1 - side));
            }
        }
        let class = classes.into_iter().flatten().min();
        let line = match class {
            Some(Subducting | Overriding) => unreachable!("handled above"),
            Some(Collision) => Line::Collision,
            Some(Diffuse) => Line::Diffuse,
            Some(Ridge) => Line::Ridge,
            Some(Rift) => Line::Rift,
            Some(Transform) => Line::Transform,
            Some(OtherConvergent) => Line::Convergent,
            Some(OtherDivergent) => Line::Divergent,
            Some(Inactive) | None => Line::Inactive,
        };
        (line, None)
    }

    fn color(self) -> [u8; 3] {
        match self {
            Line::Subduction => [225, 30, 30],
            Line::Collision => [160, 40, 190],
            Line::Diffuse => [255, 140, 0],
            Line::Ridge => [25, 80, 255],
            Line::Rift => [0, 200, 235],
            Line::Transform => [30, 170, 50],
            Line::Convergent => [255, 160, 170],
            Line::Divergent => [160, 195, 255],
            Line::Inactive => [255, 255, 255],
        }
    }

    /// Width relative to the main boundaries.
    fn width(self) -> f64 {
        match self {
            Line::Convergent | Line::Divergent | Line::Inactive => 0.6,
            _ => 1.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Line::Subduction => "Subduction",
            Line::Collision => "Collision",
            Line::Diffuse => "Diffuse conv.",
            Line::Ridge => "Ridge",
            Line::Rift => "Rift",
            Line::Transform => "Transform",
            Line::Convergent => "Other conv.",
            Line::Divergent => "Other div.",
            Line::Inactive => "Inactive",
        }
    }
}

/// Dimensions of the overlay as multiples of the arrow spacing (planet radii), so that they
/// look the same at every resolution of the mesh.
const LINE_WIDTH: f64 = 0.05;
const TOOTH_LENGTH: f64 = 0.12;
const TOOTH_SPACING: f64 = 0.35;
/// Relative-motion arrows: their spacing along the boundaries and their size (as a style for
/// arrows this far apart).
const RELATIVE_SPACING: f64 = 0.6;
const RELATIVE_SIZE: f64 = 0.45;
/// Relative speed (cm/yr) drawn with the full length: relative speeds reach about twice the
/// speeds of plates.
const RELATIVE_REFERENCE: f64 = 15.0;
/// Plate-motion arrows closer than this to a relative-motion arrow are left out.
const CLEARANCE: f64 = 0.5;
/// Lift of lines and markers above the surface (planet radii).
const LIFT: f64 = 0.003;
/// Dots of the deforming bands: radius as a share of the cell spacing, at least this much.
const DOT_RADIUS: f64 = 0.17;
const MIN_DOT_RADIUS: f64 = 0.003;
const DOT_COLOR: [u8; 3] = [45, 40, 40];
const BORDER_COLOR: [f32; 4] = [0.05, 0.05, 0.05, 1.0];

/// rad/Myr × planet radii → cm/yr.
const CM_PER_YEAR: f64 = crate::EARTH_RADIUS * 100.0 / 1e6;

fn relative_style(arrow_spacing: f64) -> ArrowStyle {
    ArrowStyle {
        exponent: 0.5,
        color: [1.0, 1.0, 1.0, 1.0],
        ..ArrowStyle::for_spacing(RELATIVE_SIZE * arrow_spacing, RELATIVE_REFERENCE)
    }
}

impl Plates {
    /// The plate id of a cell's plate (its index if the plate is unknown).
    fn plate_id(data: &ModeData, plate: u32) -> u32 {
        data.plates
            .get(plate as usize)
            .map_or(plate, |info| info.id)
    }

    fn omega(data: &ModeData, plate: u32) -> DVec3 {
        data.plates
            .get(plate as usize)
            .map_or(DVec3::ZERO, |info| info.omega())
    }
}

/// Stipples the cells in deforming bands (see [`crate::viewer::CellInfo::in_deforming_band`]).
pub(super) fn deforming_bands(data: &ModeData, overlay: &mut OverlayBuilder) {
    if data.cells.len() != data.mesh.len() {
        return;
    }
    let dot_radius = (DOT_RADIUS * data.mesh.mean_edge_angle()).max(MIN_DOT_RADIUS);
    let dot_color = rgba(DOT_COLOR);
    for (cell, info) in data.cells.iter().enumerate() {
        if info.in_deforming_band() {
            let point = data.position(cell as u32).as_dvec3();
            overlay.dot(
                point + point.normalize_or_zero() * LIFT,
                dot_radius,
                dot_color,
            );
        }
    }
}

/// The swatch of [`deforming_bands`].
pub(super) fn deforming_swatch() -> Swatch {
    Swatch::new(PALETTE[8], SwatchShape::Dots, "Deforming")
}

/// Draws the plate boundaries by what they do, with teeth on the upper plate of subduction
/// zones, `scale` times as wide as in this mode; returns the pieces of the boundaries and
/// their lines.
pub(super) fn boundaries(
    data: &ModeData,
    overlay: &mut OverlayBuilder,
    scale: f64,
) -> (Vec<BoundaryPiece>, Vec<(Line, Option<usize>)>) {
    if data.cells.len() != data.mesh.len() {
        return (Vec::new(), Vec::new());
    }
    let spacing = data.arrow_spacing;
    let position = |cell: u32| data.position(cell).as_dvec3();
    let up = |point: DVec3| point.normalize_or_zero();
    let pieces = category_boundaries(
        data.mesh.faces(),
        |cell| data.cells[cell as usize].plate,
        position,
    );
    let lines: Vec<(Line, Option<usize>)> = pieces
        .iter()
        .map(|piece| Line::of(piece.sides.map(|cell| data.cells[cell as usize].boundary())))
        .collect();
    let width = LINE_WIDTH * spacing * scale;
    for (piece, &(line, _)) in pieces.iter().zip(&lines) {
        let style = LineStyle {
            width: width * line.width(),
            border: 0.3 * width,
            color: rgba(line.color()),
            border_color: BORDER_COLOR,
            lift: LIFT,
        };
        overlay.line(piece.from, piece.to, &style);
    }
    let subduction: Vec<usize> = (0..pieces.len())
        .filter(|&index| lines[index].0 == Line::Subduction)
        .collect();
    let tooth = TOOTH_LENGTH * spacing * scale;
    for kept in anchored(
        subduction.iter().map(|&index| pieces[index].middle()),
        TOOTH_SPACING * spacing,
    ) {
        let index = subduction[kept];
        let piece = &pieces[index];
        let Some(upper) = lines[index].1 else {
            continue;
        };
        let middle = piece.middle();
        let lift = up(middle) * LIFT;
        let towards = piece.towards(upper, position);
        let along = up(middle).cross(towards);
        let base = middle + lift + towards * (width / 2.0);
        overlay.triangle(
            [
                base - along * (tooth / 2.0),
                base + along * (tooth / 2.0),
                base + towards * tooth,
            ],
            rgba(Line::Subduction.color()),
            0.3 * width,
            BORDER_COLOR,
        );
    }
    (pieces, lines)
}

/// The swatches of the boundary lines of [`boundaries`].
pub(super) fn boundary_swatches() -> impl Iterator<Item = Swatch> {
    Line::ALL.into_iter().map(|line| {
        let shape = if line == Line::Subduction {
            SwatchShape::Teeth
        } else {
            SwatchShape::Line
        };
        Swatch::new(line.color(), shape, line.label())
    })
}

impl RenderMode for Plates {
    fn label(&self) -> &'static str {
        "Plates"
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        Some(
            data.cells
                .iter()
                .map(|cell| {
                    let mut color = rgba(data.plate_colors.color(Self::plate_id(data, cell.plate)));
                    if !cell.is_continental() {
                        for channel in &mut color[..3] {
                            *channel *= OCEANIC_SHADE;
                        }
                    }
                    color
                })
                .collect(),
        )
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let spacing = data.arrow_spacing;
        let position = |cell: u32| data.position(cell).as_dvec3();
        let up = |point: DVec3| point.normalize_or_zero();

        deforming_bands(data, overlay);
        overlay.next_layer();
        let (pieces, lines) = boundaries(data, overlay, 1.0);
        let width = LINE_WIDTH * spacing;
        overlay.next_layer();

        // Relative motion across the boundaries: on each side, the motion of that plate
        // relative to the other one (converging arrows point at each other, diverging ones
        // away from each other, transforms pass each other).
        let relative = relative_style(spacing);
        let moving: Vec<usize> = (0..pieces.len())
            .filter(|&index| lines[index].0 != Line::Inactive)
            .collect();
        let mut samples = Vec::new();
        let mut anchors = Vec::new();
        for kept in anchored(
            moving.iter().map(|&index| pieces[index].middle()),
            RELATIVE_SPACING * spacing,
        ) {
            let piece = &pieces[moving[kept]];
            let middle = piece.middle();
            let direction = up(middle);
            let radius = piece
                .sides
                .iter()
                .map(|&cell| position(cell).length())
                .fold(middle.length(), f64::max);
            let [a, b] = piece.sides.map(|cell| data.cells[cell as usize].plate);
            let velocity =
                (Self::omega(data, a) - Self::omega(data, b)).cross(direction) * CM_PER_YEAR;
            anchors.push(middle);
            for (side, vector) in [(0, velocity), (1, -velocity)] {
                let tangent = vector - direction * vector.dot(direction);
                let speed = tangent.length();
                if speed <= 0.0 {
                    continue;
                }
                let along = tangent / speed;
                let length = relative.length(speed);
                let head = 0.5 * relative.width(speed) * relative.head_width;
                let towards = piece.towards(side, position);
                // Its centre on its own side, clear of the boundary line by the head's width,
                // the arrow's length along the normal, or a mix of both.
                let normal = along.dot(towards).abs();
                let offset = width + normal * length / 2.0 + (1.0 - normal) * head;
                let center = middle + towards * offset;
                let base = center - along * (length / 2.0);
                samples.push(Sample {
                    base: up(base) * radius,
                    vector: tangent,
                });
            }
        }
        overlay.arrows(&samples, &relative);
        overlay.next_layer();

        // The motion of the plates over the mantle, away from the relative-motion arrows.
        let style = plate_motion::style(spacing);
        let clearance = CLEARANCE * spacing;
        let plate_samples: Vec<Sample> = data
            .arrow_cells
            .iter()
            .filter(|&&cell| {
                let point = data.mesh.vertex(cell);
                anchors
                    .iter()
                    .all(|anchor| up(*anchor).distance(point) > clearance)
            })
            .map(|&cell| {
                let plate = data.cells[cell as usize].plate;
                Sample {
                    base: base_point(data.mesh, cell, spacing, |vertex| data.position(vertex)),
                    vector: Self::omega(data, plate).cross(data.mesh.vertex(cell)) * CM_PER_YEAR,
                }
            })
            .collect();
        overlay.arrows(&plate_samples, &style);
    }

    fn legend(&self, _options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let spacing = data.map_or(DEFAULT_ARROW_SPACING, |data| data.arrow_spacing);
        let mut swatches = vec![Swatch::new(
            PALETTE[0],
            SwatchShape::Palette(PALETTE[..8].to_vec()),
            "Plates",
        )];
        swatches.extend(boundary_swatches());
        swatches.push(deforming_swatch());
        Legend {
            description: "Each plate in its own colour for its life (oceanic crust darker), its \
                          boundaries by what they do, and its motion. Teeth point into the \
                          upper plate of a subduction zone; dots mark crust deforming behind a \
                          collision or beside a rift."
                .into(),
            scale: None,
            swatches,
            arrows: vec![
                ArrowScale::new(
                    &plate_motion::style(spacing),
                    &[1.0, 3.0, plate_motion::REFERENCE_SPEED],
                    "cm/yr",
                )
                .with_label("Plate motion over the mantle"),
                ArrowScale::new(
                    &relative_style(spacing),
                    &[1.0, 5.0, RELATIVE_REFERENCE],
                    "cm/yr",
                )
                .with_label("Relative motion across boundaries"),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_combine_the_classes_of_both_sides() {
        use BoundaryClass::*;
        assert_eq!(
            Line::of([Some(Subducting), Some(Overriding)]),
            (Line::Subduction, Some(1))
        );
        assert_eq!(
            Line::of([Some(Collision), Some(Subducting)]),
            (Line::Subduction, Some(0))
        );
        // Both sides subducting (one under a third plate): teeth on one of them.
        assert_eq!(
            Line::of([Some(Subducting), Some(Subducting)]),
            (Line::Subduction, Some(1))
        );
        // An overriding side wins over a subducting side that subducts under a third plate.
        assert_eq!(Line::of([Some(Subducting), Some(Overriding)]).1, Some(1));
        assert_eq!(
            Line::of([Some(Overriding), Some(Overriding)]),
            (Line::Subduction, Some(0))
        );
        // A subducting side hidden by a triple junction: the overriding side still marks it.
        assert_eq!(
            Line::of([Some(Transform), Some(Overriding)]),
            (Line::Subduction, Some(1))
        );
        assert_eq!(Line::of([Some(Ridge), Some(Ridge)]), (Line::Ridge, None));
        assert_eq!(
            Line::of([Some(Rift), Some(OtherDivergent)]),
            (Line::Rift, None)
        );
        assert_eq!(
            Line::of([Some(Diffuse), Some(OtherConvergent)]),
            (Line::Diffuse, None)
        );
        assert_eq!(Line::of([None, Some(Inactive)]), (Line::Inactive, None));
        assert_eq!(Line::of([None, None]), (Line::Inactive, None));
        // Every line has its own colour.
        for (index, line) in Line::ALL.iter().enumerate() {
            assert!(
                Line::ALL[..index]
                    .iter()
                    .all(|other| other.color() != line.color())
            );
        }
    }

    #[test]
    fn a_planet_in_plates() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, plate_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let mut planet = crate::build_planet(&cli).unwrap();
        planet.step(cli.dt);
        let cells = cell_infos(&planet);
        let plates = plate_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let mut plate_colors = PlateColors::default();
        plate_colors.update(mesh, &cells, &plates);
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 2);
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &plates,
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &arrow_cells,
            arrow_spacing,
            options: &OptionValues::default(),
            abiotic: false,
        };
        let colors = Plates.colors(&data).unwrap();
        assert_eq!(colors.len(), cells.len());
        // Cells of a plate have its colour (darker over oceanic crust).
        for (cell, color) in cells.iter().zip(&colors) {
            let expected = rgba(plate_colors.color(plates[cell.plate as usize].id));
            let shade = if cell.is_continental() {
                1.0
            } else {
                OCEANIC_SHADE
            };
            assert!((color[0] - expected[0] * shade).abs() < 1e-6);
        }
        let mut overlay = OverlayBuilder::default();
        Plates.overlay(&data, &mut overlay);
        let vertices = overlay.finish();
        assert!(!vertices.is_empty());
        assert_eq!(vertices.len() % 3, 0);
        // Lines are drawn in the colours of the classes; white relative and yellow plate arrows.
        let has = |color: [f32; 4]| vertices.iter().any(|vertex| vertex.color == color);
        assert!(has([1.0, 1.0, 1.0, 1.0]));
        assert!(has(plate_motion::style(arrow_spacing).color));
        let legend = Plates.legend(&OptionValues::default(), Some(&data));
        assert_eq!(legend.arrows.len(), 2);
        assert_eq!(legend.swatches.len(), Line::ALL.len() + 2);
    }
}
