//! Crust: the type of the crust and its thickness (#99).
//!
//! The type refines the simulation's classes oceanic / transitional / continental (the same
//! classes as the crust age mode and the cell widget) by the composition of the basement and,
//! for felsic continental crust, by its thickness (see [`CrustType`]): normal oceanic crust;
//! transitional crust on a mafic basement (intra-oceanic arcs, thickened oceanic crust) or on a
//! felsic one (continental crust thinned at rifts and passive margins); continental crust on a
//! mafic basement (intra-oceanic arcs grown too buoyant to subduct), normal felsic continental
//! crust, and felsic crust thicker than 50 km (orogens, plateaus). The colours are those of
//! Okabe & Ito (2008), distinguishable with all common colour-vision deficiencies; the felsic
//! types go from yellow over orange to vermillion with their thickness.
//!
//! The simulation also tracks metamorphosed sediment as a share of the crust; it is not shown:
//! in 4 seeds at 300–500 Myr only ~1 % of the cells have any and ~0.2 % have more than half
//! (accretionary wedges a cell wide), and it would need a channel more in the frame history.
//!
//! The thickness (of the crystalline crust, metasediment included, without the loose sediment)
//! is shown on *viridis* (perceptually uniform, its lightness rises monotonically, readable with
//! colour-vision deficiencies and in greyscale), or as the shade of the type's colour: lighter
//! for thinner crust, darker for thicker, on a logarithmic scale so that the thin oceanic
//! crust shows its variations too. Optional isolines of the thickness.
//!
//! Everything comes from the cells every frame keeps (thickness at 10 m, the crust flags), so
//! past frames show alike.

use super::super::cell_info::{CrustType, THICK_CRUST};
use super::super::overlay::{LineStyle, OverlayBuilder, isolines};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Shading,
    Swatch, SwatchShape, coastlines, rgba,
};
use crate::viewer::CellInfo;

pub struct Crust;

/// Indices of the options.
const SHOW: usize = 0;
const ISOLINES: usize = 1;
const COASTLINES: usize = 2;

const OPTIONS: [ModeOption; 3] = [
    ModeOption {
        label: "Show",
        hover: "The type of the crust, its thickness, or both: the type's colour lighter for \
                thinner and darker for thicker crust",
        kind: OptionKind::Choice(&["Type", "Thickness", "Both"]),
    },
    ModeOption {
        label: "Isolines",
        hover: "Lines of equal crustal thickness",
        kind: OptionKind::Choice(&["Off", "10 km", "20 km"]),
    },
    ModeOption {
        label: "Coastlines",
        hover: "The coastlines; continental crust reaches beyond them where shelves are flooded",
        kind: OptionKind::Switch { default: true },
    },
];

/// What the colours show: the value of the option [`SHOW`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Show {
    Type,
    Thickness,
    Both,
}

impl Show {
    fn of(options: &OptionValues) -> Self {
        match options.get(SHOW) {
            1 => Show::Thickness,
            2 => Show::Both,
            _ => Show::Type,
        }
    }
}

/// The spacing of the isolines in km, if they are shown.
fn isoline_interval(options: &OptionValues) -> Option<f32> {
    match options.get(ISOLINES) {
        1 => Some(10.0),
        2 => Some(20.0),
        _ => None,
    }
}

/// The colour of a type (Okabe & Ito 2008).
fn type_color(crust: CrustType) -> [u8; 3] {
    match crust {
        CrustType::Metasedimentary => [86, 180, 233],
        CrustType::Oceanic => [0, 114, 178],
        CrustType::MaficTransitional => [0, 158, 115],
        CrustType::ThinnedContinental => [240, 228, 66],
        CrustType::MaficContinental => [204, 121, 167],
        CrustType::Continental => [230, 159, 0],
        CrustType::ThickContinental => [213, 94, 0],
    }
}

/// The label of a type in the legend.
fn type_label(crust: CrustType) -> &'static str {
    match crust {
        CrustType::Metasedimentary => "Metasediment (accretionary wedge)",
        CrustType::Oceanic => "Oceanic",
        CrustType::MaficTransitional => "Arc, plateau (transitional)",
        CrustType::ThinnedContinental => "Thinned continental (transitional)",
        CrustType::MaficContinental => "Oceanic arc (continental)",
        CrustType::Continental => "Continental",
        CrustType::ThickContinental => "Thick continental (> 50 km)",
    }
}

/// The thickest crust on the thickness scale (km); thicker crust gets its last colour.
const SCALE_MAX: f64 = 80.0;

/// *viridis* (matplotlib) at every eighth of its range.
const VIRIDIS: [[u8; 3]; 9] = [
    [68, 1, 84],
    [71, 44, 122],
    [59, 81, 139],
    [44, 113, 142],
    [33, 144, 141],
    [39, 173, 129],
    [92, 200, 99],
    [170, 220, 50],
    [253, 231, 37],
];

/// The thickness scale of the "Thickness" view.
fn thickness_scale() -> ColorScale {
    ColorScale {
        stops: VIRIDIS
            .iter()
            .enumerate()
            .map(|(index, &color)| (SCALE_MAX * index as f64 / 8.0, color))
            .collect(),
        title: "Crustal thickness (km)".into(),
        ticks: (0..=8)
            .map(|step| {
                let value = 10 * step;
                let label = if value == 80 {
                    "≥ 80".to_owned()
                } else {
                    value.to_string()
                };
                (f64::from(value), label.into())
            })
            .collect(),
    }
}

/// The shades of the "Both" view: from [`SHADE_THIN`] (lightest) to [`SHADE_THICK`] (darkest)
/// km, logarithmically.
const SHADE_THIN: f64 = 5.0;
const SHADE_THICK: f64 = 80.0;
/// The largest share of white (thinnest) and of black (thickest) mixed into a type's colour;
/// less black, so that the thick felsic types (orange, vermillion) stay apart.
const LIGHTEN: f64 = 0.6;
const DARKEN: f64 = 0.35;

/// The shade of a thickness (km) from −1 (thinnest, lightest) to 1 (thickest, darkest).
fn shade(thickness_km: f64) -> f64 {
    if thickness_km.is_nan() {
        return 0.0;
    }
    let t = (thickness_km.max(SHADE_THIN).ln() - SHADE_THIN.ln())
        / (SHADE_THICK.ln() - SHADE_THIN.ln());
    2.0 * t.clamp(0.0, 1.0) - 1.0
}

/// A colour mixed with white (negative shade) or black (positive shade).
fn shaded(color: [u8; 3], shade: f64) -> [u8; 3] {
    let (amount, target) = if shade < 0.0 {
        (-LIGHTEN * shade, 255.0)
    } else {
        (DARKEN * shade, 0.0)
    };
    color.map(|channel| {
        let channel = f64::from(channel);
        (channel + (target - channel) * amount).round() as u8
    })
}

/// Thicknesses (km) of the shades shown per type in the legend of the "Both" view.
const LEGEND_SHADES: [f64; 4] = [7.0, 15.0, 35.0, 70.0];

/// The legend bar of the "Both" view: the shades of a mid grey.
fn shade_scale() -> ColorScale {
    let values = [5.0, 7.0, 10.0, 15.0, 20.0, 30.0, 40.0, 50.0, 65.0, 80.0];
    ColorScale {
        stops: values
            .iter()
            .map(|&value| (value, shaded([150, 150, 150], shade(value))))
            .collect(),
        title: "Thickness (km): shade of the type's colour".into(),
        ticks: [
            (5.0, "5"),
            (20.0, "20"),
            (35.0, "35"),
            (50.0, "50"),
            (80.0, "≥ 80"),
        ]
        .into_iter()
        .map(|(value, label)| (value, label.into()))
        .collect(),
    }
}

/// The thickness of a cell's crust in km.
fn thickness_km(cell: &CellInfo) -> f32 {
    cell.crust_thickness / 1000.0
}

/// The colour of a cell.
fn color(show: Show, scale: &ColorScale, cell: &CellInfo) -> [u8; 3] {
    match show {
        Show::Type => type_color(cell.crust_type()),
        Show::Thickness => scale
            .color(f64::from(thickness_km(cell)))
            .unwrap_or(MISSING),
        Show::Both => shaded(
            type_color(cell.crust_type()),
            shade(f64::from(thickness_km(cell))),
        ),
    }
}

/// Colour of cells without a thickness.
const MISSING: [u8; 3] = [128, 128, 128];

/// Lines: widths in multiples of the arrow spacing, and colours.
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [25, 25, 25];
/// Isolines: thin dark lines with a white border, which show on light and dark colours alike;
/// thinner than the coastlines, which have no border.
const ISOLINE_WIDTH: f64 = 0.015;
const ISOLINE_COLOR: [u8; 3] = [45, 45, 45];
const ISOLINE_BORDER: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Lift of the lines above the surface (planet radii).
const LIFT: f64 = 0.003;

impl RenderMode for Crust {
    fn label(&self) -> &'static str {
        "Crust"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let show = Show::of(data.options);
        let scale = thickness_scale();
        Some(
            data.cells
                .iter()
                .map(|cell| rgba(color(show, &scale, cell)))
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
        if data.options.is_on(COASTLINES) {
            coastlines(
                data,
                overlay,
                COAST_WIDTH * spacing,
                rgba(COAST_COLOR),
                LIFT,
            );
            overlay.next_layer();
        }
        if let Some(interval) = isoline_interval(data.options) {
            let style = LineStyle {
                width: ISOLINE_WIDTH * spacing,
                border: 0.3 * ISOLINE_WIDTH * spacing,
                color: rgba(ISOLINE_COLOR),
                border_color: ISOLINE_BORDER,
                lift: LIFT,
            };
            let position = |cell: u32| data.position(cell).as_dvec3();
            let (low, high) = data
                .cells
                .iter()
                .map(thickness_km)
                .filter(|value| value.is_finite())
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), value| {
                    (low.min(value), high.max(value))
                });
            let mut level = interval;
            while level <= high {
                if level > low {
                    for [from, to] in isolines(
                        data.mesh.faces(),
                        |cell| thickness_km(&data.cells[cell as usize]),
                        level,
                        position,
                    ) {
                        overlay.line(from, to, &style);
                    }
                }
                level += interval;
            }
        }
    }

    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend {
        let show = Show::of(options);
        // The types present in the shown frame (all of them before the first one).
        let mut present = [data.is_none(); CrustType::ALL.len()];
        for cell in data.map_or(&[][..], |data| data.cells) {
            present[cell.crust_type() as usize] = true;
        }
        let types = CrustType::ALL
            .into_iter()
            .filter(|&crust| present[crust as usize]);
        let mut swatches: Vec<Swatch> = match show {
            Show::Thickness => Vec::new(),
            Show::Type => types
                .map(|crust| Swatch::fill(type_color(crust), type_label(crust)))
                .collect(),
            // Each type as a row of its shades at typical thicknesses.
            Show::Both => types
                .map(|crust| {
                    let base = type_color(crust);
                    Swatch::new(
                        base,
                        SwatchShape::Palette(
                            LEGEND_SHADES
                                .iter()
                                .map(|&thickness| shaded(base, shade(thickness)))
                                .collect(),
                        ),
                        type_label(crust),
                    )
                })
                .collect(),
        };
        if options.is_on(COASTLINES) {
            swatches.push(Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline"));
        }
        if let Some(interval) = isoline_interval(options) {
            swatches.push(Swatch::new(
                ISOLINE_COLOR,
                SwatchShape::Line,
                format!("Every {interval:.0} km"),
            ));
        }
        let description = match show {
            Show::Type => format!(
                "The type of the crust: oceanic, transitional (arcs and plateaus on oceanic \
                 crust, thinned continental margins and rifts) or continental (felsic crust, \
                 thick from {:.0} km; oceanic arcs grown too buoyant to subduct).",
                THICK_CRUST / 1000.0
            ),
            Show::Thickness => "The thickness of the crystalline crust (without loose \
                                sediment): oceanic crust ~7 km, continental ~35 km, orogens \
                                up to ~70 km."
                .to_owned(),
            Show::Both => "The type of the crust in its colour, lighter where the crust is \
                           thinner and darker where it is thicker (logarithmic; the legend \
                           shows each type at 7, 15, 35 and 70 km)."
                .to_owned(),
        };
        Legend {
            description: description.into(),
            scale: match show {
                Show::Type => None,
                Show::Thickness => Some(thickness_scale()),
                Show::Both => Some(shade_scale()),
            },
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lightness([r, g, b]: [u8; 3]) -> f64 {
        0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)
    }

    #[test]
    fn the_scales_darken_or_lighten_monotonically() {
        let scale = thickness_scale();
        for step in 1..80 {
            let t = f64::from(step);
            let (thinner, thicker) = (scale.color(t - 1.0).unwrap(), scale.color(t).unwrap());
            assert!(lightness(thicker) > lightness(thinner), "{t}");
        }
        assert_eq!(scale.range(), (0.0, SCALE_MAX));
        // The shades: lighter for thinner crust, the type's colour itself at the middle.
        for crust in CrustType::ALL {
            let base = type_color(crust);
            let mut previous = f64::INFINITY;
            for thickness in [3.0, 5.0, 7.0, 10.0, 20.0, 35.0, 50.0, 70.0, 80.0, 100.0] {
                let l = lightness(shaded(base, shade(thickness)));
                assert!(l <= previous, "{crust:?} {thickness}");
                previous = l;
            }
            assert_eq!(shaded(base, 0.0), base);
        }
        assert_eq!(shade(f64::NAN), 0.0);
        assert_eq!(shade(1.0), -1.0);
        assert_eq!(shade(200.0), 1.0);
        let shades = shade_scale();
        assert!(shades.ticks.iter().all(|&(t, _)| {
            let (low, high) = shades.range();
            t >= low && t <= high
        }));
    }

    #[test]
    fn every_type_has_its_own_colour() {
        for (index, crust) in CrustType::ALL.iter().enumerate() {
            assert!(
                CrustType::ALL[..index]
                    .iter()
                    .all(|other| type_color(*other) != type_color(*crust))
            );
        }
    }

    #[test]
    fn options_choose_the_view() {
        let mut options = OptionValues::defaults(&OPTIONS);
        assert_eq!(Show::of(&options), Show::Type);
        assert_eq!(isoline_interval(&options), None);
        assert!(options.is_on(COASTLINES));
        let legend = Crust.legend(&options, None);
        assert!(legend.scale.is_none());
        assert_eq!(legend.swatches.len(), CrustType::ALL.len() + 1);
        options.set(SHOW, 1);
        options.set(ISOLINES, 1);
        options.set(COASTLINES, 0);
        let legend = Crust.legend(&options, None);
        assert_eq!(legend.scale.unwrap().title, "Crustal thickness (km)");
        assert_eq!(legend.swatches.len(), 1);
        options.set(SHOW, 2);
        let legend = Crust.legend(&options, None);
        assert!(legend.scale.unwrap().title.contains("shade"));
        assert_eq!(legend.swatches.len(), CrustType::ALL.len() + 1);
    }

    #[test]
    fn a_planet_in_crust() {
        use super::super::super::plate_colors::PlateColors;
        use super::super::super::{Fields, arrows, cell_infos, surface_vertices};
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "3"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = surface_vertices(mesh, &cells, 40.0, 1e11);
        let plate_colors = PlateColors::default();
        let fields = Fields::default();
        let (arrow_cells, arrow_spacing) = arrows::sample_cells(mesh, 2);
        let mut options = OptionValues::defaults(&OPTIONS);
        let mut overlay_sizes = Vec::new();
        for (show, isolines) in [(0, 0), (1, 1), (2, 2)] {
            options.set(SHOW, show);
            options.set(ISOLINES, isolines);
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
            let colors = Crust.colors(&data).unwrap();
            assert_eq!(colors.len(), cells.len());
            let shown = Show::of(&options);
            for (cell, color) in cells.iter().zip(&colors) {
                assert_eq!(*color, rgba(super::color(shown, &thickness_scale(), cell)));
                if shown == Show::Type {
                    assert_eq!(*color, rgba(type_color(cell.crust_type())));
                }
            }
            // The initial planet has oceanic and continental crust; the legend lists those.
            let legend = Crust.legend(&options, Some(&data));
            if shown != Show::Thickness {
                let labels: Vec<_> = legend.swatches.iter().map(|s| s.label.clone()).collect();
                assert!(labels.contains(&type_label(CrustType::Oceanic).into()));
                assert!(labels.contains(&type_label(CrustType::Continental).into()));
            }
            let mut overlay = OverlayBuilder::default();
            Crust.overlay(&data, &mut overlay);
            let vertices = overlay.finish();
            assert_eq!(vertices.len() % 3, 0);
            let has = |color: [u8; 3]| vertices.iter().any(|vertex| vertex.color == rgba(color));
            assert!(has(COAST_COLOR));
            assert_eq!(has(ISOLINE_COLOR), isolines != 0);
            overlay_sizes.push(vertices.len());
        }
        // Isolines every 10 km are more than every 20 km, and both add to the coastlines.
        assert!(overlay_sizes[0] < overlay_sizes[2] && overlay_sizes[2] < overlay_sizes[1]);
    }
}
