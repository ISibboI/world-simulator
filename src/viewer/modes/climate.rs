//! Climate classes: the Köppen–Geiger class of every land cell (#103), in the standard colours
//! of Beck et al. (2018), or only its main group (A tropical, B arid, C temperate, D cold,
//! E polar) in colour-blind-safe colours.
//!
//! The simulation classifies the cells from its twelve monthly temperatures and the months its
//! water cycle solves (see [`crate::koppen`]) and sends the class with every cell
//! ([`CellInfo::climate_class`]); every frame keeps it, so past frames show alike. The view only
//! colours it. As in the other climate modes, the colours are shown unlit with the coastlines
//! drawn as lines. The ocean has no class.
//!
//! The standard colours are not colour-blind safe: several greens (Cfa, Cfb, Cwa), blues (Dfb,
//! Dwb, Am) and yellows (Csa, BSk) look alike without full colour vision. The main-group view
//! uses colours of the Okabe–Ito palette, which stay apart; the cell widget names the class of
//! any cell.

use super::super::overlay::{LineStyle, OverlayBuilder, category_boundaries};
use super::{
    Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Shading, Swatch,
    SwatchShape, coastlines, rgba,
};
use crate::koppen::{Group, KoppenClass};
use crate::viewer::CellInfo;
use crate::viewer::cell_info::climate_class;

pub struct Climate;

/// Indices of the options.
const DETAIL: usize = 0;
const BORDERS: usize = 1;

const OPTIONS: [ModeOption; 2] = [
    ModeOption {
        label: "Show",
        hover: "The 30 Köppen–Geiger classes in their standard colours, or only the five main \
                groups in colours that stay apart for colour-blind eyes",
        kind: OptionKind::Choice(&["Classes", "Main groups"]),
    },
    ModeOption {
        label: "Borders",
        hover: "Lines between land of different classes (or main groups)",
        kind: OptionKind::Switch { default: false },
    },
];

/// The colours of the classes, by id − 1: the legend of Beck et al. (2018).
const CLASS_COLORS: [[u8; 3]; 30] = [
    [0, 0, 255],     // Af
    [0, 120, 255],   // Am
    [70, 170, 250],  // Aw
    [255, 0, 0],     // BWh
    [255, 150, 150], // BWk
    [245, 165, 0],   // BSh
    [255, 220, 100], // BSk
    [255, 255, 0],   // Csa
    [200, 200, 0],   // Csb
    [150, 150, 0],   // Csc
    [150, 255, 150], // Cwa
    [100, 200, 100], // Cwb
    [50, 150, 50],   // Cwc
    [200, 255, 80],  // Cfa
    [100, 255, 80],  // Cfb
    [50, 200, 0],    // Cfc
    [255, 0, 255],   // Dsa
    [200, 0, 200],   // Dsb
    [150, 50, 150],  // Dsc
    [150, 100, 150], // Dsd
    [170, 175, 255], // Dwa
    [90, 120, 220],  // Dwb
    [75, 80, 180],   // Dwc
    [50, 0, 135],    // Dwd
    [0, 255, 255],   // Dfa
    [55, 200, 255],  // Dfb
    [0, 125, 125],   // Dfc
    [0, 70, 95],     // Dfd
    [178, 178, 178], // ET
    [102, 102, 102], // EF
];

/// The colours of the main groups (Okabe & Ito, 2008): blue, orange, bluish green, reddish
/// purple and grey.
const GROUP_COLORS: [[u8; 3]; 5] = [
    [0, 114, 178],
    [230, 159, 0],
    [0, 158, 115],
    [204, 121, 167],
    [160, 160, 160],
];

/// The ocean: a pale grey-blue, as on the maps of Beck et al. (white there), apart from every
/// class colour.
const OCEAN: [u8; 3] = [214, 226, 238];
/// Land without a class (no climate data yet).
const MISSING: [u8; 3] = [70, 45, 45];

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [25, 25, 25];
const BORDER_WIDTH: f64 = 0.015;
const BORDER_COLOR: [u8; 3] = [60, 60, 60];
/// Lift of the lines above the surface (planet radii).
const LIFT: f64 = 0.003;

/// What the mode shows: the value of the option [`DETAIL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detail {
    Classes,
    Groups,
}

impl Detail {
    fn of(options: &OptionValues) -> Self {
        match options.get(DETAIL) {
            1 => Detail::Groups,
            _ => Detail::Classes,
        }
    }

    /// The category of a cell: the class or main group, 0 for the ocean and land without data.
    fn category(self, cell: &CellInfo) -> u32 {
        match (self, land_class(cell)) {
            (_, None) => 0,
            (Detail::Classes, Some(class)) => u32::from(class.id()),
            (Detail::Groups, Some(class)) => class.group() as u32 + 1,
        }
    }
}

/// The class of land outside lakes: the ocean and lakes (#128) are drawn as water.
fn land_class(cell: &CellInfo) -> Option<KoppenClass> {
    if cell.is_water() {
        None
    } else {
        climate_class(cell)
    }
}

fn class_color(class: KoppenClass) -> [u8; 3] {
    CLASS_COLORS[usize::from(class.id()) - 1]
}

fn group_color(group: Group) -> [u8; 3] {
    GROUP_COLORS[group as usize]
}

/// The colour of a cell.
fn color(detail: Detail, cell: &CellInfo) -> [u8; 3] {
    match land_class(cell) {
        Some(class) => match detail {
            Detail::Classes => class_color(class),
            Detail::Groups => group_color(class.group()),
        },
        None if cell.is_water() => OCEAN,
        None => MISSING,
    }
}

/// A share as a percentage for the legend: "12 %", "<1 %".
fn percent(share: f64) -> String {
    if share > 0.0 && share < 0.005 {
        "<1 %".to_owned()
    } else {
        format!("{:.0} %", 100.0 * share)
    }
}

impl RenderMode for Climate {
    fn label(&self) -> &'static str {
        "Climate classes"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let detail = Detail::of(data.options);
        Some(
            data.cells
                .iter()
                .map(|cell| rgba(color(detail, cell)))
                .collect(),
        )
    }

    fn shading(&self) -> Shading {
        Shading::Unlit
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let spacing = data.arrow_spacing;
        if data.options.is_on(BORDERS) {
            let detail = Detail::of(data.options);
            let style = LineStyle {
                width: BORDER_WIDTH * spacing,
                border: 0.0,
                color: rgba(BORDER_COLOR),
                border_color: rgba(BORDER_COLOR),
                lift: LIFT,
            };
            let category = |cell: u32| detail.category(&data.cells[cell as usize]);
            let pieces = category_boundaries(data.mesh.faces(), category, |cell| {
                data.position(cell).as_dvec3()
            });
            // Between classified land only: the coastlines are drawn apart.
            for piece in pieces {
                if piece.sides.iter().all(|&cell| category(cell) != 0) {
                    overlay.line(piece.from, piece.to, &style);
                }
            }
            overlay.next_layer();
        }
        coastlines(
            data,
            overlay,
            COAST_WIDTH * spacing,
            rgba(COAST_COLOR),
            LIFT,
        );
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let detail = Detail::of(options);
        // Land cells of each class, and land without a class.
        let mut counts = [0usize; 30];
        let mut missing = 0;
        for cell in data.map_or(&[][..], |data| data.cells) {
            match land_class(cell) {
                Some(class) => counts[usize::from(class.id()) - 1] += 1,
                None if !cell.is_water() => missing += 1,
                None => {}
            }
        }
        let land = counts.iter().sum::<usize>().max(1) as f64;
        let mut swatches = Vec::new();
        match detail {
            Detail::Classes => {
                for class in KoppenClass::ALL {
                    let count = counts[usize::from(class.id()) - 1];
                    if count > 0 {
                        swatches.push(Swatch::new(
                            class_color(class),
                            SwatchShape::Entry,
                            format!(
                                "{} — {} ({})",
                                class.code(),
                                class.name(),
                                percent(count as f64 / land)
                            ),
                        ));
                    }
                }
            }
            Detail::Groups => {
                for group in Group::ALL {
                    let count: usize = KoppenClass::ALL
                        .into_iter()
                        .filter(|class| class.group() == group)
                        .map(|class| counts[usize::from(class.id()) - 1])
                        .sum();
                    let label = format!("{} — {}", group.letter(), group.name());
                    if data.is_none() {
                        swatches.push(Swatch::new(group_color(group), SwatchShape::Entry, label));
                    } else if count > 0 {
                        swatches.push(Swatch::new(
                            group_color(group),
                            SwatchShape::Entry,
                            format!("{label} ({})", percent(count as f64 / land)),
                        ));
                    }
                }
            }
        }
        swatches.push(Swatch::fill(OCEAN, "Ocean and lakes"));
        if missing > 0 {
            swatches.push(Swatch::fill(MISSING, "No data"));
        }
        swatches.push(Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline"));
        if options.is_on(BORDERS) {
            swatches.push(Swatch::new(
                BORDER_COLOR,
                SwatchShape::Line,
                match detail {
                    Detail::Classes => "Class border",
                    Detail::Groups => "Group border",
                },
            ));
        }
        Legend {
            description: "Köppen–Geiger climate classes of land (Beck et al., 2018) from the \
                          monthly temperatures and precipitation, with the share of land in \
                          each. Months between those the water cycle solves are interpolated, \
                          so the dry seasons are approximate. Ice sheets are EF."
                .into(),
            scale: None,
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(class: Option<KoppenClass>, elevation: f32) -> CellInfo {
        CellInfo {
            elevation,
            climate_class: class.map_or(0, |class| u32::from(class.id())),
            ..bytemuck::Zeroable::zeroed()
        }
    }

    #[test]
    fn colours_are_distinct_and_apart_from_the_ocean() {
        for (index, a) in CLASS_COLORS.iter().enumerate() {
            assert_ne!(*a, OCEAN);
            assert_ne!(*a, MISSING);
            for b in &CLASS_COLORS[index + 1..] {
                assert_ne!(a, b);
            }
        }
        for (index, a) in GROUP_COLORS.iter().enumerate() {
            assert_ne!(*a, OCEAN);
            for b in &GROUP_COLORS[index + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(class_color(KoppenClass::Af), [0, 0, 255]);
        assert_eq!(class_color(KoppenClass::EF), [102, 102, 102]);
    }

    #[test]
    fn cells_get_their_colours() {
        let cfb = cell(Some(KoppenClass::Cfb), 100.0);
        assert_eq!(color(Detail::Classes, &cfb), [100, 255, 80]);
        assert_eq!(color(Detail::Groups, &cfb), GROUP_COLORS[2]);
        assert_eq!(color(Detail::Classes, &cell(None, -100.0)), OCEAN);
        assert_eq!(color(Detail::Classes, &cell(None, 100.0)), MISSING);
        assert_eq!(Detail::Classes.category(&cfb), 15);
        assert_eq!(Detail::Groups.category(&cfb), 3);
        assert_eq!(Detail::Groups.category(&cell(None, 5.0)), 0);
        // Closed depressions below sea level are land of their class; lakes are water (#128).
        let dry = CellInfo {
            drainage: CellInfo::drainage_bits(Some(0), false, true, 0.0),
            ..cell(Some(KoppenClass::BWh), -100.0)
        };
        assert_eq!(color(Detail::Classes, &dry), class_color(KoppenClass::BWh));
        let lake = CellInfo {
            drainage: CellInfo::drainage_bits(Some(0), false, true, 20.0),
            lake_depth: 20.0,
            ..cell(Some(KoppenClass::BWh), 300.0)
        };
        assert_eq!(color(Detail::Classes, &lake), OCEAN);
        assert_eq!(Detail::Classes.category(&lake), 0);
        // Ids beyond the classes are no class.
        let mut odd = cfb;
        odd.climate_class = 77;
        assert_eq!(climate_class(&odd), None);
    }

    #[test]
    fn the_legend_lists_the_classes_present() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = super::super::super::cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let mut options = OptionValues::defaults(&OPTIONS);
        for (detail, borders) in [(0, 0), (1, 1)] {
            options.set(DETAIL, detail);
            options.set(BORDERS, borders);
            let data = ModeData {
                mesh,
                vertices: &vertices,
                cells: &cells,
                plates: &[],
                plate_colors: &plate_colors,
                fields: &fields,
                arrow_cells: &[],
                arrow_spacing: 0.1,
                options: &options,
                abiotic: false,
            };
            let colors = Climate.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            let legend = Climate.legend(&options, Some(&data));
            let entries: Vec<&Swatch> = legend
                .swatches
                .iter()
                .filter(|swatch| swatch.shape == SwatchShape::Entry)
                .collect();
            // Every colour on the planet is in the legend.
            for color in &colors {
                assert!(
                    legend
                        .swatches
                        .iter()
                        .any(|swatch| rgba(swatch.color) == *color),
                    "{color:?}"
                );
            }
            assert!(entries.len() >= 2);
            if detail == 0 {
                // Only classes present, in the order of their ids.
                assert!(entries.len() < 30);
            }
            let mut overlay = OverlayBuilder::default();
            Climate.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            assert!(vertices.iter().any(|v| v.color == rgba(COAST_COLOR)));
            assert_eq!(
                vertices.iter().any(|v| v.color == rgba(BORDER_COLOR)),
                borders == 1
            );
        }
        // Without data, the legend still describes the mode.
        let legend = Climate.legend(&OptionValues::defaults(&OPTIONS), None);
        assert!(!legend.description.is_empty());
    }

    #[test]
    fn percentages() {
        assert_eq!(percent(0.0), "0 %");
        assert_eq!(percent(0.001), "<1 %");
        assert_eq!(percent(0.123), "12 %");
    }
}
