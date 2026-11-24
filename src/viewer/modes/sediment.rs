//! Sediment: the thickness of the sediment on the bedrock, on land and on the sea floor, on a
//! logarithmic sequential scale (#100), with optional isopachs (lines of equal thickness).
//!
//! Everything comes from the cells every frame keeps (the rock-equivalent sediment thickness at
//! 1 m, see `history::codec`), so past frames show alike. The simulation keeps sediment as its
//! mass, a rock-equivalent thickness ([`CellInfo::sediment`]); the bulk thickness, with the
//! pores of the compacted column, is what seismic surveys (and isopach maps of Earth) measure.
//! It follows from the rock-equivalent thickness by the simulation's compaction law
//! ([`crate::isostasy::sediment_bulk_thickness`]), so the mode shows either at no cost in the
//! frame. As in the temperature mode, the colours are shown unlit with the coastlines drawn as
//! lines.

use glam::DVec3;

use super::super::overlay::{LineStyle, OverlayBuilder, category_boundaries, isolines};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Shading,
    Swatch, SwatchShape, rgba,
};
use crate::viewer::CellInfo;

pub struct Sediment;

/// Which thickness is shown: the value of the option [`THICKNESS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Thickness {
    /// With the pores of the compacted column.
    Bulk,
    /// The mass as solid rock.
    Rock,
}

impl Thickness {
    fn of(options: &OptionValues) -> Self {
        match options.get(THICKNESS) {
            1 => Thickness::Rock,
            _ => Thickness::Bulk,
        }
    }

    /// The thickness of a cell's sediment in metres (NaN if unknown).
    fn value(self, cell: &CellInfo) -> f32 {
        match self {
            Thickness::Rock => cell.sediment,
            Thickness::Bulk => bulk(cell.sediment),
        }
    }
}

/// The bulk thickness of rock-equivalent sediment (m), NaN for NaN.
pub fn bulk(sediment: f32) -> f32 {
    if sediment.is_nan() {
        f32::NAN
    } else {
        crate::isostasy::sediment_bulk_thickness(f64::from(sediment)) as f32
    }
}

/// Indices of the options.
const THICKNESS: usize = 0;
const ISOPACHS: usize = 1;

const OPTIONS: [ModeOption; 2] = [
    ModeOption {
        label: "Thickness",
        hover: "Bulk: the thickness of the compacted sediment with its pores, as seismic surveys \
                measure it. As rock: its mass as a layer of a fixed density of 2050 kg/m³, \
                what the simulation keeps; thinner than the bulk where the sediment is porous, \
                thicker than the bulk for columns over about 4.4 km, which are compacted to a \
                higher density",
        kind: OptionKind::Choice(&["Bulk", "As rock"]),
    },
    ModeOption {
        label: "Isopachs",
        hover: "Lines of equal sediment thickness: 0.1, 1 and 10 km (wider for thicker)",
        kind: OptionKind::Switch { default: false },
    },
];

/// Colours: the 9 colours of ColorBrewer's YlOrBr as stops of a continuous scale (sequential, colour-blind safe, lightness falling
/// monotonically from pale yellow to dark brown), evenly spread on a logarithmic axis over
/// [`RANGE`].
const YL_OR_BR: [[u8; 3]; 9] = [
    [255, 255, 229],
    [255, 247, 188],
    [254, 227, 145],
    [254, 196, 79],
    [254, 153, 41],
    [236, 112, 20],
    [204, 76, 2],
    [153, 52, 4],
    [102, 37, 6],
];

/// The ends of the scale in metres; values beyond are clamped. Less than 10 m is bare rock in
/// all but name (young ocean floor at the ridges, eroding mountains); the thickest basins of
/// Earth hold 15–20 km.
const RANGE: (f64, f64) = (10.0, 20_000.0);
/// Ticks in metres with their labels in km (none at the end, which would crowd the 10 on
/// phones).
const TICKS: [(f64, &str); 6] = [
    (10.0, "0.01"),
    (100.0, "0.1"),
    (300.0, "0.3"),
    (1000.0, "1"),
    (3000.0, "3"),
    (10_000.0, "10"),
];

/// The isopachs: thickness (m) and line width in multiples of the arrow spacing.
/// The lines are drawn on the logarithmic axis of the colours, so that they follow the colour
/// steps.
const ISOPACH_LEVELS: [(f32, f64); 3] = [(100.0, 0.02), (1000.0, 0.03), (10_000.0, 0.045)];

/// Colour of cells without data.
const MISSING: [u8; 3] = [128, 128, 128];

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
/// The coastline in a blue that none of the browns and yellows has, with a thin white border
/// that keeps it visible on the dark browns of deltas and shelves along the coasts.
const COAST_COLOR: [u8; 3] = [0, 90, 210];
/// Dark isopachs with a white border, which stand out on the pale and the dark end alike.
const ISOPACH_COLOR: [u8; 3] = [20, 20, 20];
const BORDER_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Lift of the lines above the surface (planet radii).
const LIFT: f64 = 0.003;

/// The scale (log10 of metres) for the shown thickness.
fn scale(thickness: Thickness) -> ColorScale {
    let log = crate::math::log10;
    let (low, high) = (log(RANGE.0), log(RANGE.1));
    let last = YL_OR_BR.len() - 1;
    ColorScale {
        stops: YL_OR_BR
            .iter()
            .enumerate()
            .map(|(index, &color)| (low + (high - low) * index as f64 / last as f64, color))
            .collect(),
        title: match thickness {
            Thickness::Bulk => "Sediment thickness, bulk (km, logarithmic)",
            Thickness::Rock => "Sediment thickness as rock (km, logarithmic)",
        }
        .into(),
        ticks: TICKS
            .into_iter()
            .map(|(value, label)| (log(value), label.into()))
            .collect(),
    }
}

/// The value on the axis of the scale: log10 of metres, NaN where unknown; no sediment lies at
/// the thin end.
fn axis(thickness: f32) -> f32 {
    if thickness.is_nan() {
        f32::NAN
    } else {
        thickness.max(1e-3).log10()
    }
}

impl RenderMode for Sediment {
    fn label(&self) -> &'static str {
        "Sediment"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let thickness = Thickness::of(data.options);
        Some(scale(thickness).colors(
            data.cells.iter().map(|cell| axis(thickness.value(cell))),
            MISSING,
        ))
    }

    fn shading(&self) -> Shading {
        Shading::Unlit
    }

    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if data.cells.len() != data.mesh.len() {
            return;
        }
        let spacing = data.arrow_spacing;
        let position = |cell: u32| -> DVec3 { data.position(cell).as_dvec3() };
        let coast = LineStyle {
            width: COAST_WIDTH * spacing,
            border: 0.25 * COAST_WIDTH * spacing,
            color: rgba(COAST_COLOR),
            border_color: BORDER_COLOR,
            lift: LIFT,
        };
        for piece in category_boundaries(
            data.mesh.faces(),
            |cell| u32::from(data.cells[cell as usize].elevation >= 0.0),
            position,
        ) {
            overlay.line(piece.from, piece.to, &coast);
        }
        if !data.options.is_on(ISOPACHS) {
            return;
        }
        overlay.next_layer();
        let thickness = Thickness::of(data.options);
        let values: Vec<f32> = data
            .cells
            .iter()
            .map(|cell| axis(thickness.value(cell)))
            .collect();
        for (level, width) in ISOPACH_LEVELS {
            let style = LineStyle {
                width: width * spacing,
                border: 0.3 * width * spacing,
                color: rgba(ISOPACH_COLOR),
                border_color: BORDER_COLOR,
                lift: LIFT,
            };
            for [from, to] in isolines(
                data.mesh.faces(),
                |cell| values[cell as usize],
                level.log10(),
                position,
            ) {
                overlay.line(from, to, &style);
            }
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let thickness = Thickness::of(options);
        let mut swatches = vec![Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline")];
        if data.is_some_and(|data| data.cells.iter().any(|cell| cell.sediment.is_nan())) {
            swatches.insert(0, Swatch::fill(MISSING, "No data"));
        }
        if options.is_on(ISOPACHS) {
            swatches.push(Swatch::new(
                ISOPACH_COLOR,
                SwatchShape::Line,
                "Isopachs: 0.1, 1, 10 km",
            ));
        }
        let shown = match thickness {
            Thickness::Bulk => "bulk, with the pores of the compacted sediment",
            Thickness::Rock => "as rock: its mass as a layer of 2050 kg/m³",
        };
        Legend {
            description: format!(
                "Thickness of the sediment on the bedrock, on land and on the sea floor, \
                 {shown}. It is thick in deltas and fans, on shelves and passive margins, in \
                 enclosed basins and in accretionary wedges at trenches, and thin on young ocean \
                 floor near the ridges and on eroding mountains."
            )
            .into(),
            scale: Some(scale(thickness)),
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_sequential_and_logarithmic() {
        let scale = scale(Thickness::Bulk);
        let lightness = |metres: f64| {
            let [r, g, b] = scale
                .color(crate::math::log10(metres))
                .unwrap()
                .map(f64::from);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        // Lightness falls monotonically from thin to thick.
        let mut metres = 10.0;
        while metres < 20_000.0 {
            let next = metres * 1.2;
            assert!(lightness(next) < lightness(metres), "{metres}");
            metres = next;
        }
        // Each decade takes the same share of the bar; the ends clamp.
        assert_eq!(scale.color(f64::from(axis(0.0))), Some(YL_OR_BR[0]));
        assert_eq!(scale.color(f64::from(axis(5.0))), Some(YL_OR_BR[0]));
        assert_eq!(scale.color(f64::from(axis(1e6))), Some(YL_OR_BR[8]));
        assert!(axis(f32::NAN).is_nan());
        let (low, high) = scale.range();
        assert!(
            scale
                .ticks
                .iter()
                .all(|&(t, _)| t >= low - 1e-9 && t <= high + 1e-9)
        );
    }

    #[test]
    fn bulk_grows_with_rock_and_is_thicker_only_while_porous() {
        assert_eq!(bulk(0.0), 0.0);
        assert!(bulk(f32::NAN).is_nan());
        let mut previous = 0.0;
        for rock in [10.0, 100.0, 1000.0, 3000.0, 8000.0, 15_000.0] {
            let bulk = bulk(rock);
            assert!(bulk > previous, "{rock} {bulk}");
            previous = bulk;
        }
        // Thin, porous layers are thicker than their rock equivalent (at the density of the
        // grains); thick, compacted columns thinner.
        assert!(bulk(100.0) > 100.0 && bulk(1000.0) > 1000.0);
        assert!(bulk(15_000.0) < 15_000.0);
    }

    #[test]
    fn options_choose_thickness_and_lines() {
        let mut options = OptionValues::defaults(&OPTIONS);
        assert_eq!(Thickness::of(&options), Thickness::Bulk);
        assert!(!options.is_on(ISOPACHS));
        assert_eq!(Sediment.legend(&options, None).swatches.len(), 1);
        options.set(THICKNESS, 1);
        options.set(ISOPACHS, 1);
        assert_eq!(Thickness::of(&options), Thickness::Rock);
        let legend = Sediment.legend(&options, None);
        assert!(legend.scale.unwrap().title.contains("as rock"));
        assert_eq!(legend.swatches.len(), 2);
    }

    #[test]
    fn a_planet_in_sediment() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = cell_infos(&planet);
        assert!(cells.iter().any(|cell| cell.sediment > 100.0));
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 2);
        let mut options = OptionValues::defaults(&OPTIONS);
        let mut overlay_sizes = Vec::new();
        let mut all_colors = Vec::new();
        for (thickness, isopachs) in [(0, 0), (0, 1), (1, 0)] {
            options.set(THICKNESS, thickness);
            options.set(ISOPACHS, isopachs);
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
            let colors = Sediment.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            let shown = Thickness::of(&options);
            for (cell, color) in cells.iter().zip(&colors) {
                let expected = scale(shown)
                    .color(f64::from(axis(shown.value(cell))))
                    .unwrap();
                assert_eq!(*color, rgba(expected));
            }
            let mut overlay = OverlayBuilder::default();
            Sediment.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            assert_eq!(vertices.len() % 3, 0);
            let has = |color: [u8; 3]| vertices.iter().any(|vertex| vertex.color == rgba(color));
            assert!(has(COAST_COLOR));
            assert_eq!(has(ISOPACH_COLOR), isopachs == 1);
            overlay_sizes.push(vertices.len());
            all_colors.push(colors);
        }
        // The isopachs add lines; bulk and rock-equivalent thickness colour differently.
        assert!(overlay_sizes[0] < overlay_sizes[1]);
        assert_ne!(all_colors[0], all_colors[2]);
    }
}
