//! Hydrology (#102): rivers by their discharge, drainage basins (those that drain to the ocean
//! and the endorheic ones in palettes of their own), lakes by their depth and dry closed
//! depressions.
//!
//! Lakes follow Sebastian's rule of #128: a closed depression with at least 1 m of lake water
//! is a lake, otherwise land (see [`CellInfo::water_body`]). Everything comes from the cells of
//! the frame (the receivers, the ocean, depression and lake bits, the lake depth and the
//! discharge; see [`Drainage`]), so past frames show alike.

use glam::DVec3;

use super::super::CellInfo;
use super::super::cell_info::WaterBody;
use super::super::hydrology::{Drainage, MIN_BASIN, NO_BASIN, receiver};
use super::super::overlay::{LineStyle, OverlayBuilder, category_boundaries};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Swatch,
    SwatchShape, rgba,
};

pub struct Hydrology;

/// Indices of the options.
const BASINS: usize = 0;
const RIVERS: usize = 1;
const OUTLINES: usize = 2;

const OPTIONS: [ModeOption; 3] = [
    ModeOption {
        label: "Basins",
        hover: "Colour the land by drainage basin (each basin, or only the endorheic ones, \
                whose water never reaches the ocean), or show the elevation",
        kind: OptionKind::Choice(&["All", "Endorheic", "Elevation"]),
    },
    ModeOption {
        label: "Rivers",
        hover: "Rivers from about 3000 m³/s (as in the topographic mode), from about 300 m³/s, \
                or none",
        kind: OptionKind::Choice(&["Large", "All", "Off"]),
    },
    ModeOption {
        label: "Divides",
        hover: "Lines along the drainage divides between basins",
        kind: OptionKind::Switch { default: true },
    },
];

/// How the land is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Basins {
    All,
    Endorheic,
    Elevation,
}

impl Basins {
    fn of(options: &OptionValues) -> Self {
        match options.get(BASINS) {
            1 => Basins::Endorheic,
            2 => Basins::Elevation,
            _ => Basins::All,
        }
    }
}

/// Discharge (m³/yr) from which rivers are drawn, by the option, or `None` for no rivers.
fn river_threshold(options: &OptionValues) -> Option<f64> {
    match options.get(RIVERS) {
        // As the topographic mode's default `--river-threshold`.
        0 => Some(LARGE_RIVER),
        1 => Some(SMALL_RIVER),
        _ => None,
    }
}

/// ~3200 m³/s and ~320 m³/s. A cell of the default grid makes ~4 × 10⁹ m³/yr of runoff at
/// 0.3 m/yr, so the smaller rivers drain a few cells.
const LARGE_RIVER: f64 = 1e11;
const SMALL_RIVER: f64 = 1e10;
/// Discharge (m³/yr) drawn with the full width and darkest colour: ~300 000 m³/s, a little
/// more than the Amazon.
const LARGEST_RIVER: f64 = 1e13;

/// Basins draining to the ocean: light colours without the warm hues of the endorheic ones
/// (ColorBrewer "Set3" and "Pastel1" without orange, tan and grey).
const OCEAN_BASINS: [[u8; 3]; 10] = [
    [141, 211, 199],
    [255, 255, 179],
    [190, 186, 218],
    [128, 177, 211],
    [179, 222, 105],
    [252, 205, 229],
    [188, 128, 189],
    [204, 235, 197],
    [102, 194, 165],
    [158, 218, 229],
];
/// Endorheic basins: warm, sandy hues.
const ENDORHEIC_BASINS: [[u8; 3]; 6] = [
    [253, 180, 98],
    [229, 196, 148],
    [214, 140, 90],
    [240, 160, 120],
    [196, 156, 118],
    [245, 205, 120],
];
/// Endorheic land when only those basins are coloured.
const ENDORHEIC: [u8; 3] = [222, 150, 80];
/// Small coastal basins, and land draining to the ocean when only endorheic basins are shown.
const OTHER_LAND: [u8; 3] = [222, 222, 212];
const OCEAN: [u8; 3] = [42, 72, 122];
/// Dry closed depressions are drawn in their basin's colour times this, and stippled.
const DEPRESSION_SHADE: f32 = 0.8;
const DOT_COLOR: [u8; 3] = [45, 40, 40];
/// Lakes: light for shallow, deep blue for deep ones (by the depth in m, logarithmic).
const LAKE_SHALLOW: [u8; 3] = [150, 205, 245];
const LAKE_DEEP: [u8; 3] = [20, 55, 160];
/// Rivers: from light (small) to dark blue (large), with a dark border.
const RIVER_SMALL: [u8; 3] = [90, 160, 255];
const RIVER_LARGE: [u8; 3] = [10, 45, 175];
const RIVER_BORDER: [f32; 4] = [0.02, 0.08, 0.25, 1.0];
const DIVIDE_COLOR: [u8; 3] = [60, 50, 50];

/// Widths as multiples of the arrow spacing (planet radii), as in the other modes.
const RIVER_MIN_WIDTH: f64 = 0.02;
const RIVER_MAX_WIDTH: f64 = 0.08;
const DIVIDE_WIDTH: f64 = 0.02;
/// Dots of closed lakes and dry depressions: radius as a share of the cell spacing.
const DOT_RADIUS: f64 = 0.12;
const MIN_DOT_RADIUS: f64 = 0.002;
/// Lift above the surface (planet radii).
const LIFT: f64 = 0.003;

/// A stable index into a palette of `length` for a basin, from its end cell (so a basin keeps
/// its colour as long as its mouth stays in the same cell).
fn palette_index(end: u32, length: usize) -> usize {
    // SplitMix32-like mixing, so that neighbouring end cells get unrelated colours.
    let mut x = end.wrapping_add(0x9e37_79b9);
    x = (x ^ (x >> 16)).wrapping_mul(0x85eb_ca6b);
    x = (x ^ (x >> 13)).wrapping_mul(0xc2b2_ae35);
    x ^= x >> 16;
    x as usize % length
}

/// The lake depth scale: on log10 of the depth in m.
fn depth_scale() -> ColorScale {
    ColorScale {
        stops: vec![(0.0, LAKE_SHALLOW), (3.0, LAKE_DEEP)],
        title: "Lake depth (m)".into(),
        ticks: vec![
            (0.0, "1".into()),
            (1.0, "10".into()),
            (2.0, "100".into()),
            (3.0, "1000".into()),
        ],
    }
}

/// The colour of a lake of this depth (m).
fn lake_color(depth: f32) -> [u8; 3] {
    depth_scale()
        .color(f64::from(depth.max(1.0)).log10())
        .unwrap_or(LAKE_SHALLOW)
}

/// The colour of the land of a cell in its basin.
fn basin_color(drainage: &Drainage, cell: u32, basins: Basins) -> Option<[u8; 3]> {
    let end = drainage.basin_of(cell)?;
    let endorheic = drainage.is_endorheic(end);
    Some(match (basins, endorheic) {
        (Basins::Endorheic | Basins::Elevation, false) => OTHER_LAND,
        (Basins::Endorheic | Basins::Elevation, true) => ENDORHEIC,
        (Basins::All, true) => ENDORHEIC_BASINS[palette_index(end, ENDORHEIC_BASINS.len())],
        (Basins::All, false) if drainage.size_of(end) < MIN_BASIN => OTHER_LAND,
        (Basins::All, false) => OCEAN_BASINS[palette_index(end, OCEAN_BASINS.len())],
    })
}

/// The category of a cell for the divides: its basin, one for all small coastal basins, and
/// [`NO_BASIN`] for the ocean.
fn divide_category(drainage: &Drainage, cell: u32) -> u32 {
    match drainage.basin_of(cell) {
        None => NO_BASIN,
        Some(end) if !drainage.is_endorheic(end) && drainage.size_of(end) < MIN_BASIN => {
            NO_BASIN - 1
        }
        Some(end) => end,
    }
}

/// The colour of every cell.
fn colors(data: &ModeData, drainage: &Drainage) -> Vec<[f32; 4]> {
    let basins = Basins::of(data.options);
    data.cells
        .iter()
        .enumerate()
        .map(|(cell, info)| {
            let land = |shade: f32| {
                let mut color = match basins {
                    Basins::Elevation => data
                        .vertices
                        .get(cell)
                        .map_or(rgba(OTHER_LAND), |vertex| vertex.color),
                    _ => rgba(basin_color(drainage, cell as u32, basins).unwrap_or(OTHER_LAND)),
                };
                for channel in &mut color[..3] {
                    *channel *= shade;
                }
                color
            };
            match info.water_body() {
                WaterBody::Ocean => match basins {
                    Basins::Elevation => land(1.0),
                    _ => rgba(OCEAN),
                },
                WaterBody::Lake(depth) => rgba(lake_color(depth)),
                WaterBody::DryDepression => land(DEPRESSION_SHADE),
                WaterBody::Land => land(1.0),
            }
        })
        .collect()
}

/// Whether a river is drawn on a cell: land that is not under a lake.
fn river_cell(info: &CellInfo) -> bool {
    matches!(
        info.water_body(),
        WaterBody::Land | WaterBody::DryDepression
    )
}

/// The share (0 to 1) of a discharge between the threshold and [`LARGEST_RIVER`], on a
/// logarithmic axis.
fn river_size(discharge: f64, threshold: f64) -> f64 {
    let span = (LARGEST_RIVER / threshold).log10();
    if span <= 0.0 {
        return 1.0;
    }
    ((discharge / threshold).max(1.0).log10() / span).clamp(0.0, 1.0)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    std::array::from_fn(|i| {
        let (a, b) = (f64::from(a[i]), f64::from(b[i]));
        (a + (b - a) * t).round() as u8
    })
}

impl RenderMode for Hydrology {
    fn label(&self) -> &'static str {
        "Hydrology"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let drainage = Drainage::of(data.mesh, data.cells);
        Some(colors(data, &drainage))
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let drainage = Drainage::of(data.mesh, data.cells);
        let spacing = data.arrow_spacing;
        let position = |cell: u32| data.position(cell).as_dvec3();
        let up = |point: DVec3| point.normalize_or_zero();

        // Closed lakes and dry depressions, stippled.
        let dot_radius = (DOT_RADIUS * data.mesh.mean_edge_angle()).max(MIN_DOT_RADIUS);
        for (cell, info) in data.cells.iter().enumerate() {
            let stippled = match info.water_body() {
                WaterBody::Lake(_) => drainage.is_closed(cell as u32),
                WaterBody::DryDepression => true,
                WaterBody::Ocean | WaterBody::Land => false,
            };
            if stippled {
                let point = position(cell as u32);
                overlay.dot(point + up(point) * LIFT, dot_radius, rgba(DOT_COLOR));
            }
        }
        overlay.next_layer();

        // The drainage divides between basins on land.
        if data.options.is_on(OUTLINES) && Basins::of(data.options) != Basins::Elevation {
            let style = LineStyle {
                width: DIVIDE_WIDTH * spacing,
                border: 0.0,
                color: rgba(DIVIDE_COLOR),
                border_color: rgba(DIVIDE_COLOR),
                lift: LIFT,
            };
            let category = |cell: u32| divide_category(&drainage, cell);
            for piece in category_boundaries(data.mesh.faces(), category, position) {
                if piece.sides.iter().all(|&cell| category(cell) != NO_BASIN) {
                    overlay.line(piece.from, piece.to, &style);
                }
            }
            overlay.next_layer();
        }

        // Rivers, from each cell to the one its water flows to; into the sea or a lake only to
        // the shore (half way). Small ones first, so that large rivers lie on top.
        let Some(threshold) = river_threshold(data.options) else {
            return;
        };
        let mut rivers: Vec<(f64, u32, u32)> = data
            .cells
            .iter()
            .enumerate()
            .filter(|(_, info)| river_cell(info) && f64::from(info.discharge) >= threshold)
            .filter_map(|(cell, info)| {
                let to = receiver(data.mesh, data.cells, cell as u32)?;
                Some((f64::from(info.discharge), cell as u32, to))
            })
            .collect();
        rivers.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (discharge, from, to) in rivers {
            let size = river_size(discharge, SMALL_RIVER);
            let width = (RIVER_MIN_WIDTH + (RIVER_MAX_WIDTH - RIVER_MIN_WIDTH) * size) * spacing;
            let style = LineStyle {
                width,
                border: 0.15 * width,
                color: rgba(mix(RIVER_SMALL, RIVER_LARGE, size)),
                border_color: RIVER_BORDER,
                lift: LIFT,
            };
            let start = position(from);
            let end = if river_cell(&data.cells[to as usize]) {
                position(to)
            } else {
                (start + position(to)) / 2.0
            };
            overlay.line(start, end, &style);
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let basins = Basins::of(options);
        let mut description = "Rivers by their discharge, drainage basins and lakes. Endorheic \
                               basins never reach the ocean: their water evaporates from \
                               closed lakes. Closed depressions with at least 1 m of water are \
                               lakes, the others dry land."
            .to_owned();
        if let Some(data) = data.filter(|data| data.cells.len() == data.mesh.len()) {
            let drainage = Drainage::of(data.mesh, data.cells);
            let summary = drainage.summary(data.cells);
            description += &format!(
                "\n{} basins of at least {MIN_BASIN} cells drain to the ocean; the largest basin \
                 covers {:.1}% of the land{}. Endorheic: {:.1}% of the land in {} basins. \
                 Lakes: {:.1}% of the land ({:.1}% closed); dry depressions {:.1}%.",
                summary.large_ocean_basins,
                summary.percent(summary.largest),
                if summary.largest_endorheic {
                    " (endorheic)"
                } else {
                    ""
                },
                summary.percent(summary.endorheic),
                summary.endorheic_basins,
                summary.percent(summary.lake),
                summary.percent(summary.closed_lake),
                summary.percent(summary.dry_depression),
            );
        }
        let mut swatches = Vec::new();
        if basins != Basins::Elevation {
            swatches.push(Swatch::fill(OCEAN, "Ocean"));
        }
        match basins {
            Basins::All => {
                swatches.push(Swatch::new(
                    OCEAN_BASINS[0],
                    SwatchShape::Palette(OCEAN_BASINS.to_vec()),
                    "Basins draining to the ocean",
                ));
                swatches.push(Swatch::new(
                    ENDORHEIC_BASINS[0],
                    SwatchShape::Palette(ENDORHEIC_BASINS.to_vec()),
                    "Endorheic basins",
                ));
                swatches.push(Swatch::fill(
                    OTHER_LAND,
                    format!("Small coastal basins (< {MIN_BASIN} cells)"),
                ));
            }
            Basins::Endorheic | Basins::Elevation => {
                if basins == Basins::Endorheic {
                    swatches.push(Swatch::fill(OTHER_LAND, "Drains to the ocean"));
                }
                swatches.push(Swatch::fill(ENDORHEIC, "Endorheic"));
            }
        }
        if basins == Basins::Elevation {
            // Only lakes are recoloured there; endorheic land keeps the elevation colours.
            swatches.pop();
        }
        swatches.push(Swatch::new(
            LAKE_SHALLOW,
            SwatchShape::Dots,
            "Closed lake (no outflow)",
        ));
        swatches.push(Swatch::new(
            OTHER_LAND.map(|c| (f32::from(c) * DEPRESSION_SHADE) as u8),
            SwatchShape::Dots,
            "Dry closed depression",
        ));
        if let Some(threshold) = river_threshold(options) {
            let small = if threshold >= LARGE_RIVER {
                "River ≥ 3000 m³/s"
            } else {
                "River ≥ 300 m³/s"
            };
            swatches.push(Swatch::new(
                mix(RIVER_SMALL, RIVER_LARGE, river_size(threshold, SMALL_RIVER)),
                SwatchShape::Line,
                small,
            ));
            swatches.push(Swatch::new(
                RIVER_LARGE,
                SwatchShape::Line,
                "River ≥ 300 000 m³/s",
            ));
        }
        if options.is_on(OUTLINES) && basins != Basins::Elevation {
            swatches.push(Swatch::new(
                DIVIDE_COLOR,
                SwatchShape::Line,
                "Drainage divide",
            ));
        }
        Legend {
            description: description.into(),
            scale: Some(depth_scale()),
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_are_stable_and_distinct() {
        assert_eq!(palette_index(1234, 10), palette_index(1234, 10));
        let used: std::collections::BTreeSet<usize> =
            (0..200).map(|end| palette_index(end, 10)).collect();
        assert_eq!(used.len(), 10);
        // The endorheic hues are not among the others.
        for color in ENDORHEIC_BASINS {
            assert!(!OCEAN_BASINS.contains(&color));
        }
    }

    #[test]
    fn lakes_darken_with_depth_and_rivers_grow() {
        let lightness = |[r, g, b]: [u8; 3]| u32::from(r) + u32::from(g) + u32::from(b);
        assert!(lightness(lake_color(1.0)) > lightness(lake_color(50.0)));
        assert!(lightness(lake_color(50.0)) > lightness(lake_color(2000.0)));
        assert_eq!(lake_color(0.5), LAKE_SHALLOW);
        assert_eq!(river_size(SMALL_RIVER, SMALL_RIVER), 0.0);
        assert!(river_size(LARGE_RIVER, SMALL_RIVER) > 0.3);
        assert_eq!(river_size(1e15, SMALL_RIVER), 1.0);
    }

    #[test]
    fn a_planet_in_hydrology() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let mut planet = crate::build_planet(&cli).unwrap();
        planet.step(cli.dt);
        let cells = cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 2);
        let mut options = OptionValues::defaults(&OPTIONS);
        for (basins, rivers) in [(0, 0), (1, 1), (2, 2)] {
            options.set(BASINS, basins);
            options.set(RIVERS, rivers);
            let data = ModeData {
                mesh,
                vertices: &vertices,
                cells: &cells,
                plates: &[],
                plate_colors: &plate_colors,
                fields: &fields,
                arrow_cells: &arrow_cells,
                arrow_spacing,
                options: &options,
            };
            let colors = Hydrology.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            for (cell, color) in cells.iter().zip(&colors) {
                if let WaterBody::Lake(depth) = cell.water_body() {
                    assert_eq!(*color, rgba(lake_color(depth)));
                } else if cell.is_ocean() && basins != 2 {
                    assert_eq!(*color, rgba(OCEAN));
                }
            }
            let mut overlay = OverlayBuilder::default();
            Hydrology.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            if rivers == 0 {
                // A planet after one step has rivers of the largest kind.
                assert!(vertices.iter().any(|v| v.color[3] > 0.0
                    && v.color[2] > v.color[0]
                    && v.color != rgba(OCEAN)));
            }
            let legend = Hydrology.legend(&options, Some(&data));
            assert!(legend.description.contains("Endorheic"));
        }
    }
}
